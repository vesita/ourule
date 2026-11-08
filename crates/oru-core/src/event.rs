//! 领域事件：规则只产生事件，文本/渲染由上层派生。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    Public,
    Private(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceScope {
    Player(usize),
    Game,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventKind {
    MatchStarted {
        seats: usize,
    },
    TurnStarted {
        seat: usize,
        turn: u32,
    },
    ActionResolved {
        seat: usize,
        action: String,
        label: String,
    },
    ResourceChanged {
        scope: ResourceScope,
        name: String,
        from: i64,
        to: i64,
    },
    PiecePlaced {
        seat: usize,
        target: String,
    },
    Said {
        text: String,
    },
    TurnEnded {
        seat: usize,
    },
    MatchEnded {
        outcome: String,
        winner: Option<usize>,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub seq: u64,
    pub turn: u32,
    pub active: usize,
    pub visibility: Visibility,
    pub kind: EventKind,
}

impl Event {
    /// 给人看的单行摘要（CLI/日志用；正式渲染由 present 层决定）。
    pub fn text(&self) -> String {
        match &self.kind {
            EventKind::MatchStarted { seats } => format!("对局开始（{seats} 座）"),
            EventKind::TurnStarted { seat, turn } => {
                format!("第 {turn} 回合 · 轮到 玩家{}", seat + 1)
            }
            EventKind::ActionResolved {
                seat,
                action,
                label,
            } => {
                format!("玩家{} 使用 {label}（{action}）", seat + 1)
            }
            EventKind::ResourceChanged {
                scope,
                name,
                from,
                to,
            } => match scope {
                ResourceScope::Player(seat) => {
                    format!("玩家{} {name}：{from} → {to}", seat + 1)
                }
                ResourceScope::Game => format!("{name}：{from} → {to}"),
            },
            EventKind::PiecePlaced { seat, target } => {
                format!("玩家{} 在 {target} 落子", seat + 1)
            }
            EventKind::Said { text } => text.clone(),
            EventKind::TurnEnded { seat } => format!("玩家{} 回合结束", seat + 1),
            EventKind::MatchEnded { text, .. } => text.clone(),
        }
    }
}
