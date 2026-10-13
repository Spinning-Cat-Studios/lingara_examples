package nightmarket

import com.getlingara.embed.Command
import com.getlingara.embed.CommandOutcome
import com.getlingara.embed.LingaraEmbed
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.time.Duration

/** The stand-in session header the server reads as the `player_ref`. */
const val PLAYER_HEADER = "X-Night-Market-Player"

/**
 * Everything the game asks of the outside world: the kit's commands, and the
 * server's lantern total. [MarketLogic] is the only caller of [execute]; the
 * test backs it with a fake that records each `Command`.
 */
interface MarketPort {
    /** One kit command, answered as the kit answers it. */
    fun execute(command: Command): CommandOutcome

    /** The player's running lantern total, from `GET <server>/rewards`. */
    fun rewards(): Int
}

/** What the screen turns input into. [MarketLogic] reads these and nothing else from the view. */
sealed interface MarketIntent {
    data class WalkTo(val stall: Stall) : MarketIntent

    /** A line the player typed, for the keeper at the current stall. */
    data class SayLine(val text: String) : MarketIntent

    /** `+1` or `-1` to the level of the keeper at the current stall. */
    data class ChangeLevel(val delta: Int) : MarketIntent

    data object BeginPractice : MarketIntent

    data class PracticeKey(val action: PracticeAction) : MarketIntent

    /** The server's running lantern total for this player. */
    data class RewardsPolled(val total: Int) : MarketIntent
}

/** One key in a practice, as the kit's practice calls take it. */
sealed interface PracticeAction {
    data class Answer(val text: String) : PracticeAction

    data class Choose(val option: Int) : PracticeAction

    data object Reveal : PracticeAction

    data class SelfGrade(val correct: Boolean) : PracticeAction

    data object Next : PracticeAction

    data object Finish : PracticeAction
}

/**
 * The live port: the kit for commands, and the server for rewards. The poll
 * runs off the render thread with a short timeout, and throws when the server
 * cannot be reached or answers anything but a total.
 */
class LiveMarketPort(
    private val lingara: LingaraEmbed,
    server: String,
    private val player: String,
) : MarketPort {
    private val rewardsUrl = URI.create("${server.trimEnd('/')}/rewards")
    private val http = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build()

    override fun execute(command: Command): CommandOutcome = lingara.execute(command)

    override fun rewards(): Int {
        val request = HttpRequest.newBuilder(rewardsUrl).header(PLAYER_HEADER, player).timeout(Duration.ofSeconds(10)).GET().build()
        val response = http.send(request, HttpResponse.BodyHandlers.ofString())
        return parseLanterns(response.body()) ?: error("GET $rewardsUrl answered ${response.statusCode()}")
    }
}

/**
 * The total in a `/rewards` body, or null for anything else. The body is one
 * known key, so a split is enough and the game needs no JSON library.
 */
fun parseLanterns(body: String): Int? {
    val digits = body.substringAfter("\"lanterns\":", missingDelimiterValue = "").trimStart()
    return digits.takeWhile(Char::isDigit).toIntOrNull()
}
