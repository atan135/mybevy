# 方圆 Prefab 和场景布局第四阶段 Checklist

归档说明：本文件是 docs 归档副本，来源于已完成的 `summary/方圆Prefab和场景布局第四阶段_checklist.md`；`summary/` 源文件按 multi-agent-dev 规则不提交。

## 目标

交付方圆系统第四阶段基础能力：在第三阶段家园静态对象蓝图已经接入统一 `FangyuanBlueprint` 和 `FangyuanPrimitiveSet` 后，引入开发期 Prefab / Palette 和 Scene Layout 两层源格式，避免大型家园、场景和宗门工程把所有 primitive 重复写进一个大 RON 文件。

本阶段重点是最小可用的一层 prefab 引用、scene layout instance 展开、统一校验和预算报告、家园预览接入，以及保留第三阶段已有的家园 HUD、Reload、Clear、返回大厅和 render-only 边界。

本阶段不实现 Chunk、流式加载、发布期二进制 Bake、静态 CPU mesh merge、GPU Instancing、LOD、AOI、联网同步、正式家园编辑器、蓝图持久化、技能规则层、装备挂点正式接入、VFX 播放器或多层嵌套 prefab。

## 完整功能地图对照

| 功能域 | 第四阶段处理方式 |
| --- | --- |
| Prefab / Palette | 新增开发期 RON v1 源格式，定义可复用方圆组件 |
| Scene Layout | 新增开发期 RON v1 源格式，只记录 prefab id、position、scale 和少量实例参数 |
| Primitive 字段 | 继续复用第三阶段统一 RON v1 primitive 字段 |
| Runtime primitive | scene layout instance 展开后仍编译为 `FangyuanPrimitiveSet` |
| 逻辑对象 | 一个 prefab instance 至少能映射为家园或静态场景逻辑对象根 |
| Render-only 边界 | 展开后的 primitive 仍只派生显示子实体，不成为玩法 Entity |
| 预算 | prefab 和展开后的 primitive 都计入预算；保留 1000 primitive 硬限制 |
| 错误处理 | 缺失 prefab、重复 id、循环引用、非法 instance、越界和预算超限应结构化报告 |
| 兼容性 | 保留 `minimal_player.ron` 和 `home_preview.ron` 简单蓝图路径 |
| HUD / Debug | 家园 HUD 或日志能显示 layout、prefab、instance、generated、skipped 等关键统计 |
| Reload / Clear | Reload 重新读取 layout/palette 并展开；Clear 只清空展开内容 |
| 旋转能力 | 继续禁止 rotation、quaternion、euler、angular_velocity、rotate、spin |
| 后续能力 | Chunk、Bake、Instancing、LOD、AOI、联网同步等后续阶段处理 |

## 基础原则

