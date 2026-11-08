//! Oru 的规则定义、对局内核与事件流（M1/M2）。
//!
//! 这里只做纯解释：不读文件、不联网、不读时钟；随机来自 Match 内部 RNG。

pub mod definition;
pub mod engine;
pub mod event;

pub use definition::{
    ActionDef, BoardDefinition, Definition, DefinitionError, DuelDefinition, FamilyDefinition,
    ResourceDef, WinDef,
};
pub use engine::{
    ActionOffer, ActionTarget, BoardState, BoardView, DuelState, Match, OutcomeKind, OutcomeRecord,
    Perspective, ReadModel, ResourceValues, ResourceView, RuleError, Step,
};
pub use event::{Event, EventKind, ResourceScope, Visibility};
