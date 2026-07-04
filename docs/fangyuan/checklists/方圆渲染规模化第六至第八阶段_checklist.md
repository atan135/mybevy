# 方圆渲染规模化第六至第八阶段 Checklist

## 目标

合并推进方圆系统第六至第八阶段：静态 CPU 合并渲染、静态实例化渲染原型和统一 FangyuanMaterial。目标是在现有 primitive、prefab、layout、audit 和预算系统基础上，建立可回退、可观测、可继续扩展的渲染规模化底座。

本 checklist 重点处理静态内容的 draw call / Entity 压力、shared mesh + instance data 原型、材质 profile 和透明/发光预算。本阶段不实现动态 VFX、技能规则层、装备 NPC 接入、Chunk / LOD / AOI、发布期 Bake、蓝图缓存和纪元继承。

## 功能地图

| 功能域 | 处理方式 |
| --- | --- |
| 静态 CPU 合并 | 按区域、kind、material profile、透明路径分组生成少量 mesh |
| 静态实例化 | 对大量静态 cube / sphere 使用 shared mesh + instance data 原型 |
| 统一材质 | 建立 FangyuanMaterialProfile，颜色、alpha、emissive 优先走实例数据 |
| Fallback | 保留 standard render 和 CPU merge fallback，便于排错和回退 |
| 调试统计 | 输出 group、mesh、instance、batch、buffer、profile、alpha、emissive 摘要 |
| 非目标 | 不做动态技能 VFX、Chunk、LOD、AOI、Bake、联网同步或任意 shader |

## 基础原则

- [x] 所有渲染路径都消费已审核通过的方圆 runtime 数据，不绕过 audit / validator。（验证：`project/src/game/scenes/fangyuan_home.rs:2388` 日志输出 audit_status / audit_errors / audit_warnings 与 render_mode 同步；`cargo test fangyuan -- --nocapture` 234 passed）
- [x] primitive 不能变成正式玩法 Entity；调试实体必须隔离在 debug 或 fallback 路径。（验证：`project/src/framework/fangyuan/primitive.rs` 保持 `FangyuanPrimitiveSet` 数据边界；`project/src/game/scenes/fangyuan_home.rs:4219` 阶段 9 测试断言 static instance / CPU merge 路径清理 render-only 内容且不残留）
- [x] 静态 CPU merge、static instancing 和 standard render 必须能显式切换或回退。（验证：`project/src/game/scenes/fangyuan_home.rs:386` 读取 `MYBEVY_FANGYUAN_HOME_RENDER_MODE`，`:399` 解析 standard / cpu_merge / static_instance，`:4219` 阶段 9 测试覆盖 StaticInstance -> CPU merge 模式切换和退出清理）
- [x] 材质自由度通过受控 profile 和实例字段表达，不引入玩家任意 shader。（验证：`project/src/framework/fangyuan/material_profile.rs:21` 定义 `FangyuanMaterialProfile`，`docs/世界观/方圆灵构蓝图规则.md:479` 明确 `material_profile_id` 不允许引用 shader、脚本、外部纹理、模型路径或任意材质程序）
- [x] 透明和发光必须单独预算、单独统计，并预留热点降级入口。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:257` 起 HUD 输出 matprof / opaque / trans / emi / uniq；`project/src/framework/fangyuan/material_profile.rs:927` 起测试覆盖透明、alpha、profile 预算 finding）
- [x] 不引入 rotation、quaternion、euler、angular_velocity、rotate 或 spin 能力。（验证：`docs/世界观/方圆灵构蓝图规则.md:584` 仍禁止 rotation、quaternion、euler、angular_velocity、rotate、spin；本 checklist 改动未新增相关字段）
- [x] 每个阶段完成后运行对应验证，并按阶段提交。（验证：阶段 1-10 均记录验证；代码提交包括 `657c387 test(fangyuan): 补充家园渲染阶段验收`，文档提交包括 `5e79b44 docs(fangyuan): 同步渲染规模化文档`）

## 阶段 1：渲染规模化边界复核

- 开始时间：2026-07-03 10:32:30 +08:00
- 结束时间：2026-07-03 13:06:10 +08:00
- 开发总结：完成第六至第八阶段渲染规模化边界只读复核，确认本 checklist 应先在已审核 runtime primitive / layout / prefab 数据之上推进静态 CPU 合并、静态实例化和统一材质 profile；动态 VFX、技能规则层、Chunk、LOD、AOI、Bake 和缓存继承继续留在后续 checklist。阶段 1 未修改业务代码。
- 验证记录：worker 执行只读 `git status --short`、`rg`、`Get-Content` / PowerShell 片段读取并汇报首尾 `git status --short` 无输出；主 agent 复核后再次执行 `git status --short` 无输出。

- [x] 复核方圆技术路线中阶段 6、7、8 的目标、验收、风险和控制方式。（验证：worker 复核 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1657`、`:1688`、`:1719`，确认阶段 6 为静态 CPU 合并且动态玩家/技能/短生命周期 primitive 不进静态合并，阶段 7 为静态实例化且保留 standard/CPU merge fallback，阶段 8 为统一 `FangyuanMaterial` 且透明/不透明分通道和透明预算受控）
- [x] 复核现有 `project/src/framework/fangyuan/` 中 primitive set、stats、audit、layout compile report、家园预览和资源路径。（验证：worker 复核 `project/src/framework/fangyuan/primitive.rs:311` 的 `FangyuanPrimitiveSet`、`stats.rs:14` 的 stats 扫描、`layout.rs:604` 的 layout compile report、`audit.rs:9` 的 audit report、`prefab.rs:16` 的默认 palette path、`project/src/game/scenes/fangyuan_home.rs:1086` 的家园 layout/palette 加载和 spawn 路径）
- [x] 明确 static、dynamic、debug fallback 三类内容边界，确定哪些 lifecycle / role 可进入静态路径。（验证：worker 复核 `project/src/framework/fangyuan/primitive.rs:45`、`:132`、`render_assets.rs:28`、`project/src/game/scenes/fangyuan_home.rs:1440`；建议 static 首版只纳入静态家园/测试场景 layout/palette 展开且 lifecycle 为空的内容，role 先允许 `structure`/`core`/`boundary`/`decoration`/`archive` 并结合对象静态性判断；dynamic 排除玩家、技能、短生命周期、非空 lifecycle、`warning`/`trail`/`impact`，`socket` 先作为语义锚点；debug fallback 保留当前标准 render-only 子实体路径）
- [x] 明确本 checklist 不处理动态 VFX、技能规则层、Chunk、LOD、AOI、Bake 和缓存继承。（验证：worker 复核 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1747`、`:1828`、`:1861` 和 `docs/世界观/方圆灵构蓝图规则.md:551`，确认 VFX 属阶段 9，Chunk/LOD/AOI 属阶段 12，Bake 属阶段 13，缓存/继承属于后续能力）
- [x] 验证命令：只读 `rg`、`Get-Content`、`git status --short`。（验证：worker 实际使用 `git status --short`、`rg`、`Get-Content` / PowerShell 片段读取，未运行写入、格式化、测试、commit 或修改命令；主 agent 再次执行 `git status --short` 无输出）

## 阶段 2：静态分组和合并输入模型

- 开始时间：2026-07-03 13:07:06 +08:00
- 结束时间：2026-07-03 13:46:48 +08:00
- 开发总结：新增 framework 方圆静态合并分组模型，包含 source 定位、merge group key、可合并筛选、layout / runtime primitive set 纯数据入口和统计摘要；本阶段只做数据分组和测试，未接入场景、未生成 Mesh、未实现渲染系统。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_static_merge_group -- --nocapture` 通过（5 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 通过，仅有 Git LF/CRLF 提示。

