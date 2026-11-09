use oru_core::{Definition, FamilyDefinition, Match, OutcomeKind, Perspective};
use oru_lang::check_source;

fn board_definition(source: &str) -> Definition {
    let program = check_source(source).expect("board source should check");
    Definition::from_program(&program).expect("board definition should build")
}

fn step(
    game: &mut Match,
    action: &str,
    target: Option<&str>,
) -> Result<oru_core::Step, oru_core::RuleError> {
    let actor = game.actor(game.active())?;
    game.step(actor, action, target, Perspective::Admin)
}

const BOARD: &str = r#"
game "井字棋"
seats 2
maxTurns 9
family board
board {
  rows 3
  cols 3
  line 3
}
"#;

#[test]
fn board_family_builds_initial_grid_and_offers_cells() {
    let definition = board_definition(BOARD);
    assert!(matches!(definition.family(), FamilyDefinition::Board(_)));
    let game = Match::start(definition, None, 1).expect("start");
    let model = game.read(Perspective::Admin).expect("read");
    assert_eq!(model.board.as_ref().expect("board view").cells.len(), 9);
    assert_eq!(model.offers.len(), 9);
    assert_eq!(model.offers[0].target.as_ref().unwrap().id, "a1");
}

#[test]
fn place_uses_target_and_winning_line_ends_match() {
    let mut game = Match::start(board_definition(BOARD), None, 1).expect("start");
    for target in ["a1", "a2", "b1", "b2"] {
        step(&mut game, "place", Some(target)).expect("place");
    }
    let step = step(&mut game, "place", Some("c1")).expect("winning place");
    assert_eq!(game.outcome().unwrap().kind, OutcomeKind::Win);
    assert_eq!(game.outcome().unwrap().winner, Some(0));
    assert!(
        step.events
            .iter()
            .any(|event| event.text().contains("连成一线"))
    );
    assert!(game.offers().unwrap().is_empty());
}

#[test]
fn occupied_or_missing_target_is_rejected_atomically() {
    let mut game = Match::start(board_definition(BOARD), None, 1).expect("start");
    step(&mut game, "place", Some("a1")).expect("first place");
    let before = game.read(Perspective::Admin).unwrap();
    assert!(step(&mut game, "place", Some("a1")).is_err());
    assert!(step(&mut game, "place", None).is_err());
    let after = game.read(Perspective::Admin).unwrap();
    assert_eq!(before.events, after.events);
    assert_eq!(before.board, after.board);
}

#[test]
fn empty_board_cell_offers_are_legal_and_occupied_cells_are_removed() {
    let mut game = Match::start(board_definition(BOARD), None, 1).expect("start");
    let first = game.offers().unwrap();
    assert_eq!(first.len(), 9);
    step(&mut game, "place", Some("b2")).expect("place");
    let next = game.offers().unwrap();
    assert_eq!(next.len(), 8);
    assert!(!next.iter().any(|offer| {
        offer
            .target
            .as_ref()
            .is_some_and(|target| target.id == "b2")
    }));
}

#[test]
fn line_length_supports_short_diagonal_windows() {
    let source = BOARD.replace("line 3", "line 2");
    let mut game = Match::start(board_definition(&source), None, 1).expect("start");
    for (action, target) in [("place", "a1"), ("place", "c1"), ("place", "b2")] {
        step(&mut game, action, Some(target)).expect("place");
    }
    assert_eq!(game.outcome().unwrap().winner, Some(0));
}

#[test]
fn full_board_draws_without_line() {
    let mut game = Match::start(board_definition(BOARD), None, 1).expect("start");
    // X O X / O X X / O X O：满盘但没有一条三连线。
    for target in ["a1", "b1", "c1", "a2", "b2", "a3", "c2", "c3", "b3"] {
        step(&mut game, "place", Some(target)).expect("place");
    }
    let outcome = game.outcome().expect("draw");
    assert_eq!(outcome.kind, OutcomeKind::Draw);
    assert!(outcome.text.contains("棋盘已满"));
}
