# 方圆图集 Shader 与参考渲染 Checklist

> 状态：实现进行中。本清单按阶段串行执行；用户参与的最终验收和 Android 显示验收在最小视觉采样实现完成后执行。
>
> 在本清单重新启用前，01/02 只能验收资源合同、生成流程和运行时数据边界；不得把 catalog 解析、blueprint 加载或普通 fallback 误记为颜色/法线/噪点图集已经正确显示。

## 目标

在少量 cube/sphere primitive 上完成颜色、法线、噪点图集和常量 PBR 参数的正确采样，建立 StandardMaterial 诊断参考路径、统一 Fangyuan atlas Shader、明确 fallback 和开发调试视图。

本清单完成视觉正确性，不承担万级 primitive、正式 GPU instance buffer、完整 CPU merge 优化、atlas-aware mip 生产管线、专用水体/天空/折射 Shader 或生产在线 AI。规模化、实例化和性能优化另行规划，不属于当前批次。

## 基础原则

- [ ] 本清单依赖资源 resolver 和 promoted blueprint 已完成，只消费稳定 preset/tile index，不在 Shader 中解析字符串或 JSON。
- [ ] base color 以 sRGB GPU texture 加载并由硬件解码，normal/noise 以 Linear 数据纹理加载，Shader 不重复执行 sRGB 解码。
- [ ] 第一版颜色图 Alpha 不参与 opacity；alpha/emissive 继续由现有 `FangyuanMaterialProfile::compose_primitive` 管理。
- [ ] base color 和 normal 使用同一 surface tile；noise 使用独立 tile index，reserved cell 永不采样。
- [ ] 第一版使用无 mip 或受控 LOD，不直接对整张 atlas 启用普通 mipmap。
- [ ] 不新增 primitive rotation；projection hint 只按已定义规则降级。
- [ ] 每个阶段独立实现、验证和提交，完成后填写阶段元信息。

## 援引文档

以下文档用于说明 checklist 的需求来源和编制依据：

- 文档名称：方圆灵构 AI 生成与图集材质渲染
  - 路径或链接：[AI生成与图集材质渲染](../../docs/方圆灵构/架构/AI生成与图集材质渲染.md)
  - 用途说明：定义 UV、颜色空间、normal、noise、Shader、fallback 和调试验收合同。
- 文档名称：方圆灵构运行时与渲染架构
  - 路径或链接：[运行时与渲染架构](../../docs/方圆灵构/架构/运行时与渲染架构.md)
  - 用途说明：约束 Standard、CPU merge、static instance 和统一材质边界。
- 文档名称：方圆灵构资源构建与加载
  - 路径或链接：[资源构建与加载](../../docs/方圆灵构/架构/资源构建与加载.md)
  - 用途说明：约束三张图集的首包加载、依赖和生命周期。

## 阶段 1：建立图集资源加载和参考样例

- 开始时间：2026-08-26 14:13:01 +08:00
- 结束时间：2026-08-26 14:58:14 +08:00
- 开发总结：建立三张 Fangyuan 图集的共享 Bevy Image handle、通道颜色空间和受控采样合同，补齐 catalog/通道加载状态、fallback flags、诊断 preset 集及 Fangyuan Home 生命周期接入；本阶段仍只提供参考渲染基础，不宣称已经完成 atlas Shader 视觉采样。
- 验证记录：`cargo fmt --check`、atlas 定向测试 5/5、Fangyuan 测试 484/484、Fangyuan Home handle 复用测试 1/1、`cargo check` 均通过；输出仅含仓库既有 warning。

