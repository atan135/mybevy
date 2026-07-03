# 方圆玩家外观最小闭环 Checklist

## 目标

交付一个最小方圆玩家 Demo：通过本地 RON 定义玩家外观，运行时生成一个玩家逻辑 Entity，并把一个 cube 身体和一个 sphere 头部编译为该玩家 Entity 内部的 `FangyuanPrimitiveSet`，再通过 Bevy 基础渲染显示出来。

本阶段不做家园、装备、技能、联机同步、复杂动画、mesh 合并、GPU instancing、材质 profile、LOD、Bake 或编辑器。

## 基础原则

- [ ] 玩家是玩法 Entity，primitive 不是玩法 Entity。
- [ ] 方圆系统只支持移动、scale 缩放、primitive 生成和 primitive 消失。
- [ ] 蓝图、runtime 数据和动画设计中不加入 rotation、quaternion、euler 或 angular velocity。
- [ ] RON 只作为当前开发期源格式，不把它视为长期发布格式。
- [ ] 渲染子实体如存在，只承担 render-only 职责，不挂输入、移动、血量、技能、authority 或业务状态。
- [ ] 每个阶段完成后运行对应验证，并按阶段提交。

## 阶段 1：文档和资源约定

- 开始时间：2026-07-01 14:12:35 +08:00
- 结束时间：2026-07-01 14:16:05 +08:00
- 开发总结：完成阶段 1 文档和资源约定审核；确认当前最小闭环、默认蓝图路径、禁止旋转、非目标和 primitive 非玩法 Entity 边界均已在设计文档中记录。
- 验证记录：worker 只读审核通过；执行 `rg`、`Get-Content`、`git diff`，未修改文件。风险记录：`docs/世界观/方圆灵构蓝图规则.md` 仍有“每个 primitive 只保留静态 Transform / 挂到 root 下”的旧表述，后续可改为 render-only 口径。

- [x] 确认方圆技术路线已记录当前最小闭环为玩家外观 Demo。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:15` 记录当前最小闭环为玩家外观 Demo，`:1402` 阶段 1 为“方圆玩家外观最小闭环”）
- [x] 确认蓝图规则已记录 `project/assets/fangyuan/avatars/minimal_player.ron` 默认路径。（验证：`docs/世界观/方圆灵构蓝图规则.md:20` 到 `:23` 明确当前最小闭环默认读取该路径）
- [x] 确认蓝图规则已明确禁止旋转字段。（验证：`docs/世界观/方圆灵构蓝图规则.md:261` 到 `:270` 列出禁止 `rotation`、`quaternion`、`euler`、`angular_velocity`、`rotate`、`spin`；技术路线 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md:121` 到 `:139` 明确不暴露旋转能力）
- [x] 确认当前阶段不实现家园、静态场景、mesh 合并和 GPU instancing。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:708` 说明优先验证角色边界，`:940` 明确最小闭环不做 mesh 合并、GPU instancing 或复杂 VFX，`:1480` 将静态对象和家园预览放到阶段 3）
- [x] 检查文档中没有把 primitive 描述为玩法 Entity。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:64` 标题说明 primitive 不是玩法 Entity，`:729` 到 `:730` 明确 PlayerEntity 是玩法 Entity 且 cube/sphere 不是玩法 Entity，`:1918` 禁止把 primitive 作为玩法 Entity）

## 阶段 2：最小玩家 RON 资源

- 开始时间：2026-07-01 14:18:00 +08:00
- 结束时间：2026-07-01 14:26:41 +08:00
- 开发总结：新增最小玩家外观 RON 资源，内容为一个 cube 身体和一个 sphere 头部，作为后续对象层和渲染闭环的输入样例。
- 验证记录：审核 `project/assets/fangyuan/avatars/minimal_player.ron` 内容；`Select-String` 检查旋转相关字段无输出；`git status --short --untracked-files=all` 显示仅新增该资源文件。