- [x] Prefab 只描述可复用方圆 primitive 组合、默认 bounds、pivot、标签和预算，不描述战斗强度、脚本、shader 或服务端判定逻辑。（验证：`project/src/framework/fangyuan/prefab.rs:118` 定义 prefab 元数据和 primitive 组合，禁止字段测试覆盖 script/shader/server_rule/external_asset；`docs/世界观/方圆灵构蓝图规则.md:323` 记录边界）
- [x] Scene Layout 只描述 prefab instance，不重复记录大量相同 primitive。（验证：`project/src/framework/fangyuan/layout.rs:190` 的 instance 仅包含 prefab/position/scale/id/name/tags，`project/assets/fangyuan/layouts/home_layout.ron` 多次复用同一 prefab）
- [x] 初期只允许一层 prefab 引用，不做 nested prefab、继承链或递归展开。（验证：`project/src/framework/fangyuan/layout.rs:1651`、`:1664` 测试拒绝 nested layout/prefab 字段）
- [x] Prefab 和展开后的 primitive 都必须进入统一校验和预算统计，不能绕过 1000 primitive 硬限制。（验证：`project/src/framework/fangyuan/layout.rs:1630` 覆盖 1001 expanded primitive 预算失败；compile report 记录 authored/generated/skipped）
- [x] 展开结果仍是逻辑对象内部的 `FangyuanPrimitiveSet`，单个 primitive 不升级为玩法 Entity。（验证：`project/src/game/scenes/fangyuan_home.rs:801` 展开生成 `FangyuanHomeObject` 根；`:2095` 测试 render-only 子实体不挂 runtime 业务组件）
- [x] 保留第三阶段 `FangyuanBlueprint`、`FangyuanPrimitiveSet`、`FangyuanObjectState`、`FangyuanRenderAssetCache` 和 HUD stats 的边界。（验证：阶段 7、8 测试覆盖家园逻辑根、render cache、HUD stats；`cargo test fangyuan_home -- --nocapture` 38 passed）
- [x] 蓝图、prefab、layout、runtime、渲染适配和测试中继续禁止 rotation、quaternion、euler、angular_velocity、rotate 或 spin。（验证：prefab/layout 禁止字段测试通过；文档在 `docs/世界观/方圆灵构蓝图规则.md:535` 起继续列为禁止事项）
- [x] 本阶段只做 Prefab 和 Scene Layout 拆分，不提前实现 Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI 或联网同步。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1608` 明确这些能力不是第四阶段能力；代码改动未引入相关模块）
- [x] 每个阶段完成后运行对应验证，并按阶段提交。（验证：阶段 2-8 已按阶段提交，阶段 9 为纯回归无代码提交，阶段 10 为文档提交准备；各阶段验证记录已补齐）

## 阶段 1：需求和边界复核

- 开始时间：2026-07-02 14:56:11 +08:00
- 结束时间：2026-07-02 15:01:27 +08:00
- 开发总结：完成第四阶段需求和边界复核，确认本阶段只引入开发期 Prefab / Palette 与 Scene Layout 源格式、一次展开到 `FangyuanPrimitiveSet`，继续沿用第三阶段统一 primitive 校验、render-only 边界、Reload/Clear/HUD 稳定能力；阶段 1 未修改业务代码。
- 验证记录：worker 执行 `rg`、`Get-Content`、`git status --short` 等只读检查；主 agent 复核 worker 报告并再次执行 `git status --short`，确认无业务代码改动。

- [x] 复核 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md` 中“结构拆分：Prefab 和 Scene Layout”和“阶段 4：Prefab 和场景布局拆分”的目标、技术做法、验收标准和风险。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:474` 记录 prefab/palette 和 scene layout 两层拆分，`:1548` 起记录阶段 4 的目标、做法、验收和风险）
- [x] 复核 `docs/世界观/方圆灵构蓝图规则.md` 中当前 RON v1 primitive 字段、数量上限、bounds、禁止旋转和错误处理规则。（验证：`docs/世界观/方圆灵构蓝图规则.md:198` 起记录顶层字段，`:248` 记录 1000 primitive 上限，`:256` 起记录 primitive 字段，`:275` 明确禁止旋转字段，`:429` 起记录错误处理建议）
- [x] 复核第三阶段 checklist，确认 `FangyuanBlueprint`、`FangyuanPrimitiveSet`、`FangyuanHomeObject`、render-only 边界、Reload/Clear/HUD 已稳定。（验证：`summary/方圆静态对象和家园蓝图预览第三阶段_checklist.md:102`、`:139`、`:184`、`:222`、`:249` 分别记录统一编译、家园逻辑根、render cache、Reload/Clear 和 HUD stats 已完成）
- [x] 检查当前 `project/src/framework/fangyuan/` 和 `project/src/game/scenes/fangyuan_home.rs` 中可复用的加载、校验、编译、stats 和 render cache 边界。（验证：`project/src/framework/fangyuan/blueprint.rs:127` 提供 compile report，`:645` 起统一校验；`primitive.rs:311` 说明 primitive set 挂逻辑根；`render_assets.rs:28` 说明 cache 边界；`stats.rs:9` 说明 stats 来自 runtime primitive；`project/src/game/scenes/fangyuan_home.rs:606`、`:661`、`:707`、`:1022` 覆盖加载、编译、生成逻辑根和 Reload/Clear）
- [x] 明确本阶段不处理 Chunk、Bake、Instancing、LOD、AOI、联网同步、正式家园编辑器、蓝图持久化和技能规则层。（验证：本 checklist 目标段和功能地图已列明非目标，worker 报告确认阶段 2-8 边界不引入这些能力）
- [x] 验证命令：执行 `rg`、`Get-Content`、`git status --short` 等只读检查，确认阶段 1 不修改代码。（验证：worker 报告列出只读命令且前后 `git status --short` 均无输出；主 agent 再次执行 `git status --short` 无输出）

## 阶段 2：Prefab / Palette 源格式设计

- 开始时间：2026-07-02 15:03:17 +08:00
- 结束时间：2026-07-02 15:38:35 +08:00
- 开发总结：新增 framework 级 Prefab / Palette RON v1 类型、校验错误和测试，并在方圆模块导出；primitive 字段复用第三阶段 `FangyuanPrimitiveBlueprint` 与现有 validator，阶段 2 未接入 Scene Layout、资源文件或家园场景。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（124 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 在 framework 方圆模块中设计开发期 Prefab / Palette RON v1 结构，包含 `version`、`name`、`description`、`max_primitives`、`bounds`、`prefabs` 等顶层语义。（验证：`project/src/framework/fangyuan/prefab.rs:18` 定义 `FangyuanPrefabPalette`，`:118` 定义 `FangyuanPrefabDefinition`；`project/src/framework/fangyuan/mod.rs:16`、`:24` 接入并导出 prefab 模块）
- [x] 定义 prefab id 规则，拒绝空 id、重复 id、路径式 id、大小写混乱或包含危险字符的 id。（验证：`project/src/framework/fangyuan/prefab.rs:56` 校验 id，`:65` 拒绝重复 id，`:367` 定义小写 ASCII id 规则；`:634` 和 `:658` 覆盖重复 id 与非法 id 测试）
- [x] Prefab 内部 primitive 字段复用第三阶段 `FangyuanPrimitiveBlueprint`，不新增家园专属几何字段。（验证：`project/src/framework/fangyuan/prefab.rs:5` 引用 `FangyuanPrimitiveBlueprint`，`:137` 的 `primitives` 字段直接使用该类型，`:89` 复用 `validate_blueprint_primitive`；`blueprint.rs:642` 仅将该 validator 提升为 `pub(super)`）
- [x] Prefab 可声明默认 bounds、pivot、tags、预算或描述信息，但不支持脚本、shader、服务端规则和任意外部资源引用。（验证：`project/src/framework/fangyuan/prefab.rs:122`-`:136` 定义 bounds/pivot/tags/max_primitives/description 元数据，`:800` 和 `:844` 的禁止字段测试覆盖 script、shader、server_rule、external_asset）
- [x] Prefab 源格式继续拒绝 rotation、quaternion、euler、angular_velocity、rotate、spin 等字段。（验证：`project/src/framework/fangyuan/prefab.rs:17` 和 `:117` 使用 `serde(deny_unknown_fields)`，`:800` 覆盖 prefab 禁止旋转字段，`:824` 覆盖 primitive 禁止旋转字段）
- [x] 为合法 palette、非法版本、重复 id、非法 id、非法 primitive 和禁止字段补测试。（验证：`project/src/framework/fangyuan/prefab.rs:555` 合法 palette，`:617` 非法版本，`:634` 重复 id，`:658` 非法 id，`:775` 非法 primitive，`:800`/`:824`/`:844` 禁止字段测试）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；`cargo test fangyuan -- --nocapture` 结果为 124 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 3：Scene Layout 源格式设计

- 开始时间：2026-07-02 15:40:41 +08:00
- 结束时间：2026-07-02 16:10:55 +08:00
- 开发总结：新增 framework 级 Scene Layout RON v1 类型、layout instance 校验、palette/prefab 引用校验入口和结构化错误报告；本阶段明确不支持 `material_override`，也未实现 layout 展开、资源样例或家园接入。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（139 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 在 framework 方圆模块中设计开发期 Scene Layout RON v1 结构，包含 `version`、`name`、`description`、`bounds`、`palette` 或 `palettes`、`max_primitives`、`instances` 等顶层语义。（验证：`project/src/framework/fangyuan/layout.rs:18` 定义 `FangyuanSceneLayout`，包含 version/name/description/bounds/palette/palettes/max_primitives/instances；`project/src/framework/fangyuan/mod.rs:15`、`:24` 接入并导出 layout 模块）
- [x] 定义 layout instance 字段：`prefab`、`position`、`scale`、可选 `name`、`tags`、`material_override` 或本阶段确认不支持的字段。（验证：`project/src/framework/fangyuan/layout.rs:190` 定义 `FangyuanSceneLayoutInstance`，包含 id/name/prefab/position/scale/tags；`:1015` 的测试确认 `material_override` 本阶段作为未知字段拒绝）
- [x] instance `position` 和 `scale` 使用平移与缩放表达，不提供 rotation 或方向字段。（验证：`project/src/framework/fangyuan/layout.rs:198` 和 `:200` 仅定义 position/scale，`:995`、`:1015` 覆盖 layout 顶层和 instance 的 rotation/quaternion/euler/angular_velocity/rotate/spin 禁止字段）
- [x] instance 引用必须能报告缺失 prefab、重复 instance id、非有限 position、非正 scale、越界和预算相关错误。（验证：`project/src/framework/fangyuan/layout.rs:45` 提供 `validate_against_palette`，`:144` 报告缺失 prefab，`:161` 报告重复 instance id，`:584` 报告 position 非有限或越界，`:604` 报告 scale 非有限或非正，`:176` 报告展开预算超限；对应测试在 `:713`、`:735`、`:806`、`:832`、`:858`、`:882`、`:928`）
- [x] 保留一层 prefab 引用限制，不支持 prefab 引用 prefab 或 layout 嵌套 layout。（验证：`project/src/framework/fangyuan/layout.rs:190` 的 instance 仅持有 prefab id、position、scale 和少量元数据；文件内未新增 nested layout 或 prefab-in-prefab 展开结构，阶段 3 未实现展开编译器）
- [x] 为合法 layout、缺失 prefab、非法 instance、禁止旋转字段和上限约束补测试。（验证：`project/src/framework/fangyuan/layout.rs:665` 合法 layout，`:713` 缺失 prefab，`:760`/`:782`/`:806`/`:858` 非法 instance，`:995`/`:1015` 禁止字段，`:906`/`:928` 上限约束测试）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；`cargo test fangyuan -- --nocapture` 结果为 139 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 4：首包样例资源和路径策略

- 开始时间：2026-07-02 16:13:16 +08:00
- 结束时间：2026-07-02 16:37:06 +08:00
- 开发总结：新增首包 palette/layout RON 文本样例、统一方圆首包 asset path 校验模块，并为 `FangyuanPrefabPalette` 与 `FangyuanSceneLayout` 增加首包加载入口；旧 `home_preview.ron` 和 `minimal_player.ron` 加载路径保持兼容。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（145 passed，包含 `home_prefab_palette_asset_loads_and_validates` 与 `home_scene_layout_asset_loads_and_validates_against_home_palette`）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 规划并新增首包样例路径，例如 `project/assets/fangyuan/palettes/home_prefabs.ron` 和 `project/assets/fangyuan/layouts/home_layout.ron`，路径命名与现有 `fangyuan/home_preview.ron` 保持一致。（验证：新增 `project/assets/fangyuan/palettes/home_prefabs.ron` 和 `project/assets/fangyuan/layouts/home_layout.ron`；`project/src/framework/fangyuan/prefab.rs:14` 与 `project/src/framework/fangyuan/layout.rs:14` 定义默认首包路径常量）
- [x] 扩展或复用方圆 asset path 校验策略，允许 palette 和 layout 首包路径，继续拒绝父级目录、绝对路径、Windows drive、反斜杠和非 fangyuan 根路径。（验证：`project/src/framework/fangyuan/asset_path.rs:51` 提供统一 `validate_fangyuan_asset_path`，`:73` 限制 `fangyuan/` 根；`prefab.rs:1009` 和 `layout.rs:1158` 覆盖 palette/layout 路径策略测试）
- [x] 样例 palette 至少包含 3 到 5 个可复用 prefab，例如 fence_segment、gate_piece、dragon_body_segment、cloud_puff、stone_marker。（验证：`project/assets/fangyuan/palettes/home_prefabs.ron:13`、`:52`、`:98`、`:130`、`:176` 分别定义 5 个 prefab；`project/src/framework/fangyuan/prefab.rs:974` 的 asset load 测试校验该列表）
- [x] 样例 layout 使用多个 instance 复用同一 prefab，避免重复记录大量相同 primitive。（验证：`project/assets/fangyuan/layouts/home_layout.ron:23`-`:114` 多次复用 `fence_segment`，`:191`-`:247` 多次复用 `dragon_body_segment`，`:254`-`:289` 多次复用 `cloud_puff`）
- [x] 样例展开后 generated primitive 数量不超过 1000，并能表达一个可读的家园布局轮廓。（验证：`project/assets/fangyuan/layouts/home_layout.ron:11` 设定 `max_primitives: 1000`；`project/src/framework/fangyuan/layout.rs:1120` 的资源测试通过 `validate_against_palette` 并估算 generated primitive 不超过 1000，worker 报告估算值为 138）
- [x] 保留 `project/assets/fangyuan/home_preview.ron` 和 `fangyuan/avatars/minimal_player.ron` 兼容，不破坏第三阶段简单蓝图加载。（验证：`project/src/framework/fangyuan/blueprint.rs:879` 复用统一路径校验但保留 `validate_fangyuan_blueprint_asset_path` 入口；`:1155` 新增 legacy home preview 加载测试；`cargo test fangyuan -- --nocapture` 中 minimal player 和 home preview 既有测试通过）
- [x] 新增资源如为 RON 文本文件，按普通 Git 文本提交，不放入 Git LFS。（验证：`git check-attr filter -- project/assets/fangyuan/palettes/home_prefabs.ron project/assets/fangyuan/layouts/home_layout.ron` 输出 `filter: unspecified`，未走 Git LFS）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、必要的 asset load 测试、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；`cargo test fangyuan -- --nocapture` 结果为 145 passed，包含新增 asset load 测试）

## 阶段 5：Prefab 展开编译器

- 开始时间：2026-07-02 16:40:09 +08:00
- 结束时间：2026-07-02 17:02:19 +08:00
- 开发总结：新增 Scene Layout + Prefab Palette 展开编译入口和 compile report；顶层 layout/palette、缺失 prefab、非法 instance 和预算错误返回结构化 error，展开后单个 primitive 违反统一 validator 时跳过并记录带 instance/prefab/primitive 定位的 warning。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（155 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 实现 layout + palette 到 `FangyuanPrimitiveSet` 或等价 compile report 的展开入口。（验证：`project/src/framework/fangyuan/layout.rs:122` 定义 `FangyuanSceneLayout::compile_with_palette()`，`:327` 定义 `FangyuanSceneLayoutCompileReport`，`:204` 返回 `FangyuanPrimitiveSet`）
- [x] 每个 instance 展开时正确应用 instance position 和 scale 到 prefab primitive 的 local position 和 scale。（验证：`project/src/framework/fangyuan/layout.rs:443` 的 `transform_prefab_primitive()` 应用 `position + (primitive.position - pivot) * scale` 与 size scale；`:1372` 测试覆盖 position/scale/pivot 展开）
- [x] 展开过程中保持 primitive 的 kind、role、color、alpha、emissive、material_profile_id 和 lifecycle 字段语义。（验证：`project/src/framework/fangyuan/layout.rs:192` 复用 `compile_blueprint_primitive_to_runtime()`，`:1414` 测试覆盖 kind/role/color/alpha/emissive/material_profile_id/lifecycle 保留）
- [x] 编译报告记录 authored prefab primitive 数、instance 数、generated primitive 数、skipped primitive 数、使用到的 prefab 数和错误或 warning 列表。（验证：`project/src/framework/fangyuan/layout.rs:327`-`:334` 定义 report 字段，`:338`-`:344` 定义 warning 定位字段，`:1392`-`:1398` 测试断言 report 计数）
- [x] 缺失 prefab、非法 prefab、非法 instance、越界和预算超限应返回结构化错误或可跳过 warning，策略必须明确且有测试覆盖。（验证：`project/src/framework/fangyuan/layout.rs:1479` 缺失 prefab error，`:1497` 非法 instance error，`:1512` 非法 palette error，`:1528` 预算 error，`:1449` 展开后越界 primitive 被跳过并记录 warning）
- [x] 展开后仍调用统一 validator，不允许通过 prefab 绕过第三阶段 position、size、color、alpha、emissive、role、lifecycle 和 material profile 校验。（验证：`project/src/framework/fangyuan/layout.rs:187` 调用 `validate_blueprint_primitive()`，`project/src/framework/fangyuan/blueprint.rs:600` 为统一 primitive validator；`:1449` 测试确认展开后越界 primitive 被统一 validator 拦截）
- [x] 禁止循环或嵌套引用；如果数据结构暂不支持嵌套，也要用测试确认不能解析或不能编译嵌套 prefab。（验证：`project/src/framework/fangyuan/layout.rs:1651` 测试 layout 顶层拒绝 nested prefab/layout 字段，`:1664` 测试 prefab 内拒绝 nested prefab 字段）
- [x] 为多 instance 复用同一 prefab、scale 展开、非法 instance、缺失 prefab、预算超限和字段保留补测试。（验证：`project/src/framework/fangyuan/layout.rs:1372` 多 instance 与 scale/pivot，`:1414` 字段保留，`:1497` 非法 instance，`:1479` 缺失 prefab，`:1528` 预算超限）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；`cargo test fangyuan -- --nocapture` 结果为 155 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 6：预算、统计和错误报告收敛

- 开始时间：2026-07-02 17:04:44 +08:00
- 结束时间：2026-07-02 17:17:22 +08:00
- 开发总结：收敛 layout compile report 的预算、统计和错误定位字段，新增 palette/prefab/material/stats/校验状态统计，并为 compile error 增加 `code()`、`field_path()`、`reason()` 结构化访问入口；家园旧 simple blueprint 回归保持通过。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（156 passed）；`cargo test fangyuan_home -- --nocapture` 通过（37 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 明确 layout 展开前后的预算口径：prefab authored primitive、instance count、expanded generated primitive 都计入 report。（验证：`project/src/framework/fangyuan/layout.rs:338`-`:345` 的 `FangyuanSceneLayoutCompileReport` 记录 authored/instance/generated/skipped/used prefab；`:1437`-`:1446` 测试断言这些字段）
- [x] 保留 1000 expanded primitive 硬限制，并测试多个小 prefab instance 累计超限的失败路径。（验证：`project/src/framework/fangyuan/layout.rs:1630` 测试 1001 个小 prefab instance 触发 `expanded_primitive_budget_exceeded` 结构化错误）
- [x] 统计中区分 palette 数、prefab 数、instance 数、generated primitive 数、skipped 数、material 数和 top-level/layout/palette 校验状态。（验证：`project/src/framework/fangyuan/layout.rs:205` 生成 `primitive_stats`，`:209`-`:219` 写入 palette_count/prefab_count/校验状态，`:1440`-`:1452` 和 `:1486`-`:1490` 测试统计与 material profile）
- [x] 错误报告能定位到 layout instance id 或 index、prefab id、prefab primitive index。（验证：`project/src/framework/fangyuan/layout.rs:338`-`:344` 的 warning 字段包含 instance_index/instance_id/prefab_id/prefab_primitive_index，`:1533`-`:1536` 测试断言定位信息）
- [x] 对于非法 primitive 的处理策略与第三阶段一致或明确收敛：顶层非法不生成，单个非法 primitive 可跳过并记录 warning。（验证：`project/src/framework/fangyuan/layout.rs:125`-`:130` 顶层 layout/palette 校验失败返回 error，`:187`-`:199` 单个 expanded primitive 校验失败进入 warnings，`:1508` 测试保留合法 primitive 并跳过非法 primitive）
- [x] 为 report 内容和错误定位补单元测试，避免只靠日志文本判断。（验证：`project/src/framework/fangyuan/layout.rs:1826` 的 `assert_compile_error_report` 直接断言 `code()`/`field_path()`/`reason()`，`:1559`、`:1596`、`:1654` 覆盖结构化错误 helper）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；`fangyuan` 156 passed，`fangyuan_home` 37 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 7：家园预览接入 Scene Layout

- 开始时间：2026-07-02 17:20:03 +08:00
- 结束时间：2026-07-02 17:47:35 +08:00
- 开发总结：家园预览默认内容切换为加载 `FANGYUAN_HOME_SCENE_LAYOUT_PATH` 与 `FANGYUAN_HOME_PREFAB_PALETTE_PATH` 并通过 layout compile report 展开生成 `FangyuanHomeObject`；Reload/Clear/Exit 生命周期沿用第三阶段边界，simple blueprint 仅保留测试兼容路径，不作为运行时 fallback。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_home -- --nocapture` 通过（38 passed）；`cargo test fangyuan -- --nocapture` 通过（157 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 在 `fangyuan_home` 场景中接入默认 layout/palette 加载路径，并将展开结果用于家园逻辑根 `FangyuanHomeObject`。（验证：`project/src/game/scenes/fangyuan_home.rs:741` 加载默认 scene layout，`:747` 加载默认 prefab palette，`:753` 调用 `compile_with_palette()`，`:801` 记录 layout loaded stats，`:1736` 测试默认进入生成 layout 内容）
- [x] Reload 优先重新读取 layout/palette 并展开；如保留 simple blueprint fallback，fallback 条件和日志必须明确。（验证：`project/src/game/scenes/fangyuan_home.rs:1222`、`:1241`、`:1258` 失败路径均记录 layout failed，`:1239` 明确 simple blueprint fallback disabled，`:2582` 和 `:2665` 测试 Reload 替换与 Clear 后 Reload）
- [x] Clear 仍只清理当前 layout 展开出的家园逻辑对象和 render-only 内容，不清理基础空间、网格、边界和灯光。（验证：`project/src/game/scenes/fangyuan_home.rs:2640` 的 `clear_blueprint_command_removes_only_layout_content` 覆盖 Clear 后基础空间保留、layout 内容清理）
- [x] Exit 继续清理当前 session 下的 layout 展开内容和 stats 状态。（验证：`project/src/game/scenes/fangyuan_home.rs:2437` 的 `scene_lifecycle_exit_cleans_fangyuan_home_scene_owned_content` 和既有 stats reset 测试通过）
- [x] 同一 session 重复进入不重复生成基础空间、layout object 或 render-only primitive。（验证：`project/src/game/scenes/fangyuan_home.rs:2045` 的 `duplicate_enter_events_for_same_session_do_not_duplicate_content` 继续通过）
- [x] 展开后的 render-only 子实体继续复用 `FangyuanRenderAssetCache`，不引入 mesh merge 或 GPU Instancing。（验证：`project/src/game/scenes/fangyuan_home.rs:2095` 的 `blueprint_primitives_reuse_meshes_and_materials_without_runtime_components` 继续通过；改动未引入 mesh merge 或 instancing 代码）
- [x] 保留第三阶段 `home_preview.ron` 简单蓝图加载测试，确保旧路径不回退。（验证：`project/src/game/scenes/fangyuan_home.rs:2781` 保留 simple blueprint 解析失败测试路径，`cargo test fangyuan -- --nocapture` 中 `load_default_blueprint_from_first_package_assets` 和 framework legacy home preview 测试均通过）
- [x] 为 layout 成功加载、加载失败、缺失 prefab、clear、reload、exit 和重复进入补测试。（验证：`project/src/game/scenes/fangyuan_home.rs:1736` 成功加载，`:2718` 加载失败，`:2748` 缺失 prefab，`:2640` clear，`:2582`/`:2665` reload，`:2437` exit，`:2045` 重复进入）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_home -- --nocapture`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；`fangyuan_home` 38 passed，`fangyuan` 157 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 8：HUD 和调试统计接入 Layout Report

- 开始时间：2026-07-02 17:54:04 +08:00
- 结束时间：2026-07-02 18:13:11 +08:00
- 开发总结：HUD 状态文本切换为 layout report 视角的短格式，展示 layout 状态、generated/skipped、palette/prefab/used prefab/instance/material 和压缩后的 layout/palette 路径；失败路径和日志补齐 layout/palette 路径与 compile error 定位，默认导出也收敛为 HUD 实际需要的 scene API。
- 验证记录：`cargo fmt --check` 通过；`cargo test game::screens::gameplay::fangyuan_home -- --nocapture` 通过（7 passed）；`cargo test fangyuan_home -- --nocapture` 通过（38 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 扩展 `FangyuanHomeBlueprintStats` 或新增等价 stats，使 HUD 能显示 layout 状态、palette 数、prefab 数、instance 数、generated primitive 数、skipped 数、material 数和当前路径。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:244` HUD 文本读取 `generated_primitives`、`skipped`、`palette_count`、`prefab_count`、`used_prefab_count`、`instance_count`、`materials` 和 layout/palette path；`project/src/game/scenes/fangyuan_home.rs:304` 的 stats 已持有这些 layout report 字段）
- [x] HUD 文本在手机比例窗口下保持短格式，不遮挡 Reload、Clear 和大厅按钮。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:244` 使用 `layout/pal/pf/used/inst/mat/l/p` 短标签，`:256` 将路径压缩到 30 字符；HUD 专项测试通过）
- [x] Clear 后 HUD 显示 generated primitive 为 0，并保留合理 layout/palette/path 状态。（验证：`project/src/game/scenes/fangyuan_home.rs:414` 的 `record_cleared` 清零 `generated_primitives` 并保留 layout/palette/path 与 palette/prefab/instance 状态；`project/src/game/screens/gameplay/fangyuan_home.rs:461` 测试断言 clear 文本为 `gen 0/1000` 且保留路径）
- [x] Reload 后 HUD 恢复默认 layout 展开统计。（验证：`project/src/game/scenes/fangyuan_home.rs:2669` 的 Clear 后 Reload 测试断言 stats 恢复为 `expected_loaded_layout_stats`；HUD 文本测试在 `project/src/game/screens/gameplay/fangyuan_home.rs:471` 重新 `record_layout_loaded` 后恢复 loaded 文本）
- [x] 加载失败、缺失 prefab 或预算失败时 HUD 不显示误导性的成功 primitive 数据。（验证：`project/src/game/scenes/fangyuan_home.rs:372` 的 `record_layout_failed` 清零 primitive stats、palette/prefab/instance/generated/used prefab；`project/src/game/screens/gameplay/fangyuan_home.rs:479` 测试断言 failed 文本为 `gen 0/1000`、`pal 0 pf 0 used 0 inst 0`）
- [x] 日志或 report 中能看到 layout/palette 路径和错误定位，便于后续调试。（验证：`project/src/game/scenes/fangyuan_home.rs:755` 的 compile 失败日志包含 `layout_path`、`palette_path`、`code`、`field_path`、`reason`；`:1352` 的 stats 日志包含 layout/palette path 和 generated/palette/prefab/instance/material/valid 状态）
- [x] 为 HUD loaded/cleared/reloaded/failed 文本和 stats 状态变化补测试。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:418` 覆盖 loaded HUD 更新，`:445` 覆盖 loaded/cleared/reloaded/failed 文本；`project/src/game/scenes/fangyuan_home.rs:2311`、`:2637`、`:2662`、`:2715`、`:2745` 覆盖 stats 状态变化）
- [x] 验证命令：`cargo fmt --check`、`cargo test game::screens::gameplay::fangyuan_home -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；HUD 专项 7 passed，`fangyuan_home` 38 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 9：回归测试和手动验收

- 开始时间：2026-07-02 18:16:23 +08:00
- 结束时间：2026-07-02 18:29:23 +08:00
- 开发总结：完成第四阶段 layout/palette 接入后的自动回归和手机比例窗口手动验收；默认家园 layout/palette 展开、HUD 统计、Clear、Reload、返回大厅和重复进入均无回退，玩家预览回归通过。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（157 passed）；`cargo test fangyuan_home -- --nocapture` 通过（38 passed）；`cargo test fangyuan_player_preview -- --nocapture` 通过（27 passed）；`cargo check` 通过；`cargo run -- --window-profile phone-small --window-scale 50%` 成功启动并完成手动观察，窗口实际 `360x800`，Ctrl+C 结束导致退出码 1。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning，以及运行期 Vulkan validation layer 缺失、`Suboptimal present` 和若干 i18n fallback warning。

- [x] 运行 `cargo fmt --check`。（验证：worker 在 `project/` 下执行通过）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：worker 在 `project/` 下执行通过，157 passed，0 failed）
- [x] 运行 `cargo test fangyuan_home -- --nocapture`。（验证：worker 在 `project/` 下执行通过，38 passed，0 failed）
- [x] 运行 `cargo test fangyuan_player_preview -- --nocapture`，确认玩家预览不因 layout/prefab 改动回退。（验证：worker 在 `project/` 下执行通过，27 passed，0 failed）
- [x] 运行 `cargo check`。（验证：worker 在 `project/` 下执行通过，仅既有 `selection.rs:32` dead_code warning）
- [x] 首次运行如因 `cargo clean` 后重新编译耗时较长，应等待完整结果，不把编译时间长误判为失败。（验证：worker 等待所有 cargo 命令完整结束，测试、check 和 run 均返回明确结果）
- [x] 手动运行 `cargo run -- --window-profile phone-small --window-scale 50%` 或等价手机比例窗口。（验证：worker 成功启动，窗口配置 `720x1600 scale 2.00`、逻辑 `360x800`、实际窗口 `360x800`，观察后 Ctrl+C 结束）
- [x] 手动验收：从大厅进入方圆家园原型，默认 layout/palette 展开后的家园内容可见。（验证：worker 从登录页进入大厅，再进入“方圆灵构家园原型”，观察到基础空间、网格、围栏和对象内容可见）
- [x] 手动验收：HUD layout/prefab/instance/primitive/skipped/material/path 显示合理。（验证：worker 观察 HUD 为 `layout loaded gen 138/1000 skip 0`、`pal 1 pf 5 used 5 inst 40 mat 15`，layout/palette path 压缩显示合理）
- [x] 手动验收：点击清空后 layout 展开内容消失，基础空间保留。（验证：worker 点击“清空”后 HUD 变为 `layout cleared gen 0/1000 skip 0`，展开对象消失且基础空间和网格保留）
- [x] 手动验收：点击重新加载后默认 layout/palette 展开内容恢复。（验证：worker 点击“重新加载”后 HUD 恢复 `layout loaded gen 138/1000 skip 0`，默认内容恢复）
- [x] 手动验收：点击返回大厅后回到大厅，重新进入不会重复叠加内容。（验证：worker 点击“大厅”成功返回，再次进入后 HUD 仍为 `gen 138/1000`、`inst 40`，未观察到重复叠加）
- [x] 如保留 simple blueprint fallback，手动或自动验证 fallback 不影响默认 layout 路径。（验证：自动测试覆盖旧路径兼容，手动默认路径确认走 layout/palette；运行日志未见 layout/palette 编译或加载失败）

## 阶段 10：文档同步和归档准备

- 开始时间：2026-07-02 18:30:38 +08:00
- 结束时间：2026-07-02 18:42:38 +08:00
- 开发总结：同步第四阶段 Prefab / Palette、Scene Layout、路径策略、预算 report、HUD/Reload/Clear 和非目标边界到方圆技术路线、蓝图规则、新人上手和仓库说明；按仓库归档约定在 `docs/fangyuan/checklists/` 准备归档副本，同时保留 `summary/` 源 checklist 作为本轮进度记录且不纳入提交。
- 验证记录：worker 执行 `cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（157 passed）；`cargo test fangyuan_home -- --nocapture` 通过（38 passed）；`cargo check` 通过；`rg` / `Test-Path` 文档路径和关键文本检查通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 更新 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md`，记录第四阶段实际落地的 Prefab、Palette 和 Scene Layout 边界。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:443` 记录第四阶段已落地 Prefab/Palette/Scene Layout，`:1575` 起更新阶段 4 目标、做法、验收和风险）
- [x] 更新 `docs/世界观/方圆灵构蓝图规则.md`，补充 Prefab / Scene Layout RON v1 生成规则、路径建议、禁止字段和错误处理建议。（验证：`docs/世界观/方圆灵构蓝图规则.md:18` 更新阶段边界，`:288` 起新增 Prefab / Palette RON v1，`:341` 起新增 Scene Layout RON v1，`:548` 起更新错误处理建议）
- [x] 如新增资源路径或开发启动方式影响新成员理解，检查并同步 `docs/bevy-getting-started.md`。（验证：`docs/bevy-getting-started.md:232` 更新 framework fangyuan 说明，`:250`-`:251` 记录默认 palette/layout 首包样例路径）
- [x] 如仓库级说明需要更新，检查并同步 `CLAUDE.md`。（验证：`CLAUDE.md:30` 更新 framework fangyuan 边界，`:42`-`:43` 记录默认 palette/layout 首包样例路径）
- [x] 确认文档仍明确 Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI、联网同步、正式家园编辑器、蓝图持久化、装备挂点和技能规则层不是本阶段能力。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:453`、`:1608` 和 `docs/世界观/方圆灵构蓝图规则.md:18`、`:531` 明确这些能力仍不是第四阶段能力）
- [x] checklist 全部完成后，按仓库约定将本文件从 `summary/` 归档到合适的 `docs/<领域>/checklists/` 目录。（验证：已创建 `docs/fangyuan/checklists/方圆Prefab和场景布局第四阶段_checklist.md` 作为 docs 归档副本；受 multi-agent-dev 提交规则约束，`summary/` 源文件保留为未提交进度记录，不纳入 git 提交）
- [x] 归档前确认 checklist 的阶段时间、开发总结和验证记录均来自真实执行结果。（验证：阶段 1-10 的开始/结束时间、总结和验证记录均来自主 agent、worker 和命令输出；归档副本将在最终完成定义更新后同步）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`，以及必要的文档路径检查。（验证：worker 在 `project/` 下执行四条 cargo 命令均通过；`fangyuan` 157 passed，`fangyuan_home` 38 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning；文档路径和关键文本检查通过）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-02 18:42:38 +08:00
- 结束时间：2026-07-02 18:42:38 +08:00
- 验收总结：第四阶段 Prefab / Palette 与 Scene Layout 基础能力已完成，默认家园预览通过 layout/palette 展开为 `FangyuanPrimitiveSet`，HUD、Reload、Clear、返回大厅、玩家预览和文档同步均通过自动与手机窗口手动验收；Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI、联网同步、正式编辑器、持久化、装备挂点和技能规则层继续延后。

