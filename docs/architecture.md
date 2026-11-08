# Oru 架构草案

Oru 是面向对话场景的规则驱动解释型游戏引擎。M0 先立语言前端，后续按下面的层次推进。

## 分层

```text
Application    oru CLI、后续 dsh 适配、编辑器/LSP
Session        开局、推进、存档、回放、版本
Kernel          Match、Turn、Phase、ActionBatch、Event、Outcome、RNG
Family          规则解释器：duel / board / card / auction / social...
Definition      .oru → 不可变 IR（带版本/hash）
Observation     Match + Perspective → ReadModel（隐藏信息投影）
Presentation    ReadModel + ViewSpec → 文本 / SVG / PNG
```

约束：

- Kernel 不知道具体 family；
- family 不拥有换人/判和/回合推进；
- Presentation 不读原始 Match；
- Session 不写规则；
- 包不能执行代码，没有 build script。

## 运行流

```text
.oru → parse/check → Definition
Definition + seats + seed → Match
Match → offers
Action → resolve
Event[] + StateDelta → Match
Match + Perspective → ReadModel
ReadModel + ViewSpec → 文本/图
```

## 事件

从 M1 开始，规则只产生领域事件，不直接拼中文：

```text
MatchStarted
TurnStarted
ActionAccepted
ResourceChanged
CardDrawn
CardPlayed
PiecePlaced
BidCommitted
VoteCast
MatchEnded
```

事件带 `visibility`，投影层按视角过滤；叙述是事件的渲染。

## 里程碑

- **M0（当前）**：`.oru` 词法/语法/AST/检查器 + `Oru.toml` + `oru new/init/check`。
- **M1**：Definition/Match/Kernel，事件模型，顺序行动，duel/board/card。
- **M2**：同时行动、阶段机、队伍/角色、私密投影。
- **M3**：`oru test`、黄金回放、`oru fmt`、LSP。
- **M4**：依赖解析/锁文件/本地与 Git 依赖；注册表最后做。
- **M5**：dsh-pod 适配与聊天渲染。

## 包管理原则

- Manifest：`Oru.toml`；锁文件：`Oru.lock`；
- 包类型：`game` / `rules`；
- 依赖来源：registry / path / git；
- 版本：SemVer；
- 无 build script、无安装脚本、无任意代码执行；
- `oru check` 必须能独立完成规则验证。
