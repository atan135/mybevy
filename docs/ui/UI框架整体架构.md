# UI 框架整体架构

当前 UI 框架是一层建立在 Bevy UI / ECS 之上的游戏 UI 应用框架。它不重写 Bevy 的渲染、布局和基础输入系统，而是把页面模式、面板层级、输入阻断、主题、国际化、控件和调试能力收敛到统一插件组里。

## 插件入口

`project/src/game/screens/mod.rs` 的 `ScreensPlugin` 是页面系统入口：

```rust
app.add_plugins((NavigationPlugin, UiFrameworkPlugin))
    .add_plugins((auth::AuthScreensPlugin, lobby::LobbyScreensPlugin))
    .add_plugins(dev::DevScreensPlugin)
    .add_plugins(gameplay::GameplayScreensPlugin);
```

`UiFrameworkPlugin` 位于 `project/src/game/ui/core/framework.rs`，当前按顺序注册这些能力：

- `UiFontPlugin`：加载 UI 字体资源。
- `UiI18nPlugin`：加载 UI 文案资源并刷新带 `UiI18nText` 的文本节点。
- `UiThemePlugin`：加载主题 token，刷新带 theme role marker 的节点。
- `UiViewportPlugin`：维护 `UiViewport` 和 `UiMetrics`。
- `UiWidgetsPlugin`：注册滚动、输入框、按钮、数值控件等通用控件系统。
- `UiLayerPlugin`：定义 UI 层枚举和层标记。
- `UiRouterPlugin`：处理页面路由和 Toast 命令。
- `UiPanelPlugin`：处理 Loading、Confirm、Floating 等面板命令。
- `UiInputPlugin`：汇总 UI 输入阻断状态。
- `UiFocusPlugin`：维护按钮和输入框焦点。
- `UiBindingPlugin`：应用简单路径绑定。
- `UiAnimationPlugin`：驱动 UI alpha 动画。
- `UiStatsPlugin`：统计 UI 节点和面板数量。
- `UiDebugPlugin`：提供 F3 调试面板。

## 目录边界

- `project/src/game/navigation/`：主流程状态 `AppUiMode` 和路由按钮数据。
- `project/src/game/screens/`：登录、大厅、玩法 HUD、UI Gallery 等业务页面。页面负责在 `OnEnter(AppUiMode)` 生成自己的 Page/HUD 根节点。
- `project/src/game/ui/core/`：框架核心能力，包含 viewport、panel、layer、input、focus、binding、animation、stats。
- `project/src/game/ui/widgets/`：可复用 UI 控件和布局 helper。
- `project/src/game/ui/overlays/`：Toast、Loading、Confirm modal 等顶层 UI 实现和命令路由。
- `project/src/game/ui/style/`：主题 token、主题刷新、字体资源加载。
- `project/assets/ui/`：UI 字体、主题、国际化和示例图片等首包资源。

业务页面可以组合 `widgets` 和 `core` 提供的资源、命令、组件，但不应绕过 Panel Manager 自行管理全局 Loading 或 Confirm。

## 核心数据流

页面模式由 `AppUiMode` 表示。按钮携带 `RouteButton { target }` 后，`UiRouterPlugin` 在按钮按下时写入 `UiRouteCommand::ChangeMode`，系统会先发送 `UiPanelCommand::CloseAllForMode(current_mode)`，再设置 `NextState<AppUiMode>`。

覆盖层分两条流：

- Toast 通过 `UiRouteCommand::ShowToast(UiToast)` 直接关闭旧 Toast 并生成新 Toast。
- Loading、Confirm、Floating 通过 `UiPanelCommand::Open/Toggle/Close` 进入 `UiPanelPlugin` 的 Panel Manager。

页面、HUD 和覆盖层根节点通过两个标记进入框架管理：

- `UiPanelRoot { id, kind, owner_mode }`：描述面板身份、类型和所属页面模式。
- `UiLayerRoot { layer }`：描述渲染和调试层级。

`UiInputPlugin` 和 `UiFocusPlugin` 都依赖这些面板标记判断当前是否存在阻断层、焦点应限制在哪个 panel 内，以及 gameplay 输入是否应被 UI 吞掉。

## 主题和资源流

主题由 `UiTheme` 资源承载，默认从 `project/assets/ui/themes/default.ron` 加载。节点通过 `UiThemeBackgroundRole`、`UiThemeBorderRole`、`UiThemeTextColorRole`、`UiThemeTextStyleRole`、`UiThemeButtonNodeRole`、`UiThemePanelNodeRole`、`UiThemeRootNodeRole` 等 marker 接收主题刷新。

国际化由 `UiI18n` 资源承载，默认 locale 是 `zh_cn`。文本节点如果带有 `UiI18nText`，在 i18n 资源变化时会重新解析 key。业务代码创建文本时优先使用 `*_key` helper，保留 fallback。

字体由 `UiFontAssets` 资源提供，目前统一加载 `ui/fonts/MyBevyUiCjk-Regular.otf`。

## 扩展原则

新增页面优先放入 `project/src/game/screens/`，并在对应 `OnEnter(AppUiMode)` 创建 `UiPanelRoot`。新增可复用控件优先放入 `project/src/game/ui/widgets/`。新增框架级能力才放入 `project/src/game/ui/core/`，例如新的输入仲裁、焦点策略或绑定机制。

新增 UI 资源应放在 `project/assets/ui/` 下的合适子目录；后续下载资源不要放入首包 assets。
