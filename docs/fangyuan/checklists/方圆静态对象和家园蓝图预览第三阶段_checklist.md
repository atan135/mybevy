# 方圆静态对象和家园蓝图预览第三阶段 Checklist

## 目标

交付方圆系统第三阶段基础能力：在第一阶段家园原型和第二阶段统一方圆数据模型稳定后，把 `project/assets/fangyuan/home_preview.ron` 接入统一 runtime primitive 模型，让家园或静态对象蓝图编译为 `FangyuanPrimitiveSet`，由家园逻辑对象根 Entity 持有，再派生 render-only 子实体显示。

本阶段重点是家园蓝图和玩家蓝图的 primitive 字段、校验、编译和 render-only 边界收敛；保留现有大厅入口、家园场景、基础空间、HUD、重新加载、清空和返回大厅链路。

本阶段不实现 Prefab、Scene Layout 拆分、Chunk、Bake、静态 CPU mesh 合并、GPU Instancing、LOD、AOI、联网同步、正式家园编辑器、蓝图持久化、装备挂点正式接入或技能规则层。

## 完整功能地图对照

| 功能域 | 第三阶段处理方式 |
| --- | --- |
| 默认家园蓝图 | 继续读取 `project/assets/fangyuan/home_preview.ron` |
| 蓝图 primitive 字段 | 与第二阶段统一数据模型保持一致 |
| runtime primitive | 编译为 `FangyuanPrimitiveSet` |
| 逻辑对象 | 增加或收敛家园静态对象根 Entity |
| 对象根状态 | 使用 `FangyuanObjectState` 或等价统一根状态 |
| render-only 边界 | primitive 渲染子实体只承担显示职责 |
| mesh 复用 | 继续复用 unit cube 和 unit sphere |
| 材质复用 | 继续按基础颜色或既有 key 复用材质 |
| 统计 | 优先复用 `FangyuanPrimitiveSet::stats()` |
| HUD | 保留 primitive 数、跳过数、材质数和蓝图路径显示 |
| Reload/Clear | 基于统一模型重新加载或清空蓝图内容 |
| 预算 | 保留 1000 primitive 硬限制，不实现完整预算系统 |
| 错误处理 | 非法顶层蓝图不生成，非法 primitive 跳过或结构化记录 |
| 旋转能力 | 继续禁止 rotation、quaternion、euler、angular_velocity、rotate、spin |
| Prefab/Bake/Instancing | 后续阶段，不在本阶段实现 |

## 基础原则

- [x] 家园或静态对象才是逻辑 Entity，单个 primitive 仍是对象内部数据，不是玩法 Entity。（验证：阶段 4 在 `project/src/game/scenes/fangyuan_home.rs` 增加 `FangyuanHomeObject` 逻辑根并让 render-only primitive 不挂业务状态；阶段 4/5 组件边界测试通过）
- [x] `FangyuanPrimitiveSet` 是玩家、家园、装备、技能、NPC 和天道生成物共享的数据容器。（验证：阶段 2 新增通用 `FangyuanBlueprint` 入口，阶段 3 家园和玩家蓝图均编译到 `FangyuanPrimitiveSet`；阶段 9 文档同步记录共享边界）
- [x] 蓝图、runtime、渲染适配和测试中继续禁止 rotation、quaternion、euler、angular_velocity、rotate 或 spin。（验证：阶段 2/3/5 测试覆盖预留旋转字段拒绝和 Transform 默认 rotation；阶段 9 文档继续列出禁止字段）
- [x] 保留现有 `dev.fangyuan_home` 场景入口、基础空间、HUD、reload、clear 和返回大厅行为。（验证：阶段 6/7 测试覆盖 Reload/Clear/Exit/HUD，阶段 8 手机窗口手动链路覆盖大厅进入、清空、重新加载、返回大厅和重新进入）
- [x] 本阶段只收敛统一模型和静态预览，不提前实现 Prefab、Bake、mesh merge、GPU Instancing、LOD 或联网同步。（验证：阶段 5 仅复用 unit mesh/material cache；阶段 9 两份文档明确这些能力仍为后续阶段）
- [x] 每个阶段完成后运行对应验证，并按阶段提交。（验证：阶段 2-7 和阶段 9 均有对应 git commit；阶段 8 为回归和手动验收，无业务代码提交，验证记录已填写）

## 阶段 1：需求和边界复核

- 开始时间：2026-07-02 10:13:24 +08:00
- 结束时间：2026-07-02 10:21:44 +08:00
- 开发总结：完成第三阶段目标、家园蓝图规则、第一阶段家园原型能力、第二阶段统一模型能力和当前家园私有实现残留位置的只读复核；确认后续实现重点是把 `fangyuan_home` 私有蓝图、校验、渲染和 HUD 统计逐步接入 framework 层统一模型。
- 验证记录：worker 执行 `git status --short`、`git diff --stat`、多组 `rg -n` 和 `Get-Content` 切片读取；未修改文件，未运行 cargo，未提交 git；最终 `git status --short` 和 `git diff --stat` 均无输出。

