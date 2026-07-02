# 方圆蓝图审核和预算系统第五阶段 Checklist

## 目标

交付方圆系统第五阶段基础能力：在第四阶段已经落地开发期 Prefab / Palette 与 Scene Layout，并能展开为统一 `FangyuanPrimitiveSet` 后，把世界观限制、技术校验、预算统计和调试反馈收敛成可执行、可复用、可报告的蓝图审核系统。

本阶段重点是建立统一审核结果模型、预算 profile、错误与 warning 列表、降级建议、simple blueprint / prefab palette / scene layout 三类入口的审核适配，以及家园 HUD / 日志 / 测试中的审核信息展示。

本阶段不实现 Chunk、流式加载、发布期二进制 Bake、静态 CPU mesh merge、GPU Instancing、LOD、AOI、联网同步、正式家园编辑器、蓝图持久化、角色四属性正式额度、职业权限、装备挂点、技能规则层、AI 内容审核或玩家自定义 shader。

## 功能地图

| 功能域 | 第五阶段处理方式 |
| --- | --- |
| 审核入口 | 新增 framework 级统一审核 API，覆盖 blueprint、prefab palette 和 scene layout |
| 预算口径 | 抽象 primitive 数、体积、bounds、透明、发光、材质 profile、role 和 lifecycle 统计 |
| 预算 profile | 提供默认 profile，预留后续角色、四属性、职业、世界层级额度接入点 |
| 错误报告 | 统一 code、severity、field_path、reason、source、定位信息和可降级建议 |
| 降级建议 | 先返回建议，不自动改写源 RON 或运行时 primitive |
| 家园预览 | 默认 layout/palette 加载后记录审核状态，HUD / 日志显示简短审核摘要 |
| 兼容性 | 保留第四阶段 compile report 与现有 validator，不重复造一套 primitive 校验 |
| 非目标 | 不引入正式编辑器、持久化、联网同步、LOD、Bake、AOI 或自定义 shader |

## 基础原则

- [ ] 审核系统复用现有 `FangyuanBlueprint`、`FangyuanPrefabPalette`、`FangyuanSceneLayout`、`FangyuanPrimitiveSetStats` 和 compile report，不绕过现有 validator。
- [ ] 审核结果区分 error、warning 和 info；error 阻止生成非法内容，warning 可以生成但必须可见。
- [ ] 审核 report 必须可测试，不依赖日志文本作为唯一证据。
- [ ] 所有审核项必须有稳定 code 和 field_path，便于 HUD、日志、工具和未来编辑器复用。
- [ ] 预算 profile 先以默认本地配置为主，预留角色、四属性、职业和世界层级额度字段，但不接入正式玩法判定。
- [ ] 降级建议只作为 report 数据返回，本阶段不自动重写 RON、不生成替代蓝图、不做正式修复器。
- [ ] 审核不得引入 rotation、quaternion、euler、angular_velocity、rotate 或 spin 能力。
- [ ] 本阶段只做审核、预算、report 和调试接入，不提前实现 Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI 或联网同步。
- [ ] 每个阶段完成后运行对应验证，并按阶段提交。

## 阶段 1：需求和边界复核

- 开始时间：2026-07-02 19:06:48 +08:00
- 结束时间：2026-07-02 19:11:59 +08:00
- 开发总结：完成第五阶段需求和边界复核，确认本阶段应在现有 blueprint / prefab / layout validator、compile report、stats 和 asset path 基础上新增统一审核层；阶段 1 只读执行，未修改业务代码。
- 验证记录：worker 执行 `git status --short`、`git log --oneline -n 12`、`git show --stat --oneline --name-only`、`rg` 和 `Get-Content` 等只读检查；worker 结束前 `git status --short` 无输出，主 agent 复核后工作区仍干净。

- [x] 复核 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md` 中“审核和安全”“蓝图审核”“阶段 5：蓝图审核和预算系统”的目标、做法、验收和风险。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1341` 起定义审核和安全，`:1345`-`:1360` 列出审核重点，`:1610`-`:1628` 记录阶段 5 目标和做法，`:1630`-`:1632` 记录主要风险）
- [x] 复核 `docs/世界观/方圆灵构蓝图规则.md` 中现有数量、bounds、size、color、alpha、emissive、material profile、lifecycle、禁止字段和错误处理建议。（验证：`docs/世界观/方圆灵构蓝图规则.md:380`-`:400` 限定 kind 和禁止旋转字段，`:417`-`:425` 记录 bounds，`:440`-`:467` 记录 size/color/alpha，`:496`-`:503` 记录 1000 数量限制，`:533`-`:559` 记录禁止事项和错误处理建议）
- [x] 复核第四阶段 checklist 和提交记录，确认 blueprint、prefab、layout、compile report、HUD stats 和家园 Reload/Clear 已稳定。（验证：最近提交包含 `1d70554`、`1ee142a`、`bf4fce6`、`9d98c65`、`0128ad7`；`docs/fangyuan/checklists/方圆Prefab和场景布局第四阶段_checklist.md:172`-`:187` 记录阶段 9 自动和手动验收，`:211`-`:232` 记录最终完成定义）
- [x] 检查 `project/src/framework/fangyuan/` 中现有 validation、compile report、stats、asset path 和错误类型，明确哪些能力复用、哪些需要新增审核层。（验证：`project/src/framework/fangyuan/blueprint.rs:123`-`:159` 提供 simple compile report，`:600` 起提供 primitive validator；`layout.rs:122`-`:215` 提供 layout compile report；`stats.rs:14`-`:30` 提供 stats；`asset_path.rs:51`-`:76` 提供统一路径校验；worker 报告确认需新增统一 AuditReport/Finding/Suggestion、severity/status、budget profile 和三类 audit adapter）
- [x] 明确本阶段不处理正式编辑器、持久化、联网同步、四属性正式额度、职业权限、AI 内容审核、自定义 shader、Chunk、Bake、LOD 和 AOI。（验证：本 checklist 目标段和基础原则明确非目标，worker 报告确认阶段 2-8 只做审核、预算、report 和调试接入）
- [x] 验证命令：执行 `rg`、`Get-Content`、`git status --short` 等只读检查，确认阶段 1 不修改业务代码。（验证：worker 报告列出只读命令并确认未修改任何文件；主 agent 再次执行 `git status --short` 无输出）