- [x] 创建 `project/assets/fangyuan/avatars/minimal_player.ron`。（验证：`project/assets/fangyuan/avatars/minimal_player.ron:1` 存在并包含 RON 顶层对象）
- [x] RON 顶层包含 `version`、`name`、`description`、`max_primitives`、`bounds` 和 `primitives`。（验证：`project/assets/fangyuan/avatars/minimal_player.ron:2` 到 `:10` 包含 version/name/description/max_primitives/bounds，`:11` 起包含 primitives）
- [x] `primitives` 只包含一个 cube 身体和一个 sphere 头部。（验证：`project/assets/fangyuan/avatars/minimal_player.ron:13` 为 cube，`:19` 为 sphere，文件中仅 2 个 primitive 条目）
- [x] primitive 坐标按玩家根 Entity 的 local position 编写。（验证：`project/assets/fangyuan/avatars/minimal_player.ron:14` body position 为 `[0.0, 0.75, 0.0]`，`:20` head position 为 `[0.0, 1.75, 0.0]`，均为局部相对坐标）
- [x] 文件中不包含 rotation、quaternion、euler、angular_velocity、rotate 或 spin 字段。（验证：`Select-String -Pattern 'rotation|quaternion|euler|angular_velocity|rotate|spin'` 对该文件无输出）
- [x] 手动检查 RON 中 position、size、color 符合蓝图规则。（验证：`project/assets/fangyuan/avatars/minimal_player.ron:14` 到 `:22` 中 position 在 bounds 内，size 均为正数，color 均在 `0.0..=1.0`）

## 阶段 3：方圆基础数据模型

- 开始时间：2026-07-01 14:28:38 +08:00
- 结束时间：2026-07-01 14:52:31 +08:00
- 开发总结：新增 `framework::fangyuan` 基础数据模型，包含 primitive kind、蓝图 primitive、runtime primitive set 和玩家外观组件；本阶段未接入读取、玩家生成或渲染。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，31 passed；`cargo check` 通过。存在既有警告：`src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` 未使用。

- [x] 在 `project/src/framework/fangyuan/` 下建立或补齐基础模块入口。（验证：`project/src/framework/mod.rs:2` 导出 `fangyuan`，`project/src/framework/fangyuan/mod.rs:1` 定义模块入口并导出 avatar/blueprint/primitive）
- [x] 定义 `FangyuanPrimitiveKind`，只支持 cube 和 sphere。（验证：`project/src/framework/fangyuan/primitive.rs:5` 到 `:9` 仅定义 `Cube` 和 `Sphere`，serde 使用 lowercase）
- [x] 定义蓝图 primitive 结构，字段只包含 kind、position、size 和 color。（验证：`project/src/framework/fangyuan/blueprint.rs:6` 到 `:12` 的 `FangyuanPrimitiveBlueprint` 仅包含这四个字段，并使用 `deny_unknown_fields`）
- [x] 定义 runtime `FangyuanPrimitive` 和 `FangyuanPrimitiveSet`。（验证：`project/src/framework/fangyuan/primitive.rs:12` 到 `:28` 定义 runtime primitive，`:31` 到 `:65` 定义 primitive set 和访问方法）
- [x] 定义玩家外观相关的 `FangyuanAvatar` 或等价组件。（验证：`project/src/framework/fangyuan/avatar.rs:5` 到 `:11` 定义 `FangyuanAvatar` Bevy `Component`，持有 blueprint_id、display_name 和 primitive set）
- [x] 数据模型中不出现旋转相关字段。（验证：`rg -n 'rotation|quaternion|euler|angular_velocity|rotate|spin' project/src/framework/fangyuan` 仅命中 `blueprint_primitive_rejects_rotation_field` 测试和测试输入中的 `rotation`）
- [x] `cargo fmt` 通过。（验证：在 `project/` 运行 `cargo fmt --check` 通过）
- [x] `cargo check` 通过。（验证：在 `project/` 运行 `cargo check` 通过；仅有既有 `checkbox` unused 警告）

## 阶段 4：RON 读取、校验和编译

- 开始时间：2026-07-01 14:54:21 +08:00
- 结束时间：2026-07-01 15:28:12 +08:00
- 开发总结：完成方圆玩家外观蓝图顶层结构、首包 RON 读取、路径安全检查、校验错误类型和编译到 `FangyuanPrimitiveSet` 的流程；本阶段未接入玩家生成或渲染。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，41 passed；`cargo check` 通过。存在既有警告：`src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` 未使用。