- [x] 复核 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md` 中“阶段 3：静态对象和家园蓝图预览”的目标、技术做法、验收标准和风险。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1494` 明确读取家园蓝图、生成 HomeObject/FangyuanObject、编译到 `FangyuanPrimitiveSet`、复用 unit mesh/材质、1000 上限、非法 primitive 跳过和主要风险）
- [x] 复核 `docs/世界观/方圆灵构蓝图规则.md` 中静态对象或家园蓝图结构、RON v1 字段、数量上限、bounds 和禁止旋转规则。（验证：`docs/世界观/方圆灵构蓝图规则.md:164` 给出静态对象或家园 RON v1 结构，`:247` 规定数量不超过 `min(max_primitives, 1000)`，`:271` 禁止旋转字段，`:401` 说明 primitive 是 `FangyuanPrimitiveSet` 内 runtime 数据）
- [x] 复核 `docs/世界观/checklists/方圆灵构家园原型第一阶段_checklist.md`，确认第一阶段已完成的家园场景、HUD、reload、clear 和手动验收能力。（验证：`docs/世界观/checklists/方圆灵构家园原型第一阶段_checklist.md:42` 记录场景注册和基础空间完成，`:88` 记录 HUD、重新加载、清空和返回大厅完成，`:104` 记录 reload/clear/手动验收通过）
- [x] 复核 `summary/方圆统一数据模型第二阶段_checklist.md`，确认第二阶段已完成的 `FangyuanPrimitiveSet`、role、alpha、emissive、material profile、lifecycle、`FangyuanObjectState` 和 stats 能力。（验证：`summary/方圆统一数据模型第二阶段_checklist.md:94`、`:109`、`:125`、`:175`、`:222` 分别记录 runtime primitive、role、alpha/emissive/material profile/lifecycle、对象根状态和 stats 已完成）
- [x] 检查当前家园蓝图相关代码仍有多少独立的临时数据结构、校验逻辑和渲染适配逻辑。（验证：`project/src/game/scenes/fangyuan_home.rs:256` 起仍有 `FangyuanHomeBlueprint`、`FangyuanHomeBlueprintPrimitive`、`ValidatedFangyuanHomeBlueprintPrimitive` 和 validation 结构；`:291`、`:364` 包含私有校验逻辑；`:480`、`:1092` 包含私有渲染资产缓存和实体生成；HUD 仍在 `project/src/game/screens/gameplay/fangyuan_home.rs:223` 读取私有 `FangyuanHomeBlueprintStats`）
- [x] 明确本阶段不处理 Prefab、Scene Layout 拆分、Bake、Instancing、LOD、AOI 和联网同步。（验证：`summary/方圆静态对象和家园蓝图预览第三阶段_checklist.md:9` 明确本阶段非目标；`docs/fangyuan/方圆对象资源构建与渲染技术路线.md` 将 Prefab/Scene Layout、LOD/AOI/Bake 放到后续阶段）
- [x] 验证命令：执行 `rg` / `Get-Content` / `git status --short` 等只读检查，确认阶段 1 不修改代码。（验证：worker 报告执行 `git status --short`、`git diff --stat`、多组 `rg -n` 和 `Get-Content`；未修改文件、未运行 cargo、未提交 git，最终状态无业务 diff）

## 阶段 2：统一蓝图结构入口设计

