# Oru 项目约定

- 这是独立项目，不依赖 dsh-pod；dsh-pod 只会在后续通过适配层接入。
- M0 的边界：语言前端 + 检查器 + 项目脚手架；不要在 M0 里提前写运行时。
- 检查器是“编译期”：所有规则错误应尽量在 `oru check` 报出来，运行时只做快速校验。
- 无 build script、无任意代码执行；`.oru` 是声明式数据。
- 修改语言/检查器时，同步更新 `docs/language.md`、示例和测试。

## 命令

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p oru-cli -- check examples/duel.oru
```
