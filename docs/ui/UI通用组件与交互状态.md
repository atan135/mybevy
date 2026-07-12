# UI 通用组件与交互状态

通用组件实现在 `project/src/framework/ui/widgets/controls/components.rs` 和 `selection.rs`，Tooltip/Dropdown 的覆盖层实现在 `project/src/framework/ui/overlays/popover.rs`。业务页面应组合这些公共 API，不要依赖控件内部 child entity、内部 marker 层级或 Gallery 私有结构。

## 公共组件

- Badge：`badge_key`，用于短状态标签，不可交互。
- Progress：`progress_key`，值域规范化到 `0..=1`，提供 loading、empty 和 error 终态。
- Tab：`tab_list` + `tab_key`，选中时同一 TabList 内只保留一个 selected 项。
- Checkbox：`checkbox_key` / `checked_checkbox_key` / `disabled_checkbox_key`。
- Toggle：`toggle_key` / `toggle_on_key` / `disabled_toggle_key`。
- Segmented：`segmented_control` + segment option helper。
- Tooltip：在可聚焦 Button 上组合 `tooltip_target`。
- Dropdown：`dropdown_key` + `UiDropdownOption`。

Checkbox 使用固定 box/mark 子节点，Toggle 使用固定 track/thumb，Segmented 使用固定 indicator。状态变化只修改颜色、可见性和 Toggle thumb 的视觉位置，不修改根节点点击区域或布局尺寸；文案不再拼接 `[x]`、`ON/OFF` 等文本符号。

## 状态支持矩阵

`UiControlKind::supports_state` 是类型化支持矩阵。`UiControlState` 的九个状态不是要求每类控件伪造全部语义：

| 组件 | 支持状态 |
| --- | --- |
| Badge | normal、selected、disabled、loading、empty、error |
| Progress | normal、disabled、loading、empty、error |
| Tab | normal、hovered、pressed、focused、selected、disabled、loading |
| Tooltip | normal、disabled、error |
| Dropdown | 九态全部支持 |
| Checkbox / Toggle / Segmented | 除 empty 外的八态 |

交互控件的视觉优先级固定为：

```text
disabled > loading > error > pressed > hovered > selected > focused > empty > normal
```

`Interaction` 提供 pressed/hovered，`FocusedButton` 提供 focused，`UiControlFlags` 是 selected/disabled/loading/empty/error 的根状态真值。交互 consumer 会直接拒绝 disabled/loading flags；`sync_control_gate_markers` 再把它们同步成焦点和 pointer 系统已有的 `DisabledButton` / `LoadingButton`。真实 Dropdown 选择会在同一提交中同步 selected index、flags 和兼容 marker。

## 稳定事件

业务监听 `UiControlEvent`，不要扫描 `UiCheckboxChecked`、`UiToggleThumb` 或 Dropdown option 子实体。事件包含：

- `entity`：业务持有的根控件实体；Dropdown option 选择仍返回 Dropdown 根实体。
- `owner`：显式 `UiControlOwner`，未声明时回退当前 `UiCurrentOwner`。
- `control_id`：稳定 `UiControlId`；带 `_key` 的选择控件以 i18n key 作为 ID。
- `control_kind`：Tab、Dropdown、Checkbox、Toggle 或 Segmented 等类型。
- `kind`：`ValueChanged`、`Opened` 或 `Closed`。
- `value`：`Bool`、`Text` 或 `None`。
- `reason`：`Pointer`、`Keyboard`、`ClickAway`、`Escape` 或 `OwnerRemoved`。

Checkbox/Toggle/Segmented/Tab 只在有效 `Click` 后发送 `ValueChanged`。Dropdown 打开时发送 `Opened`；选择 option 发送 `ValueChanged` 和 `Closed`。点击外部、Escape/BrowserBack 和 anchor 销毁分别产生对应关闭原因。

## Tooltip 与 Dropdown

两个组件都通过 Panel Manager 打开，固定 panel id 分别是 `UI_PANEL_TOOLTIP` 和 `UI_PANEL_DROPDOWN`，layer 是 `UiLayer::Floating`，当前 `ZIndex(120)`。

Tooltip 根节点忽略 picking，不中断 owner 的 hover；hover、pressed 或键盘 focus 均可打开。Tooltip 没有可聚焦子项，因此不会从页面或 Modal 抢走焦点。

Dropdown 使用全屏透明 dismiss surface 阻断下层 picking，popup body 会阻止内部点击被识别为 click-away。option 列表使用 `UiScrollView`，支持滚轮/触控拖动；禁用 option 不进入焦点候选。键盘支持 Tab、Shift+Tab、上下方向、Home、End、Enter/Space 和 Escape/BrowserBack。

Popover 每帧根据 anchor 的 `ComputedNode` 和 `UiGlobalTransform` 更新位置：优先显示在下方，空间不足时翻转到上方，并在左右和安全区边界内 clamp。Dropdown 宽度至少覆盖 anchor，受 220 到 420 逻辑像素及当前 viewport 可用宽度约束。Dropdown trigger 始终保持主题按钮高度；长 label 使用单行字素簇安全省略和独立 clip frame，不会扩大点击区域。option panel 内的长 label 仍允许换行。

打开在 Modal 上方的 Dropdown 是最近打开的 transient panel，焦点限制到 option；选择、click-away 或 Escape 关闭后，在 trigger 仍可聚焦时稳定恢复到 trigger。Escape 只有在 Dropdown 确实是 Panel Manager 的 CloseTop 目标时才报告 Dropdown Closed；更晚打开的 transient 或 blocking overlay 在上方时不会误报。不可交互 Tooltip 不改变 Modal 的焦点范围。

## 生命周期和失败边界

- 同类 Tooltip/Dropdown 同时只保留一个；新请求替换旧 panel。
- anchor despawn 后，Popover 在清理系统中关闭并发送 `OwnerRemoved`。
- 页面切换仍由 `CloseAllForOwner` 同步清理，不等待动画。
- disabled/loading/empty/error Dropdown 不打开 option panel。
- Dropdown label 和 option 接收已经解析的字符串；业务运行时切换 locale 后如需更新模型文案，应更新 `UiDropdown` 数据或重建控件。通用 loading/empty/error 文案由 i18n 初始化。
- Tooltip/Dropdown 当前没有延迟打开、搜索过滤、分组、多选或虚拟列表；大量 option 应使用业务列表页面。

## Gallery 与审计

UI Gallery 的 `ui_gallery.components*` child anchor 覆盖公共组件支持矩阵、正式选择控件结构和长 Dropdown option。`components` 捕获静态上半区；`component_checkboxes`、`component_toggles`、`component_segmented` 分别捕获三类选择控件的完整状态；`component_overlays` 和 `component_tooltip` 分别确定性打开 Dropdown 与 Tooltip。审计 metadata 的 `control_snapshots` 按 control ID 稳定排序，记录 kind、解析状态和 flags。
