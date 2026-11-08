//! M1 内核：Match 状态、回合游标、纯函数 step、事件流、只读投影。

use std::collections::BTreeMap;

use oru_lang::ast::{
    BinaryOp, Effect, EffectKind, Expr, ExprKind, Outcome as AstOutcome, Path, PathRoot, Per,
    UnaryOp, Visibility as AstVisibility,
};
use serde::Serialize;

use crate::definition::{BoardDefinition, Definition, DefinitionError, FamilyDefinition};
use crate::event::{Event, EventKind, ResourceScope, Visibility};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutcomeKind {
    Win,
    Lose,
    Draw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeRecord {
    pub kind: OutcomeKind,
    pub winner: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOffer {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ActionTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionTarget {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perspective {
    Admin,
    Spectator,
    Seat(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuleError {
    GameOver,
    UnknownAction(String),
    NotAllowed { action: String, reason: String },
    Eval(String),
    BadSeats(String),
    Definition(DefinitionError),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameOver => f.write_str("对局已经结束"),
            Self::UnknownAction(action) => write!(f, "没有动作「{action}」"),
            Self::NotAllowed { action, reason } => {
                write!(f, "动作「{action}」现在不能做：{reason}")
            }
            Self::Eval(message) => write!(f, "规则表达式错误：{message}"),
            Self::BadSeats(message) => write!(f, "座位数非法：{message}"),
            Self::Definition(error) => write!(f, "规则定义错误：{error}"),
        }
    }
}

impl std::error::Error for RuleError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Values {
    Player(Vec<i64>),
    Game(i64),
}

#[derive(Debug, Clone)]
pub struct Match {
    definition: Definition,
    family: FamilyState,
    seats: usize,
    active: usize,
    turn: u32,
    taken: u32,
    resources: BTreeMap<String, Values>,
    outcome: Option<OutcomeRecord>,
    rng: u64,
    events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FamilyState {
    Duel,
    Board(BoardState),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardView {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Option<usize>>,
    pub line_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardState {
    pub cells: Vec<Option<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceView {
    pub per: String,
    pub visibility: String,
    pub values: Vec<Option<i64>>,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadModel {
    pub title: String,
    pub seats: usize,
    pub active: usize,
    pub turn: u32,
    pub taken: u32,
    pub finished: bool,
    pub outcome: Option<OutcomeRecord>,
    pub offers: Vec<ActionOffer>,
    pub resources: BTreeMap<String, ResourceView>,
    pub board: Option<BoardView>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub events: Vec<Event>,
    pub model: ReadModel,
}

impl Match {
    pub fn start(
        definition: Definition,
        seats: Option<usize>,
        seed: u64,
    ) -> Result<Self, RuleError> {
        let seats = seats.unwrap_or(definition.seats);
        if !(2..=8).contains(&seats) {
            return Err(RuleError::BadSeats(format!("必须是 2–8，拿到 {seats}")));
        }
        let family = match definition.family() {
            FamilyDefinition::Duel(_) => FamilyState::Duel,
            FamilyDefinition::Board(board) => FamilyState::Board(BoardState {
                cells: vec![None; board.rows.saturating_mul(board.cols)],
            }),
        };
        let mut resources = BTreeMap::new();
        for (name, resource) in &definition.resources {
            let values = match resource.per {
                Per::Player => {
                    Values::Player((0..seats).map(|_| clamp(resource, resource.init)).collect())
                }
                Per::Game => Values::Game(clamp(resource, resource.init)),
                Per::Team => {
                    return Err(RuleError::Definition(DefinitionError(
                        "M1 不支持 per team 资源".to_string(),
                    )));
                }
            };
            resources.insert(name.clone(), values);
        }
        let mut game = Self {
            definition,
            family,
            seats,
            active: 0,
            turn: 1,
            taken: 0,
            resources,
            outcome: None,
            rng: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
            events: Vec::new(),
        };
        game.push_event(Visibility::Public, EventKind::MatchStarted { seats });
        let start_effects = game.definition.trigger("start").to_vec();
        game.apply_effects(&start_effects, None, None)?;
        if game.outcome.is_none() {
            game.push_event(
                Visibility::Public,
                EventKind::TurnStarted {
                    seat: game.active,
                    turn: game.turn,
                },
            );
            let turn_start = game.definition.trigger("turnStart").to_vec();
            game.apply_effects(&turn_start, None, None)?;
        }
        Ok(game)
    }

    pub fn definition(&self) -> &Definition {
        &self.definition
    }

    pub fn seats(&self) -> usize {
        self.seats
    }

    pub fn family_state(&self) -> &FamilyState {
        &self.family
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn taken(&self) -> u32 {
        self.taken
    }

    pub fn outcome(&self) -> Option<&OutcomeRecord> {
        self.outcome.as_ref()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn offers(&self) -> Result<Vec<ActionOffer>, RuleError> {
        if self.outcome.is_some() {
            return Ok(Vec::new());
        }
        let mut offers = Vec::new();
        if let FamilyState::Board(board_state) = &self.family {
            let FamilyDefinition::Board(board) = self.definition.family() else {
                return Err(RuleError::Eval(
                    "Match family 与 Definition 不匹配".to_string(),
                ));
            };
            return Ok(board_offers(board, board_state, self.active));
        }
        for action in &self.definition.actions {
            let allowed = match &action.when {
                None => true,
                Some(when) => self.eval(when)?.as_bool()?,
            };
            if allowed {
                offers.push(ActionOffer {
                    id: action.id.clone(),
                    label: action.label.clone(),
                    target: None,
                });
            }
        }
        Ok(offers)
    }

    /// 推进一帧：一帧只处理一个动作；失败时原 Match 完全不变。
    pub fn step(&mut self, action_id: &str, target: Option<&str>) -> Result<Step, RuleError> {
        let mut next = self.clone();
        let start = next.events.len();
        next.step_inner(action_id, target)?;
        let model = next.read(Perspective::Admin)?;
        let events = next.events[start..].to_vec();
        *self = next;
        Ok(Step { events, model })
    }

    fn step_inner(&mut self, action_id: &str, target: Option<&str>) -> Result<(), RuleError> {
        if self.outcome.is_some() {
            return Err(RuleError::GameOver);
        }
        if matches!(self.family, FamilyState::Board(_)) {
            return self.step_board(action_id, target);
        }
        let action = self
            .definition
            .action(action_id)
            .cloned()
            .ok_or_else(|| RuleError::UnknownAction(action_id.to_string()))?;
        if let Some(when) = &action.when
            && !self.eval(when)?.as_bool()?
        {
            return Err(RuleError::NotAllowed {
                action: action_id.to_string(),
                reason: "when 条件不满足".to_string(),
            });
        }
        if target.is_some() {
            return Err(RuleError::NotAllowed {
                action: action_id.to_string(),
                reason: "duel 动作不接受 target".to_string(),
            });
        }

        let actor = self.active;
        self.push_event(
            Visibility::Public,
            EventKind::ActionResolved {
                seat: actor,
                action: action.id.clone(),
                label: action.label.clone(),
            },
        );
        self.apply_effects(&action.effects, Some(actor), Some(&action.label))?;

        if self.outcome.is_none() {
            self.check_wins(actor)?;
        }
        if self.outcome.is_none() {
            let turn_end = self.definition.trigger("turnEnd").to_vec();
            self.apply_effects(&turn_end, Some(actor), Some(&action.label))?;
            self.push_event(Visibility::Public, EventKind::TurnEnded { seat: actor });
            self.taken = self.taken.saturating_add(1);
            if self.outcome.is_some() {
                return Ok(());
            }
            self.advance_turn();
            let limit = self
                .definition
                .max_turns
                .saturating_mul(u32::try_from(self.seats).unwrap_or(u32::MAX));
            if self.taken >= limit {
                self.finish(OutcomeKind::Draw, None, "回合数用尽，平局".to_string());
            } else {
                self.push_event(
                    Visibility::Public,
                    EventKind::TurnStarted {
                        seat: self.active,
                        turn: self.turn,
                    },
                );
                let turn_start = self.definition.trigger("turnStart").to_vec();
                self.apply_effects(&turn_start, None, None)?;
            }
        }
        Ok(())
    }

    pub fn read(&self, perspective: Perspective) -> Result<ReadModel, RuleError> {
        let resources = self
            .definition
            .resources
            .iter()
            .map(|(name, resource)| {
                let values = match self.resources.get(name) {
                    Some(Values::Player(values)) => values
                        .iter()
                        .enumerate()
                        .map(|(seat, value)| {
                            if visible(resource.visibility, perspective, seat) {
                                Some(*value)
                            } else {
                                None
                            }
                        })
                        .collect(),
                    Some(Values::Game(value)) => {
                        vec![if visible(resource.visibility, perspective, usize::MAX) {
                            Some(*value)
                        } else {
                            None
                        }]
                    }
                    None => Vec::new(),
                };
                (
                    name.clone(),
                    ResourceView {
                        per: match resource.per {
                            Per::Player => "player",
                            Per::Game => "game",
                            Per::Team => "team",
                        }
                        .to_string(),
                        visibility: match resource.visibility {
                            AstVisibility::Public => "public",
                            AstVisibility::Private => "private",
                        }
                        .to_string(),
                        values,
                        min: resource.min,
                        max: resource.max,
                    },
                )
            })
            .collect();
        let board = match (&self.family, self.definition.family()) {
            (FamilyState::Board(state), FamilyDefinition::Board(def)) => Some(BoardView {
                rows: def.rows,
                cols: def.cols,
                cells: state.cells.clone(),
                line_length: def.line_length,
            }),
            _ => None,
        };
        Ok(ReadModel {
            title: self.definition.title.clone(),
            seats: self.seats,
            active: self.active,
            turn: self.turn,
            taken: self.taken,
            finished: self.outcome.is_some(),
            outcome: self.outcome.clone(),
            offers: self.offers()?,
            resources,
            board,
            events: self
                .events
                .iter()
                .filter(|event| event_visible(event.visibility, perspective))
                .cloned()
                .collect(),
        })
    }

    fn apply_effects(
        &mut self,
        effects: &[Effect],
        actor: Option<usize>,
        action: Option<&str>,
    ) -> Result<(), RuleError> {
        for effect in effects {
            match &effect.kind {
                EffectKind::Set { path, value } => {
                    let value = self.eval(value)?.as_int()?;
                    self.write_path(path, value)?;
                }
                EffectKind::Add { path, value } => {
                    let delta = self.eval(value)?.as_int()?;
                    let previous = self.read_path(path)?;
                    let next = self.clamp_path(path, previous.saturating_add(delta))?;
                    self.write_path(path, next)?;
                }
                EffectKind::Sub { path, value } => {
                    let delta = self.eval(value)?.as_int()?;
                    let previous = self.read_path(path)?;
                    let next = self.clamp_path(path, previous.saturating_sub(delta))?;
                    self.write_path(path, next)?;
                }
                EffectKind::Say { text } => {
                    let text = self.interpolate(text, actor, action);
                    self.push_event(Visibility::Public, EventKind::Said { text });
                }
                EffectKind::End { outcome, text } => {
                    let outcome = match outcome {
                        AstOutcome::Win => OutcomeKind::Win,
                        AstOutcome::Lose => OutcomeKind::Lose,
                        AstOutcome::Draw => OutcomeKind::Draw,
                    };
                    let winner = match outcome {
                        OutcomeKind::Win => Some(self.active),
                        OutcomeKind::Lose => Some((self.active + 1) % self.seats),
                        OutcomeKind::Draw => None,
                    };
                    let text = text
                        .as_deref()
                        .map(|template| self.interpolate(template, actor, action))
                        .unwrap_or_else(|| default_outcome_text(&outcome, winner));
                    self.finish(outcome, winner, text);
                    break;
                }
            }
            if self.outcome.is_some() && !matches!(effect.kind, EffectKind::End { .. }) {
                // 结束效果之后的同批效果不再执行，避免在已结束对局上继续改状态。
                break;
            }
        }
        Ok(())
    }

    fn step_board(&mut self, action_id: &str, target: Option<&str>) -> Result<(), RuleError> {
        if action_id != "place" {
            return Err(RuleError::UnknownAction(action_id.to_string()));
        }
        let target = target.ok_or_else(|| RuleError::NotAllowed {
            action: action_id.to_string(),
            reason: "place 需要 target 格位，例如 a1".to_string(),
        })?;
        let FamilyDefinition::Board(def) = self.definition.family() else {
            return Err(RuleError::Eval(
                "board state 与 Definition 不匹配".to_string(),
            ));
        };
        let index = parse_cell(target, def).ok_or_else(|| RuleError::NotAllowed {
            action: action_id.to_string(),
            reason: format!("target `{target}` 不是合法格位"),
        })?;
        let actor = self.active;
        let won = {
            let FamilyState::Board(board) = &mut self.family else {
                return Err(RuleError::Eval("board state 与 Match 不匹配".to_string()));
            };
            if board.cells.get(index).is_none_or(Option::is_some) {
                return Err(RuleError::NotAllowed {
                    action: action_id.to_string(),
                    reason: format!("格位 {target} 已被占用"),
                });
            }
            board.cells[index] = Some(actor);
            board_winner(board, def, actor)
        };
        let label = format!("落子 {target}");
        self.push_event(
            Visibility::Public,
            EventKind::ActionResolved {
                seat: actor,
                action: action_id.to_string(),
                label,
            },
        );
        self.push_event(
            Visibility::Public,
            EventKind::PiecePlaced {
                seat: actor,
                target: target.to_string(),
            },
        );
        if won {
            self.finish(
                OutcomeKind::Win,
                Some(actor),
                format!("玩家{} 连成一线获胜", actor + 1),
            );
        }
        if self.outcome.is_none() {
            self.push_event(Visibility::Public, EventKind::TurnEnded { seat: actor });
            self.taken = self.taken.saturating_add(1);
            self.advance_turn();
            let limit = self
                .definition
                .max_turns
                .saturating_mul(u32::try_from(self.seats).unwrap_or(u32::MAX));
            if self.taken >= limit {
                self.finish(OutcomeKind::Draw, None, "回合数用尽，平局".to_string());
            } else if board_full(&self.family) {
                self.finish(OutcomeKind::Draw, None, "棋盘已满，平局".to_string());
            } else {
                self.push_event(
                    Visibility::Public,
                    EventKind::TurnStarted {
                        seat: self.active,
                        turn: self.turn,
                    },
                );
            }
        }
        Ok(())
    }

    fn check_wins(&mut self, actor: usize) -> Result<(), RuleError> {
        for win in &self.definition.wins {
            if !self.eval(&win.when)?.as_bool()? {
                continue;
            }
            let text = win
                .say
                .as_deref()
                .map(|template| self.interpolate(template, Some(actor), None))
                .unwrap_or_else(|| format!("玩家{} 获胜", actor + 1));
            self.finish(OutcomeKind::Win, Some(actor), text);
            return Ok(());
        }
        Ok(())
    }

    fn finish(&mut self, kind: OutcomeKind, winner: Option<usize>, text: String) {
        self.outcome = Some(OutcomeRecord {
            kind: kind.clone(),
            winner,
            text: text.clone(),
        });
        let outcome = match kind {
            OutcomeKind::Win => "win",
            OutcomeKind::Lose => "lose",
            OutcomeKind::Draw => "draw",
        };
        self.push_event(
            Visibility::Public,
            EventKind::MatchEnded {
                outcome: outcome.to_string(),
                winner,
                text,
            },
        );
    }

    fn advance_turn(&mut self) {
        self.active = (self.active + 1) % self.seats.max(1);
        if self.active == 0 {
            self.turn = self.turn.saturating_add(1);
        }
    }

    fn push_event(&mut self, visibility: Visibility, kind: EventKind) {
        let seq = self.events.len() as u64 + 1;
        self.events.push(Event {
            seq,
            turn: self.turn,
            active: self.active,
            visibility,
            kind,
        });
    }

    fn interpolate(&self, template: &str, actor: Option<usize>, action: Option<&str>) -> String {
        let mut text = template.to_string();
        text = text.replace("{title}", &self.definition.title);
        text = text.replace("{turn}", &self.turn.to_string());
        text = text.replace("{active}", &(self.active + 1).to_string());
        if let Some(actor) = actor {
            text = text.replace("{actor}", &format!("玩家{}", actor + 1));
        }
        if let Some(action) = action {
            text = text.replace("{action}", action);
        }
        text
    }

    fn read_path(&self, path: &Path) -> Result<i64, RuleError> {
        match path.root {
            PathRoot::SelfSeat => self.player_value(self.active, &path.name),
            PathRoot::Other => self.player_value((self.active + 1) % self.seats, &path.name),
            PathRoot::Game => match self.resources.get(&path.name) {
                Some(Values::Game(value)) => Ok(*value),
                _ => Err(RuleError::Eval(format!("game 资源 `{}` 不存在", path.name))),
            },
        }
    }

    fn player_value(&self, seat: usize, name: &str) -> Result<i64, RuleError> {
        match self.resources.get(name) {
            Some(Values::Player(values)) => values
                .get(seat)
                .copied()
                .ok_or_else(|| RuleError::Eval(format!("玩家资源 `{name}` 没有座位 {seat}"))),
            _ => Err(RuleError::Eval(format!("玩家资源 `{name}` 不存在"))),
        }
    }

    fn write_path(&mut self, path: &Path, value: i64) -> Result<(), RuleError> {
        let (scope, name) = self.resolve_path(path)?;
        let previous = self.read_path(path)?;
        let resource = self
            .definition
            .resource(&name)
            .ok_or_else(|| RuleError::Eval(format!("资源 `{name}` 不存在")))?;
        let visibility = match (resource.visibility, path.root) {
            (AstVisibility::Public, _) => Visibility::Public,
            (AstVisibility::Private, PathRoot::SelfSeat) => Visibility::Private(self.active),
            (AstVisibility::Private, PathRoot::Other) => {
                Visibility::Private((self.active + 1) % self.seats)
            }
            (AstVisibility::Private, PathRoot::Game) => Visibility::Private(self.active),
        };
        let next = clamp(resource, value);
        match (scope, self.resources.get_mut(&name)) {
            (PathRoot::Game, Some(Values::Game(value))) => *value = next,
            (PathRoot::SelfSeat | PathRoot::Other, Some(Values::Player(values))) => {
                let seat = match scope {
                    PathRoot::SelfSeat => self.active,
                    PathRoot::Other => (self.active + 1) % self.seats,
                    PathRoot::Game => unreachable!(),
                };
                let Some(slot) = values.get_mut(seat) else {
                    return Err(RuleError::Eval("座位越界".to_string()));
                };
                *slot = next;
            }
            _ => return Err(RuleError::Eval(format!("资源 `{name}` 作用域不匹配"))),
        }
        if previous != next {
            let scope = match path.root {
                PathRoot::Game => ResourceScope::Game,
                PathRoot::SelfSeat => ResourceScope::Player(self.active),
                PathRoot::Other => ResourceScope::Player((self.active + 1) % self.seats),
            };
            self.push_event(
                visibility,
                EventKind::ResourceChanged {
                    scope,
                    name,
                    from: previous,
                    to: next,
                },
            );
        }
        Ok(())
    }

    fn clamp_path(&self, path: &Path, value: i64) -> Result<i64, RuleError> {
        let resource = self
            .definition
            .resource(&path.name)
            .ok_or_else(|| RuleError::Eval(format!("资源 `{}` 不存在", path.name)))?;
        Ok(clamp(resource, value))
    }

    fn resolve_path(&self, path: &Path) -> Result<(PathRoot, String), RuleError> {
        Ok((path.root, path.name.clone()))
    }

    fn eval(&self, expr: &Expr) -> Result<Value, RuleError> {
        match &expr.kind {
            ExprKind::Int(value) => Ok(Value::Int(*value)),
            ExprKind::Bool(value) => Ok(Value::Bool(*value)),
            ExprKind::Path(path) => Ok(Value::Int(self.read_path(path)?)),
            ExprKind::Unary { op, expr } => {
                let value = self.eval(expr)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!value.as_bool()?)),
                    UnaryOp::Neg => Ok(Value::Int(-value.as_int()?)),
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.eval(left)?;
                let right = self.eval(right)?;
                match op {
                    BinaryOp::Add => Ok(Value::Int(left.as_int()?.saturating_add(right.as_int()?))),
                    BinaryOp::Sub => Ok(Value::Int(left.as_int()?.saturating_sub(right.as_int()?))),
                    BinaryOp::Mul => Ok(Value::Int(left.as_int()?.saturating_mul(right.as_int()?))),
                    BinaryOp::Div => {
                        let divisor = right.as_int()?;
                        if divisor == 0 {
                            return Err(RuleError::Eval("除以零".to_string()));
                        }
                        Ok(Value::Int(left.as_int()? / divisor))
                    }
                    BinaryOp::Eq => Ok(Value::Bool(left == right)),
                    BinaryOp::Ne => Ok(Value::Bool(left != right)),
                    BinaryOp::Lt => Ok(Value::Bool(left.as_int()? < right.as_int()?)),
                    BinaryOp::Le => Ok(Value::Bool(left.as_int()? <= right.as_int()?)),
                    BinaryOp::Gt => Ok(Value::Bool(left.as_int()? > right.as_int()?)),
                    BinaryOp::Ge => Ok(Value::Bool(left.as_int()? >= right.as_int()?)),
                    BinaryOp::And => Ok(Value::Bool(left.as_bool()? && right.as_bool()?)),
                    BinaryOp::Or => Ok(Value::Bool(left.as_bool()? || right.as_bool()?)),
                }
            }
        }
    }

    /// 保留给后续 roll/随机效果；目前 M1 没有随机效果。
    #[allow(dead_code)]
    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }
}

fn board_offers(def: &BoardDefinition, state: &BoardState, _active: usize) -> Vec<ActionOffer> {
    let mut offers = Vec::new();
    for (index, cell) in state.cells.iter().enumerate() {
        if cell.is_none() {
            let target = cell_name(index, def.cols);
            offers.push(ActionOffer {
                id: "place".to_string(),
                label: format!("落子 {target}"),
                target: Some(ActionTarget {
                    id: target.clone(),
                    label: target,
                }),
            });
        }
    }
    offers
}

fn cell_name(index: usize, cols: usize) -> String {
    let row = index / cols;
    let col = index % cols;
    let letter = char::from(b'a' + u8::try_from(col).unwrap_or(0));
    format!("{letter}{}", row + 1)
}

fn parse_cell(target: &str, def: &BoardDefinition) -> Option<usize> {
    let mut chars = target.chars();
    let col_char = chars.next()?;
    if !col_char.is_ascii_lowercase() {
        return None;
    }
    let row = chars.as_str().parse::<usize>().ok()?.checked_sub(1)?;
    let col = usize::try_from(u32::from(col_char) - u32::from('a')).ok()?;
    if row >= def.rows || col >= def.cols {
        return None;
    }
    Some(row * def.cols + col)
}

fn board_full(state: &FamilyState) -> bool {
    match state {
        FamilyState::Board(board) => board.cells.iter().all(Option::is_some),
        FamilyState::Duel => false,
    }
}

fn board_winner(state: &BoardState, def: &BoardDefinition, seat: usize) -> bool {
    let n = def.line_length;
    if n == 0 || n > def.rows.max(def.cols) {
        return false;
    }
    let directions = [(0isize, 1isize), (1, 0), (1, 1), (1, -1)];
    for row in 0..def.rows {
        for col in 0..def.cols {
            for (dr, dc) in directions {
                let end_row = row as isize + dr * (n as isize - 1);
                let end_col = col as isize + dc * (n as isize - 1);
                if end_row < 0
                    || end_col < 0
                    || end_row >= def.rows as isize
                    || end_col >= def.cols as isize
                {
                    continue;
                }
                if (0..n).all(|offset| {
                    let r = (row as isize + dr * offset as isize) as usize;
                    let c = (col as isize + dc * offset as isize) as usize;
                    state.cells[r * def.cols + c] == Some(seat)
                }) {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Value {
    Int(i64),
    Bool(bool),
}

impl Value {
    fn as_int(self) -> Result<i64, RuleError> {
        match self {
            Self::Int(value) => Ok(value),
            Self::Bool(_) => Err(RuleError::Eval("这里需要数字，拿到布尔".to_string())),
        }
    }

    fn as_bool(self) -> Result<bool, RuleError> {
        match self {
            Self::Bool(value) => Ok(value),
            Self::Int(_) => Err(RuleError::Eval("这里需要布尔，拿到数字".to_string())),
        }
    }
}

fn clamp(resource: &crate::definition::ResourceDef, value: i64) -> i64 {
    let mut value = value;
    if let Some(min) = resource.min {
        value = value.max(min);
    }
    if let Some(max) = resource.max {
        value = value.min(max);
    }
    value
}

fn event_visible(visibility: Visibility, perspective: Perspective) -> bool {
    match (visibility, perspective) {
        (Visibility::Public, _) | (Visibility::Private(_), Perspective::Admin) => true,
        (Visibility::Private(_), Perspective::Spectator) => false,
        (Visibility::Private(owner), Perspective::Seat(viewer)) => owner == viewer,
    }
}

fn visible(visibility: AstVisibility, perspective: Perspective, seat: usize) -> bool {
    match visibility {
        AstVisibility::Public => true,
        AstVisibility::Private => match perspective {
            Perspective::Admin => true,
            Perspective::Spectator => false,
            Perspective::Seat(viewer) => viewer == seat,
        },
    }
}

fn default_outcome_text(outcome: &OutcomeKind, winner: Option<usize>) -> String {
    match (outcome, winner) {
        (OutcomeKind::Win, Some(seat)) | (OutcomeKind::Lose, Some(seat)) => {
            format!("玩家{} 获胜", seat + 1)
        }
        (OutcomeKind::Draw, _) => "平局".to_string(),
        _ => "对局结束".to_string(),
    }
}