- 开始时间：2026-07-02 10:23:28 +08:00
- 结束时间：2026-07-02 10:48:40 +08:00
- 开发总结：在 framework 层新增通用 `FangyuanBlueprint`、`FangyuanBlueprintBounds`、通用加载/编译/路径校验和错误类型，并保留 `FangyuanAvatarBlueprint` 等旧名称作为兼容别名；玩家最小蓝图和家园默认蓝图可通过同一首包蓝图入口加载，玩家蓝图仍可编译为 `FangyuanPrimitiveSet`。
- 验证记录：主流程复跑 `cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过，98 passed；`cargo check` 通过。验证中仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 仅提示 CRLF 行尾转换，不存在空白错误。

- [x] 评估现有 `FangyuanAvatarBlueprint` 是否应重命名或抽出通用 `FangyuanBlueprint` / `FangyuanPrimitiveBlueprint`，避免家园和玩家各维护一套 primitive 字段。（验证：`project/src/framework/fangyuan/blueprint.rs:31` 新增通用 `FangyuanBlueprint`，`:46` 将 `FangyuanAvatarBlueprint` 保留为兼容别名，统一复用已有 `FangyuanPrimitiveBlueprint`）
- [x] 保留玩家最小蓝图和家园默认蓝图的 RON v1 兼容性，不要求资产破坏性改名或改字段。（验证：`project/src/framework/fangyuan/blueprint.rs:943` 的测试通过同一入口加载 `fangyuan/avatars/minimal_player.ron` 和 `fangyuan/home_preview.ron`，未修改资产文件）
- [x] 统一蓝图顶层字段的语义：`version`、`name`、`description`、`max_primitives`、`bounds`、`primitives`。（验证：`project/src/framework/fangyuan/blueprint.rs:31` 的 `FangyuanBlueprint` 定义和字段注释覆盖 6 个顶层字段，`:1055` 的测试构造并断言这些字段语义）
- [x] 统一 primitive 字段的语义：`kind`、`role`、`position`、`size`、`color`、`alpha`、`emissive`、`material_profile_id`、`lifecycle`。（验证：`project/src/framework/fangyuan/blueprint.rs:172` 的 `FangyuanPrimitiveBlueprint` 字段注释覆盖共享 primitive 字段，`:1082` 起测试断言编译后的 kind/role/local position/scale/color/alpha/emissive/profile/lifecycle）
- [x] 统一蓝图 asset path 校验策略，允许 `fangyuan/avatars/minimal_player.ron` 和 `fangyuan/home_preview.ron` 这类首包路径。（验证：`project/src/framework/fangyuan/blueprint.rs:711` 暴露 `validate_fangyuan_blueprint_asset_path` 并限制在 `assets/fangyuan` 下；`:1008` 测试允许两个合法路径并拒绝外部、父级、反斜杠、Windows drive 和绝对路径）
- [x] 明确玩家蓝图和家园蓝图的差异只存在于调用方语义、默认路径和逻辑对象组件，不分裂 primitive 数据模型。（验证：`project/src/framework/fangyuan/blueprint.rs:23` 注释声明 Player/home/static-object previews 仅按调用方语义、默认路径和逻辑根组件变化，并共享 `FangyuanBlueprint` 到 `FangyuanPrimitiveSet` 的数据形状）
- [x] 为统一蓝图入口补编译测试，确认玩家蓝图仍能加载并编译为 `FangyuanPrimitiveSet`。（验证：`project/src/framework/fangyuan/blueprint.rs:973` 的 `shared_blueprint_entry_compiles_minimal_player_to_primitive_set` 断言玩家蓝图通过通用入口编译为 2 个 primitive 的 set，`:995` 覆盖旧 Avatar API 兼容）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主流程复跑三条命令均通过；`cargo test fangyuan -- --nocapture` 为 98 passed，`cargo check` 仅有既有 `checkbox` dead_code warning）

## 阶段 3：家园蓝图编译为统一 PrimitiveSet

- 开始时间：2026-07-02 10:50:57 +08:00
- 结束时间：2026-07-02 11:42:43 +08:00
- 开发总结：新增 `FangyuanBlueprint::compile_skipping_invalid_primitives()` 和 `FangyuanBlueprintCompileReport`，顶层非法直接返回结构化错误，primitive 非法时跳过并记录 warning；家园场景改为加载通用 `FangyuanBlueprint`，将合法 primitive 编译进统一 `FangyuanPrimitiveSet`，再由家园 render-only 生成流程消费。默认 `home_preview.ron` 当前 505 条 primitive 中 493 条合法生成、12 条 below-ground 被跳过。
- 验证记录：主流程复跑 `cargo fmt --check` 通过；`cargo test fangyuan_home -- --nocapture` 通过，28 passed；`cargo test fangyuan -- --nocapture` 通过，103 passed；`cargo check` 通过。验证中仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 仅提示 CRLF 行尾转换，不存在空白错误。

- [x] 将 `home_preview.ron` 的加载结果编译为统一 `FangyuanPrimitiveSet`，不再只停留在家园场景私有 validated primitive 结构。（验证：`project/src/framework/fangyuan/blueprint.rs:127` 新增宽容编译报告并返回 `FangyuanPrimitiveSet`；`project/src/game/scenes/fangyuan_home.rs:564` 调用 `compile_skipping_invalid_primitives()`，`:607` 的生成函数接收 `&FangyuanPrimitiveSet`）
- [x] 保留 `version == "1"` 的校验。（验证：`project/src/framework/fangyuan/blueprint.rs:79` `validate_top_level()` 校验版本；`project/src/game/scenes/fangyuan_home.rs:1304` 测试非法版本返回 `unsupported_version`）
- [x] 保留 `primitives.len() <= min(max_primitives, 1000)` 的校验。（验证：`project/src/framework/fangyuan/blueprint.rs:91` 使用 `min(max_primitives, FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT)`；`project/src/game/scenes/fangyuan_home.rs:1323` 测试超出有效数量返回 `primitive_count_exceeded`）
- [x] 保留 `kind` 只允许 `cube` 和 `sphere` 的校验。（验证：`project/src/framework/fangyuan/primitive.rs:7` kind 枚举仅含 `Cube`/`Sphere`，`project/src/framework/fangyuan/blueprint.rs:978` 反序列化未知 kind 拒绝；家园生成在 `project/src/game/scenes/fangyuan_home.rs:623` 遍历统一 primitive set）
- [x] 保留 position、size、color、alpha、emissive、role、lifecycle 和 material profile 的合法性校验。（验证：`project/src/framework/fangyuan/blueprint.rs:646` 统一调用位置、尺寸、地面、颜色、alpha、emissive、role、material profile 和 lifecycle 校验；`:824`、`:869` 分别实现 profile 和 lifecycle 校验；`project/src/game/scenes/fangyuan_home.rs:1333`、`:1382` 测试覆盖基础字段和预留 metadata 错误跳过）
- [x] 保留不生成地面以下主体结构的校验或等价测试。（验证：`project/src/framework/fangyuan/blueprint.rs:764` 以 bottom_y 校验不低于 0；`:1202` 和 `project/src/game/scenes/fangyuan_home.rs:1258` 测试默认 home preview 编译后所有生成 primitive 都不在地面以下）
- [x] 非法顶层蓝图不得生成 primitive set；非法 primitive 应按既有策略跳过或返回结构化错误，并记录 skipped 数。（验证：`project/src/framework/fangyuan/blueprint.rs:127` 先校验顶层，再把非法 primitive 收集到 warnings；`project/src/game/scenes/fangyuan_home.rs:563` 顶层错误不生成内容，`:595` 记录 skipped 数；`:1333` 和 `framework/fangyuan/blueprint.rs:1526` 测试覆盖顶层失败和 primitive 跳过）
- [x] 为默认 `home_preview.ron` 补测试，确认可编译为统一 `FangyuanPrimitiveSet` 且数量不超过 1000。（验证：`project/src/framework/fangyuan/blueprint.rs:1202` 测试默认 home preview 编译为 493 个合法 primitive、12 个 skipped 且不超过 1000；`project/src/game/scenes/fangyuan_home.rs:1258` 测试家园默认蓝图加载编译结果）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_home -- --nocapture`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主流程复跑四条命令均通过；`fangyuan_home` 28 passed，`fangyuan` 103 passed，`cargo check` 仅有既有 `checkbox` dead_code warning）

## 阶段 4：家园逻辑对象根 Entity

- 开始时间：2026-07-02 11:45:11 +08:00
- 结束时间：2026-07-02 12:07:25 +08:00
- 开发总结：新增 `FangyuanHomeObject` 逻辑根 marker，并将现有蓝图内容 root 明确为家园对象根；该 Entity 同时持有 `FangyuanPrimitiveSet`、`FangyuanObjectState`、`SceneOwned` 和 session 信息，render-only primitive 作为其子实体生成。Clear/Reload 继续清理该逻辑根及其子实体，基础空间保留，Scene Exit 清理 session 下全部相关内容。
- 验证记录：主流程复跑 `cargo fmt --check` 通过；`cargo test fangyuan_home -- --nocapture` 通过，28 passed；`cargo check` 通过。验证中仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 仅提示 CRLF 行尾转换，不存在空白错误。

