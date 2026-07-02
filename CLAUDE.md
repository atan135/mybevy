# CLAUDE.md

## 项目概况

这个仓库用于开发一个基于 Rust 和 Bevy 的游戏项目。

当前约定：

- 仓库根目录用于放协作文档、说明文件和仓库级配置
- `project/` 是实际的游戏工程根目录
- 当前游戏工程使用 Rust stable 和 `bevy = "0.18.1"`
- 当前玩法是单界面触控/鼠标互动，并通过 authority 帧同步回放 `ui_touch` 输入：按下显示硬边圆形反馈，拖动生成水波纹拖尾，松开后在原地淡出
- 当前内置 `project/src/framework/network/` 网络框架模块，提供 HTTP、TCP 和 KCP 的 Bevy 消息接口
- 当前内置 `project/src/game/authority/` 控制机会话模块，提供本地控制机、局域网控制机和远端 MyServer 控制机的统一命令/事件接口
- MyServer 登录链路采用账号 `player_id` 和玩法 `character_id` 分离：账号 ID 只用于登录、安全和审计；房间、匹配、输入、移动、战斗、背包、transfer 和 authority MyServer endpoint 都以当前 character-bound ticket 绑定的 `character_id` 作为玩法主体
- `android/` 是 Android Gradle 壳工程，用于加载 Rust 产出的 `libproject.so` 并打包 APK

## 目录约定

- `docs/`：项目文档
- `docs/bevy-getting-started.md`：当前 Bevy 入门说明
- `docs/assets-workflow.md`：项目资源使用方式，覆盖首包、APK 包内和后续下载资源
- `docs/fangyuan/`：方圆灵构基础技术文档，覆盖对象资源构建、渲染、Bake、加载、预算和 LOD 等工程路线
- `docs/scene/`：场景框架相关文档，当前总文档规划场景生命周期、资源、切换、流式加载、相机和联机同步
- `docs/ui/`：UI 框架相关文档，描述整体架构、输入实现、组件使用、布局、主题和限制，不记录开发期任务流程
- `scripts/`：仓库级开发辅助脚本
- `project/`：Rust/Bevy 工程根目录
- `project/src/`：游戏源码
- `project/src/framework/`：框架层横向能力入口，当前包含 UI、network、scene、fight 和 fangyuan 边界
- `project/src/framework/fangyuan/`：方圆灵构数据模型入口，包含 blueprint、prefab/palette、scene layout、runtime primitive、对象状态、primitive set 统计，以及审核 report / budget profile / finding / suggestion
- `project/src/framework/network/`：网络通信框架插件和 HTTP/TCP/KCP 接口
- `project/src/framework/scene/`：场景框架插件、命令、事件、生命周期、注册表、首包 RON manifest、Loading、根实体、相机、spawn/anchor、trigger、streaming 元数据和 debug 配置
- `project/src/framework/ui/`：UI 框架能力，包含核心系统、通用控件、覆盖层、主题和国际化
- `project/src/game/`：游戏层插件、页面、玩法和框架适配模块
- `project/src/game/authority/`：本地联机/远端联机的控制机会话接口和轻量 authority 协议
- `project/src/game/screens/`：登录、大厅、玩法 HUD、UI Gallery 等具体页面
- `project/src/game/features/`：Touch Ripple、Fangyuan Player Preview 等具体玩法功能模块
- `project/src/game/scenes/`：具体游戏场景 ID、场景注册适配和场景专属组合逻辑
- `project/src/game/navigation/`：游戏层页面模式、路由命令和路由按钮适配
- `project/src/game/myserver/`：当前游戏的 MyServer 登录、角色列表、选角、character-bound ticket、game proxy 鉴权、房间、四属性和协议适配模块
- `project/assets/`：贴图、音频、字体和其他资源
- `project/assets/fangyuan/palettes/home_prefabs.ron`：方圆家园默认 Prefab / Palette 首包样例
- `project/assets/fangyuan/layouts/home_layout.ron`：方圆家园默认 Scene Layout 首包样例
- `project/Cargo.toml`：Rust 项目配置
- `android/`：Android 打包工程

除非有明确理由，不要把游戏源码放在仓库根目录。

## 开发约定

