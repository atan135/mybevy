# 方圆统一数据模型第二阶段 Checklist

## 目标

交付方圆系统第二阶段基础能力：把第一阶段玩家外观最小闭环中已经跑通的蓝图读取、runtime primitive、玩家 Entity 持有 primitive set、render-only 基础渲染边界，收敛为后续玩家、家园、装备、技能、NPC 和天道自演都能复用的统一方圆数据模型。

本阶段重点是数据分层、字段边界、校验错误、role 语义、材质和生命周期预留、玩家预览迁移与测试覆盖。

本阶段不实现 Prefab、Scene Layout、Chunk、Bake、静态 CPU mesh 合并、GPU Instancing、动态 VFX 播放器、LOD、AOI、联网同步、装备挂点正式接入或技能规则层。

## 完整功能地图对照

| 功能域 | 方圆统一数据模型完整能力 | 第二阶段处理方式 |
| --- | --- | --- |
| 数据分层 | 区分蓝图 primitive、runtime primitive、render instance | 落地 |
| primitive 类型 | 只允许 `cube` 和 `sphere` | 落地 |
| 变换约束 | 只允许 local position、scale、生命周期；禁止 rotation | 落地 |
| 对象归属 | primitive 归属于玩家、家园物件、装备、技能、NPC、天道生成物等逻辑 Entity | 落地边界 |
| 统一集合 | `FangyuanPrimitiveSet` 成为通用数据容器 | 落地 |
| 蓝图编译 | RON v1 编译到统一 runtime primitive | 落地 |
| 当前 demo 兼容 | `minimal_player.ron` 不破坏，继续能加载渲染 | 落地 |
| 语义 role | `structure`、`core`、`boundary`、`warning`、`trail`、`impact`、`decoration`、`socket`、`archive` | 枚举落地，少量使用 |
| 材质参数 | `color`、`alpha`、`emissive`、`material_profile_id` | 预留字段，继续以 color 为主 |
| 生命周期 | primitive spawn、despawn、fade、lifetime 数据表达 | 预留字段或类型，不做播放器 |
| 对象根状态 | 逻辑对象整体移动、整体 scale、active/visible 状态 | 最小落地 |
| 预算字段 | primitive 数量、role 成本、透明预算、发光预算、bounds 成本 | 预留统计入口，完整预算后续 |
| 校验器 | kind、数量、bounds、size、color、alpha、emissive、role 合法性 | 基础落地 |
| 错误报告 | 结构化错误码、路径、primitive index、原因 | 落地 |
| 降级建议 | 超预算时建议删减 decoration、trail、impact 等 | 后续 |
| 材质 profile | 玉石、金属、灵木、石质、火核、水膜等 profile 表 | 后续 |
| Prefab | palette/prefab 定义可复用 primitive 组合 | 后续 |
| Scene Layout | layout 只记录 prefab instance，不重复 primitive | 后续 |
| Chunk | chunk 作为空间加载单元，不是体素网格 | 后续 |
| Bake | RON -> validator -> runtime/binary -> report | 后续 |
| hash/version | 蓝图、prefab、chunk 的 version、hash、cache key | 只保留设计位置 |
| 渲染适配 | runtime primitive -> render-only entity / mesh / instance data | 整理接口，继续用 render-only |
| 静态 CPU 合并 | 静态场景 primitive 合并 mesh | 后续 |
| GPU Instancing | cube/sphere 两路 instance buffer | 后续 |
| 动态 VFX | 技能 runtime instance 内部批量计算 primitive | 后续 |
| LOD | 按距离、role、屏幕占比、灵压降级 | 后续 |
| AOI/流式 | 按兴趣范围加载/卸载方圆对象 | 后续 |
| 网络同步 | 服务端只同步语义事件和 blueprint id，不同步 primitive 帧状态 | 后续 |
| 装备挂点 | `grip`、`tip`、`core`、`guard`、`aura` socket role | 只通过 role 预留 |
| 技能可读层 | `warning`、`boundary`、`core` 保留规则可读性 | 只通过 role 预留 |
| 天道自演 | 临时、固化、衰退、回收生命周期 | 后续 |
| 调试统计 | primitive 数、role 分布、材质数、预算、跳过原因 | 基础统计可落地 |
| 测试覆盖 | 无 rotation、RON 兼容、编译、校验、render-only 边界 | 落地 |

## 基础原则

