# UI 文档总览

这个目录记录当前自研 UI 框架的实现细节、运行机制、使用方式和已知限制。这里不记录阶段任务、开发流水或后续排期；需要判断框架现状时，以本目录和 `project/src/framework/ui/` 的代码为准。

## 阅读顺序

- [UI框架整体架构.md](UI框架整体架构.md)：插件入口、目录边界、核心资源和系统关系。
- [UI模式与面板层级.md](UI模式与面板层级.md)：页面模式、Panel Manager、层级和关闭语义。
- [UI输入路由与焦点.md](UI输入路由与焦点.md)：输入阻断、焦点候选、键盘激活和滚动协作。
- [UI组件功能与使用.md](UI组件功能与使用.md)：文本、按钮、图标按钮、选择控件、数值控件、输入框和绑定。
- [UI高保真视觉能力.md](UI高保真视觉能力.md)：参考图视觉能力分类、支持状态、Direct Bevy 逃生口、固定 Gallery 验收区域和资源许可边界。
- [UI视觉效果与材质边界.md](UI视觉效果与材质边界.md)：阴影、渐变、独立描边、裁切、自定义材质准入、降级和移动端预算。
- [UI响应式布局.md](UI响应式布局.md)：视口分类、指标推导、布局 helper、安全区和窗口验收。
- [UI主题字体与国际化.md](UI主题字体与国际化.md)：主题 RON、字体资源、i18n RON 和热更新机制。
- [UI覆盖层与弹窗.md](UI覆盖层与弹窗.md)：Toast、Loading、Confirm、Floating 的命令流和层级行为。
- [UI调试与验收.md](UI调试与验收.md)：F3 调试面板、窗口级验收命令和 Android 验收关注点。
- [UI当前限制.md](UI当前限制.md)：当前实现边界和使用时需要规避的点。

## 设计方案

- [UI自动化审计与优化方案.md](UI自动化审计与优化方案.md)：内置 UI 审计模式、全屏截图、滚动检查、AI 分析和自动修复闭环的方案设计。该文档描述目标机制，不代表当前已完整实现。

## 总览图

```mermaid
flowchart TD
    ScreensPlugin["ScreensPlugin"]
    NavigationPlugin["NavigationPlugin"]
    UiFrameworkPlugin["UiFrameworkPlugin"]
    Screens["业务页面插件<br/>auth / lobby / dev / gameplay"]

    AppUiMode["AppUiMode<br/>Login / Lobby / WanfaTouchRipple / UiGallery"]
    GameRouteCommand["GameRouteCommand<br/>ChangeMode"]
    UiOverlayCommand["UiOverlayCommand<br/>ShowToast"]
    UiPanelCommand["UiPanelCommand<br/>Open / Close / Toggle / CloseTop"]

    UiViewport["UiViewport + UiMetrics"]
    UiTheme["UiTheme + role markers"]
    UiI18n["UiI18n + UiI18nText"]
    UiWidgets["UiWidgetsPlugin"]
    UiInput["UiInputState"]
    UiFocus["UiFocusState"]
    UiBinding["UiBindingValues"]
    UiDebug["UiDebugPlugin"]
    UiOverlayPlugin["UiOverlayPlugin"]

    PanelManager["UiPanelPlugin / Panel Manager"]
    LayerRoots["UiLayerRoot<br/>Page / Floating / Modal / Loading / Toast / Debug"]
    Panels["UiPanelRoot<br/>Page / Hud / Floating / Modal / BlockingOverlay"]
    Overlays["Toast / Loading / Confirm / Floating"]

    ScreensPlugin --> NavigationPlugin
    ScreensPlugin --> UiFrameworkPlugin
    ScreensPlugin --> Screens
    NavigationPlugin --> AppUiMode
    NavigationPlugin --> GameRouteCommand
    GameRouteCommand --> AppUiMode
    GameRouteCommand --> UiPanelCommand
    Screens --> Panels
    UiFrameworkPlugin --> UiViewport
    UiFrameworkPlugin --> UiTheme
    UiFrameworkPlugin --> UiI18n
    UiFrameworkPlugin --> UiWidgets
    UiFrameworkPlugin --> UiInput
    UiFrameworkPlugin --> UiFocus
    UiFrameworkPlugin --> UiBinding
    UiFrameworkPlugin --> UiDebug
    UiFrameworkPlugin --> UiOverlayPlugin
    UiOverlayPlugin --> UiOverlayCommand
    UiOverlayCommand --> Overlays
    UiPanelCommand --> PanelManager
    PanelManager --> Panels
    Panels --> LayerRoots
    Overlays --> LayerRoots
    UiInput --> Panels
    UiFocus --> Panels
    UiDebug --> UiViewport
    UiDebug --> UiInput
    UiDebug --> Panels
```

## 代码入口

- `project/src/game/screens/mod.rs`：注册 `NavigationPlugin`、`UiFrameworkPlugin` 和各业务页面插件。
- `project/src/game/navigation/mod.rs`：定义 `AppUiMode` 和 `RouteButton`。
- `project/src/game/navigation/widgets.rs`：定义游戏层路由按钮和 `game_panel_root` 适配 helper。
- `project/src/game/ui_ids.rs`：集中定义游戏层 panel、owner、modal 和 action ID 常量。
- `project/src/framework/ui/core/framework.rs`：统一注册 UI 框架插件。
- `project/src/framework/ui/core/`：视口、层级、面板、输入、焦点、绑定、动画、统计。
- `project/src/framework/ui/widgets/`：通用控件、布局 helper、滚动容器、图片 helper。
- `project/src/framework/ui/overlays/`：Toast、Loading、Confirm modal 和覆盖层命令处理。
- `project/src/framework/ui/style/`：字体加载、作用域样式、主题 token、受限视觉效果和材质策略。
- `project/src/framework/ui/i18n.rs`：UI 文案加载、fallback 和热更新。

## 文档维护规则

修改 `project/src/framework/ui/`、`project/src/game/screens/` 的 UI 结构、输入规则、主题资源、i18n 资源、窗口验收方式或 Android UI 行为时，需要同步检查本目录。文档应描述已经存在的机制和限制，不应写成待办清单。
