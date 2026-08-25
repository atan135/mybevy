# 方圆图集资源合同与蓝图接入 Checklist

> 当前批次：实施。本清单是当前阶段的核心开发合同，必须逐阶段填写实现、验证和边界说明。
>
> 当前批次只负责 catalog、blueprint/runtime 数据、resolver、fallback、审核、identity 和 Bake 接口。图集 Shader、视觉采样、GPU instance、CPU merge 优化、规模性能和 Android 真机验收不在本清单内；分别由暂缓或后续任务处理。
>
> 当方圆03处于“暂缓”状态时，本清单中的 Standard、CPU merge 和 static instance 只验证字段、source mapping、fallback 和 hash 能进入边界结构，不得宣称已经完成图集渲染或实例化显示。

## 目标

建立方圆颜色、法线、噪点图集及表面材质预设的 Rust 数据合同、校验器和 runtime resolver，并把 `surface_material_preset_id` 与受控的 `surface_noise_enabled` 贯通 blueprint、runtime primitive、Prefab、Scene Layout、审核、统计、identity 和 Bake。

本清单完成后，旧 RON v1 蓝图在不填写新字段时保持兼容；256 个 surface preset 和 122 个有效 noise tile 可以解析为只读运行时表。本文不实现本地 Codex 生成流程、GPU atlas Shader、生产在线 AI、triplanar、专用水体/天空 Shader 或正式实例渲染。

## 基础原则

- [x] 保留现有 `material_profile_id` 的颜色、alpha、emissive policy、fallback 和预算语义，不把 256 个 surface preset 塞入 `FangyuanMaterialProfileRegistry`。（验证：surface registry 独立于 material profile registry；material compose、alpha/emissive、预算和旧行为测试通过。）
- [x] 第一版让 primitive 直接保存可选 `surface_material_preset_id` 和默认关闭的 `surface_noise_enabled`，暂不引入对象级 material binding table。（验证：blueprint/runtime primitive 字段与旧构造器默认 `None/false`；Prefab/Layout/static 边界仅传播这两个字段。）
- [x] `surface_noise_enabled` 只能启用 preset 已登记的 noise suggestion；第一版仍不允许指定 noise tile、strength、target 或任意材质参数。（验证：resolver `resolve_with_noise` 只接受 bool，catalog suggestion 决定可用性；blueprint unknown-field tests 拒绝 noise tile/UV/shader/参数。）
- [x] 开发期和跨版本数据保存稳定字符串 ID，运行时只在已校验的 catalog 版本内使用紧凑索引。（验证：RON/JSON 保留稳定 ID，resolver canonical preset/surface/noise indices 仅来自 validated catalog。）
- [x] AI 输出和 blueprint 只能引用稳定 ID，不允许携带任意文件路径、URL、UV、Shader 或脚本。（验证：文档合同、`deny_unknown_fields`、路径/任意字段拒绝测试和 fixture 均只使用受控 preset ID/noise bool。）
- [x] catalog 顶层错误使整份 catalog 不可用；单个 primitive 的未知 preset 进入安全 fallback，并保留 requested ID 和原因。（验证：bundle validator/registry 构造前 `ValidationFailed` 原子错误；resolver unknown fallback 保留 requested/resolved/reason/source。）
- [x] 每个阶段独立实现、验证和提交，完成后填写阶段元信息。（验证：阶段 1-8 均有开始/结束时间、总结、验证记录和独立提交；提交列表记录于整体验收记录。）

## 援引文档

以下文档用于说明 checklist 的需求来源和编制依据：

- 文档名称：方圆灵构 AI 生成与图集材质渲染
  - 路径或链接：[AI生成与图集材质渲染](../../docs/方圆灵构/架构/AI生成与图集材质渲染.md)
  - 用途说明：定义 catalog、稳定 ID、材质职责、resolver、fallback、版本和测试合同。
- 文档名称：方圆灵构蓝图格式与生成约束
  - 路径或链接：[蓝图格式与生成约束](../../docs/方圆灵构/规范/蓝图格式与生成约束.md)
  - 用途说明：约束当前 RON v1、primitive 字段、路径和旧蓝图兼容行为。
- 文档名称：方圆灵构审核与预算规则
  - 路径或链接：[审核与预算规则](../../docs/方圆灵构/规范/审核与预算规则.md)
  - 用途说明：约束 finding、预算、fallback 和报告行为。
- 文档名称：方圆灵构资源构建与加载
  - 路径或链接：[资源构建与加载](../../docs/方圆灵构/架构/资源构建与加载.md)
  - 用途说明：约束首包资源、hash、Bake 和加载依赖。

## 阶段 1：锁定 schema、路径和兼容边界

