# UI 响应式布局与尺寸需求

## 1. 背景

当前 UI 框架已经具备 Panel、Overlay、Toast、Loading、Confirm、基础控件、焦点、调试面板和桌面设备分辨率模拟能力。Android 真机验收暴露出的主要问题不再只是单个控件交互，而是不同终端上的布局和尺寸适配：

- 桌面编辑器窗口、Android 手机竖屏、手机横屏、平板和桌面宽屏的逻辑宽高不同。
- 同一物理分辨率在不同 DPI / scale 下对应的 UI 逻辑尺寸不同。
- 控件在窄屏上容易横向溢出，在宽屏和平板上又容易过度拉伸。
- 按钮组、选择控件、数字控件、弹窗操作区和调试面板需要在不同宽度下切换排布。
- 控件相对父级的靠左、居中、靠右、拉伸等对齐规则需要明确表达。

因此后续 UI 框架需要把响应式能力作为基础设施，而不是在每个页面里手工判断窗口宽度。

## 2. 目标

- 使用逻辑尺寸作为 UI 判断依据，物理分辨率只影响清晰度和桌面预览窗口大小。
- 建立统一的 viewport、metrics 和 layout 上下文，让页面和控件读取同一套响应式参数。
- 将尺寸、排布、对齐拆成独立概念，避免混在页面代码中。
- 通用控件默认具备紧凑屏、普通屏、平板 / 桌面宽屏的适配策略。
- 页面根节点、弹窗、滚动区域和调试面板具备安全区、最大宽高和内部滚动能力。
- 桌面模拟命令可以覆盖 Android 真机常见逻辑尺寸，降低真机反复打包成本。

非目标：

- 不实现完整 CSS 引擎。
- 不做可视化布局编辑器。
- 不要求第一版支持所有复杂断点组合。
- 不把所有控件做全局等比例缩放。

## 3. 核心原则

1. 物理分辨率不直接决定 UI 尺寸。

   例如 Android 真机 `1280x2772`，density 约 `3.25`，UI 逻辑尺寸约为 `394x853`。按钮、字号、间距和断点应基于 `394x853` 判断，而不是基于 `1280x2772` 判断。

2. 逻辑尺寸决定布局。

   页面应该关心当前逻辑宽高、方向、宽度等级、高度等级和安全区，而不是设备型号。

3. 尺寸 token 决定控件大小。

   按钮高度、输入框高度、图标尺寸、字号、页面边距、控件间距都应来自统一 metrics，不在页面中散落硬编码。

4. 断点决定排布方式。

   手机竖屏优先单列、换行和纵向堆叠；平板和桌面优先多列、限制内容最大宽度和增加留白。

5. 对齐规则独立表达。

   一个控件相对父级靠左、居中、靠右或拉伸，应由布局 API 明确表达，不依赖临时 margin 或固定宽度。

6. 触控尺寸优先可用。

   触控设备上的主要可点击区域不应小于 44 到 48 逻辑像素等效尺寸。窄屏时应减少列数和间距，而不是压缩点击高度。

## 4. 术语

- 物理尺寸：设备真实像素，例如 `1280x2772`。
- 设备缩放：DPI / scale，例如 `3.25`。
- 逻辑尺寸：物理尺寸除以设备缩放，例如 `394x853`。
- 预览缩放：桌面窗口显示缩放，例如 `50%`，只影响桌面窗口大小，不改变 UI 逻辑排版。
- 断点：根据逻辑宽度和高度得到的布局等级。
- metrics：根据断点、方向和输入方式计算出的尺寸 token。
- 安全区：刘海、圆角、系统导航栏、状态栏等不可被关键 UI 覆盖的区域。

## 5. 视口上下文需求

新增或完善 `UiViewport` 资源，运行时由主窗口逻辑尺寸和平台信息更新。

建议字段：

```rust
pub struct UiViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub width_class: UiWidthClass,
    pub height_class: UiHeightClass,
    pub orientation: UiOrientation,
    pub input_mode: UiInputMode,
    pub safe_area: UiSafeArea,
}
```

宽度等级建议：

```text
Compact:  logical_width < 480
Medium:   480 <= logical_width < 840
Expanded: logical_width >= 840
```

高度等级建议：

```text
Short:    logical_height < 600
Regular:  600 <= logical_height < 1000
Tall:     logical_height >= 1000
```

方向建议：

```text
Portrait:  logical_height >= logical_width
Landscape: logical_width > logical_height
```

验收要求：

- 桌面模拟 `1280x2772`、scale `3.25` 时，视口判断为 `Compact + Tall + Portrait`。
- 桌面 `1280x720` 时，视口判断为 `Expanded + Regular + Landscape`。
- 平板横屏 profile 判断为 `Expanded + Regular/Tall + Landscape`。
- 窗口 resize 后，`UiViewport` 能更新，页面后续可以响应。

