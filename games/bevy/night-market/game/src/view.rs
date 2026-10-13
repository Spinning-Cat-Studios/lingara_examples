//! `MarketView`: draws `MarketLogic`'s state and turns the outside world into
//! the game's Messages. It holds no game state: the typed line and the
//! rewards poll's plumbing are the view's own, and everything else it reads.
//!
//! Keys: `←` / `→` walk, `Enter` types a line (`Enter` again sends it, `Esc`
//! drops it), `+` / `-` change the keeper's level, `P` practises. In a
//! practice: `1`–`4` choose, `Space` reveals or goes on, `Y` / `N` grade
//! yourself, `Esc` finishes early, and a typed line is an answer.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bevy::window::{Ime, PrimaryWindow};
use bevy_lingara::lingara_embed::PracticePhase;
use bevy_lingara::{LingaraError, LingaraStatus, NpcDialogue, PracticeAction, PracticeSession};

use crate::logic::{BeginPractice, ChangeLevel, Market, Practice, PracticeKey, RewardsPolled, SayLine, Stall, StallLog, WalkTo};

/// The bundled CJK font (SIL OFL 1.1, `assets/fonts/`): Bevy's built-in one
/// has no Chinese or Japanese glyphs, so without it the replies are boxes.
const FONT: &str = "fonts/NotoSansCJKsc-Regular-cjk-lite-subset.otf";
const POLL_EVERY: Duration = Duration::from_secs(5);
const LOG_LINES: usize = 8;

pub struct MarketView {
    /// `NIGHT_MARKET_SERVER`, the server half's base URL.
    pub server: String,
    /// `NIGHT_MARKET_PLAYER`, the stand-in session value.
    pub player: String,
}

/// The line being typed, if any.
#[derive(Resource, Default)]
struct Typing(Option<String>);

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct PlayerMarker;

/// `GET <server>/rewards` off the frame: once at startup, then every 5 s
/// while the logic awaits a result. A finished poll leaves its total in
/// `answer`, and the next frame writes it as `RewardsPolled`.
#[derive(Resource)]
struct RewardsPoll {
    url: String,
    player: String,
    answer: Arc<Mutex<Option<u32>>>,
    in_flight: Arc<AtomicBool>,
    timer: Timer,
    started: bool,
}

impl Plugin for MarketView {
    fn build(&self, app: &mut App) {
        let poll = RewardsPoll {
            url: format!("{}/rewards", self.server.trim_end_matches('/')),
            player: self.player.clone(),
            answer: Arc::default(),
            in_flight: Arc::default(),
            timer: Timer::new(POLL_EVERY, TimerMode::Repeating),
            started: false,
        };
        app.insert_resource(poll)
            .init_resource::<Typing>()
            .add_systems(Startup, draw_street)
            .add_systems(Update, (keyboard, ime, poll_rewards, move_marker, show).chain());
    }
}

fn stall_x(stall: Stall) -> f32 {
    match stall {
        Stall::Tea => -320.0,
        Stall::Lanterns => 0.0,
        Stall::Books => 320.0,
    }
}

fn draw_street(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load(FONT);
    commands.spawn(Camera2d);
    for stall in Stall::STREET {
        let colour = match stall {
            Stall::Tea => Color::srgb(0.80, 0.30, 0.22),
            Stall::Lanterns => Color::srgb(0.90, 0.70, 0.25),
            Stall::Books => Color::srgb(0.25, 0.45, 0.60),
        };
        let x = stall_x(stall);
        commands.spawn((Sprite::from_color(colour, Vec2::new(200.0, 140.0)), Transform::from_xyz(x, 160.0, 0.0)));
        let label = stall.keeper().map_or("Lanterns", |k| k.name);
        commands.spawn((Text2d::new(label), TextFont::from_font_size(20.0).with_font(font.clone()), Transform::from_xyz(x, 250.0, 1.0)));
    }
    commands.spawn((Sprite::from_color(Color::WHITE, Vec2::new(28.0, 28.0)), Transform::from_xyz(0.0, 60.0, 1.0), PlayerMarker));
    let node = Node { position_type: PositionType::Absolute, left: Val::Px(16.0), right: Val::Px(16.0), bottom: Val::Px(16.0), ..default() };
    commands.spawn((Text::new(""), TextFont::from_font_size(18.0).with_font(font), node, Panel));
}