- 开始时间：2026-08-25 18:31:08 +08:00
- 结束时间：2026-08-25 18:37:11 +08:00
- 开发总结：完成三份 catalog 的真实字段、schema v1、稳定 ID、图集规格、companion 路径和枚举边界合同；将 `surface_noise_enabled` 纳入第一版最小字段，明确默认关闭及无 suggestion 时的 warning/fallback。仅修改架构和资源说明文档，未提前实现 Rust runtime。
- 验证记录：PowerShell 读取三份 JSON 确认 schema/catalog id 和 256/256/122 条目；核对 `tiles[].tile_id`、`presets[].surface_tile_id`、`presets[].preset_id` 及 `../noise/noise_atlas.json`；`git diff --check` 通过。

- [x] 复核 `surface_atlas.json`、`surface_material_presets.json`、`noise_atlas.json` 的真实字段，记录 `tile_id` 与跨目录 `surface_tile_id` 的字段差异。（验证：`docs/方圆灵构/架构/AI生成与图集材质渲染.md` 的 schema v1 表格记录三份顶层/条目字段；PowerShell 确认 surface/preset/noise 条目为 256/256/122。）
- [x] 锁定第一版稳定 ID 语法、最大长度、大小写规则、唯一性和排序规则，并确认 `surface_material_preset_id` 缺省等价于安全默认表面。（验证：AI 图集材质文档固定小写 ASCII、64 字节、catalog 内唯一、`index = y * 16 + x` row-major 和 `material:surface/xNN/yNN`；文档明确缺省使用安全默认表面。）
- [x] 锁定 `surface_noise_enabled` 缺省为 `false`；值为 `true` 但 preset 没有 noise suggestion 时产生 warning 并保持 noise disabled。（验证：AI 图集材质文档和 `project/assets/fangyuan/README.md` 固定默认关闭、只允许受控 bool；JSON 校验确认 248 个 preset 有 suggestion、8 个没有 suggestion。）
- [x] 在开始 Rust schema 实现前，把该受控 noise bool 同步回 AI 图集设计文档的“第一版最小字段”，替换文档中把全部 `noise_enabled` 推迟到后续 override 的旧表述。（验证：`AI生成与图集材质渲染.md` 第一版最小字段包含 `surface_noise_enabled`，禁止任意 noise tile/strength/path，并删除后续 override 表中的 `noise_enabled`。）
- [x] 锁定 catalog schema version、catalog id、图集尺寸、`16 x 16` 网格、`64 x 64` cell、`60 x 60` 内容和 `2px` padding。（验证：三份 JSON 均为 schema v1；catalog id 为 `fangyuan.base.surface`、`fangyuan.base.surface_material_presets`、`fangyuan.noise.procedural`；文档固定 1024 x 1024、16 x 16、64、60、2px。）
- [x] 明确 companion path 是只供首包 catalog 使用的受控路径，不复用 AI/blueprint 路径输入。（验证：AI 图集材质、资源构建与加载文档及 Fangyuan README 均明确 companions 仅供首包 catalog，AI/blueprint 只提交稳定 ID。）
- [x] 决定并记录 `../noise/noise_atlas.json` 的实现方式：使用专用 catalog-relative 规范化并校验最终路径仍在 `fangyuan/atlases/`，不得放宽 `fangyuan-core::validate_fangyuan_asset_path` 对普通资源的 traversal 禁令。（验证：文档记录从 catalog 所在目录规范化 `../noise/noise_atlas.json`，最终必须 containment 于 `fangyuan/atlases/`，且不放宽普通资源 traversal/绝对路径/drive/URI 禁止规则。）
- [x] 锁定普通 preset、water、sky、emissive preset 和 projection hint 的已知枚举集合；未知枚举为 catalog error，已知但未实现能力为 runtime diagnostic。（验证：文档固定 `standard_pbr`、`standard_pbr_opaque_surface`、`standard_pbr_emissive`、`special_water`、`special_unlit_sky` 及四类 projection；未知值为 catalog error，已知未实现能力进入 runtime diagnostic。）
- [x] 验证本阶段只形成合同或测试入口；如修改 Rust，运行 `cargo fmt` 和 `cargo check`。（验证：本阶段无 Rust 改动，worker 未运行 Cargo；文档 diff 已通过 `git diff --check`，JSON 合同读取校验通过。）

## 阶段 2：实现 catalog 数据模型和反序列化

- 开始时间：2026-08-25 18:39:21 +08:00
- 结束时间：2026-08-25 19:02:44 +08:00
- 开发总结：新增独立 `catalog.rs` schema v1 数据合同，覆盖 surface、surface preset 和 noise catalog 的运行时必需字段与 AI/authoring 字段；加入严格 serde unknown-field 策略、稳定 ID 校验、PBR/normal/noise/repeat 浮点范围校验和结构化字段错误。
- 验证记录：`cargo fmt --all -- --check`、`cargo test --lib fangyuan`（420 passed，0 failed）和 `cargo check` 通过；仅有仓库既有 unused/dead-code warnings。

