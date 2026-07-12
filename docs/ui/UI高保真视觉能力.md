# UI 高保真视觉能力

本文定义参考图复刻时使用的视觉能力分类、支持状态和逃生口。它描述当前公共能力，不是阶段任务清单。能力实现变化时，需要同步更新本文、UI Gallery 样例和对应测试。

## 支持状态

| 状态 | 稳定 ID | 判定标准 | 页面侧规则 |
| --- | --- | --- | --- |
| 框架支持 | `framework_supported` | `project/src/framework/ui/` 已提供稳定数据模型或 helper、运行时行为和已知失败边界，并有测试或 Gallery 样例 | 业务页面只组合公共能力，不复制底层计算或状态优先级 |
| 允许直接使用 Bevy | `direct_bevy_allowed` | Bevy 0.18.1 已有足够稳定的原语，但框架尚未形成可复用约定；该用法能限制在单个页面或试验区域 | 根实体必须附加 `UiDirectBevyVisual::new(capability, reason)`，并在页面测试或 Gallery 中留有可复现样例 |
| 暂不支持 | `unsupported` | Bevy 原语、平台能力、性能预算或当前框架边界不足以给出可验证结果 | 不得静默近似；应调整设计、使用有明确许可的资源化结果，或等待公共能力补齐 |

判定顺序固定为：先查公共 API，再判断是否满足受控的 Direct Bevy 条件，最后才归为暂不支持。`direct_bevy_allowed` 是可追踪的临时边界，不等于框架兼容承诺。

视觉能力和状态的稳定代码模型位于 `project/src/framework/ui/visual.rs`。Direct Bevy 逃生口示例：

```rust
commands.spawn((
    bevy_visual_bundle,
    UiDirectBevyVisual::new(
        UiVisualCapability::Shadow,
        "screen-local comparison before a shared shadow token exists",
    ),
));
```

`reason` 不能为空。暂不支持的能力不能通过该 marker 绕过边界。

## 能力矩阵

| 分类 | 当前能力 | 状态 | 当前边界或公共入口 |
| --- | --- | --- | --- |
| 布局 `layout` | 响应式列、网格、滚动、视口分类和安全区 padding | 框架支持 | `widgets/layout.rs`、`widgets/scroll.rs`、`core/viewport.rs` |
| 布局 `layout` | 页面专属绝对定位或 Transform 组合 | 允许直接使用 Bevy | 仅限局部组合，必须标记；不得复制通用响应式计算 |
| 文字 `typography` | family/weight/role/face/coverage 注册、真实 Latin 400/500/700 fixture、整节点 fallback、行高/对齐/换行、clip/grapheme ellipsis 和 i18n 刷新 | 框架支持 | `style/fonts.rs`、`style/theme.rs`、`i18n.rs`；产品 CJK 当前仅 Regular |
| 文字 `typography` | 自动逐字 fallback、字距、复杂富文本、路径文字和高级排版 | 暂不支持 | 不依赖 tofu 或 `TextBounds` 冒充；受控静态字图必须通过 `UiRasterizedTextSpec` 校验 |
| 图片 `image` | `Natural`、`Stretch`、`Contain`、焦点 `Cover`、组合尺寸约束和加载状态占位 | 框架支持 | `widgets/image.rs`；页面使用 frame + image helper，不复制适配或裁切计算 |
| 切片 `slice` | 九宫格边距、中心/边缘缩放策略、X/Y/双向平铺和重复预算 | 框架支持 | `widgets/image.rs`；由可序列化描述校验后映射到 `NodeImageMode::Sliced` / `Tiled` |
| 图片 `image` | 图集源纹理、像素帧、原始尺寸和归一化 pivot 描述 | 框架支持 | `UiAtlasFrame` + `try_ui_advanced_image`；越界或不支持的切片组合在生成 bundle 前返回错误 |
| 图标 `icon` | 稳定图标 ID、正式 PNG、默认尺寸、单色着色边界、加载状态和缺失图标占位 | 框架支持 | `widgets/icon.rs`；业务只传 `UiIconId`，不得直接拼接资源路径或用 Unicode 文本冒充图标 |
| 图标 `icon` | 纯图标、左右图标文字和固定尺寸图片按钮 | 框架支持 | `widgets/controls/button.rs`；根节点保存 i18n 可访问名称，状态切换不改变布局或点击区域 |
| 表面 `surface` | 主题色驱动的纯色页面、面板和按钮背景 | 框架支持 | `UiThemeBackgroundRole`；新页面可用 `UiStyleBinding` + 最近祖先 scope |
| 表面 `surface` | 页面/子树 variant、嵌套继承和移除后恢复 | 框架支持 | `UiStyleScope`，解析顺序为 base -> request variant -> 根到最近 scope |
| 边框 `border` | 统一边宽、纯色边框和圆角 | 框架支持 | `UiThemeBorderRole`；作用域使用类型化 Border role |
| 边框 `border` | 独立四边宽、独立四角圆角、轮廓及圆角裁切组合 | 框架支持 | `style/effects.rs` 的受限 preset 映射到 `Node` / `Outline`，并保存解绑 baseline |
| 阴影 `shadow` | 最多三层 `BoxShadow`；单层原生 `TextShadow` | 框架支持 | `UiEffectBinding`；文字 blur/spread/多层因 Bevy 0.18.1 不支持而显式报错 |
| 渐变 `gradient` | 背景和边框线性渐变、角度、2 至 6 个有序 RGBA 色标 | 框架支持 | `UiEffectBinding` 映射到 `BackgroundGradient` / `BorderGradient`；有限值和预算先校验 |
| 表面 `surface` | 任意自定义 `UiMaterial` | 暂不支持 | 仅允许框架 allowlist + 类型化 adapter；当前登记的策略项没有 shader/adapter，始终使用可见 fallback |
| 动画 `animation` | alpha、视觉/布局位置、尺寸、缩放和颜色属性轨道 | 框架支持 | `core/animation.rs`；统一 spec、消息/事件、打断、完成、主题刷新和 motion policy |
| 动画 `animation` | 页面、控件、弹窗和 loading 的组合过渡 | 框架支持 | 视觉移动/缩放优先 `UiTransform`；布局 target 是显式 opt-in，详见 `UI动画与动态效果.md` |
| 动画 `animation` | 关键帧序列、弹簧/物理曲线和旋转轨道 | 暂不支持 | 不在页面私建与公共命令冲突的第二套 player |
| 控件状态 `control_state` | 按钮 idle/hovered/pressed/focused/selected/disabled/loading 视觉优先级 | 框架支持 | `widgets/controls/button.rs` |
| 控件状态 `control_state` | Checkbox、Toggle、Segmented 的当前轻量状态结构 | 框架支持 | 当前仍是按钮式视觉，限制见 `UI当前限制.md` |
| 控件状态 `control_state` | 图标按钮状态贴图和 tint/background override | 框架支持 | `UiIconButtonVisuals`；复用 `Interaction` 和既有 focus/selected/disabled/loading marker |
| 控件状态 `control_state` | Badge、Progress、Tab、Tooltip 和下拉选择 | 暂不支持 | 不在业务页面建立私有状态优先级协议 |

