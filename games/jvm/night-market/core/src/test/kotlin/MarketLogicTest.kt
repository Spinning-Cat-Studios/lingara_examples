package nightmarket

import com.getlingara.embed.Command
import com.getlingara.embed.CommandOutcome
import com.getlingara.embed.EmbedEvent
import com.getlingara.embed.NpcTurnUpdate
import com.getlingara.embed.PracticePhase
import com.getlingara.embed.PracticeSnapshot
import com.getlingara.embed.PracticeSource
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The four game-logic cases, offline: [MarketLogic] over a fake [MarketPort],
 * fed [MarketIntent]s and constructed `EmbedEvent`s, with no screen, no
 * natives and no network. This classpath has no JNA at all, so a kit type
 * that loaded the native library could not be built here. It tests the game,
 * never the kit.
 */
class MarketLogicTest {
    /** Records every command; an open or a start answers the next id. */
    private class FakePort : MarketPort {
        val sent = mutableListOf<Command>()
        private var nextId = 0L

        override fun execute(command: Command): CommandOutcome {
            sent += command
            return when (command) {
                is Command.DialogueOpen, is Command.PracticeStart -> CommandOutcome.Id(++nextId)
                else -> CommandOutcome.Done
            }
        }

        override fun rewards(): Int = error("the logic never polls; the screen does")

        inline fun <reified C : Command> sent(): List<C> = sent.filterIsInstance<C>()
    }

    private val port = FakePort()
    private val market = MarketLogic(port)

    private fun keeper(stall: Stall) = market.keepers.getValue(stall)

    // 13.10.26i AC1
    /** Walking to a keeper's stall sends exactly one world context with that stall's scene; the lantern stall sends none. */
    @Test
    fun `arriving at a stall sends one context`() {
        assertTrue(port.sent<Command.SendContext>().isEmpty(), "opening the market sends no context")

        market.handle(MarketIntent.WalkTo(Stall.TEA))
        val sent = port.sent<Command.SendContext>()
        assertEquals(1, sent.size, "$sent")
        assertEquals(Stall.TEA.scene, sent[0].context.scene)
        assertEquals("zh" to 2, sent[0].context.targetLang to sent[0].context.level)

        market.handle(MarketIntent.WalkTo(Stall.LANTERNS))
        assertEquals(1, port.sent<Command.SendContext>().size, "nobody keeps the lantern stall")

        market.handle(MarketIntent.WalkTo(Stall.BOOKS))
        assertEquals(listOf(Stall.TEA.scene, Stall.BOOKS.scene), port.sent<Command.SendContext>().map { it.context.scene })
    }

    // 13.10.26i AC2
    /** A keeper's finished `NpcTurn` lands in that keeper's stall log and in no other. */
    @Test
    fun `a reply lands with its speaker`() {
        val mei = keeper(Stall.TEA)
        market.onEvent(EmbedEvent.NpcTurn(mei.dialogue, 1, NpcTurnUpdate.Fragment("你好")))
        market.onEvent(EmbedEvent.NpcTurn(mei.dialogue, 1, NpcTurnUpdate.Finished("你好！要喝茶吗？")))

        assertEquals(listOf("Mei: 你好！要喝茶吗？"), mei.log)
        assertTrue(keeper(Stall.BOOKS).log.isEmpty())
    }

    // 13.10.26i AC3
    /** `ChangeLevel` at a stall sets that keeper's level, clamped to the kit's range, and leaves the other keeper's unchanged. */
    @Test
    fun `a level change reaches its keeper`() {
        market.handle(MarketIntent.WalkTo(Stall.TEA))
        market.handle(MarketIntent.ChangeLevel(1))
        assertEquals(3 to 6, keeper(Stall.TEA).level to keeper(Stall.BOOKS).level)
        assertEquals(listOf(Command.DialogueSetLevel(keeper(Stall.TEA).dialogue, 3)), port.sent<Command.DialogueSetLevel>())

        repeat(5) { market.handle(MarketIntent.ChangeLevel(-1)) }
        assertEquals(1 to 6, keeper(Stall.TEA).level to keeper(Stall.BOOKS).level, "clamped at the kit's floor")
    }

    private fun reported() = PracticeSnapshot(PracticePhase.REPORTED, null, 0, 1, 4, 5, null, null, null)

    // 13.10.26i AC4
    /**
     * A finished practice moves to `AwaitingResult`; a polled total one higher
     * gives exactly one lantern, thanks the keeper in its log and returns to
     * `Idle`; the same total polled again gives none.
     */
    @Test
    fun `a practice result earns one lantern`() {
        market.handle(MarketIntent.RewardsPolled(2))
        assertEquals(2, market.lanterns, "the startup poll is adopted silently")

        market.handle(MarketIntent.WalkTo(Stall.TEA))
        market.handle(MarketIntent.BeginPractice)
        val running = market.practice as Practice.Running
        assertEquals(Stall.TEA.scene, (port.sent<Command.PracticeStart>().single().source as PracticeSource.Request).topic)

        market.onEvent(EmbedEvent.Practice(running.session, reported()))
        assertEquals(Stall.TEA, (market.practice as Practice.AwaitingResult).keeper)
        market.handle(MarketIntent.RewardsPolled(2))
        assertTrue(market.practice is Practice.AwaitingResult, "the same total is not a result")

        market.handle(MarketIntent.RewardsPolled(3))
        assertEquals(3 to Practice.Idle, market.lanterns to market.practice)
        assertEquals(listOf("Mei: $THANKS"), keeper(Stall.TEA).log)

        market.handle(MarketIntent.RewardsPolled(3))
        assertEquals(3, market.lanterns)
        assertEquals(1, keeper(Stall.TEA).log.size, "no second thanks")
        assertTrue(keeper(Stall.BOOKS).log.isEmpty())
    }

    /** A result that never arrives stops being awaited at the deadline. */
    @Test
    fun `an unanswered result is given up on`() {
        market.handle(MarketIntent.WalkTo(Stall.BOOKS))
        market.handle(MarketIntent.BeginPractice)
        market.onEvent(EmbedEvent.Practice((market.practice as Practice.Running).session, reported()))
        market.update(RESULT_DEADLINE_SECS - 1f)
        assertTrue(market.practice is Practice.AwaitingResult)
        market.update(1f)
        assertEquals(Practice.Idle, market.practice)
    }

    /** The server's `{"lanterns":N}`, spaced or not; anything else is no total. */
    @Test
    fun `a rewards body reads as its total`() {
        assertEquals(0, parseLanterns("""{"lanterns":0}"""))
        assertEquals(12, parseLanterns("""{"lanterns": 12}"""))
        assertNull(parseLanterns("""{"lanterns":"x"}"""))
        assertNull(parseLanterns("Unauthorized"))
    }
}
