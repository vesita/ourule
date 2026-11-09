use oru_core::{Definition, Match, OutcomeKind, Perspective, RuleError};
use oru_lang::check_source;

fn step(game: &mut Match, action: &str, target: Option<&str>) -> Result<oru_core::Step, RuleError> {
    let actor = game.actor(game.active())?;
    game.step(actor, action, target, Perspective::Admin)
}

fn step_as(
    game: &mut Match,
    actor_seat: usize,
    action: &str,
    perspective: Perspective,
) -> Result<oru_core::Step, RuleError> {
    let actor = game.actor(actor_seat)?;
    game.step(actor, action, None, perspective)
}

fn definition(source: &str) -> Definition {
    let program = check_source(source).expect("source should check");
    Definition::from_program(&program).expect("definition should build")
}

const DUEL: &str = r#"
game "月影对决"
seats 2
maxTurns 20

resource hp {
  per player
  init 12
  min 0
  max 12
}

resource energy {
  per player
  init 2
  min 0
  max 3
}

action slash "挥砍" {
  when self.energy >= 1
  do {
    other.hp -= 4
    self.energy -= 1
  }
}

action meditate "冥想" {
  do {
    self.energy += 1
  }
}

on turnEnd {
  self.energy += 1
}

win {
  when other.hp <= 0
  say "{actor} 击倒了对手"
}
"#;

#[test]
fn start_emits_match_and_turn_events() {
    let game = Match::start(definition(DUEL), None, 42).expect("start");
    assert_eq!(game.active(), 0);
    assert_eq!(game.turn(), 1);
    let model = game.read(Perspective::Admin).expect("read");
    assert!(!model.finished);
    assert!(model.offers.iter().any(|offer| offer.id == "slash"));
    assert!(
        model
            .events
            .iter()
            .any(|event| format!("{:?}", event.kind).contains("MatchStarted"))
    );
}

#[test]
fn duel_state_and_resource_view_start_consistently() {
    let game = Match::start(definition(DUEL), None, 42).expect("start");
    let model = game.read(Perspective::Admin).unwrap();
    assert_eq!(model.resources["hp"].values, vec![Some(12), Some(12)]);
    assert_eq!(model.resources["energy"].values, vec![Some(2), Some(2)]);
}

#[test]
fn slash_changes_resources_and_advances_turn() {
    let mut game = Match::start(definition(DUEL), None, 42).expect("start");
    let step = step(&mut game, "slash", None).expect("step");
    assert!(
        step.events
            .iter()
            .any(|event| event.text().contains("挥砍"))
    );
    assert!(
        step.events
            .iter()
            .any(|event| event.text().contains("hp：12 → 8"))
    );
    assert_eq!(game.active(), 1);
    assert_eq!(game.turn(), 1);
    assert_eq!(game.taken(), 1);
}

#[test]
fn invalid_action_is_atomic() {
    let mut game = Match::start(definition(DUEL), None, 42).expect("start");
    let before = game.read(Perspective::Admin).expect("read");
    let error = step(&mut game, "nope", None).expect_err("unknown action");
    assert!(matches!(error, RuleError::UnknownAction(_)));
    let after = game.read(Perspective::Admin).expect("read");
    assert_eq!(before.events, after.events);
    assert_eq!(before.resources, after.resources);
    assert_eq!(before.active, after.active);
}

#[test]
fn game_reaches_win() {
    let mut game = Match::start(definition(DUEL), None, 7).expect("start");
    for _ in 0..20 {
        if game.outcome().is_some() {
            break;
        }
        let offers = game.offers().expect("offers");
        let action = if offers.iter().any(|offer| offer.id == "slash") {
            "slash"
        } else {
            "meditate"
        };
        step(&mut game, action, None).expect("step");
    }
    let outcome = game.outcome().expect("should finish");
    assert_eq!(outcome.kind, OutcomeKind::Win);
    assert!(outcome.text.contains("击倒"));
}

#[test]
fn max_turns_draws() {
    let source = r#"
game "draw"
seats 2
maxTurns 1
resource hp {
  per player
  init 1
}
action pass {
  do { }
}
win {
  when false
}
"#;
    let mut game = Match::start(definition(source), None, 1).expect("start");
    step(&mut game, "pass", None).expect("first");
    assert_eq!(game.taken(), 1, "每个成功动作都记入帧计数");
    step(&mut game, "pass", None).expect("second");
    assert_eq!(game.taken(), 2);
    let outcome = game.outcome().expect("draw");
    assert_eq!(outcome.kind, OutcomeKind::Draw);
}

#[test]
fn turn_end_can_end_game_without_advancing_to_next_turn() {
    let source = r#"
game "turn-end"
seats 2
resource hp {
  per player
  init 2
}
action wait { }
on turnEnd {
  end draw "回合收尾结束"
}
win {
  when false
}
"#;
    let mut game = Match::start(definition(source), None, 1).expect("start");
    let step = step(&mut game, "wait", None).expect("step");
    assert_eq!(game.outcome().unwrap().kind, OutcomeKind::Draw);
    assert_eq!(game.taken(), 1, "已接受的动作计数，即使 turnEnd 结束对局");
    assert!(
        !step
            .events
            .iter()
            .any(|event| matches!(event.kind, oru_core::EventKind::TurnStarted { .. }))
    );
}