## 稳定验收区域

UI Gallery 的第一个内容面板是固定的 `visual foundation` 区域，代码 marker 为 `GalleryVisualFoundationRegion`。该区域展示已实现的图片适配矩阵和可追溯 fixture，不把未来能力伪装成已实现能力。

- 页面：`ui_gallery`，别名 `ui-gallery`、`gallery`
- 图片适配审计 state：`image_fit`
- fixture 兼容审计 state：`visual_foundation`
- 九宫格审计 state：`image_modes`
- 平铺审计 state：`image_tiling`
- 图集帧审计 state：`image_atlas`
- 字体角色和字重审计 state：`typography`
- 混排和溢出审计 state：`typography_overflow`
- 图标与图片按钮审计 state：`icons`
- 图标七态矩阵审计 state：`icon_states`
- 作用域样式审计 state：`style_scopes`
- 阴影、渐变和材质降级审计 state：`effects`
- 通用属性动画与动态策略审计 state：`animations`
- 滚动目标：`ui_gallery.main`
- 图片适配位置：主滚动容器顶部
- 高级图片 anchor：`ui_gallery.image_modes`
- 平铺/图集 anchor：`ui_gallery.image_tiling`、`ui_gallery.image_atlas`
- 文字 anchor：`ui_gallery.typography`、`ui_gallery.typography_overflow`
- 图标 anchor：`ui_gallery.icons`、`ui_gallery.icon_states`
- 样式 anchor：`ui_gallery.style_scopes`
- 效果 anchor：`ui_gallery.effects`
- 动画 anchor：`ui_gallery.animations`
- fixture 清单：`project/assets/ui/fixtures/manifest.ron`
- 正式图标清单：`project/assets/ui/icons/manifest.ron`

批量 runner 的 `-States auto` 会为 UI Gallery 选择 `image_fit,visual_foundation,image_modes,image_tiling,image_atlas,typography,typography_overflow,icons,icon_states,style_scopes,effects,animations,middle,bottom`。`image_fit` 和 `visual_foundation` 固定指向顶部区域；高级图片、文字、图标、作用域样式、效果和动画 state 根据命名 child anchor 计算逻辑滚动偏移，不依赖页面总高度。审计应用第一个 Gallery capture state 时会统一冻结动画样例，避免布局循环影响任何后续 state。仍可显式请求兼容 state `top`。