- [x] primitive 不是玩法 Entity，玩家、家园物件、装备、技能、NPC 和天道生成物才是逻辑 Entity。（验证：`project/src/framework/fangyuan/primitive.rs:311` 声明 `FangyuanPrimitiveSet` 挂在逻辑对象根 Entity；`project/src/game/features/fangyuan_player_preview/mod.rs:412`、`:668` 测试确认 primitive 是玩家 Entity 内部数据，visual child 不挂业务组件）
- [x] 方圆模型只支持移动、scale 缩放、primitive 生成和 primitive 消失。（验证：`project/src/framework/fangyuan/object.rs:9` 根状态仅包含 active/visible/root_translation/root_scale；`project/src/framework/fangyuan/primitive.rs:138` 生命周期仅预留 lifetime/spawn_tick/despawn_tick 数据）
- [x] 蓝图、runtime、role、生命周期和渲染适配中不加入 rotation、quaternion、euler、angular_velocity、rotate 或 spin。（验证：`project/src/framework/fangyuan/blueprint.rs:819` 拒绝 6 个旋转字段；`project/src/framework/fangyuan/primitive.rs:178` runtime primitive 字段不含旋转；`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:425` 文档明确不含旋转字段）
- [x] 第二阶段只增强统一数据模型，不把 Prefab、Bake、Instancing、VFX、LOD、AOI 等后续路线提前混入实现。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:430` 和 `docs/世界观/方圆灵构蓝图规则.md:406` 明确这些能力未实现；代码提交仅涉及 fangyuan 数据模型、玩家预览测试和文档）
- [x] `minimal_player.ron` 保持兼容，玩家预览入口和最小渲染闭环不回退。（验证：`project/src/framework/fangyuan/blueprint.rs:850` 最小玩家资产加载编译测试通过；`cargo test fangyuan_player_preview -- --nocapture` 24 passed 覆盖入口、返回大厅、相机、灯光和 render-only 闭环）
- [x] 每个阶段完成后运行对应验证，并按阶段提交。（验证：阶段 2-12 已提交 `195ae80`、`e7a11c2`、`d24896a`、`e514962`、`c928e74`、`a56955a`、`2c60854`、`eaffaf7`、`4dc4504`、`a2be9d6`、`ad668b8`；各阶段验证记录已写入）

## 阶段 1：需求和边界复核

- 开始时间：2026-07-01 18:30:55 +08:00
- 结束时间：2026-07-01 18:38:05 +08:00
- 开发总结：完成第二阶段目标、非目标、RON v1 字段、第一阶段遗留风险和当前代码模块分布的只读复核；确认 `FangyuanPrimitiveSet` 的 Component 边界仍是后续迁移重点。
- 验证记录：worker 执行 `Get-Content`、`rg`、`rg --files`、`git status --short`、`git diff --stat`，未修改代码，未运行 cargo；`git status --short` 和 `git diff --stat` 在只读复核中无业务代码差异。

- [x] 复核 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md` 中“阶段 2：统一方圆数据模型”的目标和非目标。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1448`、`:1452`、`:1457` 记录阶段 2 统一 primitive 结构、blueprint/runtime 分层、role/material/生命周期预留和无 rotation 目标）
- [x] 复核 `docs/世界观/方圆灵构蓝图规则.md` 中当前 RON v1 字段、默认路径和禁止旋转规则。（验证：`docs/世界观/方圆灵构蓝图规则.md:18`、`:130`、`:194`、`:250`、`:261` 记录默认路径、v1 字段和禁止 `rotation/quaternion/euler/angular_velocity/rotate/spin`）
- [x] 复核 `summary/方圆玩家外观最小闭环_checklist.md`，确认第一阶段已完成的能力和遗留风险。（验证：`summary/方圆玩家外观最小闭环_checklist.md:45`、`:61`、`:81`、`:97` 已完成基础模型、RON 编译、玩家 Entity 和 render-only 适配；`:86`、`:112` 记录 Component 边界和手动验收遗留）
- [x] 明确第二阶段不处理家园恢复、Prefab、Chunk、Bake、mesh 合并、GPU Instancing、技能 VFX 和 LOD。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1478`、`:1511`、`:1661`、`:1742`、`:1775` 将这些能力放入后续阶段或长期路线）
- [x] 检查当前代码里 primitive 相关 Component、蓝图、runtime、渲染适配分别位于哪些模块。（验证：`project/src/framework/fangyuan/primitive.rs:6`、`:39`、`:63` 定义 kind/runtime/set；`blueprint.rs:13`、`:17`、`:89`、`:150` 定义蓝图、默认路径和编译；`avatar.rs:5` 定义玩家外观组件；`project/src/game/features/fangyuan_player_preview/mod.rs:60` 暂放 Component impl，`:90`、`:180`、`:183`、`:190` 处理 render-only 适配）
- [x] 验证命令：只读阶段至少执行 `rg` / `Get-Content` / `git diff --stat`，不修改代码。（验证：worker 报告执行 `Get-Content`、`rg`、`rg --files`、`git diff --stat`、`git status --short`；只读复核未产生业务代码 diff）

## 阶段 2：模块边界和命名收敛

