use std::process::Command;

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "oru-cli-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

#[test]
fn new_then_check_succeeds() {
    let dir = temp_dir("new");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("new")
        .arg("demo")
        .current_dir(&dir)
        .output()
        .expect("run oru new");
    assert!(output.status.success(), "{output:?}");
    let project = dir.join("demo");
    assert!(project.join("Oru.toml").is_file());
    assert!(project.join("src/main.oru").is_file());

    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("check")
        .current_dir(&project)
        .output()
        .expect("run oru check");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Finished: 0 errors"), "{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_file_exits_2() {
    let dir = temp_dir("bad");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("bad.oru");
    std::fs::write(
        &file,
        "game \"bad\"\nseats 9\nresource hp { per player }\naction slash {}\nwin { when self.hp <= 0 }\n",
    )
    .expect("write");
    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("run check");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[ORU0304]"), "{stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn json_output_is_machine_readable() {
    let dir = temp_dir("json");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("ok.oru");
    std::fs::write(
        &file,
        "game \"ok\"\nseats 2\nresource hp {\n  per player\n  init 1\n}\naction hit {\n  do {\n    other.hp -= 1\n  }\n}\nwin {\n  when other.hp <= 0\n}\n",
    )
    .expect("write");
    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("check")
        .arg(&file)
        .arg("--json")
        .output()
        .expect("run check");
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(value["ok"], true);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_actions_reaches_result() {
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/duel.oru");
    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("run")
        .arg(&example)
        .arg("--actions")
        .arg("slash,slash,slash,slash,slash,slash")
        .output()
        .expect("run oru run");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("击倒了对手"), "{stdout}");
}

#[test]
fn run_board_targets_reach_win_and_render_state() {
    let example =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/moon-chess.oru");
    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("run")
        .arg(&example)
        .arg("--actions")
        .arg("place:a1,place:b1,place:a2,place:b2,place:a3")
        .output()
        .expect("run board");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("连成一线获胜"), "{stdout}");
    assert!(stdout.contains("X"), "board state should render: {stdout}");
}

#[test]
fn interactive_board_commands_accept_action_and_target() {
    let example =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/moon-chess.oru");
    let mut child = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("run")
        .arg(&example)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn oru run");
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"act place a1\nact place b1\nquit\n")
        .expect("write input");
    let output = child.wait_with_output().expect("wait");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("玩家1 在 a1 落子"), "{stdout}");
    assert!(stdout.contains("玩家2 在 b1 落子"), "{stdout}");
}

#[test]
fn run_board_full_without_line_draws() {
    let example =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/moon-chess.oru");
    let output = Command::new(env!("CARGO_BIN_EXE_oru"))
        .arg("run")
        .arg(&example)
        .arg("--actions")
        .arg("place:a1,place:b1,place:c1,place:a2,place:b2,place:a3,place:c2,place:c3,place:b3")
        .output()
        .expect("run board draw");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("棋盘已满，平局"), "{stdout}");
}