## 6. 尺寸体系需求

新增或完善 `UiMetrics` 资源，由 `UiViewport` 和主题 token 共同计算。

建议字段：

```rust
pub struct UiMetrics {
    pub page_padding: f32,
    pub panel_padding: f32,
    pub control_gap: f32,
    pub section_gap: f32,
    pub button_height: f32,
    pub input_height: f32,
    pub icon_size: f32,
    pub touch_target_min: f32,
    pub font_body: f32,
    pub font_button: f32,
    pub font_title: f32,
    pub content_max_width: f32,
    pub dialog_max_width: f32,
}
```

建议默认值：

```text
Compact:
- page_padding: 12-16
- panel_padding: 12-16
- control_gap: 8
- section_gap: 12-16
- button_height: 44-48
- input_height: 44-48
- icon_size: 20-24
- touch_target_min: 44-48
- content_max_width: unlimited 或 480
- dialog_max_width: logical_width - page_padding * 2

Medium:
- page_padding: 20-24
- panel_padding: 16-20
- control_gap: 10-12
- section_gap: 16-20
- button_height: 44-48
- input_height: 44-48
- content_max_width: 640-720
- dialog_max_width: 520-640

Expanded:
- page_padding: 24-32
- panel_padding: 20-24
- control_gap: 12
- section_gap: 20-24
- button_height: 40-44
- input_height: 40-44
- content_max_width: 840-960
- dialog_max_width: 560-720
```

尺寸缩放规则：

- 允许非常轻微的字体和间距缩放，例如 `0.92` 到 `1.12`。
- 不允许把所有 UI 按屏幕比例全局等比放大。
- 平板和桌面主要通过更宽内容区、更多列数和更大留白适配。
- 窄屏主要通过单列、换行、内部滚动和减少横向间距适配。

验收要求：

- 主要按钮在手机竖屏上高度不小于触控目标。
- 平板上按钮不会因屏幕宽而变成过宽的大块。
- 输入框、stepper、slider、icon button 的尺寸来自 metrics。
- 状态变化不会改变控件尺寸导致布局跳动。

## 7. 排布能力需求

布局组件或 builder 需要提供以下能力：

```text
Row: 横向排列
Column: 纵向排列
WrapRow: 横向排列，不够时换行
Grid: 按列数排列
Stack: 层叠排列，用于 overlay / badge / debug highlight
ScrollView: 可滚动内容区域
```

响应式排布建议：

```text
Compact:
- 页面主内容单列
- 表单控件拉伸到父级宽度
- 按钮组优先纵向堆叠或换行
- 选择控件单列或两列，按可读性决定
- 数字控件 label、slider、value、stepper 可分行

Medium:
- 页面主内容可居中并限制最大宽度
- 表单仍拉伸
- 控件组可两列
- 弹窗操作按钮可横向靠右，空间不足时换行

Expanded:
- 页面内容居中，使用最大宽度
- 列表、卡片、选择项可多列
- 工具栏靠左，主要操作靠右
- 调试面板和内容区可左右分栏
```

验收要求：

- UiGallery 在 `Compact` 宽度下无横向溢出。
- button group、selection controls、numeric controls、overlay action row 不再依赖固定横向空间。
- 页面高度不足时可以纵向滚动查看底部内容。
- 同一控件区域在平板横屏下能利用宽度，但不无限拉伸。

## 8. 对齐能力需求

对齐需要分三层表达。

### 8.1 父级对子级的对齐

建议提供：

```rust
pub enum UiJustify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

pub enum UiAlign {
    Start,
    Center,
    End,
    Stretch,
}
```

含义：

- 横向 Row 中，`justify` 控制横向分布，`align` 控制纵向对齐。
- 纵向 Column 中，`justify` 控制纵向分布，`align` 控制横向对齐。
- 使用 `Start / End`，不要直接使用 `Left / Right`，为未来 RTL 语言预留。

### 8.2 子级覆盖父级对齐

建议提供：

```rust
pub enum UiAlignSelf {
    Auto,
    Start,
    Center,
    End,
    Stretch,
}
```

典型场景：

- 表单整体 `Stretch`，但提交按钮 `End`。
- 标题靠左，状态标签靠右。
- 弹窗内容拉伸，底部按钮组靠右。

### 8.3 控件内部内容对齐

建议提供：

```rust
pub enum UiContentAlign {
    Start,
    Center,
    End,
}
```

默认策略：