#[test]
fn private_resource_hides_other_values_and_filters_action_offers() {
    let source = r#"
game "private"
seats 2
resource secret {
  per player
  init 1
  visibility private
}
action probe "probe" {
  when self.secret == 1
}
action pass {
  do { }
}
win {
  when false
}
"#;
    let game = Match::start(definition(source), None, 1).expect("start");
    let admin = game.read(Perspective::Admin).expect("admin");
    assert_eq!(admin.resources["secret"].values, vec![Some(1), Some(1)]);
    assert!(admin.offers.iter().any(|offer| offer.id == "probe"));
    let seat0 = game.read(Perspective::Seat(0)).expect("seat");
    assert_eq!(seat0.resources["secret"].values, vec![Some(1), None]);
    assert!(seat0.offers.iter().any(|offer| offer.id == "probe"));
    let spectator = game.read(Perspective::Spectator).expect("spectator");
    assert_eq!(spectator.resources["secret"].values, vec![None, None]);
    assert!(!spectator.offers.iter().any(|offer| offer.id == "probe"));
    assert!(spectator.offers.iter().any(|offer| offer.id == "pass"));
}

#[test]
fn private_resource_change_event_is_filtered_by_perspective() {
    let source = r#"
game "private"
seats 2
resource secret {
  per player
  init 1
  visibility private
}
action spend {
  do {
    self.secret -= 1
  }
}
win {
  when false
}
"#;
    let mut game = Match::start(definition(source), None, 1).expect("start");
    step_as(&mut game, 0, "spend", Perspective::Seat(0)).expect("step");
    let seat0 = game.read(Perspective::Seat(0)).unwrap();
    let seat1 = game.read(Perspective::Seat(1)).unwrap();
    let admin = game.read(Perspective::Admin).unwrap();
    assert!(
        seat0
            .events
            .iter()
            .any(|event| event.text().contains("secret"))
    );
    assert!(
        !seat1
            .events
            .iter()
            .any(|event| event.text().contains("secret"))
    );
    assert!(
        admin
            .events
            .iter()
            .any(|event| event.text().contains("secret"))
    );
}

#[test]
fn stale_actor_capability_is_rejected_without_mutation() {
    let source = r#"
game "actor"
seats 2
resource count {
  per game
  init 0
}
action tick {
  do { game.count += 1 }
}
win {
  when false
}
"#;
    let mut game = Match::start(definition(source), None, 1).expect("start");
    let stale = game.actor(0).expect("actor at turn 1");
    game.step(stale.clone(), "tick", None, Perspective::Seat(0))
        .expect("turn 1");
    let before = game.read(Perspective::Admin).expect("before");
    let result = game.step(stale, "tick", None, Perspective::Seat(1));
    assert!(result.is_err(), "stale actor cannot replay its capability");
    assert_eq!(game.read(Perspective::Admin).unwrap(), before);

    let other = Match::start(definition(source), None, 1).expect("second match");
    let foreign = other.actor(0).unwrap();
    let error = game
        .step(foreign, "tick", None, Perspective::Seat(0))
        .unwrap_err();
    assert!(matches!(error, RuleError::NotAllowed { .. }));
    assert_eq!(game.read(Perspective::Admin).unwrap(), before);
}

#[test]
fn spectator_cannot_probe_private_action_through_step() {
    let source = r#"
game "private-oracle"
seats 2
resource secret {
  per player
  init 1
  visibility private
}
action probe { when self.secret == 1 }
win { when false }
"#;
    let mut game = Match::start(definition(source), None, 1).expect("start");
    let spectator = game.read(Perspective::Spectator).unwrap();
    assert!(!spectator.offers.iter().any(|offer| offer.id == "probe"));
    let actor = game.actor(0).unwrap();
    let before = game.read(Perspective::Admin).unwrap();
    let error = game
        .step(actor, "probe", None, Perspective::Spectator)
        .expect_err("spectator view must not probe private action");
    assert!(matches!(error, RuleError::NotAllowed { .. }));
    assert_eq!(game.read(Perspective::Admin).unwrap(), before);
}

#[test]
fn boolean_operators_short_circuit() {
    let source = r#"
game "short-circuit"
seats 2
resource hp {
  per player
  init 1
}
action pass {
  when false and (1 / 0 == 0)
}
win {
  when false
}
"#;
    let game = Match::start(definition(source), None, 1).expect("start");
    assert!(
        game.offers().unwrap().is_empty(),
        "right side of false and is not evaluated"
    );
}

#[test]
fn integer_overflow_returns_rule_error_without_panicking() {
    let source = r#"
game "overflow"
seats 2
resource value {
  per game
  init 0
}
action bump {
  do {
    game.value = (-9223372036854775807 - 1) / -1
  }
}
win {
  when false
}
"#;
    let mut game = Match::start(definition(source), None, 1).expect("start");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        step(&mut game, "bump", None)
    }));
    assert!(result.is_ok(), "overflow must not panic");
    assert!(result.unwrap().is_err(), "overflow should be a rule error");
}