## 阶段 2：统一审核数据模型

- 开始时间：2026-07-02 19:13:11 +08:00
- 结束时间：2026-07-02 19:26:39 +08:00
- 开发总结：新增 framework 级 `audit` 模块和统一审核数据模型，包含 report、summary、status、severity、finding、suggestion 和 suggestion action，并导出到方圆模块；本阶段仅提供可测试数据结构，不接入具体 blueprint/prefab/layout 审核入口。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_audit -- --nocapture` 通过（5 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 在 framework 方圆模块中设计统一审核 report 类型，例如 `FangyuanAuditReport`，包含 source_kind、source_path、status、summary、findings 和 suggestions。（验证：`project/src/framework/fangyuan/audit.rs:4` 定义 `FangyuanAuditReport`，字段包含 source_kind/source_path/status/summary/findings/suggestions；`project/src/framework/fangyuan/mod.rs:12`、`:23` 接入并导出 audit 模块）
- [x] 定义审核 finding 字段：severity、code、field_path、reason、source_kind、instance_id 或 primitive_index 等定位信息。（验证：`project/src/framework/fangyuan/audit.rs:119` 定义 `FangyuanAuditFinding`，包含 severity/code/field_path/reason/source_kind/source_path/primitive_index/prefab_id/instance_id/instance_index/prefab_primitive_index）
- [x] 定义 severity 等级，至少区分 error、warning、info，并提供稳定排序策略。（验证：`project/src/framework/fangyuan/audit.rs:101` 定义 Error/Warning/Info，`:145` 起为 finding 实现稳定 `Ord`，`:276` 测试排序）
- [x] 定义审核状态规则：存在 error 为 failed；无 error 但有 warning 为 passed_with_warnings；无 warning 为 passed。（验证：`project/src/framework/fangyuan/audit.rs:81` 定义状态，`:89` 的 `from_summary()` 实现规则，`:231`、`:245`、`:257` 覆盖 passed/warning/failed 测试）
- [x] 定义降级建议类型，例如 reduce_primitives、shrink_bounds、lower_emissive、remove_alpha、replace_material_profile，并保留可读 reason。（验证：`project/src/framework/fangyuan/audit.rs:188` 定义 `FangyuanAuditSuggestion`，`:213` 定义 ReducePrimitives/ShrinkBounds/LowerEmissive/RemoveAlpha/ReplaceMaterialProfile）
- [x] 为 report 汇总、finding 排序、状态判定和 suggestions 去重补单元测试。（验证：`project/src/framework/fangyuan/audit.rs:231` 起 5 个 `fangyuan_audit_*` 测试覆盖默认状态、warning/failed 状态、finding 排序和 suggestion 去重）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_audit -- --nocapture` 或等价精确测试、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；audit 测试 5 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 3：预算 Profile 和统计口径

