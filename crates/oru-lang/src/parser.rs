//! 递归下降解析器：把 `.oru` token 流转成 AST。
//!
//! M0 的语法是显式块结构，先用它把语义/诊断/工具链跑通；后续可以再加缩进式表面语法糖。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};
use crate::span::Span;

pub fn parse(tokens: Vec<Token>) -> Result<Program, Diagnostic> {
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        loop {
            self.skip_separators();
            if self.at_eof() {
                break;
            }
            let item = match self.peek_ident() {
                Some("game") => Some(self.parse_game()?),
                Some("seats") => Some(self.parse_seats()?),
                Some("edition") => Some(self.parse_edition()?),
                Some("resource") => Some(Item::Resource(self.parse_resource()?)),
                Some("action") => Some(Item::Action(self.parse_action()?)),
                Some("on") => Some(Item::Trigger(self.parse_trigger()?)),
                Some("win") => Some(Item::Win(self.parse_win()?)),
                Some(other) => {
                    return Err(Diagnostic::error(
                        "ORU0201",
                        format!("不认识的顶层声明 `{other}`"),
                        self.current_span(),
                    )
                    .with_help("M0 支持 game / seats / edition / resource / action / on / win"));
                }
                None => {
                    return Err(Diagnostic::error(
                        "ORU0202",
                        "这里需要声明关键字，拿到的是符号",
                        self.current_span(),
                    ));
                }
            };
            if let Some(item) = item {
                items.push(item);
            }
        }
        Ok(Program { items })
    }

    fn parse_game(&mut self) -> Result<Item, Diagnostic> {
        let start = self.bump_ident("game")?.start;
        let name = self.expect_string("game 后面需要游戏名，例如 game \"月影对决\"")?;
        let span = Span::new(start, name.1.end);
        self.finish_statement()?;
        Ok(Item::Game { name: name.0, span })
    }

    fn parse_seats(&mut self) -> Result<Item, Diagnostic> {
        let start = self.bump_ident("seats")?.start;
        let (count, span) = self.expect_int("seats 后面需要座位数（2–8）")?;
        let count = u32::try_from(count)
            .map_err(|_| Diagnostic::error("ORU0203", "seats 必须是正数", span))?;
        self.finish_statement()?;
        Ok(Item::Seats {
            count,
            span: Span::new(start, span.end),
        })
    }

    fn parse_edition(&mut self) -> Result<Item, Diagnostic> {
        let start = self.bump_ident("edition")?.start;
        let (edition, span) = self.expect_int("edition 后面需要语言版本，例如 edition 1")?;
        let edition = u32::try_from(edition)
            .map_err(|_| Diagnostic::error("ORU0204", "edition 必须是正数", span))?;
        self.finish_statement()?;
        Ok(Item::Edition {
            edition,
            span: Span::new(start, span.end),
        })
    }

    fn parse_resource(&mut self) -> Result<Resource, Diagnostic> {
        let start = self.bump_ident("resource")?.start;
        let (name, name_span) = self.expect_ident("resource 后面需要资源名")?;
        self.expect_lbrace("resource 需要用 `{ ... }` 声明字段")?;
        let mut resource = Resource {
            name,
            name_span,
            span: Span::empty(start),
            per: Per::Player,
            per_span: None,
            init: None,
            min: None,
            max: None,
            visibility: Visibility::Public,
        };
        loop {
            self.skip_separators();
            if self.eat_rbrace() {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    "ORU0205",
                    "resource 块没有闭合",
                    Span::new(start, self.current_span().end),
                ));
            }
            let (field, field_span) = self.expect_ident("resource 里需要字段名")?;
            match field.as_str() {
                "per" => {
                    let (value, value_span) =
                        self.expect_ident("per 后面需要 player / game / team")?;
                    resource.per = match value.as_str() {
                        "player" => Per::Player,
                        "game" => Per::Game,
                        "team" => Per::Team,
                        other => {
                            return Err(Diagnostic::error(
                                "ORU0206",
                                format!("未知作用域 `{other}`"),
                                value_span,
                            )
                            .with_help("可用：player / game / team"));
                        }
                    };
                    resource.per_span = Some(field_span.join(value_span));
                }
                "init" => resource.init = Some(self.expect_signed_int("init 后面需要整数")?.0),
                "min" => resource.min = Some(self.expect_signed_int("min 后面需要整数")?.0),
                "max" => resource.max = Some(self.expect_signed_int("max 后面需要整数")?.0),
                "visibility" => {
                    let (value, value_span) =
                        self.expect_ident("visibility 后面需要 public / private")?;
                    resource.visibility = match value.as_str() {
                        "public" => Visibility::Public,
                        "private" => Visibility::Private,
                        other => {
                            return Err(Diagnostic::error(
                                "ORU0207",
                                format!("未知可见性 `{other}`"),
                                value_span,
                            )
                            .with_help("可用：public / private"));
                        }
                    };
                }
                other => {
                    return Err(Diagnostic::error(
                        "ORU0208",
                        format!("resource 里不认识的字段 `{other}`"),
                        field_span,
                    )
                    .with_help("可用：per / init / min / max / visibility"));
                }
            }
            self.finish_statement()?;
        }
        resource.span = Span::new(start, self.previous_span().end);
        Ok(resource)
    }

    fn parse_action(&mut self) -> Result<Action, Diagnostic> {
        let start = self.bump_ident("action")?.start;
        let (id, id_span) = self.expect_ident("action 后面需要动作 id")?;
        let label = if self.peek_string().is_some() {
            self.expect_string("action 的展示名必须是字符串")?.0
        } else {
            id.clone()
        };
        self.expect_lbrace("action 需要 `{ ... }`")?;
        let mut when = None;
        let mut effects = Vec::new();
        loop {
            self.skip_separators();
            if self.eat_rbrace() {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    "ORU0209",
                    "action 块没有闭合",
                    Span::new(start, self.current_span().end),
                ));
            }
            let (field, field_span) = self.expect_ident("action 里需要字段名")?;
            match field.as_str() {
                "label" => {
                    let _ = self.expect_string("label 后面需要字符串")?;
                }
                "when" => {
                    let expr = self.parse_expr()?;
                    when = Some(expr);
                    self.finish_statement()?;
                }
                "do" => {
                    effects = self.parse_effect_block()?;
                }
                other => {
                    return Err(Diagnostic::error(
                        "ORU0210",
                        format!("action 里不认识的字段 `{other}`"),
                        field_span,
                    )
                    .with_help("M0 可用：label / when / do"));
                }
            }
        }
        let span = Span::new(start, self.previous_span().end);
        Ok(Action {
            id,
            id_span,
            label,
            span,
            when,
            effects,
        })
    }

    fn parse_trigger(&mut self) -> Result<Trigger, Diagnostic> {
        let start = self.bump_ident("on")?.start;
        let (on, on_span) = self.expect_ident("on 后面需要触发时机，例如 turnEnd")?;
        let effects = self.parse_effect_block()?;
        Ok(Trigger {
            on,
            on_span,
            span: Span::new(start, self.previous_span().end),
            effects,
        })
    }

    fn parse_win(&mut self) -> Result<WinRule, Diagnostic> {
        let start = self.bump_ident("win")?.start;
        self.expect_lbrace("win 需要 `{ when ... }`")?;
        let mut when = None;
        let mut say = None;
        loop {
            self.skip_separators();
            if self.eat_rbrace() {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    "ORU0211",
                    "win 块没有闭合",
                    Span::new(start, self.current_span().end),
                ));
            }
            let (field, field_span) = self.expect_ident("win 里需要字段名")?;
            match field.as_str() {
                "when" => {
                    when = Some(self.parse_expr()?);
                    self.finish_statement()?;
                }
                "say" => {
                    say = Some(self.expect_string("say 后面需要字符串")?.0);
                    self.finish_statement()?;
                }
                other => {
                    return Err(Diagnostic::error(
                        "ORU0212",
                        format!("win 里不认识的字段 `{other}`"),
                        field_span,
                    )
                    .with_help("M0 可用：when / say"));
                }
            }
        }
        let Some(when) = when else {
            return Err(Diagnostic::error(
                "ORU0213",
                "win 块必须有 when 条件",
                Span::new(start, self.previous_span().end),
            ));
        };
        Ok(WinRule {
            when,
            say,
            span: Span::new(start, self.previous_span().end),
        })
    }

    fn parse_effect_block(&mut self) -> Result<Vec<Effect>, Diagnostic> {
        self.expect_lbrace("do / on 后面需要 `{ ... }`")?;
        let mut effects = Vec::new();
        loop {
            self.skip_separators();
            if self.eat_rbrace() {
                break;
            }
            if self.at_eof() {
                return Err(Diagnostic::error(
                    "ORU0214",
                    "效果块没有闭合",
                    self.current_span(),
                ));
            }
            effects.push(self.parse_effect()?);
            self.finish_statement()?;
        }
        Ok(effects)
    }

    fn parse_effect(&mut self) -> Result<Effect, Diagnostic> {
        let start = self.current_span();
        let (head, head_span) = self.expect_ident("效果需要一个动作或赋值目标")?;
        match head.as_str() {
            "say" => {
                let (text, text_span) = self.expect_string("say 后面需要字符串")?;
                Ok(Effect {
                    kind: EffectKind::Say { text },
                    span: start.join(text_span),
                })
            }
            "end" => {
                let (outcome, outcome_span) =
                    self.expect_ident("end 后面需要 win / lose / draw")?;
                let outcome = match outcome.as_str() {
                    "win" => Outcome::Win,
                    "lose" => Outcome::Lose,
                    "draw" => Outcome::Draw,
                    other => {
                        return Err(Diagnostic::error(
                            "ORU0215",
                            format!("未知结局 `{other}`"),
                            outcome_span,
                        )
                        .with_help("可用：win / lose / draw"));
                    }
                };
                let text = if self.peek_string().is_some() {
                    Some(self.expect_string("end 的文案必须是字符串")?.0)
                } else {
                    None
                };
                Ok(Effect {
                    kind: EffectKind::End { outcome, text },
                    span: start.join(self.previous_span()),
                })
            }
            _ => {
                let path = self.parse_path_after_root(&head, head_span)?;
                let (op, op_span) = self.expect_op("赋值需要 `=` / `+=` / `-=`")?;
                let value = self.parse_expr()?;
                let span = start.join(value.span);
                let kind = match op.as_str() {
                    "=" => EffectKind::Set { path, value },
                    "+=" => EffectKind::Add { path, value },
                    "-=" => EffectKind::Sub { path, value },
                    _ => {
                        return Err(Diagnostic::error(
                            "ORU0216",
                            format!("不支持的赋值操作 `{op}`"),
                            op_span,
                        ));
                    }
                };
                Ok(Effect { kind, span })
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_and()?;
        while self.eat_keyword("or") {
            let right = self.parse_and()?;
            let span = expr.span.join(right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::Or,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_comparison()?;
        while self.eat_keyword("and") {
            let right = self.parse_comparison()?;
            let span = expr.span.join(right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::And,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_sum()?;
        while let Some(op) = self.peek_comparison_op() {
            self.bump();
            let right = self.parse_sum()?;
            let span = expr.span.join(right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_sum(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_product()?;
        loop {
            let op = match self.peek_op() {
                Some("+") => BinaryOp::Add,
                Some("-") => BinaryOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_product()?;
            let span = expr.span.join(right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_product(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = match self.peek_op() {
                Some("*") => BinaryOp::Mul,
                Some("/") => BinaryOp::Div,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            let span = expr.span.join(right.span);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.eat_op("!") {
            let expr = self.parse_unary()?;
            let span = Span::new(self.previous_span().start, expr.span.end);
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                },
                span,
            });
        }
        if self.eat_op("-") {
            let expr = self.parse_unary()?;
            let span = Span::new(self.previous_span().start, expr.span.end);
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                },
                span,
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        if self.eat_lparen() {
            let expr = self.parse_expr()?;
            self.expect_rparen("括号没有闭合")?;
            return Ok(expr);
        }
        if let Some((value, span)) = self.peek_int() {
            self.bump();
            return Ok(Expr {
                kind: ExprKind::Int(value),
                span,
            });
        }
        if self.peek_string().is_some() {
            return Err(Diagnostic::error(
                "ORU0227",
                "表达式里不能直接写字符串",
                self.current_span(),
            )
            .with_help("字符串只能用在 say / label / win.say 里"));
        }
        let (root, root_span) = self.expect_ident("表达式需要数字、变量或括号")?;
        if root == "true" || root == "false" {
            return Ok(Expr {
                kind: ExprKind::Bool(root == "true"),
                span: root_span,
            });
        }
        let path = self.parse_path_after_root(&root, root_span)?;
        let span = path.span;
        Ok(Expr {
            kind: ExprKind::Path(path),
            span,
        })
    }

    fn parse_path_after_root(&mut self, root: &str, root_span: Span) -> Result<Path, Diagnostic> {
        let root = match root {
            "self" => PathRoot::SelfSeat,
            "other" => PathRoot::Other,
            "game" => PathRoot::Game,
            other => {
                return Err(Diagnostic::error(
                    "ORU0217",
                    format!("未知变量作用域 `{other}`"),
                    root_span,
                )
                .with_help("变量路径形如 self.hp / other.energy / game.round"));
            }
        };
        self.expect_dot("变量路径需要 `.字段`，例如 self.hp")?;
        let (name, name_span) = self.expect_ident("`.` 后面需要字段名")?;
        Ok(Path {
            root,
            name,
            span: root_span.join(name_span),
        })
    }

    // ── token helpers ────────────────────────────────────────────

    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len().saturating_sub(1))]
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn previous_span(&self) -> Span {
        self.tokens[self.pos.saturating_sub(1).min(self.tokens.len() - 1)].span
    }

    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn peek_ident(&self) -> Option<&str> {
        match &self.current().kind {
            TokenKind::Ident(value) => Some(value),
            _ => None,
        }
    }

    fn bump_ident(&mut self, expected: &str) -> Result<Span, Diagnostic> {
        match &self.current().kind {
            TokenKind::Ident(value) if value == expected => Ok(self.bump().span),
            _ => Err(Diagnostic::error(
                "ORU0218",
                format!("这里需要 `{expected}`"),
                self.current_span(),
            )),
        }
    }

    fn expect_ident(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        match &self.current().kind {
            TokenKind::Ident(value) => {
                let value = value.clone();
                Ok((value, self.bump().span))
            }
            _ => Err(Diagnostic::error("ORU0219", message, self.current_span())),
        }
    }

    fn expect_string(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        match &self.current().kind {
            TokenKind::Str(value) => {
                let value = value.clone();
                Ok((value, self.bump().span))
            }
            _ => Err(Diagnostic::error("ORU0220", message, self.current_span())),
        }
    }

    fn peek_string(&self) -> Option<(&str, Span)> {
        match &self.current().kind {
            TokenKind::Str(value) => Some((value, self.current().span)),
            _ => None,
        }
    }

    fn peek_int(&self) -> Option<(i64, Span)> {
        match &self.current().kind {
            TokenKind::Int(value) => Some((*value, self.current().span)),
            _ => None,
        }
    }

    fn expect_int(&mut self, message: &str) -> Result<(i64, Span), Diagnostic> {
        match self.peek_int() {
            Some((value, _)) => {
                let span = self.bump().span;
                Ok((value, span))
            }
            None => Err(Diagnostic::error("ORU0221", message, self.current_span())),
        }
    }

    fn expect_signed_int(&mut self, message: &str) -> Result<(i64, Span), Diagnostic> {
        let negative = self.eat_op("-");
        let (value, span) = self.expect_int(message)?;
        Ok((if negative { -value } else { value }, span))
    }

    fn expect_lbrace(&mut self, message: &str) -> Result<Span, Diagnostic> {
        if matches!(self.current().kind, TokenKind::LBrace) {
            Ok(self.bump().span)
        } else {
            Err(Diagnostic::error("ORU0222", message, self.current_span()))
        }
    }

    fn eat_lparen(&mut self) -> bool {
        if matches!(self.current().kind, TokenKind::LParen) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_rparen(&mut self, message: &str) -> Result<Span, Diagnostic> {
        if matches!(self.current().kind, TokenKind::RParen) {
            Ok(self.bump().span)
        } else {
            Err(Diagnostic::error("ORU0223", message, self.current_span()))
        }
    }

    fn expect_dot(&mut self, message: &str) -> Result<Span, Diagnostic> {
        if matches!(self.current().kind, TokenKind::Dot) {
            Ok(self.bump().span)
        } else {
            Err(Diagnostic::error("ORU0224", message, self.current_span()))
        }
    }

    fn eat_rbrace(&mut self) -> bool {
        if matches!(self.current().kind, TokenKind::RBrace) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, keyword: &str) -> bool {
        if matches!(&self.current().kind, TokenKind::Ident(value) if value == keyword) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn peek_op(&self) -> Option<&str> {
        match &self.current().kind {
            TokenKind::Op(value) => Some(value),
            _ => None,
        }
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.peek_op() == Some(op) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        match &self.current().kind {
            TokenKind::Op(value) => {
                let value = value.clone();
                Ok((value, self.bump().span))
            }
            _ => Err(Diagnostic::error("ORU0225", message, self.current_span())),
        }
    }

    fn peek_comparison_op(&self) -> Option<BinaryOp> {
        let op = self.peek_op()?;
        Some(match op {
            "==" => BinaryOp::Eq,
            "!=" => BinaryOp::Ne,
            "<" => BinaryOp::Lt,
            "<=" => BinaryOp::Le,
            ">" => BinaryOp::Gt,
            ">=" => BinaryOp::Ge,
            _ => return None,
        })
    }

    fn skip_separators(&mut self) {
        while matches!(
            self.current().kind,
            TokenKind::Newline | TokenKind::Semicolon
        ) {
            self.bump();
        }
    }

    fn finish_statement(&mut self) -> Result<(), Diagnostic> {
        match self.current().kind {
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof => {
                self.skip_separators();
                Ok(())
            }
            _ => Err(Diagnostic::error(
                "ORU0226",
                "这里需要换行或 `;` 分隔",
                self.current_span(),
            )),
        }
    }
}
