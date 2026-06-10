# UI 框架执行任务

## 任务目标

一步到位把当前 `AppScreen` 语义重构为 App UI Mode，并建立第一版游戏内 UI 框架骨架。第一版重点解决主流程状态、共存 UI 层级、页面根节点统一管理、基础控件注册、Toast、确认弹窗和 UI 输入拦截。

Rust 代码命名建议使用 `AppUiMode`，而不是 `AppUIMode`。原因是 Rust 类型和枚举变体遵循 UpperCamelCase，连续全大写缩写容易触发风格问题；文档和口头概念仍可称为 App UI Mode。

本任务覆盖 `UI框架自研清单.md` 中的阶段 0，并启动阶段 1 的最小闭环。

## 范围

本轮要做：

- 将当前 `AppScreen` 重命名并重构为 `AppUiMode`。
- 将 `Login`、`GameList`、`TouchRipple` 的语义拆为主流程模式：
  - `AppUiMode::Login`
  - `AppUiMode::Lobby`
  - `AppUiMode::WanfaTouchRipple`
- 新增 `UiFrameworkPlugin`，集中注册 UI 框架相关插件、资源、事件和系统。
- 新增 UI 框架基础模块：
  - `framework.rs`
  - `panel.rs`
  - `layer.rs`
  - `router.rs`
  - `input.rs`
- 为现有登录页、游戏列表页、触控玩法 HUD 建立统一 UI 根节点标记。
- 建立最小 UI 层级：
  - 页面层
  - 弹窗层
  - Toast 层
- 建立 `UiInputState`，替换当前 `ui_touch` 中直接查询 `Button Interaction` 的临时输入拦截方式。
- 实现最小 Toast。
- 实现确认弹窗。
- 保持当前登录、列表、触控水波纹玩法可运行。

本轮不做：

- 不做完整配置化布局。
- 不做 i18n。
- 不做复杂焦点导航。
- 不做完整动画系统。
- 不做虚拟列表。
- 不做可视化编辑器。

## 建议文件改动

### 1. 主流程状态重构

文件：

- `project/src/game/navigation/mod.rs`
- `project/src/game/plugin.rs`
- `project/src/game/screens/**/*.rs`

任务：

- 将 `AppScreen` 改为 `AppUiMode`。
- 将枚举值调整为：

```rust
pub(super) enum AppUiMode {
    #[default]
    Login,
    Lobby,
    WanfaTouchRipple,
}
```

- 更新所有 `OnEnter(AppScreen::...)`、`OnExit(AppScreen::...)`、`in_state(AppScreen::...)`、`DespawnOnExit(AppScreen::...)`。
- 将 `TOUCH_START_SCREEN` 的解析语义同步改为 mode：
  - `login` -> `AppUiMode::Login`
  - `lobby` / `game_list` / `game-list` / `list` -> `AppUiMode::Lobby`
  - `wanfa_touch_ripple` / `wanfa-touch-ripple` / `touch` / `touch_ripple` / `touch-ripple` -> `AppUiMode::WanfaTouchRipple`

验收：

- `cargo check` 通过。
- 登录页、游戏列表页、触控水波纹模式仍可进入。
- 代码中不再存在作为类型名使用的 `AppScreen`。

### 2. UI 框架入口

文件：

- `project/src/game/ui/mod.rs`
- `project/src/game/ui/framework.rs`

任务：

- 新增 `UiFrameworkPlugin`。
- 由 `UiFrameworkPlugin` 统一注册：
  - `UiThemePlugin`
  - `UiWidgetsPlugin`
  - `UiPanelPlugin`
  - `UiLayerPlugin`
  - `UiRouterPlugin`
  - `UiInputPlugin`
- `ScreensPlugin` 不再直接注册 `UiThemePlugin` 和 `UiWidgetsPlugin`，而是注册 `UiFrameworkPlugin`。

验收：

- UI 相关插件入口集中。
- 后续新增 UI 框架能力只需要挂到 `UiFrameworkPlugin`。

### 3. UI Panel 和根节点

文件：

- `project/src/game/ui/core/panel.rs`
- `project/src/game/screens/auth/login.rs`
- `project/src/game/screens/lobby/game_list.rs`
- 后续可能涉及 `project/src/game/screens/gameplay/*`

建议抽象：

```rust
pub(super) enum UiPanelId {
    LoginPage,
    GameListPage,
    TouchRippleHud,
}

#[derive(Component)]
pub(super) struct UiPanelRoot {
    pub id: UiPanelId,
    pub kind: UiPanelKind,
    pub owner_mode: Option<AppUiMode>,
}
```

任务：

- 页面和 HUD 根节点统一添加 `UiPanelRoot`。
- 登录页根节点使用 `UiPanelId::LoginPage`。
- 游戏列表页根节点使用 `UiPanelId::GameListPage`。
- `AppUiMode::WanfaTouchRipple` 进入后生成一个最小 `UiPanelId::TouchRippleHud` 根节点。第一版可以只作为 HUD 容器，也可以放基础返回按钮。

验收：

- 能通过查询 `UiPanelRoot` 找到当前存在的 UI 页面或 HUD 根节点。
- mode 退出后不会留下归属该 mode 的孤儿 UI 根节点。

### 4. UI 层级

文件：

- `project/src/game/ui/layer.rs`

建议抽象：

```rust
pub(super) enum UiLayer {
    Page,
    Modal,
    Toast,
}

#[derive(Component)]
pub(super) struct UiLayerRoot {
    pub layer: UiLayer,
}
```

任务：

- 建立层级根节点或层级标记。
- 页面根节点归入 `UiLayer::Page`。
- 预留 `UiLayer::Modal` 和 `UiLayer::Toast`。
- 第一版可以先不实现复杂 z-order，只保证概念和组件存在。

验收：

- 可以区分页面层、弹窗层和 Toast 层。
- 后续弹窗和 Toast 能挂到对应层。

### 5. UI 路由命令

文件：

- `project/src/game/ui/router.rs`
- `project/src/game/navigation/mod.rs`
- `project/src/game/ui/widgets.rs`

建议抽象：

```rust
pub(super) enum UiRouteCommand {
    ChangeMode(AppUiMode),
    OpenModal(UiModalId),
    CloseModal,
    ShowToast(UiToast),
}
```

任务：

- 实现 `ChangeMode(AppUiMode)`。
- `RouteButton` 点击后不直接写 `NextState<AppUiMode>`，而是发 `UiRouteCommand::ChangeMode`。
- `UiRouterPlugin` 消费命令并设置 `NextState<AppUiMode>`。
- 实现 `OpenModal`、`CloseModal`、`ShowToast` 的最小可用流程。

验收：

- 现有按钮跳转行为不变。
- 路由入口从页面控件中解耦出来。

### 6. UI 输入拦截

文件：

- `project/src/game/ui/input.rs`
- `project/src/game/plugin.rs`

建议抽象：

```rust
#[derive(Resource, Default)]
pub(super) struct UiInputState {
    pub pointer_blocked: bool,
}
```

任务：

- 新增系统根据当前 UI 交互状态更新 `UiInputState.pointer_blocked`。
- `ui_touch` 的 `capture_local_touch_input` 不再直接查询所有 `Button Interaction`。
- `capture_local_touch_input` 改为读取 `Res<UiInputState>`。
- 如果 `pointer_blocked == true`，玩法触控输入不采集。

第一版判断规则：

- 任意 `Button` 处于 `Pressed` 或 `Hovered` 时，视为 UI 占用 pointer。
- 后续有弹窗遮罩后，弹窗遮罩也应设置 pointer blocked。

验收：

- 点击登录页、游戏列表页按钮不会触发玩法触控输入。
- 进入玩法模式后，未命中 UI 的鼠标/触控仍能生成水波纹。
- 输入拦截逻辑集中在 `ui/input.rs`。

### 7. 最小弹窗和 Toast 实现

文件：

- `project/src/game/ui/layer.rs`
- `project/src/game/ui/router.rs`
- 可选新增 `project/src/game/ui/overlay.rs`

任务：

- 定义 `UiModalId`、`UiToast`、`UiToastRequest` 等基础类型。
- 实现一个最小 Toast：
  - 文本
  - 自动消失
  - 挂在 Toast 层
- 实现一个最小确认弹窗：
  - 半透明遮罩
  - 标题和正文
  - 确认按钮
  - 取消按钮
  - 点击确认或取消后关闭弹窗并发出结果事件
- 确认弹窗打开时阻塞下层输入。

验收：