- 开始时间：2026-07-01 18:39:49 +08:00
- 结束时间：2026-07-01 19:16:33 +08:00
- 开发总结：将 `FangyuanPrimitiveSet` 的 Bevy Component 边界迁回 framework 定义处，删除 preview feature 对 framework 类型的临时 Component impl；补充 framework 模块职责说明和组件 API 测试，玩家预览保留渲染视觉标记和预览特有逻辑。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，63 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 在 `framework/fangyuan/` 内明确 `blueprint`、`primitive`、`avatar` 或新增 runtime 模块的职责边界。（验证：`project/src/framework/fangyuan/mod.rs:1` 说明 blueprint 负责 RON 读取/校验/编译、primitive 负责 runtime 模型、avatar 负责玩法组件绑定）
- [x] 明确 `BlueprintPrimitive`、`RuntimePrimitive`、`RenderInstance` 的命名关系，避免蓝图结构直接承担渲染职责。（验证：`project/src/framework/fangyuan/primitive.rs:39` 注释声明 `FangyuanPrimitive` 是由 blueprint primitive 编译得到的 runtime primitive；`project/src/framework/fangyuan/mod.rs:8` 说明渲染 feature 应从 `FangyuanPrimitiveSet` 派生自己的 render instance）
- [x] 将当前临时放在 preview feature 内的通用 `FangyuanPrimitiveSet` 组件实现迁回 framework 层或统一入口。（验证：`project/src/framework/fangyuan/primitive.rs:68` 为 `FangyuanPrimitiveSet` derive `Component`；`project/src/game/features/fangyuan_player_preview/mod.rs` 已移除对该 framework 类型的临时 `impl Component`）
- [x] 保留 `game/features/fangyuan_player_preview/` 只处理玩家预览特有逻辑。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:56` 保留 `FangyuanPlayerPosition`，`:63` 保留 preview 私有 `FangyuanPlayerPrimitiveVisual` marker，通用 Component impl 已迁出）
- [x] 检查 `framework/fangyuan/mod.rs` 的导出，确保游戏层可以使用统一模型而不依赖 preview feature 私有类型。（验证：`project/src/framework/fangyuan/mod.rs:17` 继续对外导出 `avatar`、`blueprint`、`primitive`）
- [x] 为模块边界补充单元测试或编译测试，确认对外 API 可用。（验证：`project/src/framework/fangyuan/primitive.rs:145` 新增 `primitive_set_is_framework_component_api`，在 Bevy `App` 中 spawn/query `FangyuanPrimitiveSet`）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 63 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 3：Runtime Primitive 基础字段统一

- 开始时间：2026-07-01 19:19:06 +08:00
- 结束时间：2026-07-01 19:30:26 +08:00
- 开发总结：为 runtime primitive 补齐基础字段语义、默认值和访问方法；`FangyuanPrimitiveKind` 默认 `Cube`，`FangyuanPrimitive` 默认合法 identity cube，并通过测试覆盖构造函数、默认值和访问器。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，66 passed；`cargo check` 通过。`rg -n "rotation|quaternion|euler|angular_velocity|rotate|spin" project/src/framework/fangyuan project/src/game/features/fangyuan_player_preview` 未发现 runtime primitive 提供旋转能力，命中仅为拒绝 rotation 的测试、preview Transform identity 检查和否定语义注释。

- [x] 定义或收敛统一 runtime primitive 结构，基础字段包含 `kind`、`local_position`、`scale`、`color`。（验证：`project/src/framework/fangyuan/primitive.rs:50` 的 `FangyuanPrimitive` 字段为 `kind/local_position/scale/color`）
- [x] 继续只允许 `FangyuanPrimitiveKind::Cube` 和 `FangyuanPrimitiveKind::Sphere`。（验证：`project/src/framework/fangyuan/primitive.rs:7` 枚举仅含 `Cube`、`Sphere`，`:26` 反序列化仅接受 `cube`、`sphere`）
- [x] 确认 runtime primitive 中没有 rotation、quaternion、euler、angular_velocity 或旋转语义字段。（验证：`project/src/framework/fangyuan/primitive.rs:50` 的 runtime 字段和 `kind/local_position/scale/color` 访问方法不含旋转语义；`rg` 旋转词检查未发现 runtime 能力）
- [x] 明确 `local_position` 是逻辑 Entity 根节点下的局部坐标。（验证：`project/src/framework/fangyuan/primitive.rs:54` 注释声明其为 logical Entity root node 下的 primitive-local offset）
- [x] 明确 `scale` 是 primitive 局部缩放，不表达旋转或朝向。（验证：`project/src/framework/fangyuan/primitive.rs:56` 注释声明 scale 不编码 rotation 或 facing）
- [x] 为 runtime primitive 构造函数、默认值和访问方法补测试。（验证：`project/src/framework/fangyuan/primitive.rs:167` 起新增 `primitive_kind_default_is_cube`、`primitive_constructor_stores_runtime_fields`、`primitive_default_is_legal_identity_cube`）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 66 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 4：Role 语义模型

- 开始时间：2026-07-01 19:32:24 +08:00
- 结束时间：2026-07-01 19:46:23 +08:00
- 开发总结：新增 `FangyuanPrimitiveRole` 语义枚举和 runtime role 字段；旧 RON v1 role 缺省时继续可解析，并在编译时按 kind 推导 cube=`structure`、sphere=`core`；role 仅作为语义数据进入 primitive set，不改变玩家预览 Entity 或渲染边界。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，75 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 定义 `FangyuanPrimitiveRole` 或等价枚举。（验证：`project/src/framework/fangyuan/primitive.rs:51` 定义 `FangyuanPrimitiveRole`）
- [x] 至少覆盖 `structure`、`core`、`boundary`、`warning`、`trail`、`impact`、`decoration`、`socket`、`archive`。（验证：`project/src/framework/fangyuan/primitive.rs:51` 枚举覆盖 9 个 role，`:70` 的 `as_str` 返回对应 lowercase 名称）
- [x] 为 role 设计默认值，旧 RON v1 未填写 role 时能得到稳定默认 role。（验证：`project/src/framework/fangyuan/primitive.rs:63` 默认 `Structure`，`:84` `default_for_kind` 规定 cube=`Structure`、sphere=`Core`；`project/src/framework/fangyuan/blueprint.rs:156` 的蓝图 role 为 `Option`，`:181` 缺省时按 kind 推导）
- [x] 玩家最小蓝图编译时，cube 身体可标记为 `structure`，sphere 头部可标记为 `core`，或通过明确默认规则实现等价语义。（验证：`project/src/framework/fangyuan/blueprint.rs:683` 的 `minimal_player_blueprint_loads_from_first_package_assets_and_compiles` 断言 primitive 0 role=`Structure`、primitive 1 role=`Core`）
- [x] role 只用于语义、审核、预算和后续 LOD，不改变当前玩家预览的玩法 Entity 边界。（验证：`project/src/framework/fangyuan/primitive.rs:45` 注释声明 role 不定义玩法 Entity 边界或渲染行为；`cargo test fangyuan` 中玩家预览 Entity 边界相关测试继续通过）
- [x] role 中不引入装备挂点、技能规则层或 LOD 的正式实现，只保留数据语义。（验证：本阶段 diff 仅修改 `project/src/framework/fangyuan/primitive.rs` 和 `blueprint.rs`，新增枚举、字段、serde、默认编译和校验测试，未改装备、技能或 LOD 模块）
- [x] 为 role 的 serde、默认值和非法值校验补测试。（验证：`project/src/framework/fangyuan/primitive.rs:276` 覆盖 lowercase serde 名，`:303` 覆盖未知 role 反序列化拒绝，`:316` 和 `:324` 覆盖默认值；`project/src/framework/fangyuan/blueprint.rs:742` 覆盖未知蓝图 role 拒绝）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 75 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 5：材质和生命周期预留字段

- 开始时间：2026-07-01 19:48:43 +08:00
- 结束时间：2026-07-01 20:06:26 +08:00
- 开发总结：runtime primitive 落地 `alpha`、`emissive`、`material_profile_id` 和 `FangyuanPrimitiveLifecycle` 预留数据；旧 RON v1 无新增字段时继续由 color alpha、默认 emissive、None profile 和空 lifecycle 编译；预览渲染继续按 color 复用材质，不消费 emissive/profile/lifecycle，也未实现生命周期播放器。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，81 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 在 runtime primitive 中预留或落地 `alpha`，默认从 color alpha 或 `1.0` 推导。（验证：`project/src/framework/fangyuan/primitive.rs:192` 定义 `alpha` 字段，`:232` `with_role` 从 `color.to_srgba().alpha` 推导；`project/src/framework/fangyuan/blueprint.rs:206` 蓝图缺省 alpha 使用 `color[3]`）
- [x] 在 runtime primitive 中预留或落地 `emissive`，默认值为 `0.0`。（验证：`project/src/framework/fangyuan/primitive.rs:129` 定义 `FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE = 0.0`，`:195` 定义 runtime `emissive` 字段）
- [x] 在 runtime primitive 中预留或落地 `material_profile_id`，默认使用基础 profile 或 `None`。（验证：`project/src/framework/fangyuan/primitive.rs:198` 定义 `material_profile_id: Option<String>`，`project/src/framework/fangyuan/blueprint.rs:174` 蓝图字段可选，默认 `None`）
- [x] 设计 primitive 生命周期数据位置，例如 `lifetime`、`spawn_tick`、`despawn_tick` 或后续 VFX 专用结构，但本阶段不实现 VFX 播放器。（验证：`project/src/framework/fangyuan/primitive.rs:138` 定义 `FangyuanPrimitiveLifecycle { lifetime, spawn_tick, despawn_tick }`，注释声明本阶段无 playback/ticking/VFX system 消费）
- [x] 确认 alpha/emissive/profile 不导致当前渲染路径创建大量材质或改变玩家预览外观。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:452` 的 `fangyuan_preview_material_cache_ignores_reserved_runtime_metadata` 断言不同 alpha/emissive/profile/lifecycle 但同 color 的两个 primitive 仍只产生 1 个材质）
- [x] alpha 和 emissive 的合法范围必须可由 validator 检查。（验证：`project/src/framework/fangyuan/blueprint.rs:579` 校验 alpha `0.0..=1.0`，`:595` 校验 emissive finite 且 `0.0..=FANGYUAN_PRIMITIVE_MAX_EMISSIVE`）
- [x] 为默认 alpha、emissive、material profile 和生命周期空值补测试。（验证：`project/src/framework/fangyuan/blueprint.rs:913` 覆盖默认 alpha/emissive/profile/lifecycle，`:929` 覆盖显式字段；`project/src/framework/fangyuan/primitive.rs:444` 和 `:493` 覆盖 runtime 构造和默认值）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 81 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 6：蓝图 v1 兼容编译

