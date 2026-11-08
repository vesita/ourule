//! `oru` CLI：M0 的项目脚手架与检查器。
//!
//! 这个二进制只做三件事：创建项目、读取 manifest、把 `.oru` 交给 `oru-lang` 检查。
//! 运行时/包解析/LSP 都在后续里程碑，不在 CLI 里提前长出来。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oru_core::{Definition, Match, Perspective, ReadModel};
use oru_lang::Diagnostic;
use oru_lang::ast::Program;
use oru_lang::manifest::MANIFEST_NAME;
use oru_lang::{Manifest, Severity, check, lex, parse};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("oru: {message}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        None | Some("--help") | Some("-h") | Some("help") => {
            print_help();
            Ok(ExitCode::SUCCESS)
        }
        Some("--version") | Some("-V") => {
            println!("oru {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Some("new") => cmd_new(&args[1..]),
        Some("init") => cmd_init(&args[1..]),
        Some("check") => cmd_check(&args[1..]),
        Some("run") => cmd_run(&args[1..]),
        Some(other) => Err(format!("未知命令 `{other}`；oru --help 查看用法")),
    }
}

fn print_help() {
    println!(
        "\
oru —— 面向对话场景的规则驱动游戏语言

用法：
  oru new <name>       创建项目（Oru.toml + src/main.oru）
  oru init             在当前目录创建项目
  oru check [path]     检查项目或单个 .oru 文件
  oru check --json     机器可读诊断（CI/编辑器）
  oru run [path]       开局并交互推进（act <id> / state / quit）
  oru run --actions a,b --seed 42   非交互跑一串动作
  oru --version

M0 检查：词法/语法/名字解析/类型/基础规则约束。
"
    );
}

fn cmd_new(args: &[String]) -> Result<ExitCode, String> {
    let Some(name) = args.first() else {
        return Err("oru new 需要项目名，例如 oru new moon-chess".to_string());
    };
    if args.len() > 1 {
        return Err("oru new 目前只接受一个项目名".to_string());
    }
    if !valid_package_name(name) {
        return Err("项目名必须是小写字母/数字/`-`/`_`，2–50 个字符，并以字母开头".to_string());
    }
    let dir = PathBuf::from(name);
    if dir.exists() {
        return Err(format!("目录 `{}` 已存在", dir.display()));
    }
    create_project(&dir, name)?;
    println!("创建了 Oru 项目 `{}`", dir.display());
    println!("下一步：cd {} && oru check", dir.display());
    Ok(ExitCode::SUCCESS)
}

fn cmd_init(args: &[String]) -> Result<ExitCode, String> {
    if !args.is_empty() {
        return Err("oru init 不接受额外参数".to_string());
    }
    let dir = std::env::current_dir().map_err(|error| format!("取当前目录失败：{error}"))?;
    if dir.join(MANIFEST_NAME).exists() {
        return Err(format!("{} 已存在", dir.join(MANIFEST_NAME).display()));
    }
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("oru-game");
    if !valid_package_name(name) {
        return Err("当前目录名不能作为包名；请先重命名目录".to_string());
    }
    create_project(&dir, name)?;
    println!("在 {} 初始化了 Oru 项目", dir.display());
    Ok(ExitCode::SUCCESS)
}

fn create_project(dir: &Path, name: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir.join("src")).map_err(|error| format!("创建目录失败：{error}"))?;
    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2026\"\nentry = \"src/main.oru\"\ndescription = \"A new Oru game\"\n"
    );
    std::fs::write(dir.join(MANIFEST_NAME), manifest)
        .map_err(|error| format!("写 {MANIFEST_NAME} 失败：{error}"))?;
    std::fs::write(dir.join("src/main.oru"), sample_game(name))
        .map_err(|error| format!("写 src/main.oru 失败：{error}"))?;
    let gitignore = "/target\n";
    std::fs::write(dir.join(".gitignore"), gitignore)
        .map_err(|error| format!("写 .gitignore 失败：{error}"))?;
    Ok(())
}

