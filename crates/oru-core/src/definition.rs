//! 从通过检查的 AST 构造不可变的游戏定义。
//!
//! Definition 是不可变配置；FamilyDefinition 选择解释器，内容不依赖文件/IO。

use std::collections::BTreeMap;

use oru_lang::ast::{
    Action, BoardSpec, Effect, Expr, Item, Per, Program, Resource, Visibility as AstVisibility,
    WinRule,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub title: String,
    pub seats: usize,
    pub max_turns: u32,
    pub resources: BTreeMap<String, ResourceDef>,
    pub actions: Vec<ActionDef>,
    pub triggers: BTreeMap<String, Vec<Effect>>,
    pub wins: Vec<WinDef>,
    pub family: FamilyDefinition,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FamilyDefinition {
    Duel(DuelDefinition),
    Board(BoardDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuelDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardDefinition {
    pub rows: usize,
    pub cols: usize,
    pub line_length: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDef {
    pub name: String,
    pub per: Per,
    pub init: i64,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub visibility: AstVisibility,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionDef {
    pub id: String,
    pub label: String,
    pub when: Option<Expr>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WinDef {
    pub when: Expr,
    pub say: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionError(pub String);

impl std::fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DefinitionError {}

impl Definition {
    pub fn from_program(program: &Program) -> Result<Self, DefinitionError> {
        let mut title = None;
        let mut seats = None;
        let mut max_turns = None;
        let mut family = None;
        let mut board = None;
        let mut resources = BTreeMap::new();
        let mut actions = Vec::new();
        let mut triggers: BTreeMap<String, Vec<Effect>> = BTreeMap::new();
        let mut wins = Vec::new();
        let mut board_seen = false;

        for item in &program.items {
            match item {
                Item::Game { name, .. } => title = Some(name.clone()),
                Item::Seats { count, .. } => seats = Some(*count),
                Item::MaxTurns { turns, .. } => max_turns = Some(*turns),
                Item::Family { family: name, .. } => family = Some(name.clone()),
                Item::Board(params) => {
                    board_seen = true;
                    board = Some(board_definition(params)?);
                }
                Item::Edition { .. } => {}
                Item::Resource(resource) => {
                    resources.insert(resource.name.clone(), resource_def(resource)?);
                }
                Item::Action(action) => actions.push(action_def(action)?),
                Item::Trigger(trigger) => {
                    triggers
                        .entry(trigger.on.clone())
                        .or_default()
                        .extend(trigger.effects.clone());
                }
                Item::Win(win) => wins.push(win_def(win)),
            }
        }

        let title = title.ok_or_else(|| DefinitionError("缺少 game".to_string()))?;
        let seats = seats.ok_or_else(|| DefinitionError("缺少 seats".to_string()))?;
        if !(2..=8).contains(&seats) {
            return Err(DefinitionError(format!("seats 必须在 2–8，拿到 {seats}")));
        }
        let family = match family.as_deref().unwrap_or("duel") {
            "duel" => {
                if board_seen {
                    return Err(DefinitionError("duel 不能声明 board 块".to_string()));
                }
                if actions.is_empty() || wins.is_empty() {
                    return Err(DefinitionError(
                        "duel 需要至少一个 action 和 win".to_string(),
                    ));
                }
                FamilyDefinition::Duel(DuelDefinition)
            }
            "board" => FamilyDefinition::Board(
                board
                    .ok_or_else(|| DefinitionError("family board 需要 board 参数块".to_string()))?,
            ),
            other => return Err(DefinitionError(format!("未知 family `{other}`"))),
        };

        Ok(Self {
            title,
            seats: usize::try_from(seats).unwrap_or(0),
            max_turns: max_turns.unwrap_or(40),
            resources,
            actions,
            triggers,
            wins,
            family,
        })
    }

    pub fn family(&self) -> &FamilyDefinition {
        &self.family
    }

    pub fn resource(&self, name: &str) -> Option<&ResourceDef> {
        self.resources.get(name)
    }

    pub fn action(&self, id: &str) -> Option<&ActionDef> {
        self.actions.iter().find(|action| action.id == id)
    }

    pub fn trigger(&self, on: &str) -> &[Effect] {
        self.triggers.get(on).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn board_definition(board: &BoardSpec) -> Result<BoardDefinition, DefinitionError> {
    if !(2..=26).contains(&board.rows) || !(2..=26).contains(&board.cols) {
        return Err(DefinitionError("board rows/cols 必须在 2–26".to_string()));
    }
    if board.line_length < 2 || board.line_length > board.rows.max(board.cols) {
        return Err(DefinitionError("board line 不在合法范围".to_string()));
    }
    Ok(BoardDefinition {
        rows: board.rows,
        cols: board.cols,
        line_length: board.line_length,
    })
}

fn resource_def(resource: &Resource) -> Result<ResourceDef, DefinitionError> {
    if resource.per == Per::Team {
        return Err(DefinitionError("M1 还不支持 per team 资源".to_string()));
    }
    Ok(ResourceDef {
        name: resource.name.clone(),
        per: resource.per,
        init: resource.init.unwrap_or(0),
        min: resource.min,
        max: resource.max,
        visibility: resource.visibility,
    })
}

fn action_def(action: &Action) -> Result<ActionDef, DefinitionError> {
    Ok(ActionDef {
        id: action.id.clone(),
        label: action.label.clone(),
        when: action.when.clone(),
        effects: action.effects.clone(),
    })
}

fn win_def(win: &WinRule) -> WinDef {
    WinDef {
        when: win.when.clone(),
        say: win.say.clone(),
    }
}