- 开始时间：2026-07-01 20:09:00 +08:00
- 结束时间：2026-07-01 20:17:09 +08:00
- 开发总结：补强蓝图 v1 兼容性测试，确认旧 `minimal_player.ron` 资产未修改即可编译；只含 `kind/position/size/color` 的 legacy RON primitive 会映射到统一 runtime primitive，并填充 role、alpha、emissive、material profile 和 lifecycle 默认值。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，81 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 保持 `project/assets/fangyuan/avatars/minimal_player.ron` 不需要破坏性升级即可通过编译。（验证：`git diff -- project/assets/fangyuan/avatars/minimal_player.ron` 无输出；`project/src/framework/fangyuan/blueprint.rs:753` 从首包资产加载并编译通过）
- [x] 蓝图 primitive v1 继续只要求 `kind`、`position`、`size`、`color`。（验证：`project/src/framework/fangyuan/blueprint.rs:159` 的新增 role/alpha/emissive/material_profile_id/lifecycle 均为 `Option` + serde default；`:876` 的 legacy RON 测试 primitive 只填写四个必填字段）
- [x] 编译器将 v1 字段映射为统一 runtime primitive，并填充 role、alpha、emissive、material profile 的默认值。（验证：`project/src/framework/fangyuan/blueprint.rs:92` compile 调用统一 runtime 构造；`:199`、`:206`、`:210`、`:214` 分别提供 role/alpha/emissive/lifecycle 默认值；`:876` 测试断言默认编译结果）
- [x] 编译器错误不 panic，继续返回结构化 `Result` 或现有错误类型扩展。（验证：`project/src/framework/fangyuan/blueprint.rs:803` 的 `invalid_ron_returns_parse_error_without_panicking` 断言错误通过 `Result` 返回）
- [x] 旧测试 `minimal_player_blueprint_loads_from_first_package_assets_and_compiles` 继续通过。（验证：`project/src/framework/fangyuan/blueprint.rs:753` 测试仍在且 `cargo test fangyuan` 通过）
- [x] 增加测试覆盖旧 RON v1 未填写 role/alpha/emissive/profile 时的默认编译结果。（验证：`project/src/framework/fangyuan/blueprint.rs:876` 的 `compile_defaults_legacy_v1_required_primitive_fields_to_runtime_defaults` 覆盖 role/alpha/emissive/material_profile_id/lifecycle 缺省编译）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 81 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 7：Validator 和结构化错误报告

