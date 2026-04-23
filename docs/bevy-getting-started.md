# Bevy 入门使用文档

## 1. 文档目标

这份文档用于在当前仓库内开始使用 Rust 游戏框架 Bevy。

当前仓库已经新增了一个 `project/` 目录，后续游戏项目将以它作为根目录。也就是说：

- 仓库根目录用于放文档、脚本或其他协作文件
- `project/` 目录用于放实际的 Bevy 游戏工程

因此最合适的起步方式是：

1. 在 `project/` 目录初始化 Cargo 项目。
2. 添加 Bevy 依赖。
3. 跑通一个最小可运行示例。
4. 再开始拆分模块、接入资源和写游戏逻辑。

本文内容依据 2026-04-23 访问的 Bevy 官方 Quick Start 资料整理，并额外在本机用 `bevy = "0.18.1"` 做了最小示例编译验证。

## 2. 环境准备

建议先确认本机具备以下工具：

- `rustc`
- `cargo`
- 编辑器中的 `rust-analyzer`

检查命令：

```powershell
rustc --version
cargo --version
```

如果你后面在 Windows 上遇到图形、链接器或系统依赖相关错误，优先回看 Bevy 官方的 setup 页面，确认操作系统依赖是否齐全。

## 3. 在 `project/` 目录初始化 Rust 项目

现在应该把 `project/` 当成游戏工程根目录。

方式一：先进入 `project/` 再初始化

```powershell
Set-Location project
cargo init --bin .
```

方式二：直接在仓库根目录执行

```powershell
cargo init --bin project
```

执行完成后，`project/` 目录里通常会新增：

- `project/Cargo.toml`
- `project/src/main.rs`
- `project/.gitignore`

然后继续在 `project/` 目录下添加 Bevy：

```powershell
Set-Location project
cargo add bevy
```

如果你希望严格跟本文示例保持一致，可以直接指定版本：

```powershell
Set-Location project
cargo add bevy@0.18.1
```

## 4. 推荐的 `Cargo.toml` 基础配置

Bevy 在默认 debug 配置下通常会比较慢。刚起步时，建议至少把开发期 profile 调整一下。

参考配置：

```toml
[package]
name = "project"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = "0.18.1"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

这里的 `name = "project"` 只是按当前目录名举例，你也可以改成真正的游戏名。

如果你已经有自己的 `Cargo.toml`，只需要把 `bevy` 依赖和上面的 profile 配置合并进去，不要整文件覆盖。

## 5. 第一个可运行示例

把 `project/src/main.rs` 改成下面这样：

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, spin_player)
        .run();
}

#[derive(Component)]
struct Player;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite::from_color(Color::srgb(0.2, 0.7, 0.9), Vec2::new(120.0, 120.0)),
        Transform::default(),
        Player,
    ));
}

fn spin_player(time: Res<Time>, mut query: Query<&mut Transform, With<Player>>) {
    for mut transform in &mut query {
        transform.rotate_z(time.delta_secs());
    }
}
```

然后在 `project/` 目录运行：

```powershell
Set-Location project
cargo run
```

预期效果：

- 弹出一个窗口
- 屏幕中央出现一个方块
- 方块持续旋转

第一次编译会比较久，这是正常现象，因为 Bevy 依赖较多。

## 6. 这个示例包含了哪些核心概念

这段代码已经覆盖了 Bevy 的最基本工作方式：

- `App`：应用入口，负责把插件、系统和资源组织起来
- `DefaultPlugins`：默认插件集合，包含窗口、渲染、输入、资源等基础能力
- `Startup`：启动阶段执行一次的系统
- `Update`：每帧都会执行的系统
- `Component`：挂在实体上的数据
- `Query`：按条件读取实体上的组件
- `Resource`：全局唯一状态，例如 `Time`

## 7. 用 ECS 方式理解 Bevy

Bevy 的核心是 ECS。

你可以把它简单理解成：

- `Entity`：对象 ID，本身几乎没有业务含义
- `Component`：挂在对象上的数据
- `System`：读写数据的逻辑
- `Resource`：全局状态
- `Plugin`：一组功能的打包入口

