//! `MarketLogic`: every rule of the Night Market and all of its state, and
//! nothing drawn. The view turns input into the game's own Messages
//! (`WalkTo`, `SayLine`, `ChangeLevel`, `BeginPractice`, `PracticeKey`,
//! `RewardsPolled`); this plugin reads those and `bevy_lingara`'s
//! out-Messages, and it is the only writer of `bevy_lingara`'s in-Messages.
//! That seam is what `tests/market.rs` drives, under `MinimalPlugins` with no
//! `LingaraPlugin`, no window and no network.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy_lingara::lingara_embed::{Npc, PracticeSource, WorldContext, limits};
use bevy_lingara::{NpcDialogue, NpcReplied, NpcSay, PracticeAction, PracticeFinished, PracticeInput, StartPractice, WorldContextChanged};

/// The language the stall-keepers explain things in.
pub const SOURCE_LANG: &str = "en";
/// What a keeper says when a practice result earns a lantern. Written by the
/// game, never by Lingara: a reward costs no call.
pub const THANKS: &str = "Thank you for practising! Take a lantern for the walk home.";
/// How long a finished practice waits for its webhook before giving up.
pub const RESULT_DEADLINE: Duration = Duration::from_secs(300);

/// The three stalls, left to right along the street.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stall {
    Tea,
    Lanterns,
    Books,
}

/// Who keeps a stall: the `NpcDialogue` it starts with.
pub struct KeeperSpec {
    pub name: &'static str,
    pub persona: &'static str,
    pub target_lang: &'static str,
    pub level: u8,
}

impl Stall {
    pub const STREET: [Stall; 3] = [Stall::Tea, Stall::Lanterns, Stall::Books];

    /// The fixed scene a stall sends, and a practice there is about.
    pub fn scene(self) -> &'static str {
        match self {
            Stall::Tea => "Mei's tea stall at a night market, clay pots steaming under red lanterns",
            Stall::Lanterns => "a paper-lantern stall at a night market, with nobody minding it",
            Stall::Books => "Professor Tanaka's second-hand book stall at a night market, paperbacks stacked by a lamp",
        }
    }

    pub fn keeper(self) -> Option<KeeperSpec> {
        match self {
            Stall::Tea => Some(KeeperSpec { name: "Mei", persona: "a cheerful tea seller who loves to tell customers where her leaves come from", target_lang: "zh", level: 2 }),
            Stall::Books => Some(KeeperSpec { name: "Professor Tanaka", persona: "a retired literature professor who sells second-hand books", target_lang: "ja", level: 6 }),
            Stall::Lanterns => None,
        }
    }

    /// The stall `step` places along the street, if there is one.
    pub fn neighbour(self, step: i32) -> Option<Stall> {
        let at = Stall::STREET.iter().position(|s| *s == self)? as i32 + step;
        usize::try_from(at).ok().and_then(|i| Stall::STREET.get(i).copied())
    }
}

// ── The game's own Messages, written by the view (or a test) ──

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkTo(pub Stall);

/// A line the player typed, for the keeper at the current stall.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct SayLine(pub String);

/// `+1` or `-1` to the level of the keeper at the current stall.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeLevel(pub i8);

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginPractice;

#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct PracticeKey(pub PracticeAction);

/// The server's running lantern total for this player.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardsPolled(pub u32);

// ── State ──

/// A stall-keeper entity; it also holds its `NpcDialogue` and `StallLog`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Keeper(pub Stall);

/// What the view draws for a keeper. Only this plugin appends to it; the
/// kit's own `NpcTranscript` is the kit's.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct StallLog(pub Vec<String>);

#[derive(Debug, Clone, PartialEq)]
pub enum Practice {
    Idle,
    Running { keeper: Entity },
    AwaitingResult { keeper: Entity, left: Duration },
}

#[derive(Resource, Debug)]
pub struct Market {
    pub at: Stall,
    pub keepers: HashMap<Stall, Entity>,
    pub practice: Practice,
    pub lanterns: u32,
}

impl Market {
    /// The keeper at the player's stall, if it has one.
    pub fn keeper_here(&self) -> Option<Entity> {
        self.keepers.get(&self.at).copied()
    }
}

pub struct MarketLogic;

impl Plugin for MarketLogic {
    fn build(&self, app: &mut App) {
        // Idempotent beside LingaraPlugin, and enough on its own for the test app.
        app.add_message::<WalkTo>()
            .add_message::<SayLine>()
            .add_message::<ChangeLevel>()
            .add_message::<BeginPractice>()
            .add_message::<PracticeKey>()
            .add_message::<RewardsPolled>()
            .add_message::<WorldContextChanged>()
            .add_message::<NpcSay>()
            .add_message::<StartPractice>()
            .add_message::<PracticeInput>()
            .add_message::<NpcReplied>()
            .add_message::<PracticeFinished>()
            .add_systems(Startup, open_market)
            .add_systems(Update, (walk, talk, change_level, log_replies, practise, practice_finished, rewards, deadline).chain());
    }
}

fn open_market(mut commands: Commands) {
    let mut keepers = HashMap::new();
    for stall in Stall::STREET {
        let Some(spec) = stall.keeper() else { continue };
        let dialogue = NpcDialogue::new(spec.name, spec.target_lang, spec.level)
            .and_then(|d| d.with_persona(spec.persona))
            .and_then(|d| d.with_scene(stall.scene()))
            .map(|d| d.with_source_lang(SOURCE_LANG))
            .expect("the keepers are within the kit's bounds");
        keepers.insert(stall, commands.spawn((Keeper(stall), dialogue, StallLog::default())).id());
    }
    // The player starts between the two keepers, at the stall that sends nothing.
    commands.insert_resource(Market { at: Stall::Lanterns, keepers, practice: Practice::Idle, lanterns: 0 });
}

