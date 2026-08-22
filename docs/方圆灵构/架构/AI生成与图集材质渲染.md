# 方圆灵构 AI 生成与图集材质渲染

## 文档定位

本文定义方圆异构组件从 AI 生成、蓝图审核、材质解析到游戏内运行时渲染的下一阶段实现合同，供后续生成一份或多份开发 checklist。

本文是设计目标，不代表当前代码已经完成这些能力。当前行为事实仍以 Rust 类型、validator、渲染适配层和测试为准。

本文只处理：

- AI 根据提示词生成由 cube 和 sphere 组成的方圆结构。
- AI 从受控目录选择表面、材质预设和可选噪点。
- 蓝图和 runtime primitive 保存稳定资源引用。
- 游戏内使用颜色图集、法线图集、噪点图集和 primitive 常量材质完成渲染。
- 标准渲染、静态合并和实例化路径共享同一材质语义。
- 资源缺失、非法引用、移动端预算和后续美术替换具有稳定边界。

本文不处理：

- 具体在线模型供应商、计费、凭据和网络调用；本文只定义 provider-neutral 输入、输出和校验合同。
- 玩家上传任意图片、Shader 或脚本。
- AI 直接生成伤害、冷却、控制强度、资源产出或 authority 结果。
- 新增 cylinder、自由 mesh、rotation 或其他方圆二相之外的几何能力。
- 以 ORM、高度、独立 AO、发光或透明图集作为当前前置条件。
- 把 256 个表面预设直接注册进现有 `FangyuanMaterialProfileRegistry`。

相关文档：

- [生成流程与权限边界](../规范/生成流程与权限边界.md)
- [蓝图格式与生成约束](../规范/蓝图格式与生成约束.md)
- [审核与预算规则](../规范/审核与预算规则.md)
- [运行时与渲染架构](./运行时与渲染架构.md)
- [资源构建与加载](./资源构建与加载.md)

## 当前实现边界

### 已存在的资源

当前首包资源位于 `project/assets/fangyuan/atlases/`：

| 资源 | 当前作用 |
| --- | --- |
| `base/surface_color.png` | `1024 x 1024`、`16 x 16` 表面颜色图集，按 sRGB 采样 |
| `base/surface_normal.png` | 与颜色图集同坐标的切线空间法线图集，按 Linear 采样 |
| `base/surface_atlas.json` | 256 个表面条目的稳定 `tile_id`、坐标和 AI 语义；被 preset 引用时字段名为 `surface_tile_id` |
| `base/surface_material_presets.json` | 256 个表面条目的 primitive 常量 PBR 参数、法线强度、参考重复尺寸和噪点建议 |
| `noise/procedural_noise.png` | 独立的 `1024 x 1024` 噪点图集，按 Linear 数据采样 |
| `noise/noise_atlas.json` | 122 个有效噪点条目、134 个保留格、用途和实测统计 |

图集统一使用：

- `1024 x 1024` 图片。
- `16 x 16` 目标网格。
- 每格 `64 x 64`。
- 每格有效内容 `60 x 60`。
- 四周各 `2px` 边缘延展。
- 图片坐标原点在左上，X 向右，Y 向下。

### 已存在的运行时能力

当前 `FangyuanPrimitive` 已有 kind、local position、scale、color、role、alpha、emissive、`material_profile_id` 和 lifecycle 等字段。

当前 `FangyuanMaterialProfile` 只提供：

- base color。
- alpha policy。
- emissive policy。
- profile fallback 和预算统计。

当前代码尚未：

- 解析表面或噪点 JSON 目录。
- 在 blueprint/runtime primitive 中保存 `surface_material_preset_id`。
- 把 surface/noise tile index 编译为实例数据。
- 在方圆 Shader 中采样颜色、法线和噪点图集。
- 支持 roughness、metallic、法线强度、UV 重复尺寸或噪点扰动。
- 验证法线绿色通道方向。
- 生成 atlas-aware mipmap。
- 通过真实在线 AI 自动生成蓝图。