/// Every key press in order, so the `Enter` that opens a line is never also
/// typed into it.
fn keyboard(
    mut keys: MessageReader<KeyboardInput>,
    mut typing: ResMut<Typing>,
    market: Res<Market>,
    sessions: Query<&PracticeSession>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut out: KeyOut,
) {
    let session = market.keeper_here().and_then(|e| sessions.get(e).ok()).filter(|_| matches!(market.practice, Practice::Running { .. }));
    for key in keys.read().filter(|k| k.state == ButtonState::Pressed) {
        match typing.0.take() {
            Some(line) => typing.0 = type_into(line, key, session, &mut out),
            None if key.key_code == KeyCode::Enter => typing.0 = Some(String::new()),
            None => command(key.key_code, &market, session, &mut out),
        }
    }
    // IME composition (pinyin, kana) only while a line is open, so it never
    // swallows a hotkey.
    window.ime_enabled = typing.0.is_some();
}

/// Every Message the keys write.
#[derive(bevy::ecs::system::SystemParam)]
struct KeyOut<'w> {
    walk: MessageWriter<'w, WalkTo>,
    say: MessageWriter<'w, SayLine>,
    level: MessageWriter<'w, ChangeLevel>,
    begin: MessageWriter<'w, BeginPractice>,
    practice: MessageWriter<'w, PracticeKey>,
}

/// One key into the open line: the line as it now stands, or `None` once it
/// is sent or dropped. A line typed during an unanswered question answers it.
fn type_into(mut line: String, key: &KeyboardInput, session: Option<&PracticeSession>, out: &mut KeyOut) -> Option<String> {
    match key.key_code {
        KeyCode::Escape => return None,
        KeyCode::Backspace => _ = line.pop(),
        KeyCode::Enter if line.trim().is_empty() => return None,
        KeyCode::Enter if session.is_some_and(|s| s.snapshot().phase == PracticePhase::Unanswered) => {
            out.practice.write(PracticeKey(PracticeAction::Answer(line)));
            return None;
        }
        KeyCode::Enter => {
            out.say.write(SayLine(line));
            return None;
        }
        _ => line.extend(key.text.iter().flat_map(|t| t.chars()).filter(|c| !c.is_control())),
    }
    Some(line)
}

fn command(code: KeyCode, market: &Market, session: Option<&PracticeSession>, out: &mut KeyOut) {
    if let Some(session) = session {
        for action in practice_actions(code, session) {
            out.practice.write(PracticeKey(action));
        }
        return;
    }
    match code {
        KeyCode::ArrowLeft | KeyCode::ArrowRight => {
            let step = if code == KeyCode::ArrowLeft { -1 } else { 1 };
            if let Some(stall) = market.at.neighbour(step) {
                out.walk.write(WalkTo(stall));
            }
        }
        KeyCode::Equal | KeyCode::NumpadAdd => {
            out.level.write(ChangeLevel(1));
        }
        KeyCode::Minus | KeyCode::NumpadSubtract => {
            out.level.write(ChangeLevel(-1));
        }
        KeyCode::KeyP => {
            out.begin.write(BeginPractice);
        }
        _ => {}
    }
}

/// A self-graded question is answered (its text is not graded) and graded
/// by the learner in one key, as the kit's demo does.
fn practice_actions(code: KeyCode, session: &PracticeSession) -> Vec<PracticeAction> {
    let snapshot = session.snapshot();
    let unanswered = snapshot.phase == PracticePhase::Unanswered;
    let self_graded = snapshot.question.as_ref().is_some_and(|q| q.self_graded);
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    match code {
        _ if digits.contains(&code) => vec![PracticeAction::Choose(digits.iter().position(|d| *d == code).unwrap_or_default() as u32)],
        KeyCode::KeyY | KeyCode::KeyN if unanswered && self_graded => vec![PracticeAction::Answer(String::new()), PracticeAction::SelfGrade(code == KeyCode::KeyY)],
        KeyCode::KeyY | KeyCode::KeyN => vec![PracticeAction::SelfGrade(code == KeyCode::KeyY)],
        KeyCode::Space if unanswered => vec![PracticeAction::Reveal],
        KeyCode::Space => vec![PracticeAction::Next],
        KeyCode::Escape => vec![PracticeAction::Finish],
        _ => vec![],
    }
}

/// A committed IME composition is typed into the open line.
fn ime(mut events: MessageReader<Ime>, mut typing: ResMut<Typing>) {
    for event in events.read() {
        if let (Ime::Commit { value, .. }, Some(line)) = (event, typing.0.as_mut()) {
            line.push_str(value);
        }
    }
}

