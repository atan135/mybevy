# UI 覆盖层与弹窗

覆盖层实现位于 `project/src/framework/ui/overlays/`，并由 `UiOverlayPlugin` 和 `UiPanelPlugin` 统一调度。当前覆盖层包括 Toast、Loading、Confirm modal、Floating panel、Tooltip 和 Dropdown。

## Toast

Toast 使用 `UiOverlayCommand::ShowToast(UiToast)` 打开，不进入 Panel Manager。

行为：

- 新 Toast 打开前会立即关闭所有旧 Toast。
- 默认持续时间是 2.4 秒，最小持续时间是 0.1 秒。
- fade in 默认最长 0.14 秒，fade out 默认最长 0.2 秒，短 Toast 会按持续时间一半裁剪。
- 位于 `UiLayer::Toast`，当前 `ZIndex(200)`。
- 位置在屏幕顶部居中，并叠加 `metrics.page_padding` 和 `safe_area.top`。

适用场景是短暂状态提示，不适合承载操作按钮或长文本。

## Loading

Loading 使用 `UiPanelCommand::Open(UiPanelRequest::Loading(UiLoading))` 打开。

行为：

- 固定面板 id 是 `UI_PANEL_GLOBAL_LOADING`。
- kind 是 `UiPanelKind::BlockingOverlay`。
- layer 是 `UiLayer::Loading`，当前 `ZIndex(150)`。
- 根节点是全屏 `Button`，用于参与 picking 和阻断下层交互。
- `UiLoading.cancellable` 决定 `CloseTop` 是否能关闭它。
- 入场有 alpha，内部 panel 叠加 `TransformScale` pulse；Reduced/Disabled motion policy 下循环会静止并清理。关闭时当前直接 despawn，没有出场动画。

不可取消 Loading 会消费 `Escape` / `BrowserBack` 的关闭意图，但不会关闭自己。

## Confirm Modal

Confirm 使用 `UiPanelCommand::Open(UiPanelRequest::Confirm(UiConfirmModal))` 打开。

行为：

- 固定面板 id 是 `UI_PANEL_CONFIRM_MODAL`。
- kind 是 `UiPanelKind::Modal`。
- layer 是 `UiLayer::Modal`，当前 `ZIndex(100)`。
- 弹窗外层是全屏遮罩，内部面板最大宽度来自 `metrics.dialog_max_width`。
- 正文区域使用滚动容器，最大高度由 dialog 宽度推导并 clamp。
- 动作按钮按 `UiModalActionStyle::Primary/Secondary` 使用主/次按钮色。
- 动作按钮发生 `UiButtonEvent::Click` 后会发送 `UiModalResult { id, action }`，然后关闭 Confirm。
- 入场有 alpha，内部 panel 叠加有限 `TransformScale` 入场；关闭时当前直接 despawn，没有等待出场动画。

Confirm 打开后，焦点候选会限制在该 modal 内，下层页面按钮不应响应。

## Floating Panel

Floating 使用 `UiPanelCommand::Open(UiPanelRequest::Floating(UiFloatingPanel))` 打开。

行为：

- id 由 `UiFloatingPanel.id` 指定，例如游戏层的 `PANEL_GALLERY_FLOATING`。
- kind 是 `UiPanelKind::Floating`。
- layer 是 `UiLayer::Floating`，当前 `ZIndex(80)`。
- 位置靠右上，叠加 `metrics.page_padding` 和安全区。
- 宽度不超过 `metrics.dialog_max_width`、420 和可用宽度。
- 紧凑屏最大高度比例更低，避免遮挡过多页面。

Floating 不铺满屏幕，也不是阻断 overlay。它会参与 CloseTop，但不会像 Modal 一样强制限制所有焦点到自己内部，除非它是当前最高可聚焦 panel。

## CloseTop 行为

`Escape` 和 `BrowserBack` 由 Panel Manager 转成 `UiPanelCommand::CloseTop`。关闭顺序：

1. 可取消的 `BlockingOverlay`。
2. 最近打开的 transient panel，按真实打开顺序在 `Modal` 与 `Floating` 间选择。

Toast 不参与 CloseTop。Page/HUD 不参与 CloseTop。

这个顺序保证 Modal 内随后打开的 Dropdown 会先于底层 Modal 关闭；如果 Floating 先打开、Modal 后打开，则仍先关闭 Modal。

## Tooltip 和 Dropdown

Tooltip 和 Dropdown 固定使用 `UI_PANEL_TOOLTIP` / `UI_PANEL_DROPDOWN`，位于 `UiLayer::Floating` 和 `ZIndex(120)`。同类新请求会替换旧 panel。

Tooltip 根节点忽略 picking，不打断 owner hover，也没有焦点候选。Dropdown 使用全屏透明 dismiss surface 阻断下层交互，popup body 和 option 自己接收点击；点击外部、Escape/BrowserBack、选择 option 或 anchor despawn 都有明确关闭路径。option 列表使用框架滚动容器。

Popover 根据 anchor 的最终布局矩形定位，优先下方、空间不足时翻转上方，并 clamp 到安全区和 viewport。Dropdown 在 Modal 上方打开时成为最高可聚焦 panel；选择、Escape 或 click-away 关闭后优先恢复 trigger，不会跳到底层任意按钮。Escape Closed 事件以 Panel Manager 的真实 CloseTop 目标为准。详细事件 reason、支持状态和失败边界见 [UI通用组件与交互状态.md](UI通用组件与交互状态.md)。

## owner 清理

通过 Panel Manager 打开的覆盖层会带当前 `UiCurrentOwner` 作为 `owner`。页面模式切换时，游戏层 `NavigationPlugin` 先发送 `CloseAllForOwner(current_mode.ui_owner())`，清理当前 owner 拥有的覆盖层，再进入目标模式。

## 动画

当前 UI 动画核心在 `project/src/framework/ui/core/animation.rs`。覆盖层组合兼容 alpha 与通用 transform 轨道：

- Toast：入场和生命周期末尾出场。
- Loading：alpha 入场和不触发布局重排的 scale pulse。
- Confirm：alpha 与 scale 入场。

Loading 和 Confirm 的关闭路径仍是直接 despawn，没有专门的退出动画。

所有轨道受全局 `UiMotionPolicy` 控制。主题热更新会在主题值应用后取消在途轨道；页面切换的 `CloseAllForOwner` 仍立即 despawn，不因动画延迟 owner 清理、焦点释放或输入解除。完整规则见 [UI动画与动态效果.md](UI动画与动态效果.md)。
