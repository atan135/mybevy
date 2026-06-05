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

## 15. 本项目如何打包成 Windows 和 Android App

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
crate-type = ["cdylib", "rlib"]
```

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
cargo ndk -t arm64-v8a -o ..\android\app\src\main\jniLibs build --release
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
- `project/Cargo.toml`：已经包含 `crate-type = ["rlib", "cdylib"]`
- `android/`：Android Gradle 壳工程，会加载 `libproject.so`

当前 Android 壳工程使用 Bevy 0.18.1 间接依赖的 `android-activity 0.6.1`。
因此 `android/gradle/libs.versions.toml` 里的 `androidx.games:games-activity`
需要保持为 `4.4.0`，并且不要在 Gradle 中启用 `prefab`。否则 Java 侧
`GameActivity` 和 Rust 侧 native glue 的 JNI 方法签名可能不匹配，启动时会出现
`RegisterNatives failed for 'com/google/androidgamesdk/GameActivity'`。

当前玩法是单界面触控/鼠标互动，并已接入控制机帧同步：

1. 本地鼠标左键或手指输入会按帧聚合为 `ui_touch` 输入。
2. 玩法层消费 `AuthorityEvent::FrameApplied`，按玩家回放触控位置。
3. 鼠标左键或手指按下时，在对应位置显示硬边半透明圆形反馈。
4. 按住拖动时，主圆平滑跟随，并沿拖动路径生成水波纹拖尾。
5. 松开后，主圆在原地逐帧淡出；新一次按压会直接在新位置生成。

当前工程已经内置一套网络通信接口：

- `project/src/network/`：网络插件、命令、事件和连接配置
- `NetworkPlugin`：已经在 `project/src/lib.rs` 中注册
- `NetworkCommand`：从 Bevy 系统发起 HTTP 请求、TCP/KCP 连接、TCP/KCP 监听、发送数据、断开连接或停止监听
- `NetworkEvent`：接收 HTTP 响应、连接状态、监听状态、接入连接、数据包、发送结果和错误

HTTP 是一次性请求接口；TCP 和 KCP 是长连接接口，都会返回 `ConnectionId` 对应的连接事件。
网络实际 I/O 在后台 Tokio runtime 中执行，不阻塞 Bevy 主线程。Android 包已经在
`android/app/src/main/AndroidManifest.xml` 中声明了 `android.permission.INTERNET`。

当前工程还内置一套控制机会话接口：

- `project/src/authority/`：控制机统一接口和轻量 authority 协议
- `AuthorityPlugin`：已经在 `project/src/lib.rs` 中注册
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
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs build --release

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
