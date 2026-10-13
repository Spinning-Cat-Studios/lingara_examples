//! The four game-logic cases (ADR 13.10.26h D4), offline: `MinimalPlugins`
//! and `MarketLogic` alone, with no `LingaraPlugin`, no view and no network.
//! Each test writes the game's Messages and `bevy_lingara`'s out-Messages,
//! and reads what the logic writes. It tests the game, never the kit.

// The logic is the binary's module; the test compiles the same file.
#[allow(dead_code)]
#[path = "../src/logic.rs"]
mod logic;

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy_lingara::lingara_embed::{PracticePhase, PracticeSnapshot};
use bevy_lingara::{NpcDialogue, NpcReplied, PracticeFinished, WorldContextChanged};
use logic::{ChangeLevel, Market, MarketLogic, Practice, RewardsPolled, Stall, StallLog, THANKS, WalkTo};

/// The market after its first frame, which spawns the keepers.
fn market() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MarketLogic));
    app.update();
    app
}

fn keeper(app: &App, stall: Stall) -> Entity {
    app.world().resource::<Market>().keepers[&stall]
}

/// Writes `message` and runs one frame.
fn send<M: Message>(app: &mut App, message: M) {
    app.world_mut().write_message(message);
    app.update();
}

/// Every `M` written since the last call.
fn drain<M: Message>(app: &mut App) -> Vec<M> {
    app.world_mut().resource_mut::<Messages<M>>().drain().collect()
}

fn log(app: &App, stall: Stall) -> Vec<String> {
    app.world().get::<StallLog>(keeper(app, stall)).expect("a keeper has a log").0.clone()
}

fn level(app: &App, stall: Stall) -> u8 {
    app.world().get::<NpcDialogue>(keeper(app, stall)).expect("a keeper has a dialogue").level()
}

// 13.10.26h AC1
/// Walking to a keeper's stall writes exactly one `WorldContextChanged`
/// carrying that stall's scene; walking to the lantern stall writes none.
#[test]
fn arriving_at_a_stall_sends_one_context() {
    let mut app = market();
    assert!(drain::<WorldContextChanged>(&mut app).is_empty(), "opening the market sends nothing");

    send(&mut app, WalkTo(Stall::Tea));
    let sent = drain::<WorldContextChanged>(&mut app);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].0.scene, Stall::Tea.scene());
    assert_eq!((sent[0].0.target_lang.as_str(), sent[0].0.level), ("zh", 2));

    send(&mut app, WalkTo(Stall::Lanterns));
    assert!(drain::<WorldContextChanged>(&mut app).is_empty(), "nobody keeps the lantern stall");

    send(&mut app, WalkTo(Stall::Books));
    let sent = drain::<WorldContextChanged>(&mut app);
    assert_eq!(sent.iter().map(|c| c.0.scene.as_str()).collect::<Vec<_>>(), [Stall::Books.scene()]);
}

// 13.10.26h AC2
/// An `NpcReplied` for one keeper lands in that keeper's `StallLog` and in no
/// other.
#[test]
fn a_reply_lands_with_its_speaker() {
    let mut app = market();
    let mei = keeper(&app, Stall::Tea);
    send(&mut app, NpcReplied { entity: mei, text: "你好！要喝茶吗？".into(), level: 2 });

    assert_eq!(log(&app, Stall::Tea), ["Mei: 你好！要喝茶吗？"]);
    assert!(log(&app, Stall::Books).is_empty());
}

// 13.10.26h AC3
/// `ChangeLevel` at a stall sets that keeper's level, clamped to the kit's
/// range, and leaves the other keeper's unchanged.
#[test]
fn a_level_change_reaches_its_keeper() {
    let mut app = market();
    send(&mut app, WalkTo(Stall::Tea));
    send(&mut app, ChangeLevel(1));
    assert_eq!((level(&app, Stall::Tea), level(&app, Stall::Books)), (3, 6));

    for _ in 0..5 {
        send(&mut app, ChangeLevel(-1));
    }
    assert_eq!((level(&app, Stall::Tea), level(&app, Stall::Books)), (1, 6), "clamped at the kit's floor");
}

fn finished(entity: Entity) -> PracticeFinished {
    let snapshot = PracticeSnapshot {
        phase: PracticePhase::Reported,
        plan: None,
        set_index: 0,
        set_count: 1,
        question_index: 4,
        question_count: 5,
        question: None,
        last_result: None,
        failure: None,
    };
    PracticeFinished { entity, snapshot }
}

// 13.10.26h AC4
/// A `PracticeFinished` moves practice to `AwaitingResult`; a polled total
/// one higher gives exactly one lantern, thanks the keeper in its log and
/// returns to `Idle`; the same total polled again gives none.
#[test]
fn a_practice_result_earns_one_lantern() {
    let mut app = market();
    let mei = keeper(&app, Stall::Tea);
    send(&mut app, RewardsPolled(2));
    assert_eq!(app.world().resource::<Market>().lanterns, 2, "the startup poll is adopted silently");
    assert!(log(&app, Stall::Tea).is_empty());

    send(&mut app, finished(mei));
    assert!(matches!(app.world().resource::<Market>().practice, Practice::AwaitingResult { keeper, .. } if keeper == mei));
    send(&mut app, RewardsPolled(2));
    assert!(matches!(app.world().resource::<Market>().practice, Practice::AwaitingResult { .. }), "the same total is not a result");

    send(&mut app, RewardsPolled(3));
    let market = app.world().resource::<Market>();
    assert_eq!((market.lanterns, &market.practice), (3, &Practice::Idle));
    assert_eq!(log(&app, Stall::Tea), [format!("Mei: {THANKS}")]);

    send(&mut app, RewardsPolled(3));
    assert_eq!(app.world().resource::<Market>().lanterns, 3);
    assert_eq!(log(&app, Stall::Tea).len(), 1, "no second thanks");
    assert!(log(&app, Stall::Books).is_empty());
}