因此 checklist 不得把“资源已存在”写成“运行时机制已完成”。

## 核心技术决策

### 当前美术资源已经足够跑通机制

第一版只依赖：

1. 颜色图集。
2. 同坐标法线图集。
3. 独立噪点图集。
4. 表面语义目录。
5. primitive 常量材质预设。

暂不增加其他贴图：

| 常见资源 | 当前替代方式 |
| --- | --- |
| Roughness map | 材质预设常量，可选噪点扰动 |
| Metallic map | 材质预设常量 |
| AO map | 固定为 `1.0`，遮蔽交给几何、阴影或 SSAO |
| Height / displacement | 方圆 primitive 几何表达主体体积，细节交给法线 |
| Emissive map | 继续使用现有 profile / primitive emissive；动态图案等 atlas Shader 落地后再由噪点受控调制 |
| Opacity map | 继续使用现有 profile / primitive alpha；第一版不把颜色图 Alpha 引入为新的 opacity 语义 |
| Detail map | 噪点图集 |
| ORM atlas | 当前不需要，`orm_texture_file` 保持 `null` |

后续美术可以替换现有占位图。只要稳定 ID、图集规格和颜色/法线坐标关系不变，AI 协议和 primitive 数据无需改变。

### 图集是存储方式，不是方圆结构的识别来源

方圆对象的主要识别信息必须来自：

- cube / sphere 的组合轮廓。
- primitive 的位置和 scale。
- 对象、部件和 primitive role。
- 颜色层级。
- 少量统一材质差异。

纹理只负责表面质感。AI 不应为了使用 256 个表面而给每个 primitive 随机分配不同纹理。

### 材质应先按对象和部件分组，再由 primitive 继承

推荐生成顺序：

```text
对象主题
-> 部件划分
-> 部件 role
-> 部件材质预设
-> primitive 结构
-> primitive 继承部件材质
-> 少量显式 override
```

例如：

```text
青玉机关龙
  主体 -> 青玉预设
  关节 -> 暗色金属预设
  眼睛 -> 青色发光预设
  装饰 -> 金色金属预设
```

禁止默认按 primitive 独立选材。否则会造成风格噪声、材质资源膨胀和 batch 分裂。

### 现有 material profile 与表面材质预设必须分层

现有 `material_profile_id` 已用于颜色、alpha、emissive policy、审核、CPU merge 和 static instance 分组，不应在未迁移前改变含义。

新增概念建议命名为：

```text
surface_material_preset_id
```

职责：

- `material_profile_id`：现有基础政策、fallback、透明和发光预算。
- `surface_material_preset_id`：颜色/法线 tile、roughness、metallic、法线强度、UV 和可选噪点。

现有 registry 上限为 256，默认 profile 已占一个槽位。256 个 surface preset 必须使用独立目录和解析表，不能直接全部插入现有 registry。

### 通道所有权和合成顺序

接入图集后，现有 `FangyuanMaterialProfile::compose_primitive` 仍是 tint、alpha 和 emissive policy 的唯一合成入口，不能在 surface resolver 中再实现一套相互竞争的规则。

第一版合成合同：

```text
profile_instance = material_profile.compose_primitive(primitive)
surface_sample_linear = sample_srgb_surface_texture(surface_tile_index, local_uv)

final_rgb = surface_sample_linear.rgb * profile_instance.color.rgb
final_alpha = profile_instance.alpha
final_emissive = profile_instance.emissive
final_roughness = surface_preset.perceptual_roughness
final_metallic = surface_preset.metallic
final_ao = 1.0
```

具体边界：

