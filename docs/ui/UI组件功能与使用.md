# UI 组件功能与使用

通用控件集中在 `project/src/framework/ui/widgets/`。业务页面优先使用这些 helper 生成一致的节点、主题 marker、焦点 marker 和 i18n marker。

## 文本

常用 helper 位于 `controls.rs`：

- `screen_title`
- `screen_title_key`
- `screen_label`
- `screen_label_key`

带 `_key` 的版本会同时生成 `UiI18nText`，在语言资源热更新后自动刷新文本。没有 i18n 需求的内部调试文本可以直接使用非 key 版本。

四个 helper 当前都通过 `UiTextStyleToken` 解析字体角色、family、weight、字号、行高、对齐、换行和截断，并附加 `UiFontResolution`。旧代码仍可读取 `UiFontAssets.regular`，但新增公共文本不应绕过角色注册表。

需要自定义排版时使用 `try_ui_styled_text`。它会在生成 bundle 前验证 token；ellipsis 按 grapheme cluster 截断。`Clip` 必须通过 `try_ui_text_clip_frame(width, height)` 建立固定父 frame，并把 no-wrap Text 放在其下；Bevy 不会用 Text 自身的 overflow 裁切自身字形。不要把 `TextBounds` 的非严格高度截断误当可靠 ellipsis。字体 fallback 是整节点语义，不是逐字 fallback。

UI Gallery 的 `typography` state 展示全部主题文字角色和 Figtree fixture 的真实 400/500/700 face；`typography_overflow` 展示中英文/数字/标点混排、长英文单词、超长中文、clip、ellipsis、对齐和 missing-glyph 显式替换。

## 按钮

按钮 helper 分为普通动作按钮和游戏层路由按钮：

- `primary_route_button_key`
- `secondary_route_button_key`
- `primary_action_button_key`
- `secondary_action_button_key`
- `disabled_*_button_key`
- `loading_*_button_key`

`project/src/framework/ui/widgets/` 提供通用动作按钮外观、焦点、交互状态和 `UiButtonEvent`。按钮事件至少包含：

- `Down`：指针或键盘在按钮上按下，适合做即时视觉和捕获类反馈。
- `Up`：一次按钮按下结束。
- `Click`：有效点击，按下和释放都落在同一按钮上；普通业务动作应监听这个事件。
- `Cancel`：指针取消或交互被中断。

`primary_route_button_key` 和 `secondary_route_button_key` 位于 `project/src/game/navigation/widgets.rs`，它们是在通用动作按钮上组合 `RouteButton { target }` 的游戏层 helper，由 `NavigationPlugin` 在 `Click` 时处理页面切换。

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
- `icon_label_button_key`
- `image_button_key`

图标由 `widgets/icon.rs` 的 `UiIconId` 解析为正式 PNG，不允许传任意路径或用文本符号替代。`UiIconDescriptor` 记录路径、96 x 96 源尺寸、默认逻辑尺寸和 tint policy；`UiIconResolutionStatus` 与 `UiIconAssetStatus` 分别暴露解析结果和加载状态。未知 ID、非法路径或加载失败会显示 `UiIconId::MISSING`，不会用空图片或字体 tofu 隐藏错误。

`icon_button_key` 是固定触控尺寸的纯图标按钮；`icon_label_button_key` 通过 `UiIconLabelPlacement::Leading/Trailing` 生成左右图标文字；`image_button_key` 显式声明固定按钮宽高和图片尺寸。纯图标与图片按钮保存隐藏的 i18n label，组合按钮使用可见 label，二者都会让 Bevy 按钮 accessibility node 取得可访问名称。

`UiIconButtonVisuals` 可按 idle、hovered、pressed、focused、selected、disabled、loading 覆盖图标 ID、tint 或背景。默认 tint 集中在主题 `colors.icon_tint`，优先级与普通按钮一致；状态系统只修改 `ImageNode`、解析/加载状态和背景，不修改根节点尺寸、点击区域或子层级。单色白色透明图标使用 `MonochromeTintable`；`FullColor` 会忽略 tint 并固定以白色乘色渲染。

正式资源、固定上游版本、许可和 SHA-256 见 `project/assets/ui/icons/manifest.ron`。UI Gallery 使用 `icons` 和 `icon_states` 两个 child-anchor state 验收 API 形态与七态矩阵。

## 选择控件

当前选择类控件以按钮视觉为基础：

