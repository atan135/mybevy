# UI 输入路由与焦点

当前输入层的目标是让 UI 和玩法输入互不抢夺：当用户正在操作 UI、滚动区域、输入框或被弹窗阻断时，玩法侧应能通过 `UiInputState.pointer_blocked` 判断是否忽略触控/鼠标输入。

## 输入状态资源

`project/src/framework/ui/core/input.rs` 维护 `UiInputState`：

```rust
pub struct UiInputState {
    pub pointer_blocked: bool,
    pub focused_panel: Option<UiPanelId>,
    pub top_blocking_panel: Option<UiPanelId>,
    pub pointer_block_reason: String,
    pub route_summary: String,
    pub route_history: VecDeque<UiInputRouteHistoryEntry>,
}
```

`UiInputPlugin` 在 `Update` 阶段刷新它，执行顺序在 `UiPanelSystems::Commands` 和 `UiFocusSystems::SyncFocusedMarkers` 之后。

## 阻断优先级

当前阻断原因按以下优先级解析：

1. 存在 `BlockingOverlay` 或 `Modal`。
2. 当前焦点实体是 `UiTextInput`。
3. 存在被按下的 `Button`。
4. 存在 hover 的 `Button`。
5. 存在 hover 的 `UiScrollView`。
6. 无阻断。

只要命中任一原因，`pointer_blocked` 就是 `true`。`route_history` 只记录状态变化，最多保留最近 12 条，用于 F3 调试面板观察输入路由变化。

## 焦点系统

`project/src/framework/ui/core/focus.rs` 维护 `UiFocusState`：

```rust
pub struct UiFocusState {
    pub focused_entity: Option<Entity>,
}
```

焦点候选必须满足：

- 是 Bevy `Button`。
- 带 `FocusableButton`。
- 不带 `DisabledButton`、`DisabledTextInput`、`LoadingButton`。
- 继承可见性不是 hidden。

用户点击可聚焦按钮时会设置焦点。按 `Tab` / `Shift+Tab` 会在候选之间循环移动。

## 弹窗内焦点限制

`BlockingOverlay` 始终拥有最高焦点优先级。其后在 `Modal` 与包含可聚焦候选的 `Floating` 之间按最终 ZIndex 选择最上层 panel；没有这些 panel 时，焦点优先落在层级最高且包含按钮的普通 panel 内。

这个规则避免弹窗打开后 Tab 跳到下层页面按钮，同时允许 Modal 上方随后打开的 Dropdown 接管 option 焦点。没有焦点候选的 Tooltip 不会抢走 owner 或 Modal 焦点。

## 键盘激活

`Enter` 和 `Space` 会临时把当前焦点按钮的 `Interaction` 设置为 `Pressed`，并写出 `UiButtonEvent::Down`、`UiButtonEvent::Click`、`UiButtonEvent::Up`。带 `UiTextInput` 的实体不会通过这个路径触发按钮动作，避免输入框焦点误触发提交。

## 滚动协作

`UiScrollView` 位于 `project/src/framework/ui/widgets/scroll.rs`：

- 滚轮事件根据 hover map 发送到当前 hover 链路上的实体。
- 垂直滚动使用 Bevy `ScrollPosition` 和 `ComputedNode` 计算最大偏移。
- 按住 Ctrl 时滚轮 x/y 会交换，用于横向滚动。
- 拖拽滚动会记录起始 `ScrollPosition`，再按 pointer drag 距离更新。
- `UiScrollViewConfig.should_block_lower` 默认是 `true`，会通过 `Pickable` 阻断下层 hover。

Dropdown option 列表复用同一 `UiScrollView`，而 Dropdown 的全屏 dismiss surface 会阻断底层页面和底层滚动容器；popup body 内滚动只作用于 option 列表。

## 文本输入

输入框由 `UiTextInput`、`UiTextInputValue`、`UiTextInputCursor` 等组件组成。支持：

- 键盘输入、Backspace、Delete、Home、End、左右移动。
- 鼠标/触控按压后根据相对位置移动光标。
- 选区显示。
- `UiTextInputMaxChars` 限制字符数。
- `ReadonlyTextInput` 允许聚焦和移动光标，但不编辑。
- `DisabledTextInput` 不编辑也不移动光标。
- `UiTextInputSubmitted` 消息。
- Android 下的 native text input 同步路径。

## 当前边界

输入路由是全局摘要，不是完整 hit-test、捕获、冒泡、手势竞争系统。它能解决当前页面/弹窗/按钮/滚动/文本输入与玩法触控之间的互斥，但不能表达复杂嵌套控件的精细事件传播。

需要更细粒度行为时，应先扩展 `UiInputState` 的信号和解析规则，再让业务使用新的稳定字段，不要在玩法系统里直接扫描任意 UI 节点。