- 可以通过 `UiRouteCommand::ShowToast` 显示并自动关闭 Toast。
- 可以通过 `UiRouteCommand::OpenModal` 打开确认弹窗。
- 弹窗打开时下层按钮和触控玩法不响应 pointer 输入。
- 不影响当前页面跳转和触控玩法行为。

## 执行顺序

1. 重构 `AppScreen` -> `AppUiMode`，确保功能不变。
2. 新增 `UiFrameworkPlugin`，集中注册现有 UI 插件。
3. 新增 `panel.rs`，给现有页面和 HUD 加 `UiPanelRoot`。
4. 新增 `layer.rs`，定义页面层、弹窗层、Toast 层。
5. 新增 `router.rs`，让按钮通过 `UiRouteCommand` 切换 `AppUiMode`。
6. 新增 `input.rs`，把玩法触控输入拦截改为读取 `UiInputState`。
7. 实现最小 Toast。
8. 实现最小确认弹窗。
9. 跑 `cargo fmt` 和 `cargo check`。
10. 手动运行 `cargo run`，检查登录、列表、玩法触控、Toast、确认弹窗路径。

## 验收清单

- [x] `project/src/game/navigation/mod.rs` 中类型名已改为 `AppUiMode`。
- [x] `project/src/game/plugin.rs` 中玩法系统使用 `AppUiMode::WanfaTouchRipple`。
- [ ] 登录页进入大厅可用；仍需人工窗口点击验证。
- [ ] 大厅进入触控玩法可用；仍需人工窗口点击验证。
- [ ] 触控玩法中鼠标/触控水波纹仍可用；后文仅记录 `TOUCH_START_SCREEN=touch` 启动后稳定运行 5 秒，实际点击 / 拖动水波纹视觉仍需人工窗口验证。
- [ ] UI 按钮输入不会被玩法触控重复消费；后文已记录 `UiInputState` 和 HUD 输入拦截接入，但 Touch Ripple 场景中的实际 HUD 点击拦截仍需人工窗口验证。
- [x] 页面根节点带有 `UiPanelRoot`。
- [x] 触控玩法模式有 `UiPanelId::TouchRippleHud` 根节点。
- [x] Toast 可以显示并自动消失。
- [x] 确认弹窗可以打开、关闭并阻塞下层输入。
- [x] UI 框架入口集中在 `UiFrameworkPlugin`。
- [x] `cargo fmt` 通过。
- [x] `cargo check` 通过。

## 已确认决策

1. 命名使用 App UI Mode 概念。

代码类型使用 `AppUiMode`。不使用 `AppUIMode` 是为了符合 Rust 命名惯例。

2. 触控水波纹使用专用 mode。

使用 `AppUiMode::WanfaTouchRipple`。不归入泛化的 `Gameplay`。

3. 第一版实现可见 Toast。

Toast 需要能显示文本、挂到 Toast 层并自动消失。

4. 第一版实现确认弹窗。

确认弹窗需要遮罩、确认/取消按钮、结果事件和输入阻塞。

5. 关于 `UiPanelId::TouchRippleHud`。

这个问题的意思是：进入触控水波纹模式时，是否创建一个属于玩法 HUD 的 UI 根节点。它不等于全屏页面，也不一定要有可见内容；它只是给暂停按钮、网络状态条、调试入口等玩法内 UI 预留挂载位置。

本轮建议生成一个最小 `TouchRippleHud` 根节点，先不放实际控件。这样页面层结构完整，后续添加 HUD 控件不需要再改生命周期结构。

## 后续任务入口

本任务完成后，下一轮建议做：

- Loading 遮罩。
- `UiGallery` 示例页面。
- 更完整的 `UiInputState` 命中和遮罩阻塞规则。

## P0-P3 模块化执行队列

串行规则：每个任务由单独 subagent 执行，主 agent 在验证通过后再 git 提交并启动下一项。subagent 只编辑任务列出的模块 / 文件范围；如果发现需要扩大范围，应先回报主 agent。

### P0：收敛当前状态和测试兜底

1. P0-01 文档验收状态整理
   - 优先级：P0。
   - 模块 / 文件范围：`docs/UI框架执行任务.md`。
   - 目标：对齐早期验收清单和后文完成记录；保留仍需人工窗口或 Android 真机验证的未完成项和原因；维护本执行队列。
   - 验证命令或验证方式：通读本文档，确认状态描述不与后文记录冲突。
   - 建议提交类型：`docs`。

2. P0-02 i18n 文案覆盖补齐
   - 优先级：P0。
   - 模块 / 文件范围：`project/assets/ui/i18n/*.ron`、`project/src/game/ui/i18n.rs`、`project/src/game/screens/**/*.rs`、`project/src/game/ui/overlays/**/*.rs`、`project/src/game/ui/widgets/**/*.rs`。
   - 目标：审计仍使用硬编码展示文本的 UI 节点，补齐 Login、Lobby、UiGallery、Touch Ripple HUD、Toast、Loading、Confirm、Floating Panel 和通用控件的静态文案 key；动态用户输入或运行时数据不强行纳入静态 key。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；分别用 `MYBEVY_UI_LOCALE=zh_cn` 和 `MYBEVY_UI_LOCALE=en_us` 启动窗口，抽查主要页面文案。
   - 建议提交类型：`feat(ui)`。

3. P0-03 配置和 i18n 回归测试
   - 优先级：P0。
   - 模块 / 文件范围：`project/src/game/ui/style/theme.rs`、`project/src/game/ui/i18n.rs`、`project/assets/ui/themes/default.ron`、`project/assets/ui/i18n/*.ron`。
   - 目标：为主题配置版本、缺失文件、坏 RON、locale 归一化、缺失 key fallback 和环境变量路径选择补测试或可重复验证脚本；不要改变已确认的兜底语义。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo test`、`cargo check`。
   - 建议提交类型：`test(ui)`。

### P1：运行时刷新闭环

1. P1-01 运行时 i18n 刷新
   - 优先级：P1。
   - 模块 / 文件范围：`project/src/game/ui/i18n.rs`、`project/src/game/ui/core/framework.rs`、`project/src/game/screens/**/*.rs`、`project/src/game/ui/overlays/**/*.rs`。
   - 目标：实现语言运行时切换或 i18n 文件热加载后刷新已生成 `UiI18nText` 文本节点；覆盖 Toast、Loading、Confirm、Floating Panel 等当前记录中尚未覆盖的运行时文本。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；窗口中切换语言或修改 i18n 文件，确认 Login、Lobby、UiGallery 和 overlay 文案刷新。
   - 建议提交类型：`feat(ui)`。

2. P1-02 主题运行时刷新补全
   - 优先级：P1。
   - 模块 / 文件范围：`project/src/game/ui/style/theme.rs`、`project/src/game/ui/widgets/**/*.rs`、`project/src/game/ui/overlays/**/*.rs`、`project/src/game/ui/core/panel.rs`、`project/src/game/ui/debug.rs`、`project/src/game/screens/**/*.rs`、`project/assets/ui/themes/default.ron`。
   - 目标：补全已生成 UI 节点的主题热刷新，覆盖文本字号、按钮 / 文本输入尺寸、面板 padding / border / radius、页面和 overlay root padding、Loading / Confirm 遮罩颜色；只刷新有明确 marker 或 role 的节点，避免全量重建页面。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo test`、`cargo check`；运行 UiGallery 时修改主题 RON，确认按钮、输入框、面板、Toast、Loading、Confirm 的可见样式刷新。
   - 建议提交类型：`feat(ui)`。

