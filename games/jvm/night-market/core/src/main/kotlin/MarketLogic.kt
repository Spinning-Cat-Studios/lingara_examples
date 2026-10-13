package nightmarket

import com.getlingara.embed.Command
import com.getlingara.embed.CommandOutcome
import com.getlingara.embed.DialogueConfig
import com.getlingara.embed.EmbedEvent
import com.getlingara.embed.Npc
import com.getlingara.embed.NpcTurnUpdate
import com.getlingara.embed.PracticePhase
import com.getlingara.embed.PracticeSnapshot
import com.getlingara.embed.PracticeSource
import com.getlingara.embed.WorldContext

/** The language the stall-keepers explain things in. */
const val SOURCE_LANG = "en"

/** What a keeper says when a practice result earns a lantern. Written by the game: a reward costs no Lingara call. */
const val THANKS = "Thank you for practising! Take a lantern for the walk home."

/** How long a finished practice waits for its webhook before giving up. */
const val RESULT_DEADLINE_SECS = 300f

/** The kit's NPC level range. */
private val LEVELS = 1..9

/** Who keeps a stall, and the level they start at. */
data class KeeperSpec(val name: String, val persona: String, val targetLang: String, val level: Int)

/** The three stalls, left to right along the street, each with its fixed scene. */
enum class Stall(val scene: String, val keeper: KeeperSpec?) {
    TEA(
        "Mei's tea stall at a night market, clay pots steaming under red lanterns",
        KeeperSpec("Mei", "a cheerful tea seller who loves to tell customers where her leaves come from", "zh", 2),
    ),
    LANTERNS("a paper-lantern stall at a night market, with nobody minding it", null),
    BOOKS(
        "Professor Tanaka's second-hand book stall at a night market, paperbacks stacked by a lamp",
        KeeperSpec("Professor Tanaka", "a retired literature professor who sells second-hand books", "ja", 6),
    ),
    ;

    /** The stall [step] places along the street, if there is one. */
    fun neighbour(step: Int): Stall? = entries.getOrNull(ordinal + step)
}

/** A stall-keeper: their open dialogue, their level, and the log the screen draws. */
class Keeper(val stall: Stall, val spec: KeeperSpec, val dialogue: Long) {
    var level: Int = spec.level
        internal set

    /** Only [MarketLogic] appends to it; the kit's own transcript is the kit's. */
    val log: List<String> get() = lines
    internal val lines = mutableListOf<String>()
}

sealed interface Practice {
    data object Idle : Practice

    data class Running(val keeper: Stall, val session: Long) : Practice

    data class AwaitingResult(val keeper: Stall, val secondsLeft: Float) : Practice
}

/**
 * Every rule of the Night Market and all of its state, and nothing drawn. It
 * reads the screen's [MarketIntent]s and the kit's `EmbedEvent`s, and it is
 * the only caller of the kit, through [port]. Opening the market opens one
 * dialogue per keeper.
 */
class MarketLogic(private val port: MarketPort) {
    val keepers: Map<Stall, Keeper> =
        Stall.entries.mapNotNull { stall -> stall.keeper?.let { stall to Keeper(stall, it, open(stall, it)) } }.toMap()

    /** The player starts between the two keepers, at the stall that sends nothing. */
    var at: Stall = Stall.LANTERNS
        private set
    var practice: Practice = Practice.Idle
        private set
    var snapshot: PracticeSnapshot? = null
        private set
    var lanterns: Int = 0
        private set
    var lastError: String? = null
        private set

    val keeperHere: Keeper? get() = keepers[at]

    fun handle(intent: MarketIntent) {
        when (intent) {
            is MarketIntent.WalkTo -> walk(intent.stall)
            is MarketIntent.SayLine -> say(intent.text)
            is MarketIntent.ChangeLevel -> changeLevel(intent.delta)
            MarketIntent.BeginPractice -> beginPractice()
            is MarketIntent.PracticeKey -> practise(intent.action)
            is MarketIntent.RewardsPolled -> rewardsPolled(intent.total)
        }
    }

    fun onEvent(event: EmbedEvent) {
        when (event) {
            is EmbedEvent.NpcTurn -> onTurn(event)
            is EmbedEvent.Practice -> if ((practice as? Practice.Running)?.session == event.session) onSnapshot(event.snapshot)
            is EmbedEvent.Error -> lastError = event.error.message
            else -> Unit
        }
    }