- [x] 在 `project/src/framework/fangyuan/` 下建立聚焦的 surface/noise catalog 模块并从 `mod.rs` 导出，避免把解析逻辑堆入 `material_profile.rs`。（验证：新增 `project/src/framework/fangyuan/catalog.rs` 并由 `mod.rs` 注册和 `pub use`；模块注释明确跨 catalog join、图片元数据、companion path 和 resolver 留在后续阶段。）
- [x] 为三个 JSON 建立 `serde` 数据结构，启用未知字段策略并显式表达 optional normal、optional noise、projection、orientation 和 render model。（验证：`FangyuanSurfaceAtlasCatalog`、`FangyuanSurfaceMaterialPresetCatalog`、`FangyuanNoiseAtlasCatalog` 及全部 typed nested structs 使用 `deny_unknown_fields`；preset 结构包含 `Option<FangyuanPresetNormal>`、`Option<FangyuanNoiseModulation>`、`uv.projection`、`orientation_policy` 和 `render_model`。）
- [x] 为 `surface_tile_id`、`surface_material_preset_id` 和 `noise_tile_id` 建立受控验证函数或 newtype，错误中保留 catalog、字段路径和值。（验证：`validate_surface_tile_id`、`validate_surface_material_preset_id`、`validate_noise_tile_id` 和 `FangyuanCatalogFieldError { catalog_id, field_path, value, reason }`；ID 错误测试覆盖 catalog/path/value。）
- [x] 保留 catalog 中 AI 语义字段供开发工具读取，但把运行时必需字段和仅创作字段明确分层。（验证：typed catalog 保留 semantic/rendering/AI/authoring blocks，preset runtime PBR/normal/UV/render model 与 `authoring` 分离；`serde_json::Value` 仅用于文档型语义块。）
- [x] 确认浮点字段拒绝 NaN、Infinity、负法线强度、越界 PBR 参数和无效 repeat size。（验证：`validate_unit_finite`、`validate_bounded_finite`、`validate_non_negative_finite`、`validate_positive_finite` 覆盖 PBR、normal strength、noise strength/scale、repeat size 和 measured/identification 数值；invalid float 测试覆盖 metallic 越界与 normal strength 负值，非有限值在统一 helper 中拒绝。）
- [x] 为 schema version、catalog id、缺字段、未知字段和错误类型增加解析单元测试。（验证：catalog tests 覆盖真实三份资源解析、unknown top-level、missing required、错误上下文、invalid float、schema version 和 catalog id；`cargo test --lib fangyuan` 420 passed。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test --lib fangyuan`（420 passed/0 failed）和 `cargo check` 均通过。）

## 阶段 3：实现跨 catalog 与图片元数据校验

- 开始时间：2026-08-25 19:03:46 +08:00
- 结束时间：2026-08-25 19:37:39 +08:00
- 开发总结：在 `catalog.rs` 增加原子 catalog bundle validator、结构化 validation report/finding、surface/preset/noise 跨目录对齐、16x16 坐标与 range 完整性、PBR/AO/normal/noise/repeat 范围、PNG metadata 和 companion containment 校验；顶层错误统一返回失败 report，不产生半份 registry。
- 验证记录：当前三份 catalog 和 PNG bundle 通过；失败 fixture 覆盖 duplicate、reserved、坐标越界、range overlap/coverage/count、preset 引用重复/遗漏及路径逃逸；`cargo fmt --all -- --check`、`cargo test --lib fangyuan`（425 passed/0 failed）、`cargo check` 和 `git diff --check` 通过。

- [x] 校验 256 个 surface tile 的 ID、index、坐标唯一，且 `index == y * 16 + x`。（验证：`validate_surface_catalog` 检查 tile count、ID/index/坐标唯一、16x16 bounds 和 index/坐标公式，并返回 `duplicate_*`、`coordinate_out_of_bounds`、`index_coordinate_mismatch` finding。）
- [x] 校验 256 个 preset 与 surface tile 一一对应，preset index/坐标与引用 tile 一致，normal source 为空或引用同一 surface tile。（验证：`validate_preset_catalog` 检查 surface_tile_id 存在、引用集合覆盖 256 个 tile 且无重复、index/坐标一致和 normal source 对齐；测试覆盖重复引用和遗漏。）
- [x] 校验 noise catalog 恰有 122 个有效 tile、134 个 reserved cell，preset 只能引用有效 tile。（验证：`validate_noise_catalog` 检查 122 tile、122/134 grid counts、available/reserved ranges 边界/overlap/coverage 和 reserved cell；`validate_noise_references` 只接受有效 noise tile ID；失败 fixture 覆盖 reserved 引用。）
- [x] 校验 roughness、metallic、AO、reflectance、alpha、emissive、normal strength、noise strength 和 UV scale 的有限值及范围；当前 AO 必须为 `1.0`。（验证：bundle validator 使用 finite/unit/positive checks，emissive 为 0..=16，AO 单独要求 1.0，并以结构化 `parameter_out_of_range`/`ambient_occlusion_not_unit` finding 返回。）
- [x] 校验颜色、法线、噪点图片存在且均为 `1024 x 1024`，并核对 base color 为 sRGB 语义、normal/noise 为 Linear 数据语义。（验证：`validate_catalog_images` 使用 `png` header metadata 检查三张图存在、实际与声明尺寸均为 1024x1024，且声明 color space 分别为 sRGB/Linear；真实 bundle fixture 通过。）
- [x] 对 companion path 做规范化和根目录 containment 检查，覆盖合法相邻目录引用、绝对路径、drive、URL 和逃逸 traversal。（验证：`normalize_catalog_relative_path` 允许 `base/../noise/noise_atlas.json`，拒绝绝对路径、Windows drive、URI、编码分隔符、反斜杠和 root escape；测试覆盖全部路径类别。）
- [x] 顶层错误返回结构化 catalog validation report，不得静默跳过坏条目后继续构建半份 registry。（验证：`validate_fangyuan_catalog_bundle` 聚合全部 validator findings，任何 finding 均返回 `Err(FangyuanCatalogValidationReport)`，不提供部分 registry；报告包含 catalog_id、code、field_path、value、message。）
- [x] 使用当前三份 catalog 作为通过 fixture，并为 duplicate、reserved、越界和路径逃逸建立失败 fixture。（验证：当前 JSON/PNG bundle 通过测试；catalog tests 覆盖 duplicate ID/reference、reserved tile、坐标越界、range overlap/coverage/count 和 companion escape/URI/drive。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test --lib fangyuan`（425 passed/0 failed）和 `cargo check` 均通过。）