- [x] 新增静态合并输入模型，记录 source kind、source path、layout instance、prefab id、primitive index 和审核摘要定位。（验证：`project/src/framework/fangyuan/static_merge.rs:45` 定义 `FangyuanStaticMergeSourceRef`，包含 source_kind/source_path/layout_instance/prefab_id/primitive_index/field_path/audit_code/audit_reason；`:115` 定义 `FangyuanStaticMergeInput`）
- [x] 定义稳定 merge group key，覆盖区域占位、primitive kind、material profile、透明路径和 debug label。（验证：`project/src/framework/fangyuan/static_merge.rs:158` 定义 `FangyuanStaticMergeGroupKey`，字段覆盖 region_placeholder/primitive_kind/material_profile/transparent_path/debug_label，并补 color/emissive key 防止错误合批；`:274` 使用 `BTreeMap` 按 key 稳定分组）
- [x] 从 runtime primitive set 或 layout compile output 中筛选可合并 primitive，跳过动态 lifecycle、短生命周期和非法内容。（验证：`project/src/framework/fangyuan/static_merge.rs:333` 定义 runtime primitive set 入口，`:392` 定义 layout + palette 入口，`:474` 的 `fangyuan_static_merge_skip_reason()` 跳过非空 lifecycle、`Warning`/`Trail`/`Impact`、`Socket`、非法 transform/scale/color/alpha/emissive/material profile）
- [x] 生成分组统计，包括 authored、expanded、merged group、cube、sphere、skipped、material profile 和 estimated vertex/index 数。（验证：`project/src/framework/fangyuan/static_merge.rs:253` 定义 `FangyuanStaticMergeStats`，包含 authored_primitives/expanded_primitives/merged_group_count/cube_count/sphere_count/skipped_primitives/material_profile_count/estimated_vertex_count/estimated_index_count；`:274` 起构建报告时填充统计）
- [x] 为分组 key 稳定性、动态内容跳过、透明分组和 source 定位补单元测试。（验证：`project/src/framework/fangyuan/static_merge.rs:585`、`:642`、`:703`、`:769`、`:811` 的 5 个 `fangyuan_static_merge_group_*` 测试覆盖 key 稳定排序、dynamic/short-lived/socket 跳过、非法 runtime 内容、透明分组和 layout source 定位）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_static_merge_group -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；`cargo test fangyuan_static_merge_group -- --nocapture` 为 5 passed；`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 3：CPU Mesh Builder 和家园接入

- 开始时间：2026-07-03 13:49:15 +08:00
- 结束时间：2026-07-03 15:14:32 +08:00
- 开发总结：新增方圆静态 CPU mesh builder，支持 cube 和低成本 sphere 合并、mesh metadata/source range/bounds 记录、预算失败报告；家园预览新增显式 CPU merge 渲染模式，默认仍为 standard，并覆盖 Reload、Clear、场景退出、失败 fallback 和模式切换清理路径。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_static_merge -- --nocapture` 通过（9 passed）；`cargo test fangyuan_home -- --nocapture` 通过（44 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 通过，仅有 Git LF/CRLF 提示。