/// Arriving at a keeper's stall sends its scene once. Each send spends one of
/// the player's inbound cells, so the lantern stall sends nothing.
fn walk(mut walks: MessageReader<WalkTo>, mut market: ResMut<Market>, keepers: Query<&NpcDialogue>, mut out: MessageWriter<WorldContextChanged>) {
    for WalkTo(stall) in walks.read() {
        if *stall == market.at {
            continue;
        }
        market.at = *stall;
        let Some(dialogue) = market.keeper_here().and_then(|e| keepers.get(e).ok()) else { continue };
        let npc = Npc { name: dialogue.npc().name.clone(), persona: dialogue.npc().persona.clone() };
        let context = WorldContext {
            scene: stall.scene().into(),
            source_lang: SOURCE_LANG.into(),
            target_lang: dialogue.target_lang().into(),
            level: dialogue.level(),
            npc: Some(npc),
            tags: vec![],
            generate: false,
        };
        out.write(WorldContextChanged(context));
    }
}

fn talk(mut lines: MessageReader<SayLine>, market: Res<Market>, mut logs: Query<&mut StallLog>, mut out: MessageWriter<NpcSay>) {
    for SayLine(line) in lines.read() {
        let Some(entity) = market.keeper_here() else { continue };
        if let Ok(mut log) = logs.get_mut(entity) {
            log.0.push(format!("you: {line}"));
        }
        out.write(NpcSay { entity, line: line.clone() });
    }
}

/// Level is per keeper, clamped to the kit's range; the kit's next turn for
/// that keeper reads it.
fn change_level(mut changes: MessageReader<ChangeLevel>, market: Res<Market>, mut keepers: Query<&mut NpcDialogue>) {
    for ChangeLevel(step) in changes.read() {
        let Some(mut dialogue) = market.keeper_here().and_then(|e| keepers.get_mut(e).ok()) else { continue };
        let level = (i16::from(dialogue.level()) + i16::from(*step)).clamp(limits::LEVEL_MIN.into(), limits::LEVEL_MAX.into());
        if level != i16::from(dialogue.level()) {
            dialogue.set_level(level as u8).expect("clamped to the kit's range");
        }
    }
}

fn log_replies(mut replies: MessageReader<NpcReplied>, mut keepers: Query<(&NpcDialogue, &mut StallLog)>) {
    for reply in replies.read() {
        if let Ok((dialogue, mut log)) = keepers.get_mut(reply.entity) {
            log.0.push(format!("{}: {}", dialogue.npc().name, reply.text));
        }
    }
}

/// A practice is generated from the stall's scene at the keeper's level and
/// reports its result, which is what can earn a lantern. Never a
/// `Plan { report: false }`: that reports nothing.
fn practise(
    mut begins: MessageReader<BeginPractice>,
    mut keys: MessageReader<PracticeKey>,
    mut market: ResMut<Market>,
    keepers: Query<&NpcDialogue>,
    mut out: ParamSet<(MessageWriter<StartPractice>, MessageWriter<PracticeInput>)>,
) {
    for _ in begins.read() {
        let Some(entity) = market.keeper_here().filter(|_| market.practice == Practice::Idle) else { continue };
        let Ok(dialogue) = keepers.get(entity) else { continue };
        let source = PracticeSource::Request {
            topic: market.at.scene().into(),
            source_lang: SOURCE_LANG.into(),
            target_lang: dialogue.target_lang().into(),
            level: dialogue.level(),
            tags: vec![],
        };
        out.p0().write(StartPractice { entity, source });
        market.practice = Practice::Running { keeper: entity };
    }
    for PracticeKey(action) in keys.read() {
        if let Practice::Running { keeper } = market.practice {
            out.p1().write(PracticeInput { entity: keeper, action: action.clone() });
        }
    }
}

fn practice_finished(mut finished: MessageReader<PracticeFinished>, mut market: ResMut<Market>) {
    for done in finished.read() {
        market.practice = Practice::AwaitingResult { keeper: done.entity, left: RESULT_DEADLINE };
    }
}

/// The server's total is adopted silently when nothing is pending (the first
/// poll, or a server that outlived a restart). While a result is awaited, a
/// higher total is that result: thanks, the new count, and back to idle.
fn rewards(mut polls: MessageReader<RewardsPolled>, mut market: ResMut<Market>, mut logs: Query<(&Keeper, &mut StallLog)>) {
    for RewardsPolled(total) in polls.read() {
        match market.practice {
            Practice::Idle => market.lanterns = *total,
            Practice::AwaitingResult { keeper, .. } if *total > market.lanterns => {
                if let Ok((Keeper(stall), mut log)) = logs.get_mut(keeper) {
                    let name = stall.keeper().map_or("", |k| k.name);
                    log.0.push(format!("{name}: {THANKS}"));
                }
                market.lanterns = *total;
                market.practice = Practice::Idle;
            }
            _ => {}
        }
    }
}

fn deadline(time: Res<Time>, mut market: ResMut<Market>) {
    if let Practice::AwaitingResult { left, .. } = &mut market.practice {
        *left = left.saturating_sub(time.delta());
        if left.is_zero() {
            market.practice = Practice::Idle;
        }
    }
}
