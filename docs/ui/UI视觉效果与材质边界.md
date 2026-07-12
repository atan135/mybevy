# UI 视觉效果与材质边界

共享视觉效果位于 `project/src/framework/ui/style/effects.rs`。业务页面只组合主题中的 `UiEffectPresetId`，不能传 shader 路径、任意材质类型或无界参数。默认主题的 `effects.presets` 提供可热更新的受限配置；配置先完整编译，失败时主题热更新保留 last-known-good 值。

## Bevy 映射

| 配置能力 | 最终 Bevy 0.18.1 组件 | 框架限制 |
| --- | --- | --- |
| 盒阴影 | `BoxShadow(Vec<ShadowStyle>)` | 最多 3 层；颜色、X/Y 偏移、spread 和 blur 都先校验 |
| 文字阴影 | `TextShadow` | 只允许 1 层颜色和 X/Y 偏移；Bevy 当前没有文字 blur、spread 或多层组件，相关请求直接报错 |
| 背景线性渐变 | `BackgroundGradient(LinearGradient)` | 单层，2 至 6 个有序色标，位置为 `0..=1`，角度规范化后转为弧度 |
| 边框线性渐变 | `BorderGradient(LinearGradient)` | 单层，限制与背景渐变一致 |
| 独立边宽 | `Node.border` | 四边分别为 `0..=16px` |
| 独立圆角 | `Node.border_radius` | 四角分别为 `0..=64px` |
| 裁切 | `Node.overflow = Overflow::clip()` | 与最终圆角共同由 Bevy 裁切，不复制页面私有遮罩节点 |
| 轮廓 | `Outline` | width `0..=8px`，offset `-16..=16px`，颜色必须有效 |

所有颜色通道必须为有限的 `0..=1`，所有尺寸和角度必须有限。盒阴影偏移限制为 `-64..=64px`，spread 为 `-24..=48px`，blur 为 `0..=64px`。未知 preset、重复 preset、非法颜色、非法数值、超量阴影、错误色标和超预算都有稳定错误码，不依赖 Bevy 静默 clamp。

`UiEffectBinding` 同时跟踪各字段的 baseline 和上一帧实际效果输出。拥有字段如果被主题或业务系统写入了不同于上一帧效果输出的新值，框架会先把该值更新为最近 baseline，再重新应用效果；移除绑定时恢复这个最近观察到的外部值。更换 preset 时，当前值仍等于旧 preset 的 last-applied 输出，因此旧效果不会污染 baseline；新 preset 不再拥有的字段会立即恢复。稳定输入的后续帧不重复标记组件 Changed。

## 材质准入

自定义 `UiMaterial` 只允许在以下条件全部满足时进入实现评审：

- 纯色、图片、渐变、阴影、切片和普通控件组合无法表达目标效果。
- 材质 ID 和 shader 路径由框架静态 allowlist 持有，页面和主题不能传路径。
- 参数数量、类型、有限值、纹理数量和支持平台都有硬上限。
- 对应 `UiMaterialPlugin<T>`、`MaterialNode<T>`、shader 资源和加载状态由同一个框架 adapter 负责。
- Android 和目标桌面后端完成截图与性能验证，并存在纯 UI fallback。

当前 allowlist 只登记策略 ID `frosted_panel_v1`，对应固定路径 `shaders/ui/frosted_panel_v1.wgsl`，上限为 4 个 scalar、2 个 color、0 个 texture，允许 Windows、Linux、macOS 和 Android。仓库当前没有交付该 shader 和类型化 adapter，因此即使资源状态被标记为 Loaded，也会以 `ui_material_adapter_unavailable` 降级；代码不会伪称材质已经渲染。

材质检查顺序固定为：参数 -> 平台 -> GPU capability -> shader 未注册/Loading/Failed -> adapter。稳定原因码包括：

- `ui_material_invalid_parameters`
- `ui_material_platform_unsupported`
- `ui_material_gpu_unsupported`
- `ui_material_shader_unavailable`
- `ui_material_shader_loading`
- `ui_material_shader_load_failed`
- `ui_material_adapter_unavailable`

每个材质 preset 必须声明不依赖 shader 的背景色和边框色。任一失败路径都会应用该可见 fallback，并写入 `UiResolvedEffectDebugSnapshot`；没有 fallback 的材质配置在主题编译阶段以 `ui_material_fallback_missing` 失败。

## 性能预算

效果编译器执行以下每节点硬上限：

| 指标 | 硬上限 | 移动端建议值 |
| --- | ---: | ---: |
| 盒阴影层数 | 3 | 2；大面积滚动项使用 1 |
| 文字阴影层数 | 1 | 1，且只允许原生偏移阴影 |
| 单个线性渐变色标 | 6 | 3 |
| 规划额外 draw primitive 上界 | 8 | 4 |
| 半透明 overdraw 层规划值 | 5 | 3 |
| blur | 64px | 24px；覆盖大于视口四分之一的节点建议不超过 16px |

`requested_draw_call_upper_bound` 是按阴影层、背景/边框渐变、轮廓和材质各记一个额外 primitive 的保守规划值。它不是实际 GPU draw-call 测量：Bevy 的批处理、裁切、目标后端和相邻节点会改变最终命令数。`overdraw_layers` 同样是配置层数指标，不是像素级 overdraw 采样。发布前仍需在目标 Android GPU 上使用平台分析器测量帧时间、实际 draw call 和 overdraw。

页面级 `visual_budget` 会把所有 effect 的已应用 draw 上界求和，并取单 effect 最大 overdraw；同时结合节点、图片解码 payload、材质 ID 和可见节点生成 profile 报告。它仍是开发期估算，完整阈值和统计口径见 [UI安全区与视觉预算.md](UI安全区与视觉预算.md)。

移动端超过建议值时按以下顺序建立更轻的 preset，而不是在页面运行时任意改组件：

1. 移除最远、最淡的次级盒阴影。
2. 把 blur 降到 24px 以内，并优先缩小受影响面积。
3. 把渐变色标减少到 3 个，避免叠加背景和装饰渐变。
4. 移除非语义轮廓；保留用于焦点和错误状态的轮廓。
5. 自定义材质无论何种失败都使用声明的纯 UI fallback。

Gallery 的 5 个样例用于组合验收，不代表同屏业务页面应同时使用全部效果。滚动列表应避免为每一项配置多层模糊阴影。

## 审计

UI Gallery 的 `effects` state 会滚动到 `ui_gallery.effects` anchor，展示多层盒阴影、原生文字阴影、背景/边框渐变、圆角裁切组合和材质降级。audit metadata 的 `effect_resolutions` 按名称稳定排序，记录：

- 请求和最终 preset。
- 实际应用的 Bevy 组件名。
- 材质 ID、固定 shader 路径、平台、结果和原因码。
- 请求/最终规划 draw-call 上界、overdraw 层、阴影层和色标数。
- 是否 fallback 以及稳定错误码。

该 metadata 是只读验收信息，不是运行时修改材质或效果参数的入口。
