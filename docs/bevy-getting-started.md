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
|   |-- bevy-getting-started.md
|   |-- assets-workflow.md
|   |-- audio/
|   |-- scene/
|   `-- ui/
`-- project/
    |-- assets/
    |   |-- audio/
    |   |-- game/
    |   |-- licenses/
    |   |-- models/
    |   |-- scenes/
    |   `-- ui/
    |-- src/
    |   |-- framework/
    |   |   |-- audio/
    |   |   |-- fight/
    |   |   |-- network/
    |   |   |-- scene/
    |   |   `-- ui/
    |   |       |-- core/
    |   |       |-- overlays/
    |   |       |-- style/
    |   |       `-- widgets/
    |   |-- main.rs
    |   `-- game/
    |       |-- mod.rs
    |       |-- plugin.rs
    |       |-- authority/
    |       |-- features/
    |       |-- myserver/
    |       |-- navigation/
    |       |-- scenes/
    |       |-- screens/
    |       `-- ui_ids.rs
    `-- Cargo.toml
```

可以按下面的职责划分：

- `project/src/main.rs`：程序入口、顶层插件注册
- `project/src/framework/`：框架层横向能力，当前包含 audio、UI、network、scene、fight 和 fangyuan 边界
- `project/src/framework/audio/`：音频框架能力入口，提供音频命令、事件、catalog、loading、playback、mixer、music、UI/scene/battle adapter、基础空间音频、生命周期暂停和 debug 配置
- `project/src/framework/fangyuan/`：方圆灵构数据模型入口，提供 blueprint、prefab/palette、scene layout、runtime primitive、对象状态和 primitive set 统计
- `project/src/framework/network/`：网络框架能力入口，提供 HTTP、TCP 和 KCP 的 Bevy 消息接口
- `project/src/framework/scene/`：场景框架能力入口，提供场景命令、事件、生命周期、注册表、首包 RON manifest、根实体、Loading、相机、spawn/anchor、trigger、streaming 元数据和 debug 配置
- `project/src/framework/ui/`：UI 框架能力入口
- `project/src/game/plugin.rs`：游戏主插件
- `project/src/game/authority/`：游戏层控制机会话接口和轻量 authority 协议
- `project/src/game/features/`：Touch Ripple、Robot Sync、Fangyuan Player Preview 等具体玩法功能模块
- `project/src/game/myserver/`：当前游戏的 MyServer 登录、房间和协议适配模块
- `project/src/game/navigation/`：主流程 `AppUiMode` 和路由按钮数据
- `project/src/game/scenes/`：具体游戏场景 ID、场景目录 CSV 注册适配和场景专属组合逻辑，当前包含 `sample.dungeon_room` 和 `arena.robot_sync`
- `project/src/game/screens/`：登录、大厅、玩法 HUD、UI Gallery 等具体业务页面
- `project/src/framework/ui/core/`：UI 框架入口、Panel Manager、层级、输入拦截
- `project/src/framework/ui/overlays/`：Toast、Loading、Confirm modal 等顶层 UI 实现
- `project/src/framework/ui/style/`：颜色、字号、间距、圆角等主题 token
- `project/src/framework/ui/widgets/`：按钮、文本等通用控件
- `project/assets/`：贴图、音频、字体、场景文件和首包配置数据
- `project/assets/audio/`：首包音频资源，当前样例以 `.wav` 为主，公开发布前需替换占位资源并确认授权
- `project/assets/fangyuan/palettes/home_prefabs.ron`：方圆家园默认 Prefab / Palette 首包样例
- `project/assets/fangyuan/layouts/home_layout.ron`：方圆家园默认 Scene Layout 首包样例
- `project/assets/game/scenes.csv`：游戏层场景目录表，当前注册 `sample.dungeon_room` 和 `arena.robot_sync`
- `project/assets/scenes/sample_dungeon_room/scene.ron`：样板场景 framework manifest
- `project/assets/scenes/sample_dungeon_room/layout.ron`：样板场景 game layer prefab/light 摆放数据
- `project/assets/scenes/robot_sync_arena/scene.ron`：Robot Sync 场景 framework manifest
- `project/assets/scenes/robot_sync_arena/layout.ron`：Robot Sync 场景 game layer arena/grid/spawn 摆放数据
- `docs/assets-workflow.md`：项目资源使用方式，覆盖开发期、APK 包内和后续下载资源
- `docs/scene/`：场景框架相关文档，当前总文档规划场景生命周期、资源、切换、流式加载、相机和联机同步
- `docs/ui/`：UI 框架实现机制、组件使用、响应式布局、调试验收和限制说明

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

## 14. 桌面端模拟设备分辨率

桌面开发时可以通过窗口 profile 启动，用来模拟手机或平板分辨率验收 UI。所有命令都在 `project/` 目录执行：

```powershell
cargo run -- --window-profile phone-portrait
cargo run -- --window-profile phone-1080p
cargo run -- --window-profile phone-small
cargo run -- --window-profile tablet-portrait
cargo run -- --window-profile tablet-landscape
cargo run -- --window-profile desktop
```

也可以直接传自定义设备物理分辨率。程序会对已知尺寸推断设备缩放，例如 `1280x2772` 会按当前 Android 验收机的 `3.25` 缩放模拟，UI 逻辑宽度约为 `394`：

```powershell
cargo run -- --window-size 1280x2772
```

如果需要模拟其它 DPI/缩放，可以显式传设备缩放：

```powershell
cargo run -- --window-size 1280x2772 --device-scale 3.25
```

如果设备分辨率在当前显示器上放不下，可以增加桌面预览缩放。预览缩放只影响桌面窗口显示尺寸，不改变 UI 逻辑排版：

```powershell
cargo run -- --window-profile phone-portrait --window-scale 50%
cargo run -- --window-size 1280x2772 --window-scale 0.5
```

当前内置 profile。phone/tablet profile 同时带确定性 safe-area fixture，用于桌面布局审计；它不是 Android 真机数据：

- `desktop`: `1280x720`, scale `1.0`
- `phone-portrait`: `1280x2772`, scale `3.25`
- `phone-1080p`: `1080x2400`, scale `3.0`
- `phone-small`: `720x1600`, scale `2.0`
- `tablet-portrait`: `1600x2560`, scale `2.0`
- `tablet-landscape`: `2560x1600`, scale `2.0`

可显式覆盖逻辑像素安全区，参数顺序为 `LEFT,RIGHT,TOP,BOTTOM`：

```powershell
cargo run -- --window-profile phone-small --safe-area-insets 0,0,30,24
```

Android 生产环境不读取这个桌面参数。Activity 通过 `WindowInsetsCompat -> JNI` 发布状态栏、display cutout 和导航/手势 inset，UI 再按当前 device scale 转为逻辑像素。

如果参数非法，程序会打印 warning 并回退到默认桌面尺寸。该功能只作用于桌面端 primary window 的启动尺寸，不改变 Android 真机默认行为。

### UI 调试和审计入口

桌面 debug 运行时可按 `F3` 打开 UI 调试面板，按 `F9` 保存 primary window 截图。F9 默认写入仓库根目录的 `summary/ui-audit/manual/`，该目录被 Git 忽略。

批量 UI 审计从仓库根目录运行：

```powershell
.\scripts\run-ui-audit.ps1 -SelfTest
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,tablet-landscape -States "top,middle,bottom" -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,phone-portrait,tablet-portrait,tablet-landscape -States visual_acceptance
```

`-DryRun` 只验证矩阵和报告，不启动游戏、不生成真实截图。真实审计去掉 `-DryRun`，产物会写入 `summary/ui-audit/<run-id>/`。`visual_acceptance` metadata 同时包含安全区来源、图片/字体/效果/动画汇总和开发期视觉预算；预算不是 GPU 实测。更多说明见 `docs/ui/UI调试与验收.md`、`docs/ui/UI安全区与视觉预算.md` 和 `docs/ui/UI自动化审计与优化方案.md`。

## 15. 场景框架开发期环境变量

场景框架支持一组开发期环境变量，用于从启动时进入指定场景、指定 spawn、打开诊断或模拟加载异常。所有命令仍在 `project/` 目录执行：

```powershell
$env:MYBEVY_START_SCENE="sample.dungeon_room"
$env:MYBEVY_START_SPAWN="spawn.default"
$env:MYBEVY_SCENE_DEBUG="true"
$env:MYBEVY_SCENE_LOG_LIFECYCLE="true"
$env:MYBEVY_SCENE_SLOW_LOADING_SECONDS="1.5"
$env:MYBEVY_SCENE_SIMULATE_FAILURE="asset_load"
cargo run
```

变量说明：

- `MYBEVY_START_SCENE`：启动后自动发送 `SceneCommand::Enter` 的场景 ID；该场景必须已经由 game layer 注册。
- `MYBEVY_START_SPAWN`：启动场景使用的 spawn point ID。
- `MYBEVY_SCENE_DEBUG`：启用场景 debug 配置；接受 `1`、`true`、`on`、`yes`、`enabled` 等值。
- `MYBEVY_SCENE_LOG_LIFECYCLE`：生命周期日志开关；未设置时默认跟随 `MYBEVY_SCENE_DEBUG`。
- `MYBEVY_SCENE_SLOW_LOADING_SECONDS`：慢加载模拟秒数配置，必须是正数。
- `MYBEVY_SCENE_SIMULATE_FAILURE`：失败模拟类型，当前可解析 `manifest_load`、`asset_load`、`camera_setup`。

这些变量只用于开发期。正式入口应来自登录、房间、存档或服务端协议。

当前已注册的首包场景包括：

- `sample.dungeon_room`：来自 `project/assets/scenes/sample_dungeon_room/scene.ron` 和同目录 `layout.ron`，用于验证基础世界内容流程。
- `arena.robot_sync`：来自 `project/assets/scenes/robot_sync_arena/scene.ron` 和同目录 `layout.ron`，用于验证 500x500 机器人帧同步场景。
- `dev.fangyuan_home`：方圆家园开发预览场景，默认读取 `fangyuan/layouts/home_layout.ron` 和 `fangyuan/palettes/home_prefabs.ron`，用于验收 layout/palette 展开、审核 HUD、审核日志和方圆静态渲染模式。

也可以从正常 UI 流程进入：启动游戏、登录到大厅，在 `game_list` 点击 `Sample Scene`、`Robot Sync` 或 `Fangyuan Home` 的 `Enter` 按钮；进入成功后会显示对应 HUD，点击 `Lobby` 返回大厅。

Robot Sync 单客户端本地验收：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="arena.robot_sync"
cargo run -- --window-profile phone-small --window-scale 50%
```