- 开始时间：2026-07-02 19:28:59 +08:00
- 结束时间：2026-07-02 19:42:51 +08:00
- 开发总结：新增默认审核预算 profile、primitive budget stats 和 runtime primitive-set 预算评估入口，覆盖 primitive 数、bounds、单体/总体 volume、alpha、emissive、material profile、role 和 lifecycle 统计；推荐阈值生成 warning，硬限制生成 error，并返回对应降级建议。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_budget -- --nocapture` 通过（5 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 新增默认预算 profile，记录 hard primitive limit、recommended primitive limit、bounds、单 primitive size、总体积、透明数量、发光强度、material profile 数等限制。（验证：`project/src/framework/fangyuan/audit.rs:82` 定义 `FangyuanAuditBudgetProfile`，`:102` 默认值复用 1000 hard limit 并设置 recommended、bounds、primitive extent/volume、total volume、alpha/emissive/material profile 阈值）
- [x] 预算 profile 字段预留角色、四属性、职业和世界层级额度入口，但默认不依赖玩法数据。（验证：`project/src/framework/fangyuan/audit.rs:98`-`:101` 定义 role_budget、element/profession/world_layer tier，`:132` 和 `:139` 定义预留类型，默认均为本地 Default）
- [x] 基于 `FangyuanPrimitiveSetStats` 或等价统计扩展出审核需要的 volume、alpha、emissive、material_profile、role 和 lifecycle 指标。（验证：`project/src/framework/fangyuan/audit.rs:150` 定义 `FangyuanPrimitiveBudgetStats`，`:168` 起从 runtime primitives 统计 volume、bounds、alpha、emissive、material profile、role distribution 和 lifecycle count）
- [x] 明确 authored、generated、skipped、expanded 和 runtime primitive 的预算口径，避免 prefab 成为隐藏容器。（验证：`project/src/framework/fangyuan/audit.rs:144` 注释说明 authored/generated/skipped/expanded/runtime 口径，`:151`-`:155` 提供对应字段，`:225` 用 counted_primitives 汇总 runtime/expanded 预算面）
- [x] 对超出推荐阈值和硬限制分别产生 warning 或 error。（验证：`project/src/framework/fangyuan/audit.rs:398` 的 `add_count_budget_findings()` 对 hard limit 生成 Error、推荐阈值生成 Warning；`:285` 等 scalar hard limit 生成 Error）
- [x] 为默认预算、阈值边界、统计汇总、超限 finding 和建议生成补测试。（验证：`project/src/framework/fangyuan/audit.rs:784` 起覆盖默认 profile，`:813` 覆盖统计汇总，`:868` 覆盖推荐 warning 和建议，`:920` 覆盖 hard error 和建议，`:979` 覆盖空 primitive set pass）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_budget -- --nocapture` 或等价精确测试、`cargo check`。（验证：主 agent 在 `project/` 下执行三条命令均通过；预算测试 5 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 4：Simple Blueprint 审核接入

- 开始时间：2026-07-02 19:47:49 +08:00
- 结束时间：2026-07-02 20:18:49 +08:00
- 开发总结：为 simple `FangyuanBlueprint` 接入统一审核入口，复用现有 top-level/primitive validator 和 tolerant compile 语义；顶层非法直接 failed，单个非法 primitive 以 warning 进入 report 并跳过，同时合并 runtime primitive 预算审核和 summary 统计。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_blueprint_audit -- --nocapture` 通过（6 passed）；`cargo test fangyuan -- --nocapture` 通过（173 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 为 `FangyuanBlueprint` 增加审核入口，复用现有 top-level 和 primitive validator。（验证：`project/src/framework/fangyuan/blueprint.rs:147` 定义 `audit()`，`:150` 调用 `validate_top_level()`，`:165` 调用 `validate_blueprint_primitive()`，`:198` 提供默认预算入口）
- [x] 审核 simple blueprint 时报告 primitive 数量、bounds、size、color、alpha、emissive、material_profile_id、lifecycle 和 role 相关 finding。（验证：`project/src/framework/fangyuan/blueprint.rs:658` 起统一 primitive validator 覆盖 kind/position/size/below-ground/color/alpha/emissive/role/material/lifecycle；`:698` 起把 validation error 转为 audit finding；`project/src/framework/fangyuan/audit.rs:302` 起合并 primitive count、bounds、volume、alpha、emissive、material profile 预算 finding）
- [x] 非法顶层蓝图和非法 primitive 的审核行为与现有 compile 策略一致：顶层 error 阻止生成，单个非法 primitive 可在 tolerant 路径中报告并跳过。（验证：`project/src/framework/fangyuan/blueprint.rs:150`-`:158` 顶层错误加入 Error 并返回；`:165`-`:175` 单个 primitive 错误加入 Warning 并计入 skipped；`:1746` 和 `:1650` 测试分别覆盖顶层失败与非法 primitive 跳过）
- [x] 审核 report 能包含 authored、generated、skipped、cube、sphere、material、alpha、emissive 等 summary。（验证：`project/src/framework/fangyuan/audit.rs:51` 的 `apply_primitive_budget_stats()` 写入 authored/generated/skipped/cube/sphere/color/material/alpha/emissive/lifecycle/role；`:523` 起扩展 `FangyuanAuditSummary` 字段；`project/src/framework/fangyuan/blueprint.rs:1589` 起测试 summary）
- [x] 保留 `minimal_player.ron` 和 `home_preview.ron` 兼容路径，旧资源能通过默认审核或只产生预期 warning。（验证：`project/src/framework/fangyuan/blueprint.rs:1571` 测试 minimal player 默认审核 Passed；`:1613` 测试 legacy home preview 为 PassedWithWarnings，493 generated、12 skipped 且 warning code 为 `primitive_below_ground`）
- [x] 为合法 minimal player、legacy home preview、非法 primitive、超预算和禁止字段补测试。（验证：`project/src/framework/fangyuan/blueprint.rs:1571`、`:1613`、`:1650`、`:1768`、`:1806` 覆盖对应场景，`:1746` 额外覆盖非法顶层）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_blueprint_audit -- --nocapture` 或等价精确测试、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；blueprint audit 6 passed，fangyuan 173 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 5：Prefab / Palette 审核接入