```powershell
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States visual_foundation -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States image_fit -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States image_modes -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States "image_tiling,image_atlas" -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States "typography,typography_overflow" -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States "icons,icon_states" -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States style_scopes -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States effects -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States animations -DryRun
```

## 动画规则

通用动画按实体和实际写通道仲裁。同一 target 会 replace，互不重叠的 target 可并行；Alpha 与具体颜色 target、通用颜色轨道与旧 alpha 的重叠会稳定拒绝。纯视觉移动和缩放使用 `UiTransform`，布局 position/size 只用于兄弟节点必须参与重排的少量场景。

`UiMotionPolicy` 提供 Full、Reduced 和 Disabled。Disabled 当帧到达真实最终播放方向的端点；Reduced 加速有限动画并让无限循环静止。主题热更新取消在途轨道并由新主题值接管。audit metadata 的 `motion_policy` 与 `animation_snapshots` 记录固定进度、pause、target 和 layout reflow 标记；完整语义见 [UI动画与动态效果.md](UI动画与动态效果.md)。

## 阴影、渐变和材质规则

主题 version 1 的可选 `effects` 定义具名 preset。页面只放 `UiEffectBinding::new("preset.id")`；盒阴影、文字阴影、背景/边框线性渐变、独立边宽/圆角、裁切和 Outline 由同一个编译结果应用到真实 Bevy 组件。非法配置拒绝整次主题更新，未知运行时 preset 使用明显轮廓 fallback，不 panic 或空白。

自定义材质路径永远不来自页面或 RON。框架 allowlist 固定材质 ID、shader 路径、参数数量与平台；GPU 不支持、平台不支持、shader 未注册/加载中/失败、adapter 缺失和参数非法都会应用 preset 声明的纯 UI fallback。当前没有交付真实自定义材质，因此 Gallery 不宣称 shader 渲染成功。

每个效果快照进入 audit metadata 的 `effect_resolutions`，包含最终组件、fallback reason 和保守的 draw-call/overdraw 规划预算。完整数值边界、移动端建议和降级顺序见 [UI视觉效果与材质边界.md](UI视觉效果与材质边界.md)。

## 作用域样式规则

主题 version 1 的可选 `styles` 定义 Color/Scalar token、可继承 variant 和类型化 override。Surface、Border、Text、Button、Input、Card、Dialog 七类 role 可以按节点职责组合；未知 token/variant、循环、重复、类型错误和非法值在应用前失败，热更新保留 last-known-good 主题。

每个绑定节点的 `UiResolvedStyleDebugSnapshot` 记录 scope、来源链和最终关键 token，并进入 audit metadata 的 `style_resolutions`。普通按钮、图标按钮和输入框只消费 resolved token，仍使用原有唯一状态机，不让主题刷新改写 Interaction、focused/selected/disabled/loading 或输入值。完整 schema 和优先级见 [UI主题字体与国际化.md](UI主题字体与国际化.md)。

## 图标与图片按钮规则

`UiIconId` 是业务与资源路径之间的稳定边界。注册项声明固定相对路径、96 x 96 源尺寸、默认逻辑尺寸和 `UiIconTintPolicy`；路径先复用图片层安全相对路径校验，再交给 `AssetServer`。未知 ID、非法描述或实际加载失败都会切换到正式 `missing.png`，同时保留请求 ID、实际渲染 ID 和稳定失败码。占位资源自身失败时不会继续递归回退。

- `MonochromeTintable` 只用于白色透明底单色图标，允许按钮状态覆盖 tint。
- `FullColor` 用于保留自身色彩的图片图标和缺失占位。任何主题或状态 tint 都被忽略，`ImageNode.color` 固定为 `Color::WHITE`。
- 按钮状态优先级固定为 `disabled > loading > pressed > hovered > selected > focused > idle`。触控和鼠标都使用 Bevy `Interaction::Pressed`，键盘仍由现有焦点系统写入同一状态。
- 状态可以覆盖图标 ID、tint 或背景，但只更新渲染组件。根 `Node` 的 width、height、min size、padding、点击区域和子层级不参与状态切换。
- 纯图标和固定图片按钮包含不可见的 `UiI18nText` label，供 Bevy 按钮 accessibility node 计算可访问名称；组合按钮直接复用可见 i18n label。

## 图片适配规则

`ui_image_panel_node` 或 `ui_image_panel_node_with_radius` 负责外层布局约束和 `Overflow::clip()`；`ui_image` 只负责内层图片绘制。运行时只更新内层节点尺寸和 `ImageNode.rect`，不会把图片源尺寸写回父节点布局。

