//! Oru 的规则定义、对局内核与事件流（M1）。
//!
//! 这里只做纯解释：不读文件、不联网、不读时钟；随机来自 Match 内部 RNG。

pub mod definition;
pub mod engine;
pub mod event;

pub use definition::{ActionDef, Definition, DefinitionError, ResourceDef, WinDef};
pub use engine::{
    ActionOffer, Match, OutcomeKind, OutcomeRecord, Perspective, ReadModel, ResourceView,
    RuleError, Step,
};
pub use event::{Event, EventKind, ResourceScope, Visibility};