Robot Sync 默认使用自动 bot 发送移动输入。手动验收时可以改用键盘输入，`WASD` 或方向键控制本地机器人，`W` / `ArrowUp` 对应屏幕上方，`S` / `ArrowDown` 对应屏幕下方，松开按键会发送停止输入：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="arena.robot_sync"
$env:ROBOT_SYNC_INPUT_MODE="manual"
cargo run -- --window-profile phone-small --window-scale 50%
```

Robot Sync 当前还提供开发期摄像机切换按键：按 `C` 在默认 Overview 总览和 FollowLocal 本地跟随之间切换。默认进入仍是 Overview；FollowLocal 使用高俯视 offset、FOV `0.78` 和短过渡，便于验证跟随镜头，同时可随时按 `C` 回退总览。该切换不写入 Robot Sync replay state，不进入 checksum，也不改变 MyServer/LAN authority 语义。

方圆家园审核和预算 HUD 验收：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="dev.fangyuan_home"
$env:MYBEVY_SCENE_DEBUG="true"
cargo run -- --window-profile phone-small --window-scale 50%
```

HUD 会显示 layout 展开数量、palette / prefab / instance / material 统计，以及 audit 状态、error / warning 数和主 code。Windows 下如果默认图形后端初始化或可视验收不稳定，可以在同一终端临时指定 DX12：