- [x] 为家园或静态对象生成一个明确的逻辑根 Entity，例如 `FangyuanHomeObject` 或通用 `FangyuanObject` 标记。（验证：`project/src/game/scenes/fangyuan_home.rs:365` 定义 `FangyuanHomeObject` marker，`:623` 在蓝图内容 root 上插入该 marker）
- [x] 逻辑根 Entity 持有 `FangyuanPrimitiveSet`。（验证：`project/src/game/scenes/fangyuan_home.rs:629` 在家园逻辑根 spawn 时插入 `primitive_set.clone()`；`:1493` 测试断言根上的 primitive set 等于默认编译结果）
- [x] 逻辑根 Entity 持有 `FangyuanObjectState` 或等价统一根状态。（验证：`project/src/game/scenes/fangyuan_home.rs:630` 插入 `FangyuanObjectState::default()`；`:1493` 和 `:2145` 测试断言根状态为默认值）
- [x] 逻辑根 Entity 挂到当前 `SceneRuntimeRoot` 或家园内容 root 下，并受 `SceneOwned` 清理。（验证：`project/src/game/scenes/fangyuan_home.rs:632` 将逻辑根加为家园内容 root 子实体，`:615` 同一 Entity 持有 `SceneOwned`；`:1493` 测试断言父子关系和 session ownership）
- [x] primitive 不作为独立玩法 Entity 生成，不挂输入、移动、血量、技能、authority 或业务状态。（验证：`project/src/game/scenes/fangyuan_home.rs:1841` 起测试断言 render-only primitive 不含 `FangyuanHomeContent`、`FangyuanHomeBlueprintContent`、`FangyuanHomeObject`、`FangyuanPrimitiveSet` 或 `FangyuanObjectState`）
- [x] 清空蓝图时移除家园逻辑对象及其 render-only 子实体，但不移除平面、网格、边界和灯光。（验证：`project/src/game/scenes/fangyuan_home.rs:2056` 起 `clear_blueprint_command_removes_only_blueprint_content` 断言 clear 后 `FangyuanHomeObject` 和 primitive 为 0，基础视觉数仍为 `EXPECTED_TOTAL_VISUALS`）
- [x] 退出场景时清理该 session 的家园逻辑对象、render-only 子实体和基础空间内容。（验证：`project/src/game/scenes/fangyuan_home.rs:2004` 起 `scene_lifecycle_exit_cleans_fangyuan_home_scene_owned_content` 断言 exit 后 content、visual、blueprint content、home object 和 primitive 计数均为 0）
- [x] 为逻辑根组件边界、父子关系、clear 和 exit 清理补测试。（验证：`project/src/game/scenes/fangyuan_home.rs:1493` 覆盖根组件和父子关系，`:1841` 覆盖 primitive 组件边界，`:2056` 覆盖 clear，`:2004` 覆盖 exit，`:2145` 覆盖 reload 替换逻辑根）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主流程复跑三条命令均通过；`fangyuan_home` 28 passed，`cargo check` 仅有既有 `checkbox` dead_code warning）

## 阶段 5：Render-only 渲染适配复用

- 开始时间：2026-07-02 12:09:30 +08:00
- 结束时间：2026-07-02 12:35:55 +08:00
- 开发总结：新增 framework 层 `FangyuanRenderAssetCache`、`FangyuanRenderColorKey`、runtime primitive 到 render Transform 的映射函数和基础 `StandardMaterial` helper；玩家预览和家园预览都改用共享 unit mesh、颜色材质和 Transform 映射。家园 render-only visual 继续从 `FangyuanPrimitiveSet` 派生，并补充 alpha、reserved metadata、组件边界和 mesh/material 复用测试。
- 验证记录：主流程复跑 `cargo fmt --check` 通过；`cargo test fangyuan_home -- --nocapture` 通过，32 passed；`cargo test fangyuan_player_preview -- --nocapture` 通过，27 passed；`cargo check` 通过。验证中仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 仅提示 CRLF 行尾转换，不存在空白错误。