- [x] 实现 cube primitive 到合并 Mesh 顶点、法线、UV、颜色或等价 attribute 的构建逻辑。（验证：`project/src/framework/fangyuan/static_mesh_builder.rs:388` 的 `push_cube()` 写入 position/normal/uv/color/index；`:597` 的 `fangyuan_static_merge_cpu_builder_merges_cube_vertices_normals_uvs_and_colors` 断言 24 顶点、36 索引和 `Mesh::ATTRIBUTE_COLOR`）
- [x] 实现 sphere primitive 的低成本合并策略，明确细分级别、顶点上限和预算约束。（验证：`project/src/framework/fangyuan/static_mesh_builder.rs:456` 的 `push_sphere()` 使用 sectors/stacks；`project/src/framework/fangyuan/static_merge.rs:17`-`:23` 暴露默认 24 sectors / 12 stacks、325 vertices、1728 indices；`static_mesh_builder.rs:653` 和 `:697` 测试覆盖低成本 sphere 和 budget exceeded）
- [x] 在 mesh builder 中保留 bounds、source range、primitive count 和 debug name，方便定位问题。（验证：`project/src/framework/fangyuan/static_mesh_builder.rs:62` 定义 `FangyuanStaticMeshMetadata`，包含 bounds/source_ranges/primitive_count/debug_name/vertex_count/index_count；`:110` 定义 `FangyuanStaticMeshSourceRange`；`:335` 构建 metadata；`:720` 测试 source range）
- [x] 为方圆家园或开发场景增加 CPU merge 渲染模式开关，默认策略与当前预览兼容。（验证：`project/src/game/scenes/fangyuan_home.rs:307` 定义 `FangyuanHomeBlueprintRenderMode::{Standard,CpuMerge}`；`:320` 的默认配置为 `Standard` 且 `fallback_to_standard_on_merge_failure=true`；`:1198` 起仅在 `CpuMerge` 模式调用 `fangyuan_static_meshes_from_primitive_set_with_source()`）
- [x] Reload、Clear、场景退出和 layout 变更时正确释放旧合并 mesh、material handle 和统计状态。（验证：`project/src/game/scenes/fangyuan_home.rs:331` 定义 `FangyuanHomeStaticMergeRuntime`，`:339` 的 `clear_assets()` 移除 mesh/material handles 并重置 stats/failure；`:1671`、`:1681` 在 Clear/Reload 前清理；`:1777` 的 exit 系统清理场景退出）
- [x] 为默认 `home_layout.ron` 合并路径、失败回滚、重复 reload、clear 后 reload 和 fallback 切换补测试。（验证：`project/src/game/scenes/fangyuan_home.rs:3281` 覆盖默认 layout CPU merge，`:3329` 覆盖 reload/clear/clear 后 reload 不积累资产，`:3381` 覆盖 build failure fallback 到 standard 并清旧 merge assets，`:3419` 覆盖预算恢复后切回 merge）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_static_merge -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行全部通过；static merge 9 passed，fangyuan_home 44 passed，`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 4：静态 Instance 数据模型和分组缓存

- 开始时间：2026-07-03 15:17:17 +08:00
- 结束时间：2026-07-03 16:29:27 +08:00
- 开发总结：新增方圆静态实例化纯数据模型，支持 runtime primitive set 和 layout/palette 转换为稳定排序的 instance batches；batch key 按 kind、material profile 和透明路径隔离，颜色与 emissive 作为 per-instance 数据进入 buffer hash；新增 cache key / dirty helper，用于后续实例化渲染阶段避免重复重建。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_static_instance -- --nocapture` 通过（6 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 通过，仅有 Git LF/CRLF 提示。