- 第一版忽略颜色图 Alpha，不让占位图意外改变现有透明语义。
- `surface_color.png` 必须创建为 sRGB GPU texture；正常采样结果已由 GPU 解码到 Linear，Shader 不得再次执行 sRGB 解码。
- `surface_material_presets.json` 中的 `alpha` 和 `emissive_intensity` 是 AI 创作建议。需要水体或熔岩效果时，生成器把建议值写入现有 primitive alpha / emissive，再由 material profile policy 审核、合成和 clamp；运行时不得把 preset 值重复相乘或相加。
- reflectance、roughness、metallic、normal、UV、noise 和 render model 由 surface preset 提供。
- 噪点若调制 emissive，只能在 profile 已允许的范围内变化，不能绕过现有 emissive 上限。
- 特殊 water / sky 的 render model 选择不能改变 authority 数据，也不能绕过透明和发光预算。

## 稳定标识和引用关系

### 三类稳定 ID

| ID | 示例 | 来源 |
| --- | --- | --- |
| `surface_tile_id` | `surface_x00_y00` | `surface_atlas.json` |
| `surface_material_preset_id` | `material:surface/x00/y00` | `surface_material_presets.json` |
| `noise_tile_id` | `noise_x04_y07` | `noise_atlas.json` |

关系：

```text
surface_material_preset_id
-> 唯一 surface_tile_id
-> 同坐标 base color + normal
-> roughness / metallic / AO / normal strength / UV
-> 可选 noise_tile_id
```

蓝图、AI 输出和运行时不得直接保存任意图片路径、UV 矩形或 Shader 名称。它们只能保存受控稳定 ID，由 catalog resolver 转为运行时索引。

`surface_atlas.json` 中的实际字段名是 `tile_id`；本文用 `surface_tile_id` 表示其跨目录语义，`surface_material_presets.json` 也使用该字段名引用 surface tile。实现 checklist 必须按各 JSON schema 的真实字段反序列化，不能只根据本文概念名猜字段。

### 开发期和运行期表示

开发期可以保存字符串 ID，便于人类和 AI 审查：

```text
material:surface/x00/y00
```

Bake 或运行时解析后应转为紧凑索引：

```text
surface_tile_index: u16
noise_tile_index: Option<u16>
```

索引是当前 catalog 版本内的运行时数据，不能替代跨版本稳定 ID。

## AI 生成合同

### 输入

AI 至少需要这些上下文：

- 对象用途：家园、装备、角色外观、技能视觉、NPC 或场景装饰。
- 结构主题和描述。
- 最大 primitive 数、bounds 和体积预算。
- 允许的 cube / sphere。
- 可用 role。
- 可用 surface/material/noise catalog。
- 角色权限、四属性、职业、世界层级和灵构额度。
- 禁止内容和性能预算。

### 分阶段输出

AI 不应一次输出无结构的大型 primitive 数组。推荐分四步：

1. 生成对象计划：主题、部件、role、比例和预算分配。
2. 为部件选择少量 `surface_material_preset_id`。
3. 生成 primitive，并让 primitive 引用部件材质。
4. 运行 deterministic validator 和有限修复，得到 canonical blueprint。

最小结构建议：

```text
FangyuanGeneratedObject
  object_id
  object_role
  material_bindings[]
  primitives[]
  generation_metadata
```

概念示例，不代表当前 RON 已支持：

```ron
material_bindings: [
  (
    id: "body",
    surface_material_preset_id: "material:surface/x02/y00",
  ),
  (
    id: "joint",
    surface_material_preset_id: "material:surface/x00/y01",
  ),
]

primitives: [
  (
    kind: "cube",
    position: [0.0, 1.0, 0.0],
    size: [1.0, 2.0, 0.8],
    role: "structure",
    material_binding_id: "body",
  ),
]
```

第一版可以先让 primitive 直接保存 `surface_material_preset_id`，等端到端机制稳定后再引入对象级 binding table 去重。checklist 必须明确选择其中一种，不得同时实现两种不完整路径。

### AI 选材规则

AI 必须：