- 开始时间：2026-07-02 20:22:43 +08:00
- 结束时间：2026-07-02 20:47:01 +08:00
- 开发总结：为 `FangyuanPrefabPalette` 接入统一审核入口，复用现有 palette validation helper、primitive validator 和 primitive budget audit；审核会收集 palette、prefab 和 primitive 多级 finding，并汇总 prefab 数、可复用 prefab 数、authored/generated/skipped、material、alpha 和 emissive 等统计。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_prefab_audit -- --nocapture` 通过（6 passed）；`cargo test fangyuan -- --nocapture` 通过（179 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 为 `FangyuanPrefabPalette` 增加审核入口，复用现有 palette validation 和 primitive validator。（验证：`project/src/framework/fangyuan/prefab.rs:154` 定义 `audit()`，`:201` 复用 `validate_prefab_id()`，`:250` 复用 `validate_prefab_primitive_budget()`，`:266` 复用 `validate_blueprint_primitive()`，`:321` 提供默认预算入口）
- [x] 审核 palette 顶层预算、prefab id、重复 id、prefab primitive 数、pivot、tags、bounds 和禁止字段。（验证：`project/src/framework/fangyuan/prefab.rs:162`、`:176`、`:185`、`:201`、`:215`、`:237`、`:241`、`:250`、`:286` 覆盖 version/bounds/max_primitives/id/duplicate/pivot/tags/prefab budget/total budget；`:1483` 测试禁止字段 parse）
- [x] report 区分 palette 总体 finding 和单个 prefab / primitive finding，field_path 能定位到 `prefabs[n]` 和 `primitives[n]`。（验证：`project/src/framework/fangyuan/prefab.rs:789` 起将 validation error 转为 audit finding 并填 `prefab_id`/`prefab_primitive_index`；`:827` 起将预算路径映射为 `prefabs[].primitives...`；`:1343` 测试 palette bounds、prefab pivot/tag/bounds 和 `prefabs[2].primitives[0].size[0]` 定位）
- [x] 对 prefab authored primitive 数、复用潜力、material profile 数和透明/发光风险生成 summary 或 warning。（验证：`project/src/framework/fangyuan/prefab.rs:158`-`:199` 统计 authored 和 reusable prefab，`:297`-`:316` 写入 stats/summary；`project/src/framework/fangyuan/audit.rs:527`-`:528` 扩展 prefab summary 字段；`project/src/framework/fangyuan/prefab.rs:1417` 测试 material/alpha/emissive warning 和 summary）
- [x] 默认 `home_prefabs.ron` 能通过审核，且 report 中 prefab 数、authored primitive 数和 material 数与现有资源一致。（验证：`project/src/framework/fangyuan/prefab.rs:1243` 测试默认 `FANGYUAN_HOME_PREFAB_PALETTE_PATH` 审核 Passed，prefab_count=5、authored_primitives=19、material_count=0）
- [x] 为非法 id、重复 id、prefab 超预算、非法 primitive、禁止字段和默认资源审核补测试。（验证：`project/src/framework/fangyuan/prefab.rs:1277` 覆盖非法/重复 id，`:1309` 覆盖 prefab/total budget，`:1343` 覆盖非法 primitive，`:1483` 覆盖禁止字段，`:1243` 覆盖默认资源）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_prefab_audit -- --nocapture` 或等价精确测试、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；prefab audit 6 passed，fangyuan 179 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 6：Scene Layout 审核接入

- 开始时间：2026-07-02 20:49:40 +08:00
- 结束时间：2026-07-02 21:34:50 +08:00
- 开发总结：为 `FangyuanSceneLayout` 接入统一审核入口，支持传入 palette 和预算 profile；审核复用 layout/palette validation、`compile_with_palette()` 的统计和 expanded primitive warning，并将 layout、palette、instance、budget 与 runtime primitive 预算问题统一映射到 audit report。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_layout_audit -- --nocapture` 通过（8 passed）；`cargo test fangyuan -- --nocapture` 通过（187 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 为 `FangyuanSceneLayout` 增加审核入口，支持传入 palette 和预算 profile。（验证：`project/src/framework/fangyuan/layout.rs:226` 定义 `audit(&self, palette, profile)`，`:291` 提供默认预算入口）
- [x] 审核 layout 顶层字段、palette 路径、instance id、prefab 引用、position、scale、tags、bounds 和 expanded primitive 预算。（验证：`project/src/framework/fangyuan/layout.rs:298` 起 `audit_layout_fields()` 覆盖 version/bounds/max_primitives/palette paths/instance prefab/id/tags/position/scale/expanded budget，`:760` 起转为 audit finding）
- [x] 审核结果能复用 `compile_with_palette()` 的 generated/skipped/used_prefab/material/valid 统计，但不因审核而生成额外 runtime 内容。（验证：`project/src/framework/fangyuan/layout.rs:237` 调用 `compile_with_palette()`；`:906` 起从 compile report 写入 generated/skipped/expanded，`:936` 起写入 palette/instance/used prefab/validated flags；审核仅返回 `FangyuanAuditReport`，不触碰 ECS spawn 路径）
- [x] 缺失 prefab、非法 instance、越界、预算超限和单个 expanded primitive warning 都能进入统一 audit report。（验证：`project/src/framework/fangyuan/layout.rs:2261` 缺失 prefab，`:2283` 非法 scale，`:2306` 越界 position，`:2326` expanded budget，`:2354` expanded primitive warning；`:856` 起将 compile warning 映射为 Warning finding）
- [x] 默认 `home_layout.ron` + `home_prefabs.ron` 审核通过，并能报告 generated 138、skipped 0、instance 40、prefab 5、used prefab 5。（验证：`project/src/framework/fangyuan/layout.rs:2236` 默认资源审核测试，`:2249`-`:2254` 断言 generated=138、skipped=0、instance=40、prefab=5、used_prefab=5、palette=1）
- [x] 为缺失 prefab、非法 scale、越界 position、expanded 超预算、禁止字段和默认资源审核补测试。（验证：`project/src/framework/fangyuan/layout.rs:2261`、`:2283`、`:2306`、`:2326`、`:2407`、`:2236` 分别覆盖对应场景；`:2388` 额外覆盖 palette validation error）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_layout_audit -- --nocapture` 或等价精确测试、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；layout audit 8 passed，fangyuan 187 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 7：家园预览接入审核结果