```powershell
$env:WGPU_BACKEND="dx12"
cargo run -- --window-profile phone-small --window-scale 50%
```

方圆灵构试炼场和第十五阶段调试工具入口：

```powershell
Set-Location project
$env:TOUCH_START_SCREEN="fangyuan_home"
cargo run -- --window-profile phone-small
```

如果需要同时走完整场景加载、家园 layout / palette 展开和审核 HUD，优先使用 `MYBEVY_START_SCENE="dev.fangyuan_home"`；`TOUCH_START_SCREEN=fangyuan_home` 更适合快速打开 Fangyuan Home UI、试炼场按钮和开发者 debug panel。

第十五阶段调试工具的定向验收命令：

```powershell
Set-Location project
cargo test fangyuan_trial -- --nocapture
cargo test fangyuan_debug_panel -- --nocapture
cargo test fangyuan_pressure -- --nocapture
cargo test fangyuan_visual_replay -- --nocapture
cargo test fangyuan_debug_report -- --nocapture
```

其中 `fangyuan_trial` 覆盖试炼场审核解释、预算档位、降级前后和 fallback；`fangyuan_debug_panel` 覆盖开发者面板指标和模块开关；`fangyuan_pressure` 覆盖本地 deterministic 100 / 300 / 1000 人压力模拟和报告摘要；`fangyuan_visual_replay` 覆盖 authority replay 视觉摘要 hash 和 mismatch 定位；`fangyuan_debug_report` 覆盖调试报告 schema、`artifacts/fangyuan-debug/` 输出路径约定和 pressure baseline 比较格式。

这些入口只用于本地开发、调试和回归验证。第十五阶段不是运营后台、云压测平台或商业监控系统，也不提供线上告警、商业观测计费或生产级监控数据管线。

方圆 Bake dry-run 从仓库根目录执行，用于检查 `project/assets/fangyuan` 的 RON 源、依赖表、预算统计和报告输出，不生成 `.fyb` 首包产物：

```powershell
.\scripts\run-fangyuan-bake-dry-run.ps1
```

报告默认写入 `artifacts/fangyuan-bake/dry-run/report.txt`。其中 `ron_load_us`、`bin_load_us`、`peak_resource_count` 和 `artifact_size` 是开发机诊断字段，用来对比 RON 路径和当前 `FYBAKE` artifact 路径的解析/校验差异，不代表 Android 真机性能数据。需要清理正式 bake 输出目录时，使用 `fangyuan_bake --clean-output`，不要在 dry-run 中生成或提交 `.fyb`。

方圆调试加载和缓存模型的常用验收命令：

```powershell
Set-Location project
cargo test fangyuan_chunk_loading -- --nocapture
cargo test fangyuan_runtime_load -- --nocapture
cargo test fangyuan_blueprint_cache -- --nocapture
cargo test fangyuan_streaming_update -- --nocapture
cargo test fangyuan_epoch_inheritance -- --nocapture
cargo test fangyuan_cache_authority -- --nocapture
```