- 开始时间：2026-07-01 20:19:43 +08:00
- 结束时间：2026-07-01 20:42:32 +08:00
- 开发总结：为蓝图 validation error 增加 `code`、`primitive_index`、`field_path` 和 `reason` 结构化报告 API，并让 Display 输出包含错误码、字段路径和原因；补齐数量、bounds、position、ground、size、color、alpha、emissive 等错误的定位断言，同时用自定义反序列化错误和 `deny_unknown_fields` 继续拒绝非法 kind/role 与 rotation 等旋转字段。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，85 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 扩展或整理 `FangyuanBlueprintError` / `FangyuanValidationError`，包含错误码、primitive index、字段路径和可读原因。（验证：`project/src/framework/fangyuan/blueprint.rs:307` 为 `FangyuanAvatarBlueprintValidationError` 增加 `code()`、`primitive_index()`、`field_path()`、`reason()`；Display 输出包含 code/path/reason）
- [x] 校验 primitive 数量不超过 `min(max_primitives, 1000)`。（验证：`project/src/framework/fangyuan/blueprint.rs:1110` 覆盖 effective limit，`:1138` 覆盖 hard limit，并断言 field path 为 `primitives`）
- [x] 校验 kind 只允许 cube/sphere。（验证：`project/src/framework/fangyuan/blueprint.rs:724` 自定义 `deserialize_primitive_kind`，`:905` 非法 `cylinder` parse error 断言包含 kind 和非法值）
- [x] 校验 position 在 bounds 内，且主体不生成到地面以下。（验证：`project/src/framework/fangyuan/blueprint.rs:1165` 覆盖 bounds 越界，`:1193` 覆盖非有限坐标，`:1217` 覆盖 below ground，均断言 primitive index 和字段路径）
- [x] 校验 scale/size 每轴有限且大于 0。（验证：`project/src/framework/fangyuan/blueprint.rs:1241` 覆盖 size 轴 0，`:1265` 覆盖 size 轴 infinity，并断言 `primitives[0].size[1]`）
- [x] 校验 color、alpha 在 `0.0..=1.0`。（验证：`project/src/framework/fangyuan/blueprint.rs:1289` 覆盖 color channel，`:1313` 覆盖 alpha，并断言字段路径）
- [x] 校验 emissive 有明确非负上限或阶段默认上限。（验证：`project/src/framework/fangyuan/blueprint.rs:1336` 覆盖超过 `FANGYUAN_PRIMITIVE_MAX_EMISSIVE`，`:1360` 覆盖负数 emissive）
- [x] 校验 role 合法，并拒绝未知 role。（验证：`project/src/framework/fangyuan/blueprint.rs:736` 自定义 `deserialize_optional_primitive_role`，`:949` 非法 `weapon_socket` parse error 断言包含 role 和非法值）
- [x] 使用 `deny_unknown_fields` 或等价方式继续拒绝 rotation、quaternion、euler、angular_velocity、rotate、spin。（验证：`project/src/framework/fangyuan/blueprint.rs:158` 保持 `deny_unknown_fields`；`:819` 参数化测试覆盖 6 个禁止字段均 parse error）
- [x] 为每类错误补充单元测试，断言错误信息能定位 primitive index 或字段。（验证：`project/src/framework/fangyuan/blueprint.rs:1383` 的 `assert_validation_report` 统一断言 code/index/path/reason/display；各 validator 测试调用该断言）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 85 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 8：对象根状态和 PrimitiveSet 通用组件

- 开始时间：2026-07-01 20:45:07 +08:00
- 结束时间：2026-07-01 21:06:59 +08:00
- 开发总结：新增 framework 层 `FangyuanObjectState` 组件作为逻辑对象根状态，包含 active、visible、root_translation 和 root_scale；`FangyuanPrimitiveSet` 明确为逻辑根 Entity 内部数据容器；玩家预览保留 `FangyuanPlayerPosition` 特化组件，同时同步通用根状态和 Transform，render-only 子实体继续不持有 primitive set、root state 或业务状态。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，89 passed；`cargo check` 通过。`rg` 旋转词检查未发现 `FangyuanObjectState` 暴露 rotation API，命中仅为 preview Transform identity 约束和否定说明；验证中仅出现既有 `checkbox` dead_code warning。

