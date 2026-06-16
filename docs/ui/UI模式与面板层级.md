# UI 模式与面板层级

UI 模式、面板身份和层级标记共同决定当前页面显示什么、覆盖层如何打开关闭，以及输入焦点被限制在哪个区域。

## 页面模式

`project/src/game/navigation/mod.rs` 定义 `AppUiMode`：

- `Login`
- `Lobby`
- `WanfaTouchRipple`
- `UiGallery`

桌面开发时可通过环境变量 `TOUCH_START_SCREEN` 指定启动页面。当前支持的值包括：

- `login`
- `lobby`、`game_list`、`game-list`、`list`
- `ui_gallery`、`ui-gallery`、`gallery`
- `wanfa_touch_ripple`、`wanfa-touch-ripple`、`touch`、`touch_ripple`、`touch-ripple`

页面插件通常在 `OnEnter(AppUiMode::...)` 生成页面根节点，在退出时清理自己拥有的实体。

## Panel 标记

面板根节点使用 `UiPanelRoot`：

```rust
pub struct UiPanelRoot {
    pub id: UiPanelId,
    pub kind: UiPanelKind,
    pub owner_mode: Option<AppUiMode>,
}
```

`UiPanelId` 当前包含：

- `LoginPage`
- `GameListPage`
- `UiGalleryPage`
- `GalleryFloating`
- `TouchRippleHud`
- `TouchRipplePause`
- `TouchRippleSettings`
- `GlobalLoading`
- `ConfirmModal`

`UiPanelKind` 当前包含：

- `Page`：普通页面。
- `Hud`：玩法 HUD。
- `Floating`：不会铺满屏幕的浮动面板。
- `Modal`：需要用户处理的弹窗。
- `BlockingOverlay`：全屏阻断覆盖层，例如 Loading。

`owner_mode` 用于 `CloseAllForMode(AppUiMode)` 清理当前模式持有的面板，避免切换页面后旧覆盖层残留。

## Layer 标记

`UiLayerRoot` 使用 `UiLayer` 标记视觉层：

- `Page`
- `Floating`
- `Modal`
- `Loading`
- `Toast`
- `Debug`

实际绘制顺序还依赖 Bevy UI 层级和 `ZIndex`。当前覆盖层实现中常见 Z 值为：

- Floating：`ZIndex(80)`
- Modal：`ZIndex(100)`
- Loading：`ZIndex(150)`
- Toast：`ZIndex(200)`
- Debug：调试层单独管理，支持主窗口或专用窗口。

## Panel 命令

`UiPanelCommand` 是面板管理入口：

- `Open(UiPanelRequest)`：打开 Loading、Confirm 或 Floating。
- `Close(UiPanelId)`：按 id 关闭。
- `Toggle(UiPanelRequest)`：存在则关闭，不存在则打开。
- `Hide(UiPanelId)` / `Show(UiPanelId)`：只修改 `Visibility`。
- `CloseTop`：关闭当前最上层可关闭面板。
- `CloseAllForMode(AppUiMode)`：关闭属于指定模式的面板。

`UiPanelRequest` 当前支持：

- `Loading(UiLoading)`：生成 `GlobalLoading`，kind 是 `BlockingOverlay`。
- `Confirm(UiConfirmModal)`：生成 `ConfirmModal`，kind 是 `Modal`。
- `Floating(UiFloatingPanel)`：生成业务指定 id，kind 是 `Floating`。

## CloseTop 语义

`Escape` 或 `BrowserBack` 会写入 `UiPanelCommand::CloseTop`。关闭优先级固定为：

1. 如果存在 `BlockingOverlay`，且它的 `UiBlockingOverlay.cancellable == true`，关闭它；如果不可取消，只消费关闭意图。
2. 否则关闭最近打开的 `Modal`。
3. 否则关闭最近打开的 `Floating`。

`Page` 和 `Hud` 不会被 `CloseTop` 关闭。

## 路由切换语义

`UiRouteCommand::ChangeMode(target)` 会先发送 `CloseAllForMode(current_mode)`，然后设置 `NextState<AppUiMode>`。这意味着属于旧页面的 Loading、Confirm、Floating 会随模式切换清理。

`UiRouteCommand::ShowToast(toast)` 不进入 Panel Manager。它会直接关闭当前所有 Toast 并生成新 Toast。

## 使用约束

业务页面如果需要全局覆盖层，应通过 `UiPanelCommand` 或 `UiRouteCommand` 发消息，不应直接生成全局 Loading、Confirm 或 Toast。业务页面自己的 Page/HUD 根节点应带 `UiPanelRoot`，否则调试面板、输入阻断和焦点限制无法完整识别它。