- 所有 Rust 和 Bevy 相关命令默认在 `project/` 目录执行
- 新增游戏功能时，优先把逻辑放进 `project/src/` 下的模块，而不是持续堆在 `main.rs`
- UI 页面结构放在 `project/src/game/screens/`，具体玩法放在 `project/src/game/features/`，具体游戏场景注册和适配放在 `project/src/game/scenes/`，UI 框架能力放在 `project/src/framework/ui/`
- UI 通用控件放在 `project/src/framework/ui/widgets/`，颜色、字号、间距、圆角等可微调参数集中放在 `project/src/framework/ui/style/theme.rs`
- 新增首包资源文件时，统一放入 `project/assets/`；后续下载资源不要放入 `project/assets/`
- `project/assets/` 下的图片、字体、音频、二进制模型和源工程类资源通过 Git LFS 提交；RON、JSON、TXT、授权说明等文本资源保持普通 Git 提交
- 如果修改了项目结构、初始化方式或 Bevy 版本，同时更新相关文档
- 如果改动影响新成员上手流程，优先同步更新 `docs/bevy-getting-started.md`

## 常用命令

进入项目目录：

```powershell
Set-Location project
```

启动开发版本：

```powershell
cargo run
```

用桌面窗口模拟手机/平板分辨率验收 UI：

```powershell
cargo run -- --window-profile phone-portrait
cargo run -- --window-profile phone-1080p
cargo run -- --window-profile phone-small
cargo run -- --window-profile tablet-portrait
cargo run -- --window-profile tablet-landscape
cargo run -- --window-size 1280x2772
cargo run -- --window-profile phone-portrait --window-scale 50%
cargo run -- --window-size 1280x2772 --device-scale 3.25 --window-scale 50%
```

格式化代码：

```powershell
cargo fmt
```

检查编译：

```powershell
cargo check
```

构建 Android Rust 动态库：

```powershell
Set-Location project
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --release --lib -- --crate-type cdylib
```

打包 Android Debug APK：

```powershell
Set-Location ..\android
.\gradlew.bat assembleDebug
```

如果 `JAVA_HOME` 指向 JDK 8，先在当前终端切到 JDK 17 或更新版本，例如：

```powershell
$env:JAVA_HOME="C:\Program Files\Java\jdk-21"
```

更新依赖后如需锁版本文件：

```powershell
cargo update
```

一键启动两个 Touch Ripple 客户端：

```powershell
.\scripts\start-two-clients.ps1
```

## Bevy 代码风格建议

- 优先使用 `bevy::prelude::*` 引入常用类型
- 先写最小可运行系统，再做模块拆分
- 用组件表达数据，用系统表达行为
- 随着功能增长，尽快把玩法逻辑拆到独立插件和模块
- 避免把无关功能耦合进同一个系统
- 在命名上尽量区分组件、资源、系统和插件的职责

## 文档维护约定

- `summary/` 下的 checklist 完成后，转移并归档到 `docs/<领域>/checklists/` 目录，再纳入 Git 提交。

以下变更应同步检查文档是否需要更新：

- `project/` 目录结构变化
- 初始化命令变化
- Bevy 主版本变化
- 资源目录约定变化
- 新增统一开发流程或脚手架

至少检查这些文件：

- `docs/bevy-getting-started.md`
- `docs/ui/`
- `CLAUDE.md`

## Git 提交规范

### 提交范围

- 一次提交只做一类相关改动
- 不要把无关重构、格式化和功能修改混在同一次提交里
- 如果代码改动依赖文档更新，代码和文档应在同一次提交中完成

### 提交前检查

提交前至少执行：

```powershell
Set-Location project
cargo fmt
cargo check
```

如果这次改动没有涉及 Rust 代码，至少确认变更文件内容和路径正确。

### 提交信息格式

推荐格式：

```text
<type>(<scope>): <summary>
```

如果 scope 不明显，也可以使用：

```text
<type>: <summary>
```

推荐的 `type`：

- `feat`：新功能
- `fix`：缺陷修复
- `docs`：文档修改
- `refactor`：重构
- `chore`：杂项维护
- `test`：测试相关
- `build`：构建、依赖或工具链调整

### 提交信息要求

- `summary` 使用简洁短句，说明这次提交实际做了什么
- 提交信息尽量使用中文，除非英文更准确或项目已有明确的英文约定
- 首字母小写或大写都可以，但仓库内应保持一致
- 不要写成 `update`、`fix bug`、`misc changes` 这类信息量过低的描述
- 尽量把主题控制在一行内

### 提交信息示例

```text
feat(game): add player movement prototype
fix(camera): correct follow offset in 2d scene
docs: update Bevy getting started guide for project directory
build(project): add bevy dependency and dev profile settings
chore: add repository gitignore
```

### 禁止事项

- 不要提交 `target/` 等构建产物
- 不要提交未验证的临时代码作为正式提交
- 不要使用难以理解的提交信息
- 非必要不要提交纯格式化噪音