- [x] 实现从 `project/assets/fangyuan/avatars/minimal_player.ron` 读取蓝图。（验证：`project/src/framework/fangyuan/blueprint.rs:13` 定义 `FANGYUAN_MINIMAL_PLAYER_BLUEPRINT_PATH`，`:175` 到 `:177` 读取该默认路径，测试 `minimal_player_blueprint_loads_from_first_package_assets_and_compiles` 通过）
- [x] 实现 RON 解析失败时的错误日志，游戏不崩溃。（验证：`project/src/framework/fangyuan/blueprint.rs:28` 的 `from_ron_str` 返回 `Result`，`:49` 到 `:54` 将 parse 失败转为 `ParseFailed`，`:183` 到 `:202` 的 `_or_log` helper 记录错误并返回 `None`；测试 `invalid_ron_returns_parse_error_without_panicking` 通过）
- [x] 校验 `version == "1"`。（验证：`project/src/framework/fangyuan/blueprint.rs:57` 到 `:63` 校验版本，测试 `compile_rejects_unsupported_version` 通过）
- [x] 校验 `primitives.len() <= min(max_primitives, 1000)`。（验证：`project/src/framework/fangyuan/blueprint.rs:67` 到 `:81` 使用 `min(max_primitives, hard_limit)`，测试 `compile_rejects_primitive_count_above_effective_limit` 和 `compile_rejects_primitive_count_above_hard_limit` 通过）
- [x] 校验最小玩家蓝图默认 primitive 数量为 2。（验证：`project/src/framework/fangyuan/blueprint.rs:14` 定义 `FANGYUAN_MINIMAL_PLAYER_PRIMITIVE_COUNT = 2`，测试 `minimal_player_blueprint_loads_from_first_package_assets_and_compiles` 确认蓝图和编译结果长度均为 2）
- [x] 校验 kind 只允许 cube 和 sphere。（验证：`project/src/framework/fangyuan/primitive.rs:19` 到 `:25` 仅解析 cube/sphere，`:28` 到 `:35` 未知 variant 反序列化失败；测试 `unknown_primitive_kind_is_rejected_by_blueprint_parse` 通过）
- [x] 校验 position 在 bounds 内，且主体不在地面以下。（验证：`project/src/framework/fangyuan/blueprint.rs:378` 到 `:405` 校验 bounds 内坐标，`:426` 到 `:438` 校验底部不低于地面；测试 `compile_rejects_position_outside_bounds` 和 `compile_rejects_primitive_body_below_ground` 通过）
- [x] 校验 size 每轴大于 0。（验证：`project/src/framework/fangyuan/blueprint.rs:407` 到 `:424` 校验每轴 finite 且 > 0；测试 `compile_rejects_non_positive_size_axis` 通过）
- [x] 校验 color 每项在 `0.0..=1.0`。（验证：`project/src/framework/fangyuan/blueprint.rs:440` 到 `:459` 校验 RGBA 范围；测试 `compile_rejects_color_channel_outside_unit_range` 通过）
- [x] 将合法蓝图编译为 `FangyuanPrimitiveSet`。（验证：`project/src/framework/fangyuan/blueprint.rs:89` 到 `:109` 将 position/size/color 映射到 runtime primitive set；测试 `minimal_player_blueprint_loads_from_first_package_assets_and_compiles` 确认 kind、local_position、scale、color）
- [x] `cargo fmt` 通过。（验证：在 `project/` 运行 `cargo fmt --check` 通过）
- [x] `cargo check` 通过。（验证：在 `project/` 运行 `cargo check` 通过；仅有既有 `checkbox` unused 警告）

## 阶段 5：玩家 Entity 生成