```text
Button: Center
IconButton: Center
TextInput: Start
NumberInput: End 或 Center，按控件语义决定
FormLabel: Start
DialogTitle: Start
DialogActionRow: End，Compact 下可 Stretch
Toolbar: Start
```

验收要求：

- 页面可以明确表达内容区居中但内部表单拉伸。
- 弹窗按钮在宽屏上靠右，在窄屏上可拉伸或纵向堆叠。
- 输入框文本靠 Start，按钮文本居中。
- 不通过魔法 margin 实现关键对齐。

## 9. 通用控件适配需求

### 9.1 Button / IconButton

- 使用 metrics 的高度、最小宽度、gap 和字体。
- Compact 下普通按钮默认可 `Stretch`。
- 宽屏下按钮可以保持内容宽度，不无限拉满。
- IconButton 保持稳定方形尺寸，不被普通按钮 min width 影响。
- loading / disabled / focused / selected 状态不改变尺寸。

### 9.2 TextInput

- 高度来自 metrics。
- 文本靠 Start。
- 支持选区背景色和文字色，不使用额外字符表达选中状态。
- 光标和选区变化不应引起边框或布局跳动。
- Compact 下输入框默认拉伸。

### 9.3 Selection Controls

- checkbox、toggle、segmented control 在 Compact 下支持单列或换行。
- selected 状态必须有明确背景、边框或图标差异。
- segmented control 空间不足时可换行或切换为纵向。

### 9.4 Slider / Stepper

- slider 的 track 宽度根据父级自适应。
- 点击或拖动 slider 应按控件局部坐标计算准确 value。
- Compact 下 label、track、value 可以分行。
- stepper 的 `- value +` 应保持一组，不在窄屏下被拆散。
- stepper value 区域应有稳定最小宽度，避免数值变化导致按钮跳动。

### 9.5 Dialog / ConfirmPanel

- dialog 宽度使用 `min(logical_width - safe_area - padding, dialog_max_width)`。
- Compact 下 action buttons 可纵向或换行，并保持按钮背景统一。
- Medium / Expanded 下 action row 默认靠 End。
- 内容高度超过限制时，正文区域内部滚动，标题和 action row 保持可见。

### 9.6 DebugPanel

- 主窗口内显示时不遮挡全部关键内容。
- 独立窗口显示时宽度可根据内容自适应，但仍有最大宽度。
- 文本区域有最大高度并内部滚动，打开时优先看到关键摘要。
- 支持冻结、复制、过滤和滚动查看详细信息。

## 10. 页面级适配需求

页面根节点默认策略：

```text
Compact:
- root padding 使用 safe area + page padding
- content width: 100%
- content max width: none 或 480
- vertical scroll enabled

Medium:
- root padding 增大
- content width: 100%
- content max width: 640-720
- content align: Center

Expanded:
- root padding 更大
- content max width: 840-960
- content align: Center
- 可按页面需要分栏
```

页面必须避免：

- 用固定物理像素宽度模拟手机 UI。
- 在窄屏中使用不可换行的横向按钮组。
- 让卡片或按钮横向溢出屏幕。
- 平板和桌面上把所有控件横向拉满到屏幕边缘。

## 11. 安全区和平台适配需求

需要支持：

- Android 刘海、圆角、状态栏、导航栏预留。
- 桌面窗口无安全区时安全区为 0。
- 全屏和窗口模式都能正确计算。
- 关键按钮、返回、确认、取消不放到安全区外。
- overlay 遮罩可以覆盖全屏，但内容区域要避开安全区。

第一版可以先提供 `UiSafeArea` 资源和手动 / 平台默认值，后续再接 Android 原生 inset。

## 12. 滚动和裁剪需求

滚动区域需要成为框架基础能力：

- 页面主内容可以纵向滚动。
- 弹窗正文可以内部滚动，标题和底部按钮固定。
- DebugPanel 文本详情可以内部滚动。
- 滚动区域应拦截 pointer，避免滚动时触发底层玩法触控。
- 触控拖动滚动和 slider 拖动需要有明确手势归属，避免互相抢输入。

验收要求：

- 手机竖屏下 UiGallery 可以滚动到底部。
- ConfirmPanel 内容过长时不会把按钮挤出屏幕。
- DebugPanel 内容过长时可以上下滑动或内部滚动查看。
- 滚动时不会触发 Touch Ripple 玩法输入。

## 13. 桌面模拟验收矩阵

每轮响应式改动至少用以下命令检查：

```powershell
Set-Location project
cargo run -- --window-size 1280x2772 --window-scale 0.5
cargo run -- --window-profile phone-small --window-scale 0.75
cargo run -- --window-profile phone-1080p --window-scale 0.5
cargo run -- --window-profile tablet-portrait --window-scale 0.5
cargo run -- --window-profile tablet-landscape --window-scale 0.5
cargo run -- --window-profile desktop
```

