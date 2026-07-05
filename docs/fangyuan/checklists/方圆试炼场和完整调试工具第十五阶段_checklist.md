# 方圆试炼场和完整调试工具第十五阶段 Checklist

## 目标

交付方圆系统第十五阶段能力：建设灵构试炼场和完整调试工具，让玩家能理解蓝图为什么被降级，让开发者能定位性能瓶颈，并能在 authority 回放下验证视觉一致性。该阶段同时沉淀大规模压力测试和长期调优入口。

本阶段不是新增一个单点功能，而是把前面阶段逐步积累的审核、预算、渲染、材质、VFX、技能、装备、NPC、Chunk、LOD、Bake、缓存和继承能力统一纳入可观察、可复现、可回归的工具链。

## 功能地图

| 功能域 | 第十五阶段处理方式 |
| --- | --- |
| 玩家试炼场 | 展示蓝图审核、预算、降级原因、继承结果和可见 fallback |
| 开发调试面板 | 展示实例数量、batch、buffer 更新、LOD、AOI、灵压、cache、bake artifact |
| 压力测试 | 模拟 100、300、1000 人技能释放和热点降级 |
| 回放一致性 | authority replay 下对比视觉事件、VFX state hash 和关键帧摘要 |
| 报告 | 输出可归档的性能和审核报告，便于长期调优 |
| 非目标 | 不做运营后台、云端压测平台或最终商业监控系统 |

## 基础原则

- [ ] 工具必须服务真实定位，指标名称、采样口径和报告字段要稳定。
- [ ] 玩家可见解释和开发者底层指标分层展示，避免 UI 过载。
- [ ] 压力测试必须可复现，输入 seed、人数、技能模板和场景配置可记录。
- [ ] authority 回放一致性验证优先使用摘要和 hash，不依赖肉眼判断。
- [ ] 工具本身不能显著改变被测场景的预算结果；必要时记录工具开销。
- [ ] 每个阶段完成后运行对应验证，并按阶段提交。

## 阶段 1：指标口径和调试数据总线

- 开始时间：2026-07-05 21:36:11 +08:00
- 结束时间：2026-07-05 22:11:21 +08:00
- 开发总结：新增方圆调试指标数据总线，统一 primitive / render / LOD / AOI / pressure / cache / bake / audit 等指标命名和 snapshot 聚合模型，支持 Bevy Resource / Message 注册、采样间隔、滚动窗口、峰值、平均值和 reset。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_debug_metrics -- --nocapture` 通过（7 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 统一方圆调试指标命名：primitive、instance、batch、mesh、buffer bytes、LOD、AOI、pressure、cache、bake、audit。（验证：`project/src/framework/fangyuan/debug_metrics.rs:16` 定义 `FangyuanDebugMetricKey`，`:31` 的 `ALL` 覆盖 11 个稳定指标名；`:710` 测试确认字段名顺序和 record name 稳定）
- [x] 建立调试数据聚合资源或事件，汇总各模块摘要，避免 HUD 直接读取散落内部状态。（验证：`project/src/framework/fangyuan/debug_metrics.rs:410` 定义统一 snapshot，`:545` 定义 `FangyuanDebugMetricsBus` Resource，`:523` 定义 `FangyuanDebugMetricsEvent` Message，`:759` 测试覆盖 primitive/render/LOD/AOI/pressure 聚合）
- [x] 定义指标采样频率、滚动窗口、峰值、平均值和 reset 规则。（验证：`project/src/framework/fangyuan/debug_metrics.rs:63` 定义采样配置，`:614` 实现 sample 间隔和 rolling window，`:635` 实现 reset；`:840` / `:886` 测试覆盖峰值、平均值、窗口和 reset）
- [x] 为指标聚合、缺失模块、reset、峰值和稳定字段名补测试。（验证：`project/src/framework/fangyuan/debug_metrics.rs:710` / `:741` / `:759` / `:840` / `:886` 覆盖稳定字段、缺失模块、聚合、采样窗口和 reset；`cargo test fangyuan_debug_metrics -- --nocapture` 7 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_debug_metrics -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 2：玩家灵构试炼场

- 开始时间：2026-07-05 22:13:48 +08:00
- 结束时间：2026-07-05 22:52:29 +08:00
- 开发总结：扩展方圆玩家灵构试炼场数据和 Home 接入，支持默认家园、装备、技能和外观样例选择，展示审核状态、预算、降级前后、结果桶、fallback 和通俗原因，并新增重审与预算档位切换按钮。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_trial -- --nocapture` 通过（4 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 建设玩家可进入的灵构试炼场，用于导入或选择家园、装备、技能和外观蓝图。（验证：`project/src/framework/fangyuan/object_budget.rs:355` 定义 trial 蓝图域，`:684` 提供 home/equipment/skill/appearance 默认选择；`project/src/game/scenes/fangyuan_home.rs:2625` 接入 Home trial 命令）
- [x] 展示审核 status、error、warning、suggestion、预算消耗、降级前后对比和缺失 fallback。（验证：`project/src/framework/fangyuan/object_budget.rs:455` 定义 `FangyuanTrialAuditPresentation`，包含 status/error/warning/suggestion/budget/before/after/fallback；`project/src/game/screens/gameplay/fangyuan_home.rs:285` HUD 输出 trial status、cost、before/after、result 和 fallback）
- [x] 显示通俗降级原因，例如 primitive 过多、透明过量、发光过强、规则层被遮挡。（验证：`project/src/framework/fangyuan/object_budget.rs:1137` 定义通俗原因映射，`:1740` 测试确认玩家展示原因包含技能颜色等可读说明）
- [x] 支持重新运行审核、切换预算 profile、查看 kept / degraded / rejected 结果。（验证：`project/src/framework/fangyuan/object_budget.rs:925` / `:930` 实现 rerun audit 和 switch budget profile，`:1112` 计算 kept/degraded/rejected；`project/src/game/screens/gameplay/fangyuan_home.rs:45` / `:48` 定义重审和预算按钮）
- [x] 为试炼场路由、审核展示、降级状态和返回流程补 UI / ECS 测试。（验证：`project/src/framework/fangyuan/object_budget.rs:1740` / `:1773` 覆盖审核展示和预算切换；`project/src/game/scenes/fangyuan_home.rs:4395` 覆盖 ECS 命令、预算和返回清理；`project/src/game/screens/gameplay/fangyuan_home.rs:585` 覆盖 UI 按钮和返回大厅命令）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_trial -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；`cargo test fangyuan_trial -- --nocapture` 为 4 passed，仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 3：开发者调试面板

