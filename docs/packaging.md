# Oru 包管理设计（M4）

## 结构

```text
my-game/
  Oru.toml
  Oru.lock
  src/main.oru
  tests/
```

## Manifest

```toml
[package]
name = "my-game"
version = "0.1.0"
edition = "2026"
kind = "game"        # game | rules
entry = "src/main.oru"

[dependencies]
oru-std = "0.1"
cards = { path = "../cards" }
social = { git = "https://example.com/social", tag = "v0.2.0" }
```

## 规则

1. 包不能有 build script / install script；
2. `Oru.lock` 记录精确版本、source、checksum；
3. `--locked` / `--offline` 必须可复现；
4. 第一版注册表是稀疏 HTTP index，包内容是纯声明式源码 + assets + tests；
5. `oru package --dry-run` 先跑完整 `oru check`；
6. 依赖解析不执行被依赖包的任何代码。

M0 只实现 `Oru.toml` 的 package 字段；依赖解析在 M4。
