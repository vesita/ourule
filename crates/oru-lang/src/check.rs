//! 语义检查：名字解析 + 类型检查 + 规则约束。
//!
//! M0 的检查器只依赖 AST，不读文件、不跑运行时；`oru check` 的所有“编译期错误”都在这里。

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::span::Span;

#[derive(Debug, Clone, Copy)]
struct ResourceInfo {
    per: Per,
    max: Option<i64>,
    span: Span,
}

#[derive(Debug, Default)]
struct Ctx {
    game: Option<String>,
    seats: Option<u32>,
    edition: Option<u32>,
    resources: BTreeMap<String, ResourceInfo>,
    actions: BTreeSet<String>,
    wins: usize,
}

pub fn check(program: &Program) -> Vec<Diagnostic> {
    let mut ctx = Ctx::default();
    let mut diagnostics = Vec::new();

    // 第一遍：先收集声明。这样 action 可以引用后面才声明的 resource。
    for item in &program.items {
        match item {
            Item::Game { name, span } => {
                if ctx.game.replace(name.clone()).is_some() {
                    diagnostics.push(Diagnostic::error("ORU0301", "game 只能声明一次", *span));
                }
                if name.trim().is_empty() {
                    diagnostics.push(Diagnostic::error("ORU0302", "game 名称不能为空", *span));
                }
            }
            Item::Seats { count, span } => {
                if ctx.seats.replace(*count).is_some() {
                    diagnostics.push(Diagnostic::error("ORU0303", "seats 只能声明一次", *span));
                }
                if !(2..=8).contains(count) {
                    diagnostics.push(Diagnostic::error(
                        "ORU0304",
                        format!("seats 必须在 2–8，拿到 {count}"),
                        *span,
                    ));
                }
            }
            Item::Edition { edition, span } => {
                if ctx.edition.replace(*edition).is_some() {
                    diagnostics.push(Diagnostic::error("ORU0305", "edition 只能声明一次", *span));
                }
                if *edition != 1 {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0306",
                            format!("M0 只支持 edition 1，拿到 {edition}"),
                            *span,
                        )
                        .with_help("先删掉 edition 或写成 edition 1"),
                    );
                }
            }
            Item::Resource(resource) => {
                if !valid_name(&resource.name) {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0307",
                            format!("资源名 `{}` 不合法", resource.name),
                            resource.name_span,
                        )
                        .with_help("资源名必须是小写字母/数字/下划线，2–32 个字符"),
                    );
                }
                if ctx.resources.contains_key(&resource.name) {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0308",
                            format!("资源 `{}` 重复声明", resource.name),
                            resource.name_span,
                        )
                        .with_help("每个资源只能声明一次"),
                    );
                }
                if resource.per == Per::Team {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0309",
                            "M0 还不支持 team 作用域资源",
                            resource.per_span.unwrap_or(resource.name_span),
                        )
                        .with_help("先用 player / game；team 会在 M1 加入"),
                    );
                }
                if let (Some(min), Some(max)) = (resource.min, resource.max)
                    && min > max
                {
                    diagnostics.push(Diagnostic::error(
                        "ORU0310",
                        format!("资源 `{}` 的 min({min}) 大于 max({max})", resource.name),
                        resource.span,
                    ));
                }
                let init = resource.init.unwrap_or(0);
                if resource.min.is_some_and(|min| init < min)
                    || resource.max.is_some_and(|max| init > max)
                {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0311",
                            format!(
                                "资源 `{}` 的 init({init}) 不在 [{}, {}] 内",
                                resource.name,
                                resource.min.map_or("-∞".to_string(), |v| v.to_string()),
                                resource.max.map_or("∞".to_string(), |v| v.to_string())
                            ),
                            resource.span,
                        )
                        .with_help("把 init 改到 min..=max 之间"),
                    );
                }
                ctx.resources.insert(
                    resource.name.clone(),
                    ResourceInfo {
                        per: resource.per,
                        max: resource.max,
                        span: resource.name_span,
                    },
                );
            }
            Item::Action(_) | Item::Trigger(_) | Item::Win(_) => {}
        }
    }

    // 第二遍：检查规则体。
    for item in &program.items {
        match item {
            Item::Action(action) => {
                if !valid_action_id(&action.id) {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0312",
                            format!("动作 id `{}` 不合法", action.id),
                            action.id_span,
                        )
                        .with_help("动作 id 使用小写字母/数字/`-`/`_`，2–32 个字符"),
                    );
                }
                if !ctx.actions.insert(action.id.clone()) {
                    diagnostics.push(
                        Diagnostic::error(
                            "ORU0313",
                            format!("动作 `{}` 重复声明", action.id),
                            action.id_span,
                        )
                        .with_help("每个动作只能声明一次"),
                    );
                }
                if let Some(when) = &action.when {
                    expect_type(when, Type::Bool, &ctx, &mut diagnostics, "when 条件");
                }
                check_effects(&action.effects, &ctx, &mut diagnostics);
            }
            Item::Trigger(trigger) => {
                match trigger.on.as_str() {
                    "start" | "turnStart" | "turnEnd" => {}
                    other => diagnostics.push(
                        Diagnostic::error(
                            "ORU0314",
                            format!("未知触发时机 `{other}`"),
                            trigger.on_span,
                        )
                        .with_help("M0 可用：start / turnStart / turnEnd"),
                    ),
                }
                check_effects(&trigger.effects, &ctx, &mut diagnostics);
            }
            Item::Win(win) => {
                ctx.wins += 1;
                expect_type(&win.when, Type::Bool, &ctx, &mut diagnostics, "win.when");
                if let Some(say) = &win.say
                    && say.trim().is_empty()
                {
                    diagnostics.push(Diagnostic::warning(
                        "ORU0315",
                        "win.say 是空字符串",
                        win.span,
                    ));
                }
            }
            Item::Game { .. } | Item::Seats { .. } | Item::Edition { .. } | Item::Resource(_) => {}
        }
    }

    if ctx.game.is_none() {
        diagnostics.push(
            Diagnostic::error("ORU0316", "缺少 game 声明", first_span(program))
                .with_help("在文件顶部写 `game \"游戏名\"`"),
        );
    }
    if ctx.seats.is_none() {
        diagnostics.push(
            Diagnostic::error("ORU0317", "缺少 seats 声明", first_span(program))
                .with_help("写 `seats 2`（2–8）"),
        );
    }
    if ctx.actions.is_empty() {
        diagnostics.push(
            Diagnostic::error("ORU0318", "至少需要一个 action", first_span(program))
                .with_help("动作是玩家可以做的选择"),
        );
    }
    if ctx.wins == 0 {
        diagnostics.push(
            Diagnostic::error("ORU0319", "至少需要一个 win 条件", first_span(program))
                .with_help("例如 `win { when other.hp <= 0 }`"),
        );
    }

    diagnostics
}

