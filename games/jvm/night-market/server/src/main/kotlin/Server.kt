package nightmarket.server

import com.getlingara.embed.server.kotlin.EmbedServer
import com.getlingara.embed.server.kotlin.MemoryPlayerDirectory
import com.getlingara.embed.server.kotlin.PlayerRouter
import com.getlingara.embed.server.kotlin.RoutedEvent
import com.getlingara.embed.server.kotlin.tokenHandoff
import com.getlingara.kotlin.LingaraClient
import com.getlingara.kotlin.events.PracticeCompleted
import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpHandler
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.runBlocking
import java.net.InetSocketAddress
import java.net.URI
import kotlin.system.exitProcess

// The Night Market's server half, the same contract as the Bevy one's:
//
// - POST /token (session): mints the player's Lingara token, no origin and no
//   scopes, and answers exactly {"token":…,"expires_at":…}.
// - POST /webhook (no session, mounted only with a webhook secret): verifies
//   Lingara's delivery, routes it to its player, and gives one lantern per
//   practice.completed event id. Empty body, the router's status.
// - GET /rewards (session): {"lanterns":N}, a running total that is never
//   consumed, so a repeated poll cannot add one.
//
// The session is a stand-in: the X-Night-Market-Player header, used as the
// player_ref. A real game server authenticates its player first. The kit's
// calls suspend, and run under runBlocking on the request thread: an example's
// simplification, not a production server's.

const val PLAYER_HEADER = "X-Night-Market-Player"
const val DEFAULT_PORT = 8787

/** Where the server's Lingara credentials and URLs come from. Nothing here is ever printed. */
data class ServerConfig(
    /** A metered client holding `embed:mint`. */
    val clientId: String,
    val clientSecret: String,
    /** `lgr_whsec_…`; without it `/webhook` is not mounted. */
    val webhookSecret: String? = null,
    val baseUrl: String? = null,
    val tokenUrl: String? = null,
) {
    override fun toString(): String = "ServerConfig(webhook=${webhookSecret != null}, baseUrl=$baseUrl, tokenUrl=$tokenUrl)"

    fun client(): LingaraClient {
        // Named apart: inside the builder, `baseUrl` and `tokenUrl` are the builder's.
        val (base, token) = baseUrl to tokenUrl
        return LingaraClient {
            clientCredentials(clientId, clientSecret)
            userAgentSuffix = "night-market-server-kotlin/0.1"
            base?.let { baseUrl = URI.create(it) }
            token?.let { tokenUrl = URI.create(it) }
        }
    }

    companion object {
        /** `LINGARA_CLIENT_ID`, `LINGARA_CLIENT_SECRET` (required), `LINGARA_WEBHOOK_SECRET`, `LINGARA_BASE_URL`, `LINGARA_TOKEN_URL`. */
        fun fromEnv(): ServerConfig {
            fun optional(name: String) = System.getenv(name)?.takeIf { it.isNotEmpty() }
            fun required(name: String) = optional(name) ?: throw IllegalStateException("set $name")
            return ServerConfig(
                required("LINGARA_CLIENT_ID"),
                required("LINGARA_CLIENT_SECRET"),
                optional("LINGARA_WEBHOOK_SECRET"),
                optional("LINGARA_BASE_URL"),
                optional("LINGARA_TOKEN_URL"),
            )
        }
    }
}

/** Each player's lantern total, and every event id already rewarded. */
class Rewards {
    private val lanterns = mutableMapOf<String, Int>()
    private val rewarded = mutableSetOf<String>()

    @Synchronized
    fun total(player: String): Int = lanterns[player] ?: 0

    /** One lantern for an event id's first delivery; a redelivery adds none. */
    @Synchronized
    fun award(eventId: String, player: String) {
        if (rewarded.add(eventId)) lanterns[player] = total(player) + 1
    }
}

/**
 * The routes over one directory, shared by the mint and the router: minting
 * remembers each player's subject, and that is how a webhook finds its player.
 */
