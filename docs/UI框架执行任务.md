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
- [ ] 登录页进入大厅可用。
- [ ] 大厅进入触控玩法可用。
- [ ] 触控玩法中鼠标/触控水波纹仍可用。
- [ ] UI 按钮输入不会被玩法触控重复消费。
- [x] 页面根节点带有 `UiPanelRoot`。
- [x] 触控玩法模式有 `UiPanelId::TouchRippleHud` 根节点。
- [ ] Toast 可以显示并自动消失。
- [ ] 确认弹窗可以打开、关闭并阻塞下层输入。
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
- 热加载解析失败、版本不匹配、读取失败或 stat 失败时，会保留当前有效主题并输出 `warn` 日志，不回退到坏配置。
- 第一版热加载不包含布局尺寸、字号、圆角、padding、z-order 和半透明遮罩颜色的运行时刷新；这些值仍主要在 UI 创建时生效。
- 当前不包含 AssetServer watcher 和样式类系统；i18n 已进入第一版启动期加载，详见下方记录。

### 阶段 2 小入口：基础 i18n 文案 key

- 已新增 `UiI18nPlugin`，挂入 `UiFrameworkPlugin`。
- 已新增 `UiI18n` resource，提供 `tr(key, fallback)` 和预留的 `text(key)` 轻量 API。
- 已新增 `UiI18nText { key, fallback }` 组件；新增 key helper 创建的文本节点会带上该 marker，供后续语言热刷新或运行中切换使用。
- 已新增 RON 文案配置：
  - `project/assets/ui/i18n/zh_cn.ron`
  - `project/assets/ui/i18n/en_us.ron`
- 文案配置结构为 `version`、`locale`、`texts`；当前支持版本为 `1`。
- 启动时优先读取 `MYBEVY_UI_I18N` 指定文件；否则按 `MYBEVY_UI_LOCALE` 指定语言读取 `assets/ui/i18n/<locale>.ron`。
- `MYBEVY_UI_LOCALE` 支持大小写和 `-` / `_` 形式归一，例如 `en-US` 会归一为 `en_us`。
- 未指定语言时默认使用 `zh_cn`。
- 如果指定语言文件缺失，会回退到中文默认资源；如果所有文件读取失败，会使用内置中文文案表。
- 缺失 key 时会输出 `warn`；如果内置中文表有该 key，则显示内置中文兜底，否则显示调用处 fallback，fallback 为空时显示 key 本身。
- 已接入 Login、Lobby、UiGallery 的标题、导航按钮、主要说明文字和示例按钮文案。
- 第一版暂不覆盖动态 Toast、Loading、Confirm、Floating Panel 的运行时文本，也不覆盖 Touch Ripple HUD 返回按钮。
- 第一版暂不实现运行中语言切换和 i18n 文件热加载；`UiI18nText` 已作为后续刷新已生成文本节点的 marker 预留。

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
- 调试面板自身没有主题热刷新后的布局重建；已创建节点的背景、边框和文本色继续走现有主题 role 刷新。

### 通用文本输入框第一版

- 已新增 widgets 层通用文本输入框 `text_input(...)`，根节点使用 `Button + FocusableButton + UiTextInput`，因此可以通过鼠标点击进入焦点，也可以通过现有 `Tab` 焦点系统访问。
- 已新增组件：
  - `UiTextInput`
  - `UiTextInputValue`
  - `UiTextInputPlaceholder`
  - `UiTextInputText`
- 已新增提交消息 `UiTextInputSubmitted { entity, value }`；当前 `UiGallery` 在 Enter 提交时写日志，不绑定业务逻辑。
- 文本输入基于 Bevy 0.18.1 的 `KeyboardInput` message：按下态读取 `keyboard_input.text` 追加可打印字符，`Key::Backspace` 删除末尾字符，`Key::Enter` 发送提交消息。
- 显示文本节点单独带 `UiTextInputText` marker；内容变化时只刷新该文本节点，不重建页面或输入框根节点。
- placeholder 在 value 为空时显示，并使用 muted 文本色；有 value 时显示当前值并使用 primary 文本色。
- 输入框有 idle / hovered / pressed / focused 视觉状态，第一版复用现有 secondary button 背景色和 primary focused 边框色。
- `UiInputState.pointer_blocked` 已识别当前聚焦的 `UiTextInput`，因此在 Touch Ripple 中输入文字时，玩法触控采集会被阻塞，不会同时触发水波纹。
- `UiGallery` 已新增 Inputs 区域，展示一个带初始值的普通输入框和一个 placeholder 示例。

当前限制：

- 第一版不支持光标、选区、复制粘贴、左右移动、Home/End、Delete、撤销、IME 组合态显示和密码输入。
- 第一版没有 disabled / readonly 语义。
- 第一版 value 存在组件里；业务页面如需保存数据，应监听 `UiTextInputSubmitted` 或读取对应实体上的 `UiTextInputValue`。
- 第一版没有显式长度限制、校验规则、错误态或表单布局协议。
