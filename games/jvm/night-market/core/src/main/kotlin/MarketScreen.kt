package nightmarket

import com.badlogic.gdx.ApplicationAdapter
import com.badlogic.gdx.Gdx
import com.badlogic.gdx.Input.Keys
import com.badlogic.gdx.InputAdapter
import com.badlogic.gdx.graphics.Color
import com.badlogic.gdx.graphics.g2d.BitmapFont
import com.badlogic.gdx.graphics.g2d.SpriteBatch
import com.badlogic.gdx.graphics.g2d.freetype.FreeTypeFontGenerator
import com.badlogic.gdx.graphics.glutils.ShapeRenderer
import com.badlogic.gdx.utils.ScreenUtils
import com.getlingara.embed.EmbedConfig
import com.getlingara.embed.Header
import com.getlingara.embed.LingaraEmbed
import com.getlingara.embed.PracticePhase
import com.getlingara.embed.Secret
import com.getlingara.embed.TokenSourceConfig
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/**
 * The view: draws [MarketLogic]'s state and turns the keyboard and the
 * rewards poll into [MarketIntent]s. It holds no game state; the typed line
 * and the poll's plumbing are its own, and everything else it reads.
 *
 * Keys: ← → walk, Enter types a line (Enter again sends it, Esc drops it),
 * + - change the keeper's level, P practises. In a practice: 1–4 choose,
 * Space reveals or goes on, Y N grade yourself, Esc finishes early, and a
 * typed line is an answer.
 */