fun routes(config: ServerConfig, directory: MemoryPlayerDirectory): Map<String, HttpHandler> {
    val embed = EmbedServer(config.client(), directory)
    val rewards = Rewards()
    val routes = mutableMapOf("/token" to token(embed), "/rewards" to rewards(rewards))
    config.webhookSecret?.let { routes["/webhook"] = webhook(PlayerRouter(directory, it), rewards) }
    return routes
}

/** Serves [routes] on 127.0.0.1:[port]; port 0 picks a free one. */
fun serve(port: Int, routes: Map<String, HttpHandler>): HttpServer {
    val server = HttpServer.create(InetSocketAddress("127.0.0.1", port), 0)
    // A context matches by prefix, so each handler is held to its exact path.
    routes.forEach { (path, handler) -> server.createContext(path) { ex -> if (ex.requestURI.path == path) handler.handle(ex) else respond(ex, 404) } }
    server.start()
    return server
}

private fun token(embed: EmbedServer) =
    HttpHandler { ex ->
        val player = session(ex)
        when {
            ex.requestMethod != "POST" -> respond(ex, 405)
            player == null -> respond(ex, 401)
            else ->
                runCatching { runBlocking { embed.mintToken(player) } }.fold(
                    { respond(ex, 200, tokenHandoff(it), "Cache-Control" to "no-store") },
                    {
                        System.err.println("minting for $player failed: $it")
                        respond(ex, 502)
                    },
                )
        }
    }

/** A finished practice for a player this server minted for is a lantern; every other event changes nothing. */
private fun webhook(router: PlayerRouter, rewards: Rewards) =
    HttpHandler { ex ->
        if (ex.requestMethod != "POST") return@HttpHandler respond(ex, 405)
        val body = ex.requestBody.readBytes()
        val status =
            runBlocking {
                router.handle(body, ex.requestHeaders) { routed: RoutedEvent ->
                    val player = routed.playerRef
                    if (routed.event is PracticeCompleted && player != null) rewards.award(routed.event.id, player)
                }
            }
        respond(ex, status)
    }

private fun rewards(rewards: Rewards) =
    HttpHandler { ex ->
        val player = session(ex)
        when {
            ex.requestMethod != "GET" -> respond(ex, 405)
            player == null -> respond(ex, 401)
            else -> respond(ex, 200, "{\"lanterns\":${rewards.total(player)}}")
        }
    }

/** The stand-in session: a non-empty `X-Night-Market-Player`. */
private fun session(ex: HttpExchange): String? = ex.requestHeaders.getFirst(PLAYER_HEADER)?.trim()?.takeIf { it.isNotEmpty() }

private fun respond(ex: HttpExchange, status: Int, json: String? = null, vararg headers: Pair<String, String>) {
    headers.forEach { (name, value) -> ex.responseHeaders.add(name, value) }
    if (json == null) {
        ex.sendResponseHeaders(status, -1)
    } else {
        val bytes = json.toByteArray()
        ex.responseHeaders.add("Content-Type", "application/json")
        ex.sendResponseHeaders(status, bytes.size.toLong())
        ex.responseBody.use { it.write(bytes) }
    }
    ex.close()
}

/**
 *     ./gradlew :server:run
 *     ./gradlew :server:run --args="delete-player <player_ref>"
 */
fun main(args: Array<String>) {
    val config = runCatching { ServerConfig.fromEnv() }.getOrElse { fail(it.message) }
    when (args.toList()) {
        emptyList<String>(), listOf("serve") -> {
            if (config.webhookSecret == null) println("LINGARA_WEBHOOK_SECRET is not set: /webhook is off, and no practice earns a lantern")
            serve(DEFAULT_PORT, routes(config, MemoryPlayerDirectory()))
            println("the Night Market's server is listening on http://127.0.0.1:$DEFAULT_PORT")
        }
        else ->
            if (args.size == 2 && args[0] == "delete-player") {
                // Deletes the player at Lingara, revoking their tokens.
                runCatching { runBlocking { EmbedServer(config.client()).deletePlayer(args[1]) } }.getOrElse { fail(it.toString()) }
                println("deleted ${args[1]}")
            } else {
                fail("usage: night-market-server [serve] | delete-player <player_ref>")
            }
    }
}

private fun fail(message: String?): Nothing {
    System.err.println("night-market-server: $message")
    exitProcess(1)
}