- [x] 明确 `FangyuanPrimitiveSet` 是逻辑 Entity 内部的数据容器，可作为 Bevy Component 挂到玩家、家园物件、装备、技能或天道生成物根 Entity。（验证：`project/src/framework/fangyuan/primitive.rs:311` 注释声明其挂在 logical Fangyuan object root entity，可用于 player/home object/equipment/skill/NPC/Tiandao roots；`project/src/framework/fangyuan/object.rs:76` 测试将 primitive set 与 root state 一起挂到 Entity）
- [x] 定义或整理方圆对象根状态的最小通用字段，例如 active、visible、root translation、root scale。（验证：`project/src/framework/fangyuan/object.rs:9` 定义 `FangyuanObjectState { active, visible, root_translation, root_scale }`，`:61` 测试默认 active/visible=true、translation zero、scale one）
- [x] 确认对象根状态只表达整体移动和整体 scale，不暴露旋转。（验证：`project/src/framework/fangyuan/object.rs:9` 字段不含 rotation；`:101` 测试只同步 translation/scale；`rg` 检查未发现 object state 旋转 API）
- [x] 玩家预览中的 `FangyuanPlayerPosition` 可继续作为特化组件，但不能和统一模型冲突。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:55` 保留 `FangyuanPlayerPosition`，`:203` 同步 `FangyuanObjectState.root_translation` 与 Transform）
- [x] render-only 子实体不拥有 `FangyuanPrimitiveSet`，也不拥有输入、移动、血量、技能或 authority 状态。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:549` 测试断言 visual child 不含 `FangyuanObjectState`、`FangyuanAvatar`、`FangyuanPrimitiveSet`、玩家状态等业务组件）
- [x] 为 primitive set 作为组件、根移动同步和无旋转约束补测试。（验证：`project/src/framework/fangyuan/object.rs:76` 覆盖 primitive set + root state 共同作为组件；`project/src/game/features/fangyuan_player_preview/mod.rs:342` 覆盖移动/scale 同步并保持 rotation 为 identity）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 89 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 9：玩家预览迁移到统一模型

