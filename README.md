# Oru

> A rule-driven, declarative game engine for conversation-first play.

Oru 是一个面向聊天/对话场景的独立游戏引擎项目：游戏规则、状态和渲染都用声明式
`.oru` 描述，运行时只解释，不执行任意代码。

当前是 M2：语言前端 + 检查器 + duel/board 内核 + `oru run`，尚无渲染器/包依赖解析。

## 快速开始

```bash
cargo run -p oru-cli -- new moon-chess
cd moon-chess
cargo run --manifest-path ../Cargo.toml -p oru-cli -- check
```

或者直接检查示例：

```bash
cargo run -p oru-cli -- check examples/duel.oru
cargo run -p oru-cli -- run examples/duel.oru --actions slash,slash,slash
cargo run -p oru-cli -- run examples/moon-chess.oru --actions place:a1,place:b1,place:a2,place:b2,place:a3
```

## 项目结构

```text
crates/oru-lang/   词法、语法、AST、语义检查、诊断
crates/oru-core/   Definition、Family、Match、事件、duel/board 回合解释器
crates/oru-cli/    oru new / init / check / run
docs/              语言与架构文档
examples/          最小 .oru 示例
```

## 设计约束

- 规则是数据，不是代码；没有 build script、没有任意代码执行。
- 检查器是编译期：`oru check` 必须能独立报出规则错误。
- 后续加入 card/阶段机、渲染、manifest 依赖/锁文件、测试与 LSP。
