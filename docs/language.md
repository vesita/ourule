# Oru 语言 M0

> 目标：把“规则写起来像声明，检查起来像编译器”先跑通。

## 文件

- 后缀 `.oru`；
- 一个项目由 `Oru.toml` + `src/main.oru` 组成；
- 一个 `.oru` 文件可以包含多个 `resource` / `action` / `win`，但只有一个 `game` / `seats` / `edition`。

## 最小示例

```oru
game "月影对决"
seats 2
edition 1

resource hp {
  per player
  init 12
  min 0
  max 12
}

resource energy {
  per player
  init 2
  min 0
  max 3
}

action slash "挥砍" {
  when self.energy >= 1
  do {
    other.hp -= 4
    self.energy -= 1
  }
}

on turnEnd {
  self.energy += 1
}

win {
  when other.hp <= 0
  say "{actor} 击倒了对手"
}
```

## 声明

### game / seats / edition

```oru
game "名字"
seats 2
edition 1
```

- `game` 必填；
- `seats` 必填，2–8；
- M0 只支持 `edition 1`。

### resource

```oru
resource hp {
  per player        # player | game | team（M0 的 team 只声明，不运行）
  init 12
  min 0
  max 12
  visibility public # public | private（M0 只校验，投影在 M1）
}
```

- 资源名：小写字母开头，小写字母/数字/`_`，2–32 字符；
- `init` 必须在 `min..=max` 内；
- 变量路径只能访问同作用域：
  - `self.hp` / `other.hp` → `per player`
  - `game.round` → `per game`

### action

```oru
action slash "挥砍" {
  when self.energy >= 1
  do {
    other.hp -= 4
    self.energy -= 1
  }
}
```

- `action` 的 id：小写字母/数字/`-`/`_`，2–32 字符；
- `label` 可省略，默认等于 id；
- `when` 必须是布尔表达式；
- `do` 里是顺序执行的效果。

### on

```oru
on turnEnd {
  self.energy += 1
}
```

M0 只接受：

- `start`
- `turnStart`
- `turnEnd`

### win

```oru
win {
  when other.hp <= 0
  say "{actor} 击倒了对手"
}
```

- 至少一条 `win`；
- `when` 必须是布尔表达式；
- `say` 是叙述模板，M0 只检查非空，不执行。

## 表达式

M0 支持：

```text
整数、true/false
self.hp / other.energy / game.round
+ - * / 与括号
== != < <= > >=
and / or / !
```

类型规则：

- 算术运算两边必须是数字；
- 比较运算两边必须是数字，结果是布尔；
- `and` / `or` 两边必须是布尔；
- `when` / `win.when` 必须是布尔。

## 效果

M0 支持：

```oru
other.hp -= 4
self.energy += 1
game.round = 1
say "一句话"
end win "胜利文案"
end draw
```

- `=` / `+=` / `-=` 左边必须是已声明资源路径；
- 右边必须是数字表达式；
- `end` 的结局：`win` / `lose` / `draw`。

## 检查器

`oru check` 的阶段：

```text
lex → parse → AST → 名字解析 → 类型检查 → 规则约束
```

诊断格式：

```text
error[ORU0329]: 未声明的资源 `energi`
 --> src/main.oru:12:5
  |
12 |   when self.energi >= 1
  |        ^^^^^^^^^^^
  = help: 你是想写 `energy` 吗？
```

`--json` 输出给 CI/编辑器使用。

## M0 不做的

- 运行时解释器；
- 阶段机、同时行动、队伍/角色；
- 卡牌/网格/视图/渲染；
- 依赖解析、锁文件、注册表；
- LSP、fmt、test。

这些在后续里程碑按 `docs/architecture.md` 的层次加入。