## 阶段 4：实现只读 resolver、fallback 和缓存身份

- 开始时间：2026-08-25 19:38:54 +08:00
- 结束时间：2026-08-25 20:03:50 +08:00
- 开发总结：新增独立 `resolver.rs` surface preset registry，与 `FangyuanMaterialProfileRegistry` 分离；实现 canonical `u16` 索引、只读 resolved preset、默认表面、primitive 级 fallback/source 诊断和三 JSON/三 PNG labelled content/dependency/cache identity。
- 验证记录：真实 catalog bundle 可原子构建 256 项 registry；`cargo fmt --all -- --check`、`cargo test --lib fangyuan`（429 passed/0 failed）、resolver 专项测试（4/4）、`cargo check` 和 `git diff --check` 通过。

- [x] 建立独立于 `FangyuanMaterialProfileRegistry` 的 surface preset registry，保证默认表面不挤占现有 material profile 容量。（验证：新增 `FangyuanSurfacePresetRegistry`/`FangyuanSurfacePresetTable`，未修改或引用 `FangyuanMaterialProfileRegistry`；默认表面作为 registry 内独立 resolved value。）
- [x] 把稳定字符串 ID 解析为 deterministic `u16` preset、surface tile 和 optional noise tile index；索引顺序由 canonical catalog 决定。（验证：`from_validated_catalogs` 按 `(index, preset_id)` canonical 排序并保留 u16 index；真实 fixture 断言首尾 preset index 为 0/255，surface/noise index 来自 catalog。）
- [x] 建立 `ResolvedFangyuanSurfacePreset` 等只读数据，包含 roughness、metallic、reflectance、normal strength、repeat size、projection hint、render model 和 flags。（验证：`ResolvedFangyuanSurfacePreset` 包含全部字段及 `FangyuanSurfacePresetFlags`；`projection` 与 `projection_hint` 均从受控 catalog enum 派生。）
- [x] 建立安全默认表面：白色/primitive tint、roughness `0.6`、metallic `0.0`、AO `1.0`、normal/noise disabled。（验证：`ResolvedFangyuanSurfacePreset::default_surface` 固定 roughness 0.6、metallic 0、AO 1、normal strength 0、noise indices None；注释明确颜色由 primitive tint 合成。）
- [x] 未知 primitive preset 返回 fallback，同时保留 requested ID、resolved ID、reason 和 source location；不得因单个 primitive 终止整个对象加载。（验证：`resolve_with_source_location` 保留 requested/resolved 两组字段、fallback reason（unknown/invalid/noise unavailable）和 source；未知 ID 测试通过，registry 对其他条目仍可用。）
- [x] catalog 图片、JSON 和 preset 共同参与 content/dependency hash，任一内容变化都会使 resolver cache 失效。（验证：`fangyuan_surface_preset_cache_identity_from_bytes` 对 3 JSON+3 PNG 使用 labelled content/dependency/cache hash；测试覆盖同输入稳定、JSON 变化和 image 变化触发不同 identity。）
- [x] 确认 resolver 只在加载、显式 reload 或依赖 hash 改变时重建，不在每帧解析 JSON 或查找长字符串。（验证：registry 仅通过 `load_from_paths`/`from_catalog_bundle` 构建，运行期通过 BTreeMap `get` 和 `needs_reload` identity 比较，不含每帧系统或 JSON 解析。）
- [x] 覆盖 deterministic ordering、同输入同 hash、字段变化触发 hash、fallback 和 cache invalidation 单元测试。（验证：resolver tests 覆盖 canonical ordering、默认/noise opt-in、unknown fallback/source、同输入 hash 及 JSON/PNG sensitivity；专项 4/4 通过。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test --lib fangyuan`（429 passed/0 failed）和 `cargo check` 均通过。）

## 阶段 5：扩展 blueprint 和 runtime primitive

- 开始时间：2026-08-25 20:05:24 +08:00
- 结束时间：2026-08-25 20:26:06 +08:00
- 开发总结：扩展 blueprint 与 runtime primitive 的受控 surface preset/noise 字段；保持旧 RON v1 缺省兼容，复用稳定 ID 校验并让未知但语法合法 ID 原样进入 resolver fallback；未引入 tile/index/path/UV/Shader 或 geometry 能力。
- 验证记录：`cargo fmt --all -- --check`、`cargo test --lib fangyuan`（434 passed，0 failed）、`cargo check` 和 `git diff --check` 通过；仅有仓库既有 unused/dead-code warnings。

- [x] 在 `FangyuanPrimitiveBlueprint` 增加 `surface_material_preset_id: Option<String>` 和 `surface_noise_enabled: bool`，均使用 serde default 保持旧 RON v1 可读。（验证：`blueprint.rs` 字段带 `serde(default)`，旧字段缺省测试得到 `None/false`，现有 RON fixture 继续加载。）
- [x] 在 `FangyuanPrimitive` 增加同名可选稳定 ID、noise opt-in、构造参数或 builder、只读 accessor 和 Debug/Clone/PartialEq 语义。（验证：`primitive.rs` 增加字段、`with_surface_metadata`、两个 accessor；结构体保留 `Clone/Debug/PartialEq`，构造器测试覆盖新字段。）
- [x] 更新 `compile_blueprint_primitive_to_runtime` 和 tolerant compile，使合法 ID 原样进入 runtime primitive，缺省仍保持旧视觉行为。（验证：严格与 tolerant 路径均调用同一编译函数；blueprint compile 测试断言 requested ID/noise 原样传播及旧默认。）
- [x] 新字段只允许稳定 preset ID 和布尔 noise opt-in，不接受 tile ID、index、路径、UV、noise 参数或任意 Shader 参数。（验证：`deny_unknown_fields` 保持开启；测试对 `noise_tile_id`、`uv`、`shader` 等任意字段解析失败。）
- [x] 明确 preset 中 alpha/emissive 只是创作建议；runtime primitive 仍通过现有 alpha/emissive 字段和 `FangyuanMaterialProfile::compose_primitive` 合成。（验证：新字段未修改 alpha/emissive 或 material profile 组成链，显式 material profile/alpha/emissive 测试保持原值。）
- [x] 覆盖旧蓝图无字段、新蓝图合法字段、noise 默认关闭/显式开启、preset 无 noise、空字符串、过长 ID、非法字符和未知但语法合法 ID 的测试。（验证：blueprint tests 覆盖缺省、合法 preset/noise、无 suggestion 仍由 resolver 处理、空/过长/大写/非 ASCII/非法语法、未知 ID fallback 保留。）
- [x] 确认新增字段没有引入 rotation、自由 mesh、cylinder 或其他方圆二相之外的能力。（验证：字段仅为 `Option<String>` 与 `bool`；blueprint 仍 `deny_unknown_fields` 且 kind 仅 cube/sphere，未新增 transform/mesh/shader 参数。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test --lib fangyuan`（434 passed/0 failed）和 `cargo check`，另通过 `git diff --check`。）