- [x] 新增 `FangyuanStaticInstance` 或等价类型，包含 position、scale、color、alpha、emissive、material_profile_id 和 source 定位。（验证：`project/src/framework/fangyuan/static_instance.rs:19` 定义 `FangyuanStaticInstance`，字段包含 position/scale/color/alpha/emissive/material_profile_id/source；`:30` 的 `from_primitive()` 从 runtime primitive 填充）
- [x] 新增 instance batch / buffer source 数据结构，记录 kind、profile、透明路径、bounds、instance count 和 hash。（验证：`project/src/framework/fangyuan/static_instance.rs:47` 定义 `FangyuanStaticInstanceBatch`；`:54` 定义 `FangyuanStaticInstanceBufferSource`，包含 kind/material_profile/transparent_path/bounds/instance_count/hash/source_refs；`:87` 定义 bounds）
- [x] 定义实例化 batch key，避免不同 kind、透明状态或 profile 混入同一不可渲染批次。（验证：`project/src/framework/fangyuan/static_instance.rs:65` 定义 `FangyuanStaticInstanceBatchKey`，按 primitive_kind/material_profile/transparent_path 分批；`:73` 注释明确 color/emissive 是 per-instance 数据，不进 batch key 但进入 buffer hash；`:641` 测试覆盖 kind、透明、profile 分组）
- [x] 从 runtime primitive set / layout compile output 转换出稳定排序的 instance arrays。（验证：`project/src/framework/fangyuan/static_instance.rs:261` 定义 primitive set 入口，`:281` 定义 layout + palette 入口，`:205` 对 batch instances 按 source 稳定排序；`:718` 和 `:739` 测试覆盖稳定排序和 layout source 定位）
- [x] 增加 hash 或 dirty 标记，layout / prefab 内容未变时避免无意义重建。（验证：`project/src/framework/fangyuan/static_instance.rs:152` 定义 `FangyuanStaticInstanceCacheKey`，`:168` 定义 `fangyuan_static_instance_cache_is_dirty()`，`:361` 起生成 report cache key，`:383` 起 batch hash 包含 key、instance 内容和 source；`:847` 测试同内容不重建、内容变化重建）
- [x] 为颜色、尺寸、透明、发光、material profile、source 定位、同内容不重建和内容变化重建补测试。（验证：`project/src/framework/fangyuan/static_instance.rs:562` 覆盖 runtime 字段、bounds 和 source_refs，`:641` 覆盖透明/profile/color/emissive per-instance，`:739` 覆盖 layout source，`:788` 覆盖静态筛选边界，`:847` 覆盖 cache/dirty）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_static_instance -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行全部通过；`cargo test fangyuan_static_instance -- --nocapture` 为 6 passed；`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 5：静态实例化渲染原型接入

- 开始时间：2026-07-03 16:32:10 +08:00
- 结束时间：2026-07-03 17:44:28 +08:00
- 开发总结：完成静态实例化渲染原型接入。本阶段采用保守 shared-mesh prototype：通过静态 instance render report 消费 instance arrays，家园 `StaticInstance` 模式复用 shared cube/sphere mesh handle 并生成 per-instance render entity，用于验证数据路径、mode、lifecycle、HUD stats 和 fallback；尚未实现正式 GPU instance buffer / custom render pipeline。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_static_instance -- --nocapture` 通过（10 passed）；`cargo test fangyuan_home -- --nocapture` 通过（48 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 通过，仅有 Git LF/CRLF 提示。

- [x] 建立 cube shared mesh 的静态实例化渲染原型，能消费 instance buffer 中的位置、尺寸和颜色。（验证：`project/src/framework/fangyuan/static_instance_render.rs:132` 生成 render report，`:256` 将 batch instances 转换为 render instances 并保留 position/scale/color；`project/src/game/scenes/fangyuan_home.rs:1547` 起按 batch 复用 shared mesh handle 并用 instance position/scale/color 生成可视实体；`:3853` 测试验证默认家园 StaticInstance 模式）
- [x] 建立 sphere shared mesh 的静态实例化渲染原型，明确低多边形基模和预算限制。（验证：`project/src/framework/fangyuan/render_assets.rs:9`-`:10` 导出 unit sphere 24 sectors / 12 stacks；`project/src/framework/fangyuan/static_instance_render.rs:26` 的 options 包含 `max_sphere_instances`，`:75` 的 stats 记录 sphere_sectors/sphere_stacks，`:376` 和 `:408` 测试覆盖 sphere base mesh 摘要和 sphere budget / unsupported kind）
- [x] 接入 Fangyuan 现有相机、场景根实体和资源生命周期，避免 render 资源泄漏。（验证：`project/src/game/scenes/fangyuan_home.rs:1511` 的 static instance content 接入家园 content/root，`:1547` 的 batch spawn 写入 `SceneOwned` 和 session metadata，`:2158` 的 exit 系统清理 render runtime；`:3967` 测试覆盖 reload/clear/exit/mode switch 清理 runtime state）
- [x] 增加实例化渲染模式开关，与 standard / CPU merge 模式互斥或可显式切换。（验证：`project/src/game/scenes/fangyuan_home.rs:313` 定义 `FangyuanHomeBlueprintRenderMode::{Standard,CpuMerge,StaticInstance}`，`:332` 默认仍为 `Standard`，`:1365` 起按 mode 分支互斥执行 standard / CPU merge / StaticInstance）
- [x] HUD 显示 instance mode、batch count、instance count、buffer bytes 和 fallback reason。（验证：`project/src/game/scenes/fangyuan_home.rs:422` 定义 `FangyuanHomeBlueprintRenderSummary`，`:519` 起 stats 保存 render_mode/static_instance_batch_count/static_instance_count/static_instance_buffer_bytes/static_instance_fallback_reason；`project/src/game/screens/gameplay/fangyuan_home.rs:244` HUD 增加 `render ... ib ... ii ... bytes ... fb ...` 短行，并在 `:476`、`:496`、`:526` 等测试覆盖）
- [x] 为场景接入、清理、重复进入、mode 切换、render 初始化失败和 unsupported kind 补测试。（验证：`project/src/game/scenes/fangyuan_home.rs:3853` 覆盖 StaticInstance 场景接入，`:3967` 覆盖 reload/clear/exit/mode switch，`:4060` 覆盖 buffer budget / 初始化失败 fallback，`:4118` 覆盖 unsupported sphere 且不留下半旧内容；`cargo test fangyuan_home -- --nocapture` 48 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_static_instance -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行全部通过；static instance 10 passed，fangyuan_home 48 passed，`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 6：Material Profile 数据模型

- 开始时间：2026-07-03 17:47:22 +08:00
- 结束时间：2026-07-03 18:23:53 +08:00
- 开发总结：新增方圆材质 profile 数据模型、默认 profile、registry/table、profile id 统一校验、fallback 和合成规则；合成规则保持 primitive 字段为 per-instance 输入，profile 提供 base params 与 alpha/emissive policy。本阶段未迁移正式渲染材质路径，只通过测试证明 blueprint/prefab/layout、CPU merge、static mesh 和 static instance 的材质字段传递未丢失。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_material_profile -- --nocapture` 通过（5 passed）；`cargo test fangyuan_material -- --nocapture` 通过（6 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 通过，仅有 Git LF/CRLF 提示。