3. P1-03 刷新路径测试
   - 优先级：P1。
   - 模块 / 文件范围：`project/src/game/ui/i18n.rs`、`project/src/game/ui/style/theme.rs`、相关测试模块或脚本。
   - 目标：为 i18n 和主题运行时刷新添加最小自动化测试，覆盖成功刷新、坏配置保留当前有效值、缺失 key fallback 和已生成文本节点更新。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo test`、`cargo check`。
   - 建议提交类型：`test(ui)`。

### P2：文本输入和控件库

1. P2-01 文本输入编辑能力
   - 优先级：P2。
   - 模块 / 文件范围：`project/src/game/ui/widgets/controls.rs`、`project/src/game/ui/core/focus.rs`、`project/src/game/screens/dev/ui_gallery.rs`。
   - 目标：在现有文本输入第一版基础上增加光标、左右移动、Home / End、Delete、选区、复制粘贴、长度限制和 readonly / disabled 语义；IME 组合态如果无法一次完成，应明确保留限制记录。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；在 UiGallery 手动验证键盘编辑、Tab 焦点、提交和禁用态。
   - 建议提交类型：`feat(ui)`。

2. P2-02 表单状态和校验
   - 优先级：P2。
   - 模块 / 文件范围：`project/src/game/ui/widgets/controls.rs`、`project/src/game/ui/style/theme.rs`、`project/src/game/screens/dev/ui_gallery.rs`。
   - 目标：为文本输入和后续表单控件建立 error、helper text、required、validation message 等状态表达；UiGallery 提供可见样例。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；窗口中验证错误态、禁用态和焦点态不会互相覆盖。
   - 建议提交类型：`feat(ui)`。

3. P2-03 通用控件库扩展
   - 优先级：P2。
   - 模块 / 文件范围：`project/src/game/ui/widgets/**/*.rs`、`project/src/game/ui/style/theme.rs`、`project/src/game/screens/dev/ui_gallery.rs`。
   - 目标：按现有 builder 风格补齐 checkbox、toggle、segmented control、slider / stepper 和 icon button 等常用控件；控件应接入焦点、disabled / loading 或等价状态、主题和 i18n 文案 helper。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；UiGallery 手动验证鼠标、键盘焦点和视觉状态。
   - 建议提交类型：`feat(ui)`。

4. P2-04 控件交互测试
   - 优先级：P2。
   - 模块 / 文件范围：`project/src/game/ui/widgets/**/*.rs`、`project/src/game/ui/core/focus.rs`、相关测试模块或脚本。
   - 目标：为按钮、文本输入、滚动、焦点和新增控件增加轻量交互测试或可重复验证脚本，降低后续主题 / i18n 刷新改动的回归风险。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo test`、`cargo check`。
   - 建议提交类型：`test(ui)`。

### P3：体验、诊断和平台验证

1. P3-01 UI 动画基础
   - 优先级：P3。
   - 模块 / 文件范围：可新增 `project/src/game/ui/core/animation.rs`，并涉及 `project/src/game/ui/overlays/**/*.rs`、`project/src/game/ui/style/theme.rs`。
   - 目标：建立轻量动画组件 / 系统，优先覆盖 Toast 淡入淡出、Confirm / Loading 出入场和按钮状态过渡；不引入复杂时间线编辑器。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；窗口中验证动画不影响输入阻塞和 `CloseTop` 顺序。
   - 建议提交类型：`feat(ui)`。

2. P3-02 数据绑定基础
   - 优先级：P3。
   - 模块 / 文件范围：可新增 `project/src/game/ui/core/binding.rs`，并涉及 `project/src/game/ui/core/framework.rs`、`project/src/game/ui/widgets/**/*.rs`、`project/src/game/screens/**/*.rs`。
   - 目标：提供文本、可见性、disabled 状态和简单数值显示的绑定机制，减少页面系统手动刷新节点；先覆盖 UiGallery 示例和一个真实页面。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；窗口中验证绑定值变化后 UI 正确刷新且不重建无关节点。
   - 建议提交类型：`feat(ui)`。

3. P3-03 UI 性能巡检
   - 优先级：P3。
   - 模块 / 文件范围：`project/src/game/ui/**/*.rs`、`project/src/game/screens/dev/ui_gallery.rs`。
   - 目标：检查高频系统、滚动、主题 / i18n 刷新和调试面板的 entity 查询成本；必要时增加条件运行、dirty marker 或统计日志。不要把无关重构混入本任务。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；运行 UiGallery 和 Touch Ripple，观察帧率 / 日志并记录验证结论。
   - 建议提交类型：`perf(ui)`。

4. P3-04 输入路由调试增强
   - 优先级：P3。
   - 模块 / 文件范围：`project/src/game/ui/debug.rs`、`project/src/game/ui/core/input.rs`、`project/src/game/ui/core/focus.rs`、`project/src/game/ui/core/panel.rs`。
   - 目标：在现有 F3 调试面板基础上增加冻结刷新、过滤、复制当前状态、输入事件历史和可选 panel 高亮；保持面板不阻塞 pointer 输入、不进入 Panel Manager。
   - 验证命令或验证方式：在 `project/` 运行 `cargo fmt --check`、`cargo check`；窗口中用 F3 验证调试面板和下层 UI 交互互不干扰。
   - 建议提交类型：`feat(ui)`。

5. P3-05 Android 真机验证和修复
   - 优先级：P3。
   - 模块 / 文件范围：`project/src/game/ui/**/*.rs`、`project/src/game/screens/**/*.rs`、`android/`；如果只记录验证结果，则范围为本文档。
   - 目标：在 Android 构建环境和真机上验证 Back、触控点击 / 拖动、字体、Toast、Loading、Confirm、文本输入和滚动；仅在发现平台问题时做针对性修复。
   - 验证命令或验证方式：在 `project/` 运行 `cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs build --release`；在 `android/` 运行 `.\gradlew.bat assembleDebug`；安装到真机后人工验证并记录结果。
   - 建议提交类型：`test(android)` 或 `fix(ui)`。

## 下一阶段任务：Panel Manager

### 目标

在 `AppUiMode` 之下建立真正的界面 panel 管理机制。`AppUiMode` 只表示主流程；当前 mode 下实际存在的登录页、游戏列表页、玩法 HUD、设置面板、暂停菜单、Loading 遮罩和确认弹窗，由 Panel Manager 统一管理。

本阶段已确认：

1. 用 `UiPanelId`、`UiPanelKind`、`UiPanelRoot` 替换 `UiScreenId`、`UiScreenRoot`。
2. Toast 不纳入 Panel Manager，继续作为专用通知系统，挂在 Toast 层并自动消失。
3. Loading 纳入 Panel Manager，作为 `BlockingOverlay` 处理输入阻塞和生命周期。
4. Confirm modal 迁入 Panel Manager；弹窗内容数据结构可以继续保留在 modal 模块中。

### 建议文件改动

新增或调整：

- `project/src/game/ui/core/panel.rs`
- `project/src/game/ui/core/mod.rs`
- `project/src/game/ui/core/framework.rs`
- `project/src/game/ui/core/input.rs`
- `project/src/game/ui/overlays/router.rs`
- `project/src/game/ui/overlays/loading.rs`
- `project/src/game/ui/overlays/modal.rs`
- `project/src/game/screens/**/*.rs`

如果迁移后 `screen.rs` 不再有独立价值，应删除或改名为 `panel.rs`，避免 `Screen` 和 `Panel` 两套概念长期并存。

