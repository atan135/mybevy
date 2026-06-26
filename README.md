# MyBevy

MyBevy 是一个基于 Rust 和 Bevy 的游戏项目仓库。仓库根目录用于协作文档、脚本和平台工程，实际游戏工程位于 `project/`。

当前工程使用 Rust stable、`bevy = "0.18.1"`，同一套 Bevy 代码支持桌面开发运行，并通过 `android/` Gradle 壳工程打包 Android APK。

## 当前能力

- Bevy 游戏工程：桌面入口在 `project/src/main.rs`，共享 App 入口在 `project/src/lib.rs`。
- Touch Ripple：单界面触控/鼠标互动玩法，通过 authority 帧同步回放 `ui_touch` 输入，支持按下圆形反馈、拖动水波纹拖尾和松开淡出。
- Robot Sync：`arena.robot_sync` 3D 场景，用于验证机器人移动的 authority 帧同步、坐标映射、双客户端一致性和 HUD 诊断。
- 场景框架：`project/src/framework/scene/` 提供场景命令、事件、生命周期、首包 RON manifest、Loading、根实体、相机、spawn/anchor、trigger、streaming 元数据和 debug 配置。
- UI 框架：`project/src/framework/ui/` 提供页面模式、面板层级、输入路由、焦点、通用控件、覆盖层、主题和国际化。
- 音频框架：`project/src/framework/audio/` 提供音频 catalog、cue、group、bus、scope、音乐、空间音频、场景音频 adapter、lazy bank 和调试快照。
- 网络框架：`project/src/framework/network/` 提供 HTTP、TCP 和 KCP 的 Bevy 消息接口。
- Authority 会话：`project/src/game/authority/` 提供本地控制机、局域网控制机和远端 MyServer 控制机的统一命令/事件接口。
- Android 打包：`android/` 会加载 Rust 产出的 `libproject.so`，并把 `project/assets` 打包进 APK assets。

## 目录结构

```text
mybevy/
|-- android/                 # Android Gradle 壳工程
|-- docs/                    # 项目文档
|   |-- audio/               # 音频框架说明
|   |-- gameplay/            # 玩法系统设计
|   |-- scene/               # 场景框架说明
|   |-- ui/                  # UI 框架说明
|   `-- 世界观/              # 世界观和长期玩法设定
|-- project/                 # Rust / Bevy 游戏工程根目录
|   |-- assets/              # 首包资源
|   |-- src/
|   |   |-- framework/       # UI、audio、network、scene、fight 等横向能力
|   |   |-- game/            # 游戏层插件、页面、玩法、场景和协议适配
|   |   |-- lib.rs           # 共享 Bevy App 入口
|   |   `-- main.rs          # 桌面入口
|   `-- Cargo.toml
|-- scripts/                 # 仓库级开发脚本
|-- summary/                 # 开发中 checklist，完成后归档到 docs
|-- CLAUDE.md                # 协作和开发约定
`-- README.md
```

## 环境准备

基础开发需要：

- Rust stable
- Git LFS
- Windows PowerShell

首次克隆或换新机器开发时建议执行：

```powershell
git lfs install
rustc --version
cargo --version
```

当前 `project/Cargo.toml` 依赖本地 MyServer 仓库中的 `../../MyServer/packages/authority-core`。如果编译时报找不到该路径，需要先确认同级 `MyServer` 仓库存在，或按你的本地环境调整依赖路径。

Android 打包还需要：

- Android SDK / NDK
- `cargo-ndk`
- JDK 17 或更新版本

## 快速启动

所有 Rust 和 Bevy 命令默认在 `project/` 目录执行：

```powershell
Set-Location project
cargo run
```

格式化和检查：

```powershell
Set-Location project
cargo fmt
cargo check
```

只改文档时，至少确认 diff 没有空白问题：

```powershell
git diff --check
```

## 桌面窗口 Profile

桌面端可以用窗口 profile 模拟移动设备分辨率：

```powershell
Set-Location project
cargo run -- --window-profile phone-portrait
cargo run -- --window-profile phone-1080p
cargo run -- --window-profile phone-small
cargo run -- --window-profile tablet-portrait
cargo run -- --window-profile tablet-landscape
cargo run -- --window-size 1280x2772
cargo run -- --window-profile phone-portrait --window-scale 50%
```