fn sample_game(name: &str) -> String {
    let title = name.replace(['-', '_'], " ");
    format!(
        r#"# {title}
game "{title}"
seats 2
edition 1

resource hp {{
  per player
  init 12
  min 0
  max 12
}}

resource energy {{
  per player
  init 2
  min 0
  max 3
}}

action attack "攻击" {{
  when self.energy >= 1
  do {{
    other.hp -= 4
    self.energy -= 1
  }}
}}

on turnEnd {{
  self.energy += 1
}}

win {{
  when other.hp <= 0
  say "{{actor}} 击倒了对手"
}}
"#
    )
}

fn cmd_check(args: &[String]) -> Result<ExitCode, String> {
    let mut json = false;
    let mut path: Option<&str> = None;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                print_help();
                return Ok(ExitCode::SUCCESS);
            }
            other if other.starts_with('-') => {
                return Err(format!("check 不认识参数 `{other}`"));
            }
            other => {
                if path.replace(other).is_some() {
                    return Err("check 只接受一个路径".to_string());
                }
            }
        }
    }

    let target = resolve_target(path.map(Path::new))?;
    let source = std::fs::read_to_string(&target.file)
        .map_err(|error| format!("读不到 {}：{error}", target.file.display()))?;
    let diagnostics = match checked_program(&source) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    };
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();

    if json {
        let payload = serde_json::json!({
            "ok": errors == 0,
            "file": target.file.display().to_string(),
            "package": target.package,
            "errors": errors,
            "warnings": diagnostics.len().saturating_sub(errors),
            "diagnostics": diagnostics,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("JSON 序列化失败：{error}"))?
        );
    } else {
        let label = target.file.display().to_string();
        if let Some(package) = &target.package {
            println!("Checking {package} ({label})");
        } else {
            println!("Checking {label}");
        }
        for diagnostic in &diagnostics {
            eprint!("{}", diagnostic.render_with_path(&label, &source));
        }
        if errors == 0 {
            println!("Finished: 0 errors, {} warnings", diagnostics.len());
        } else {
            eprintln!(
                "Finished: {errors} errors, {} warnings",
                diagnostics.len().saturating_sub(errors)
            );
        }
    }

    if errors == 0 {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

fn cmd_run(args: &[String]) -> Result<ExitCode, String> {
    let mut seats: Option<usize> = None;
    let mut seed: u64 = 42;
    let mut actions: Option<Vec<String>> = None;
    let mut json = false;
    let mut path: Option<&str> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seats" => {
                index += 1;
                seats = Some(
                    args.get(index)
                        .ok_or("--seats 需要数字")?
                        .parse::<usize>()
                        .map_err(|_| "--seats 需要数字")?,
                );
            }
            "--seed" => {
                index += 1;
                seed = args
                    .get(index)
                    .ok_or("--seed 需要数字")?
                    .parse::<u64>()
                    .map_err(|_| "--seed 需要数字")?;
            }
            "--actions" => {
                index += 1;
                let value = args.get(index).ok_or("--actions 需要动作 id 列表")?;
                let list: Vec<String> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect();
                if list.is_empty() {
                    return Err("--actions 列表不能为空".to_string());
                }
                actions = Some(list);
            }
            "--json" => json = true,
            "--help" | "-h" => {
                print_help();
                return Ok(ExitCode::SUCCESS);
            }
            other if other.starts_with('-') => return Err(format!("run 不认识参数 `{other}`")),
            other => {
                if path.replace(other).is_some() {
                    return Err("run 只接受一个路径".to_string());
                }
            }
        }
        index += 1;
    }

    let target = resolve_target(path.map(Path::new))?;
    let source = std::fs::read_to_string(&target.file)
        .map_err(|error| format!("读不到 {}：{error}", target.file.display()))?;
    let program = match checked_program(&source) {
        Ok(program) => program,
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprint!(
                    "{}",
                    diagnostic.render_with_path(&target.file.display().to_string(), &source)
                );
            }
            return Ok(ExitCode::from(2));
        }
    };
    let definition =
        Definition::from_program(&program).map_err(|error| format!("规则定义失败：{error}"))?;
    let mut game =
        Match::start(definition, seats, seed).map_err(|error| format!("开局失败：{error}"))?;

    if json {
        let initial = game
            .read(Perspective::Admin)
            .map_err(|error| error.to_string())?;
        let mut steps = Vec::new();
        if let Some(actions) = &actions {
            for action in actions {
                let step = game
                    .step(action)
                    .map_err(|error| format!("动作 `{action}` 失败：{error}"))?;
                steps.push(step);
            }
        }
        let payload = serde_json::json!({
            "model": initial,
            "steps": steps,
            "final": game.read(Perspective::Admin).map_err(|error| error.to_string())?,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("JSON 序列化失败：{error}"))?
        );
        return Ok(ExitCode::SUCCESS);
    }

    print_model(
        &game
            .read(Perspective::Admin)
            .map_err(|error| error.to_string())?,
    );
    if let Some(actions) = &actions {
        for action in actions {
            let step = game
                .step(action)
                .map_err(|error| format!("动作 `{action}` 失败：{error}"))?;
            print_step(&step);
            if step.model.finished {
                break;
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    interactive(&mut game)?;
    Ok(ExitCode::SUCCESS)
}

fn interactive(game: &mut Match) -> Result<(), String> {
    use std::io::{self, Write};
    let stdin = io::stdin();
    loop {
        print!("> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        if stdin
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return Ok(());
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "quit" | "exit" | "q" => return Ok(()),
            "state" | "s" => {
                print_model(
                    &game
                        .read(Perspective::Admin)
                        .map_err(|error| error.to_string())?,
                );
            }
            "help" | "h" => println!("act <id> | state | quit"),
            other => {
                let Some(action) = other.strip_prefix("act ") else {
                    println!("未知命令；用 act <id> / state / quit");
                    continue;
                };
                match game.step(action.trim()) {
                    Ok(step) => {
                        print_step(&step);
                        if step.model.finished {
                            return Ok(());
                        }
                    }
                    Err(error) => println!("动作失败：{error}"),
                }
            }
        }
    }
}

fn print_step(step: &oru_core::Step) {
    for event in &step.events {
        println!("  · {}", event.text());
    }
}

fn print_model(model: &ReadModel) {
    println!(
        "{} · 第 {} 回合 · 轮到 玩家{}",
        model.title,
        model.turn,
        model.active + 1
    );
    for (name, resource) in &model.resources {
        let values: Vec<String> = resource
            .values
            .iter()
            .map(|value| value.map_or("?".to_string(), |value| value.to_string()))
            .collect();
        println!("  {name}: {}", values.join(" / "));
    }
    if model.finished {
        if let Some(outcome) = &model.outcome {
            println!("对局结束：{}", outcome.text);
        }
        return;
    }
    let offers: Vec<String> = model
        .offers
        .iter()
        .map(|offer| format!("{}({})", offer.id, offer.label))
        .collect();
    println!("可选动作: {}", offers.join(", "));
}

fn checked_program(source: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lex(source)?;
    let program = parse(tokens).map_err(|diagnostic| vec![diagnostic])?;
    let diagnostics = check(&program);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        Err(diagnostics)
    } else {
        Ok(program)
    }
}

struct Target {
    file: PathBuf,
    package: Option<String>,
}

fn resolve_target(path: Option<&Path>) -> Result<Target, String> {
    let Some(path) = path else {
        let cwd = std::env::current_dir().map_err(|error| format!("取当前目录失败：{error}"))?;
        let project = oru_lang::find_project(&cwd)
            .ok_or("当前目录找不到 Oru.toml；先 oru new/ init，或 oru check <文件>")?;
        return project_target(&project);
    };
    if path.is_dir() {
        return project_target(path);
    }
    if path.file_name().and_then(|name| name.to_str()) == Some(MANIFEST_NAME) {
        return project_target(path.parent().unwrap_or(Path::new(".")));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("oru") {
        return Err(format!("{} 不是 .oru 文件", path.display()));
    }
    Ok(Target {
        file: path.to_path_buf(),
        package: None,
    })
}

fn project_target(dir: &Path) -> Result<Target, String> {
    let manifest = Manifest::load_in_dir(dir).map_err(|error| error.to_string())?;
    let file = manifest.entry_path(dir);
    if !file.is_file() {
        return Err(format!(
            "manifest 的 entry 不存在：{}（先创建它）",
            file.display()
        ));
    }
    Ok(Target {
        file,
        package: Some(format!(
            "{} v{}",
            manifest.package.name, manifest.package.version
        )),
    })
}

fn valid_package_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=50).contains(&bytes.len())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-' || *byte == b'_'
        })
        && bytes[0].is_ascii_lowercase()
}