- 开始时间：2026-07-05 22:54:27 +08:00
- 结束时间：2026-07-05 23:40:20 +08:00
- 开发总结：新增方圆开发者调试面板模型和 Home overlay，默认隐藏详细指标，通过“调试”按钮打开，并支持 render / LOD / cache / bake / audit / trial 模块开关和手机紧凑布局，避免继续堆叠默认 HUD。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_debug_panel -- --nocapture` 通过（6 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 增加开发调试面板，展示 render mode、mesh count、instance batch、buffer update、draw estimate 和 material profile。（验证：`project/src/framework/fangyuan/debug_metrics.rs:746` 定义 panel snapshot，`:783` 输出 render mode / mesh / instance batch / buffer update / draw estimate / material profile；`:1514` 测试覆盖字段）
- [x] 展示 LOD distribution、loaded chunks、AOI radius、hotspot pressure、degrade reason、cache hit/miss 和 bake artifact 信息。（验证：`project/src/framework/fangyuan/debug_metrics.rs:825` 输出 LOD/AOI/pressure/degrade，`:888` 输出 cache hit/miss，`:917` 输出 bake artifact；`:1514` 和 `:1616` 测试覆盖存在与缺失状态）
- [x] 支持按模块开关详细日志或 overlay，避免默认 HUD 过载。（验证：`project/src/framework/fangyuan/debug_metrics.rs:579` 定义 panel modules，`:622` 定义 toggles，`project/src/game/screens/gameplay/fangyuan_home.rs:59` / `:68` 定义调试和模块按钮，`:802` 处理开关；`:1080` 测试确认默认 HUD 不包含 debug panel 内容）
- [x] 为面板数据格式、开关、缺失模块、手机窗口布局和不遮挡主要交互补测试或手动验收。（验证：`project/src/framework/fangyuan/debug_metrics.rs:1514` / `:1595` / `:1616` 覆盖格式、开关和缺失模块；`project/src/game/screens/gameplay/fangyuan_home.rs:1044` / `:1141` 覆盖按钮开关和手机紧凑 overlay 尺寸）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_debug_panel -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；`cargo test fangyuan_debug_panel -- --nocapture` 为 6 passed，仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 4：压力测试场景生成器

- 开始时间：2026-07-05 23:42:25 +08:00
- 结束时间：2026-07-06 00:16:04 +08:00
- 开发总结：新增方圆本地 deterministic 压力测试模块，支持可序列化配置、100 / 300 / 1000 人规模预设、seed 稳定 actor 排程、tick 曲线、压力/降级摘要、chunk load 和本地 summary text。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_pressure -- --nocapture` 通过（7 passed，含既有 LOD pressure scenario 输出）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 建立可复现压力测试配置，包含人数、技能模板、触发频率、seed、场景大小、chunk 数和预算 profile。（验证：`project/src/framework/fangyuan/pressure.rs:91` 定义 `FangyuanPressureTestConfig`，`:129` 提供规模预设构造，`:147` 校验 actor/skill/interval/seed/scene/chunk/budget 等字段）
- [x] 支持 100、300、1000 人技能释放模拟，输出 active VFX、dynamic primitive、trail、transparent、emissive 和 pressure 曲线。（验证：`project/src/framework/fangyuan/pressure.rs:258` 定义 tick sample 字段，`:332` 汇总曲线，`:446` 执行 simulation；`:907` 测试覆盖 100 / 300 / 1000 规模）
- [x] 压测不依赖外部联网服务，优先本地 deterministic simulation。（验证：`project/src/framework/fangyuan/pressure.rs:446` 使用本地 skill/VFX 数据执行模拟，`:878` / `:893` 测试确认同 seed hash 稳定、不同 seed hash 改变）
- [x] 为配置解析、seed 稳定性、规模阶梯、输出 report 和失败退出补测试。（验证：`project/src/framework/fangyuan/pressure.rs:852` / `:878` / `:907` / `:935` / `:955` 覆盖配置、seed、规模、report 和错误退出；`cargo test fangyuan_pressure -- --nocapture` 7 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_pressure -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 5：authority 回放视觉一致性