## 阶段 6：贯通 Prefab、Layout 和预留渲染输入边界

- 开始时间：2026-08-25 20:27:53 +08:00
- 结束时间：2026-08-25 21:02:19 +08:00
- 开发总结：将 surface preset ID/noise opt-in 从 Prefab、Layout 复制和展开贯通到 static merge、CPU mesh source range、static instance 与 render instance；surface 字段进入 source mapping 和 instance content hash，但不进入 merge/instance batch key，也未实现 Shader、GPU instance layout 或 CPU merge 顶点属性。resolver 的实际 fallback reason 仍由阶段 4 resolver/阶段 7 审核边界提供，本阶段不虚构第二套 fallback 状态。
- 验证记录：`cargo fmt --all -- --check`、`cargo test --lib fangyuan`（438 passed，0 failed）、`cargo check` 和 `git diff --check` 通过；仅有仓库既有 unused/dead-code warnings。

- [x] 将 preset ID 和 noise opt-in 贯通 `prefab.rs`、`layout.rs` 的 primitive 复制、变换、展开和 source location，不改变 layout 对 primitive rotation 的既有限制。（验证：Prefab/Layout 传播测试断言 ID/noise；现有 layout transform 校验和 rotation 限制未改动。）
- [x] 将新字段贯通 `static_merge.rs`、`static_mesh_builder.rs`、`static_instance.rs` 和 `static_instance_render.rs` 的输入边界、中间结构、source mapping 和 content hash；本阶段不实现 atlas Shader、GPU instance layout、CPU merge vertex attribute 或视觉输出。（验证：六模块字段与 source range/render instance 复制完成；static instance/render hash 测试通过，CPU mesh 仅保留 source range，未增加 vertex 属性或 Shader。）
- [x] 暂不把 surface preset 放入 batch key；在 Shader/实例布局未实现前只保留语义和统计，避免提前造成每 preset 分批。（验证：static merge 和 static instance 测试使用不同 preset/noise 仍断言单一 batch/group；batch key 结构仍仅按 kind/profile/transparency。）
- [x] Standard、CPU merge 和 static instance 对同一 primitive 保留相同 requested preset ID 和 fallback 信息。（验证：Standard 路径继续持有同一 runtime primitive；CPU merge source range、static merge source ref、static instance/render instance 均保留 requested ID/noise。实际 resolver fallback reason 继续由 resolver 统一产生，本阶段未复制第二套 reason 字段。）
- [x] Reload、Clear、场景退出和重新进入后，新字段不会残留到其他对象或丢失 source mapping。（验证：字段仅附着于 primitive/source/instance 的场景拥有数据，source 按 primitive 同步；现有 home reload/clear/exit/re-enter 测试组通过，未改变生命周期清理路径。）
- [x] 为 blueprint、Prefab、Layout、CPU merge 和 static instance 边界结构分别增加一条字段传播测试；测试只证明数据合同，不证明对应渲染路径已经实现。（验证：blueprint 阶段测试、Prefab/Layout 测试、static merge/mesh source range、static instance/render 测试均覆盖 ID/noise 传播和 hash/batch 边界。）
- [x] 更新任何硬编码 primitive 构造器、测试 fixture 和 expected hash，避免遗漏字段导致编译或一致性回退。（验证：所有构建通过；新增字段默认值由既有 constructors 统一提供，surface hash sensitivity 与 direct constructor source-sync 测试通过。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 `cargo check`。（验证：主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test --lib fangyuan`（438 passed/0 failed）、`cargo check` 和 `git diff --check`。）

## 阶段 7：接入审核、统计、identity 和 Bake

- 开始时间：2026-08-25 21:04:01 +08:00
- 结束时间：2026-08-25 21:53:52 +08:00
- 开发总结：新增 surface 专用审核 finding、唯一 preset/noise 统计与 4/24 预算；把 resolver fallback 的 requested/resolved/reason/noise 信息接入 debug report；surface preset 作为独立 identity dependency 进入 blueprint/prefab，并通过 Bake dependency expansion 进入 layout identity；Bake dependency table、typed payload、content/normalized hash 对新字段敏感，旧 artifact 缺字段按 serde default 兼容。noise 只进入视觉/catalog 依赖，未进入 authority、碰撞、战斗或确定性玩法 hash。
- 验证记录：`cargo fmt --all -- --check`、`cargo test --lib fangyuan`（445 passed，0 failed）、`cargo check`、`git diff --check` 和 `..\scripts\build-entry.ps1 -Target fangyuan-bake -Action check -Locked`（passed）通过；仅有仓库既有 unused/dead-code warnings。

- [x] 新增 surface 专用 finding code，至少覆盖 unknown preset、reserved noise、unsupported render model、projection fallback 和 catalog unavailable，不复用含义不同的 material profile finding。（验证：`audit_fangyuan_surface_usage` 提供 `unknown_surface_preset`、`reserved_noise_tile`、`unsupported_surface_render_model`、`surface_projection_fallback`、`surface_catalog_unavailable`，finding 携带稳定 field/path 和 surface reason；audit focused tests 通过。）
- [x] 在审核报告中统计对象使用的唯一 surface preset 数量，第一版 recommended budget 为 4，并阻止用大量 preset 绕过现有材质预算。（验证：`FangyuanPrimitiveBudgetStats` 统计唯一 surface preset/noise，默认 profile 固定 recommended=4、hard=24，超限 finding 使用既有 `ReducePrimitives` action；4/24 focused budget tests 通过。）
- [x] 将 fallback count、requested/resolved preset、surface/noise tile 和降级原因接入 debug report 或稳定日志模型。（验证：`FangyuanDebugReportSurfaceSummary` 序列化 requested/resolved/noise/fallbacks，使用 resolver 的 fallback reason；surface debug JSON round-trip tests 通过。）
- [x] 将 surface preset dependency 加入 `identity.rs` 的 blueprint/prefab/layout 依赖和 order-independent dependency hash。（验证：Blueprint/Prefab 直接收集 surface dependency；Layout 通过 Bake dependency catalog 的 prefab expansion 注入 surface dependency，新增 layout identity expansion 测试验证 hash 随 preset 变化，dependency hash 对顺序无关且对 ID 敏感。）
- [x] 扩展 `bake.rs` 的 dependency table、typed payload、schema hash 和 runtime validation，使新字段进入 canonical 内容和缓存失效。（验证：dependency table 增加 `SurfaceMaterialPreset` kind/ID；typed payload 与 payload/content/normalized source hash sensitivity 测试通过；runtime dependency resolution 覆盖新 kind，Bake 20/20 通过。）
- [x] 旧 artifact 的升级或拒绝策略明确且有测试；不得把不含新字段的合法旧资产误判为损坏。（验证：`surface_material_preset_ids` 使用 `serde(default)`；legacy dependency artifact 缺字段解析为空且保持 complete，旧 runtime/bake tests 继续通过。）
- [x] noise 只作为视觉依赖，确认 authority、碰撞、战斗和确定性规则 hash 不从 noise 像素采样结果取值。（验证：identity/bake 新依赖只记录 surface preset ID 与 catalog/visual hashes，未向 authority、collision、combat 或 deterministic rule 模块增加 noise 输入；现有 deterministic tests 445 全通过。）
- [x] 覆盖 audit、stats、identity、Bake round-trip、hash sensitivity 和旧版本兼容测试。（验证：新增 audit/debug/identity/Bake focused tests（surface focused 19/19，Bake focused 20/20）及完整 fangyuan 445/445 通过。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan`、`..\scripts\build-entry.ps1 -Target fangyuan-bake -Action check -Locked` 和 `cargo check`。（验证：主 agent 独立运行全部命令通过，`git diff --check` 通过。）