- [x] 从 `FangyuanPrimitiveSet` 派生家园 render-only 子实体，而不是让渲染逻辑直接依赖家园私有 blueprint primitive。（验证：`project/src/game/scenes/fangyuan_home.rs:569` 遍历 `primitive_set.primitives()` 生成家园 visual，`project/src/game/scenes/fangyuan_home.rs:820` 从 runtime primitive 派生 Transform）
- [x] 复用或抽出玩家预览和家园预览共同需要的 unit cube / unit sphere mesh 缓存逻辑。（验证：`project/src/framework/fangyuan/render_assets.rs:34` 定义共享 `FangyuanRenderAssetCache::unit_mesh`；玩家预览在 `project/src/game/features/fangyuan_player_preview/mod.rs:63` 使用该 cache，家园预览在 `project/src/game/scenes/fangyuan_home.rs:250` 使用该 cache）
- [x] 复用或抽出玩家预览和家园预览共同需要的基础材质缓存逻辑。（验证：`project/src/framework/fangyuan/render_assets.rs:14` 定义共享 `FangyuanRenderColorKey`，`:69` 定义共享材质缓存，`:99` 定义 `fangyuan_standard_material_from_color`；两个预览模块均通过 `FangyuanRenderAssetCache::material` 取材质）
- [x] render-only 子实体只挂显示相关组件、`SceneOwned` 和视觉 marker。（验证：`project/src/game/scenes/fangyuan_home.rs:823` 的家园 primitive spawn 只插入 Mesh、Material、NoAutomaticBatching、Transform、`SceneOwned`、`FangyuanHomeBlueprintPrimitiveVisual` 和 Name；`project/src/game/features/fangyuan_player_preview/mod.rs:146` 的玩家 visual spawn 保持显示组件和 marker）
- [x] render-only 子实体不挂 `FangyuanPrimitiveSet`、`FangyuanObjectState`、玩家、家园、技能、authority、输入或其他业务状态。（验证：`project/src/game/scenes/fangyuan_home.rs:1747` 相关测试断言家园 visual 不含逻辑/root/业务组件；`project/src/game/features/fangyuan_player_preview/mod.rs:668` 既有测试继续断言玩家 visual 不含玩家/root/avatar/primitive set 等业务组件）
- [x] `local_position`、`scale`、`color` 和 `alpha` 的映射行为与第二阶段玩家预览保持一致。（验证：`project/src/framework/fangyuan/render_assets.rs:95` 统一从 runtime primitive 映射 translation/scale 且 rotation 为 identity；`project/src/game/scenes/fangyuan_home.rs:1530` 和 `:1869` 测试断言家园 visual 的 translation/scale/color alpha 映射；`project/src/game/features/fangyuan_player_preview/mod.rs:529` 既有玩家测试继续通过）
- [x] 当前阶段不消费 `emissive`、`material_profile_id` 和 `lifecycle` 的复杂表现时，应保留稳定默认行为并补测试。（验证：`project/src/framework/fangyuan/render_assets.rs:158` 测试 runtime primitive 的 profile/lifecycle 不影响基础 Transform helper；`project/src/game/scenes/fangyuan_home.rs:1869` 测试不同 alpha/emissive/profile/lifecycle 但同色的家园 primitive 共享同一基础材质）
- [x] 继续使用 unit mesh 和基础材质缓存，不引入 mesh merge、GPU Instancing 或完整材质 profile 系统。（验证：`project/src/framework/fangyuan/render_assets.rs:26` 注释明确 cache 只覆盖 unit primitive mesh 和 base color material，不编码 instancing、mesh merging、material profiles 或 lifecycle playback；代码搜索未新增 instancing/mesh merge 实现）
- [x] 为 mesh 复用、材质复用、字段映射和组件边界补测试。（验证：`project/src/framework/fangyuan/render_assets.rs:122`、`:141`、`:158` 覆盖共享 mesh、材质、Transform；`project/src/game/scenes/fangyuan_home.rs:1530`、`:1747`、`:1869` 覆盖家园 mesh/material、组件边界和字段映射；`cargo test fangyuan_player_preview -- --nocapture` 27 passed 覆盖玩家侧回归）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_home -- --nocapture`、`cargo test fangyuan_player_preview -- --nocapture`、`cargo check`。（验证：主流程复跑四条命令均通过；`fangyuan_home` 32 passed，`fangyuan_player_preview` 27 passed，`cargo check` 仅有既有 `checkbox` dead_code warning）

## 阶段 6：Reload、Clear 和场景生命周期收敛

- 开始时间：2026-07-02 12:38:28 +08:00
- 结束时间：2026-07-02 13:02:11 +08:00
- 开发总结：收敛家园蓝图 Reload/Clear/Exit 生命周期：Reload 保持先清旧逻辑根再读默认 layout/blueprint 并重新编译，Clear 只清理家园逻辑对象和 render-only 内容；新增场景退出后的蓝图统计 reset，避免 HUD/状态保留旧 session 成功数据。补充可注入蓝图加载 helper，用测试覆盖加载失败、RON 解析失败和顶层校验失败路径，不新增坏资产文件。
- 验证记录：主流程复跑 `cargo fmt --check` 通过；`cargo test fangyuan_home -- --nocapture` 通过，35 passed；`cargo check` 通过。验证中仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 仅提示 CRLF 行尾转换，不存在空白错误。

- [x] `FangyuanHomeBlueprintCommand::Reload` 重新读取默认蓝图、重新编译 `FangyuanPrimitiveSet` 并替换旧的逻辑对象和 render-only 内容。（验证：`project/src/game/scenes/fangyuan_home.rs:957` Reload 先清旧内容后加载默认 layout，`:543` 生产路径读取默认蓝图，`:578` 重新编译并生成；`:2289` 测试断言 reload 替换 home object）
- [x] Reload 不叠加旧 primitive，不重复生成基础空间。（验证：`project/src/game/scenes/fangyuan_home.rs:2289` 的 `reload_blueprint_command_replaces_content_without_duplicate_primitives` 断言 reload 前后基础 content/visual 数量稳定、blueprint content 和 primitive 数不叠加）
- [x] `FangyuanHomeBlueprintCommand::Clear` 只清空当前蓝图逻辑对象和 render-only 内容。（验证：`project/src/game/scenes/fangyuan_home.rs:894` 清理函数只处理 `FangyuanHomeBlueprintContent`，`:942` Clear 命令调用该清理；`:2354` 测试断言 clear 后基础空间保留、home object 和 primitive 为 0）
- [x] Clear 后再次 Reload 能恢复默认家园蓝图预览。（验证：`project/src/game/scenes/fangyuan_home.rs:2388` 的 `reload_blueprint_command_regenerates_preview_after_clear` 断言 clear 后 reload 恢复默认 generated primitive、home object 和统计状态）
- [x] 进入同一 session 的重复事件不重复生成家园对象或基础空间。（验证：`project/src/game/scenes/fangyuan_home.rs:1768` 的重复进入测试断言同一 session 不重复生成基础 content、home object 和 primitive）
- [x] 场景 Exit 后清理该 session 所有 `SceneOwned` 内容和统计状态。（验证：`project/src/game/scenes/fangyuan_home.rs:1015` 新增 `reset_fangyuan_home_blueprint_stats_on_exit`，`:2040` 测试 active session exit 后 stats reset，`:2144` 测试 scene exit 后 content、visual、home object、primitive 和 stats 均清理）
- [x] 加载失败、解析失败、校验失败时不崩溃，并能在 HUD 或日志中反映失败状态。（验证：`project/src/game/scenes/fangyuan_home.rs:564`、`:573`、`:582` 失败路径记录 `top_level_valid=false`；`:2454` 测试加载失败保留基础空间且统计失败；`:2487` 测试 RON 解析失败和顶层校验失败不生成内容并记录失败状态）
- [x] 为 reload、clear、clear 后 reload、重复进入、加载失败和退出清理补测试。（验证：`project/src/game/scenes/fangyuan_home.rs:2289` 覆盖 reload，`:2354` 覆盖 clear，`:2388` 覆盖 clear 后 reload，`:1768` 覆盖重复进入，`:2454` 和 `:2487` 覆盖失败路径，`:2040` 和 `:2144` 覆盖退出清理）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主流程复跑三条命令均通过；`fangyuan_home` 35 passed，`cargo check` 仅有既有 `checkbox` dead_code warning）

## 阶段 7：HUD 统计接入统一 Stats

- 开始时间：2026-07-02 13:04:30 +08:00
- 结束时间：2026-07-02 13:36:39 +08:00
- 开发总结：将 `FangyuanHomeBlueprintStats` 接入统一 `FangyuanPrimitiveSetStats`，loaded 状态由 `FangyuanPrimitiveSet::stats()` 写入，并保留 skipped、materials、blueprint path、top-level 状态和简短状态标签；HUD 改为显示 primitive、cube/sphere、skipped、material、alpha、glow、top 和 path，失败状态不复用旧成功 primitive 数据，长路径会截断，状态面板开启 clip。
- 验证记录：主流程复跑 `cargo fmt --check` 通过；`cargo test game::screens::gameplay::fangyuan_home -- --nocapture` 通过，7 passed；`cargo test fangyuan_home -- --nocapture` 通过，37 passed；`cargo check` 通过。验证中仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；`git diff --check` 仅提示 CRLF 行尾转换，不存在空白错误。

- [x] 家园 HUD 的 primitive 总数尽量来自 `FangyuanPrimitiveSet::stats()` 或统一统计结果。（验证：`project/src/game/scenes/fangyuan_home.rs:324` 的 `record_loaded()` 调用 `primitive_set.stats()` 写入 `FangyuanPrimitiveSetStats`，`project/src/game/screens/gameplay/fangyuan_home.rs:239` HUD 使用 `stats.primitive_total()`）
- [x] 家园 HUD 或日志能保留 skipped primitive 数、材质数量、蓝图路径和顶层校验状态。（验证：`project/src/game/scenes/fangyuan_home.rs:299` stats resource 保存 skipped/materials/blueprint_path/top_level_valid/state；`:1166` 日志输出这些字段；`project/src/game/screens/gameplay/fangyuan_home.rs:236` HUD 输出 skipped、mat、top 和 path）
- [x] 如显示 role、cube/sphere 或透明/发光统计，应从统一 stats 读取，不重复扫描 render-only 子实体。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:239` HUD 从 `stats.primitive_stats` 读取 cube/sphere/alpha/emissive 统计；实现未查询 render-only entity 来计算 HUD 统计）
- [x] Clear 后 HUD 显示 primitive 为 0，并保留合理的 skipped/material/path 状态。（验证：`project/src/game/scenes/fangyuan_home.rs:349` `record_cleared()` 清空 primitive stats 但保留 skipped/material/path/top；`project/src/game/screens/gameplay/fangyuan_home.rs:446` 测试 clear 后 HUD 为 `primitive 0/1000  cleared` 且保留 skipped/material/path）
- [x] Reload 后 HUD 恢复默认蓝图统计。（验证：`project/src/game/scenes/fangyuan_home.rs:2472` 测试 clear 后 reload 恢复 expected loaded stats；`project/src/game/screens/gameplay/fangyuan_home.rs:446` 测试 loaded -> cleared -> loaded 的 HUD 文本恢复）
- [x] 加载失败时 HUD 状态不显示误导性的成功数据。（验证：`project/src/game/scenes/fangyuan_home.rs:334` `record_failed()` 写入空 primitive stats 和 failed state；`project/src/game/screens/gameplay/fangyuan_home.rs:446` 测试失败 HUD 显示 `primitive 0/1000  failed`、`top invalid` 和截断路径）
- [x] 保持手机比例窗口下 HUD 文本不溢出、不遮挡关键按钮。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:186` 状态 panel 设置 `Overflow::clip()`，`:259` 将 blueprint path 截断到 32 字符以内，HUD 文本保持 4 行短格式）
- [x] 为 HUD 状态文本、clear/reload 后统计变化和失败状态补测试。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:423` 覆盖 HUD 读取 stats，`:446` 覆盖 loaded/cleared/reloaded/failed 文本，`:482` 覆盖默认 pending 失败态；`project/src/game/scenes/fangyuan_home.rs:2447`、`:2472`、`:2557` 覆盖 clear/reload/失败状态资源）
- [x] 验证命令：`cargo fmt --check`、`cargo test game::screens::gameplay::fangyuan_home -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主流程复跑四条命令均通过；HUD 聚焦测试 7 passed，`fangyuan_home` 37 passed，`cargo check` 仅有既有 `checkbox` dead_code warning）

## 阶段 8：回归测试和手动验收

- 开始时间：2026-07-02 13:38:56 +08:00
- 结束时间：2026-07-02 13:58:42 +08:00
- 开发总结：完成第三阶段回归验证和手机比例窗口手动链路验收；自动化测试覆盖统一方圆模型、家园场景和玩家预览，手机窗口从大厅进入家园后能显示默认家园蓝图，HUD 统计为 493/1000 loaded、skipped 12、mat 12，Clear/Reload/返回大厅/重新进入链路无回退。
- 验证记录：主流程复跑 `cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo test fangyuan_player_preview -- --nocapture`、`cargo check` 均通过；测试统计分别为 `fangyuan` 112 passed、`fangyuan_home` 37 passed、`fangyuan_player_preview` 27 passed，`cargo check` 仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning。另运行 `scripts/run-ui-audit.ps1 -Mode Local -Screens fangyuan_home -Devices phone-small -States initial -RunId fangyuan-home-stage3 -TimeoutSeconds 180 -AnalysisMode Off` 通过；随后真实运行 `cargo run -- --window-profile phone-small --window-scale 50%`，使用游戏内 F9 截图记录大厅、家园 loaded、clear、reload、返回大厅和重进家园状态，截图位于 `summary/ui-audit/fangyuan-home-stage3-manual/`。

- [x] 运行 `cargo fmt --check`。（验证：主流程在 `project/` 执行通过）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：主流程在 `project/` 执行通过，112 passed）
- [x] 运行 `cargo test fangyuan_home -- --nocapture`。（验证：主流程在 `project/` 执行通过，37 passed）
- [x] 运行 `cargo test fangyuan_player_preview -- --nocapture`，确认第二阶段玩家预览不回退。（验证：主流程在 `project/` 执行通过，27 passed）
- [x] 运行 `cargo check`。（验证：主流程在 `project/` 执行通过，仅保留既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning）
- [x] 首次运行如因 `cargo clean` 后重新编译耗时较长，应等待完整结果，不把编译时间长误判为失败。（验证：自动化测试和 `cargo run -- --window-profile phone-small --window-scale 50%` 均等待完整结果；后续运行已完成增量编译并进入窗口）
- [x] 手动运行 `cargo run -- --window-profile phone-small --window-scale 50%` 或等价手机比例窗口。（验证：主流程真实运行该命令，窗口日志显示 logical 360.0x800.0、physical window 360x800；游戏内截图 `summary/ui-audit/fangyuan-home-stage3-manual/1782971669_lobby_logical-360x800_physical-360x800.png` 生成）
- [x] 手动验收：从大厅进入方圆家园原型，能看到平面、网格、护栏、金黄色龙和灰白色云。（验证：点击大厅“方圆灵构家园原型”入口后截图 `summary/ui-audit/fangyuan-home-stage3-manual/1782971741_fangyuan_home_logical-360x800_physical-360x800.png` 显示网格平面、边界护栏、金黄色龙形 primitive 和灰白色云）
- [x] 手动验收：HUD primitive 数、skipped 数、材质数和蓝图路径显示合理。（验证：同一截图显示 `primitive 493/1000 loaded`、`cube 127 sphere 366 skipped 12`、`mat 12`、`path fangyuan/home_preview.ron`）
- [x] 手动验收：点击清空后蓝图内容消失，基础空间保留。（验证：点击“清空”后截图 `summary/ui-audit/fangyuan-home-stage3-manual/1782971811_fangyuan_home_logical-360x800_physical-360x800.png` 显示 `primitive 0/1000 cleared` 且基础网格平面仍保留）
- [x] 手动验收：点击重新加载后默认家园蓝图恢复。（验证：点击“重新加载”后截图 `summary/ui-audit/fangyuan-home-stage3-manual/1782971822_fangyuan_home_logical-360x800_physical-360x800.png` 恢复 `primitive 493/1000 loaded` 和默认家园蓝图视觉）
- [x] 手动验收：点击返回大厅后回到大厅，重新进入不会重复叠加内容。（验证：点击“大厅”后截图 `summary/ui-audit/fangyuan-home-stage3-manual/1782971831_lobby_logical-360x800_physical-360x800.png` 回到游戏列表；再次点击家园入口后 `summary/ui-audit/fangyuan-home-stage3-manual/1782971844_fangyuan_home_logical-360x800_physical-360x800.png` 仍为 `primitive 493/1000 loaded` 的单份默认家园画面）

## 阶段 9：文档同步和归档准备

- 开始时间：2026-07-02 14:00:31 +08:00
- 结束时间：
- 开发总结：
- 验证记录：worker 更新 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md` 和 `docs/世界观/方圆灵构蓝图规则.md`，主流程审核 diff 后打回一次错误路径并确认已修正为真实路径；主流程复跑 `cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check` 均通过，`fangyuan` 112 passed，`fangyuan_home` 37 passed，`cargo check` 仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning；路径检查确认 `docs/bevy-getting-started.md` 和 `CLAUDE.md` 存在且本阶段无需更新。