- 开始时间：2026-07-01 21:09:26 +08:00
- 结束时间：2026-07-01 21:17:22 +08:00
- 开发总结：玩家预览继续通过 framework 层 `FangyuanPrimitiveSet`、runtime primitive、`FangyuanAvatar` 和 `FangyuanObjectState` 表达玩家逻辑 Entity；本阶段补强测试，确认最小玩家 cube/sphere 的 role、alpha、emissive、material profile、lifecycle 默认值不破坏显示，visual child 仍为 render-only，入口、返回大厅、相机和灯光行为保持。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_player_preview -- --nocapture` 通过，23 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] `FangyuanPlayerPreviewPlugin` 继续只在 `AppUiMode::FangyuanPlayerPreview` 进入时生成玩家 Entity。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:23` 在 `OnEnter(AppUiMode::FangyuanPlayerPreview)` 注册生成系统；`:283` 测试非预览模式不生成玩家）
- [x] 玩家 Entity 使用 framework 层统一 `FangyuanPrimitiveSet` 和 runtime primitive 类型。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:136` spawn 插入 framework `FangyuanPrimitiveSet` 和 `FangyuanObjectState`；`:298` 测试玩家组件包含 unified primitive set）
- [x] 玩家 Entity 上的 `FangyuanAvatar` 或等价组件继续记录 blueprint id、显示名和 primitive set。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:156` 构造 `FangyuanAvatar`；`:298` 测试断言 blueprint id、display name 和 avatar primitive set）
- [x] cube 身体和 sphere 头部的 role、alpha、emissive、material profile 默认值不破坏当前显示效果。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:334` 的 `fangyuan_preview_player_uses_minimal_runtime_primitive_defaults` 断言 cube=`Structure`、sphere=`Core`、alpha=color alpha、emissive 默认、profile None、lifecycle empty，并保持既有 kind/position/scale/color）
- [x] 移动玩家根 Entity 时，所有 visual child 仍整体跟随。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:608` 的 parent/local transform 测试覆盖根移动后 visual child 仍跟随）
- [x] 玩家预览入口、返回大厅按钮、相机和灯光行为不回退。（验证：`project/src/game/screens/lobby/mod.rs:251` 覆盖大厅入口；`project/src/game/screens/gameplay/fangyuan_player_preview.rs:219` 覆盖返回大厅按钮，`:255` 覆盖相机，`:273` 覆盖灯光）
- [x] 为玩家预览迁移补测试，确认只生成一个玩家逻辑 Entity，primitive 不成为玩法 Entity。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:298` 覆盖只生成一个玩家逻辑 Entity 和组件，`:410` 覆盖 primitive 保持为玩家 Entity 内部数据，`:587` 覆盖 visual child 组件隔离）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_player_preview -- --nocapture`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan_player_preview -- --nocapture` 为 23 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 10：渲染适配层保持 Render-only

- 开始时间：2026-07-01 21:20:18 +08:00
- 结束时间：2026-07-01 21:37:57 +08:00
- 开发总结：玩家预览渲染适配继续从统一 runtime primitive 派生 render-only visual child；visual marker 记录 kind/index/alpha 以证明读取 runtime 字段，Transform 和材质继续由 local position、scale、color 派生；mesh 仍复用 unit cube/sphere，材质仍按 color 复用，emissive/profile/lifecycle 不参与材质缓存，也未引入 mesh merge、GPU instancing 或完整 FangyuanMaterial。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_player_preview -- --nocapture` 通过，24 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 渲染适配层从统一 runtime primitive 读取 kind、local position、scale、color、alpha。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:166` 的 visual spawn 读取 kind/color/local_position/scale/alpha；`:638` 字段映射测试断言 visual record 与 primitive 数据一致）
- [x] 当前阶段继续使用缓存 unit cube 和 unit sphere mesh，不引入 mesh merge 或 GPU instancing。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:91` 缓存 unit mesh；`:456` 测试两个 cube 共用 cube mesh、两个 sphere 共用 sphere mesh，未新增 merge/instancing 逻辑）
- [x] 当前阶段继续按颜色或等价方式复用基础材质，不实现完整 FangyuanMaterial。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:70` 使用 color key，`:120` 通过 material cache 按 color 取材质；`:523` 测试同色材质复用）
- [x] render-only 子实体只挂显示相关组件和视觉 marker。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:182` spawn bundle 只含 visual marker、mesh、material、Transform/GlobalTransform、Visibility/InheritedVisibility/ViewVisibility 和 Name 等显示相关组件）
- [x] render-only 子实体不挂玩家、家园、技能、authority、输入、血量或业务状态组件。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:668` 组件边界测试断言 visual child 不含 `FangyuanPlayer`、`FangyuanPlayerState`、`FangyuanPlayerPosition`、`FangyuanObjectState`、`FangyuanAvatar`、`FangyuanPrimitiveSet` 等）
- [x] alpha/emissive/profile 预留字段如果暂未渲染，必须有明确默认行为，不影响当前 demo。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:556` 测试 color alpha 决定 `StandardMaterial` alpha mode；`:585` 测试不同 alpha/emissive/profile/lifecycle 不分裂材质缓存且材质仍按 color）
- [x] 增加测试覆盖 visual child 的组件边界、mesh 复用、材质复用和 primitive 字段映射。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:456` 覆盖 mesh 复用，`:523` 覆盖材质复用，`:638` 覆盖 primitive 字段映射，`:668` 覆盖组件边界）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_player_preview -- --nocapture`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan_player_preview -- --nocapture` 为 24 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 11：基础调试统计和预算入口

- 开始时间：2026-07-01 21:40:40 +08:00
- 结束时间：2026-07-01 21:54:26 +08:00
- 开发总结：新增 framework 层 `FangyuanPrimitiveSetStats` 和 `FangyuanPrimitiveRoleDistribution`，并在 `FangyuanPrimitiveSet` 上提供 `stats()`；统计直接扫描 runtime primitive 数据，覆盖 total、cube/sphere、role 分布、颜色数量、透明 primitive 数、发光 primitive 数和非默认 profile 数，不依赖 render-only 实体，也未实现完整预算、自动降级或 UI 面板。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan` 通过，93 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 提供 primitive set 基础统计，例如 total、cube_count、sphere_count。（验证：`project/src/framework/fangyuan/stats.rs:14` 定义 `FangyuanPrimitiveSetStats`，`:39` 计算 total/cube_count/sphere_count）
- [x] 提供 role 分布统计，用于后续预算、LOD 和审核报告。（验证：`project/src/framework/fangyuan/stats.rs:72` 定义 `FangyuanPrimitiveRoleDistribution`，`:46` 逐 primitive 累加 role 分布）
- [x] 提供材质相关基础统计，例如颜色数量、alpha 使用数量、emissive 使用数量或 profile 使用数量。（验证：`project/src/framework/fangyuan/stats.rs:19` 定义 color_count/alpha_count/emissive_count/material_profile_count，`:47` 直接由 primitive 数据计算）
- [x] 统计接口不依赖渲染实体数量，直接基于 `FangyuanPrimitiveSet`。（验证：`project/src/framework/fangyuan/stats.rs:29` `from_primitive_set` 和 `:66` `FangyuanPrimitiveSet::stats()` 均从 `primitives()` 扫描；模块注释声明不检查 render-only visual entities）
- [x] 当前阶段不实现完整灵构额度、不做自动降级、不做 UI 调试面板。（验证：本阶段仅新增 `project/src/framework/fangyuan/stats.rs` 和 `framework/fangyuan/mod.rs` 导出，未改 UI、预算降级或渲染系统）
- [x] 为统计接口补测试，覆盖最小玩家 primitive set 的 total、cube/sphere 和 role 分布。（验证：`project/src/framework/fangyuan/stats.rs:157` 的 `stats_cover_minimal_player_primitive_set` 覆盖 total=2、cube=1、sphere=1、structure/core 各 1 和基础材质统计）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan`、`cargo check`。（验证：三条命令均通过；`cargo test fangyuan` 为 93 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 阶段 12：文档同步和最终验收

- 开始时间：2026-07-01 21:57:06 +08:00
- 结束时间：2026-07-01 22:08:34 +08:00
- 开发总结：同步方圆技术路线、蓝图规则、新成员入门和协作文档，记录第二阶段实际落地的数据模型边界、runtime/render-only 口径、禁止旋转和后续阶段延后事项；完整自动化验证已通过。用户手动从大厅进入玩家预览并确认 cube 身体/sphere 头部显示仍待人工执行。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过，93 passed；`cargo test fangyuan_player_preview -- --nocapture` 通过，24 passed；`cargo check` 通过。验证中仅出现既有 `src/framework/ui/widgets/controls/selection.rs:32` 的 `checkbox` dead_code warning，非本阶段改动。

- [x] 更新 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md`，记录第二阶段实际落地的数据模型边界。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:422` 记录 `blueprint.rs`、`primitive.rs`、`object.rs`、`stats.rs` 和玩家预览 render-only 适配边界）
- [x] 更新 `docs/世界观/方圆灵构蓝图规则.md`，把“每个 primitive 只保留静态 Transform / 挂到 root 下”的旧口径修正为 render-only 或 runtime 数据口径。（验证：`docs/世界观/方圆灵构蓝图规则.md:396` 改为 runtime primitive 数据和 render-only 子实体口径）
- [x] 如果代码结构变化影响新成员理解，检查是否需要同步 `docs/bevy-getting-started.md` 或根目录协作文档。（验证：已同步 `docs/bevy-getting-started.md:231` 和 `CLAUDE.md:29`，补充 `project/src/framework/fangyuan/` 与 Fangyuan Player Preview 说明）
- [x] 确认文档仍明确禁止旋转，并明确当前阶段不实现 Prefab、Bake、mesh merge、Instancing、VFX、LOD、AOI。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:425` 明确 runtime 不含旋转字段，`:430` 明确未实现项；`docs/世界观/方圆灵构蓝图规则.md:406` 明确当前阶段未实现项）
- [x] 运行完整相关验证命令并记录结果。（验证：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_player_preview -- --nocapture`、`cargo check` 均通过）
- [ ] 用户手动验收玩家预览页面仍可从大厅进入，并显示 cube 身体和 sphere 头部。
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_player_preview -- --nocapture`、`cargo check`。（验证：四条命令均通过；`fangyuan` 93 passed，`fangyuan_player_preview` 24 passed，`cargo check` 通过，仅有既有 `checkbox` dead_code warning）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-01 22:08:34 +08:00
- 结束时间：
- 验收总结：自动化验收与文档同步已完成；剩余用户手动验收游戏内玩家预览效果无回退。