fn check_effects(effects: &[Effect], ctx: &Ctx, diagnostics: &mut Vec<Diagnostic>) {
    for effect in effects {
        match &effect.kind {
            EffectKind::Set { path, value }
            | EffectKind::Add { path, value }
            | EffectKind::Sub { path, value } => {
                check_path(path, ctx, diagnostics);
                expect_type(value, Type::Int, ctx, diagnostics, "赋值右侧");
            }
            EffectKind::Say { text } => {
                if text.trim().is_empty() {
                    diagnostics.push(Diagnostic::warning(
                        "ORU0320",
                        "say 的文本是空的",
                        effect.span,
                    ));
                }
            }
            EffectKind::End { text, .. } => {
                if let Some(text) = text
                    && text.trim().is_empty()
                {
                    diagnostics.push(Diagnostic::warning(
                        "ORU0321",
                        "end 的文案是空的",
                        effect.span,
                    ));
                }
            }
        }
    }
}

fn expect_type(
    expr: &Expr,
    expected: Type,
    ctx: &Ctx,
    diagnostics: &mut Vec<Diagnostic>,
    what: &str,
) {
    let actual = expr_type(expr, ctx, diagnostics);
    if actual != Type::Unknown && actual != expected {
        diagnostics.push(
            Diagnostic::error(
                "ORU0322",
                format!(
                    "{what} 需要 {}，拿到 {}",
                    type_name(expected),
                    type_name(actual)
                ),
                expr.span,
            )
            .with_help("检查表达式里的变量类型/比较操作"),
        );
    }
}