- [x] 新增 `FangyuanMaterialProfile` 或等价类型，包含稳定 id、version、base params、emissive policy、alpha policy 和 debug label。（验证：`project/src/framework/fangyuan/material_profile.rs:21` 定义 `FangyuanMaterialProfile`，字段包含 stable_id/version/base/emissive_policy/alpha_policy/debug_label；`:119`、`:183`、`:224` 分别定义 base params、alpha policy、emissive policy）
- [x] 新增 profile table / registry，提供默认 profile、非法 profile fallback 和 profile 数量限制。（验证：`project/src/framework/fangyuan/material_profile.rs:282` 定义 `FangyuanMaterialProfileRegistry` 和 `FangyuanMaterialProfileTable`；`:292` 初始化默认 profile，`:319` 插入时校验 duplicate/limit，`:358` resolve 时处理缺省、非法和未知 profile fallback；`:572` 和 `:609` 测试覆盖 default/fallback/limit/validation）
- [x] 定义 profile 与 primitive instance 字段的合成规则，明确 color / alpha / emissive 的优先级。（验证：`project/src/framework/fangyuan/material_profile.rs:95` 的 `compose_primitive()` 明确 RGB 为 profile base * primitive RGB，alpha 使用 primitive alpha 并受 profile alpha policy 限制，emissive 使用 primitive boost 并受 emissive policy 限制；`:652`、`:689` 测试覆盖 additive/clamp、force opaque 和 disabled emissive）
- [x] 梳理 simple blueprint、prefab、layout compile output、CPU merge 和 static instance 中的材质字段传递路径。（验证：`project/src/framework/fangyuan/material_profile.rs:754` 的 `fangyuan_material_fields_flow_from_blueprint_prefab_layout_to_static_outputs` 覆盖 blueprint compile、prefab/layout compile output、static merge key、static mesh material/color attribute 和 static instance fields；`blueprint.rs:864`、`static_merge.rs:559` 复用统一 profile id 校验）
- [x] 对 legacy 资源缺省 profile 的兼容行为补测试。（验证：`project/src/framework/fangyuan/material_profile.rs:722` 的 `fangyuan_material_profile_legacy_missing_profile_uses_default_profile` 断言缺省 `material_profile_id=None` 使用 `FANGYUAN_MATERIAL_PROFILE_DEFAULT_ID`，并与 static merge 默认 profile 保持一致）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_material_profile -- --nocapture`、`cargo test fangyuan_material -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行全部通过；profile 测试 5 passed，material 测试 6 passed，`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 7：统一材质渲染、透明和发光预算

- 开始时间：2026-07-03 18:26:46 +08:00
- 结束时间：2026-07-04 17:34:43 +08:00
- 开发总结：完成统一材质渲染、透明和发光预算收口；Standard / StaticInstance 路径使用 profile 合成后的材质参数，CPU merge 继续用顶点色承载多颜色并复用少量材质，stats / audit / HUD / debug log 均输出 profile、opaque、transparent、emissive 和 material resource 摘要。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_material -- --nocapture` 通过（7 passed）；`cargo test fangyuan_audit -- --nocapture` 通过（13 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 将默认不透明材质接入 standard / CPU merge / static instancing 中至少一条主路径。（验证：`project/src/game/scenes/fangyuan_home.rs:2105` 的 standard visual 调用 `blueprint_assets.material(primitive, materials)`；`:1689` 的 StaticInstance 调用 `material_for_runtime_fields()`；`:1844` 的 CPU merge 调用 `material_for_static_merge_material()`）
- [x] 确保多颜色 primitive 不产生线性增长的 Material 资源数量。（验证：`project/src/framework/fangyuan/render_assets.rs:33` 定义颜色+emissive 量化的 `FangyuanRenderMaterialKey`；`project/src/game/scenes/fangyuan_home.rs:3837` 断言 CPU merge 由顶点色承载颜色且只保留 1 个材质 handle；`cargo test fangyuan_material -- --nocapture` 通过）
- [x] 透明 primitive 进入单独统计和可选单独渲染路径，不与默认不透明批次混淆。（验证：`project/src/framework/fangyuan/stats.rs:27`、`:29` 定义 transparent/opaque 统计，`:81` 起按 profile 合成 alpha 分类；`project/src/framework/fangyuan/audit.rs:410` 起独立 transparent count 预算 finding；`cargo test fangyuan_audit -- --nocapture` 通过）
- [x] 发光强度进入 profile 合成和 audit budget，保留热点降级入口。（验证：`project/src/framework/fangyuan/material_profile.rs:406` 的 `compose_runtime_fields()` 合成 emissive；`project/src/framework/fangyuan/audit.rs:425` 起继续生成 emissive count / intensity 预算 finding，并使用 `LowerEmissive` suggestion；`cargo test fangyuan_audit -- --nocapture` 通过）
- [x] 对 alpha、emissive、transparent count、profile limit 生成稳定 finding 和 suggestion。（验证：`project/src/framework/fangyuan/audit.rs:401`、`:413`、`:425`、`:446` 分别生成 alpha、transparent、emissive、material profile finding；`:1353`-`:1374` 和 `:1429`-`:1435` 测试断言 recommended / hard limit finding 与 suggestion）
- [x] HUD / debug stats 显示 material profile count、opaque count、transparent count、emissive total 和 unique material resource count。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:257`-`:261` HUD 输出 matprof/opaque/trans/emi/uniq；`project/src/game/scenes/fangyuan_home.rs:2361` 起日志输出 material_profiles/opaque/transparent/emissive_total/material_resources；`project/src/framework/fangyuan/audit.rs:633` debug summary 输出对应字段）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_material -- --nocapture`、`cargo test fangyuan_audit -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行全部通过；material 7 passed，audit 13 passed，`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 8：规模、Fallback 和压力验证

