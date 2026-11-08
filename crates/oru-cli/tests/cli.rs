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
