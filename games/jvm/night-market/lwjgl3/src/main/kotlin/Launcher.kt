package nightmarket

import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Application
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3ApplicationConfiguration

/**
 * The Night Market on the desktop. Start the server half first
 * (`./gradlew :server:run`), then `./gradlew :lwjgl3:run`.
 *
 * `NIGHT_MARKET_SERVER` (default `http://127.0.0.1:8787`) is the server's
 * base URL, and `NIGHT_MARKET_PLAYER` (default `player-1`) stands in for the
 * player's session: it is sent as `X-Night-Market-Player` on the token call
 * and on the rewards poll. The game holds no secret.
 */
fun main() {
    val server = System.getenv("NIGHT_MARKET_SERVER") ?: "http://127.0.0.1:8787"
    val player = System.getenv("NIGHT_MARKET_PLAYER") ?: "player-1"
    val config =
        Lwjgl3ApplicationConfiguration().apply {
            setTitle("The Night Market")
            setWindowedMode(1100, 640)
        }
    Lwjgl3Application(MarketScreen(server, player), config)
}