- Checkbox：`UiCheckbox`、`UiCheckboxChecked`
- Toggle：`UiToggle`、`UiToggleOn`
- Segmented：`UiSegmentedControl`、`UiSegmentOption`、`UiSegmentOptionSelected`

它们当前以 `UiButtonEvent::Click` 切换状态，并同步对应 marker。`Down` 只影响按钮 pressed 视觉，不会提交选择变化。

## 数值控件

`UiSlider` 和 `UiStepper` 提供数值模型和展示同步：

- Slider 会规范化 min/max、clamp value，并把 value 映射为填充比例。
- Stepper 会规范化 min/max/step，并支持加减后 clamp。
- 显示文本由同步系统根据组件值刷新。

当前已有 slider 从 normalized x 映射 value 的 helper，以及 stepper 加减 helper。Slider 在按下/拖动中持续更新；Stepper 在 `Click` 时单步加减。完整拖拽、长按连续加减和业务事件协议仍属于轻量实现，业务使用前应在 UI Gallery 和目标窗口尺寸下验证交互。

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

图片使用外层 frame 和内层 image 两层结构：

- `ui_image_panel_node`：建立尺寸约束和矩形裁切 frame。
- `ui_image_panel_node_with_radius`：额外建立圆角裁切；圆角和 `Overflow::clip()` 同属 frame。
- `ui_image`：建立图片节点，并通过 `UiImageFit` 选择 `Natural`、`Stretch`、`Contain` 或 `Cover`。
- `try_ui_advanced_image`：接收 `&AssetServer` 和 `UiAdvancedImageSpec` 建立九宫格、受预算约束的平铺或正式图集帧；先完成所有校验，再由 spec path 加载唯一实际 handle，非法组合不会注册加载或生成 bundle。

`Cover` 使用 `UiImageFocus` 控制裁切焦点。坐标基于源图左上角归一化，有限值会 clamp 到 `0..=1`，非有限值会产生 `Invalid` 状态。`UiImageSize::Constrained` 与 `UiImageConstraints` 支持固定/百分比/自动轴、宽高比和 min/max 的组合；调用 `validate` 或 `try_to_node` 可以在生成页面前取得 `UiImageError`。

每个 `ui_image` 实体都有可查询的 `UiImageStatus`：`Loading`、`Ready`、`Failed`、`Invalid`。加载中、加载失败和非法配置使用不同的稳定占位色，frame 尺寸不依赖失败纹理。页面不要自行查询源图尺寸或计算 `ImageNode.rect`。

高级描述均可通过 serde/RON 持久化。`UiNineSlice` 分别声明四边 insets、中心/边缘 Stretch 或 Tile、角块最大缩放和生成 slice 预算；`UiImageTiling` 声明 X/Y/Both、重复阈值和总预算；`UiAtlasFrame` 声明权威源纹理路径/尺寸、像素 rect、原始尺寸及可选归一化 pivot。框架拒绝非有限值、资源路径或尺寸不一致、越界帧、超预算和 atlas + slice/tile 组合，不会依赖 Bevy 的静默 clamp 或降级；高级入口当前明确拒绝空路径的程序化图片。

当前 UI Gallery 展示全部四种适配模式、横竖 frame、Cover 两端焦点，以及真实九宫格面板/多尺寸按钮边框、X/Y/Both 平铺和四个精确图集帧。

高保真视觉 fixture 位于 `project/assets/ui/fixtures/`，清单见 `manifest.ron`，来源和许可见 `LICENSES.md`。UI Gallery 的首个固定区域可通过 `image_fit` 验收图片适配，或通过兼容 state `visual_foundation` 验收整个 fixture 区域；`image_modes`、`image_tiling`、`image_atlas` 会分别滚动到稳定 child anchor 验收九宫格、平铺和图集帧。fixture 只用于框架验收，不是正式业务资源。

视觉能力状态、Direct Bevy 逃生口和非目标见 [UI高保真视觉能力.md](UI高保真视觉能力.md)。

## 数据绑定

绑定核心在 `project/src/framework/ui/core/binding.rs`：

- `UiBindingValues` 保存 path -> text/bool。
- `UiBoundText` 把文本节点绑定到 text path。
- `UiBoundVisibility` 把节点可见性绑定到 bool path。
- `UiBoundDisabled` 把按钮禁用状态绑定到 bool path。

绑定 path 使用点分隔，并会去除段前后空白，例如 `" gallery . binding . status "` 会规范化为 `"gallery.binding.status"`。

这套绑定适合 UI 示例、简单状态展示和轻量开关，不是完整 MVVM 或表单模型。