- 先匹配对象和部件 role，再匹配表面语义。
- 从 `surface_material_presets.json` 选择唯一稳定 ID。
- 默认沿用 preset 的 roughness、metallic、AO、法线强度和重复尺寸。
- 仅在明确提示词和允许范围内生成实例 override。
- 只有目标效果明确需要时才启用噪点。
- 置信度不足时返回候选和理由，不伪造唯一答案。

AI 禁止：

- 生成未登记的 tile ID。
- 直接写 atlas 像素坐标或 UV。
- 生成任意本地路径、URL、脚本或 Shader。
- 把视觉噪点作为 authority 或确定性玩法随机源。
- 根据颜色猜测 metallic。
- 绕过透明、发光、材质数量或 primitive 数量预算。

### 确定性边界

同一份已确认的 canonical blueprint、catalog 版本和资源 hash 必须解析出相同 runtime 数据。

在线 AI 只参与蓝图创作，不参与每帧运行时计算。游戏内回放、authority 和联网同步只使用已经审核并固化的稳定数据。

本文可以先用本地 Codex 或离线确定性 fixture 验证生成合同，但这只证明 schema、审核、材质选择和渲染闭环，不代表生产在线 AI 已接入。如果目标包含游戏或服务端自动调用模型，还需要单独的 provider checklist，处理凭据隔离、超时、重试、限流、成本、内容安全、审计、版本固定和不可用 fallback；该 checklist 只能消费本文定义的 provider-neutral 合同，不能绕过 canonical 和 audit。

## Blueprint 和 runtime 数据模型目标

### 第一版最小字段

建议在 blueprint 和 runtime primitive 增加：

```text
surface_material_preset_id: Option<String>
```

缺省时使用安全默认材质，不改变现有蓝图行为。

第一版暂不允许蓝图直接填写：

- 任意 roughness。
- 任意 metallic。
- 任意 atlas UV。
- 任意 noise strength。
- 任意 Shader 参数。

这些值全部从受控 preset 解析。这样可以先验证机制，减少 schema、审核和 batch 组合爆炸。

### 后续受控 override

最小闭环稳定后再考虑：

```text
roughness_delta
normal_strength_multiplier
uv_scale_multiplier
noise_enabled
emissive_override
alpha_override
```

override 必须使用 preset 文件声明的范围，并进入审核、统计、hash、Bake 和网络一致性检查。

### Runtime resolved data

catalog resolver 应先产生与字符串无关、可共享的表面预设表，例如：

```text
ResolvedFangyuanSurfacePreset
  preset_index: u16
  surface_tile_index: u16
  noise_tile_index: Option<u16>
  perceptual_roughness: f32
  metallic: f32
  reflectance: f32
  normal_strength: f32
  reference_repeat_size_m: Vec2
  projection_hint: enum
  render_model: enum
  flags: u32
```

primitive compiler 再把该表与现有 material profile 的合成结果连接起来：

```text
ResolvedFangyuanPrimitiveMaterial
  material_profile_index: u16
  surface_preset_index: u16
  composed_tint: Vec4
  composed_alpha: f32
  composed_emissive: f32
  flags: u32
```

这两个结构是职责示例，不要求最终 Rust 命名完全一致。关键约束是 catalog 常量只解析一次，而 per-primitive tint、alpha 和 emissive 仍遵循现有 profile 语义。AO 当前恒为 `1.0`，无需进入每实例 buffer。只有以后出现非恒定运行时语义时再增加。

Resolved 数据应在加载、蓝图变更或 catalog 变更时重建，不能每帧重复解析 JSON 或字符串。

## 资源加载和校验

### 开发期加载

开发期 loader 读取三个 JSON：

1. `surface_atlas.json`
2. `surface_material_presets.json`
3. `noise_atlas.json`

加载顺序：

```text
解析 JSON
-> schema version
-> catalog id
-> 唯一 ID
-> 坐标和 index
-> 图片存在及尺寸
-> surface preset 一一对应
-> normal 同坐标
-> noise 引用存在且不是 reserved
-> 参数范围
-> 构建只读 resolver
```