- 开始时间：2026-07-04 17:36:58 +08:00
- 结束时间：2026-07-04 18:38:48 +08:00
- 开发总结：完成阶段 8 规模、Fallback 和压力验证收口；新增万级静态 primitive 生成器、统一渲染规模 / pressure 摘要、large static merge / static instance 压力测试、material fallback / 透明预算测试和默认家园统计一致性测试。规模趋势使用稳定计数和 pressure units 表达，不使用不稳定 wall-clock 帧率断言。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_static_merge -- --nocapture` 通过（10 passed）；`cargo test fangyuan_static_instance -- --nocapture` 通过（11 passed）；`cargo test fangyuan_material -- --nocapture` 通过（8 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 准备万级静态 primitive 的测试数据或生成器，避免把大 RON 手写进主资源。（验证：`project/src/framework/fangyuan/stats.rs:409` 定义 `generate_fangyuan_large_static_primitive_set(count)` 测试生成器，`project/src/framework/fangyuan/static_merge.rs:875` 和 `project/src/framework/fangyuan/static_instance.rs:890` 均用 `LARGE_STATIC_PRIMITIVES = 10_000` 生成测试数据，未新增大型 RON 资源）
- [x] 对比 standard、CPU merge 和 static instancing 的生成数量、batch 数和帧率趋势。（验证：`project/src/framework/fangyuan/stats.rs:48` 的 `FangyuanRenderScaleReport` 汇总 standard / cpu_merge / static_instance 口径，`:84` 的 `format_summary()` 输出 primitives/entities/meshes/batches/materials/profiles/buffer/pressure；`static_merge.rs:900` 和 `static_instance.rs:913` 的测试输出 10000 primitive scale summary，并在 `static_merge.rs:932`、`static_instance.rs:956` 断言 pressure reduction 大于 1）
- [x] 验证 audit warning / failed、透明超预算、unsupported profile、mesh builder 错误和 render 初始化失败的 fallback 行为。（验证：`project/src/framework/fangyuan/material_profile.rs:877` 测试 unknown profile fallback、warning 和 failed budget，`:927`、`:951`、`:953` 断言透明 / alpha 超预算 finding；`project/src/framework/fangyuan/static_mesh_builder.rs:697` 覆盖 mesh builder budget error；`project/src/framework/fangyuan/static_instance_render.rs:412` 覆盖 unsupported kind / budget error，`:458` 覆盖 render initialization options；`project/src/game/scenes/fangyuan_home.rs:4187` 覆盖 static instance budget / initialization failure fallback 到 standard）
- [x] 日志输出性能摘要和关键限制，便于后续决定是否扩大实例化范围。（验证：`project/src/framework/fangyuan/stats.rs:84` 的 `format_summary()` 输出 stable scale / pressure summary 和 limiting path；`project/src/framework/fangyuan/static_merge.rs:900`、`project/src/framework/fangyuan/static_instance.rs:913`、`project/src/game/scenes/fangyuan_home.rs:4315` 在测试中打印对应 scale summary；`project/src/game/scenes/fangyuan_home.rs:2361` 的家园 stats 日志补充 render_mode、static_instance_batches/count/buffer_bytes/fallback）
- [x] 验证默认家园布局下 profile 统计、合并统计和实例化统计一致或差异可解释。（验证：`project/src/game/scenes/fangyuan_home.rs:4293` 的 `default_home_layout_reports_explainable_standard_merge_and_instance_stats` 编译默认家园 layout，`:4320` 校验 primitive 总数，`:4338` 校验 merge / instance material profile count 一致，`:4354` 校验 static instance buffer bytes，`:4384` 校验 pressure trend 和 buffer KiB）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_static_merge -- --nocapture`、`cargo test fangyuan_static_instance -- --nocapture`、`cargo test fangyuan_material -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行全部通过；static merge 10 passed，static instance 11 passed，material 8 passed，`cargo check` 仅有既有 `selection.rs:32` warning）

## 阶段 9：回归测试和手动验收

- 开始时间：2026-07-04 18:42:56 +08:00
- 结束时间：2026-07-04 20:12:29 +08:00
- 开发总结：完成阶段 9 回归测试和手动验收收口；新增 `MYBEVY_FANGYUAN_HOME_RENDER_MODE` 开发期渲染模式环境变量，便于在 phone-small 窗口分别验收 standard、CPU merge 和 static instance；补充阶段 9 生命周期验收测试，覆盖 Reload、Clear、Lobby 返回、模式切换和重复进入不叠加、不残留。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（233 passed）；`cargo test fangyuan_home -- --nocapture` 通过（51 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；默认 Vulkan 运行出现 swapchain timeout，已按 checklist 使用 DX12 复验三种渲染模式并产出 `project/target/stage9-manual/` 下截图和报告。

- [x] 运行 `cargo fmt --check`。（验证：主 agent 在 `project/` 下执行 `cargo fmt --check` 通过）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：主 agent 在 `project/` 下执行通过，233 passed，0 failed）
- [x] 运行 `cargo test fangyuan_home -- --nocapture`。（验证：主 agent 在 `project/` 下执行通过，51 passed，0 failed，包含 `stage9_reload_clear_lobby_return_mode_switch_and_reenter_do_not_leave_residual_content`）
- [x] 运行 `cargo check`。（验证：主 agent 在 `project/` 下执行通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning）
- [x] 手动运行 `cargo run -- --window-profile phone-small --window-scale 50%`，确认 standard、CPU merge、instancing 模式内容可见且 HUD 不重叠。（验证：worker 使用 `MYBEVY_FANGYUAN_HOME_RENDER_MODE=standard|cpu_merge|static_instance` 和 DX12 在 phone-small 窗口复验；`project/target/stage9-manual/stage9-*-dx12/report.md` 均 passed，CPU merge 截图内容和 HUD 可见，standard / static instance 日志分别记录 `render_mode=standard` 和 `render_mode=static_instance` 且 generated=138）
- [x] 如当前桌面 Vulkan 截图为空白，使用 `WGPU_BACKEND=dx12` 复验可视结果。（验证：默认 Vulkan 运行出现 `swap chain texture` timeout；DX12 复验报告 `stage9-standard-dx12`、`stage9-cpu-merge-dx12`、`stage9-static-instance-dx12` 均 passed）
- [x] 手动验收 Reload、Clear、Lobby 返回、模式切换和重复进入，确认不重复叠加、不残留。（验证：`project/src/game/scenes/fangyuan_home.rs:4219` 新增阶段 9 生命周期测试覆盖 Reload、Clear、SceneCommand::Exit、StaticInstance -> CPU merge 模式切换、退出后重复进入和重复 Enter 不叠加；`cargo test stage9_reload_clear_lobby_return_mode_switch_and_reenter_do_not_leave_residual_content -- --nocapture` 与 `cargo test hud_buttons_write_reload_clear_and_lobby_exit_route -- --nocapture` 均通过）

## 阶段 10：文档同步和归档准备

- 开始时间：2026-07-04 20:13:00 +08:00
- 结束时间：2026-07-04 20:26:58 +08:00
- 开发总结：完成阶段 10 文档同步，技术路线补充第六至第八阶段 CPU merge、static instance shared-mesh prototype、统一材质 profile、fallback、压力和非目标边界；世界观蓝图规则补充 `material_profile_id` 合法性、默认 profile、透明/发光预算和审核规则；新成员文档补充家园渲染模式环境变量与 DX12 复验方式。
- 验证记录：`git diff --check` 通过，仅有 Git LF/CRLF 提示；`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（234 passed）；`cargo check` 通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 更新 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md`，记录 CPU merge、static instancing、FangyuanMaterial、fallback 和风险。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1657`、`:1695`、`:1732`、`:1768` 分别记录阶段 6/7/8 和规模 fallback 验收，包含 CPU merge、static instance prototype、`FangyuanMaterialProfile`、fallback、风险和压力摘要）
- [x] 更新世界观蓝图规则或资源文档，说明 material_profile_id 的合法值、默认值、透明/发光预算和审核规则。（验证：`docs/世界观/方圆灵构蓝图规则.md:469` 新增 `material_profile_id` 规则，`:626` 起列出非法原因，`:715` 和 `:727` 记录默认 `material:default`、受控 ASCII id、透明/发光/material profile 预算检查）
- [x] 如新增运行参数、调试开关或开发验收方式，更新 `docs/bevy-getting-started.md` 或相关方圆文档。（验证：`docs/bevy-getting-started.md:502`、`:512`、`:517` 记录 `MYBEVY_FANGYUAN_HOME_RENDER_MODE=standard|cpu_merge|static_instance`、别名、HUD/日志字段和 `WGPU_BACKEND=dx12` 复验方式）
- [x] 确认文档仍明确动态 VFX、技能规则层、Chunk、LOD、AOI、Bake、缓存继承和任意 shader 不是本 checklist 能力。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1778`、`docs/世界观/方圆灵构蓝图规则.md:18`、`:574`、`:592`、`docs/bevy-getting-started.md:520` 均明确非目标边界）
- [x] checklist 全部完成后，按仓库约定从 `summary/` 归档到 `docs/fangyuan/checklists/`。（验证：最终完成定义全部通过后，主 agent 已将本 checklist 归档到 `docs/fangyuan/checklists/方圆渲染规模化第六至第八阶段_checklist.md`）
- [x] 验证命令：`git diff --check`、`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在仓库根和 `project/` 下执行全部通过；`cargo test fangyuan -- --nocapture` 为 234 passed，`cargo check` 仅有既有 `selection.rs:32` warning）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-04 20:27:00 +08:00
- 结束时间：2026-07-04 20:30:58 +08:00
- 验收总结：方圆渲染规模化第六至第八阶段已完成静态 CPU merge、static instance shared-mesh prototype、统一 `FangyuanMaterialProfile`、透明/发光预算统计、fallback、HUD / 日志摘要、压力验证、回归验收和文档同步；正式 GPU instance buffer / custom render pipeline、动态 VFX、技能规则层、正式 Chunk / LOD / AOI、发布期 Bake、缓存继承、联网同步和任意 shader 仍作为后续非目标。