## 阶段 8：集成验收和文档同步

- 开始时间：2026-08-25 21:55:02 +08:00
- 结束时间：2026-08-25 22:16:51 +08:00
- 开发总结：新增 surface contract 集成 fixture 和 resolver 验收测试，覆盖真实 256 preset、122 noise tile、默认索引/hash、旧首包蓝图/Prefab/Palette/Layout、已知 preset/noise opt-in、未知 preset fallback 及坏 catalog 原子失败；同步蓝图格式、审核预算、运行时架构和资源构建文档，明确已落地边界与 Shader/GPU/在线 AI 未完成边界。
- 验证记录：resolver 集成测试 9/9；主 agent `build-entry.ps1 -Target test -Action check -Locked` 通过（2151 passed，1 ignored）；worker 已通过 desktop/headless/fangyuan-bake locked check；`cargo fmt --all -- --check` 和 `git diff --check` 通过。`docs/引擎入门使用文档.md`、`docs/README.md`、`CLAUDE.md` 无需变更，因为项目目录、初始化、Bevy 版本和资源约定未改变。

- [x] 使用当前 256 surface preset 和 122 noise tile 构建 resolver，记录数量、hash、默认索引和加载错误为零。（验证：resolver integration test 断言 registry=256、noise catalog=122、preset index 0/255、default index 0、三类 cache hash 非零，真实 bundle 构建无错误。）
- [x] 加载现有 `minimal_player.ron`、`home_preview.ron`、默认 Prefab/Palette 和 Scene Layout，确认旧资源无需补字段即可通过。（验证：resolver integration test 加载四类现有首包资源；旧 RON 缺省字段保持 `None/false`，layout 对 palette 校验通过。）
- [x] 增加最小新蓝图 fixture，覆盖 cube、sphere、三个 surface preset、一个 noise suggestion 和一个未知 preset fallback。（验证：新增 `project/assets/fangyuan/avatars/surface_contract_fixture.ron`，集成测试编译 4 primitives 并断言 cube/sphere、x00/x01/x02、noise opt-in 和 x99 fallback。）
- [x] 验证 catalog 坏文件不会产生半份 registry，单 primitive 未知 ID 不会导致整个场景崩溃。（验证：resolver test 篡改 tile ID 后得到 `ValidationFailed`、不构造 registry；fixture 未知 ID 只由 resolver 返回 default fallback。）
- [x] 从仓库根运行 `scripts/build-entry.ps1 -Target test -Action check -Locked`、desktop/headless/fangyuan-bake check。（验证：主 agent test locked check=2151 passed/1 ignored；worker desktop、headless、fangyuan-bake locked check 均 passed。）
- [x] 更新 `docs/方圆灵构/规范/蓝图格式与生成约束.md`、审核规则、运行时架构和资源加载文档中的实际落地边界。（验证：四份指定方圆文档同步 surface contract、审核 finding/budget、resolver/hash/Bake 依赖和未实现边界；无相邻重复。）
- [x] 检查 `docs/引擎入门使用文档.md`、`docs/README.md` 和 `CLAUDE.md` 是否需要同步；无变化时在验证记录说明理由。（验证：本阶段未改变目录、初始化、Bevy 版本或资源约定，三份文件无需修改，理由已记录。）
- [x] 确认后续本地 Codex、Shader 和实例渲染 checklist 只依赖本清单公开接口，不重复定义第二套 schema。（验证：文档和 fixture 均只公开稳定 preset ID/noise bool、resolver/finding/identity/Bake 边界；明确 Shader/GPU/在线 AI 留待后续，不新增第二套字段合同。）