任何顶层错误必须使该 catalog 不可用，并启用安全 fallback，不能部分接受一个破损 catalog。

### 必须校验的资源规则

- 图片必须为 `1024 x 1024`。
- 表面颜色和法线必须为 `16 x 16` 同坐标。
- `surface_tile_id`、preset ID、noise ID 唯一。
- surface preset 必须和 256 个 surface tile 一一对应。
- noise 只能引用 `tiles` 中的 122 个有效格。
- roughness、metallic、AO、reflectance 和 alpha 在 `0..=1`。
- AO 在当前版本必须为 `1.0`。
- 法线强度有限且非负。
- catalog companion 路径相对当前 catalog 解析并规范化，最终路径必须仍位于 `fangyuan/atlases/` 受控根目录；当前 `../noise/noise_atlas.json` 属于根内合法引用。
- 任何会逃出受控根目录的父级跳转、URL、绝对路径或 Windows drive 必须拒绝。
- AI 输出和 blueprint 不允许携带任何路径，只允许稳定 ID。
- normal 和 noise 按 Linear，base color 按 sRGB。

### Fallback

至少提供：

- 缺失 surface preset：默认白色、roughness `0.6`、metallic `0.0`、AO `1.0`。
- 缺失颜色图：使用 primitive color。
- 缺失法线图：禁用 normal。
- 缺失噪点：禁用 noise modulation。
- 特殊 water/sky Shader 未实现：使用明确的开发期 fallback，并在报告中记录，不能静默伪装为正式效果。

fallback 必须进入 debug report、HUD 或日志，并保留原始 requested ID。

### Bake 和版本

开发期 JSON 不应成为长期运行时每次启动解析的大型正式格式。机制稳定后应：

```text
JSON catalog
-> validator
-> canonical
-> hash
-> bake artifact
-> runtime resolver table
```

图集图片、catalog 和材质预设必须作为同一版本依赖更新。只替换图片但不改变 ID 和坐标时，仍应更新内容 hash。

## 图集采样合同

### Surface atlas

颜色和法线共用一个 `surface_tile_index`。

每格 UV：

```text
cell_origin_px = (x * 64, y * 64)
uv_min = (cell_origin_px + 2.5) / 1024
uv_max = (cell_origin_px + 61.5) / 1024
atlas_uv = lerp(uv_min, uv_max, local_uv)
```

`local_uv` 必须先在单格局部空间执行 repeat：

```text
local_uv = fract(base_uv * repeat_scale)
```

禁止对整张 atlas 使用 repeat sampler，否则会跨到其他 tile。

### Noise atlas

噪点使用独立 `noise_tile_index`，不能复用 surface index。reserved 格永远不可采样。

噪点是数据纹理：

- 不做 sRGB 解码。
- 默认不参与玩法逻辑。
- 可以受控影响 roughness、颜色、法线强度、UV distortion 或 emissive。
- 第一版每个 material 最多启用一个 noise tile。

### 颜色空间

- base color：sRGB。
- normal：Linear。
- noise：Linear。
- roughness / metallic / AO 常量：Linear 标量。

颜色空间错误必须有自动测试或调试视图，不能仅靠肉眼猜测。

### Normal

法线图与颜色图使用相同 tile 坐标和局部 UV。

运行时接入前必须确认：

- 绿色通道是 OpenGL `+Y` 还是 DirectX `-Y`。
- cube 和 sphere mesh 都有正确 tangent basis。
- 非均匀 scale 下 normal matrix 正确。
- normal strength 的实现不会破坏单位法线。

未确认绿色通道前，默认禁用 normal 或仅在开发调试开关中启用，不能把错误凹凸方向作为正式结果。

### UV 和投影

第一版建议：

- cube：使用稳定六面 UV；每个面按自身物理尺寸计算 repeat。
- sphere：使用稳定球面 UV，接受极点失真作为第一版限制。
- 不新增 primitive rotation。
- directional texture 保持 authored up，不提供任意 UV rotation。