- 开始时间：2026-07-02 21:37:55 +08:00
- 结束时间：2026-07-02 22:08:23 +08:00
- 开发总结：家园默认 layout/palette 加载和 Reload 接入 layout audit，`FangyuanHomeBlueprintStats` 新增 audit 状态、error/warning 数和主要 finding 定位；HUD 增加短 audit 行，日志输出审核摘要，audit failed 时不生成成功内容且保留现有失败清理语义。
- 验证记录：`cargo fmt --check` 通过；`cargo test game::screens::gameplay::fangyuan_home -- --nocapture` 通过（7 passed）；`cargo test fangyuan_home -- --nocapture` 通过（40 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 家园默认 layout/palette 加载和 Reload 时执行审核，并把审核状态写入 `FangyuanHomeBlueprintStats` 或等价 stats。（验证：`project/src/game/scenes/fangyuan_home.rs:914` 默认加载调用 `audit_with_default_budget()`；`:998` 的 `record_layout_loaded()` 写入 audit report；`:1406`-`:1465` Reload 路径复用加载函数；`:326`-`:331` 和 `:589`-`:599` 保存 audit 状态、计数和主要 finding）
- [x] HUD 保持短格式，能显示 audit passed / warning / failed、error 数、warning 数和主要 code，不遮挡 Reload、Clear 和大厅按钮。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:241` 起 HUD 文本新增 `audit {} e{} w{} {}` 短行；`:423`、`:452`、`:507` 测试 passed/warning/failed/pending 文本）
- [x] 日志输出包含 audit status、主要 finding code、field_path、reason、layout_path 和 palette_path。（验证：`project/src/game/scenes/fangyuan_home.rs:1542` 起 layout stats 日志包含 audit_status/code/field_path/reason；`:1573` 起 audit result 日志包含 status/errors/warnings/code/field_path/reason/layout_path/palette_path）
- [x] 审核 failed 时不生成误导性的成功 primitive 数据；如果 compile 也失败，应保持现有失败清理语义。（验证：`project/src/game/scenes/fangyuan_home.rs:916` audit failed 返回失败 load result，`:969`-`:982` 失败时记录 failed 且不 spawn；`:3108` 测试 audit failed 不生成 content/object/primitive 且 generated=0；`:2982` reload load failure 仍清理旧内容并保留 base space）
- [x] Clear 后保留合理审核路径和状态；Reload 后恢复默认审核状态。（验证：`project/src/game/scenes/fangyuan_home.rs:493`-`:532` `record_cleared()` 保留 audit/path 相关字段；`:2889` Clear 测试保留 passed audit；`:2923` Reload 测试恢复默认 passed audit）
- [x] 为家园 loaded、warning、failed、clear、reload 和 missing prefab 审核状态补测试。（验证：`project/src/game/scenes/fangyuan_home.rs:2533` loaded，`:3058` warning，`:3108` failed，`:2889` clear，`:2923` reload，`:3013` missing prefab；HUD 测试见 `project/src/game/screens/gameplay/fangyuan_home.rs:423`、`:452`、`:507`）
- [x] 验证命令：`cargo fmt --check`、`cargo test game::screens::gameplay::fangyuan_home -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；HUD 7 passed，fangyuan_home 40 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 8：降级建议和调试输出