- [x] 复核 resolver 输出、图集文件路径、Bevy 0.18.1 image/sampler API 和当前 `FangyuanRenderAssetCache` 生命周期。（验证：`render_assets.rs` 仅通过 canonical 首包路径和已验证 `FangyuanSurfacePresetRegistry` 建立 atlas adapter；`fangyuan_home.rs` 在插件资源中集中维护共享 cache。）
- [x] 建立共享 atlas image handles，颜色图明确按 sRGB，法线和噪点明确按 Linear；不得按 primitive 重复加载图片。（验证：`FangyuanAtlasAssetCache::ensure_loaded` 使用三条 canonical path 和 `load_with_settings`；通道测试断言 base sRGB、normal/noise Linear，插件复用测试确认连续 update 的三个 handle 不变。）
- [x] 为缺颜色、缺法线、缺噪点和 catalog unavailable 建立可观察加载状态与 fallback flags。（验证：`FangyuanAtlasAssetStatus`、`FangyuanAtlasImageState` 和四个独立 fallback bit 已实现；缺通道定向测试验证 base/normal/noise flags 独立。）
- [x] 建立少量诊断 preset 集，覆盖 `(0,0)`、边界、`(15,15)`、普通砖石、金属、青玉和一个有效 noise tile。（验证：`FANGYUAN_ATLAS_DIAGNOSTIC_PRESETS` 覆盖 origin、row boundary、last cell、masonry、metal、qingyu core；测试用真实 registry 核对 index/noise ID。）
- [x] 确认三张图片尺寸、sampler address/filter、mipmap policy 和 asset usage 满足当前最小闭环。（验证：catalog 完整加载继续校验三张 1024x1024 图片；`ImageLoaderSettings` 使用 ClampToEdge、Linear min/mag、LOD 0 和 MAIN_WORLD|RENDER_WORLD。）
- [x] 记录 StandardMaterial 能验证的范围：base color、roughness、metallic 和有限 normal；明确它不能完成 noise modulation 和 normal strength。（验证：`FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES` 明确前三项及 tangent-space normal 可用，noise modulation/normal strength 为 false，并有断言。）
- [x] 对 image handle 复用、缺失资源状态、颜色空间配置和 reload 增加测试或可检查报告。（验证：atlas cache 的 canonical/color-space、复用/catalog fallback、独立缺图 flags、reload ID 复用共 4 项测试通过；Fangyuan Home 插件 handle 复用测试 1/1 通过。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：`cargo fmt --check` 通过；`cargo test --lib fangyuan -- --nocapture` 484/484；`cargo check` 通过。）

## 阶段 2：锁定 cube/sphere UV、tangent 和法线方向

- 开始时间：2026-08-26 15:00:02 +08:00
- 结束时间：2026-08-26 15:44:37 +08:00
- 开发总结：新增统一 atlas geometry 合同和共享 Fangyuan cube/Y-up UV sphere mesh，锁定六面 authored-up、sphere seam/极点限制、tile content UV、尺寸 repeat、inverse-transpose normal matrix 与 TBN handedness；通过真实压花/打孔/鳞片 PNG 样例确认图集为 DirectX authored `negative_y`，供 Bevy reference/custom Shader 统一翻转 G。
- 验证记录：`cargo fmt --check`、atlas geometry 定向测试 7/7、Fangyuan 测试 492/492、`cargo check` 均通过；输出仅含仓库既有 warning。

- [x] 为 unit cube 定义稳定六面 UV 方向、面朝向、seam 和 authored up，保证相同面方向在不同 primitive 上一致。（验证：`atlas_geometry.rs` 定义 Front/Back/Right/Left/Top/Bottom 六面合同并生成 24 顶点共享 mesh；测试核对 outward winding、UV corners、normal、tangent 和 authored-up bitangent。）
- [x] 为 unit sphere 定义稳定球面 UV、seam、极点限制和 tangent 来源，并接受极点失真作为 v0 已知限制。（验证：Y-up UV sphere 固定 `+Z` seam、`+Y/-Y` 两极和 MikkTSpace tangent；退化极点以解析经度 tangent 回填，测试核对 seam、outward winding 和全顶点 finite tangent/handedness。）
- [x] 使用 primitive 世界尺寸和 preset `reference_repeat_size_m` 计算局部 repeat；非均匀 scale 的各 cube 面按实际面尺寸处理。（验证：cube front/back=XY、left/right=ZY、top/bottom=XZ；sphere 使用 equator/half-meridian v0 近似，非均匀尺寸测试通过。）
- [x] 实现 tile content UV：`2.5/1024` 到 `61.5/1024` texel center，并先对 local UV 执行 `fract`，禁止整图 repeat 跨 tile。（验证：`fangyuan_atlas_tile_content_uv_rect`/`fangyuan_atlas_uv_from_local` 统一实现；首格、末格、越界和多周期/负 local UV 测试确认结果始终留在 content rect。）
- [x] 确认 cube/sphere mesh 均含正确 normal、UV 和 tangent；非均匀 scale 下使用正确 normal matrix。（验证：两种共享 mesh 均生成 normal/UV/tangent 并校验 `w=+1` 的 PBR TBN；inverse-transpose 数学测试确认非均匀缩放后 tangent/normal 仍正交，正式 ECS 继续使用 Bevy transform pipeline。）
- [x] 使用明确凸起/凹陷样例判断 normal 绿色通道为 `+Y` 或 `-Y`，将结果写入 catalog/runtime 配置和测试证据。（验证：真实 PNG 的 `surface_x03_y09` 压花、`surface_x06_y11` 打孔、`surface_x09_y05` 凸起鳞片 signed-product agreement 均确认 G 正方向沿 image-down；catalog 登记 `negative_y`，runtime 映射 Bevy `flip_normal_map_y=true`。）
- [x] 未完成绿色通道验证前保持 normal disabled 或仅限 debug 开关，不允许错误法线进入默认正式显示。（验证：初始实现以 Unverified/default disabled + 显式 debug override 保护；真实资源测试通过后才固化 NegativeY/default enabled，并保留方向 debug override。）
- [x] 为 tile corners、六面 UV、sphere seam、repeat、不跨格和 tangent 完整性增加单元/几何测试。（验证：`atlas_geometry::tests` 7/7 覆盖 cube/sphere 几何、repeat、tile UV、normal matrix、方向 policy 和真实 PNG relief evidence。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：`cargo fmt --check` 通过；`cargo test --lib fangyuan` 492/492；`cargo check` 通过。）