其中 chunk loading 覆盖 load、unload、reload、失败 fallback 和场景退出 clear；runtime load 覆盖 `.fyb` header/schema/kind/hash/payload 和 RON fallback 边界；blueprint cache 覆盖 version/hash/dependency 校验、LRU/use_count 淘汰和损坏文件；streaming update 覆盖部分更新、依赖缺失、版本回退、预算、签名占位、权限占位和 rollback key；epoch inheritance 覆盖旧蓝图升级、重新审核和预算降级；cache authority 覆盖服务端 manifest 覆盖本地 stale cache。

完整方圆回归可直接运行：

```powershell
Set-Location project
cargo test fangyuan -- --nocapture
```

方圆家园还支持开发期渲染模式环境变量，用于对比 standard、CPU merge 和 static instance shared-mesh prototype。未设置或填入非法值时默认使用 `standard`：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="dev.fangyuan_home"
$env:MYBEVY_SCENE_DEBUG="true"
$env:MYBEVY_FANGYUAN_HOME_RENDER_MODE="standard"
cargo run -- --window-profile phone-small --window-scale 50%

$env:MYBEVY_FANGYUAN_HOME_RENDER_MODE="cpu_merge"
cargo run -- --window-profile phone-small --window-scale 50%

$env:MYBEVY_FANGYUAN_HOME_RENDER_MODE="static_instance"
cargo run -- --window-profile phone-small --window-scale 50%
```

`MYBEVY_FANGYUAN_HOME_RENDER_MODE` 当前支持 `standard`、`cpu_merge` 和 `static_instance`；`cpu-merge`、`merge`、`static-instance`、`staticinstance`、`instancing`、`instance` 也会解析到对应模式。HUD / 日志会显示 `render_mode`、material profile、opaque / transparent、emissive、material resource、static instance batch / count / buffer bytes 和 fallback reason。CPU merge 或 static instance 构建失败时默认回退到 `standard`。

如需在 Windows 下用 DX12 复验三种模式，可以在同一终端先设置：

```powershell
$env:WGPU_BACKEND="dx12"
```

该入口只用于开发期可视验收。当前方圆已具备 chunk/loading、LOD、AOI、热点灵压、Bake/runtime loader、blueprint cache、streaming update、epoch inheritance 和 cache authority 的本地模型、工具链与测试；仍不包含完整服务器集群、商业 CDN、运营后台、账号资产交易、最终云压测平台、正式 GPU instance buffer / custom render pipeline、联网同步、正式家园编辑器、蓝图持久化或任意 shader。

Robot Sync MyServer 模式常用环境变量：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="arena.robot_sync"
$env:ROBOT_SYNC_AUTHORITY_MODE="myserver"
$env:AUTHORITY_PLAYER_ID="robot-player-a"
$env:AUTHORITY_MYSERVER_GUEST_ID="robot-guest-a"
$env:AUTHORITY_MYSERVER_ROOM="robot-sync-room"
$env:AUTHORITY_MYSERVER_POLICY="robot_sync_room"
$env:ROBOT_SYNC_INPUT_MODE="manual"
$env:MYSERVER_TRANSPORT="tcp"
$env:MYSERVER_GAME_HOST="127.0.0.1"
$env:MYSERVER_TCP_FALLBACK_PORT="17002"
cargo run -- --window-profile phone-small --window-scale 50%
```

如果本机 MyServer `game-proxy` 没有覆盖 TCP fallback 端口，端口也可能是默认 `PROXY_PORT + 10000`。以 `C:\project\MyServer\apps\game-proxy\.env` 或启动日志为准。

`TOUCH_START_SCREEN=sample_scene` 只会把 UI state 切到样板场景 HUD，适合调试 HUD 本身；它不会自动发送 `SceneCommand::Enter`，因此不是完整场景加载验收方式。完整验收优先使用大厅入口或 `MYBEVY_START_SCENE="sample.dungeon_room"`。

## 16. 音频框架开发期环境变量

音频框架当前支持开发期 debug 开关。所有命令仍在 `project/` 目录执行：

```powershell
$env:MYBEVY_AUDIO_DEBUG="true"
cargo run
```

变量说明：

- `MYBEVY_AUDIO_DEBUG`：启用 `AudioDebugSnapshot` 更新；接受 `1`、`true`、`on`、`yes`、`enabled` 等值。

启用后可以从资源中查看当前活跃音频实例数量、按 bus 统计、实例详情、加载 group 进度、最近播放 cue、最近跳过 cue 和最近加载失败资源。游戏内 `Audio Monitor` 页面会自动启用采集，并展示当前 debug snapshot。

开发期可以直接打开音频页面：

```powershell
Set-Location project
$env:TOUCH_START_SCREEN="audio_monitor"
cargo run

$env:TOUCH_START_SCREEN="audio_gallery"
cargo run
```

`TOUCH_START_SCREEN=audio-gallery` 也会进入 Audio Gallery。Audio Gallery 是开发测试页，可主动触发 SFX、loop、music、spatial、bus、rules 和 `bank.audio_gallery` lazy-unload 行为；它使用 `dev.audio_gallery` scope，退出页面时会清理本页测试实例。

基本验收入口：