- [x] 更新 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md`，记录阶段 3 实际落地的统一模型接入边界。（验证：文档新增第三阶段已落地边界，记录 `FangyuanBlueprint`、`compile_skipping_invalid_primitives()`、`FangyuanBlueprintCompileReport`、`FangyuanHomeObject`、`FangyuanRenderAssetCache`、`FangyuanPrimitiveSetStats` 和默认家园 505/493/12 统计；主流程审核并修正错误路径为 `project/src/game/scenes/fangyuan_home.rs` 与 `project/src/framework/fangyuan/render_assets.rs`）
- [x] 更新 `docs/世界观/方圆灵构蓝图规则.md`，说明玩家、家园和静态对象共用 RON v1 primitive 字段与禁止旋转规则。（验证：文档说明玩家、家园和静态对象共用 RON v1 primitive 字段并统一编译到 `FangyuanPrimitiveSet`，继续列出禁止 `rotation`、`quaternion`、`euler`、`angular_velocity`、`rotate`、`spin`）
- [x] 如模块结构、启动方式或新成员理解路径变化，检查并同步 `docs/bevy-getting-started.md`。（验证：主流程和 worker 均确认本阶段未改变项目结构、启动方式、Bevy 版本或新成员上手路径；`docs/bevy-getting-started.md` 存在且无需修改）
- [x] 如仓库级说明需要更新，检查并同步 `CLAUDE.md`。（验证：主流程和 worker 均确认仓库级目录约定和开发流程未变化；`CLAUDE.md` 存在且无需修改）
- [x] 确认文档仍明确 Prefab、Bake、mesh merge、GPU Instancing、LOD、AOI 和联网同步不是本阶段能力。（验证：两份文档均明确本阶段不实现 Prefab、Scene Layout、Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI、联网同步等后续能力）
- [ ] checklist 全部完成后，按仓库约定将本文件从 `summary/` 归档到合适的 `docs/<领域>/checklists/` 目录。（待确认：`multi-agent-dev` 提交规则要求提交时排除 checklist 文件和 `summary/`，与仓库归档并提交 checklist 的约定存在冲突）
- [ ] 归档前确认 checklist 的阶段时间、开发总结和验证记录均来自真实执行结果。（待归档前最终复核）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`，以及必要的文档路径检查。（验证：主流程复跑四条命令均通过；`fangyuan` 112 passed，`fangyuan_home` 37 passed，`cargo check` 仅有既有 `checkbox` dead_code warning；路径检查确认新增引用路径存在）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-02 14:17:46 +08:00
- 结束时间：2026-07-02 14:17:46 +08:00
- 验收总结：第三阶段代码、文档、回归测试和手机窗口手动链路均已完成；默认家园蓝图通过统一蓝图入口编译为 `FangyuanPrimitiveSet`，家园逻辑根持有统一 primitive set 和对象状态，render-only 子实体不承担玩法状态，HUD 使用统一 stats 和 compile report。仍待单独决策的是 checklist 从 ignored `summary/` 归档到 `docs/<领域>/checklists/` 的处理方式，因为该动作与本次 `multi-agent-dev` 提交排除 checklist/summary 的规则存在冲突。

