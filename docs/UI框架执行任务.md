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
  - `screen.rs`
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
  - `UiScreenPlugin`
  - `UiLayerPlugin`
  - `UiRouterPlugin`
  - `UiInputPlugin`
- `ScreensPlugin` 不再直接注册 `UiThemePlugin` 和 `UiWidgetsPlugin`，而是注册 `UiFrameworkPlugin`。

验收：

- UI 相关插件入口集中。
- 后续新增 UI 框架能力只需要挂到 `UiFrameworkPlugin`。

### 3. UI 屏幕和根节点

文件：

- `project/src/game/ui/screen.rs`
- `project/src/game/screens/auth/login.rs`
- `project/src/game/screens/lobby/game_list.rs`
- 后续可能涉及 `project/src/game/screens/gameplay/*`

建议抽象：

```rust
pub(super) enum UiScreenId {
    LoginPage,
    GameListPage,
    TouchRippleHud,
}

#[derive(Component)]
pub(super) struct UiScreenRoot {
    pub id: UiScreenId,
}
```

任务：

- 页面根节点统一添加 `UiScreenRoot`。
- 登录页根节点使用 `UiScreenId::LoginPage`。
- 游戏列表页根节点使用 `UiScreenId::GameListPage`。
- `AppUiMode::WanfaTouchRipple` 进入后生成一个最小 `UiScreenId::TouchRippleHud` 根节点。第一版可以只作为 HUD 容器，不需要放实际按钮。

验收：

- 能通过查询 `UiScreenRoot` 找到当前存在的 UI 页面根节点。
- 页面退出后不会留下孤儿 UI 根节点。

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
3. 新增 `screen.rs`，给现有页面加 `UiScreenRoot`。
4. 新增 `layer.rs`，定义页面层、弹窗层、Toast 层。
5. 新增 `router.rs`，让按钮通过 `UiRouteCommand` 切换 `AppUiMode`。
6. 新增 `input.rs`，把玩法触控输入拦截改为读取 `UiInputState`。
7. 实现最小 Toast。
8. 实现最小确认弹窗。
9. 跑 `cargo fmt` 和 `cargo check`。
10. 手动运行 `cargo run`，检查登录、列表、玩法触控、Toast、确认弹窗路径。

## 验收清单

- [ ] `project/src/game/navigation/mod.rs` 中类型名已改为 `AppUiMode`。
- [ ] `project/src/game/plugin.rs` 中玩法系统使用 `AppUiMode::WanfaTouchRipple`。
- [ ] 登录页进入大厅可用。
- [ ] 大厅进入触控玩法可用。
- [ ] 触控玩法中鼠标/触控水波纹仍可用。
- [ ] UI 按钮输入不会被玩法触控重复消费。
- [ ] 页面根节点带有 `UiScreenRoot`。
- [ ] 触控玩法模式有 `UiScreenId::TouchRippleHud` 根节点。
- [ ] Toast 可以显示并自动消失。
- [ ] 确认弹窗可以打开、关闭并阻塞下层输入。
- [ ] UI 框架入口集中在 `UiFrameworkPlugin`。
- [ ] `cargo fmt` 通过。
- [ ] `cargo check` 通过。

## 已确认决策

1. 命名使用 App UI Mode 概念。

代码类型使用 `AppUiMode`。不使用 `AppUIMode` 是为了符合 Rust 命名惯例。

2. 触控水波纹使用专用 mode。

使用 `AppUiMode::WanfaTouchRipple`。不归入泛化的 `Gameplay`。

3. 第一版实现可见 Toast。

Toast 需要能显示文本、挂到 Toast 层并自动消失。

4. 第一版实现确认弹窗。

确认弹窗需要遮罩、确认/取消按钮、结果事件和输入阻塞。

5. 关于 `UiScreenId::TouchRippleHud`。

这个问题的意思是：进入触控水波纹模式时，是否创建一个属于玩法 HUD 的 UI 根节点。它不等于全屏页面，也不一定要有可见内容；它只是给暂停按钮、网络状态条、调试入口等玩法内 UI 预留挂载位置。

本轮建议生成一个最小 `TouchRippleHud` 根节点，先不放实际控件。这样页面层结构完整，后续添加 HUD 控件不需要再改生命周期结构。

## 后续任务入口

本任务完成后，下一轮建议做：

- Loading 遮罩。
- `UiGallery` 示例页面。
- 更完整的 `UiInputState` 命中和遮罩阻塞规则。