- 注册或加载 `ui.click` cue 后，启动游戏并点击普通 UI 按钮，验证默认 UI click adapter 可触发音效。
- 从大厅进入 `Sample Scene`，验证 `sample.dungeon_room` 的场景 ambience 由 game layer 注册并在退出时按 scene scope 清理。
- 从大厅、`Audio Settings` 或 `Audio Monitor` 进入 `Audio Gallery`，验证开发样例音频、bus 控制、diagnostics 和 lazy bank 状态。
- 需要同时看场景和音频诊断时，可同时设置 `MYBEVY_START_SCENE="sample.dungeon_room"`、`MYBEVY_SCENE_DEBUG="true"` 和 `MYBEVY_AUDIO_DEBUG="true"`。

## 17. 官方参考入口

- Bevy Quick Start: `https://bevy.org/learn/quick-start/getting-started/`
- Bevy Setup: `https://bevy.org/learn/quick-start/getting-started/setup/`
- Bevy 官方 examples: `https://github.com/bevyengine/bevy/tree/latest/examples`
- 本仓库资源使用方式：`docs/assets-workflow.md`
- 本仓库音频框架说明：`docs/audio/README.md`
- 本仓库场景框架设计：`docs/scene/README.md`

## 18. 本项目如何打包成 Windows 和 Android App

这一节只针对当前仓库结构说明：

- 仓库根目录不是 Cargo 工程根目录
- 真正的游戏工程在 `project/`
- 当前已经是一个可运行的桌面 Bevy 二进制项目

### Windows 打包

Windows 版最直接，就是构建 release 可执行文件。

在仓库根目录执行：

```powershell
Set-Location project
cargo build --release
```

构建完成后，产物在：

```text
project/target/release/project.exe
```

如果后续你在 `project/assets/` 里放了贴图、音频、字体等资源，发布时通常要把资源目录一起带上。常见发布目录结构：

```text
dist/
|-- project.exe
`-- assets/
```

也就是说：

1. 先执行 `cargo build --release`
2. 拿到 `project/target/release/project.exe`
3. 把 `project/assets/` 复制到最终发布目录
4. 然后把整个目录发给别人运行

如果你后面想做真正的安装包，再额外接 Inno Setup、WiX 或 NSIS 即可，但对 Bevy 来说第一步并不是“安装包”，而是先产出 release 的 `.exe`。

### Android 打包

Android 不能直接把当前这个 `main.rs` 桌面程序原样打成 APK。

你还需要补三层东西：

1. Rust 的 Android 目标工具链
2. Android NDK 和 `cargo-ndk`
3. 一个 Android Studio / Gradle 壳工程，用来把 Rust 产出的 `.so` 打进 APK

#### 第一步：补齐 Android 构建环境

先安装 Rust Android targets：

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi
```

再安装 `cargo-ndk`：

```powershell
cargo install cargo-ndk
```

然后确认 Android Studio 里已经安装：

- Android SDK
- Android NDK
- platform-tools
- build-tools

建议把下面环境变量配好：

```powershell
$env:ANDROID_SDK_ROOT="C:\Users\你的用户名\AppData\Local\Android\Sdk"
$env:ANDROID_NDK_HOME="C:\Users\你的用户名\AppData\Local\Android\Sdk\ndk\版本号"
```

#### 第二步：把游戏逻辑从 `main.rs` 抽到 `lib.rs`

桌面版保留 `main.rs` 没问题，但 Android 一般需要把主逻辑做成库，再由 Android 工程加载。

建议改成：

```text
project/
|-- src/
|   |-- main.rs
|   `-- lib.rs
```

推荐结构：

- `src/lib.rs`：提供 `pub fn run()`，里面放 `App::new()...run()`
- `src/main.rs`：桌面入口，只负责调用 `project::run()`

同时在 `project/Cargo.toml` 里补一个库目标：

```toml
[lib]
crate-type = ["rlib"]
```

桌面开发默认只构建 `rlib`，避免 Windows 本地运行和测试额外链接动态库。Android 需要的 `libproject.so` 在构建时通过 `cargo ndk ... rustc --lib --crate-type cdylib` 显式产出。

如果你准备跟 Bevy 当前移动端默认方案保持一致，通常用 `GameActivity` 即可；如果你要兼容更老的 Android API，再考虑 `android-native-activity`。

#### 第三步：补 Android 壳工程

最省事的方式不是自己从零配 Gradle，而是直接参考 Bevy 官方的移动示例：

- `examples/mobile/android_example/`

你可以在仓库根目录旁边或仓库内新建一个 Android 工程目录，例如：

```text
mybevy/
|-- android/
|-- docs/
`-- project/
```

这个 `android/` 工程的职责只有两个：

1. 从 Rust 工程编译出 `.so`
2. 把 `.so` 和 `assets/` 一起打包成 APK

#### 第四步：编译 Android 的 Rust 动态库

在 `project/` 目录执行类似命令：

```powershell
cargo ndk -t arm64-v8a -o ..\android\app\src\main\jniLibs rustc --release --lib --crate-type cdylib
```

执行后会在 `android/app/src/main/jniLibs/arm64-v8a/` 下得到对应的 Rust 动态库。

