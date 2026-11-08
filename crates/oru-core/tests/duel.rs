use oru_core::{Definition, Match, OutcomeKind, Perspective, RuleError};
use oru_lang::check_source;

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
fn duel_mutable_resources_live_inside_duel_family_state() {
    let game = Match::start(definition(DUEL), None, 42).expect("start");
    let oru_core::FamilyState::Duel(duel) = game.family_state() else {
        panic!("duel Definition must produce DuelState");
    };
    assert!(matches!(
        duel.resources.get("hp"),
        Some(oru_core::ResourceValues::Player(values)) if values == &[12, 12]
    ));
    assert!(matches!(
        duel.resources.get("energy"),
        Some(oru_core::ResourceValues::Player(values)) if values == &[2, 2]
    ));
    assert!(
        game.read(Perspective::Admin)
            .unwrap()
            .resources
            .contains_key("hp")
    );
}

#[test]
fn slash_changes_resources_and_advances_turn() {
    let mut game = Match::start(definition(DUEL), None, 42).expect("start");
    let step = game.step("slash", None).expect("step");
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
    let error = game.step("nope", None).expect_err("unknown action");
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
        game.step(action, None).expect("step");
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
    game.step("pass", None).expect("first");
    assert_eq!(game.taken(), 1, "每个成功动作都记入帧计数");
    game.step("pass", None).expect("second");
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
    let step = game.step("wait", None).expect("step");
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
fn private_resource_hides_other_values() {
    let source = r#"
game "private"
seats 2
resource secret {
  per player
  init 3
  visibility private
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
    assert_eq!(admin.resources["secret"].values, vec![Some(3), Some(3)]);
    let seat0 = game.read(Perspective::Seat(0)).expect("seat");
    assert_eq!(seat0.resources["secret"].values, vec![Some(3), None]);
    let spectator = game.read(Perspective::Spectator).expect("spectator");
    assert_eq!(spectator.resources["secret"].values, vec![None, None]);
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
    game.step("spend", None).expect("step");
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
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.step("bump", None)));
    assert!(result.is_ok(), "overflow must not panic");
    assert!(result.unwrap().is_err(), "overflow should be a rule error");
}