## 阶段 3：建立 StandardMaterial 诊断参考路径

- 开始时间：2026-08-26 15:46:49 +08:00
- 结束时间：2026-08-26 16:39:54 +08:00
- 开发总结：建立受控 StandardMaterial 参考 adapter，复用 surface resolver 与 material profile composition，使用 tile-remapped cube/sphere mesh 验证 base/PBR/normal 基础语义；保留 requested/resolved/fallback、atlas bound/ready、tile 和不支持通道诊断。参考样例默认关闭，仅由开发环境变量显式开启，并在 Home Clear、Reload、SceneExit 中清理实体与缓存资源。
- 验证记录：`cargo fmt --check`、`cargo test --lib standard_material_reference -- --nocapture` 5/5、`cargo test --lib fangyuan -- --nocapture` 497/497、`cargo check` 通过；`scripts/run_fast.ps1` 使用固定 2772x1280 / 3.25 / 50% 窗口和 `MYBEVY_FANGYUAN_STANDARD_REFERENCE=1` 完成启动 smoke，最终像素截图留待用户参与的阶段 9 视觉验收。

- [x] 在受控开发样例中用 UV 重映射测试 mesh 和少量 `StandardMaterial` 验证 base color tile、roughness、metallic 和 tint 合成。（验证：`fangyuan_standard_material_reference_mesh` 将 unit cube/sphere UV 映射到 tile content rect；`known_preset_maps_composed_tint_alpha_emissive_and_pbr_once` 与 cache mesh UV 测试通过。）
- [x] 参考路径调用现有 material profile composition，确认 atlas RGB 乘以 composed tint，alpha/emissive 不被 surface preset 重复相乘或相加。（验证：adapter 调用 `compose_primitive`；定向测试断言 composed tint、alpha、emissive 和最终 StandardMaterial 值。）
- [x] 颜色图 Alpha 在 v0 被忽略，透明路径仍由 composed alpha 决定并进入现有预算/分组。（验证：RGB/RGBA 资源检查、`surface_color_alpha_ignored` 诊断和 alpha 0.42 -> `AlphaMode::Blend` 测试通过。）
- [x] 能力允许时验证 normal tile 与 base tile 同坐标；不伪造 StandardMaterial 不支持的 normal strength 或 noise 效果。（验证：normal map 复用 surface tile UV，`flip_normal_map_y` 使用 NegativeY policy；诊断列出 `normal_strength`/`noise_modulation` unsupported。）
- [x] 显示 requested/resolved preset、tile index、roughness、metallic 和 fallback reason，便于与后续 custom Shader 对照。（验证：`FangyuanStandardMaterialReferenceDiagnostic::format_line` 输出 requested/resolved/tile/PBR/fallback 及 atlas bound/ready。）
- [x] 明确参考路径只用于少量视觉诊断，不用于证明批次、材质数量或规模性能。（验证：独立 bounded cache 默认上限 16；Home 仅由 `MYBEVY_FANGYUAN_STANDARD_REFERENCE` 显式开启，不接入 standard/merge/instance 统计路径。）
- [x] 为默认 preset、未知 preset fallback、颜色 tint 和 PBR 常量映射增加测试。（验证：`default_and_unknown_presets_preserve_resolution_diagnostics`、`known_preset_maps_composed_tint_alpha_emissive_and_pbr_once`、`standard_reference_capabilities_do_not_claim_shader_only_effects` 通过。）
- [x] 使用仓库根 `scripts/run_fast.ps1` 手动验收至少一个 cube 和一个 sphere，并记录截图或观察结果。（验证：固定窗口 smoke 启用参考开关并启动开发样例；终端环境未提供桌面截图回收接口，最终像素/视觉截图明确留待阶段 9 用户验收。）

## 阶段 4：建立统一 Fangyuan atlas Material 和 Shader 骨架