如果你还要支持更多架构，再额外构建：

- `armeabi-v7a`
- `x86_64`

#### 第五步：在 Android 工程里打 APK

进入 Android 工程目录后执行：

```powershell
.\gradlew assembleDebug
```

或发布版：

```powershell
.\gradlew assembleRelease
```

最终 APK 通常在：

```text
android/app/build/outputs/apk/debug/
android/app/build/outputs/apk/release/
```

#### 资源目录怎么带进 Android

如果你的资源放在 `project/assets/`，Android 工程也要能看到它。

最常见有两种做法：

1. 构建前把 `project/assets/` 复制到 `android/app/src/main/assets/`
2. 在 Gradle `sourceSets` 里直接把 Rust 工程的 `assets/` 目录映射进去

第二种通常更适合当前仓库，因为可以继续只维护一份资源。

### 当前仓库的实际结论

当前仓库已经具备同一套 Bevy 代码分别运行桌面版和 Android 版的基础结构：

- `project/src/main.rs`：桌面入口，只负责调用 `project::run()`
- `project/src/lib.rs`：共享 Bevy App 入口，并通过 `#[bevy_main]` 支持移动端入口
- `project/src/game/`：当前游戏玩法模块
- `project/Cargo.toml`：桌面默认构建 `rlib`；Android 动态库由 `cargo ndk ... rustc --lib --crate-type cdylib` 产出
- `android/`：Android Gradle 壳工程，会加载 `libproject.so`

当前 Android 壳工程使用 Bevy 0.18.1 间接依赖的 `android-activity 0.6.1`。
因此 `android/gradle/libs.versions.toml` 里的 `androidx.games:games-activity`
需要保持为 `4.4.0`，并且不要在 Gradle 中启用 `prefab`。否则 Java 侧
`GameActivity` 和 Rust 侧 native glue 的 JNI 方法签名可能不匹配，启动时会出现
`RegisterNatives failed for 'com/google/androidgamesdk/GameActivity'`。

`MainActivity` 还覆写 GameActivity 的 `WindowInsetsCompat` 回调，并保留 `super` 调用；系统栏、display cutout 和导航/手势物理 inset 通过项目 JNI symbol 进入 `UiSafeAreaStatus`。修改 Activity 或 Rust FFI 后，除了 `cargo check`，还应重新构建 arm64 动态库和 Debug APK。没有授权设备时只能证明编译/打包成立，不能声称安全区、字体、触控、图片切片或效果降级已通过真机验收。

当前玩法是单界面触控/鼠标互动，并已接入控制机帧同步：

1. 本地鼠标左键或手指输入会按帧聚合为 `ui_touch` 输入。
2. 玩法层消费 `AuthorityEvent::FrameApplied`，按玩家回放触控位置。
3. 鼠标左键或手指按下时，在对应位置显示硬边半透明圆形反馈。
4. 按住拖动时，主圆平滑跟随，并沿拖动路径生成水波纹拖尾。
5. 松开后，主圆在原地逐帧淡出；新一次按压会直接在新位置生成。

当前还内置 `arena.robot_sync` 场景，用于正式验证场景内机器人帧同步：

1. 场景显示 500x500 sync arena，并按 `0.1 world3d units / sync unit` 渲染 glTF 地板、边界、网格、出生点和 GLB 人物机器人。
2. 本地 bot 发送 `robot_move` 输入。
3. 玩法层只消费 `AuthorityEvent::FrameApplied` 推进机器人 fixed 坐标。
4. Robot Sync HUD 显示 room、player、authority 状态、frame、机器人数量和本地 fixed/sync/world3d 坐标。
5. 双客户端 MyServer 联调依赖 MyServer `robot_sync_room` policy；服务端只校验和转发 `robot_move`，不广播机器人坐标。

当前工程已经内置一套网络通信接口：

- `project/src/framework/network/`：网络框架插件、命令、事件和连接配置
- `NetworkPlugin`：已经在 `project/src/lib.rs` 中注册
- `NetworkCommand`：从 Bevy 系统发起 HTTP 请求、TCP/KCP 连接、TCP/KCP 监听、发送数据、断开连接或停止监听
- `NetworkEvent`：接收 HTTP 响应、连接状态、监听状态、接入连接、数据包、发送结果和错误

HTTP 是一次性请求接口；TCP 和 KCP 是长连接接口，都会返回 `ConnectionId` 对应的连接事件。
网络实际 I/O 在后台 Tokio runtime 中执行，不阻塞 Bevy 主线程。Android 包已经在
`android/app/src/main/AndroidManifest.xml` 中声明了 `android.permission.INTERNET`。

当前工程还内置一套控制机会话接口：

- `project/src/game/authority/`：控制机统一接口和轻量 authority 协议
- `AuthorityPlugin`：已经在 `project/src/game/plugin.rs` 中注册
- `AuthorityCommand`：创建本地控制机、创建局域网控制机、加入控制机、切换控制机、发送玩法输入或离开
- `AuthorityEvent`：接收控制机连接状态、peer 加入/离开、输入确认、权威帧、快照和迁移事件

