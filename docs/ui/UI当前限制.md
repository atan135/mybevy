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
- Loading 和 Confirm 目前有入场 alpha；Loading panel 叠加 scale pulse，Confirm panel 叠加 scale 入场。关闭时仍直接 despawn，没有等待退出动画。
- Toast 不进入 Panel Manager，因此不参与 `CloseTop`。
- 当前只支持一个固定 `UI_PANEL_CONFIRM_MODAL` id；并发多个 Confirm 需要扩展 id 或栈语义。

## 数据绑定

- `UiBindingValues` 是简单 path -> text/bool 资源，不是 typed model。
- 不支持表达式绑定、列表绑定、双向绑定、批量 diff、作用域模型或生命周期自动回收。
- 绑定按钮禁用态通过插入/移除 `DisabledButton` marker 实现，复杂业务状态仍应由业务系统管理。

## 作用域样式

- 当前是受限的 token/variant 系统，不是 CSS、完整设计语言或任意属性反射。只支持 Surface、Border、Text、Button、Input、Card、Dialog 已声明字段。
- variant 名和 token 名来自主题配置，运行时业务使用类型化 role；动态未知 variant 会回退基础 role，并在 `UiResolvedStyleDebugSnapshot` 报错，不会即时创建样式。
- 同实体组合多个 composite role 时按 `Surface/Border/Text -> Button/Input -> Card -> Dialog` 提交。优先拆到合理的父子节点，避免依赖重叠优先级设计页面。
- scope 继承按实际 Bevy 父子层级每帧解析。大量绑定节点会增加 CPU 与 audit metadata 体积；当前没有增量依赖图或跨 World 样式缓存。
- resolved component 只拥有样式字段，不拥有 Interaction、焦点、选择、禁用、loading、输入值或文本内容。业务直接修改 resolved component 会在下次解析时被覆盖。
- 作用域 variant 当前不直接覆盖效果 preset；阴影、渐变、独立四边框和轮廓通过同级 `UiEffectBinding` 选择。通用属性动画由独立命令/player 管理，不属于样式继承。

## 动画

- 通用轨道支持 alpha、视觉/布局位置、尺寸、缩放和背景/文字颜色；当前没有旋转、关键帧序列、轨道组 barrier、弹簧或物理曲线。
- `Alpha` 只写目标实体实际的 BackgroundColor 或 TextColor，不是子树继承 opacity。
- ContinueFromCurrent 只读取 px 布局值和 px `UiTransform.translation`；Percent/Auto 会稳定拒绝。
- 页面切换和 Panel owner 清理优先立即释放实体、输入和焦点，不等待整页退出动画。
- LayoutPosition/LayoutSize 会触发 Bevy 重排，只适合少量且确需布局参与的节点；常驻视觉 loop 应使用 `UiTransform`。

## 表单与文本输入

- IME composition 展示仍不完整。
- 剪贴板当前是内部 UI 文本输入 clipboard，不是完整系统剪贴板桥接。
- 不支持 undo/redo、密码遮罩、复杂 selection 工具栏或富文本。
- 表单 helper 还不是完整 form container；没有统一 dirty/touched/cross-field validation 模型。
- 部分 helper/validation message 保存的是已解析字符串，locale 热更新后不一定自动刷新。

## 选择和数值控件

- Checkbox、Toggle、Segmented 已使用固定 box/mark、track/thumb 和 indicator 结构，并通过统一 `UiControlEvent` 输出根控件级 value；当前不包含三态 checkbox、Toggle 拖动手势或 Segmented 动态增删 option 动画。
- Slider/Stepper 已有数值模型和部分交互 helper，但完整拖拽、点击、键盘、业务事件协议仍需在具体页面验证。

## 图标与可访问性

- 正式 PNG 图标已通过稳定 `UiIconId` 注册；当前不支持运行时 SVG、动态图集打包、任意业务路径或专用图标字体。
- tint 只支持白色透明底单色图标；全彩图片图标固定保留原色。需要多层独立着色的图标应拆成受控资源或等待专用模型。
- 纯图标按钮已通过隐藏 i18n 文本接入 Bevy 按钮 accessibility node；实际平台朗读仍受 Bevy/accesskit 与操作系统辅助技术环境影响，需在目标平台验收。
- Tooltip/Dropdown 已有公共 Panel/焦点/清理能力；当前没有 Tooltip 延迟、Dropdown 搜索/分组/多选/虚拟列表，超大数据集应使用业务列表页面。

