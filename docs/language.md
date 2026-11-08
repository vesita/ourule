# Oru 语言 M0–M2

> 目标：把“规则写起来像声明，检查起来像编译器”先跑通。

## 文件

- 后缀 `.oru`；
- 一个项目由 `Oru.toml` + `src/main.oru` 组成；
- 一个 `.oru` 文件可包含多个 family 专属规则声明；`game` / `seats` / `edition` / `family` 最多一个。

## 最小示例

```oru
game "月影对决"
seats 2
edition 1
maxTurns 20

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
- 只支持 `edition 1`；
- `family` 可省略（默认 duel），支持 duel / board；board 必须显式 `family board`；
- duel 的 `maxTurns` 可选，默认 40，按座位数换算成动作上限。

### resource

```oru
resource hp {
  per player        # duel 支持 player / game；team 后续加入
  init 12
  min 0
  max 12
  visibility public # public / private，读取模型按 perspective 投影
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

duel 目前接受：

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
- `say` 是叙述模板，目前只检查非空；事件文案在内核产生，渲染格式由上层负责。

## 表达式

duel 支持：

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

duel 支持：

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

## Board family（M2）

```oru
game "月亮棋"
seats 2
family board
board {
  rows 3
  cols 3
  line 3
}
```

board family 自动提供 `place` 动作；目标是格位 `a1`…，合法落点从 `offers` 读取。
连成 `line` 长度的横/竖/斜线获胜；棋盘满且无人获胜则平局。

board family 目前只支持棋盘参数，不接收 duel 的 `resource/action/on/win` 声明。
`maxTurns` 仍作为通用硬上限；在 board 满盘/连线判定之前，胜负优先于和局。

CLI 示例：

```bash
oru run examples/moon-chess.oru --actions place:a1,place:b1,place:a2,place:b2,place:a3
```

## 暂不支持

- 同时行动/阶段机；
- 卡牌、队伍/角色与私密行动；
- 依赖解析、锁文件、注册表；
- LSP、fmt、完整测试 DSL；
- 自定义渲染。

这些按 `docs/architecture.md` 的里程碑加入。