玩法层应优先依赖 `AuthorityCommand` / `AuthorityEvent`，而不是直接依赖 `MyServerCommand`。
远端 MyServer 仍作为一种控制机 endpoint 由 adapter 桥接；本地和局域网控制机使用客户端内置 authority 协议。

Touch Ripple 默认会在进入玩法界面时自动启动本地控制机，方便单机验证。连接 MyServer 的
`UITouchRoom` 时，可使用：

```powershell
$env:TOUCH_AUTHORITY_MODE="myserver"
$env:TOUCH_ROOM_ID="ui-touch-room"
$env:MYSERVER_GUEST_ID="bevy-a"
cargo run
```

客户端会登录 MyServer、加入 `policy_id = "ui_touch_room"` 的房间、准备并尝试开始房间。
如果要关闭本地自动控制机，可设置：

```powershell
$env:TOUCH_AUTO_LOCAL_AUTHORITY="false"
```

也可以在仓库根目录一键启动两个 Touch Ripple 客户端。脚本会先构建一次项目，再启动一个
`lan-host` 客户端和一个 `lan-client` 客户端，并把日志写到 `logs/two-clients/`：

```powershell
.\scripts\start-two-clients.ps1
```

开发期可以用环境变量直接启动 authority 测试入口：

```powershell
# 连接 MyServer，自动登录、进房、准备、开始并定时发输入
$env:AUTHORITY_DEV_MODE="myserver"
$env:AUTHORITY_MYSERVER_GUEST_ID="bevy-a"
$env:AUTHORITY_MYSERVER_ROOM="room-dev"
$env:AUTHORITY_MYSERVER_POLICY="movement_demo"
cargo run

# 开一个局域网控制机
$env:AUTHORITY_DEV_MODE="lan-host"
$env:AUTHORITY_PLAYER_ID="host-a"
$env:AUTHORITY_BIND_ADDR="127.0.0.1:15000"
cargo run

# 另一个终端连接这个控制机
$env:AUTHORITY_DEV_MODE="lan-client"
$env:AUTHORITY_PLAYER_ID="client-b"
$env:AUTHORITY_REMOTE_HOST="127.0.0.1"
$env:AUTHORITY_REMOTE_PORT="15000"
cargo run
```

Robot Sync 双客户端脚本会默认启动两个 `arena.robot_sync` 客户端，设置同一 room、`robot_sync_room` policy、不同 player/guest，并把日志写到 `logs/robot-sync-two-clients/<timestamp>/`：

```powershell
# 只打印两端环境变量和 launcher，不启动窗口
.\scripts\start-robot-sync-two-clients.ps1 -DryRun -SkipBuild

# MyServer 模式，默认使用 tcp transport；A 端手动控制，B 端静止观察
.\scripts\start-robot-sync-two-clients.ps1 -SkipBuild -HostAddress 127.0.0.1 -Port 17002

# 如需自动 bot，可显式开启
.\scripts\start-robot-sync-two-clients.ps1 -SkipBuild -HostAddress 127.0.0.1 -Port 17002 -InputModeA bot -InputModeB bot

# LAN fallback，不依赖 MyServer
.\scripts\start-robot-sync-two-clients.ps1 -Mode lan -SkipBuild
```

MyServer 本地联调前，先在服务端仓库启动完整栈：

```powershell
Set-Location C:\project\MyServer
powershell -ExecutionPolicy Bypass -File .\scripts\dev-stack.ps1 -WithMatch
```

当前本地完整栈需要 `-WithMatch`，否则 `game-server` 可能因为缺少 `match-service.grpc` 发现而无法正常启动。Robot Sync 使用的 TCP fallback 端口由 `game-proxy` 配置决定；脚本的 `-Port` 会写入 `MYSERVER_TCP_FALLBACK_PORT`，KCP transport 时写入 `MYSERVER_KCP_PORT`。

MyServer 最新登录链路是账号身份和游戏角色身份分离的流程：

1. 账号登录或游客登录先访问 `auth-http`，获得账号级 access token。
2. 客户端拉取角色列表；没有角色时先创建角色，有角色时可读取 profile。
3. 客户端选择角色后获得 character-bound game ticket。
4. 客户端用 ticket 连接 `game-proxy` 并发送 `AuthReq`。
5. `AuthRes.player_id` 是账号级 ID，仅保存到 `MyServerSession.player_id`；玩法、房间、输入、authority 本地主体、Touch Ripple 和 Robot Sync replay 都使用当前 ticket 绑定的 `character_id`。
6. ticket 缺少 `character_id`、ticket 与当前选角不一致，或鉴权成功返回时本地没有选中角色，都按可诊断失败处理，不回退到账号 `player_id`。

常用客户端环境变量：

```powershell
Set-Location project
$env:MYSERVER_HTTP_BASE_URL="http://127.0.0.1:3000"
$env:MYSERVER_GAME_HOST="127.0.0.1"
$env:MYSERVER_TRANSPORT="tcp"
$env:MYSERVER_TCP_FALLBACK_PORT="14000"
$env:MYSERVER_TICKET_REFRESH_MARGIN_MS="30000"
$env:MYSERVER_AUTO_RECONNECT_WITH_FRESH_TICKET="true"
$env:MYSERVER_DIAGNOSTIC_TRACE="true"
```

