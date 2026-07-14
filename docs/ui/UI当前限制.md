# UI 当前限制

本页记录当前实现边界。它不是开发任务清单，而是使用 UI 框架时需要明确规避或验证的限制。

## 输入与焦点

- `UiInputState` 是全局摘要，不是完整 hit-test、捕获、冒泡或手势仲裁系统。
- 当前阻断依据主要是 blocking/modal panel、文本输入焦点、按钮 pressed/hovered、滚动区域 hovered。
- 复杂嵌套控件、长按、双指手势、拖拽和 gameplay 同时竞争的场景仍需扩展输入路由。
- 焦点候选当前以 `Button + FocusableButton` 为主，非按钮控件需要通过按钮实体或额外扩展进入焦点系统。

## 安全区与平台适配

- `UiSafeArea` 已接 Android `WindowInsetsCompat -> JNI -> UiSafeAreaStatus` 生产链，覆盖系统栏、display cutout 和手势区域；首个原生回调前仍会以 `unavailable + zero` 启动。
- 桌面 phone/tablet profile 的 inset 是确定性 fixture，不代表 OEM cutout、手势导航或三键导航实测。
- Android 设备级安全区、触控、字体、图片切片、效果降级和 soft keyboard 仍依赖 `adb` 或真机环境。当前开发环境没有可用 `adb`，正式未验条件见 [UI安全区与视觉预算.md](UI安全区与视觉预算.md)。

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
- audit 的图片解码内存、render primitive、draw-call、材质和 overdraw 字段是 ECS/资源快照上的开发期估算或规划上界，不是目标 GPU 的 VRAM、batch 或像素实测；移动端发布仍需要平台分析器和真机截图。
- Badge、Progress、Tab、Tooltip 和 Dropdown 已形成公共能力，并有类型化状态支持矩阵；Dropdown label/option 保存的是已解析字符串，运行时切换 locale 后需要业务更新 `UiDropdown` 模型或重建控件。通用属性动画已支持 transform、布局、alpha 和颜色，但图标按钮尚未内置旋转 loading 图标协议。
- 作用域样式只覆盖固定 role 和纯色/尺寸 token，不自动把任意 Bevy Node 字段转成主题属性；页面私有 transform、grid、margin 和业务状态仍由调用方拥有。
- 允许临时直接使用的 Bevy 原语必须附加 `UiDirectBevyVisual` marker；完整状态和判定规则见 [UI高保真视觉能力.md](UI高保真视觉能力.md)。

## 测试覆盖

- 许多核心 helper 有单元测试，包括 viewport、binding、focus、scroll、按钮视觉优先级、文本输入编辑等。
- Scroll 和复杂窗口布局仍主要依赖窗口级人工验收。
- Android 真机触控、输入法、字体渲染和安全区仍需要设备级验收。

## 声明式预览

- 声明式 diff 能稳定区分原位字段、子树和整页重建，但当前实际 commit 统一使用隐藏新树 + 原子 replace；尚未开放绕过完整校验的 ECS 局部 patch。
- 本地 document watch 只在 desktop debug 构建中可显式启用，release/Android 默认且强制关闭；它不是生产内容更新通道。
- reload 状态迁移覆盖 ID/kind/owner 兼容的 focus、输入值/光标/selection、scroll、数值与选择控件状态；不迁移 IME composition、native keyboard session、动画播放头或任意业务 component，未迁移的已识别状态会写入 decision reason。
- 自动生成的 document audit recipe 按 `(document_id, owner)` 隔离且只来自 preview registration；直接 Stage 10 `Open` 不生成 recipe，页面能否由审计 runner 进入仍需要游戏层路由注册。
- 详细边界和命令见 [UI声明式预览与热更新.md](UI声明式预览与热更新.md)。

## AI 参考图生成

- 独立 `tools/ui-generation/` 工具工程已实现 Stage 1 严格任务输入、参考图 bytes/SHA-256 校验、结构化缺失问题、run 目录只读规划、任务状态/取消，以及结构化 Cargo manifest/path 依赖图与 lockfile 的离线边界检查；当前不会创建实际 run 目录。
- Stage 2 已提供供应商无关的视觉分析/结构化生成请求、凭据读取与脱敏边界、超时/取消/限速/有限重试 runner、capability 检查以及文本 Fixture/Mock provider；请求中的 prompt、图片 bytes 和结构化输入不能作为普通 metadata 序列化。
- Stage 3 已提供受限 PNG/JPEG 解码、EXIF/ICC/alpha 记录、八方向归一化、显式裁切/安全区/系统 UI 排除、五坐标系往返、确定性 PNG 下采样、独立辅助图和版本化原子缓存；所有产物仍是被忽略的生成期证据。
- 预处理不会猜测区域，也不执行未经验证的 ICC 色彩转换；每张参考图最多允许 64 个显式系统 UI 排除区。页面类参考图的“空白”判定只覆盖无可见像素或 visible RGBA 全通道变化不超过 2 的页面，纯色 detail 仍允许。当前只支持 PNG/JPEG，编码上限 64 MiB、单边上限 16384 px、解码像素上限 2400 万。
- 在线 provider 适配、结构化视觉分析协议、`UiDocument` 生成/修复/评测、预览接入和 `promote` 命令尚未实现，当前不能从参考图产出或晋升正式页面。
- 生成工具属于桌面/CI 开发工具，拥有独立 Cargo 根，只能单向依赖 `project::framework::ui::document::tooling`，不进入 `project` 正式依赖图，不注册进 `UiFrameworkPlugin`，也不由 Android `cargo ndk ... --lib` 构建。
- 原始参考图、模型响应、日志、草稿、source map 和生成期素材规划保存在被忽略的 `summary/ui-generation/<run-id>/`，不能写入 `project/assets/` 或随正式包交付。
- 只有通过 Schema、语义、能力、action/binding、资源预算和授权检查，并经过人工批准与显式晋升的 `UiDocument` JSON、授权资源和必要确定性注册适配，才属于正式游戏内容并随桌面和 Android 包交付。
- 页面主体规划为声明式 JSON；未知业务 action/binding 必须阻塞晋升，模型不能生成任意 Rust 业务实现。完整设计边界见 [UI参考图生成与正式包边界.md](UI参考图生成与正式包边界.md)。
- 参考图与渲染结果的视觉相似度审核属于后续独立工作，当前生成边界和现有 preview 不能替代该验收。