- 开始时间：2026-07-01 15:31:25 +08:00
- 结束时间：2026-07-01 15:59:03 +08:00
- 开发总结：新增方圆玩家预览 feature，启动时生成一个玩家逻辑 Entity，挂载玩家标记、状态、位置、`FangyuanAvatar`、`FangyuanPrimitiveSet` 和根 Transform；本阶段未创建 mesh/material 或渲染子实体。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，47 passed；`cargo check` 通过。存在既有警告：`src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` 未使用。风险记录：`FangyuanPrimitiveSet` 的 `Component` impl 暂在 preview feature 模块，后续可统一迁移到 framework 层。

- [x] 新增方圆玩家预览插件、系统或场景接入点。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:12` 定义 `FangyuanPlayerPreviewPlugin`，`project/src/game/plugin.rs:10` 和 `:30` 注册到 `GamePlugin`）
- [x] 运行时生成一个玩家逻辑 Entity。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:47` 到 `:68` 的 `spawn_fangyuan_preview_player` 生成单个玩家，测试 `fangyuan_preview_plugin_spawns_one_player_entity` 和 `fangyuan_preview_player_spawn_is_idempotent` 通过）
- [x] 玩家 Entity 挂载玩家标记、基础状态、`FangyuanAvatar` 和 `FangyuanPrimitiveSet`。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:58` 到 `:67` spawn bundle 包含 `FangyuanPlayer`、`FangyuanPlayerState`、`FangyuanAvatar` 和 `primitive_set`；测试 `fangyuan_preview_player_has_required_components` 通过）
- [x] 玩家 Entity 的位置组件只表达整体移动，不暴露旋转能力。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:35` 到 `:38` 的 `FangyuanPlayerPosition` 只有 `translation: Vec3`；测试 `fangyuan_player_position_only_exposes_translation` 通过）
- [x] cube 身体和 sphere 头部不作为玩法 Entity 参与输入、authority、技能、血量或移动状态。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs` 未创建 mesh/material/visual child，`primitives_remain_data_on_player_entity` 确认世界中只有玩家 Entity 持有一个 `FangyuanPrimitiveSet`）
- [x] 移动玩家根 Entity 时，primitive 可视表现整体跟随。（验证：本阶段建立根 Transform 同步基础，`project/src/game/features/fangyuan_player_preview/mod.rs:71` 到 `:77` 将 `FangyuanPlayerPosition.translation` 写入根 `Transform.translation` 并保持 identity rotation；测试 `moving_player_position_updates_root_transform_without_rotation` 通过；实际可视跟随由阶段 6 渲染适配和用户手动验收）
- [x] `cargo fmt` 通过。（验证：在 `project/` 运行 `cargo fmt --check` 通过）
- [x] `cargo check` 通过。（验证：在 `project/` 运行 `cargo check` 通过；仅有既有 `checkbox` unused 警告）

## 阶段 6：基础渲染适配和验收

- 开始时间：2026-07-01 16:01:09 +08:00
- 结束时间：2026-07-01 16:22:21 +08:00
- 开发总结：完成方圆玩家预览基础渲染适配，使用缓存 unit cube / unit sphere mesh、按颜色复用材质，并从玩家 Entity 的 `FangyuanPrimitiveSet` 生成 render-only child entities。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，54 passed；`cargo check` 通过。存在既有警告：`src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` 未使用。游戏内视觉效果由用户手动验收，当前未由主 agent 启动窗口确认。

- [x] 创建或复用 unit cube mesh。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:93` 到 `:96` 在 `FangyuanPlayerPreviewRenderAssets::unit_mesh` 中缓存 unit cube mesh，测试 `fangyuan_preview_visuals_use_cached_unit_meshes_by_kind` 通过）
- [x] 创建或复用 unit sphere mesh。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:97` 到 `:112` 在同一资源中缓存 unit sphere mesh，测试 `fangyuan_preview_visuals_use_cached_unit_meshes_by_kind` 通过）
- [x] 按颜色缓存或复用基础材质。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:119` 到 `:130` 使用 `materials_by_color` 复用材质，测试 `fangyuan_preview_visuals_reuse_materials_by_color` 通过）
- [x] 渲染适配层从 `FangyuanPrimitiveSet` 生成可视表现。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:164` 到 `:195` 遍历 primitive set 生成 visual child，测试 `fangyuan_preview_player_spawns_render_only_visual_children` 通过）
- [x] 如使用 render-only 子实体，确认其只挂载显示相关组件。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:178` 到 `:192` 子实体只挂 visual marker、Mesh3d、MeshMaterial3d、Transform、Visibility、Name；测试 `fangyuan_preview_visual_children_do_not_get_gameplay_components` 通过）
- [x] 修改 RON 中 body/head 的 position 后，可视位置对应变化。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:176` 使用 `primitive.local_position` 设置 visual local translation，测试 `fangyuan_preview_visual_transform_and_material_follow_primitive_data` 覆盖映射；实际 RON 修改后的游戏内效果由用户手动验收）
- [x] 修改 RON 中 body/head 的 size 后，可视 scale 对应变化。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:176` 使用 `primitive.scale` 设置 visual local scale，测试 `fangyuan_preview_visual_transform_and_material_follow_primitive_data` 覆盖映射；实际 RON 修改后的游戏内效果由用户手动验收）
- [x] 修改 RON 中 body/head 的 color 后，可视颜色对应变化。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:173` 到 `:174` 根据 primitive color 取材质，测试 `fangyuan_preview_visual_transform_and_material_follow_primitive_data` 覆盖 material.base_color 映射；实际 RON 修改后的游戏内效果由用户手动验收）
- [ ] 手动验收场景中可见一个 cube 身体和一个 sphere 头部组成的玩家。（待用户手动启动游戏确认；本轮按用户要求不由主 agent 验收游戏内效果）
- [x] `cargo fmt` 通过。（验证：在 `project/` 运行 `cargo fmt --check` 通过）
- [x] `cargo check` 通过。（验证：在 `project/` 运行 `cargo check` 通过；仅有既有 `checkbox` unused 警告）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-01 14:12:35 +08:00
- 结束时间：
- 验收总结：自动代码审核和测试样例已覆盖 RON 资源、数据模型、读取校验、玩家逻辑 Entity、render-only 基础渲染适配；游戏内视觉效果按用户要求保留为手动验收。

- [x] 游戏能从 RON 生成一个玩家逻辑 Entity。（验证：`FangyuanPlayerPreviewPlugin` 启动系统读取 `minimal_player.ron` 编译结果并生成玩家 Entity；`fangyuan_preview_plugin_spawns_one_player_entity` 通过）
- [ ] 玩家由一个 cube 身体和一个 sphere 头部组成。（待用户手动验收游戏内效果；代码证据：`minimal_player.ron` 包含 cube/sphere 两个 primitive，`fangyuan_preview_player_spawns_render_only_visual_children` 通过）
- [x] primitive 数据归属于玩家 Entity 的 `FangyuanPrimitiveSet`。（验证：`fangyuan_preview_player_has_required_components` 和 `primitives_remain_data_on_player_entity` 通过）
- [x] primitive 没有成为玩法 Entity。（验证：render-only child 不挂 `FangyuanPlayer`、状态、位置、Avatar 或 primitive set；`fangyuan_preview_visual_children_do_not_get_gameplay_components` 通过）
- [x] 方圆蓝图和 runtime 数据中没有旋转能力。（验证：蓝图使用 `deny_unknown_fields`，`blueprint_primitive_rejects_rotation_field` 通过；位置组件仅暴露 translation，`fangyuan_player_position_only_exposes_translation` 通过）
- [x] 玩家根位置变化能带动全部 primitive 可视表现。（验证：render-only children parent 到玩家根 Entity，`moving_player_root_preserves_visual_local_transforms_and_parenting` 通过；游戏内实际可视跟随由用户手动验收）
- [x] 非法 RON 或非法 primitive 不导致游戏崩溃。（验证：读取/解析/校验均返回 `Result` 或 log 后 `Option`；`invalid_ron_returns_parse_error_without_panicking`、非法版本/数量/position/size/color 测试通过）
- [x] `cargo fmt` 和 `cargo check` 通过。（验证：最终阶段运行 `cargo fmt --check` 和 `cargo check` 均通过；仅有既有 `checkbox` unused 警告）