- [x] 静态方圆内容可以通过 CPU merge 生成少量合并 mesh，并能在默认家园或测试场景中显示。（验证：`project/src/game/scenes/fangyuan_home.rs:4567` 的默认家园统计测试输出 CPU merge meshes=15 / pressure_units=15；DX12 `stage9-cpu-merge-dx12` UI audit report passed 且截图可见内容和 HUD）
- [x] 大量静态 cube / sphere 可以通过 shared mesh + instance data 渲染，并保持 source 定位。（验证：`project/src/framework/fangyuan/static_instance.rs:19` 定义 `FangyuanStaticInstance` 携带 source；`:913` 万级静态实例压力测试输出 10000 instances / 22 batches；`cargo test fangyuan -- --nocapture` 234 passed）
- [x] standard、CPU merge 和 static instancing fallback 可用，失败时不留下误导性的成功状态。（验证：`project/src/game/scenes/fangyuan_home.rs:4483`、`:4558` 测试覆盖 static instance fallback / failure 状态；`:4219` 阶段 9 测试覆盖模式切换和退出清理；`cargo test fangyuan_home -- --nocapture` 51 passed）
- [x] 存在统一 FangyuanMaterialProfile / profile table，并能被 primitive、merge 和 instance 路径消费。（验证：`project/src/framework/fangyuan/material_profile.rs:21` 定义 profile，`:754` 测试覆盖 blueprint / prefab / layout / static merge / static mesh / static instance 材质字段传递）
- [x] 多颜色、透明和发光差异不会导致 Material 资源数量随 primitive 数线性爆炸。（验证：`project/src/framework/fangyuan/render_assets.rs:33` 使用量化 material key；`project/src/game/scenes/fangyuan_home.rs:4567` 默认家园 scale summary 显示 138 primitives / 15 materials，万级测试显示 10000 primitives / 32 materials）
- [x] HUD / 日志能显示 group、mesh、batch、instance、buffer、profile、alpha、emissive 和 fallback 摘要。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:257` 起 HUD 输出 material / render / instance 摘要；`project/src/game/scenes/fangyuan_home.rs:2388` 日志输出 material_profiles、opaque、transparent、emissive_total、render_mode、static_instance_batches/count/buffer_bytes/fallback）
- [x] Reload、Clear、场景退出和重复进入不泄漏 mesh、instance buffer 或材质资源。（验证：`project/src/game/scenes/fangyuan_home.rs:4219` 阶段 9 测试覆盖 Reload、Clear、SceneCommand::Exit、模式切换、退出后重复进入和重复 Enter 不叠加；`cargo test fangyuan_home -- --nocapture` 51 passed）
- [x] `cargo fmt --check` 通过。（验证：主 agent 在 `project/` 下执行 `cargo fmt --check` 通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：主 agent 在 `project/` 下执行通过，234 passed，0 failed）
- [x] `cargo test fangyuan_home -- --nocapture` 通过。（验证：主 agent 在 `project/` 下最终复跑通过，51 passed，0 failed）
- [x] `cargo check` 通过。（验证：主 agent 在 `project/` 下执行通过，仅保留既有 `project/src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning）