## 字体和本地化

- 产品 UI family 当前只有 CJK Regular。Medium/Bold 的真实静态 face 仅存在于 Figtree Latin 开发 fixture；产品角色请求这些 weight 会明确 fallback 到 Regular，不做合成粗体。
- 产品字体声明 coverage 不覆盖扩展 CJK、emoji、日文假名、韩文和繁体专用扩展字形。无 coverage grapheme 会替换为 `?` 并暴露状态，而不是依赖 tofu。
- fallback 当前以整个 Text 节点为单位；框架不会自动把中英文或缺字内容拆成多字体 `TextSpan`。
- Bevy 0.18.1 的当前公共封装不支持字距 token、自动复杂富文本、文字沿路径或高级排版。`TextBounds` 也不是严格像素裁切保证。
- `UiRasterizedTextSpec` 只允许有可访问/i18n fallback 和明确来源的 `project/assets/ui/` 静态字图，不是动态文案替代品。
- i18n fallback 优先中文内置文案，适合当前开发期验证，但正式多语言发布前需要补齐每个 locale 的完整资源。

## 高保真视觉能力

- 图片 frame 已支持 `Natural`、`Stretch`、`Contain`、焦点 `Cover`、组合尺寸约束、圆角裁切和加载状态占位；高级 API 已支持九宫格、受预算约束的 X/Y/Both 平铺和正式图集帧描述。
- atlas frame 当前只允许 Stretch，不支持与 NineSlice/Tiled 组合；`original_size` 和 pivot 已进入正式数据描述与校验，但当前静态 UI helper 不负责按 pivot 进行动画定位。
- 高级图片必须来自可验证的首包/AssetServer 相对路径，不接受无路径程序化纹理；基础整图的程序化 handle 仍可使用 `ui_image`。
- `Failed` 和 `Invalid` 当前使用稳定颜色占位并暴露组件状态，尚无通用重试按钮、错误图标或面向玩家的错误文案协议。
- 阴影、线性背景/边框渐变、独立边宽/圆角、裁切和 Outline 已有受限 preset、组合校验和规划预算；当前不支持径向/锥形渐变、内阴影或基于内容自动生成效果。
- Bevy 0.18.1 的 `TextShadow` 只有单层颜色与偏移。文字多层、spread 和 blur 会显式失败，不会用重复文本节点伪装。
- 自定义材质当前只有 allowlist、参数/平台校验和可见 fallback，没有已交付 shader/adapter。所有材质样例都应显示降级结果，不能将其描述为真实材质渲染。
- draw-call 和 overdraw 字段是保守配置预算，不是目标 GPU 实测；移动端发布仍需要平台分析器和真机截图。
- Badge、Progress、Tab、Tooltip 和 Dropdown 已形成公共能力，并有类型化状态支持矩阵；Dropdown label/option 保存的是已解析字符串，运行时切换 locale 后需要业务更新 `UiDropdown` 模型或重建控件。通用属性动画已支持 transform、布局、alpha 和颜色，但图标按钮尚未内置旋转 loading 图标协议。
- 作用域样式只覆盖固定 role 和纯色/尺寸 token，不自动把任意 Bevy Node 字段转成主题属性；页面私有 transform、grid、margin 和业务状态仍由调用方拥有。
- 允许临时直接使用的 Bevy 原语必须附加 `UiDirectBevyVisual` marker；完整状态和判定规则见 [UI高保真视觉能力.md](UI高保真视觉能力.md)。

## 测试覆盖

- 许多核心 helper 有单元测试，包括 viewport、binding、focus、scroll、按钮视觉优先级、文本输入编辑等。
- Scroll 和复杂窗口布局仍主要依赖窗口级人工验收。
- Android 真机触控、输入法、字体渲染和安全区仍需要设备级验收。