## 阶段 8 之后：整体验收记录

- 本批开发提交：`ca389bf`、`8086829`、`870d1fe`、`c904131`、`1b8e236`、`a1bef0a`、`ccb2e3f`、`5333343`。

## 最终完成定义

以下项目作为本清单整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-25 18:39:21 +08:00
- 结束时间：2026-08-25 22:19:00 +08:00
- 验收总结：完成 catalog 合同/schema/跨目录校验、独立 resolver、blueprint/runtime/Prefab/Layout/static 边界、审核统计与 fallback debug、identity/Bake 依赖和集成 fixture；旧 RON/首包资源兼容，三类 locked 构建和全量测试通过。坏 catalog 采用结构化原子拒绝，单 primitive 未知 preset 采用安全默认表面 fallback。Shader/GPU instance/在线 AI 仍明确留待后续。

- [x] 三份 JSON 和三张图集能作为一个版本依赖闭包被完整校验和解析。（验证：真实 bundle resolver 集成测试通过，三 JSON/三 PNG 均参与 catalog validation 与 cache identity。）
- [x] 256 个 surface preset 与 256 个 surface tile 一一对应，122 个 noise tile 可用且 reserved cell 永不进入 resolver。（验证：catalog bundle validator 与 resolver 集成测试断言 256/256/122；reserved range/reference 测试通过。）
- [x] `surface_material_preset_id` 与 `surface_noise_enabled` 已贯通 blueprint、runtime primitive、Prefab、Layout、Standard 输入、CPU merge、static instance、审核、identity 和 Bake。（验证：阶段5-7传播、审核、identity、Bake 与阶段8 integration fixture 全部通过。）
- [x] 旧 RON v1 和旧首包资源保持兼容，缺省新字段继续使用安全默认表面。（验证：旧 blueprint/prefab/layout 加载测试通过；serde default 得到 `None/false`，resolver default surface index 0。）
- [x] surface registry 与 `FangyuanMaterialProfileRegistry` 分离，现有 color/alpha/emissive policy 语义未改变。（验证：独立 `FangyuanSurfacePresetRegistry`；material profile compose/render tests 与全量测试通过。）
- [x] catalog 顶层错误、单 primitive 未知 ID、缺图、缺法线和缺噪点均具有稳定且可诊断的处理。（验证：catalog bundle 在 registry 构造前以结构化 `ValidationFailed` 原子拒绝坏/缺失资源；未知 primitive 由 resolver 保留 requested ID 并 fallback default，audit/debug 记录 reason。）
- [x] canonical、hash、Bake 和 resolver cache 对 catalog 或新字段变化敏感，并保持同输入同结果。（验证：resolver JSON/PNG identity、Bake payload/content/normalized hash、identity dependency hash sensitivity/order-independence tests 通过。）
- [x] 视觉 noise 未进入 authority 或玩法确定性逻辑。（验证：noise 只存在 surface resolver/debug/identity visual dependency，未进入 authority/collision/combat/deterministic rule 输入；全量 lockstep/determinism tests 通过。）
- [x] 相关单元测试、集成测试和统一构建检查通过。（验证：`cargo test --lib fangyuan` 445/445、根 `build-entry test --locked` 2151 passed/1 ignored，desktop/headless/fangyuan-bake locked check 通过。）
- [x] 实际实现边界已同步到方圆文档，未把 Shader 或在线 AI 写成已完成能力。（验证：四份方圆文档记录 catalog/resolver/runtime/audit/Bake 已落地，并明确 atlas Shader、GPU instance buffer、在线 AI provider 未完成。）