这些参数只影响桌面开发窗口，不改变 Android 真机默认行为。

## 常用开发入口

直接启动样板场景：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="sample.dungeon_room"
cargo run
```

直接启动 Robot Sync 场景：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="arena.robot_sync"
cargo run -- --window-profile phone-small --window-scale 50%
```

Robot Sync 手动输入模式：

```powershell
Set-Location project
$env:MYBEVY_START_SCENE="arena.robot_sync"
$env:ROBOT_SYNC_INPUT_MODE="manual"
cargo run -- --window-profile phone-small --window-scale 50%
```

音频监控和音频测试页：

```powershell
Set-Location project
$env:TOUCH_START_SCREEN="audio_monitor"
cargo run

$env:TOUCH_START_SCREEN="audio_gallery"
cargo run
```

一键启动两个 Touch Ripple 客户端：

```powershell
.\scripts\start-two-clients.ps1
```

一键启动两个 Robot Sync 客户端：

```powershell
.\scripts\start-robot-sync-two-clients.ps1 -DryRun -SkipBuild
.\scripts\start-robot-sync-two-clients.ps1 -Mode lan -SkipBuild
```

## Android 构建

安装 Android target 和 `cargo-ndk`：

```powershell
rustup target add aarch64-linux-android
cargo install cargo-ndk
```

构建 Rust 动态库：

```powershell
Set-Location project
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --release --lib -- --crate-type cdylib
```

打包 Debug APK：

```powershell
Set-Location ..\android
.\gradlew.bat assembleDebug
```

如果 `JAVA_HOME` 指向 JDK 8，先在当前终端切到 JDK 17 或更新版本：

```powershell
$env:JAVA_HOME="C:\Program Files\Java\jdk-21"
```

Debug APK 通常输出到：

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

## 资源约定

首包资源统一放在 `project/assets/`。代码中引用资源时，从 `project/assets/` 下一级开始写：

```text
ui/fonts/MyBevyUiCjk-Regular.otf
audio/ui/click_wood_01.wav
scenes/sample_dungeon_room/scene.ron
```

不要写成：

```text
project/assets/ui/fonts/MyBevyUiCjk-Regular.otf
```

图片、字体、音频、二进制模型和源工程类资源通过 Git LFS 提交；RON、JSON、TXT、授权说明等文本资源保持普通 Git 提交。

后续下载资源不要放入 `project/assets/`。相关设计见 [docs/assets-workflow.md](docs/assets-workflow.md)。

## 文档索引

- [Bevy 入门使用文档](docs/bevy-getting-started.md)
- [资源使用方式](docs/assets-workflow.md)
- [UI 文档总览](docs/ui/README.md)
- [场景功能设计文档](docs/scene/README.md)
- [音频框架说明](docs/audio/README.md)
- [Gameplay 系统文档](docs/gameplay/README.md)
- [世界观文档](docs/世界观/README.md)
- [协作和开发约定](CLAUDE.md)

## 开发约定

- 新增游戏功能优先放入 `project/src/` 下的模块，不持续堆在 `main.rs`。
- UI 页面结构放在 `project/src/game/screens/`。
- 具体玩法放在 `project/src/game/features/`。
- 具体游戏场景注册和适配放在 `project/src/game/scenes/`。
- UI 框架能力放在 `project/src/framework/ui/`。
- UI 通用控件放在 `project/src/framework/ui/widgets/`。
- 颜色、字号、间距、圆角等主题参数集中放在 `project/src/framework/ui/style/theme.rs`。
- 修改项目结构、初始化方式、Bevy 版本、资源目录约定或新成员上手流程时，同步检查相关文档。

## 提交前检查

涉及 Rust 代码时至少执行：

```powershell
Set-Location project
cargo fmt
cargo check
```

只涉及文档或资源时，至少确认变更路径和内容正确：

```powershell
git status --short
git diff --check
```

提交信息建议使用：

```text
<type>(<scope>): <summary>
```

示例：

```text
docs: add project overview readme
feat(scene): add robot sync arena entry
fix(ui): correct lobby route button state
```
