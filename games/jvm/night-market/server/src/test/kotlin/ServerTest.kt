package nightmarket.server

import com.getlingara.embed.server.kotlin.MemoryPlayerDirectory
import com.sun.net.httpserver.HttpServer
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import java.net.InetSocketAddress
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.util.Base64
import java.util.concurrent.CopyOnWriteArrayList
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/**
 * The server's routes, offline: the real handlers on a local port, over an
 * in-process stub of Lingara's two endpoints. No network, no secret, and no
 * kit under test: the stub answers what the library expects, and the webhook
 * is signed here the way Lingara signs one (Standard Webhooks, HMAC-SHA256).
 */
class ServerTest {
    private val mints = CopyOnWriteArrayList<String>()
    private val lingara = stub()
    private val lingaraUrl = "http://127.0.0.1:${lingara.address.port}"
    private val config =
        ServerConfig(
            clientId = "lgr_ci_test",
            clientSecret = "lgr_cs_test",
            webhookSecret = "lgr_whsec_" + Base64.getEncoder().encodeToString(WEBHOOK_KEY),
            baseUrl = lingaraUrl,
            tokenUrl = "$lingaraUrl/oauth/token",
        )
    private val server = serve(0, routes(config, MemoryPlayerDirectory()))
    private val http = HttpClient.newHttpClient()

    @AfterEach
    fun stop() {
        server.stop(0)
        lingara.stop(0)
    }

    /** Lingara's token and mint endpoints, recording each mint's body. */
    private fun stub(): HttpServer {
        val stub = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        stub.createContext("/oauth/token") { ex -> answer(ex, """{"access_token":"lgr_at_stub","token_type":"Bearer","expires_in":3600}""") }
        stub.createContext("/v1/embed/tokens") { ex ->
            mints += ex.requestBody.readBytes().decodeToString()
            answer(ex, """{"token":"lgr_et_stub","expires_at":"2026-10-13T12:00:00Z","expires_in":900,"subject":"$SUBJECT","scopes":["embed:play"],"account_linked":false}""")
        }
        stub.start()
        return stub
    }

    private fun answer(ex: com.sun.net.httpserver.HttpExchange, json: String) {
        val bytes = json.toByteArray()
        ex.responseHeaders.add("Content-Type", "application/json")
        ex.sendResponseHeaders(200, bytes.size.toLong())
        ex.responseBody.use { it.write(bytes) }
    }

    /** One request to the server: its status and its body. */
    private fun call(method: String, path: String, player: String? = PLAYER, body: ByteArray = ByteArray(0), headers: Map<String, String> = emptyMap()): Pair<Int, String> {
        val request = HttpRequest.newBuilder(URI.create("http://127.0.0.1:${server.address.port}$path")).method(method, HttpRequest.BodyPublishers.ofByteArray(body))
        player?.let { request.header(PLAYER_HEADER, it) }
        headers.forEach { (name, value) -> request.header(name, value) }
        val response = http.send(request.build(), HttpResponse.BodyHandlers.ofString())
        return response.statusCode() to response.body()
    }

    // 13.10.26i AC5
    /**
     * `POST /token` with the session mints for that player with no origin and
     * answers exactly `token` and `expires_at`; without it, `401` and the stub
     * sees no mint.
     */
    @Test
    fun `the handoff needs a session and is token and expiry only`() {
        assertEquals(200 to """{"token":"lgr_et_stub","expires_at":"2026-10-13T12:00:00Z"}""", call("POST", "/token"))
        val sent = mints.single()
        assertTrue(sent.contains("\"player_ref\":\"$PLAYER\""), sent)
        assertFalse(sent.contains("origin") || sent.contains("scopes"), "no origin, no scopes: $sent")

        assertEquals(401, call("POST", "/token", player = null).first)
        assertEquals(1, mints.size, "no session, no mint")
    }

    /** A `practice.completed` for the stub's subject, and its Standard Webhooks headers, signed now. */
    private fun signedResult(id: String): Pair<ByteArray, Map<String, String>> {
        val body =
            """{"id":"$id","type":"practice.completed","created_at":"2026-10-13T09:00:00Z","api_version":"2026-09-equipped-boxfish","subject":"$SUBJECT",""" +
                """"data":{"plan_id":"plan-1","completed":true,"items":[{"set_number":1,"question":1,"outcome":"correct"}]}}"""
        val timestamp = (System.currentTimeMillis() / 1000).toString()
        val mac = Mac.getInstance("HmacSHA256").apply { init(SecretKeySpec(WEBHOOK_KEY, "HmacSHA256")) }
        val signature = "v1," + Base64.getEncoder().encodeToString(mac.doFinal("$id.$timestamp.$body".toByteArray()))
        return body.toByteArray() to mapOf("webhook-id" to id, "webhook-timestamp" to timestamp, "webhook-signature" to signature)
    }

    private fun lanterns(): String = call("GET", "/rewards").let { (status, body) -> assertEquals(200, status); body }

    // 13.10.26i AC6
    /**
     * After a mint, a signed `practice.completed` for that subject is `204` and
     * earns one lantern; the same delivery again earns none; a body with one
     * byte changed is `401` and earns none; `GET /rewards` needs the session.
     */
    @Test
    fun `a signed result rewards its player once`() {
        assertEquals("""{"lanterns":0}""", lanterns())
        call("POST", "/token")

        val (body, headers) = signedResult("lgr_evt_night_market_1")
        assertEquals(204, call("POST", "/webhook", null, body, headers).first)
        assertEquals("""{"lanterns":1}""", lanterns())

        assertEquals(204, call("POST", "/webhook", null, body, headers).first, "a redelivery is acknowledged")
        assertEquals("""{"lanterns":1}""", lanterns(), "and earns nothing")

        val tampered = body.copyOf()
        tampered[tampered.indexOf('1'.code.toByte())] = '2'.code.toByte()
        assertEquals(401, call("POST", "/webhook", null, tampered, headers).first)
        assertEquals("""{"lanterns":1}""", lanterns())

        assertEquals(401, call("GET", "/rewards", player = null).first)
    }

    private companion object {
        const val PLAYER = "player-1"
        const val SUBJECT = "lgr_sub_night_market_test"
        val WEBHOOK_KEY = ByteArray(32) { 7 }
    }
}