当前预设目录中的 projection 是创作提示，不等于 runtime 已支持对应算法。最小闭环必须定义可诊断的降级映射：

| Catalog 提示 | 第一版运行时行为 |
| --- | --- |
| `planar` | cube 使用稳定面 UV；sphere 使用球面 UV fallback |
| `planar_or_triplanar` | 先按 `planar` 处理，支持 triplanar 后再升级 |
| `triplanar` | 第一版降级到稳定 mesh UV 并记录诊断，不得拒绝整个 catalog |
| `spherical_or_background` | sphere 使用球面 UV；background / sky 使用明确的特殊表面 fallback |

`orientation_policy: free_rotation` 只表示素材没有固定朝向要求，不授权新增 primitive rotation；`preserve_authored_up` 必须保持图案上方向。loader 应接受目录中已声明的提示值，runtime 对当前不支持的能力执行上述降级；只有未知枚举值才属于 catalog 错误。

后续可增加 triplanar 或 object-space projection，但必须作为单独阶段，因为它会影响 Shader、实例字段、normal 处理和性能。

### Mipmap 和边缘串色

`2px` padding 只足以缓解基础双线性过滤，不能证明低 LOD mipmap 安全。

建议分阶段：

1. 最小闭环使用无 mip 或受控 LOD，先验证 tile、颜色空间和 normal。
2. 规模化前生成 atlas-aware mipmap，或限制最大 LOD。
3. 如果长期仍有串色或采样复杂度，再评估 texture array。

不得在未验证时直接启用普通整图 mipmap。

## Shader 和渲染路径

### 统一材质目标

目标不是创建 256 个或每 primitive 一个 `StandardMaterial`，而是：

```text
少量 Fangyuan atlas material
+ shared cube/sphere mesh
+ per-instance material data
```

实例数据建议至少包含：

```text
position
scale
color
surface_tile_index
roughness
metallic
normal_strength
uv_scale
noise_tile_index_or_sentinel
noise_strength
alpha
emissive
flags
```

字段布局必须先计算移动端 buffer 成本，并明确对齐、更新频率和 hash 口径。

### 分阶段渲染

第一阶段：标准参考路径

- 在少量测试 primitive 上验证 catalog 解析、tile UV、颜色空间和 roughness / metallic 常量。
- 可以用少量 UV 重映射测试 mesh 和 `StandardMaterial` 验证 base color；法线只在 atlas 子区域 UV 与 tangent 均正确时验证。
- `StandardMaterial` 没有本合同所需的 noise modulation 和 normal strength 语义，因此该路径不能宣称全部通道已经完成，也不能用于证明规模性能。

第二阶段：统一 atlas Shader

- 统一绑定三张图集。
- 按实例 tile index 和常量材质采样。
- cube 和 sphere 使用共享 mesh。
- 对缺失 normal/noise 使用 flags 禁用，不创建变体爆炸。
- 提供 base-only、normal-only、noise-only 和 final 调试模式。

第三阶段：接入静态合并和实例化

- Standard、CPU merge 和 static instance 解析出相同材质语义。
- batch key 只包含真正影响 pipeline 或资源绑定的字段。
- 颜色、tile index 和常量参数尽量作为 vertex/instance data，不进入材质资源 key。
- 不因 256 个 preset 创建 256 个 draw batch。

### 特殊表面

当前 preset 中 water 和 sky 标记为特殊 render model。

最小闭环可以：

- water：先用受控透明或不透明 fallback。
- sky：先用 unlit/background fallback。

专用水体、天空、折射、透明深度和复杂 VFX 不应阻塞基础 atlas 机制，但 fallback 必须可诊断。

## 审核、预算和安全

新增审核至少覆盖：

- 未知 `surface_material_preset_id`。
- preset 与 surface tile 不一致。
- reserved noise tile。
- 非法材质参数。
- 未知的 projection / render model，以及已知但发生运行时降级的能力。
- 透明和发光预算。
- 同对象材质绑定数量。
- 同屏 material preset 数量。
- atlas Shader fallback 数量。
- 实例 buffer 字节数。