    /** Once a frame: a result that never arrives stops being awaited. */
    fun update(deltaSeconds: Float) {
        val awaiting = practice as? Practice.AwaitingResult ?: return
        val left = awaiting.secondsLeft - deltaSeconds
        practice = if (left <= 0f) Practice.Idle else awaiting.copy(secondsLeft = left)
    }

    private fun open(stall: Stall, spec: KeeperSpec): Long {
        val config = DialogueConfig(Npc(spec.name, spec.persona), stall.scene, SOURCE_LANG, spec.targetLang, spec.level, emptyList())
        return id(port.execute(Command.DialogueOpen(config)))
    }

    /** Arriving at a keeper's stall sends its scene once; the lantern stall sends nothing. */
    private fun walk(stall: Stall) {
        if (stall == at) return
        at = stall
        val keeper = keeperHere ?: return
        val npc = Npc(keeper.spec.name, keeper.spec.persona)
        port.execute(Command.SendContext(WorldContext(stall.scene, SOURCE_LANG, keeper.spec.targetLang, keeper.level, npc, emptyList(), false)))
    }

    private fun say(text: String) {
        val keeper = keeperHere ?: return
        keeper.lines += "you: $text"
        port.execute(Command.DialogueSay(keeper.dialogue, text))
    }

    /** Level is per keeper, clamped to the kit's range; that keeper's next turn reads it. */
    private fun changeLevel(delta: Int) {
        val keeper = keeperHere ?: return
        val level = (keeper.level + delta).coerceIn(LEVELS)
        if (level == keeper.level) return
        keeper.level = level
        port.execute(Command.DialogueSetLevel(keeper.dialogue, level))
    }

    /** A replied turn lands in its own keeper's log; fragments are the kit's to stream. */
    private fun onTurn(event: EmbedEvent.NpcTurn) {
        val update = event.update as? NpcTurnUpdate.Finished ?: return
        val keeper = keepers.values.firstOrNull { it.dialogue == event.dialogue } ?: return
        keeper.lines += "${keeper.spec.name}: ${update.text}"
    }

    /**
     * Generated from the stall's scene at the keeper's level, and reported,
     * which is what can earn a lantern. Never a plan with `report = false`:
     * that reports nothing.
     */
    private fun beginPractice() {
        val keeper = keeperHere ?: return
        if (practice != Practice.Idle) return
        val source = PracticeSource.Request(at.scene, SOURCE_LANG, keeper.spec.targetLang, keeper.level, emptyList())
        practice = Practice.Running(at, id(port.execute(Command.PracticeStart(source))))
        snapshot = null
    }

    private fun practise(action: PracticeAction) {
        val session = (practice as? Practice.Running)?.session ?: return
        val command =
            when (action) {
                is PracticeAction.Answer -> Command.PracticeAnswer(session, action.text)
                is PracticeAction.Choose -> Command.PracticeChoose(session, action.option.toLong())
                PracticeAction.Reveal -> Command.PracticeReveal(session)
                is PracticeAction.SelfGrade -> Command.PracticeSelfGrade(session, action.correct)
                PracticeAction.Next -> Command.PracticeNext(session)
                PracticeAction.Finish -> Command.PracticeFinish(session)
            }
        (port.execute(command) as? CommandOutcome.Practice)?.snapshot?.let(::onSnapshot)
    }

    private fun onSnapshot(next: PracticeSnapshot) {
        snapshot = next
        val running = practice as? Practice.Running ?: return
        if (isTerminal(next)) practice = Practice.AwaitingResult(running.keeper, RESULT_DEADLINE_SECS)
    }

    /**
     * Adopted silently when nothing is pending (the first poll, or a server
     * that outlived a restart). While a result is awaited, a higher total is
     * that result: thanks, the new count, and back to idle.
     */
    private fun rewardsPolled(total: Int) {
        when (val current = practice) {
            Practice.Idle -> lanterns = total
            is Practice.AwaitingResult ->
                if (total > lanterns) {
                    keepers[current.keeper]?.let { it.lines += "${it.spec.name}: $THANKS" }
                    lanterns = total
                    practice = Practice.Idle
                }
            is Practice.Running -> Unit
        }
    }

    private fun id(outcome: CommandOutcome): Long = (outcome as? CommandOutcome.Id)?.id ?: error("the kit answered $outcome, not an id")
}

/** The first phase a session cannot leave, as the kit's engine plugins read it. */
private fun isTerminal(snapshot: PracticeSnapshot): Boolean =
    when (snapshot.phase) {
        PracticePhase.REPORTED, PracticePhase.ENDED, PracticePhase.FAILED -> true
        PracticePhase.REPORT_FAILED -> snapshot.failure?.retryable != true
        else -> false
    }