重点检查：

- 登录页、Lobby、UiGallery、Overlay 示例。
- 按钮组是否换行或堆叠。
- selection controls 是否有选中差异且不溢出。
- numeric controls 是否可读、可点、可拖。
- ConfirmPanel action row 是否统一按钮背景。
- DebugPanel 是否可读、可滚动、不遮挡关键路径。

## 14. 真机验收矩阵

Android 真机至少检查：

- 首屏启动后 UI 是否在安全区内。
- Touch Ripple 玩法触控是否只在未命中 UI 时触发。
- slider 点击和拖动是否和手指位置一致。
- stepper 加减按钮是否可点击且不误触。
- 文本输入是否可聚焦，软键盘或外部输入是否符合当前能力边界。
- Back 键优先关闭 overlay / modal，再回到上层 mode。
- F3 / F4 / F8 调试能力在真机上可用。

## 15. 建议实施拆分

建议按以下小任务串行实现：

1. 引入 `UiViewport` 和 `UiMetrics` 资源。
2. 将主题 token 与 metrics 结合，统一按钮、输入框、icon button、slider、stepper 的基础尺寸。
3. 增加通用 layout builder：Row、Column、WrapRow、Grid、ScrollView 的响应式参数。
4. 改造 UiGallery 的 button group、selection controls、numeric controls。
5. 改造 ConfirmPanel、Loading、DebugPanel 的最大宽高、内部滚动和 action row 对齐。
6. 增加安全区资源和页面 root padding 策略。
7. 补充桌面模拟验收记录和必要单元测试。

## 16. 完成标准

- 常见手机竖屏逻辑宽度下没有横向溢出。
- 平板和桌面宽屏下内容不会无限拉伸。
- 主要控件尺寸来自统一 metrics。
- 页面、弹窗、调试面板在高度不足时有合理滚动。
- 对齐方式可以通过 API 明确表达，而不是靠临时 margin。
- 桌面模拟和 Android 真机看到的布局接近一致。

## 17. 当前实现状态与开发验收记录

截至第 10 节收尾，响应式 UI 基础已按分节开发接入：

- `UiViewport` / `UiMetrics` 已在 `project/src/game/ui/core/viewport.rs` 实现，并由主窗口逻辑尺寸和主题 token 更新。
- 宽度分级当前为 `Compact < 480`、`Medium < 840`、`Expanded >= 840`。
- 高度分级当前实现为 `Short < 600`、`Regular < 800`、`Tall >= 800`。本文前文建议值曾写 `Tall >= 1000`，实际实现按开发验收用例 `394x853 => Compact + Tall + Portrait` 收敛为 800 阈值。
- `Button`、`IconButton`、`TextInput`、`Slider`、`Stepper` 的基础尺寸已改为优先来自 `UiMetrics` 或 metrics 派生值。
- 通用 layout helper 已提供响应式 row / column / wrap row / grid / content container / action row，以及 `UiJustify`、`UiAlign`、`UiAlignSelf`、`UiContentAlign` 等对齐表达。
- `UiGallery` 的按钮组、图标按钮组、selection controls、numeric controls、overlay action row 和 stress grid 已接入响应式布局 helper。
- `ConfirmPanel`、`Loading overlay`、`Toast`、`FloatingPanel`、`DebugPanel` 已接入 metrics / safe area / 最大宽高 / 内部滚动中的相应能力。
- 页面 root 已接入 `UiViewport` safe area 与 `UiMetrics.page_padding` 合成 padding；桌面和当前 Android 第一版 safe area 值仍为 0，Android 原生 inset 后续再接。
- ScrollView helper 已统一 `UiScrollView`、`ScrollPosition`、拖动起点组件和 `Pickable` 阻挡策略，避免外部手写滚动节点时遗漏基础组件。
- DebugPanel 已显示 viewport / metrics 摘要，便于验收当前 logical size、width / height class、orientation、content / dialog max width。

仍需用户或主 agent 后续手动自测：

- 用第 13 节桌面 profile 矩阵检查 Login、Lobby、UiGallery、Overlay、DebugPanel 的真实视觉表现。
- Android 真机检查刘海 / 圆角 / 状态栏 / 导航栏安全区；当前代码只保留 `UiSafeArea` 结构和 padding 合成能力，未接 Android 原生 inset。
- 检查 slider 拖动与 ScrollView 触控拖动在真机上的手势归属；当前通过滚动 helper 和输入阻挡降低误触，但未做完整手势仲裁重写。
- 检查长 Confirm 正文、长 DebugPanel 内容、Toast 顶部位置、FloatingPanel 边距和关闭路径。