### 建议抽象

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiPanelId {
    LoginPage,
    GameListPage,
    UiGalleryPage,
    GalleryFloating,
    TouchRippleHud,
    TouchRipplePause,
    TouchRippleSettings,
    GlobalLoading,
    ConfirmModal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiPanelKind {
    Page,
    Hud,
    Floating,
    Modal,
    BlockingOverlay,
}

#[derive(Component)]
pub(in crate::game) struct UiPanelRoot {
    pub id: UiPanelId,
    pub kind: UiPanelKind,
    pub owner_mode: Option<AppUiMode>,
}

#[derive(Message)]
pub(in crate::game) enum UiPanelCommand {
    Open(UiPanelRequest),
    Close(UiPanelId),
    Toggle(UiPanelRequest),
    Hide(UiPanelId),
    Show(UiPanelId),
    CloseTop,
    CloseAllForMode(AppUiMode),
}
```

`UiPanelRequest` 用于承载打开 panel 所需的数据。第一版可以先支持：

- `UiPanelRequest::Loading(UiLoading)`
- `UiPanelRequest::Confirm(UiConfirmModal)`
- `UiPanelRequest::Floating(UiFloatingPanel)`

页面类 panel 仍由 `OnEnter(AppUiMode)` 创建也可以接受，但根节点必须统一标记为 `UiPanelRoot`。后续再决定是否把页面类 panel 也完全改成命令式打开。

### 行为规则

- `Page` / `Hud`：通常随 `AppUiMode` 进入而创建，随 mode 退出清理。
- `Floating`：可以多个共存，参与 `CloseTop`，不阻塞全局 pointer 输入。
- `Modal`：使用栈结构，打开时阻塞下层 UI 和玩法输入。
- `BlockingOverlay`：通常单例，打开时阻塞下层 UI 和玩法输入；Loading 属于这一类。默认不可被返回键取消，只有显式标记为可取消时才响应 `CloseTop`。
- `Toast`：不参与 Panel Manager，不进入返回栈，不阻塞输入。

返回键 / Esc 的优先级：

1. 如果存在 `BlockingOverlay`，先处理最高层的阻塞遮罩：可取消则关闭，不可取消则忽略返回，不继续关闭下面的弹窗或浮动面板。
2. 如果存在 `Modal`，关闭最上层 modal。
3. 如果存在 `Floating`，关闭最上层 floating panel。
4. 都没有时，交给 mode 级返回逻辑，例如玩法返回 Lobby。

当前第一版已接入桌面 `Esc` 和 Android Back，行为等价于发送 `UiPanelCommand::CloseTop`。Android Back 在 Bevy 0.18 / winit 0.30 中按逻辑键 `Key::BrowserBack` 处理。

Loading 的取消规则：

- `UiLoading::new(text)` 默认不可取消，只能通过显式 `UiPanelCommand::Close(UiPanelId::GlobalLoading)` 或 mode 清理关闭。
- 需要允许玩家返回取消的加载流程，使用 `UiLoading::new(text).cancellable()`。
- 可取消只影响 `CloseTop` 行为，不影响遮罩本身的输入阻塞；Loading 打开期间下层 UI 和玩法输入仍被阻塞。

### 输入拦截

`UiInputState` 应扩展为：

```rust
#[derive(Resource, Default)]
pub(in crate::game) struct UiInputState {
    pub pointer_blocked: bool,
    pub focused_panel: Option<UiPanelId>,
    pub top_blocking_panel: Option<UiPanelId>,
}
```

更新规则：

- 任意按钮 hover / pressed 时，`pointer_blocked = true`。
- 存在 `Modal` 或 `BlockingOverlay` 时，`pointer_blocked = true`。
- `top_blocking_panel` 指向当前最高优先级阻塞 panel。
- 玩法输入只读取 `UiInputState`，不直接扫描 UI 节点。

### 执行顺序

1. 新增 `panel.rs`，定义 `UiPanelId`、`UiPanelKind`、`UiPanelRoot`、`UiPanelCommand` 和基础状态资源。
2. 将 `UiScreenId`、`UiScreenRoot` 替换为 `UiPanelId`、`UiPanelRoot`。
3. 将 `UiScreenPlugin` 替换为 `UiPanelPlugin`，并挂入 `UiFrameworkPlugin`。
4. 迁移登录页、游戏列表页、玩法 HUD、UiGallery 页的根节点标记。
5. 将 Loading 从专用 overlay 命令迁入 `UiPanelCommand::Open(UiPanelRequest::Loading(...))`。
6. 将 Confirm modal 从 `UiRouteCommand::OpenModal` 迁入 `UiPanelCommand::Open(UiPanelRequest::Confirm(...))`。
7. 保留 Toast 的 `UiRouteCommand::ShowToast` 或改成独立 `UiToastCommand`，但不纳入 Panel Manager。
8. 扩展 `UiInputState`，由 Panel Manager 提供当前最高阻塞 panel 信息。
9. 实现 `CloseTop`，并接入桌面 `Esc` 和 Android Back。
10. 在 `UiGallery` 增加 `GalleryFloating` 示例 panel，用 `Show Floating`、`Close Top`、`Esc` 和 Android Back 验证 floating 栈行为。
11. 跑 `cargo fmt`、`cargo check`，并手动验证 Login、Lobby、UiGallery、Touch Ripple、Toast、Loading、Confirm。

### 验收清单

- [x] 代码中不再使用 `UiScreenId` 和 `UiScreenRoot`。
- [x] 页面、HUD、Loading、Confirm 的根节点统一使用 `UiPanelRoot`。
- [x] Loading 通过 Panel Manager 打开和关闭，并阻塞下层输入。
- [x] Confirm modal 通过 Panel Manager 打开和关闭，并发出结果事件。
- [x] Toast 仍能显示并自动消失，且不进入 panel 栈。
- [x] `UiInputState.top_blocking_panel` 能反映当前阻塞输入的 panel。
- [x] `CloseTop` 能按层级优先处理 `BlockingOverlay`、`Modal` 和 `Floating` panel。
- [x] `BlockingOverlay` 支持可取消 / 不可取消规则；不可取消 Loading 会消费返回但不关闭下层 panel。
- [x] 桌面 `Esc` 已接入 `CloseTop`。
- [x] Android Back 已按 `Key::BrowserBack` 接入 `CloseTop`。
- [x] `UiGallery` 有 `GalleryFloating` 示例 panel，可用 `Show Floating`、`Close Top`、`Esc` 和 Android Back 验证。
- [x] 通用按钮支持 `disabled` 视觉状态，带 `DisabledButton` 的按钮不会触发路由、弹窗和页面 action。
- [x] 通用按钮支持 `focused`、`selected`、`loading` 视觉状态；带 `LoadingButton` 的按钮不会触发路由、弹窗和页面 action。
- [x] `UiGallery` 有禁用按钮样例，可验证 disabled 状态。
- [x] `UiGallery` 有 Focused、Selected、Loading 按钮样例，以及不可取消 / 可取消 Loading 遮罩样例。
- [x] mode 切换后不会留下旧 mode 的 panel 节点。
- [x] `cargo fmt` 通过。
- [x] `cargo check` 通过。

### 本轮验证记录

- 已跑通 1：桌面 `Esc` 和 Android Back 会写入 `UiPanelCommand::CloseTop`，`CloseTop` 优先关闭最上层 `Modal`，没有 modal 时关闭最上层 `Floating`。
- 已跑通 2：`UiGallery` 增加 `Show Floating` 和 `Close Top`，可打开 `UiPanelId::GalleryFloating` 示例 panel，并通过 `CloseTop`/`Esc` 关闭。
- 已测试 3：`cargo fmt --check`、`cargo check`、`cargo build` 通过；以 `TOUCH_START_SCREEN=touch` 启动 `target/debug/project.exe` 后稳定运行 5 秒，确认 Touch Ripple 启动路径没有回归性崩溃。
- 仍需人工窗口验证：在 Touch Ripple 场景中实际点击/拖动，确认水波纹视觉和 HUD 按钮输入拦截符合预期。

### 按钮状态小闭环

- 已新增 `DisabledButton` 标记和禁用按钮构建函数。
- 已新增 `FocusedButton`、`SelectedButton`、`LoadingButton` 标记，按钮视觉状态覆盖 `normal / hovered / pressed / focused / selected / disabled / loading`。
- 通用按钮主题新增 `focused`、`selected`、`disabled`、`loading` 色值。
- 视觉优先级为：`disabled > loading > pressed > hovered > selected > focused > normal`。
- 路由按钮、弹窗按钮、Lobby Play 按钮和 `UiGallery` action 按钮都会跳过 `DisabledButton` 和 `LoadingButton`。
- `UiGallery` 的 Buttons 区域已增加 `Focused`、`Selected`、`Loading`、`Disabled` 和 `Unavailable` 样例。

### 焦点系统第一版小闭环

- 已新增 `UiFocusPlugin` 和 `UiFocusState`，并挂入 `UiFrameworkPlugin`。
- 通用按钮构建函数会添加 `FocusableButton`，用于区分真实按钮和遮罩根节点等阻塞用 `Button`。
- `Tab` / `Shift+Tab` 会在当前可聚焦按钮中循环移动焦点。
- 焦点移动时会自动给当前按钮添加 `FocusedButton`，并移除其他按钮上的 `FocusedButton`。
- `Enter` / `Space` 会把当前聚焦按钮的 `Interaction` 写为 `Pressed`，现有基于 `Changed<Interaction> + Interaction::Pressed` 的路由按钮、弹窗按钮、Lobby Play 按钮和 `UiGallery` action 按钮可直接响应。
- 带 `DisabledButton` 或 `LoadingButton` 的按钮不会获得焦点，也不会被键盘触发。
- 存在 `Modal` 或 `BlockingOverlay` 时，焦点候选限制在最高层阻塞 panel 内；否则候选优先取当前最高层可聚焦 panel。
- `Esc` / Android Back 的 `CloseTop` 路径不依赖焦点系统，仍由 Panel Manager 处理。

验证方式：

1. 启动 `TOUCH_START_SCREEN=ui-gallery target/debug/project.exe`。
2. 在 UiGallery 按 `Tab`，确认按钮焦点高亮按稳定顺序移动；按 `Shift+Tab` 确认反向移动。
3. 焦点停在 `Show Toast`、`Show Floating`、`Show Confirm` 或 `Close Top` 时，按 `Enter` / `Space` 确认行为等价于鼠标点击。
4. 打开 Confirm 后继续按 `Tab`，确认焦点优先在弹窗按钮间循环；按 `Esc` 确认仍能关闭顶层弹窗。

当前限制：

- 第一版焦点顺序按 `Entity` 稳定顺序遍历，不做空间导航、声明式 tab index 或布局顺序计算。
- 第一版只覆盖带 `FocusableButton` 的通用按钮；后续自定义控件需要显式接入 marker 或扩展统一控件接口。
- 键盘激活通过一帧 `Interaction::Pressed` 兼容现有点击系统，不区分按下/松开完整语义。

### 布局组件小闭环

- 已新增通用布局 builder：`ui_column`、`ui_row`、`ui_wrap_row`、`ui_grid`。
- `ui_column` 用于纵向堆叠内容。
- `ui_row` 用于单行横向排列。
- `ui_wrap_row` 用于轻量横向换行排列。
- `ui_grid` 基于 Bevy UI Grid，用于固定列数网格布局，适合按钮组、卡片组、表格雏形。
- `UiGallery` 的 Buttons 和 Overlays 区域已改为 `ui_grid(theme, 4)`，避免按钮换行后面板高度没有正确撑开导致重叠。

### ScrollView 小闭环

- 已新增框架级 `UiScrollView` 和 `ui_scroll_column`。
- `UiScrollView` 基于 Bevy UI 的 `Overflow::scroll_y()` 与 `ScrollPosition`，由 `UiScrollPlugin` 统一处理滚动输入。
- 桌面鼠标滚轮会根据 hover 到的 UI 节点向上冒泡，优先滚动最近的 `UiScrollView`。
- 支持按住 Ctrl 后将滚轮方向切换为横向滚动的基础逻辑，供后续横向 ScrollView 复用。
- 支持 pointer drag 更新 `ScrollPosition`，作为触控拖动滚动的第一版机制。
- `UiInputState.pointer_blocked` 会识别 hover 中的 `UiScrollView`，避免滚动 UI 时玩法触控同时采集。
- `UiGallery` 已改为固定 header + 可滚动 body，正文内容放入 `ui_scroll_column`。

### BlockingOverlay 可取消规则记录

- Loading 作为 `UiPanelKind::BlockingOverlay` 进入 Panel Manager 栈。
- `UiLoading::new(text)` 默认不可取消；`UiLoading::new(text).cancellable()` 明确表示可被 `CloseTop` 关闭。
- `CloseTop` 当前优先处理 `BlockingOverlay`，再处理 `Modal`，最后处理 `Floating`。
- 如果当前存在不可取消 `BlockingOverlay`，返回键 / Esc / Android Back 不会关闭它，也不会绕过它关闭下面的弹窗或浮动面板。
- `UiGallery` 的 Overlays 区域已增加 `Loading` 和 `Cancelable` 两种 Loading 示例，用于验证显式关闭和返回取消两种路径。

### 阶段 2 小入口：主题配置加载

- 已新增 `project/assets/ui/themes/default.ron`，用 RON 保存第一版主题 token。
- `UiThemePlugin` 启动时会优先读取 `MYBEVY_UI_THEME` 指定文件，其次读取默认主题路径。
- 主题配置字段包含 `version`；当前支持版本为 `1`。
- 配置读取失败、缺失或版本不匹配时，会保留内置 `UiTheme::default()` 并输出诊断日志。
- 已新增主题热加载第一版：运行中定时轮询当前成功加载的主题文件 modified 时间；如果启动期没有成功加载配置，则优先轮询 `MYBEVY_UI_THEME` 指定路径，否则轮询默认主题路径。
- 热加载解析成功后会替换 `UiTheme` 资源，并刷新已标记节点的通用按钮背景、页面背景、面板背景、面板边框、主文本色和弱化文本色。
- 已补全运行时刷新 marker：`UiThemeTextStyleRole` 覆盖 `title_large / title / subtitle / section_label / body / caption / button` 字号；`UiThemeButtonNodeRole` 覆盖通用按钮和文本输入的 `min_width / height / min_height / padding_x / radius`；`UiThemePanelNodeRole` 覆盖标准面板、内容面板、Toast、Loading 和调试面板的 `padding / border / radius`；`UiThemeRootNodeRole` 覆盖页面根、HUD overlay、blocking overlay、Toast root、Floating Panel 和调试面板的 `screen_padding / overlay_padding` 相关字段。
- 已新增遮罩语义色 `loading_overlay_background` 和 `modal_overlay_background`，替换 Loading / Confirm 根节点硬编码半透明背景；内置默认主题和 `default.ron` 保持一致。
- 热加载解析失败、版本不匹配、读取失败或 stat 失败时，会保留当前有效主题并输出 `warn` 日志，不回退到坏配置。
- 当前主题热刷新仍不包含未打 marker 的临时自定义节点、列表行内部 gap / margin、content width / auth panel width、z-order 和全量页面结构重排；这些值仍主要在 UI 创建时生效。
- 当前不包含 AssetServer watcher 和样式类系统；i18n 已进入启动期加载和文件热加载第一版，详见下方记录。

### 阶段 2 小入口：基础 i18n 文案 key

- 已新增 `UiI18nPlugin`，挂入 `UiFrameworkPlugin`。
- 已新增 `UiI18n` resource，提供 `tr(key, fallback)` 和预留的 `text(key)` 轻量 API。
- 已新增 `UiI18nText { key, fallback }` 组件；新增 key helper 创建的文本节点会带上该 marker，`UiI18n` 变化后会刷新已生成 `Text` 节点。
- 已新增 RON 文案配置：
  - `project/assets/ui/i18n/zh_cn.ron`
  - `project/assets/ui/i18n/en_us.ron`
- 文案配置结构为 `version`、`locale`、`texts`；当前支持版本为 `1`。
- 启动时优先读取 `MYBEVY_UI_I18N` 指定文件；否则按 `MYBEVY_UI_LOCALE` 指定语言读取 `assets/ui/i18n/<locale>.ron`。
- `MYBEVY_UI_LOCALE` 支持大小写和 `-` / `_` 形式归一，例如 `en-US` 会归一为 `en_us`。
- 未指定语言时默认使用 `zh_cn`。
- 如果指定语言文件缺失，会回退到中文默认资源；如果所有文件读取失败，会使用内置中文文案表。
- 缺失 key 时会输出 `warn`；如果内置中文表有该 key，则显示内置中文兜底，否则显示调用处 fallback，fallback 为空时显示 key 本身。
- 已接入 Login、Lobby、UiGallery、Touch Ripple HUD 返回按钮的标题、导航按钮、主要说明文字和示例按钮文案。
- 已新增 i18n 文件热加载第一版：运行中定时轮询当前成功加载的 i18n 文件 modified 时间；如果启动期没有成功加载配置，则优先轮询 `MYBEVY_UI_I18N` 指定路径，否则轮询当前语言资源路径。
- 热加载解析成功后会替换 `UiI18n` 资源，并刷新带 `UiI18nText` marker 的已生成 `Text` 节点。
- 热加载解析失败、版本不匹配、读取失败或 stat 失败时，会保留当前有效文案并输出 `warn` 日志，不回退到坏配置。
- 动态 Toast、Loading、Confirm 和 UiGallery Floating Panel 预览文案已带 i18n marker，可随 `UiI18n` 热加载刷新。
- 当前暂不提供运行中语言切换 UI；语言仍通过启动前环境变量选择，运行中刷新以当前轮询文件为准。

### Android Back 接入记录

- Android Back 通过 Bevy 0.18 的逻辑键 `Key::BrowserBack` 接入，与桌面 `Esc` 一样写入 `UiPanelCommand::CloseTop`。
- 已用 `cargo fmt --check` 和 `cargo check` 验证桌面目标。
- Android 目标验证尝试过 `cargo ndk -t arm64-v8a -P 26 check`，但超过 5 分钟未完成；需要后续在 Android 构建环境中继续验证真机 Back 行为。

### 输入路由调试面板第一版

- 已新增 `UiDebugPlugin`，挂入 `UiFrameworkPlugin`。
- 开发快捷键为 `F3`：按下后切换显示 / 隐藏输入路由调试面板，不占用 `Esc`、`Tab`、`Enter` 等交互键。
- 调试面板位于独立 `UiLayer::Debug`，使用高 `ZIndex(250)` 显示在常规 UI 之上。
- 调试面板不纳入 Panel Manager，不添加 `UiPanelRoot`，不会进入 `CloseTop` 栈，也不会改变 `UiInputState.focused_panel`。
- 调试根节点和文本节点使用 `Pickable::IGNORE`，不阻塞下层 pointer 输入，也不参与 hover / focusable button 规则。
- 面板当前显示：
  - `UiInputState.pointer_blocked`
  - `UiInputState.focused_panel`
  - `UiInputState.top_blocking_panel`
  - `UiFocusState.focused_entity`
  - 当前可见 `UiPanelRoot` 列表：`id`、`kind`、`owner_mode`、可见性、`Entity`
- 文本每帧刷新，刷新顺序排在 `UiInputSystems::Update` 之后，便于观察当前帧输入路由状态。

当前限制：

- 第一版只做只读诊断，不提供点击选择实体、冻结刷新、过滤、复制或历史记录。
- 可见 panel 列表只基于 `Visibility` / `InheritedVisibility` 过滤，不做屏幕裁剪或实际命中区域判断。
- 调试面板自身不做全量重建；已创建节点的背景、边框、文本色、字号、overlay 位置、padding 和圆角走现有主题 role 刷新。

### 输入路由调试面板独立窗口

- F3 调试面板支持显示目标切换：默认显示在游戏主窗口，桌面调试时可按 `F7` 切到独立 `MyBevy UI Debug` 窗口，再按一次回到主窗口。
- 启动时可通过环境变量 `MYBEVY_UI_DEBUG_TARGET` 指定默认显示目标：
  - `game` / `main-window` / `inline`：显示在游戏主窗口。
  - `window` / `debug-window` / `popout`：显示在独立调试窗口。
- Android 等移动端强制回退到游戏主窗口，避免多窗口能力差异影响真机运行。
- 独立窗口模式使用专用 `Window + Camera2d + UiTargetCamera`，调试面板会自适应填充窗口内容区宽度；调试 UI 仍不纳入 Panel Manager，且继续使用 `Pickable::IGNORE`，不会阻塞主窗口下层 pointer 输入。
- 手动关闭独立调试窗口时，F3 调试状态会自动关闭；再次按 `F3` 会按当前显示目标重新创建。

### P3-04-04 调试状态复制

- F3 调试面板新增 `F8` 复制当前调试状态快捷键，header 的快捷键行已同步显示 `F8 copy`。
- 第一版不接系统剪贴板、不引入新 crate；按下 `F8` 后会把当前面板显示文本写入 `UiDebugState` 内部缓存，并输出 `info!` 日志 `UI debug state copied to internal buffer (...)`。
- 复制内容与屏幕显示保持一致：包含当前 header 和 body；如果 `F4` 冻结中，会复制当前 header 加冻结 body。
- 当前限制：内部缓存主要用于调试和后续扩展，暂不能直接粘贴到外部编辑器。后续如需要桌面系统剪贴板，可在现有 `last_copied_text` 基础上增加平台剪贴板桥接；Android 仍建议保留内部缓存 / 日志降级。

### P3-03 UI 性能统计与高频系统巡检

- P3-03-01 已新增 `UiStatsPlugin` 和 `UiStats`，当前每帧采集 UI `Node` 总数、可见 `Node` 数、`Text` 节点数、Panel 总数和按 `UiPanelKind` 聚合计数。
- P3-03-02 已巡检高频系统：
  - `theme.rs`：主题热加载以 0.8 秒定时轮询文件修改时间，视觉刷新已有 `theme.is_changed()` 早退；本轮无需调整。
  - `i18n.rs`：i18n 热加载同样以 0.8 秒轮询，文本刷新已有 `i18n.is_changed()` 早退，且写文本前做差异判断；本轮无需调整。
  - `scroll.rs`：滚轮和拖拽走事件 / observer，不存在无条件每帧写滚动位置；本轮无需调整。
  - `controls.rs`：按钮、文本输入和控件显示系统仍在 `Update` 中运行；本轮已补充若干差异写入保护，并把 slider / stepper 数值显示同步收窄到 `Changed<UiSlider>` / `Changed<UiStepper>`。
  - `debug.rs`：F3 文本仍按帧构建以便观察输入路由；高亮系统在调试关闭且高亮关闭时会直接早退，避免无意义扫描 Panel。
  - `stats.rs`：统计仍保留每帧全量采集，但只有统计值变化时才写回 `UiStats`，减少下游 `Changed<UiStats>` 误触发。
- P3-03-03 已把 `UiStats` 接入 F3 调试面板正文：
  - 显示 `ui stats` 区域，包含 UI `Node` 总数、可见 `Node` 数、`Text` 节点数，以及 Panel 总数和 `page / hud / floating / modal / blocking` 分类计数。
  - 统计信息属于 F3 body，按 `F4` 冻结时会随 route history 和 panel 列表一起冻结；header 仍继续显示当前 `freeze / filter / highlight / target` 状态。
- P3-03-04 已在 `UiGallery` 增加静态压力样例：
  - `Stress Sample` 区域位于 UiGallery 滚动正文末尾，包含 96 个小条目。
  - 每个条目包含编号文本、状态文本和一个 `Inspect` 小按钮，用于人工观察大量 `Node` / `Text` / `Button` 对 F3 `ui stats` 的影响。
  - 样例不引入定时器或每帧动态刷新，避免把刷新压力和静态节点压力混在一起。
  - 已补充中英文 i18n 资源和内置中文 fallback。
- 当前保留的成本：`UiStats` 每帧全量扫描 UI 节点，F3 打开时调试文本每帧构建。这两处是诊断能力的有意成本，后续可按需增加采样间隔、dirty marker 或只在独立调试窗口打开时提高采样频率。

#### P3-03 收口状态

- 完成状态：
  - 已具备 `UiStats` 统计资源和 `UiStatsPlugin`，可持续采集 UI 节点、文本节点和 Panel 分类计数。
  - F3 调试面板已显示 `ui stats` 区域，支持在主窗口和独立调试窗口中观察统计数据。
  - `UiGallery` 已提供 96 项静态压力样例，用于观察大量静态节点、文本和按钮对统计数据的影响。
  - 高频系统巡检已完成，主题 / i18n / scroll 保持现有早退或事件驱动策略，controls 和 debug 已补充差异写入、`Changed<T>` 过滤或早退保护。
- 自动验证：
  - P3-03-01 至 P3-03-04 每步均已通过 `cargo fmt --check`、`cargo test` 和 `cargo check`。
  - P3-03-05 仅整理文档，不改 Rust 代码；文档提交前至少运行 `git diff --check`。
- 手动验收建议：
  - 启动 `UiGallery`，按 `F3` 打开调试面板，确认正文中存在 `ui stats` 区域。
  - 滚动到 `Stress Sample` 区域，观察 `nodes`、`text` 和 panel 统计保持合理变化，不出现异常跳动。
  - 按 `F7` 切换到独立调试窗口，确认 `ui stats` 仍持续显示且窗口宽度自适应。
  - 操作 slider、stepper 和 text input，确认控件交互正常，统计数据不会因为无关刷新产生持续异常增长。
- 当前保留成本：
  - `UiStats` 仍在每帧全量扫描 UI `Node`、`Text` 和 `UiPanelRoot`，适合作为第一版诊断数据源，但不是最终低成本采集方案。
  - F3 打开时仍会每帧构建调试文本，以保证输入路由、冻结和过滤状态可观察。
  - `Stress Sample` 当前只覆盖静态节点压力，不覆盖高频动态文本刷新、列表虚拟化或批量重建压力。
- 后续优化建议：
  - 给 `UiStats` 增加采样间隔、dirty marker 或按调试开关启停采集，降低常驻成本。
  - 增加文本刷新计数、panel 打开 / 关闭次数、panel 栈变化次数等更贴近业务页面的指标。
  - 接入系统耗时或阶段耗时记录，观察 theme、i18n、input、controls、debug 等系统在真实页面中的开销。
  - 为 panel 切换、长列表滚动和大量文本更新补充专门压力样例，区分静态节点成本和动态刷新成本。
  - 在 Android 真机记录实际帧率、触控响应和 F3 stats 表现，作为 P3-05 平台验证输入。

### P3-02 数据绑定基础

- P3-02-01 已新增数据绑定核心模块 `project/src/game/ui/core/binding.rs`：
  - `UiBindingPlugin` 已挂入 `UiFrameworkPlugin`，当前只注册 `UiBindingSystems::Apply` 系统集，为后续绑定应用系统提供排序入口。
  - 新增 `UiBindingPath`，用于表示简单字符串数据源路径；会去除路径和分段两侧空白，并拒绝空路径或空分段。
  - 新增预留组件 `UiBoundText`、`UiBoundVisibility`、`UiBoundDisabled`，分别对应后续文本、可见性和 disabled 绑定。
  - 新增 `UiVisibilityBindingMode` / `UiDisabledBindingMode` 和纯 helper，用于把 bool 映射为 `Visibility` 或 disabled marker 意图。
- 本阶段未接入真实页面、控件行为或 `DisabledButton` / `DisabledTextInput` 等具体 marker，只建立最小 API 和测试边界。
- 自动验证：P3-02-01 已通过 `cargo fmt --check`、`cargo test` 和 `cargo check`。

- P3-02-02 已实现最小文本绑定：
  - `UiBindingValues` 作为第一版绑定值资源，当前保存文本路径和值，提供 `set_text(...)`、`text(...)`、`set_text_path(...)` 和 `text_path(...)` API。
  - `UiBindingPlugin` 已注册文本 apply 系统：当 `UiBindingValues` 或 `UiBoundText` 变化时，自动把绑定值写入对应 `Text`，缺值时使用 `UiBoundText.fallback`。
  - 写入 `Text` 前会做差异判断，避免无意义刷新。
  - `UiGallery` 已新增 `Binding Sample` 区域，包含一个绑定文本和 `Update Binding` 按钮；点击按钮只更新绑定资源，文本由绑定系统自动刷新。
  - 已补充中英文 i18n 资源和内置中文 fallback。
- 本阶段仍不接真实页面，不实现可见性 / disabled 绑定，也不引入每帧动态绑定刷新。

- P3-02-03 已实现 bool 绑定：
  - `UiBindingValues` 新增 bool 值存储，提供 `set_bool(...)`、`bool(...)`、`set_bool_path(...)` 和 `bool_path(...)` API，路径规范复用 `UiBindingPath`。
  - `UiBindingPlugin` 已注册可见性 apply 系统：`UiBoundVisibility + Visibility` 会根据 bool 值和 `UiVisibilityBindingMode` 写入 `Visibility`；缺值默认按 `false` 处理。
  - `UiBindingPlugin` 已注册按钮 disabled apply 系统：`UiBoundDisabled + Button` 会根据 bool 值和 `UiDisabledBindingMode` 插入或移除 `DisabledButton`，写入前做差异判断。
  - `UiGallery` 的 `Binding Sample` 已扩展为文本、可见提示和绑定按钮；点击 `Update Binding` 会同时更新文本并切换提示可见性和按钮 disabled 状态。
  - 已补充中英文 i18n 资源和内置中文 fallback。
- 当前 disabled 绑定第一版只支持 `Button` 的 `DisabledButton` marker，暂不处理 `DisabledTextInput`、slider、stepper 或自定义禁用 marker。

- P3-02-04 已接入真实页面：
  - Login 页面副标题复用 `auth.login.subtitle` 作为绑定路径，进入页面时由 `UiBindingValues` 写入当前 i18n 文案，文本节点挂载 `UiBoundText` 后由绑定系统刷新。
  - Login 状态下新增轻量同步系统：当 `UiI18n` 热更新时，重新写入同一路径绑定值，让真实页面文本通过绑定链路响应文案变化。
  - 本阶段只接一个低风险文本点，不批量改造页面，也不改变登录按钮路由行为。
  - 后续可选择 Lobby 状态说明、联网状态或真实表单状态作为更有业务意义的绑定来源。

- P3-02-05 已完成数据绑定基础收口：
  - 测试覆盖已包含路径规范化、非法路径拒绝、文本 / bool set-get、文本 fallback、文本 apply、可见性 apply、按钮 disabled apply 和绑定路径构造失败场景。
  - 本次补充 `remove_text(...)` / `remove_bool(...)` 纯逻辑测试，覆盖删除已有值、重复删除和非法路径删除。
  - 自动验证：P3-02-01 到 P3-02-04 均已通过 `cargo fmt --check`、`cargo test` 和 `cargo check`；P3-02-05 收口也按同一组命令验证。
  - 手动验收建议：进入 UiGallery，点击 `Binding Sample` 的 `Update Binding`，确认绑定文本刷新、提示文本显隐切换、绑定按钮 disabled 状态切换；进入 Login 页面并热更新 i18n 文案时，确认副标题可通过绑定链路刷新。
  - 当前限制：第一版绑定源仍是简单路径资源，不支持表达式、派生值、集合绑定、双向绑定、typed model、批量 diff 或跨页面生命周期回收策略。
  - 后续建议：优先把真实页面状态文本、联网状态和表单提交状态接入绑定；等页面数量变多后，再考虑 typed binding source、列表绑定和 dev inspector 中的绑定路径可视化。

### 通用文本输入框第一版

- 已新增 widgets 层通用文本输入框 `text_input(...)`，根节点使用 `Button + FocusableButton + UiTextInput`，因此可以通过鼠标点击进入焦点，也可以通过现有 `Tab` 焦点系统访问。
- 已新增组件：
  - `UiTextInput`
  - `UiTextInputValue`
  - `UiTextInputCursor`
  - `UiTextInputPlaceholder`
  - `UiTextInputText`
  - `UiTextInputMaxChars`
  - `ReadonlyTextInput`
  - `DisabledTextInput`
- 已新增提交消息 `UiTextInputSubmitted { entity, value }`；当前 `UiGallery` 在 Enter 提交时写日志，不绑定业务逻辑。
- 文本输入基于 Bevy 0.18.1 的 `KeyboardInput` message：按下态读取 `keyboard_input.text` 插入可打印字符，支持左右移动、Home / End、Delete、Backspace、Space、Ctrl+A 全选、内部 Ctrl+C / Ctrl+V 复制粘贴和 Enter 提交。
- 文本输入光标保存在 `UiTextInputCursor` 中，当前显示实现是在文本中插入 `|`，用于第一版可见光标位置。
- `UiTextInputMaxChars` 支持按字符数限制输入长度；超出部分会在插入或粘贴时截断。
- `ReadonlyTextInput` 不接受编辑、不发送提交，但允许焦点和光标移动；`DisabledTextInput` 不参与焦点遍历，不接受编辑、不发送提交，并使用禁用态视觉。
- 显示文本节点单独带 `UiTextInputText` marker；内容变化时只刷新该文本节点，不重建页面或输入框根节点。
- placeholder 在 value 为空时显示，并使用 muted 文本色；有 value 时显示当前值并使用 primary 文本色。
- 输入框有 idle / hovered / pressed / focused 视觉状态，第一版复用现有 secondary button 背景色和 primary focused 边框色。
- `UiInputState.pointer_blocked` 已识别当前聚焦的 `UiTextInput`，因此在 Touch Ripple 中输入文字时，玩法触控采集会被阻塞，不会同时触发水波纹。
- `UiGallery` 已新增 Inputs 区域，展示普通可编辑输入、只读输入、禁用输入、长度限制输入和 placeholder 示例。

当前限制：

- 当前不支持 IME 组合态显示。
- 当前只支持 Ctrl+A 全选的内部选择状态，不支持 Shift+方向键、鼠标拖拽、多段选择或选区高亮样式。
- 当前 Ctrl+C / Ctrl+V 只使用 UI 内部剪贴板缓存，不接入系统剪贴板。
- 当前不支持撤销、密码输入或输入掩码。
- 第一版 value 存在组件里；业务页面如需保存数据，应监听 `UiTextInputSubmitted` 或读取对应实体上的 `UiTextInputValue`。
- 第一版没有完整表单容器、提交聚合、业务校验规则或表单布局协议；基础 helper / required / validation / error 状态见下方记录。

### 表单状态和校验第一版

- 已新增文本输入表单状态组件：
  - `UiTextInputRequired`
  - `UiTextInputError`
  - `UiTextInputHelperText`
  - `UiTextInputValidationMessage`
  - `UiTextInputFormMessage`
- 已新增 `text_input_form_message(...)` helper，用于创建绑定到文本输入实体的 helper / validation 文本节点。
- 表单消息优先级为：显式 validation message > 显式 error marker > required 空值 > helper text。
- 文本输入错误态会使用主题错误边框；表单错误消息使用主题错误文本色。
- 禁用态视觉优先级高于错误态和焦点态：禁用输入框保持禁用背景、禁用边框和 muted 表单消息。
- 主题新增 `colors.text_error` 和 `colors.error` token，并同步到内置默认主题与 `assets/ui/themes/default.ron`；旧版主题配置缺少这两个字段时会使用默认值。
- `UiGallery` Inputs 区域已展示普通 helper、必填空值、显式错误、长度限制、只读、禁用覆盖错误和可选空输入样例。
- 已补充中英文 i18n 资源和内置中文 fallback，覆盖新增输入框 placeholder、helper 和 validation 文案。
- 已新增单元测试覆盖 helper 显示、validation 优先级、required 空值错误和 disabled/error 边框优先级。

当前限制：

- 当前只提供文本输入层面的状态表达，不提供完整 Form 容器、字段注册、统一 submit、dirty / touched 状态或跨字段校验。
- `UiTextInputRequired` 只做空值校验；更复杂的校验由业务系统写入 `UiTextInputValidationMessage` 或 `UiTextInputError`。
- helper / required / validation 组件保存的是已解析字符串；运行中 i18n 热加载不会自动改写这些组件里的字符串，后续需要 key 化表单消息组件才能完全跟随语言热刷新。

### Selection 控件第一版

- P2-03 已拆为多个小任务串行处理，selection 类、numeric 类和 icon button 第一版均已完成。
- 已新增 checkbox builder：
  - `checkbox_key(...)`
  - `checked_checkbox_key(...)`
  - `disabled_checkbox_key(...)`
- 已新增 toggle builder：
  - `toggle_key(...)`
  - `toggle_on_key(...)`
  - `disabled_toggle_key(...)`
- 已新增 segmented control builder：
  - `segmented_control(...)`
  - `segment_option_key(...)`
  - `selected_segment_option_key(...)`
  - `disabled_segment_option_key(...)`
- Selection 控件第一版复用 `Button + FocusableButton`，因此可接入现有 Tab 焦点系统；禁用态复用 `DisabledButton`，不会进入焦点候选。
- 视觉状态复用现有 `primary_button` / `secondary_button` 主题 token 和 `SelectedButton` / `DisabledButton` 优先级；未新增主题 schema。
- `UiGallery` 已新增 Selection Controls 区域，展示 checkbox 未选 / 已选 / 禁用、toggle off / on / disabled，以及 segmented small / selected medium / disabled large。
- 已补充中英文 i18n 资源和内置中文 fallback，覆盖新增 selection 样例文案。
- 已新增单元测试覆盖 selection 视觉优先级：disabled 高于 hovered/focused，selected 使用 selected 色，idle focused 使用 focused 色。

当前限制：

- 当前 selection 控件只提供静态状态和 builder，不提供点击后自动切换 checked/on/selected 的统一状态机或业务事件。
- checkbox / toggle 暂未绘制独立勾选框或滑块轨道；第一版用按钮 selected/disabled 视觉表达状态。
- segmented control 只提供选项按钮排列和 selected marker，不负责互斥选择更新。

### Numeric 控件第一版

- 已新增 slider builder：
  - `slider_key(...)`
  - `disabled_slider_key(...)`
- 已新增 stepper builder：
  - `stepper_key(...)`
  - `disabled_stepper_key(...)`
- Slider 第一版保存 `UiSlider { value, min, max }`，会对 value 做边界夹取，并根据 ratio 渲染静态 track / fill / value 文本。
- Stepper 第一版保存 `UiStepper { value, min, max, step }`，会对 value 和 step 做边界整理，并展示 `- / value / +` 的静态控件组。
- Numeric 控件复用现有主题 token：文本、按钮、输入框背景、panel border 和 disabled 色；未新增主题 schema。
- `UiGallery` 已新增 Numeric Controls 区域，展示正常 / 禁用 slider 和正常 / 禁用 stepper。
- 已补充中英文 i18n 资源和内置中文 fallback，覆盖新增 numeric 样例文案。
- 已新增单元测试覆盖 slider ratio 夹取、反向边界排序、零范围处理，以及 stepper increment / decrement 的边界夹取。

当前限制：

- 当前 slider 不支持拖拽、点击定位、键盘调整或业务事件，只提供静态 value 展示。
- 当前 stepper 的 `-` / `+` 按钮只用于视觉展示，不自动修改 `UiStepper.value`，后续需要统一控件事件协议后再接入交互。
- Numeric 控件的 label 文案支持 i18n marker 热刷新；value 来自组件，不做 i18n 格式化、本地化小数分隔或单位显示。

### Icon Button 控件第一版

- 已新增 icon button builder：
  - `icon_button_key(...)`
  - `disabled_icon_button_key(...)`
  - `loading_icon_button_key(...)`
- Icon button 第一版复用 `Button + FocusableButton`，因此可接入现有 Tab 焦点系统；禁用态和加载态分别复用 `DisabledButton` / `LoadingButton`，不会触发现有 action 处理。
- 已新增 `UiIconButton { label, accessible_label }` 状态组件；`label` 是当前可见短符号，`accessible_label` 使用 i18n key 解析结果，为后续无障碍名称或 tooltip 系统预留。
- Icon button 使用稳定方形尺寸，`width / min_width / height` 都取 `theme.button.height`，不会继承普通按钮 `theme.button.min_width`。
- `UiGallery` 已新增 Icon Buttons 区域，展示 normal、focused、selected、disabled 和 loading 样例。
- 已补充中英文 i18n 资源和内置中文 fallback，覆盖新增 icon button 样例的 accessible label 文案。
- 已新增单元测试覆盖 icon button 方形尺寸 helper。

当前限制：

- 第一版不引入外部图标库、不新增图标资产，只用 `+`、`-`、`?`、`x`、`...` 等单字符 / 短文本符号作为可见 icon。
- 当前不实现 tooltip 系统，也不接入复杂 icon atlas；`accessible_label` 只作为组件状态保存，尚未桥接到平台无障碍树。
- 当前只提供静态 builder 和视觉状态，不提供统一 icon button 业务事件协议。

### P2-04 控件交互测试记录

- 已补充 `project/src/game/ui/widgets/controls.rs` 单元测试，覆盖通用按钮视觉优先级 `disabled > loading > pressed > hovered > selected > focused > normal`。
- 已补充 selection / icon button 纯逻辑测试，覆盖 disabled 文本色角色、selected / loading 背景色和 icon button 方形尺寸 helper。
- 已补充 numeric 控件测试，覆盖 `UiSlider::new` 的边界排序、NaN 夹取、value 格式化，以及 `UiStepper::new` 的边界排序、value 夹取和 step 归一化。
- 已补充 `project/src/game/ui/core/focus.rs` 单元测试，覆盖 `next_focus_entity` 前后循环、Tab / Shift+Tab 焦点移动、hidden / disabled / loading 过滤，以及 Modal panel 限制焦点候选。

当前限制：

- `project/src/game/ui/widgets/scroll.rs` 的滚动边界依赖 Bevy `ComputedNode` 布局结果，本次未新增窗口自动化或大规模 E2E；滚轮、拖拽滚动仍需要在 UiGallery 人工验证，后续如抽出纯滚动计算 helper 可再补单元测试。
- 当前 icon button 的 `accessible_label` 热刷新仍通过 ECS 系统跟随 i18n 资源变化，本次只覆盖底层视觉 / 状态 helper；完整平台无障碍名称桥接仍是后续能力。

### UI 字体和中文字形修复

- 已新增 UI 字体资源 `UiFontAssets` 和 `UiFontPlugin`，挂入 `UiFrameworkPlugin`。
- UI 字体通过 Bevy `AssetServer` 加载 `project/assets/ui/fonts/MyBevyUiCjk-Regular.otf`，Android Gradle 壳工程已把 `project/assets` 打包进 APK assets，因此桌面和 Android 使用同一项目内字体路径。
- 桌面运行时已把 Bevy `AssetPlugin.file_path` 固定为 `project/assets` 绝对路径，避免直接启动 `target/debug/project.exe` 时从 `target/debug/assets` 查找字体。
- 字体资产基于 Noto Sans CJK SC Regular 生成子集，并保留 `project/assets/ui/fonts/NotoSansCJKsc-LICENSE.txt`。不复制 Windows 系统字体，避免系统字体许可不明的问题。
- 通用文本 helper、按钮文本、文本输入显示、Toast、Loading、Confirm、Floating Panel 和调试面板文本都显式使用 `UiFontAssets.regular`，避免落回 Bevy 默认 `FiraMono-subset.ttf` 导致中文显示方框。
- 子集当前覆盖基本拉丁字符、常用标点、全角符号和 CJK Unified Ideographs `U+4E00..U+9FFF`；默认 `zh_cn` 文案和常见中文输入可正常显示。

当前限制：

- 字体子集不覆盖扩展汉字区、emoji、日文假名、韩文、繁体专用扩展字形等；后续新增语言或特殊符号时需要重新生成或替换字体子集。
- 当前只有 Regular 字重，`TextFont.weight` 不单独加载 Bold/Medium 字体。