建议第一版预算：

- 一个对象优先不超过 4 个 surface material binding。
- 普通 primitive 默认不透明、非发光、noise 关闭。
- water、sky、lava 等特殊预设需要显式 role 或提示词。
- warning 可以降级到默认 surface；顶层 catalog 错误必须阻止加载。

AI 不能通过大量不同 preset 绕过现有 material profile 预算。后续 checklist 需要决定 surface preset 数量是复用现有 finding，还是新增独立 finding，不能静默不统计。

## 调试和验收工具

至少需要一个受控开发视图或试炼场：

- 显示 surface atlas 256 格及 ID。
- 显示 noise atlas 122 个有效格和 reserved 区域。
- 选择一个 preset 后同步显示 base、normal、noise 和最终 PBR。
- 切换 normal 绿色通道方向。
- 调整 roughness、metallic、normal strength 和 UV scale。
- 显示最终 UV rect、tile index、preset ID 和 fallback reason。
- 在 cube、sphere、非均匀 scale 和多个尺寸上检查。
- 放置相邻 primitive 检查局部平铺、接缝和材质继承。
- 显示 batch、draw estimate、instance bytes 和材质绑定数量。

调试工具不得变成正式玩家自定义 Shader 编辑器。

## 测试策略

### 单元测试

- JSON schema 和版本。
- ID、坐标、index 唯一性。
- surface/preset 一一对应。
- noise reserved 拒绝。
- 参数范围和有限数。
- 路径安全。
- fallback 保留 requested ID。
- blueprint 编译保持旧文件兼容。
- canonical/hash 对字段变化敏感。

### 渲染测试

- `(0,0)`、边界 tile 和 `(15,15)` UV 正确。
- base color 不被当作 Linear。
- normal/noise 不被当作 sRGB。
- cube 六面和 sphere UV 可接受。
- normal 开关和绿色通道切换可见。
- local repeat 不跨 tile。
- padding 和 LOD 不出现明显串色。
- 缺失资源 fallback 可见且可诊断。

### 规模测试

- 默认家园和万级静态测试数据可以构建 resolved material。
- catalog 只在加载或变更时解析。
- 不按 primitive 创建独立图片或材质资源。
- static instance buffer 大小符合预算。
- Reload、Clear、场景退出和资源变更不会泄漏 handle。

### 端到端测试

```text
固定提示词
-> 结构化 AI fixture
-> canonical blueprint
-> audit
-> runtime primitive set
-> material resolve
-> atlas render
-> 截图和 debug report
```

第一版可以使用离线确定性 fixture，不必等待在线 AI provider。

## 后续美术替换合同

美术后续替换资源时有两种方式。

保持兼容：

- 图片仍为 `1024 x 1024`。
- 仍为 `16 x 16`、每格 `64 x 64`。
- padding 和有效内容区域不变。
- `surface_tile_id` 语义和坐标不变。
- base color 与 normal 同坐标。
- 更新图片 hash 和必要的 preset 参数。

不保持兼容：

- 改变 tile 位置、数量或语义。
- 改变 atlas 尺寸、padding 或颜色空间。
- 拆分 texture array。

不兼容替换必须提升 catalog schema/content version，并提供旧 ID 到新 ID 的迁移或明确失效处理。

## 推荐 checklist 拆分

该目标跨数据、AI、资源、Shader、实例化和验收，推荐拆成五份 checklist，而不是一个超大清单。

### Checklist 1：资源合同与 runtime resolver

范围：

- Rust schema。
- JSON loader。
- validator。
- stable ID 和索引。
- fallback。
- cache、版本和 hash。
- blueprint/runtime 最小字段。

完成条件：旧蓝图兼容，256 surface preset 和 122 noise tile 可解析为只读 resolved table。