最小手工验收顺序：

1. 启动 MyServer 完整本地栈，并确认 `auth-http`、`game-server`、`game-proxy`、Redis、NATS 和 PostgreSQL 均可达。
2. 用登录页执行正式账号登录，或点击游客登录。
3. 拉取角色列表；如为空，创建角色。
4. 选择当前角色，确认 ticket 签发请求体使用 `character_id`。
5. 连接 game proxy，确认 `AuthReq` 后进入 `Authenticated`，authority local subject 显示为 `chr_*`。
6. 加入 `ui-touch-room`、`robot-sync-room` 或当前测试房间，验证 ready、start、输入和服务端帧推送。
7. 验证 ticket 提前补发、断开重连、redirect、kick 和维护模式的错误展示与日志字段。
8. 进入游戏后发送 `GetCharacterElementsReq(1413)`，并能消费 `CharacterElementsChangePush(1505)` 或断线后重拉快照。

详细验收、四属性、日志脱敏和暂缓项见 `docs/myserver/服务端最新登录流程客户端验收.md`。

示例用法：

```rust
use bevy::prelude::*;
use project::network::{HttpRequest, NetworkCommand, NetworkEvent, TcpConnectConfig};

fn send_http(mut commands: MessageWriter<NetworkCommand>) {
    commands.write(NetworkCommand::Http(HttpRequest::get("https://example.com")));
}

fn connect_tcp(mut commands: MessageWriter<NetworkCommand>) {
    commands.write(NetworkCommand::ConnectTcp(TcpConnectConfig::new(
        "127.0.0.1:9000",
    )));
}

fn read_network(mut events: MessageReader<NetworkEvent>) {
    for event in events.read() {
        info!("{event:?}");
    }
}
```

桌面开发验证：

```powershell
Set-Location project
cargo fmt
cargo check
cargo run
```

Android Debug APK 构建流程：

```powershell
rustup target add aarch64-linux-android
cargo install cargo-ndk

Set-Location project
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --release --lib --crate-type cdylib

Set-Location ..\android
.\gradlew.bat assembleDebug
```

如果本机 `JAVA_HOME` 指向 JDK 8，Android Gradle Plugin 8.4.0 会构建失败。需要先把当前终端的 `JAVA_HOME` 临时切到 JDK 17 或更新版本，例如：

```powershell
$env:JAVA_HOME="C:\Program Files\Java\jdk-21"
```

构建完成后，Debug APK 通常在：

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

## 19. 运行离线 UI 生成验收

`tools/ui-generation/` 是独立开发工具，不属于正式游戏 workspace。新成员只需要在修改生成链路或复验 approved 页面时运行它：

```powershell
cargo test --manifest-path tools/ui-generation/Cargo.toml
cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .
cargo run --manifest-path tools/ui-generation/Cargo.toml -- generate-fixture --task tools/ui-generation/fixtures/acceptance/task.valid.json --repository-root . --document-id generated.acceptance_fixture
```

`generate-fixture` 使用仓库自有结构化 fixture，但会真实读取任务中的参考图、校验 hash 和 viewport、运行 Bevy 预览并生成 sealed bundle。任务的 `run_id` 必须唯一；重复运行时使用一份带新 run ID 的任务文件。输出只进入被忽略的 `summary/ui-generation/<run-id>/`，不会自动进入正式游戏。

正式晋升必须单独执行 `promotion-decisions`、`record-promotion-decisions`、`promotion-plan` 和带精确 plan hash 的 `promote`。已晋升验收样例可用正常游戏路由检查：

```powershell
$env:TOUCH_START_SCREEN="ui_generated_acceptance"
cargo run --manifest-path project/Cargo.toml -- --window-profile phone-portrait
```

该离线链路不是在线 AI 验收。仓库当前仍没有在线 provider、OCR 或图片生成适配，不能把 FixtureProvider 的成功结果解释为任意用户参考图已经通过真实模型分析。

## 20. 方圆 Bake 与 Android 验证边界

方圆 Bake 与 Android 验证边界：`project/assets` 会整体进入 APK assets，因此 `.fyb` 只有在明确作为首包发布资源时才放入 `project/assets`；普通 dry-run 产物留在被忽略的 `artifacts/`。当前最小验证顺序是先跑 `.\scripts\run-fangyuan-bake-dry-run.ps1`，再用 `MYBEVY_START_SCENE="dev.fangyuan_home"` 的手机比例窗口确认 RON 首包路径没有被破坏；如需覆盖缓存和后续下载边界，再运行 `cargo test fangyuan_blueprint_cache -- --nocapture`、`cargo test fangyuan_streaming_update -- --nocapture` 和 `cargo test fangyuan_cache_authority -- --nocapture`。完整 Android APK 构建按上面的 `cargo ndk` 和 `gradlew.bat assembleDebug` 命令在需要时单独执行。