- 开始时间：2026-07-02 22:11:39 +08:00
- 结束时间：2026-07-02 22:35:07 +08:00
- 开发总结：为统一审核建议补充稳定 `estimated_effect`、去重合并和排序能力，预留 warning role 降级 action；新增 audit debug formatter 输出 summary、前 N 条 finding/suggestion 和 omitted 计数，并让家园审核日志使用该截断摘要。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_audit -- --nocapture` 通过（13 passed）；`cargo test fangyuan_home -- --nocapture` 通过（40 passed）；`cargo check` 通过。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 为常见审核失败生成稳定降级建议：减少 primitive、缩小 size、收缩 bounds、降低 alpha、降低 emissive、替换 material profile、移除 warning role 过量内容。（验证：`project/src/framework/fangyuan/audit.rs:535` 为预算 suggestion 填默认 effect；`:854` 预留 `ReduceWarningRole` action；`:858` 起为 ReducePrimitives/ShrinkBounds/RemoveAlpha/LowerEmissive/ReplaceMaterialProfile/ReduceWarningRole 定义稳定 effect；`:1073` 测试全部 action 映射）
- [x] 降级建议包含 action、field_path、reason、estimated_effect 或等价可测试字段。（验证：`project/src/framework/fangyuan/audit.rs:817` 起 suggestion builder 支持 `new_with_effect()` 和 `with_default_estimated_effect()`；`:1246` 与 `:1305` 测试预算建议均带 estimated_effect）
- [x] 降级建议不自动改写 RON、不直接修改 runtime primitive，只作为 report 数据和日志/HUD 摘要。（验证：本阶段改动集中在 `project/src/framework/fangyuan/audit.rs` suggestion/report formatter 和 `project/src/game/scenes/fangyuan_home.rs:1576` 日志消费；未新增源 RON 写入或 runtime primitive 修改路径）
- [x] 调试日志中能输出 summary 和前 N 条 finding / suggestion，避免长日志刷屏。（验证：`project/src/framework/fangyuan/audit.rs:539` 定义 `format_fangyuan_audit_debug_lines()`，`:562` 输出 summary，`:589` 和 `:618` 输出并截断 finding/suggestion；`project/src/game/scenes/fangyuan_home.rs:309`-`:310` 限制前 4 条，`:1576` 起日志使用该 formatter）
- [x] 为建议去重、排序、截断、常见失败映射和空建议路径补测试。（验证：`project/src/framework/fangyuan/audit.rs:981` 去重，`:1013` 排序，`:1110` 截断，`:1073` 常见映射，`:1144` 空 field path）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_audit -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 下执行四条命令均通过；audit 13 passed，fangyuan_home 40 passed，`cargo check` 仅输出既有 `checkbox` dead_code warning）

## 阶段 9：回归测试和手动验收

- 开始时间：2026-07-02 22:37:53 +08:00
- 结束时间：2026-07-02 23:03:12 +08:00
- 开发总结：完成第五阶段回归测试和手机比例窗口验收。默认 Vulkan 在当前桌面会话中首帧截图为空白，改用 `WGPU_BACKEND=dx12` 后可见方圆家园默认 layout/palette 内容和审核 HUD；GUI 自动鼠标注入未能可靠触发按钮点击，因此 Clear / Reload / Lobby 行为以已通过的 ECS/UI 单元测试作为验证证据。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（194 passed）；`cargo test fangyuan_home -- --nocapture` 通过（40 passed）；`cargo test fangyuan_player_preview -- --nocapture` 通过（27 passed）；`cargo check` 通过。`cargo run -- --window-profile phone-small --window-scale 50%` 启动成功并打印 `device 720x1600 scale 2.00, logical 360.0x800.0, preview 0.50, physical window 360x800`；`TOUCH_START_SCREEN=fangyuan_home` + `MYBEVY_START_SCENE=dev.fangyuan_home` + `WGPU_BACKEND=dx12` 截图 `project/target/fangyuan_stage9_dx12_start_scene_screen.png` 显示默认家园内容、HUD `layout loaded gen 138/1000 skip 0`、`audit passed e0 w0 -`、`pal 1 pf 5 used 5 inst 40 mat 15` 和 layout/palette path。仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。

- [x] 运行 `cargo fmt --check`。（验证：主 agent 在 `project/` 下执行通过）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：主 agent 在 `project/` 下执行通过，194 passed，仅既有 `checkbox` dead_code warning）
- [x] 运行 `cargo test fangyuan_home -- --nocapture`。（验证：主 agent 在 `project/` 下执行通过，40 passed，仅既有 `checkbox` dead_code warning）
- [x] 运行 `cargo test fangyuan_player_preview -- --nocapture`，确认玩家预览不因审核系统回退。（验证：主 agent 在 `project/` 下执行通过，27 passed，仅既有 `checkbox` dead_code warning）
- [x] 运行 `cargo check`。（验证：主 agent 在 `project/` 下执行通过，仅既有 `selection.rs:32` warning）
- [x] 手动运行 `cargo run -- --window-profile phone-small --window-scale 50%` 或等价手机比例窗口。（验证：普通启动与 DX12 完整场景启动均成功，窗口配置输出为 `device 720x1600 scale 2.00, logical 360.0x800.0, preview 0.50, physical window 360x800`；进程已停止）
- [x] 手动验收：从大厅进入方圆家园原型，默认 layout/palette 展开内容可见。（验证：等价完整场景入口 `TOUCH_START_SCREEN=fangyuan_home` + `MYBEVY_START_SCENE=dev.fangyuan_home` + `WGPU_BACKEND=dx12` 截图 `project/target/fangyuan_stage9_dx12_start_scene_screen.png` 可见家园网格、边界、primitive 内容和 HUD；大厅进入路由由 `cargo test fangyuan_home` 中 `fangyuan_home_entered_routes_to_fangyuan_home_hud` 覆盖）
- [x] 手动验收：HUD 审核状态、layout/prefab/instance/generated/skipped/material/path 显示合理。（验证：DX12 截图显示 `layout loaded gen 138/1000 skip 0`、`audit passed e0 w0 -`、`pal 1 pf 5 used 5 inst 40 mat 15`、layout/palette path；`game::screens::gameplay::fangyuan_home` HUD 测试也覆盖 passed/warning/failed/pending 文本）
- [x] 手动验收：点击清空后展开内容消失，基础空间保留，审核状态不误报成功内容。（验证：GUI 自动鼠标注入在当前会话未可靠触发按钮；行为由 `cargo test fangyuan_home -- --nocapture` 覆盖，`clear_blueprint_command_removes_only_layout_content` 和 `clearing_blueprint_content_does_not_remove_base_space` 断言 blueprint 内容清空、base space 保留、stats 为 cleared 且 audit 路径保留）
- [x] 手动验收：点击重新加载后默认 layout/palette 和审核状态恢复。（验证：GUI 自动鼠标注入在当前会话未可靠触发按钮；行为由 `cargo test fangyuan_home -- --nocapture` 覆盖，`reload_layout_command_regenerates_preview_after_clear` 和 `reload_layout_command_replaces_content_without_duplicate_primitives` 断言 Reload 恢复默认 layout/palette、audit passed、generated=138 且不重复叠加）
- [x] 手动验收：点击返回大厅后回到大厅，重新进入不会重复叠加内容。（验证：GUI 自动鼠标注入在当前会话未可靠触发按钮；行为由 `cargo test fangyuan_home -- --nocapture` 覆盖 `hud_buttons_write_reload_clear_and_lobby_exit_route`、`fangyuan_home_exit_fallback_only_routes_while_hud_is_active`、`duplicate_enter_events_for_same_session_do_not_duplicate_content`，`cargo test fangyuan -- --nocapture` 覆盖场景退出清理）

## 阶段 10：文档同步和归档准备

- 开始时间：2026-07-02 23:05:46 +08:00
- 结束时间：2026-07-02 23:18:53 +08:00
- 开发总结：完成第五阶段文档同步和归档准备。方圆技术路线、世界观蓝图规则、Bevy 入门文档和仓库说明均已补充审核入口、预算 profile、finding、suggestion、家园 HUD/日志、DX12 可视验收入口和第五阶段非目标边界；归档前复核阶段时间、验证记录和最终完成定义均来自本轮真实执行结果。
- 验证记录：worker 执行 `git diff --check` 通过；主 agent 复核 diff 后再次执行 `git diff --check` 通过，仅有 Git LF/CRLF 提示；`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（194 passed）；`cargo test fangyuan_home -- --nocapture` 通过（40 passed）；`cargo test fangyuan_player_preview -- --nocapture` 通过（27 passed）；`cargo check` 通过，仅保留既有 `selection.rs:32` 的 `checkbox` dead_code warning。`rg` 复核旋转和后续能力关键词，确认方圆文档/测试中均为禁止、拒绝或后续能力说明，业务代码未新增方圆旋转能力。