常见开发顺序通常是：

1. 在 `Startup` 里生成实体。
2. 给实体挂上组件。
3. 在 `Update` 里通过 `Query` 读写这些组件。
4. 当逻辑变多后，再拆成自己的 `Plugin`。

## 8. 推荐的项目目录结构

刚开始你可以把逻辑都写在 `main.rs` 里，但只适合非常短的原型阶段。项目一旦开始增长，建议尽快拆目录。

建议结构：

```text
mybevy/
|-- docs/
|   `-- bevy-getting-started.md
`-- project/
    |-- assets/
    |-- src/
    |   |-- main.rs
    |   `-- game/
    |       |-- mod.rs
    |       |-- plugin.rs
    |       |-- player.rs
    |       |-- camera.rs
    |       `-- ui.rs
    `-- Cargo.toml
```

可以按下面的职责划分：

- `project/src/main.rs`：程序入口、顶层插件注册
- `project/src/game/plugin.rs`：游戏主插件
- `project/src/game/player.rs`：玩家组件和玩家系统
- `project/src/game/camera.rs`：摄像机初始化与跟随逻辑
- `project/src/game/ui.rs`：HUD、菜单、按钮
- `project/assets/`：贴图、音频、字体、场景文件

## 9. 第一阶段之后，尽快改成插件化

当你跑通最小示例后，建议把逻辑从 `main.rs` 挪进自己的插件里。

`project/src/main.rs` 可以收敛成这样：

```rust
use bevy::prelude::*;

mod game;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(game::GamePlugin)
        .run();
}
```

`project/src/game/mod.rs`：

```rust
pub mod plugin;

pub use plugin::GamePlugin;
```

`project/src/game/plugin.rs`：

```rust
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_game)
            .add_systems(Update, update_game);
    }
}

fn setup_game() {}

fn update_game() {}
```

这样做的好处是后面接玩家、敌人、地图、UI、状态机时不会把入口文件写乱。

## 10. 推荐的起步里程碑

如果你准备正式在这个仓库里写游戏，建议按下面顺序推进：

1. 先跑通窗口和一个可见实体。
2. 接键盘输入，让玩家实体能移动。
3. 在 `project/assets/` 下放一张贴图并成功加载。
4. 加入碰撞、边界或最简单的游戏规则。
5. 加入状态管理，比如菜单、游戏中、暂停。
6. 把功能拆到不同模块和插件里。

## 11. 常见早期问题

### 编译很慢

第一次编译慢是正常的，前面的 `profile.dev` 配置会明显改善开发体验。

### 窗口打不开

优先检查这些问题：

- 显卡驱动
- 系统图形环境
- Windows 依赖是否齐全
- 是否处在不支持图形窗口的运行环境

### 编译通过但看不到东西

通常是下面几类原因：

- 忘了生成相机
- 实体没有可见的渲染组件
- 位置或缩放把对象放到了视野之外

## 12. 下一步学什么

最值得先学的顺序是：

1. ECS 基础
2. `Resource`
3. `Plugin`
4. 输入处理
5. 资源加载
6. `State`
7. `Event`

最有效的学习方式不是只看概念，而是：

- 先读官方 Quick Start
- 再跑官方 examples
- 然后把一个小例子拷进自己的项目里改出新行为

## 13. 本仓库的最小启动清单

在你开始写正式玩法之前，至少先完成这几件事：

- 在 `project/` 目录执行 `cargo init --bin .`
- 在 `project/` 目录执行 `cargo add bevy@0.18.1` 或 `cargo add bevy`
- 配好 `profile.dev`
- 创建 `project/assets/` 目录
- 在 `project/` 目录跑通一次 `cargo run`
- 确认窗口正常打开

## 14. 官方参考入口

- Bevy Quick Start: `https://bevy.org/learn/quick-start/getting-started/`
- Bevy Setup: `https://bevy.org/learn/quick-start/getting-started/setup/`
- Bevy 官方 examples: `https://github.com/bevyengine/bevy/tree/latest/examples`