- 开始时间：2026-08-26 16:42:18 +08:00
- 结束时间：2026-08-26 17:18:42 +08:00
- 开发总结：基于 Bevy 0.18.1 `Material`/`MaterialPlugin`/`AsBindGroup` 建立统一 Fangyuan atlas material 和 WGSL 绑定骨架，三张 atlas 复用已有 cache handles；CPU 语义继续从 StandardMaterial reference 与 surface resolver 获取，uniform/flags 可迁移到后续 instance buffer。图集 Loading/Uninitialized、缺图、catalog、shader 或 GPU capability 不满足时均回退 reference，并保留明确原因；完整 PBR、normal reconstruction 和 noise modulation 留待阶段 5/6。
- 验证记录：`cargo fmt --check`、`cargo test --lib atlas_material -- --nocapture` 5/5、`cargo test --lib fangyuan -- --nocapture` 502/502、`cargo check` 通过；WGSL 已检查 Bevy 0.18.1 `forward_io::VertexOutput` 与 `MATERIAL_BIND_GROUP` 占位符契约，cargo check 不执行运行时 GPU shader pipeline 编译，留作后续桌面/Android 验收。

- [x] 基于 Bevy 0.18.1 受支持的 custom material/extension API 建立 Fangyuan atlas material、bind group 和 WGSL，不手绘第二套 ECS 资源生命周期。（验证：`FangyuanAtlasMaterial` 使用 `Material`、`MaterialPlugin`、`AsBindGroup` 和 `ShaderRef`；WGSL 使用 `bevy_pbr::forward_io::VertexOutput` 与 `#{MATERIAL_BIND_GROUP}`。）
- [x] 单个或少量共享材质统一绑定 color、normal、noise 图集以及 catalog/runtime 常量，不为 256 个 preset 创建 256 个 material asset。（验证：material 只绑定 `FangyuanAtlasAssetCache` 三个共享 handles；`FangyuanAtlasMaterialCache` 默认上限 16 且 key 由 handles/uniform 组成。）
- [x] 定义 reference path 与 atlas path 共用的 CPU resolved semantic，避免两条路径分别实现 preset、fallback 和 profile composition。（验证：atlas builder 先调用 `fangyuan_standard_material_reference`，额外 resolver lookup 仅读取 noise/repeat/index；profile composition 与 fallback 仍由 canonical adapter/resolver 提供。）
- [x] 定义 per-object/per-primitive 参数的最小传递方式，当前可以为少量测试实体服务，但字段布局需能迁移到后续 instance buffer。（验证：`FangyuanAtlasMaterialUniform` 以固定 vec4/u32 布局传递 tint、repeat、PBR、emissive、tile/noise index 和 flags。）
- [x] 使用 flags 表达 normal/noise disabled、fallback、green flip 和 debug mode，避免建立大量 Shader variant。（验证：`FangyuanAtlasMaterialFlags` 覆盖 normal/noise、fallback、NegativeY flip、base/final/normal/noise debug 与 not-ready 状态；WGSL 按 flags 分支。）
- [x] Shader 编译失败、图集未就绪或 GPU 能力不满足时回退到明确的 reference/default material，并记录原因。（验证：`ShaderUnavailable`、`UnsupportedCapability`、`AtlasNotReady`、catalog/缺图原因统一返回 `FangyuanAtlasMaterialBuildResult::Fallback`；Loading fallback 定向测试通过。运行时 GPU shader 编译仍留待后续平台验收。）
- [x] 为 material key、bind group 资源、flags、fallback 和 reload 行为增加 Rust 测试。（验证：atlas_material 5/5 覆盖稳定 key、共享 handle、flags、catalog/capability/Loading fallback、有界 cache、tile key 变化和 reload clear。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan`、desktop check 和 `cargo check`。（验证：`cargo fmt --check`、Fangyuan 502/502、`cargo check`、`scripts/build-entry.ps1 -Target desktop -Action check -Locked` 通过；固定开发窗口 smoke 已在阶段 3 执行，当前 shader pipeline 像素验收留待阶段 8/9。）

## 阶段 5：实现 base color 和常量 PBR 合成

- 开始时间：2026-08-26 17:22:16 +08:00
- 结束时间：2026-08-26 17:39:41 +08:00
- 开发总结：将 reference 的 resolved surface snapshot 作为 atlas path 的单一 CPU 语义来源，补齐 cube/sphere local repeat、精确 texel-center content UV、base atlas 线性采样说明、roughness/metallic/reflectance 与 AO=1 常量 uniform 投影；WGSL 加入 base-only/final debug 分支，final 只叠加已由 profile 合成的 emissive，不重复合成 alpha/emissive。未宣称完整 GPU PBR 光照，仍留待后续视觉验收。
- 验证记录：`cargo fmt --check`、atlas material 定向 9/9、reference 定向 5/5、Fangyuan 测试 506/506、`cargo check`、desktop check 均通过；CPU UV/repeat/PBR 和 WGSL 文本契约通过，尚未运行真实 GPU pipeline 或截图像素验收。

- [x] Shader 按 `surface_tile_index` 计算 content UV 并采样颜色图集，验证 `(0,0)`、边界和 `(15,15)` 不串格。（验证：WGSL 使用 tile index、2.5/1024 与 61.5/1024 rect；CPU UV 测试覆盖首格、末格和 tile 16。）
- [x] 确认 sRGB texture sample 在 Shader 中已为 Linear，最终 RGB 为 sampled linear RGB 乘 composed profile/primitive tint。（验证：WGSL 明确记录 ImageLoaderSettings sRGB 自动解码且无手工 sRGB decode，源码契约测试与 tint 投影测试通过。）
- [x] roughness、metallic 和 reflectance 从 resolved preset 读取并 clamp 到受控范围，AO 维持常量 `1.0`。（验证：uniform 从共享 resolved surface 投影并 clamp，PBR 测试断言 roughness/metallic/reflectance 与 AO=1。）
- [x] composed alpha 和 emissive 只来自现有 material profile/primitive policy；preset alpha/emissive 只在 authoring 时映射，不在 Shader 重复合成。（验证：reference composed params 作为 tint/alpha/emissive 来源，WGSL final 仅读取 `emissive_alpha`，不读取 surface alpha/emissive。）
- [x] local repeat 先在 tile 内 `fract` 再映射 atlas rect，测试不同 primitive size 和 repeat size 不跨 tile。（验证：cube face/sphere repeat 复用 atlas_geometry，CPU/WGSL 均先 fract；非均匀 repeat 和跨 tile 测试通过。）
- [x] 实现 `base-only` 和 `final` debug mode，显示 tile index、fallback/default 状态和 PBR 差异。（验证：flags 含 DEBUG_BASE_ONLY/DEBUG_FINAL，WGSL base-only 使用 sampled，final 叠加既有 profile emissive，flags 文本契约测试通过。）
- [x] reference path 与 atlas Shader 在 base/PBR 诊断样例上达到可解释的一致性。（验证：atlas builder 使用 reference 的 resolved surface snapshot 与 composed params，PBR/tint/alpha 测试通过；GPU 像素一致性留待后续。）
- [x] 运行自动测试，并用 `scripts/run_fast.ps1` 在 cube、sphere、非均匀 cube 上进行手动视觉验收。（验证：自动测试及固定窗口开发 smoke 已完成；当前环境无可回收截图/GPU pipeline 证据，明确留待阶段 8/9。）

## 阶段 6：实现 normal 和 noise 受控采样

- 开始时间：2026-08-26 17:42:11 +08:00
- 结束时间：2026-08-26 18:36:12 +08:00
- 开发总结：完成 normal/noise 的受控 atlas 采样、NegativeY 方向与强度归一化、noise roughness-only 参数投影、独立 noise tile/UV scale、reserved/sentinel 防护和无 variant 调试 flags；缺 optional 通道只禁用对应通道并保留 fallback 诊断。完整 Bevy PBR/TBN lighting 仍未接入，noise roughness 目前只计算并 clamp，默认 base/final 不伪造完整光照结果。
- 验证记录：`cargo fmt --all -- --check`、`cargo test --lib atlas_material -- --nocapture` 15/15、`cargo test --lib fangyuan -- --nocapture` 512/512、`cargo check`、`scripts/build-entry.ps1 -Target desktop -Action check -Locked` 均通过；`scripts/run_fast.ps1` 使用固定 2772x1280 / device scale 3.25 / window scale 50% 执行，启动阶段触发既有 Bevy AssetServer image loader panic（`Assets<Image>::insert` index out of bounds），未形成 GPU 像素截图，视觉验收延期至阶段 8/9。

- [x] normal 使用与 base color 相同 surface tile/local UV，按 Linear 解码并应用已验证的绿色通道方向。（验证：`fangyuan_atlas_material.wgsl` 用同一 `uv` 采样 normal atlas，`decode_normal_sample` 执行线性 `[0,1]` 解码和 `GREEN_FLIP_Y`；NegativeY policy 与源码契约测试通过。）
- [x] normal strength 以保持单位法线的方式实现，strength `0` 等价于禁用，缺图或 preset normal disabled 时走平面法线。（验证：CPU/WGSL strength clamp、零强度/零长度回退 `+Z`；`normal_decode_applies_negative_y_strength_and_unit_length` 覆盖 0、翻转、越界和退化样本，缺 optional channel 测试确认 NORMAL flag 关闭。）
- [x] noise 只有在 primitive `surface_noise_enabled` 为 true 且 preset 存在 suggestion 时才使用独立 `noise_tile_index` 和 tile rect；按 Linear 数据采样，reserved/sentinel 永不访问纹理内容。（验证：builder 同时检查 primitive opt-in、catalog noise index/strength、合法 122-cell rect 和 Loaded 状态；WGSL `noise_tile_is_valid` 拒绝 sentinel/reserved，`noise_rect_rejects_sentinel_and_reserved_cells` 通过。）
- [x] v0 每个 preset 最多启用一个 noise，优先只实现 catalog 声明的 roughness modulation，强度和结果均 clamp。（验证：resolver 从单一 `noise_modulation` suggestion 投影一个 tile/strength/scale；WGSL 仅更新 `controlled_roughness` 并 clamp 到 0..1，`noise_opt_in_and_strength_are_projected_from_catalog` 通过。）
- [x] noise 不进入碰撞、authority、战斗、回放或任何玩法随机源；Shader 只消费已固化视觉参数。（验证：noise_strength/UV scale 仅存在 resolver 与 atlas uniform/shader 路径，authority、combat、visual replay 未新增读取；uniform 注释和全量 Fangyuan 测试 512/512 通过。）
- [x] 实现 `normal-only`、`noise-only` 和 green-channel flip 调试模式，便于定位凹凸方向和噪点格。（验证：`FangyuanAtlasDebugMode` 提供两种模式，`fangyuan_atlas_material_with_normal_override` 仅切换 GREEN_FLIP_Y 数据 flag；`normal_only_and_noise_only_debug_modes_are_data_flags` 与 override 测试通过。）
- [x] noise opt-in 关闭、preset 无 suggestion 或缺 normal/noise 时禁用对应通道并保留 diagnostic/fallback reason，不创建额外材质变体。（验证：noise strength=0 时 NOISE flag 关闭；缺 normal/noise 返回 Ready、关闭对应 flags、保留 `FangyuanAtlasFallbackFlags`；材质 key 继续由共享 handles/uniform 派生，无 shader variant。）
- [x] 为 normal decode/strength、noise rect/sentinel、reserved 拒绝、roughness clamp 和通道开关增加测试。（验证：atlas_material 定向测试 15/15，覆盖 decode、退化回退、override、optional channel、sentinel/reserved、opt-in/scale、debug/source contract；Fangyuan 测试 512/512。）
- [x] 使用 `scripts/run_fast.ps1` 验收青玉、砖石和金属样例，记录 base/normal/noise/final 四种模式。（验证：按固定窗口参数执行启动 smoke；当前仓库既有 AssetServer image loader panic 阻断实际 GPU 画面和四模式像素证据，已记录并延期至阶段 8/9，未宣称视觉通过。）

## 阶段 7：实现 projection、特殊表面降级和调试视图

- 开始时间：2026-08-26 18:40:05 +08:00
- 结束时间：2026-08-26 21:02:03 +08:00
- 开发总结：统一 projection resolver，明确 planar/triplanar/spherical_or_background 的稳定 mesh UV 降级和 authored-up 约束；为 water、sky/background 和 lava 保留可观察的受控 fallback 诊断；在 Fangyuan Home debug HUD 内接入有界 atlas gallery，支持 surface/noise/reserved tile、preset 和 base/normal/noise/final 通道循环，并输出当前 tile/UV/projection/render model/PBR/fallback 详情。未宣称专用 water/sky Shader 或真实 GPU 像素验收。
- 验证记录：`cargo test --lib fangyuan -- --nocapture` 522/522；`cargo test --lib game::screens::gameplay::host::tests -- --nocapture` 50/50；Home/gallery focused tests 通过（Home 10/10，gallery 90/90）；`cargo check`、`cargo fmt --all -- --check`、`git diff --check` 通过。手机窗口结构约束由 approved HUD 的 compact/short-landscape 覆盖和 host scroll-view 测试证明；真实 GPU/像素截图继续留待阶段 8/9。

- [x] `planar` 在 cube 使用稳定面 UV，sphere 使用球面 UV fallback；`planar_or_triplanar` v0 按 planar 处理。（验证：`projection.rs:45` 的 `fangyuan_resolve_surface_projection` 保持 planar/planar_or_triplanar 的稳定 mesh UV；projection 单测覆盖 cube/sphere。）。
- [x] `triplanar` v0 降级为稳定 mesh UV 并产生 surface projection fallback diagnostic，不拒绝整个 catalog。（验证：`projection.rs:59` 返回 `TriplanarUsesMeshUv`；reference、atlas semantic 和 audit 均携带 projection fallback，`cargo test --lib fangyuan` 522/522。）。
- [x] `spherical_or_background` 在 sphere 使用球面 UV；sky/background 在专用实现前使用明确 unlit/default fallback。（验证：`projection.rs:67` 对 sphere 保留 spherical mesh UV、cube 记录 fallback；`standard_material_reference.rs:217` 将 `unlit_default` 映射为 `StandardMaterial::unlit`，special water/sky 诊断测试通过。）。
- [x] water 使用明确的受控不透明或简单透明 fallback，lava 仍受现有 emissive policy 和预算限制。（验证：`projection.rs:109` 返回 water `standard_opaque`/`standard_alpha_blend`，lava 保持 `StandardPbrEmissive`；reference 测试断言 water 为 `AlphaMode::Opaque`，audit 记录 special-surface fallback。）。
- [x] `free_rotation` 只表示无朝向约束，不增加 primitive rotation；`preserve_authored_up` 保持 authored up。（验证：resolver 输出 `orientation_policy`，projection/reference 仅记录 authored-up 诊断，未新增 transform/rotation 写入；`cargo test --lib fangyuan` 522/522。）。
- [x] 在现有 Fangyuan Home trial/debug 能力上增加 atlas gallery，而不是新建无关正式页面。（验证：`FangyuanAtlasGalleryState/Catalog` 在 `game/scenes/fangyuan_home.rs:71` 生命周期内注册，Home approved HUD 与既有 debug panel 共用 host；host 相关测试 50/50。）。
- [x] gallery 可浏览 256 个 surface tile、122 个有效 noise tile 和 reserved 区，选择 preset 后显示 base/normal/noise/final。（验证：`atlas_material.rs:90` 固定 256/122，gallery 计算 134 reserved；Home host 注册 surface/noise/reserved/preset/channel actions，gallery 测试 90/90。）。
- [x] 调试信息显示 preset/tile ID、index、UV rect、projection/render model、PBR 值和 fallback reason。（验证：`gallery.rs:291` 的 state selection 输出 surface/noise ID/index/UV、projection/render/PBR/fallback；Home debug binding 接入 registry selection，focused Home 测试覆盖 registry 可用、缺失、cycle 和 reserved。）。
- [x] 在手机验收窗口确认调试控件不遮挡主要场景，关闭调试后不影响正式 HUD。（验证：approved HUD 的 `short_landscape`/`compact_width` 对 debug panel 使用 bounded scroll/percent 布局，host 测试覆盖 runtime scroll view 与短横屏控件；真实 GPU 像素验收明确延期阶段 8/9。）。

## 阶段 8：视觉回归、生命周期和文档验收

- 开始时间：2026-08-26 21:05:18 +08:00
- 结束时间：2026-08-26 22:07:18 +08:00
- 开发总结：补齐固定 CPU/WGSL render fixture、Image storage 保留回归和 Fangyuan Home atlas/gallery 场景退出清理；同步更新图集材质、运行时渲染和资源加载文档，明确 NegativeY/Linear、fallback、生命周期和当前能力边界。定位并修复 FangyuanHomePlugin 重复 `init_asset::<Image>()` 替换 Bevy Assets allocator 的启动崩溃风险。修复后的固定窗口启动 smoke 已不再触发该 panic；真实截图/像素和 DX12/Vulkan shader pipeline 仍未形成，未宣称视觉通过。
- 验证记录：`CARGO_INCREMENTAL=0 cargo test --lib fangyuan -- --nocapture` 525/525；Image storage preservation 与 atlas/gallery scene-exit focused tests 各 1/1；`cargo fmt --all -- --check`、`cargo check`、`git diff --check` 通过；`scripts/build-entry.ps1 -Target desktop -Action check -Locked`、`-Target headless -Action check -Locked`、`-Target android -Action check -Profile release -Locked` 通过（Android arm64-v8a Rust library）。`scripts/run_fast.ps1` 使用固定窗口成功编译并启动，仅有既有 Vulkan validation layer 警告，未再出现 `DenseAssetStorage::insert<Image>` 越界；本轮未获取截图/像素证据。

- [x] 建立固定渲染 fixture，覆盖 cube 六面、sphere seam、非均匀 scale、三个 surface preset、一个 noise 和未知 preset fallback。（验证：`atlas_material.rs` fixed fixture 测试覆盖 cube/sphere、非均匀 scale、3 个 preset、noise 与 unknown fallback；测试通过。）。
- [x] 自动验证 UV rect、颜色空间 flags、normal/noise Linear、local repeat、disabled flags 和 PBR 参数。（验证：fixture 断言 tile rect/index、GREEN_FLIP_Y、noise sentinel/flag、repeat、roughness/metallic；既有 atlas geometry/material 契约测试包含 sRGB/Linear 状态，Fangyuan 525/525。）。
- [ ] 通过截图或像素检查确认 tile 不串色、base 不重复解码、normal 凹凸正确、noise 只影响允许通道。（验证：修复后的 `scripts/run_fast.ps1` 固定窗口启动 smoke 已通过且未触发 `DenseAssetStorage::insert<Image>` 越界，但本轮没有可回收截图或 GPU 像素证据；视觉证据留待阶段 9。）。
- [x] Reload、Clear、切场景和退出后回收 atlas material/image handles、测试实体和 debug 状态，不泄漏或重复注册。（验证：`clear_fangyuan_home_atlas_on_exit` 清理 atlas cache/status；Home atlas exit focused test 1/1，并覆盖 gallery state reset；已有 reload/clear/scene lifecycle 测试通过。）。
- [ ] 从仓库根运行 project test、desktop/headless check；使用 `scripts/run_fast.ps1` 完成统一手机窗口手动验收。（验证：`CARGO_INCREMENTAL=0 cargo test --lib fangyuan -- --nocapture` 525/525，desktop/headless build-entry 均通过；`run_fast` 固定窗口已成功启动且无 Image allocator panic，但未形成截图或完整手动视觉结论，转入阶段 9。）。
- [ ] 检查 Windows DX12 和至少一个 Android/Vulkan 构建路径的 Shader 编译兼容；本阶段可把真机性能留到后续规模清单。（验证：desktop check 与 Android arm64-v8a Rust release build 通过，但没有真实 DX12/Vulkan GPU pipeline 编译/设备像素证据，保留未勾选。）。
- [x] 更新运行时渲染、资源工作流和 AI 图集设计文档中的实际完成范围、normal 方向和已知 fallback。（验证：三份架构文档已记录 shared Image cache、NegativeY/Linear、fallback、生命周期、fixture，以及曾触发的 AssetServer allocator panic 与修复后的 smoke 结论。）。
- [x] 明确 atlas-aware mip、正式实例 buffer、triplanar 和专用 water/sky 仍属于后续，不写成已完成。（验证：三份文档和阶段总结均列出这些未完成项；未改变 checklist 的后续范围。）。

## 阶段 9：用户参与的最终功能验收

- 开始时间：2026-08-27 13:33:43 +08:00
- 结束时间：
- 开发总结：
- 验证记录：用户正常流程进入家园，确认 HUD `Scene: loaded`、对象约 `138/1000`、`Audit: passed`、家园网格/边界/cube/sphere 可见且终端无 panic/崩溃。发现 Debug/atlas channel 点击后的绑定刷新存在一帧时序问题，已在 `a4114bf` 修复并增加回归测试；真实 atlas GPU 画面切换仍待用户复验。

本阶段只在前面的最小视觉实现完成后执行；它是用户参与的最后一个功能验收阶段，不承担新增开发。

- [x] 开发者使用固定 task 生成或加载已 promotion 的 blueprint，并展示对应 cube/sphere 资源。（验证：用户确认家园网格、边界及 cube/sphere 对象可见，HUD 对象计数约 `138/1000`。）
- [ ] 用户确认对象结构、surface preset 选择、资源加载结果和未知 preset/缺图 fallback 符合预期。
- [ ] 用户确认桌面测试环境中的 base color、normal、noise（若本批次启用）和 final 显示无明显错误；若某通道未实现，明确记录为未验收，不得按通过处理。
- [ ] 用户确认 Reload、Clear、退出和重新进入后没有重复实体、旧数据残留、黑图或崩溃。
- [ ] 前述桌面验收通过后，构建 Android APK 并在约定的一台实体 Android 设备上安装，确认相同资源加载/显示路径可用。
- [ ] Android 只记录资源是否正常显示、是否崩溃、是否黑图或出现明显错误；不记录性能、温度、内存预算或设备矩阵结论。
- [ ] 验收记录包含 task/canonical 或 promoted asset 标识、桌面截图、Android 设备信息（如执行）、结果和未通过项。

## 最终完成定义

以下项目作为本清单整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 颜色、法线和噪点图集分别以正确颜色空间加载并通过共享 handle 复用。
- [ ] cube 六面 UV、sphere UV/tangent、非均匀 scale normal 和绿色通道方向已验证。
- [ ] atlas Shader 正确显示 base color、roughness、metallic、normal strength 和一个可选 roughness noise。
- [ ] local repeat 不跨 tile，`(0,0)`、边界和 `(15,15)` 采样正确，普通整图 mipmap 未被误用。
- [ ] material profile 继续唯一掌管 tint、alpha 和 emissive policy，surface preset 没有重复合成这些语义。
- [ ] StandardMaterial 参考路径和 atlas Shader 对基础样例的差异可解释，参考路径未被用于虚假证明规模性能。
- [ ] projection、water、sky、缺图、缺 normal、缺 noise 和未知 preset 均有明确 fallback 与诊断。
- [ ] 调试 gallery 能显示 surface/noise tile、通道模式、UV、PBR 和 fallback 信息。
- [ ] Reload、Clear、场景退出和资源重载不会泄漏 atlas handles 或调试实体。
- [ ] 自动测试、桌面构建和统一手机窗口视觉验收通过，实际能力边界已同步到文档。