- 开始时间：2026-07-06 00:18:06 +08:00
- 结束时间：2026-07-06 01:07:33 +08:00
- 开发总结：新增方圆 authority visual replay 摘要和一致性报告，视觉 hash 覆盖 VFX 状态、规则层、个性层、LOD、降级、cache/fallback 和关键材质参数，并能在 hash 不一致时定位 tick、frame、event、recipe、object 和差异字段。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_visual_replay -- --nocapture` 通过（6 passed）；`cargo test authority -- --nocapture` 通过（13 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 定义视觉摘要 hash，覆盖 skill VFX state、规则层、个性层、LOD、degrade 和关键 material params。（验证：`project/src/framework/fangyuan/visual_replay.rs:258` 定义 sample 字段，`:327` 构建 replay summary，`:497` 计算 material hash；`:952` 测试覆盖 degrade 和 LOD hash 输入）
- [x] 同一 authority replay 多次运行时输出一致视觉摘要，差异时能定位 tick、event、recipe 或 object id。（验证：`project/src/framework/fangyuan/visual_replay.rs:278` 定义 mismatch summary，`:352` 比对报告，`:864` 测试同 replay hash 稳定，`:884` 测试 mismatch 定位 tick/event/recipe/object）
- [x] 为延迟输入、跳帧、seed、降级压力和缓存命中/缺失路径补一致性测试。（验证：`project/src/framework/fangyuan/visual_replay.rs:910` 覆盖延迟输入和跳帧，`:927` 覆盖 seed 差异，`:952` 覆盖降级压力，`:989` 覆盖 cache hit/miss 与 fallback）
- [x] 报告中记录 replay id、start tick、event count、visual hash、mismatch summary。（验证：`project/src/framework/fangyuan/visual_replay.rs:221` 定义 `FangyuanVisualReplayConsistencyReport` 字段，`:404` 输出带 mismatch 的一致性报告；`project/src/game/authority/plugin.rs:1560` 测试从 authority frame payload 构建 visual replay report）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_visual_replay -- --nocapture`、`cargo test authority -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑四条命令全部通过；`fangyuan_visual_replay` 6 passed，`authority` 13 passed，仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 6：报告导出和长期回归基线

- 开始时间：2026-07-06 01:09:43 +08:00
- 结束时间：2026-07-06 01:52:20 +08:00
- 开发总结：新增方圆 debug report schema 和本地 pressure baseline 快照/比较格式，聚合 audit、budget、render、LOD、AOI、cache、bake、pressure 和 replay 摘要，并明确本地输出到已忽略的 `artifacts/fangyuan-debug/`。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_debug_report -- --nocapture` 通过（5 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 输出方圆调试报告，包含审核、预算、渲染、LOD、AOI、cache、bake、pressure 和 replay 摘要。（验证：`project/src/framework/fangyuan/debug_report.rs:48` 定义 `FangyuanDebugReport`，`:136` / `:178` / `:239` / `:277` / `:309` / `:339` / `:363` / `:383` / `:566` 分别覆盖 audit、budget、render、LOD、AOI、cache、bake、pressure 和 replay）
- [x] 定义本地性能基线文件或快照格式，便于长期比较 100 / 300 / 1000 压测结果。（验证：`project/src/framework/fangyuan/debug_report.rs:644` 定义 baseline snapshot，`:755` 定义 baseline entry，`:842` 定义 comparison；`:1327` 测试覆盖 hash 和指标变化比较）
- [x] 报告导出不提交大体量运行产物，必要时写入 ignored 目录或明确路径。（验证：`project/src/framework/fangyuan/debug_report.rs:16` / `:1027` / `:1031` 指定 `artifacts/fangyuan-debug/` 和 JSON 路径；`git check-ignore -v artifacts/fangyuan-debug/pressure-baseline.json` 确认被根 `.gitignore` 的 `/artifacts/` 规则忽略）
- [x] 为 report schema、字段稳定性、空数据、压测摘要和回放 mismatch 补测试。（验证：`project/src/framework/fangyuan/debug_report.rs:1218` / `:1248` / `:1274` / `:1295` / `:1327` 覆盖 schema、空数据、pressure summary、replay mismatch 和 baseline compare；`cargo test fangyuan_debug_report -- --nocapture` 5 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_debug_report -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 7：端到端手动验收