fn expr_type(expr: &Expr, ctx: &Ctx, diagnostics: &mut Vec<Diagnostic>) -> Type {
    match &expr.kind {
        ExprKind::Int(_) => Type::Int,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::Path(path) => {
            check_path(path, ctx, diagnostics);
            Type::Int
        }
        ExprKind::Unary { op, expr } => {
            let inner = expr_type(expr, ctx, diagnostics);
            match op {
                UnaryOp::Not => {
                    if inner != Type::Unknown && inner != Type::Bool {
                        diagnostics.push(Diagnostic::error("ORU0323", "`!` 需要布尔值", expr.span));
                    }
                    Type::Bool
                }
                UnaryOp::Neg => {
                    if inner != Type::Unknown && inner != Type::Int {
                        diagnostics.push(Diagnostic::error("ORU0324", "`-` 需要数字", expr.span));
                    }
                    Type::Int
                }
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left_ty = expr_type(left, ctx, diagnostics);
            let right_ty = expr_type(right, ctx, diagnostics);
            match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                    if (left_ty != Type::Unknown && left_ty != Type::Int)
                        || (right_ty != Type::Unknown && right_ty != Type::Int)
                    {
                        diagnostics.push(Diagnostic::error(
                            "ORU0325",
                            "算术运算需要两个数字",
                            expr.span,
                        ));
                    }
                    Type::Int
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    if (left_ty != Type::Unknown && left_ty != Type::Int)
                        || (right_ty != Type::Unknown && right_ty != Type::Int)
                    {
                        diagnostics.push(Diagnostic::error(
                            "ORU0326",
                            "比较运算需要两个数字",
                            expr.span,
                        ));
                    }
                    Type::Bool
                }
                BinaryOp::Eq | BinaryOp::Ne => {
                    if left_ty != right_ty && left_ty != Type::Unknown && right_ty != Type::Unknown
                    {
                        diagnostics.push(Diagnostic::error(
                            "ORU0327",
                            "`==` / `!=` 两边类型必须一致",
                            expr.span,
                        ));
                    }
                    Type::Bool
                }
                BinaryOp::And | BinaryOp::Or => {
                    if (left_ty != Type::Unknown && left_ty != Type::Bool)
                        || (right_ty != Type::Unknown && right_ty != Type::Bool)
                    {
                        diagnostics.push(Diagnostic::error(
                            "ORU0328",
                            "`and` / `or` 需要两个布尔值",
                            expr.span,
                        ));
                    }
                    Type::Bool
                }
            }
        }
    }
}

fn check_path(path: &Path, ctx: &Ctx, diagnostics: &mut Vec<Diagnostic>) {
    let Some(info) = ctx.resources.get(&path.name) else {
        let mut diagnostic = Diagnostic::error(
            "ORU0329",
            format!("未声明的资源 `{}`", path.name),
            path.span,
        );
        if let Some(suggestion) = suggest(&path.name, ctx.resources.keys()) {
            diagnostic = diagnostic.with_help(format!("你是想写 `{suggestion}` 吗？"));
        } else {
            diagnostic = diagnostic.with_help("先在 resource 块里声明它");
        }
        diagnostics.push(diagnostic);
        return;
    };
    let expected = match path.root {
        PathRoot::SelfSeat | PathRoot::Other => Per::Player,
        PathRoot::Game => Per::Game,
    };
    if info.per != expected {
        diagnostics.push(
            Diagnostic::error(
                "ORU0330",
                format!(
                    "资源 `{}` 是 {} 作用域，不能用 `{}` 访问",
                    path.name,
                    per_name(info.per),
                    root_name(path.root)
                ),
                path.span,
            )
            .with_help(format!(
                "把路径改成 {} 作用域，或把资源改成 {}",
                root_name(path.root),
                per_name(expected)
            )),
        );
    }
    if let Some(max) = info.max
        && max < 0
    {
        diagnostics.push(Diagnostic::warning(
            "ORU0331",
            format!("资源 `{}` 的 max 是负数", path.name),
            info.span,
        ));
    }
}

fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        && bytes[0].is_ascii_lowercase()
}

fn valid_action_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-' || *byte == b'_'
        })
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
}

fn first_span(program: &Program) -> Span {
    program
        .items
        .first()
        .map(|item| match item {
            Item::Game { span, .. }
            | Item::Seats { span, .. }
            | Item::Edition { span, .. }
            | Item::Resource(Resource { span, .. })
            | Item::Action(Action { span, .. })
            | Item::Trigger(Trigger { span, .. })
            | Item::Win(WinRule { span, .. }) => *span,
        })
        .unwrap_or(Span::empty(0))
}

fn type_name(ty: Type) -> &'static str {
    match ty {
        Type::Int => "数字",
        Type::Bool => "布尔值",
        Type::Unknown => "未知类型",
    }
}

fn per_name(per: Per) -> &'static str {
    match per {
        Per::Player => "player",
        Per::Game => "game",
        Per::Team => "team",
    }
}

fn root_name(root: PathRoot) -> &'static str {
    match root {
        PathRoot::SelfSeat => "self",
        PathRoot::Other => "other",
        PathRoot::Game => "game",
    }
}

fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a String>) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        let distance = edit_distance(name, candidate);
        if distance <= 2 && best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate.to_string())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            current[j + 1] = if ca == cb {
                previous[j]
            } else {
                1 + previous[j].min(current[j]).min(previous[j + 1])
            };
        }
        previous = current;
    }
    previous[b.len()]
}