- [x] 更新 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md`，记录第五阶段实际落地的审核入口、预算 profile、report、finding 和 suggestion 边界。（验证：文档新增第五阶段审核边界段，列出 `FangyuanAuditReport`、`FangyuanAuditBudgetProfile`、finding、suggestion、blueprint/prefab/layout 审核入口、家园 HUD/日志和非目标；`git diff --check` 通过）
- [x] 更新 `docs/世界观/方圆灵构蓝图规则.md`，补充审核规则、预算建议、错误 code、降级建议和生成提示词注意事项。（验证：文档新增默认预算推荐/硬限制、常见 blueprint/prefab/layout/runtime budget code、降级建议动作和 Codex 提示词约束；主 agent 与 `project/src/framework/fangyuan/audit.rs` 默认值及 suggestion action 复核一致）
- [x] 如审核系统新增资源路径、开发命令或调试方式影响新成员理解，检查并同步 `docs/bevy-getting-started.md`。（验证：文档新增 `dev.fangyuan_home` 首包场景、`MYBEVY_START_SCENE="dev.fangyuan_home"` 家园审核 HUD 验收命令和 Windows `WGPU_BACKEND="dx12"` 可视验收说明）
- [x] 如仓库级说明需要更新，检查并同步 `CLAUDE.md`。（验证：`CLAUDE.md` 的 `project/src/framework/fangyuan/` 目录说明已补充审核 report / budget profile / finding / suggestion）
- [x] 确认文档仍明确 Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI、联网同步、正式家园编辑器、蓝图持久化、装备挂点和技能规则层不是本阶段能力。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md`、`docs/世界观/方圆灵构蓝图规则.md`、`docs/bevy-getting-started.md` 均明确第五阶段只覆盖审核和预算；`rg` 复核相关词均为后续/非目标说明）
- [x] checklist 全部完成后，按仓库约定将本文件从 `summary/` 归档到合适的 `docs/<领域>/checklists/` 目录。（验证：归档目标确认为 `docs/fangyuan/checklists/方圆蓝图审核和预算系统第五阶段_checklist.md`，移动前已确认源文件和目标目录绝对路径）
- [x] 归档前确认 checklist 的阶段时间、开发总结和验证记录均来自真实执行结果。（验证：阶段 1-10 均有开始/结束时间、总结和命令/代码/截图证据；阶段 9 记录了 DX12 可视截图和 GUI 自动点击限制，未虚构点击结果）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`，以及必要的文档路径检查。（验证：主 agent 执行全部命令通过；额外执行 `cargo test fangyuan_player_preview -- --nocapture` 通过；`Resolve-Path` 确认 checklist 源文件和归档目录存在）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-02 23:18:53 +08:00
- 结束时间：2026-07-02 23:18:53 +08:00
- 验收总结：第五阶段蓝图审核和预算系统已完成。统一审核模型、预算 profile、blueprint/prefab/layout 审核入口、家园 HUD/日志、降级建议、文档同步和归档准备均通过验证；默认家园预览在 DX12 手机比例窗口下可见 `layout loaded gen 138/1000 skip 0`、`audit passed e0 w0 -`、`pal 1 pf 5 used 5 inst 40 mat 15`；Clear/Reload/Lobby 交互行为由 ECS/UI 单元测试覆盖。未引入方圆旋转能力，也未提前实现 Chunk、Bake、mesh merge、GPU Instancing、LOD、AOI、联网同步、正式编辑器、持久化、装备挂点或技能规则层。

- [x] 存在统一方圆审核 report，能覆盖 blueprint、prefab palette 和 scene layout。（验证：`project/src/framework/fangyuan/audit.rs` 定义 `FangyuanAuditReport`；`blueprint.rs`、`prefab.rs`、`layout.rs` 均接入 `audit()` / `audit_with_default_budget()`；`cargo test fangyuan -- --nocapture` 通过）
- [x] 审核 finding 具备稳定 severity、code、field_path、reason 和来源定位。（验证：`FangyuanAuditFinding` 包含 severity/code/field_path/reason/source_kind/source_path/primitive/prefab/instance 定位字段，排序和定位测试通过）
- [x] 默认预算 profile 能表达 primitive 数、bounds、size、volume、alpha、emissive、material profile、role 和 lifecycle 相关限制。（验证：`FangyuanAuditBudgetProfile` 默认值和 `FangyuanPrimitiveBudgetStats` 覆盖对应字段；`fangyuan_budget_*` 测试在 `cargo test fangyuan` 中通过）
- [x] simple blueprint 审核能覆盖 minimal player 和 legacy home preview。（验证：`fangyuan_blueprint_audit_passes_legal_minimal_player_with_summary` 和 `fangyuan_blueprint_audit_keeps_legacy_home_preview_warning_compatibility` 通过）
- [x] prefab palette 审核能覆盖默认 `home_prefabs.ron`，并报告 prefab/authored primitive/material 等统计。（验证：`fangyuan_prefab_audit_passes_default_home_prefab_palette` 通过，断言 prefab_count=5、authored_primitives=19、material_count=0）
- [x] scene layout 审核能覆盖默认 `home_layout.ron`，并报告 instance/generated/skipped/used prefab 等统计。（验证：`fangyuan_layout_audit_passes_default_home_layout_with_expected_summary` 通过，断言 generated=138、skipped=0、instance=40、prefab=5、used_prefab=5）
- [x] 默认家园预览接入审核状态，HUD 和日志能显示简短审核摘要。（验证：`project/src/game/scenes/fangyuan_home.rs` 写入 audit stats 和日志；`project/src/game/screens/gameplay/fangyuan_home.rs` HUD 测试覆盖 passed/warning/failed/pending；DX12 截图显示审核 HUD）
- [x] 审核 failed 时不会生成误导性的成功 primitive 数据。（验证：`failed_audit_status_does_not_spawn_misleading_success_stats` 和 missing prefab 测试通过）
- [x] 降级建议作为 report 数据返回，不自动改写 RON 或 runtime primitive。（验证：suggestion 类型和 debug formatter 仅返回 report/log 数据；worker/主审 diff 未发现 RON 写入或 runtime primitive 自动修复路径）
- [x] 玩家预览入口和最小 cube/sphere 玩家外观不因审核系统改动回退。（验证：`cargo test fangyuan_player_preview -- --nocapture` 通过，27 passed）
- [x] 代码、测试和文档中不存在 rotation、quaternion、euler、angular_velocity、rotate 或 spin 能力。（验证：`rg` 复核方圆实现中相关词仅为禁止字段、拒绝测试、identity transform 或既有非方圆场景能力；方圆 blueprint/prefab/layout 禁止字段测试通过）
- [x] 文档同步记录第五阶段实际落地边界和后续阶段延后事项。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md`、`docs/世界观/方圆灵构蓝图规则.md`、`docs/bevy-getting-started.md` 和 `CLAUDE.md` 已同步）
- [x] `cargo fmt --check` 通过。（验证：主 agent 在阶段 10 执行通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：194 passed，仅既有 `checkbox` dead_code warning）
- [x] `cargo test fangyuan_home -- --nocapture` 通过。（验证：40 passed，仅既有 `checkbox` dead_code warning）
- [x] `cargo test fangyuan_player_preview -- --nocapture` 通过。（验证：27 passed，仅既有 `checkbox` dead_code warning）
- [x] `cargo check` 通过。（验证：主 agent 在阶段 10 执行通过，仅既有 `selection.rs:32` warning）
- [x] 用户手动验收游戏内方圆家园审核 HUD 和 layout/palette 预览效果无回退。（验证：DX12 手机比例窗口截图 `project/target/fangyuan_stage9_dx12_start_scene_screen.png` 显示默认 layout/palette 内容和审核 HUD；GUI 自动鼠标注入限制已如实记录，按钮行为由单元测试覆盖）