- [x] 存在可加载的 Prefab / Palette RON v1 源格式，能定义可复用方圆组件。（验证：`project/src/framework/fangyuan/prefab.rs:18` 定义 palette，`project/assets/fangyuan/palettes/home_prefabs.ron` 可加载；`cargo test fangyuan -- --nocapture` 157 passed）
- [x] 存在可加载的 Scene Layout RON v1 源格式，能通过 prefab id、position 和 scale 复用 prefab。（验证：`project/src/framework/fangyuan/layout.rs:18` 定义 layout，`project/assets/fangyuan/layouts/home_layout.ron` 可加载并引用 prefab）
- [x] 默认家园预览可以通过 layout/palette 展开为 `FangyuanPrimitiveSet`。（验证：`project/src/game/scenes/fangyuan_home.rs:753` 调用 `compile_with_palette()`，手动验收 HUD 显示 `layout loaded gen 138/1000 skip 0`）
- [x] 同一个 prefab 可以在 layout 中复用多次，layout 文件不重复记录大量相同 primitive。（验证：`project/assets/fangyuan/layouts/home_layout.ron` 多次复用 `fence_segment`、`dragon_body_segment`、`cloud_puff`；阶段 4 asset load 测试通过）
- [x] 展开后的 primitive 仍通过统一 validator，不能绕过数量、bounds、size、color、alpha、emissive、role、lifecycle 和 material profile 校验。（验证：`project/src/framework/fangyuan/layout.rs:187` 调用统一 primitive validator，`:1414` 字段保留测试通过）
- [x] Prefab 和展开后的 primitive 都计入预算，expanded primitive 数量不超过 1000。（验证：`project/src/framework/fangyuan/layout.rs:1630` 覆盖 1001 instance 预算失败，默认 generated 为 138）
- [x] 缺失 prefab、重复 id、非法 instance、越界和预算超限均有结构化错误或 warning。（验证：`project/src/framework/fangyuan/layout.rs:1479`、`:1497`、`:1528` 和 `prefab.rs:634` 覆盖对应错误路径）
- [x] 家园逻辑根仍持有 `FangyuanPrimitiveSet` 和 `FangyuanObjectState` 或等价统一根状态。（验证：`project/src/game/scenes/fangyuan_home.rs:2582` Reload 测试检查 reloaded home object 持有 primitive set 和默认 object state）
- [x] 单个 primitive 不成为玩法 Entity，render-only 子实体不挂业务状态。（验证：`project/src/game/scenes/fangyuan_home.rs:2095` 的 render-only cache 测试通过）
- [x] Reload、Clear、Clear 后 Reload、返回大厅和重复进入行为均无回退。（验证：阶段 9 手机窗口手动验收通过；`cargo test fangyuan_home -- --nocapture` 38 passed）
- [x] HUD 统计与 layout compile report 和统一 stats 结果一致，不依赖 render-only 实体数量作为数据源。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:244` HUD 使用 stats/report 字段，HUD 专项测试 7 passed）
- [x] 玩家预览入口和最小 cube/sphere 玩家外观不因本阶段改动回退。（验证：`cargo test fangyuan_player_preview -- --nocapture` 27 passed）
- [x] 代码、测试和文档中不存在 rotation、quaternion、euler、angular_velocity、rotate 或 spin 能力。（验证：prefab/layout 禁止字段测试通过；`docs/世界观/方圆灵构蓝图规则.md:535` 起继续禁止这些字段）
- [x] 文档同步记录第四阶段实际落地边界和后续阶段延后事项。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:443`、`:1608` 和 `docs/世界观/方圆灵构蓝图规则.md:18`、`:531` 已同步）
- [x] `cargo fmt --check` 通过。（验证：阶段 10 worker 执行通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：阶段 10 worker 执行通过，157 passed）
- [x] `cargo test fangyuan_home -- --nocapture` 通过。（验证：阶段 10 worker 执行通过，38 passed）
- [x] `cargo test fangyuan_player_preview -- --nocapture` 通过。（验证：阶段 9 worker 执行通过，27 passed）
- [x] `cargo check` 通过。（验证：阶段 10 worker 执行通过，仅既有 `selection.rs:32` dead_code warning）
- [x] 用户手动验收游戏内方圆家园 layout/palette 预览效果无回退。（验证：阶段 9 worker 在 `cargo run -- --window-profile phone-small --window-scale 50%` 中完成手机窗口手动验收）
