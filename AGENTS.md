# Oru 项目约定

- 这是独立项目，不依赖 dsh-pod；dsh-pod 只会在后续通过适配层接入。
- 当前是 M2：语言前端 + 检查器 + duel/board 内核（`oru run`）；不要提前写渲染/包解析。
- Kernel 是唯一推进回合/判和的地方；family 只结算一步；渲染只读 ReadModel。
- 动作必须经 `Match::actor(activeSeat)` 铸造当回合 `ActorSeat` 并传入 `step`；Perspective 只控制输出，不替代 actor capability。私有条件的 offers 与 step 投影由 Kernel 过滤。
- 检查器是“编译期”：所有规则错误应尽量在 `oru check` 报出来，运行时只做快速校验。
- 无 build script、无任意代码执行；`.oru` 是声明式数据。
- 修改语言/检查器时，同步更新 `docs/language.md`、示例和测试。

## 命令

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p oru-cli -- check examples/duel.oru
cargo run -p oru-cli -- run examples/duel.oru --actions slash,slash,slash
cargo run -p oru-cli -- run examples/moon-chess.oru --actions place:a1,place:b1,place:a2,place:b2,place:a3
```