- [x] 方圆数据模型明确区分蓝图 primitive、runtime primitive 和渲染适配数据。（验证：`project/src/framework/fangyuan/blueprint.rs:93` 编译蓝图到 primitive set；`project/src/framework/fangyuan/primitive.rs:178` 定义 runtime primitive；`project/src/game/features/fangyuan_player_preview/mod.rs:166` 从 runtime primitive 派生 render-only visual child）
- [x] `FangyuanPrimitiveSet` 位于 framework 层，可作为后续玩家、家园、装备、技能、NPC 和天道生成物复用的数据容器。（验证：`project/src/framework/fangyuan/primitive.rs:317` 定义 framework 层 Component；`:311` 注释声明可挂到 player/home/equipment/skill/NPC/Tiandao roots）
- [x] runtime primitive 至少包含 kind、local position、scale、color，并具备 role、alpha、emissive、material profile 的稳定默认语义。（验证：`project/src/framework/fangyuan/primitive.rs:178` 字段包含 kind/local_position/scale/color/role/alpha/emissive/material_profile_id/lifecycle；`:203`、`:220` 提供默认构造）
- [x] 当前 RON v1 最小玩家蓝图不需要破坏性升级即可继续加载。（验证：`project/src/framework/fangyuan/blueprint.rs:850` 从 `minimal_player.ron` 加载并编译通过；资产文件未修改）
- [x] 蓝图编译器能为旧字段填充 role、alpha、emissive 和 material profile 默认值。（验证：`project/src/framework/fangyuan/blueprint.rs:976` 的 legacy v1 测试只填写 kind/position/size/color 并断言 role/alpha/emissive/profile 默认值）
- [x] validator 能返回结构化错误，并覆盖数量、kind、bounds、size、color、alpha、emissive、role 和禁止旋转字段。（验证：`project/src/framework/fangyuan/blueprint.rs:307` 提供 code/index/path/reason；`:819`、`:905`、`:949`、`:1074` 起覆盖各类错误）
- [x] 玩家预览继续从大厅入口进入，且只生成一个玩家逻辑 Entity。（验证：`project/src/game/screens/lobby/mod.rs:251` 覆盖大厅入口；`project/src/game/features/fangyuan_player_preview/mod.rs:269` 覆盖只生成一个玩家 Entity）
- [x] cube 身体和 sphere 头部仍是玩家 Entity 内部 primitive set 的渲染表现，不是玩法 Entity。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:333` 覆盖 minimal runtime primitive；`:412` 覆盖 primitive 仍是玩家 Entity 内部数据）
- [x] render-only 子实体只承担显示职责，不挂输入、移动、血量、技能、authority 或业务状态。（验证：`project/src/game/features/fangyuan_player_preview/mod.rs:668` 断言 visual child 不含玩家/root/avatar/primitive set 等业务组件）
- [x] 方圆数据模型和测试中不存在 rotation、quaternion、euler、angular_velocity、rotate 或 spin 能力。（验证：`project/src/framework/fangyuan/blueprint.rs:819` 禁止字段测试覆盖 6 个旋转字段；`project/src/framework/fangyuan/primitive.rs:184` 声明 scale 不编码 rotation/facing；`docs/世界观/方圆灵构蓝图规则.md:416` 禁止生成旋转字段）
- [x] 基础统计能输出 primitive 总数、cube/sphere 数量和 role 分布。（验证：`project/src/framework/fangyuan/stats.rs:14` 定义 stats 字段，`:158` 测试最小玩家 total=2、cube=1、sphere=1、structure/core 分布）
- [x] 文档同步记录第二阶段落地边界和后续阶段延后事项。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:422` 记录落地边界，`:430` 记录未实现项；`docs/世界观/方圆灵构蓝图规则.md:250` 记录 RON v1 可选字段）
- [x] `cargo fmt --check` 通过。（验证：2026-07-01 22:08 阶段 12 主流程复跑通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：2026-07-01 22:08 阶段 12 主流程复跑通过，93 passed）
- [x] `cargo test fangyuan_player_preview -- --nocapture` 通过。（验证：2026-07-01 22:08 阶段 12 主流程复跑通过，24 passed）
- [x] `cargo check` 通过。（验证：2026-07-01 22:08 阶段 12 主流程复跑通过，仅有既有 `checkbox` dead_code warning）
- [ ] 用户手动验收游戏内玩家预览效果无回退。
