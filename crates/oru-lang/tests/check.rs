use oru_lang::{check, lex, parse};

fn diagnostics(source: &str) -> Vec<oru_lang::Diagnostic> {
    let tokens = lex(source).expect("lex");
    let program = parse(tokens).expect("parse");
    check(&program)
}

fn valid_source() -> &'static str {
    r#"
game "月影对决"
seats 2
edition 1

resource hp {
  per player
  init 12
  min 0
  max 12
}

action slash "挥砍" {
  when self.energy >= 1
  do {
    other.hp -= 4
    self.energy -= 1
  }
}

resource energy {
  per player
  init 2
  min 0
  max 3
}

win {
  when other.hp <= 0
  say "{actor} 击倒了对手"
}
"#
}

#[test]
fn valid_game_has_no_errors() {
    assert!(diagnostics(valid_source()).is_empty());
}

#[test]
fn unknown_resource_is_an_error_with_suggestion() {
    let source = valid_source().replace("self.energy >= 1", "self.energi >= 1");
    let diagnostics = diagnostics(&source);
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "ORU0329")
        .expect("unknown resource");
    assert!(error.message.contains("energi"));
    assert!(error.help.as_deref().unwrap_or("").contains("energy"));
}

#[test]
fn when_requires_bool() {
    let source = valid_source().replace("self.energy >= 1", "self.energy");
    let diagnostics = diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0322")
    );
}

#[test]
fn arithmetic_requires_numbers() {
    let source = valid_source().replace("other.hp -= 4", "other.hp -= true");
    let diagnostics = diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0322")
    );
}

#[test]
fn duplicate_action_is_reported() {
    let source = valid_source().replace(
        "action slash \"挥砍\" {",
        "action slash \"挥砍\" {\n  do { }\n}\n\naction slash \"重复\" {",
    );
    let diagnostics = diagnostics(&source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0313")
    );
}

#[test]
fn game_and_seats_are_required() {
    let source =
        "resource hp {\n  per player\n}\naction slash {\n}\nwin {\n  when self.hp <= 0\n}\n";
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0316")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0317")
    );
}

#[test]
fn parser_reports_unclosed_block() {
    let source = "game \"x\"\nseats 2\nresource hp {\n  per player\n";
    let tokens = lex(source).expect("lex");
    let error = parse(tokens).expect_err("parse should fail");
    assert_eq!(error.code, "ORU0205");
}

#[test]
fn board_family_does_not_require_duel_win_or_action() {
    let source = "game \"棋\"\nseats 2\nfamily board\nboard {\n rows 3\n cols 3\n line 3\n}\n";
    let diagnostics = diagnostics(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn board_family_rejects_duel_action() {
    let source = "game \"棋\"\nseats 2\nfamily board\nboard {\n rows 3\n cols 3\n line 3\n}\naction foo {\n}\n";
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0340")
    );
}

#[test]
fn board_block_requires_explicit_board_family() {
    let source = "game \"棋\"\nseats 2\nboard {\n rows 3\n cols 3\n line 3\n}\n";
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ORU0341")
    );
}