### Checklist 2：AI 方圆结构与材质生成

范围：

- 生成输入和对象计划。
- 部件 role。
- preset 选择。
- primitive 输出。
- deterministic canonical。
- 有限修复。
- audit 和离线 fixture。

完成条件：本地 Codex 或确定性 AI fixture 对固定提示词可得到合法蓝图，生成输出只含受控 ID，不含路径、UV 或 Shader。该条件不等价于生产在线模型已经接入。

### Checklist 3：图集 Shader 与参考渲染

范围：

- 图集加载和颜色空间。
- cube/sphere UV 和 tangent。
- base/normal/noise 采样。
- roughness/metallic 常量。
- local repeat。
- normal 方向验证。
- debug gallery 和 fallback。

完成条件：标准参考路径能验证 base color 和 PBR 常量；统一 atlas Shader 能在少量 primitive 上正确显示 base、normal、noise 和最终合成结果。

### Checklist 4：统一实例渲染与规模化

范围：

- instance data layout。
- atlas Shader。
- static instance。
- CPU merge 兼容。
- batch key。
- buffer、draw、LOD 和 mip 策略。
- Android 性能与资源生命周期。

完成条件：大量 primitive 不产生线性增长的材质资源，并通过桌面和 Android 预算验收。

### Checklist 5：端到端验收、Bake 和美术替换

范围：

- prompt 到截图闭环。
- catalog/bake dependency。
- hash 和缓存失效。
- 错误注入和 fallback。
- screenshot/debug report。
- 美术替换回归。
- 文档和清单归档。

完成条件：替换兼容图片后无需改变蓝图和 AI 协议，固定 fixture 仍可审核、加载和渲染。

如果必须生成单份 checklist，应按以上五个范围作为五个阶段，并保持依赖顺序，不能并行假设下游字段已经存在。

## 最终完成定义

整个机制完成必须同时满足：

- provider-neutral AI 生成链路能根据固定提示词输出合法 cube/sphere 方圆结构；若产品要求生产在线模型，还必须完成独立 provider checklist。
- AI 以对象和部件为单位选择少量材质，不逐 primitive 随机选材。
- 所有资源引用使用稳定受控 ID。
- 旧 blueprint 不填写新字段时保持兼容。
- surface preset、颜色、法线和噪点正确解析并有 fallback。
- 游戏内能显示 base color、normal、roughness、metallic 和可选 noise 效果。
- 不需要 ORM、高度、AO、发光或透明图集即可完成闭环。
- 不按 primitive 创建独立材质或图片资源。
- Standard、CPU merge 和 static instance 的材质语义一致。
- 非法 catalog、未知 ID、reserved noise 和错误参数被 validator 拒绝。
- authority 和玩法逻辑不读取视觉噪点作为随机源。
- 桌面和 Android 的视觉、性能、生命周期和资源回收通过验收。
- 后续美术可以在稳定 ID 和图集合同内替换占位资源。

## 仍需在 checklist 前确认的决策

以下问题不阻塞文档，但生成正式 checklist 时必须明确选择：

1. 第一版 blueprint 是直接增加 `surface_material_preset_id`，还是先引入对象级 material binding table。
2. 标准参考路径覆盖到哪些诊断项，以及自定义 atlas Shader 从哪一步接管；noise modulation 和 normal strength 不能只靠 `StandardMaterial` 完成。
3. cube 六面 UV 的具体方向和 sphere mesh/tangent 来源。
4. 法线绿色通道采用 OpenGL 还是 DirectX。
5. 最小闭环是否禁用 mipmap，还是同步实现 atlas-aware mip。
6. water/sky 第一版采用何种明确 fallback。
7. surface preset 审核使用新 finding，还是扩展现有 material profile finding。
8. 正式 GPU instance buffer 的平台对齐和字段压缩方案。
9. projection 提示发生降级时采用哪组 report code，以及何时把 triplanar 从降级能力提升为正式能力。
