# Oru 架构草案

Oru 是面向对话场景的规则驱动解释型游戏引擎。先立语言前端，再用 duel 与 board 验证 Definition/Family/Kernel 的边界。

## 分层

```text
Application    oru CLI、后续 dsh 适配、编辑器/LSP
Session        开局、推进、存档、回放、版本
Kernel          Match、Turn、Phase、ActionBatch、Event、Outcome、RNG
Family          封闭 FamilyDefinition/FamilyState：duel / board / card / ...
Definition      .oru → 不可变 IR（带版本/hash）
Observation     Match + Perspective → ReadModel（隐藏信息投影）
Presentation    ReadModel + ViewSpec → 文本 / SVG / PNG
```

约束：

- `Definition` 只保留游戏名、座位数、回合上限和 `FamilyDefinition`；duel 专属 resources/actions/triggers/wins 放在 `DuelDefinition`，board 参数放在 `BoardDefinition`；
- Kernel 通过封闭 `FamilyDefinition/FamilyState` 表达具体 family；通用 Match 回合壳只做游标、事件、Outcome、透视投影；
- `ActionOffer` 与 `step` 使用通用 `ActionTarget`，目标细节属于 family；duel 无 target，board 的 place 使用 cell id；`step` 必须传入由当前 Match 铸造、绑定 active seat/turn 的 `ActorSeat`；
- `ReadModel.offers` 按 perspective 投影：条件引用 private resource 时，只对可见该资源的座位/Admin 计算和返回 offer；Spectator 不能经 offers 或 `step` 结果探测 private 条件；private game resource 对普通 Seat/Spectator 隐藏，事件 owner 使用不可映射到玩家的私有 sentinel；
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

- **M0（已完成）**：`.oru` 词法/语法/AST/检查器 + `Oru.toml` + `oru new/init/check`。
- **M1（已完成）**：Definition/Match/Kernel，事件模型，顺序行动，duel 解释器，`oru run`。
- **M2（当前/已落地）**：FamilyDefinition/FamilyState、ActionTarget、board 解释器；共享 `complete_action` 处理 TurnEnded/上限/游标，两个 family 共用同一收尾路径。
- **M2.5**：事件回放、`oru test` 雏形、定义 hash/state version。
- **M3**：card family、阶段机、同时行动；队伍/角色与私密行动。
- **M4**：`oru fmt`、黄金回放、LSP。
- **M5**：依赖解析/锁文件/本地与 Git 依赖；注册表最后做。
- **M6**：dsh-pod 适配与聊天渲染。

## 包管理原则

- Manifest：`Oru.toml`；锁文件：`Oru.lock`；
- 包类型：`game` / `rules`；
- 依赖来源：registry / path / git；
- 版本：SemVer；
- 无 build script、无安装脚本、无任意代码执行；
- `oru check` 必须能独立完成规则验证。