fn poll_rewards(time: Res<Time>, market: Res<Market>, mut poll: ResMut<RewardsPoll>, mut out: MessageWriter<RewardsPolled>) {
    if let Some(total) = poll.answer.lock().unwrap_or_else(|p| p.into_inner()).take() {
        out.write(RewardsPolled(total));
    }
    let awaiting = matches!(market.practice, Practice::AwaitingResult { .. });
    let due = !poll.started || (awaiting && poll.timer.tick(time.delta()).just_finished());
    if !due || poll.in_flight.swap(true, Ordering::SeqCst) {
        return;
    }
    poll.started = true;
    let (url, player, answer, in_flight) = (poll.url.clone(), poll.player.clone(), poll.answer.clone(), poll.in_flight.clone());
    IoTaskPool::get()
        .spawn(async move {
            if let Some(total) = fetch_lanterns(&url, &player) {
                *answer.lock().unwrap_or_else(|p| p.into_inner()) = Some(total);
            }
            in_flight.store(false, Ordering::SeqCst);
        })
        .detach();
}

/// `{"lanterns":N}` from the server, or `None` when it cannot be reached.
fn fetch_lanterns(url: &str, player: &str) -> Option<u32> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into();
    let body = agent.get(url).header(crate::PLAYER_HEADER, player).call().ok()?.body_mut().read_to_string().ok()?;
    parse_lanterns(&body)
}

/// The total in a `/rewards` body, or `None` for anything else. The body is
/// one known key, so a split is enough and the game needs no JSON crate.
fn parse_lanterns(body: &str) -> Option<u32> {
    let digits = body.split("\"lanterns\":").nth(1)?.trim_start();
    digits[..digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len())].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_lanterns;

    /// The server's `{"lanterns":N}`, spaced or not; anything else is no total.
    #[test]
    fn a_rewards_body_reads_as_its_total() {
        assert_eq!(parse_lanterns(r#"{"lanterns":0}"#), Some(0));
        assert_eq!(parse_lanterns(r#"{"lanterns": 12}"#), Some(12));
        assert_eq!(parse_lanterns(r#"{"lanterns":"x"}"#), None);
        assert_eq!(parse_lanterns("Unauthorized"), None);
    }
}

fn move_marker(market: Res<Market>, mut marker: Single<&mut Transform, With<PlayerMarker>>) {
    marker.translation.x = stall_x(market.at);
}

fn show(
    market: Res<Market>,
    typing: Res<Typing>,
    status: Res<LingaraStatus>,
    mut errors: MessageReader<LingaraError>,
    mut last_error: Local<Option<String>>,
    keepers: Query<(&NpcDialogue, &StallLog, Option<&PracticeSession>)>,
    mut panel: Single<&mut Text, With<Panel>>,
) {
    if let Some(e) = errors.read().last() {
        *last_error = Some(e.error.to_string());
    }
    let mut text = format!("The Night Market · lanterns: {} · Lingara: {:?}\n{}\n", market.lanterns, status.phase, market.at.scene());
    match market.keeper_here().and_then(|e| keepers.get(e).ok()) {
        Some((dialogue, log, session)) => {
            let _ = writeln!(text, "{} speaks {} at level {}", dialogue.npc().name, dialogue.target_lang(), dialogue.level());
            log.0.iter().rev().take(LOG_LINES).rev().for_each(|l| _ = writeln!(text, "  {l}"));
            if let Some(s) = session.filter(|_| market.practice != Practice::Idle) {
                write_practice(&mut text, s);
            }
        }
        None => text.push_str("Nobody minds this stall. Walk on: ← →\n"),
    }
    if matches!(market.practice, Practice::AwaitingResult { .. }) {
        text.push_str("Waiting for your result…\n");
    }
    if let Some(e) = last_error.as_deref() {
        let _ = writeln!(text, "error: {e}");
    }
    match &typing.0 {
        Some(line) => _ = writeln!(text, "> {line}_"),
        None => text.push_str("← → walk · Enter talk · + - level · P practise\n"),
    }
    panel.0 = text;
}

fn write_practice(text: &mut String, session: &PracticeSession) {
    let s = session.snapshot();
    let _ = writeln!(text, "practice: {:?}, question {}/{}", s.phase, s.question_index + 1, s.question_count);
    if let Some(q) = &s.question {
        let _ = writeln!(text, "  {}", q.prompt);
        for (i, option) in q.options.iter().flatten().enumerate() {
            let _ = writeln!(text, "  {}. {option}", i + 1);
        }
    }
    text.push_str("1-4 choose · Space reveal/next · Y N grade · Enter answer · Esc finish\n");
}