- [x] `home_preview.ron` 可以通过统一蓝图或统一 primitive 编译入口生成 `FangyuanPrimitiveSet`。（验证：`project/src/framework/fangyuan/blueprint.rs` 的默认家园蓝图测试编译为 493 个 generated primitive、12 个 skipped；`cargo test fangyuan -- --nocapture` 112 passed）
- [x] 家园或静态对象逻辑根 Entity 持有 `FangyuanPrimitiveSet` 和 `FangyuanObjectState` 或等价统一根状态。（验证：`project/src/game/scenes/fangyuan_home.rs` 生成 `FangyuanHomeObject` 逻辑根并插入 `FangyuanPrimitiveSet`、`FangyuanObjectState`；`cargo test fangyuan_home -- --nocapture` 37 passed）
- [x] 单个 primitive 不成为玩法 Entity，render-only 子实体不挂业务状态。（验证：阶段 4/5 组件边界测试断言 render-only primitive 不挂 `FangyuanPrimitiveSet`、`FangyuanObjectState`、输入、authority 或业务组件）
- [x] 默认家园蓝图仍能显示护栏、入口门、金黄色龙和灰白色云轮廓。（验证：手机窗口截图 `summary/ui-audit/fangyuan-home-stage3-manual/1782971741_fangyuan_home_logical-360x800_physical-360x800.png` 显示网格、边界护栏、金黄色龙形轮廓和灰白云）
- [x] primitive 数量不超过 1000，kind 只允许 `cube` 和 `sphere`。（验证：默认家园编译结果为 493/1000 generated；kind 由 `FangyuanPrimitiveKind` 限定 cube/sphere，相关测试通过）
- [x] 顶层非法蓝图不会生成内容，非法 primitive 能被跳过或结构化记录。（验证：`FangyuanBlueprint::compile_skipping_invalid_primitives()` 先校验顶层再记录 invalid primitive warnings；`invalid_or_malformed_blueprint_sources_do_not_spawn_preview_content`、`invalid_blueprint_primitives_are_skipped_and_valid_primitives_remain` 等测试通过）
- [x] Reload、Clear、Clear 后 Reload、返回大厅和重复进入行为均无回退。（验证：阶段 6 测试覆盖 reload/clear/clear 后 reload/重复进入/exit；阶段 8 手机窗口截图覆盖清空、重新加载、返回大厅和重新进入）
- [x] HUD 统计与统一 primitive set 或统一 stats 结果一致，不依赖 render-only 实体数量作为数据源。（验证：`FangyuanHomeBlueprintStats` 写入 `FangyuanPrimitiveSetStats`，HUD 文本显示 primitive/cube/sphere/skipped/material/path；阶段 7 HUD 测试通过）
- [x] 玩家预览入口和最小 cube/sphere 玩家外观不因本阶段改动回退。（验证：`cargo test fangyuan_player_preview -- --nocapture` 27 passed，`cargo test fangyuan -- --nocapture` 同时覆盖玩家预览回归）
- [x] 代码、测试和文档中不存在 rotation、quaternion、euler、angular_velocity、rotate 或 spin 能力。（验证：代码和文档仅保留禁止字段、拒绝字段或 Bevy Transform 默认 rotation 说明；未新增可配置旋转能力，相关拒绝测试通过）
- [x] 文档同步记录第三阶段实际落地边界和后续阶段延后事项。（验证：提交 `f05d306 docs(fangyuan): 同步静态对象预览第三阶段边界` 更新两份方圆文档，记录统一模型接入边界和 Prefab/Bake/Instancing/LOD/AOI/联网同步等延后事项）
- [x] `cargo fmt --check` 通过。（验证：阶段 9 主流程复跑通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：阶段 9 主流程复跑通过，112 passed）
- [x] `cargo test fangyuan_home -- --nocapture` 通过。（验证：阶段 9 主流程复跑通过，37 passed）
- [x] `cargo test fangyuan_player_preview -- --nocapture` 通过。（验证：阶段 8 主流程复跑通过，27 passed）
- [x] `cargo check` 通过。（验证：阶段 9 主流程复跑通过，仅有既有 `checkbox` dead_code warning）
- [x] 用户手动验收游戏内方圆家园预览效果无回退。（验证：阶段 8 真实运行 `cargo run -- --window-profile phone-small --window-scale 50%`，从大厅进入家园、清空、重新加载、返回大厅和重新进入均有游戏内 F9 截图记录）