- 开始时间：2026-07-06 01:54:35 +08:00
- 结束时间：2026-07-06 02:14:35 +08:00
- 开发总结：完成第十五阶段端到端验收。首次完整 `fangyuan` 回归发现 trial 清理后状态误报为 passed，已打回 worker 修复为 pending/ok 清理摘要；修复后完整 `fangyuan`、authority、check、手机窗口启动、100/300/1000 压测阶梯和同 replay hash 稳定性均通过。
- 验证记录：`cargo fmt --check` 通过；首次 `cargo test fangyuan -- --nocapture` 失败 1 项（trial 清理状态误报），修复后 `cargo test reload_failure_clears_old_layout_content_but_keeps_base_space -- --nocapture` 1 passed、`cargo test fangyuan_trial -- --nocapture` 4 passed、`cargo test fangyuan -- --nocapture` 416 passed；`cargo test authority -- --nocapture` 13 passed；`cargo check` 通过；`cargo run --bin project -- --window-profile phone-portrait` 与 `TOUCH_START_SCREEN=fangyuan_home cargo run --bin project -- --window-profile phone-small` 均限时启动成功并主动结束，未见 panic。

- [x] 运行 `cargo fmt --check`。（验证：主 agent 在 `project/` 执行通过；修复 trial 清理回归后未产生格式差异）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：首次执行发现 `reload_failure_clears_old_layout_content_but_keeps_base_space` 失败，修复 `project/src/framework/fangyuan/object_budget.rs:1032` 的 clear summary 后复跑通过，416 passed / 0 failed / 674 filtered out）
- [x] 运行 `cargo test authority -- --nocapture` 或等价回放测试。（验证：主 agent 在 `project/` 执行通过，13 passed / 0 failed，包含 `authority_visual_replay_summary_can_be_built_from_frame_payload`）
- [x] 运行 `cargo check`。（验证：主 agent 在 `project/` 执行通过，仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）
- [x] 手动运行手机比例窗口，进入灵构试炼场，确认审核解释、降级对比和调试面板可用。（验证：`cargo run --bin project -- --window-profile phone-portrait` 限时启动成功；`TOUCH_START_SCREEN=fangyuan_home cargo run --bin project -- --window-profile phone-small` 启动到 Fangyuan Home，日志出现 `trial_rerun`/`trial_budget`/`debug`/debug module fallback 文案且未见 panic；`cargo test fangyuan_trial -- --nocapture` 4 passed 覆盖重审、预算切换、降级展示和返回流程）
- [x] 手动运行 100 / 300 / 1000 压测配置，记录性能摘要和热点降级可读性。（验证：`cargo test fangyuan_pressure_scale_steps_cover_100_300_and_1000_actor_simulations -- --nocapture` 1 passed；完整 `cargo test fangyuan -- --nocapture` 中 pressure scenario 输出 100 / 300 / 1000 bottleneck、degrade、lod 和 path 摘要）
- [x] 手动验证回放一致性报告在相同 seed / replay 下稳定。（验证：`cargo test fangyuan_visual_replay_same_authority_replay_outputs_stable_summary_hash -- --nocapture` 1 passed；完整 `cargo test authority -- --nocapture` 13 passed 覆盖 authority frame payload 到 visual replay report）