class MarketScreen(
    private val server: String,
    private val player: String,
) : ApplicationAdapter() {
    private lateinit var lingara: LingaraEmbed
    private lateinit var port: MarketPort
    private lateinit var market: MarketLogic
    private lateinit var batch: SpriteBatch
    private lateinit var shapes: ShapeRenderer
    private lateinit var generator: FreeTypeFontGenerator
    private lateinit var font: BitmapFont
    private var typing: String? = null

    // GET /rewards off the render thread: once at startup, then every 5 s
    // while a result is awaited. Answers queue here for the next frame.
    private val polls = Executors.newSingleThreadExecutor { Thread(it, "night-market-rewards").apply { isDaemon = true } }
    private val polled = ConcurrentLinkedQueue<Int>()
    private val inFlight = AtomicBoolean(false)
    private var sincePoll = POLL_EVERY_SECS
    private var started = false

    override fun create() {
        // The token comes from the server, which mints it for the stand-in
        // session; the stream stays paused, because the reward arrives by
        // webhook at the server and the game reads no stream event.
        val session = Header(PLAYER_HEADER, Secret(player))
        val config =
            EmbedConfig(
                tokenSource = TokenSourceConfig.Http("${server.trimEnd('/')}/token", listOf(session)),
                streamOnStart = false,
                userAgentSuffix = "night-market-libgdx/0.1",
            )
        lingara = LingaraEmbed.start(config)
        port = LiveMarketPort(lingara, server, player)
        market = MarketLogic(port)
        batch = SpriteBatch()
        shapes = ShapeRenderer()
        font = cjkFont()
        Gdx.input.inputProcessor = KeyInput()
    }

    /**
     * libGDX's default font has no Chinese or Japanese glyphs, so the replies
     * would be boxes: the bundled OFL font (`fonts/`), glyphs made as drawn.
     */
    private fun cjkFont(): BitmapFont {
        generator = FreeTypeFontGenerator(Gdx.files.classpath(FONT))
        val parameter = FreeTypeFontGenerator.FreeTypeFontParameter().apply {
            size = 20
            incremental = true
        }
        return generator.generateFont(parameter)
    }

    override fun render() {
        lingara.drain(LingaraEmbed.DEFAULT_DRAIN) { market.onEvent(it) }
        generateSequence { polled.poll() }.forEach { market.handle(MarketIntent.RewardsPolled(it)) }
        val delta = Gdx.graphics.deltaTime
        market.update(delta)
        poll(delta)

        ScreenUtils.clear(0.08f, 0.07f, 0.12f, 1f)
        drawStreet()
        batch.begin()
        Stall.entries.forEach { font.draw(batch, it.keeper?.name ?: "Lanterns", stallX(it) - 40f, STALL_Y + 170f) }
        panel().forEachIndexed { n, line -> font.draw(batch, line, 16f, PANEL_TOP - n * 24f) }
        batch.end()
    }

    override fun dispose() {
        polls.shutdownNow()
        lingara.close()
        batch.dispose()
        shapes.dispose()
        font.dispose()
        generator.dispose()
    }

    private fun poll(delta: Float) {
        sincePoll += delta
        val due = sincePoll >= POLL_EVERY_SECS && (!started || market.practice is Practice.AwaitingResult)
        if (!due || !inFlight.compareAndSet(false, true)) return
        started = true
        sincePoll = 0f
        polls.execute {
            runCatching { port.rewards() }.onSuccess { polled.add(it) }
            inFlight.set(false)
        }
    }

    private fun stallX(stall: Stall): Float = Gdx.graphics.width / 2f + (stall.ordinal - 1) * 320f

    private fun drawStreet() {
        shapes.begin(ShapeRenderer.ShapeType.Filled)
        for (stall in Stall.entries) {
            shapes.color = STALL_COLOURS.getValue(stall)
            shapes.rect(stallX(stall) - 100f, STALL_Y, 200f, 140f)
        }
        shapes.color = Color.WHITE
        shapes.rect(stallX(market.at) - 14f, STALL_Y - 60f, 28f, 28f)
        shapes.end()
    }

    private fun panel(): List<String> {
        val lines = mutableListOf("The Night Market · lanterns: ${market.lanterns}", market.at.scene)
        val keeper = market.keeperHere
        if (keeper == null) {
            lines += "Nobody minds this stall. Walk on: ← →"
        } else {
            lines += "${keeper.spec.name} speaks ${keeper.spec.targetLang} at level ${keeper.level}"
            lines += keeper.log.takeLast(LOG_LINES).map { "  $it" }
            if (market.practice is Practice.Running) lines += practiceLines()
        }
        if (market.practice is Practice.AwaitingResult) lines += "Waiting for your result…"
        market.lastError?.let { lines += "error: $it" }
        lines += typing?.let { "> ${it}_" } ?: "← → walk · Enter talk · + - level · P practise"
        return lines
    }

    private fun practiceLines(): List<String> {
        val s = market.snapshot ?: return listOf("practice: starting…")
        val question = s.question
        return listOfNotNull(
            "practice: ${s.phase}, question ${s.questionIndex + 1}/${s.questionCount}",
            question?.prompt?.let { "  $it" },
            question?.options?.mapIndexed { i, o -> "  ${i + 1}. $o" }?.joinToString("   "),
            "1-4 choose · Space reveal/next · Y N grade · Enter answer · Esc finish",
        )
    }

    private inner class KeyInput : InputAdapter() {
        private val practising get() = market.practice is Practice.Running

        override fun keyDown(keycode: Int): Boolean {
            val line = typing
            when {
                line != null -> typeKey(line, keycode)
                keycode == Keys.ENTER -> typing = ""
                practising -> practiceKeys(keycode).forEach { market.handle(MarketIntent.PracticeKey(it)) }
                else -> marketKey(keycode)
            }
            return true
        }

        /** A committed character, IME compositions (pinyin, kana) included, while a line is open. */
        override fun keyTyped(character: Char): Boolean {
            val line = typing ?: return false
            if (!character.isISOControl()) typing = line + character
            return true
        }

        private fun typeKey(line: String, keycode: Int) {
            when (keycode) {
                Keys.ESCAPE -> typing = null
                Keys.BACKSPACE -> typing = line.dropLast(1)
                Keys.ENTER -> {
                    typing = null
                    if (line.isBlank()) return
                    val answering = practising && market.snapshot?.phase == PracticePhase.UNANSWERED
                    market.handle(if (answering) MarketIntent.PracticeKey(PracticeAction.Answer(line)) else MarketIntent.SayLine(line))
                }
            }
        }

        private fun marketKey(keycode: Int) {
            when (keycode) {
                Keys.LEFT, Keys.RIGHT -> market.at.neighbour(if (keycode == Keys.LEFT) -1 else 1)?.let { market.handle(MarketIntent.WalkTo(it)) }
                Keys.EQUALS, Keys.PLUS, Keys.NUMPAD_ADD -> market.handle(MarketIntent.ChangeLevel(1))
                Keys.MINUS, Keys.NUMPAD_SUBTRACT -> market.handle(MarketIntent.ChangeLevel(-1))
                Keys.P -> market.handle(MarketIntent.BeginPractice)
            }
        }

        /** A self-graded question is answered (its text is not graded) and graded in one key. */
        private fun practiceKeys(keycode: Int): List<PracticeAction> {
            val s = market.snapshot
            val unanswered = s?.phase == PracticePhase.UNANSWERED
            val selfGraded = s?.question?.selfGraded == true
            return when (keycode) {
                in Keys.NUM_1..Keys.NUM_4 -> listOf(PracticeAction.Choose(keycode - Keys.NUM_1))
                Keys.Y, Keys.N ->
                    if (unanswered && selfGraded) {
                        listOf(PracticeAction.Answer(""), PracticeAction.SelfGrade(keycode == Keys.Y))
                    } else {
                        listOf(PracticeAction.SelfGrade(keycode == Keys.Y))
                    }
                Keys.SPACE -> listOf(if (unanswered) PracticeAction.Reveal else PracticeAction.Next)
                Keys.ESCAPE -> listOf(PracticeAction.Finish)
                else -> emptyList()
            }
        }
    }

    private companion object {
        const val FONT = "fonts/NotoSansCJKsc-Regular-cjk-lite-subset.otf"
        const val POLL_EVERY_SECS = 5f
        const val LOG_LINES = 8
        const val STALL_Y = 340f
        const val PANEL_TOP = 300f
        val STALL_COLOURS =
            mapOf(
                Stall.TEA to Color(0.80f, 0.30f, 0.22f, 1f),
                Stall.LANTERNS to Color(0.90f, 0.70f, 0.25f, 1f),
                Stall.BOOKS to Color(0.25f, 0.45f, 0.60f, 1f),
            )
    }
}
