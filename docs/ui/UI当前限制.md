# UI 当前限制

本页记录当前实现边界。它不是开发任务清单，而是使用 UI 框架时需要明确规避或验证的限制。

## 输入与焦点

- `UiInputState` 是全局摘要，不是完整 hit-test、捕获、冒泡或手势仲裁系统。
- 当前阻断依据主要是 blocking/modal panel、文本输入焦点、按钮 pressed/hovered、滚动区域 hovered。
- 复杂嵌套控件、长按、双指手势、拖拽和 gameplay 同时竞争的场景仍需扩展输入路由。
- 焦点候选当前以 `Button + FocusableButton` 为主，非按钮控件需要通过按钮实体或额外扩展进入焦点系统。

## 安全区与平台适配

- `UiSafeArea` 已有结构和 padding helper，但 `platform_safe_area()` 当前返回零值。
- Android 刘海、状态栏、导航栏 inset 尚未接入原生数据。
- Android 设备级安装、触控、字体和 soft keyboard 验收依赖 `adb` 或真机环境；没有设备时只能完成构建和桌面模拟。

## 覆盖层

- Toast replacement 会立即 despawn 旧 Toast，没有旧 Toast 的替换出场动画。
- Loading 和 Confirm 目前有入场 alpha 动画，关闭时直接 despawn，没有退出动画。
- Toast 不进入 Panel Manager，因此不参与 `CloseTop`。
- 当前只支持一个固定 `UI_PANEL_CONFIRM_MODAL` id；并发多个 Confirm 需要扩展 id 或栈语义。

## 数据绑定

- `UiBindingValues` 是简单 path -> text/bool 资源，不是 typed model。
- 不支持表达式绑定、列表绑定、双向绑定、批量 diff、作用域模型或生命周期自动回收。
- 绑定按钮禁用态通过插入/移除 `DisabledButton` marker 实现，复杂业务状态仍应由业务系统管理。

## 表单与文本输入

- IME composition 展示仍不完整。
- 剪贴板当前是内部 UI 文本输入 clipboard，不是完整系统剪贴板桥接。
- 不支持 undo/redo、密码遮罩、复杂 selection 工具栏或富文本。
- 表单 helper 还不是完整 form container；没有统一 dirty/touched/cross-field validation 模型。
- 部分 helper/validation message 保存的是已解析字符串，locale 热更新后不一定自动刷新。

## 选择和数值控件

- Checkbox、Toggle、Segmented 当前更接近静态状态 builder，加视觉同步；没有统一选择事件协议。
- Checkbox/Toggle 视觉仍是按钮式表达，不是原生 checkbox 或 switch track。
- Slider/Stepper 已有数值模型和部分交互 helper，但完整拖拽、点击、键盘、业务事件协议仍需在具体页面验证。

## 图标与可访问性

- 图标按钮当前使用文本符号，不是 icon atlas、SVG 或专用图标字体。
- 可访问 label 只保存在组件数据和文本状态中，还没有接入平台 accessibility bridge。
- Tooltip 系统尚未形成通用能力。

## 字体和本地化

- 当前 UI 字体只有 regular 字重。
- 字体子集不覆盖扩展 CJK、emoji、日文、韩文和繁体专用字形。
- i18n fallback 优先中文内置文案，适合当前开发期验证，但正式多语言发布前需要补齐每个 locale 的完整资源。

## 高保真视觉能力

- 图片 frame 已支持 `Natural`、`Stretch`、`Contain`、焦点 `Cover`、组合尺寸约束、圆角裁切和加载状态占位；高级 API 已支持九宫格、受预算约束的 X/Y/Both 平铺和正式图集帧描述。
- atlas frame 当前只允许 Stretch，不支持与 NineSlice/Tiled 组合；`original_size` 和 pivot 已进入正式数据描述与校验，但当前静态 UI helper 不负责按 pivot 进行动画定位。
- 高级图片必须来自可验证的首包/AssetServer 相对路径，不接受无路径程序化纹理；基础整图的程序化 handle 仍可使用 `ui_image`。
- `Failed` 和 `Invalid` 当前使用稳定颜色占位并暴露组件状态，尚无通用重试按钮、错误图标或面向玩家的错误文案协议。
- 阴影、渐变和复杂描边尚无共享 token、组合校验和移动端降级规则。
- 通用属性动画、正式图片图标、状态贴图、Badge、Progress、Tab、Tooltip 和下拉选择尚未形成公共能力。
- 允许临时直接使用的 Bevy 原语必须附加 `UiDirectBevyVisual` marker；完整状态和判定规则见 [UI高保真视觉能力.md](UI高保真视觉能力.md)。

## 测试覆盖

- 许多核心 helper 有单元测试，包括 viewport、binding、focus、scroll、按钮视觉优先级、文本输入编辑等。
- Scroll 和复杂窗口布局仍主要依赖窗口级人工验收。
- Android 真机触控、输入法、字体渲染和安全区仍需要设备级验收。