## 阶段 8：文档同步和归档准备

- 开始时间：2026-07-06 02:17:09 +08:00
- 结束时间：2026-07-06 02:33:47 +08:00
- 开发总结：同步方圆技术路线和新成员上手文档，记录第十五阶段已落地的试炼场、调试面板、压测、视觉回放一致性、debug report 和 baseline 边界，并创建 checklist 归档副本。
- 验证记录：`git diff --check` 通过（仅 LF/CRLF warning）；`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（416 passed，0 failed，674 filtered out）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 更新方圆技术路线，记录试炼场、调试面板、压测、报告和回放一致性。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:2051` 起记录 debug metrics bus、玩家试炼场、developer debug panel、pressure simulation、visual replay consistency 和 debug report / baseline）
- [x] 更新新成员上手或调试文档，说明如何运行试炼场、压力测试和报告导出。（验证：`docs/bevy-getting-started.md:496` 起记录 `TOUCH_START_SCREEN=fangyuan_home cargo run -- --window-profile phone-small` 以及 `fangyuan_trial` / `fangyuan_debug_panel` / `fangyuan_pressure` / `fangyuan_visual_replay` / `fangyuan_debug_report` 定向命令）
- [x] 确认文档明确该阶段不是运营后台、云压测平台或商业监控系统。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:2058` 和 `docs/bevy-getting-started.md:517` 明确排除运营后台、云压测平台、商业监控系统、线上告警和生产级监控管线）
- [x] checklist 完成后归档到 `docs/fangyuan/checklists/`。（验证：`docs/fangyuan/checklists/方圆试炼场和完整调试工具第十五阶段_checklist.md:1` 已创建归档副本，并在最终完成后与 summary 源文件同步）
- [x] 验证命令：`git diff --check`、`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 复跑四条命令全部通过；`cargo test fangyuan -- --nocapture` 为 416 passed / 0 failed / 674 filtered out，仅有既有 `selection.rs:32` dead_code warning）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-05 21:36:11 +08:00
- 结束时间：2026-07-06 02:33:47 +08:00
- 验收总结：方圆试炼场和完整调试工具第十五阶段完成。已建立 debug metrics bus、玩家灵构试炼场、开发者调试面板、本地 100 / 300 / 1000 压测模拟、authority visual replay 一致性、debug report / pressure baseline schema，并完成文档同步和 checklist 归档。运营后台、云压测平台、商业监控系统、线上告警和生产级监控数据管线仍不属于本阶段能力。

- [x] 玩家能在灵构试炼场看到蓝图为什么被降级，以及降级前后差异。（验证：`project/src/framework/fangyuan/object_budget.rs:455` 定义 trial audit presentation，`project/src/game/screens/gameplay/fangyuan_home.rs:285` HUD 输出 trial status/cost/before/after/result/fallback/reason；`cargo test fangyuan_trial -- --nocapture` 4 passed）
- [x] 开发者能在调试面板看到实例数量、batch、buffer、LOD、AOI、灵压、cache 和 bake 摘要。（验证：`project/src/framework/fangyuan/debug_metrics.rs:746` 定义 panel snapshot，`:783` / `:825` / `:888` / `:917` 输出 render、LOD/AOI/pressure、cache 和 bake；`cargo test fangyuan_debug_panel -- --nocapture` 6 passed）
- [x] 100 / 300 / 1000 人压力测试可复现，并输出稳定报告。（验证：`project/src/framework/fangyuan/pressure.rs:91` 定义 pressure config，`:446` 执行 simulation，`:540` 输出 report text；`cargo test fangyuan_pressure_scale_steps_cover_100_300_and_1000_actor_simulations -- --nocapture` 1 passed）
- [x] authority replay 视觉一致性可以通过摘要 hash 自动验证。（验证：`project/src/framework/fangyuan/visual_replay.rs:221` 定义 consistency report，`:327` 汇总 replay hash，`project/src/game/authority/plugin.rs:1560` 覆盖 authority frame payload；`cargo test authority -- --nocapture` 13 passed）
- [x] `cargo fmt --check` 通过。（验证：阶段 7 和阶段 8 均由主 agent 在 `project/` 执行通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：阶段 7 修复 trial 清理回归后通过，阶段 8 最终复跑通过，416 passed / 0 failed / 674 filtered out）
- [x] `cargo test authority -- --nocapture` 或等价回放测试通过。（验证：阶段 7 主 agent 执行通过，13 passed / 0 failed）
- [x] `cargo check` 通过。（验证：阶段 7 和阶段 8 均由主 agent 在 `project/` 执行通过，仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）
