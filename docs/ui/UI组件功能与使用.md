# UI 组件功能与使用

通用控件集中在 `project/src/framework/ui/widgets/`。业务页面优先使用这些 helper 生成一致的节点、主题 marker、焦点 marker 和 i18n marker。

## 文本

常用 helper 位于 `controls.rs`：

- `screen_title`
- `screen_title_key`
- `screen_label`
- `screen_label_key`

带 `_key` 的版本会同时生成 `UiI18nText`，在语言资源热更新后自动刷新文本。没有 i18n 需求的内部调试文本可以直接使用非 key 版本。

## 按钮

按钮 helper 分为普通动作按钮和游戏层路由按钮：

- `primary_route_button_key`
- `secondary_route_button_key`
- `primary_action_button_key`
- `secondary_action_button_key`
- `disabled_*_button_key`
- `loading_*_button_key`

`project/src/framework/ui/widgets/` 提供通用动作按钮外观、焦点和交互状态；业务系统需要自行监听 `Interaction::Pressed`。`primary_route_button_key` 和 `secondary_route_button_key` 位于 `project/src/game/navigation/widgets.rs`，它们是在通用动作按钮上组合 `RouteButton { target }` 的游戏层 helper，由 `NavigationPlugin` 处理页面切换。

按钮视觉优先级固定为：

```text
disabled > loading > pressed > hovered > selected > focused > normal
```

相关 marker：

- `PrimaryButton` / `SecondaryButton`
- `DisabledButton`
- `LoadingButton`
- `SelectedButton`
- `FocusableButton`
- `FocusedButton`

## 图标按钮

图标按钮 helper：

- `icon_button_key`
- `disabled_icon_button_key`
- `loading_icon_button_key`

当前图标使用文本符号，不是图集或矢量图标系统。`UiIconButton` 会保存可访问标签 key、fallback 和解析后的 label，i18n 更新后同步 accessible label。

## 选择控件

当前选择类控件以按钮视觉为基础：

- Checkbox：`UiCheckbox`、`UiCheckboxChecked`
- Toggle：`UiToggle`、`UiToggleOn`
- Segmented：`UiSegmentedControl`、`UiSegmentOption`、`UiSegmentOptionSelected`

它们当前主要是静态状态 builder 和视觉同步。业务如果需要真正切换状态，需要监听按钮交互并自行更新对应 marker。

## 数值控件

`UiSlider` 和 `UiStepper` 提供数值模型和展示同步：

- Slider 会规范化 min/max、clamp value，并把 value 映射为填充比例。
- Stepper 会规范化 min/max/step，并支持加减后 clamp。
- 显示文本由同步系统根据组件值刷新。

当前已有 slider 从 normalized x 映射 value 的 helper，以及 stepper 加减 helper。完整拖拽/点击/键盘事件协议仍属于轻量实现，业务使用前应在 UI Gallery 和目标窗口尺寸下验证交互。

## 输入框

输入框相关组件：

- `UiTextInput`
- `UiTextInputValue`
- `UiTextInputCursor`
- `UiTextInputMaxChars`
- `UiTextInputPlaceholder`
- `UiTextInputHelperText`
- `UiTextInputRequired`
- `UiTextInputAlphanumeric`
- `UiTextInputValidationMessage`
- `ReadonlyTextInput`
- `DisabledTextInput`

支持的行为包括字符插入、删除、光标移动、选区显示、最大字符数、只读、禁用、必填校验和字母数字校验。错误态边框和辅助文案通过同步系统刷新。

## 布局 helper

`layout.rs` 提供：

- `ui_column`
- `ui_row`
- `ui_wrap_row`
- `ui_grid`
- `ui_responsive_row`
- `ui_responsive_column`
- `ui_responsive_wrap_row`
- `ui_responsive_grid`
- `ui_content_container`
- `ui_action_row`
- `ui_metrics_scroll_column`

优先用 `UiMetrics` 推导间距、最大宽度和按钮高度，不要在业务页面散落一套新的尺寸 token。

## 滚动容器

`scroll.rs` 提供：

- `ui_scroll_column`
- `ui_scroll_column_with_max_height`
- `ui_scroll_column_bundle`
- `UiScrollViewConfig`

默认滚动容器会阻断下层 picking。弹窗正文、长列表和调试内容优先用这些 helper。

## 图片

UI 图片 helper 位于 `widgets/image.rs`，示例资源位于：

- `project/assets/ui/images/`
- `project/assets/ui/atlas/`

当前 UI Gallery 展示首包图片和图集源图，图集源图只是普通 PNG 展示，不是正式图集帧预览。

## 数据绑定

绑定核心在 `project/src/framework/ui/core/binding.rs`：

- `UiBindingValues` 保存 path -> text/bool。
- `UiBoundText` 把文本节点绑定到 text path。
- `UiBoundVisibility` 把节点可见性绑定到 bool path。
- `UiBoundDisabled` 把按钮禁用状态绑定到 bool path。

绑定 path 使用点分隔，并会去除段前后空白，例如 `" gallery . binding . status "` 会规范化为 `"gallery.binding.status"`。

这套绑定适合 UI 示例、简单状态展示和轻量开关，不是完整 MVVM 或表单模型。