- `Natural`：按源图片像素作为逻辑尺寸绘制；超出 frame 的部分由 frame 裁切。
- `Stretch`：忽略源宽高比，填满 frame。
- `Contain`：保持源宽高比，取能完整放入 frame 的最大尺寸，剩余区域透明。
- `Cover`：填满 frame，并在源图范围内生成保持 frame 宽高比的裁切矩形。焦点使用归一化源图坐标，`(0, 0)` 是左上，`(1, 1)` 是右下；有限的越界值会 clamp，`NaN` 和 infinity 会进入 `Invalid`。

frame 支持固定尺寸、百分比尺寸、单轴自动尺寸、宽高比以及 min/max 组合。非有限值、非正基础尺寸或宽高比、超过 `100%` 的百分比、矛盾 min/max、不同单位的 min/max 对，以及同时指定宽、高和宽高比都会返回稳定错误码。运行时状态为 `Loading`、`Ready`、`Failed` 或 `Invalid`；后三种非就绪路径不会回退到图片纹理的 1x1 尺寸。

## 九宫格、平铺与图集帧规则

`UiAdvancedImageSpec` 组合可序列化的源纹理描述与 `Stretch`、`NineSlice` 或 `Tiled` 模式。`try_ui_advanced_image(&AssetServer, spec, size)` 先校验全部静态约束，再且仅按 spec 的 source path 创建实际图片 handle；调用方不能另行注入同尺寸纹理。实际资源加载后仍会比对声明尺寸，并继续复用 `UiImageStatus` 和现有占位状态机。

- 九宫格使用 `UiNineSlice`：insets 必须有限、非负，且对边之和严格小于源图尺寸。中心和边缘分别选择 Stretch 或 Tile；Tile 的 `stretch_value` 限制为 `0.001..=1.0`，避免依赖 Bevy 的静默 clamp。
- 角块缩放与 Bevy 0.18.1 一致：取目标/源图两轴比例的较小值并受 `max_corner_scale` 限制。小目标会等比缩小四角，不会让对边超过目标；目标每轴至少覆盖一个物理像素，高 DPI 下按 `device_scale` 判断。
- 九宫格 Tile 在构建运行时模式前估算生成 slice 数，并受 `max_generated_slices` 限制；整图平铺使用 `UiImageTiling` 的 X、Y 或 Both 轴向和 `max_repeats` 总预算。
- `UiAtlasFrame` 记录权威资源路径、源纹理尺寸、像素 rect、未裁原始尺寸和可选 pivot。pivot 是未裁帧左上原点的 `0..=1` 坐标。空路径、绝对/父级/Windows drive-relative 路径、反斜杠、零尺寸、帧越界、original size 小于帧或非有限 pivot 均返回稳定错误。高级 API 当前不接受无路径的程序化图片；此类整图仍使用基础 `ui_image`，需要高级模式时应先建立显式 source variant。
- 当前明确拒绝 atlas frame 与 NineSlice/Tiled 组合。即使底层 `ImageNode.rect` 与 slice mode 可以同时赋值，也不把 Bevy 0.18.1 的组合渲染细节当作框架兼容承诺。

后续能力应扩展此固定区域或增加新的命名 state；不要依赖内容总高度计算出的 `middle` 作为唯一高保真基线。

## 资源与许可

- 阶段 fixture 放在 `project/assets/ui/fixtures/`，正式业务资源不要依赖 fixture ID 或路径。
- 图片、字体等二进制继续命中仓库 `.gitattributes` 中的 Git LFS 规则；RON、Markdown 和许可文本保持普通 Git 文件。
- 每项 fixture 必须在 `manifest.ron` 和 `LICENSES.md` 中记录用途、尺寸或字重、来源、许可和固定上游版本。
- 正式图标位于 `project/assets/ui/icons/`。Lucide 几何固定在 0.468.0/ISC，项目自有全彩图标由 `scripts/generate-ui-icons.ps1` 确定性导出；`manifest.ron` 固定每个 PNG 的来源与 SHA-256。
- 仓库自产几何 fixture 不含外部参考图像素；外部字体随附原始许可文本。
- 来源或许可不明确的参考图只能用于本地对照，不能进入 `project/assets/`、APK 或正式游戏资源。

## 非目标

本能力模型不定义 AI 模型调用、参考图理解、声明式 UI 文档协议、图像差异算法或完整样式语言。材质边界只提供静态 allowlist 和失败降级策略，不等于仓库已经交付可用的自定义 shader。fixture 也不是正式美术资产，不用于建立产品视觉风格。
