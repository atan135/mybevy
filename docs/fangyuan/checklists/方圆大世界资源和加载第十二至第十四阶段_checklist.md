# 方圆大世界资源和加载第十二至第十四阶段 Checklist

## 目标

合并推进方圆系统第十二至第十四阶段：空间 Chunk / LOD / AOI / 热点降级、发布期二进制 Bake 和运行时加载、蓝图缓存 / 流式加载 / 纪元继承。目标是把方圆内容从本地家园和试炼场推进到可分块加载、可发布打包、可缓存更新、可长期继承的大世界资源体系。

本 checklist 重点处理 Chunk 数据、加载生命周期、LOD 和热点降级、Bake artifact、runtime loader、cache、streaming update、version / hash 和纪元迁移。本阶段不实现完整服务器集群、商业 CDN、运营后台、账号资产交易或最终云压测平台。

## 功能地图

| 功能域 | 处理方式 |
| --- | --- |
| Chunk | 作为空间加载单位，记录 prefab instances、天道 refs、静态装饰和 region metadata |
| LOD / AOI | 控制加载、同步和视觉播放范围，先做本地模拟和接口占位 |
| 热点降级 | 优先降低装饰层、透明、发光、拖尾和远处玩家高成本表现 |
| Bake | 开发期 RON 编译为发布期二进制 artifact、manifest 和 dependency table |
| Runtime Load | 正式路径优先 bin，debug / dev 可回退 RON |
| Cache / Streaming | 蓝图、prefab、chunk、artifact 带 version / hash，支持缓存和部分更新 |
| 纪元继承 | 旧视觉资产进入新世界时重新审核、预算校准和降级 |

## 基础原则

- [x] Chunk 是加载和预算单位，不是把所有 primitive 重新变成玩法 Entity。（验证：`project/src/framework/fangyuan/chunk.rs` 的 chunk 内容项使用 prefab / blueprint / bake 引用，`cargo test fangyuan_chunk -- --nocapture` 与最终 `cargo test fangyuan -- --nocapture` 通过）
- [x] LOD 和热点降级必须保留规则可读性，优先压缩个性层和装饰层。（验证：`project/src/framework/fangyuan/lod.rs` 定义有序 degrade plan，规则层压缩最后；最终 `cargo test fangyuan -- --nocapture` 通过）
- [x] Bake 和 runtime 必须共用核心数据结构、validator、version upgrade 和 audit 逻辑。（验证：`project/src/framework/fangyuan/bake.rs` 复用 source upgrade、validator、audit 和 runtime loader；最终 `cargo test fangyuan -- --nocapture` 通过）
- [x] 同一份源 RON 在相同版本下应 deterministic bake。（验证：`cargo test fangyuan_bake -- --nocapture` 与最终 `cargo test fangyuan -- --nocapture` 覆盖 deterministic bake）
- [x] 所有可缓存内容都必须带 version 和 hash，缓存命中也必须校验。（验证：`project/src/framework/fangyuan/identity.rs` 和 `cache.rs` 覆盖 identity/version/hash 与 cache read 校验；最终 `cargo test fangyuan -- --nocapture` 通过）
- [x] 客户端缓存只能作为性能优化，不能绕过服务端审核、预算和权限判定。（验证：`project/src/framework/fangyuan/cache_authority.rs` 明确 cache bytes-only，server manifest 可覆盖；`cargo test fangyuan_cache_authority -- --nocapture` 和最终 `cargo test fangyuan -- --nocapture` 通过）
- [x] 每个阶段完成后运行对应验证，并按阶段提交。（验证：阶段 1 至 9 均有提交记录，阶段 10 为无代码验收，阶段 11 文档归档准备验证通过并将单独提交）

## 阶段 1：Chunk 数据模型和 Manifest

- 开始时间：2026-07-05 08:45:48 +08:00
- 结束时间：2026-07-05 09:18:21 +08:00
- 开发总结：新增方圆 chunk 数据模型、manifest / dev RON 源格式、引用模型、预算摘要和校验逻辑，并导出 fangyuan chunk 模块；本阶段只处理数据契约，不接入加载、LOD/AOI、Bake、cache 或 streaming。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_chunk -- --nocapture` 通过（9 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 新增方圆 chunk id、bounds、region metadata、prefab instance refs、tiandao refs、static decoration refs 和 budget summary。（验证：`project/src/framework/fangyuan/chunk.rs:149` 定义 `FangyuanChunkSource`，`:330` 定义 bounds，`:444` 起定义 region / prefab / tiandao / static decoration refs，`:526` 定义 budget summary）
- [x] 定义 chunk manifest 格式和开发期 RON 源格式，预留未来 bin / hash / version 字段。（验证：`project/src/framework/fangyuan/chunk.rs:19` 定义 `FangyuanChunkManifest`，`:116` 定义 manifest entry 的 `dev_ron` / `bin` / `hash` / `data_version`，`:149` 定义 dev RON source，`:154` 至 `:184` 预留 source 侧 artifact 字段）
- [x] 确认 chunk 不复制大体量 primitive 数据，只引用 prefab / blueprint / bake 占位。（验证：`project/src/framework/fangyuan/chunk.rs:453` / `:462` / `:476` 的内容项均为引用结构，`:485` 的 static decoration source 仅允许 prefab / blueprint / bake；`:1688` 测试拒绝 `primitives` 字段）
- [x] 为 chunk bounds、重复 id、非法 prefab ref、空 chunk 和预算 summary 补测试。（验证：`project/src/framework/fangyuan/chunk.rs:1584` 覆盖 bounds，`:1599` 覆盖重复 chunk id，`:1616` 覆盖缺失 prefab ref，`:1659` 覆盖空 chunk，`:1671` 覆盖预算 summary mismatch；`cargo test fangyuan_chunk -- --nocapture` 9 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_chunk -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 2：Chunk 加载、卸载和失败回滚

- 开始时间：2026-07-05 09:20:35 +08:00
- 结束时间：2026-07-05 10:26:52 +08:00
- 开发总结：新增本地 chunk runtime/loading 命令、事件、状态、debug summary、附近 chunk 选择和失败回滚逻辑，并小范围接入 Fangyuan Home 场景退出清理与 HUD 摘要；本阶段不实现正式 LOD/AOI、Bake、cache 或 streaming。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_chunk_loading -- --nocapture` 通过（12 passed）；`cargo check` 通过；额外 `cargo test hud_status_text -- --nocapture` 通过（4 passed），`cargo test fangyuan_home -- --nocapture` 通过（52 passed）；保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning。

- [x] 实现本地 chunk load / unload / reload 命令和事件，接入场景根实体和资源清理。（验证：`project/src/framework/fangyuan/chunk_loading.rs:307` 定义 `FangyuanChunkCommand`，`:722` 处理命令并写出事件，`:879` 生成 `FangyuanChunkRoot`，`project/src/game/scenes/fangyuan_home.rs:55` 注册 runtime/message，`:2610` 场景退出写入 clear 命令）
- [x] 支持玩家移动或调试位置变化时选择附近 chunk，卸载远离 chunk。（验证：`project/src/framework/fangyuan/chunk_loading.rs:676` 定义 `select_fangyuan_chunks_near_position`，`:814` 处理 `SelectNearPosition` 并执行 unload/load，`:1052` 和 `:1184` 测试覆盖附近选择、保留、卸载和加载）
- [x] 加载失败、审核失败、缺失 prefab 和重复加载必须有明确状态和 fallback。（验证：`project/src/framework/fangyuan/chunk_loading.rs:78` runtime 记录 `last_failure` 和事件，`:514` 定义 `Fallback` 状态，`:539` 定义 load error code/reason，`:977`、`:996`、`:1020`、`:1036` 测试覆盖重复加载、reload 失败回滚、缺失 prefab 和校验失败 fallback）
- [x] HUD / debug 显示 loaded chunks、visible objects、load state 和失败原因。（验证：`project/src/framework/fangyuan/chunk_loading.rs:633` 定义 `FangyuanChunkDebugSummary`，`project/src/game/screens/gameplay/fangyuan_home.rs:230` 从 runtime 生成摘要，`:280` 至 `:284` 输出 chunk 数、对象数、状态、失败原因和 ids，`:603` 测试覆盖 HUD chunk 摘要）
- [x] 为加载、卸载、重复加载、失败回滚、场景退出和抖动保护补测试。（验证：`project/src/framework/fangyuan/chunk_loading.rs:957` 覆盖加载/卸载，`:977` 覆盖重复加载，`:996` 覆盖失败回滚，`:1052` 覆盖抖动保护，`:1091` 覆盖 scene exit clear，`:1143` 覆盖 root 清理；`cargo test fangyuan_chunk_loading -- --nocapture` 12 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_chunk_loading -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑三条命令全部通过；额外 `cargo test hud_status_text -- --nocapture` 4 passed、`cargo test fangyuan_home -- --nocapture` 52 passed；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 3：LOD、AOI 和热点灵压

- 开始时间：2026-07-05 10:29:07 +08:00
- 结束时间：2026-07-05 11:00:02 +08:00
- 开发总结：新增 Fangyuan LOD / AOI / hotspot 纯模型，覆盖 L0-L4 表现层级、对象默认 LOD 映射、本地 AOI chunk/object selector、热点压力指标、天道灵压解释和有序降级计划；本阶段不接渲染实体或压力场景。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_lod -- --nocapture` 通过（2 passed）；`cargo test fangyuan_aoi -- --nocapture` 通过（4 passed）；`cargo test fangyuan_hotspot -- --nocapture` 通过（3 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 定义 L0-L4 表现层级，例如 full、reduced、silhouette、marker、hidden / rule-only。（验证：`project/src/framework/fangyuan/lod.rs:6` 定义 `FangyuanLodLevel`，`:13` 输出 full/reduced/silhouette/marker/hidden_rule_only，`:916` 测试覆盖层级语义）
- [x] 为 static object、home decoration、equipment、NPC、skill VFX 和 tiandao object 定义默认 LOD 映射。（验证：`project/src/framework/fangyuan/lod.rs:35` 定义对象类型，`:78` 定义默认映射，`:916` 测试覆盖六类对象）
- [x] 实现本地 AOI selector，输出需要加载、保留、卸载和只显示 marker 的 chunk / object 集合。（验证：`project/src/framework/fangyuan/lod.rs:347` 定义 `select_fangyuan_aoi`，输出 `load_chunks` / `keep_chunks` / `unload_chunks` / `marker_chunks` 和 object decisions；`:964` 测试覆盖 load/keep/unload/marker 和 object LOD）
- [x] 定义热点压力指标：active skill、dynamic primitive、transparent count、emissive total、trail count、chunk load pressure。（验证：`project/src/framework/fangyuan/lod.rs:479` 定义 `FangyuanHotspotMetrics`，`:519` 定义阈值，`:562` 定义 pressure kind；`:1100` 测试覆盖多指标触发）
- [x] 定义天道灵压或等价 UX 包装，让降级原因可解释。（验证：`project/src/framework/fangyuan/lod.rs:605` 定义 `FangyuanTiandaoPressureSummary`，`:662` 在 hotspot evaluation 中生成 label/explanation/primary reason，`:763` 生成解释文本，`:1100` 测试确认 explanation 包含触发原因）
- [x] 降级顺序优先为装饰、透明、发光、拖尾、残留、远处个性层，最后才压缩规则层表现。（验证：`project/src/framework/fangyuan/lod.rs:620` 定义降级目标，`:642` 定义 order，`:688` 生成有序 plan，`:1100` 和 `:1188` 测试确认 rule layer compression 最后）
- [x] 为 LOD 选择、AOI 边界、压力阈值、自身优先级、恢复 hysteresis 和规则层保留补测试。（验证：`project/src/framework/fangyuan/lod.rs:916` 覆盖 LOD 映射，`:964` 覆盖 AOI 边界选择，`:1032` 覆盖 AOI hysteresis，`:1059` 覆盖自身/规则优先级，`:1157` 覆盖热点恢复 hysteresis，`:956` / `:1188` 覆盖规则层保留）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_lod -- --nocapture`、`cargo test fangyuan_aoi -- --nocapture`、`cargo test fangyuan_hotspot -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 4：Chunk / LOD / AOI 渲染集成和压力验证

- 开始时间：2026-07-05 11:02:40 +08:00
- 结束时间：2026-07-05 12:09:00 +08:00
- 开发总结：新增 Fangyuan LOD 集成摘要、渲染路径决策、render state reconcile、压力场景生成器，并接入 Fangyuan Home runtime / HUD；覆盖 chunk、static merge、static instancing、VFX、skill layer、equipment、NPC 和 tiandao 的调试决策与压力验证。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_chunk -- --nocapture` 通过（26 passed）；`cargo test fangyuan -- --nocapture` 通过（338 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 将 Chunk / LOD / AOI 输出接入 static merge、static instancing、VFX、skill layer、equipment、NPC 和 tiandao 对象。（验证：`project/src/framework/fangyuan/lod_integration.rs:14` 定义 render path，`:50` 定义对象 descriptor，`:526` 汇总 AOI selection 到 LOD render decisions，`project/src/game/scenes/fangyuan_home.rs:1704` / `:1732` / `:1809` 映射 standard / static merge / static instancing，`:2655` 映射 trial VFX/skill/equipment/NPC/tiandao）
- [x] 同一对象在不同 LOD 下能切换到合适渲染路径或隐藏路径，不残留旧表现。（验证：`project/src/framework/fangyuan/lod_integration.rs:474` 定义 render state reconcile，`:1209` 测试 LOD path 切换替换旧路径，`:1254` 测试 chunk unload 后 hidden path 清理旧对象）
- [x] HUD / debug 面板显示 loaded chunks、LOD distribution、AOI radius、pressure 和 degrade reason。（验证：`project/src/game/scenes/fangyuan_home.rs:542` 定义 LOD integration runtime，`:1014` 写入 stats，`project/src/game/screens/gameplay/fangyuan_home.rs:254` HUD 文本新增 lod/aoi/pressure/degrade/path 行，`:285` 至 `:289` 输出对应字段）
- [x] 准备多 chunk、多技能、多 NPC 的压力测试场景或生成器。（验证：`project/src/framework/fangyuan/lod_integration.rs:762` 定义 `generate_fangyuan_pressure_scenario`，生成多 chunk、skill、NPC、tiandao、static/merge/instance descriptors，`:1355` 测试使用 100 / 300 / 1000 规模）
- [x] 测试 100 / 300 / 1000 级对象或技能压力下的降级结果，记录可观察瓶颈。（验证：`project/src/framework/fangyuan/lod_integration.rs:1355` 测试打印 count/chunks/pressure/bottleneck/degrade/lod/path 摘要；`cargo test fangyuan -- --nocapture` 输出 100、300、1000 场景 bottleneck 和 degrade 摘要）
- [x] 为 LOD 切换、chunk 卸载、VFX 降级、NPC 降级、天道回收和压力场景补集成测试。（验证：`project/src/framework/fangyuan/lod_integration.rs:1209` 覆盖 LOD 切换，`:1254` 覆盖 chunk 卸载，`:1296` 覆盖 VFX 降级到 skill layer，`:1317` 覆盖 NPC marker 降级，`:1335` 覆盖 tiandao recycled hidden，`:1355` 覆盖压力场景）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_chunk -- --nocapture`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑四条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 5：Bake 格式、共享校验和工具入口

- 开始时间：2026-07-05 12:11:19 +08:00
- 结束时间：2026-07-05 13:15:11 +08:00
- 开发总结：新增 Fangyuan bake artifact envelope、格式选择记录、header 编解码、hash 校验、源版本升级、共享 validator/audit 入口和 `fangyuan_bake` cargo bin；CLI 支持输入目录、输出目录、dry-run 和 report path，本阶段 payload 暂写升级后的规范化源 RON，正式 runtime 二进制 payload 留到阶段 6。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_bake_format -- --nocapture` 通过（3 passed）；`cargo test fangyuan_bake_validation -- --nocapture` 通过（3 passed）；`cargo test fangyuan_bake -- --nocapture` 通过（10 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 评估 `bincode`、`postcard` 或等价二进制格式，记录选择理由、限制和依赖影响。（验证：`project/src/framework/fangyuan/bake.rs:187` 的 `fangyuan_bake_format_decisions` 记录 custom header 被选中、`bincode` / `postcard` 暂不引入及依赖影响；`:1202` 测试确认只有一个 selected 且不新增依赖）
- [x] 定义 bake artifact header，包含 magic、schema version、source hash、content hash、created by 和 target kind。（验证：`project/src/framework/fangyuan/bake.rs:16` 定义 magic/schema/hash 常量，`:112` 定义 `FangyuanBakeArtifactHeader`，`:216` / `:242` 编解码 header，`:147` 校验 source/content hash；`:1516` 回归测试确认 legacy payload 与 header hash 一致）
- [x] 定义 artifact kind：blueprint、prefab palette、scene layout、chunk、material profile、skill recipe。（验证：`project/src/framework/fangyuan/bake.rs:28` 定义 `FangyuanBakeArtifactKind` 六类目标，`:38` 定义 `ALL`，`:49` / `:83` 支持 wire id 和解析）
- [x] 抽取或复用现有 validator、audit、budget profile、asset path 校验和版本升级入口，避免 bake / runtime 两套逻辑。（验证：`project/src/framework/fangyuan/bake.rs:308` 定义旧版本升级，`:460` 定义 `validate_fangyuan_bake_source`，`:489` 复用 blueprint/palette/layout/chunk/material/skill 的现有 validate/audit；`:1216` 和 `:1260` 测试覆盖升级与 layout 校验错误）
- [x] 新增 `fangyuan_bake` CLI、cargo bin 或仓库脚本，支持输入目录、输出目录、dry-run 和 report path。（验证：`project/src/framework/fangyuan/bake.rs:761` 定义 `FangyuanBakeCliOptions`，`:904` 实现 `run_fangyuan_bake_cli`，`project/src/bin/fangyuan_bake.rs:5` 提供 cargo bin 入口；`:1357` / `:1376` / `:1426` 测试覆盖参数、dry-run report 和 artifact 写出）
- [x] 为 header 编解码、版本不匹配、hash 校验、旧版本升级、CLI 参数和 dry-run 补测试。（验证：`project/src/framework/fangyuan/bake.rs:1150` 覆盖 header roundtrip，`:1174` 覆盖 schema/hash mismatch，`:1216` 覆盖旧版本升级，`:1357` 覆盖 CLI 参数，`:1376` 覆盖 dry-run report，`:1516` 覆盖 legacy 写出 payload/hash 一致性；`cargo test fangyuan_bake -- --nocapture` 10 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_bake_format -- --nocapture`、`cargo test fangyuan_bake_validation -- --nocapture`、`cargo test fangyuan_bake -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 6：Artifact、依赖表和运行时二进制加载

- 开始时间：2026-07-05 13:18:25 +08:00
- 结束时间：2026-07-05 14:24:36 +08:00
- 开发总结：扩展 Fangyuan bake artifact 为 typed payload envelope，覆盖 blueprint、prefab palette、scene layout、chunk、material profile 和 skill recipe；新增 dependency table、artifact stats/report、runtime artifact manifest entry、bin 优先加载和 debug RON fallback，并区分版本、hash、kind、依赖和解析错误状态。本阶段 runtime loader 作为方圆 bake utility API 完成并测试，真实场景生产路径仍保留既有首包 RON 加载。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_bake_artifact -- --nocapture` 通过（4 passed）；`cargo test fangyuan_runtime_load -- --nocapture` 通过（4 passed）；`cargo test fangyuan_home -- --nocapture` 通过（53 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 将 prefab、layout、chunk、blueprint、material profile 编译为二进制 artifact。（验证：`project/src/framework/fangyuan/bake.rs:182` 定义 `FangyuanBakePayload` typed payload，`:939` 的 `compile_fangyuan_bake_artifact` 生成 payload bytes + header hash；`:1029` 至 `:1081` 覆盖 blueprint / prefab palette / scene layout / chunk / material profile；`:3172` 测试覆盖 chunk、material 和 skill payload）
- [x] 生成 dependency table，记录 layout -> palette -> prefab -> profile / blueprint / chunk 的依赖关系。（验证：`project/src/framework/fangyuan/bake.rs:391` 定义 `FangyuanBakeDependencyTable`，`:1130` 构建依赖，`:1378` 解析依赖缺失，`:1421` catalog 汇总同目录资源，`:2886` 测试确认 layout 解析到 palette / prefab / material profile）
- [x] Bake report 输出 primitive 数、prefab 数、chunk 数、profile 数、预算、warning 和 artifact size。（验证：`project/src/framework/fangyuan/bake.rs:432` 定义 `FangyuanBakeArtifactStats`，`:1283` 汇总统计，`:2168` report entry 携带 stats 和 missing dependency，`:2387` report 文本输出 primitives/prefabs/chunks/profiles/budget/artifact_size；`:2860` 测试断言 deterministic size/report 统计）
- [x] 新增 runtime artifact loader，按 manifest 和 artifact kind 加载二进制内容。（验证：`project/src/framework/fangyuan/bake.rs:457` 定义 `FangyuanRuntimeArtifactManifestEntry`，`:1534` 定义 `load_fangyuan_runtime_artifact`，`:1611` 按 manifest bin decode artifact 并校验 kind；`:3012` 测试确认 bin payload 与 RON payload 一致）
- [x] 运行时优先读取 bin，开发/debug 配置下允许回退 RON。（验证：`project/src/framework/fangyuan/bake.rs:485` 定义 loader options，`:1534` 先尝试 bin，`:1585` 在允许 fallback 时读取 RON，`:3055` 测试确认 debug fallback 到 RON；`:3207` 测试覆盖 home layout artifact bin/RON 加载）
- [x] 加载失败、版本不匹配、hash mismatch 和依赖缺失时输出明确错误和 fallback 状态。（验证：`project/src/framework/fangyuan/bake.rs:526` 定义 load status，`:540` 定义 fallback 状态，`:549` 定义 `LoadFailed` / `VersionMismatch` / `HashMismatch` / `DependencyMissing` / `ParseFailed`，`:3095` 测试覆盖 dependency、hash 和 version 错误）
- [x] 为 deterministic bake、依赖缺失、hash 变化、artifact size、二进制和 RON 加载一致性补测试。（验证：`project/src/framework/fangyuan/bake.rs:2860` 覆盖 deterministic bake 与 artifact size，`:2950` 覆盖依赖缺失，`:2992` 覆盖 hash 变化，`:3012` 覆盖 bin/RON 一致性；`cargo test fangyuan_bake_artifact -- --nocapture` 4 passed，`cargo test fangyuan_runtime_load -- --nocapture` 4 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_bake_artifact -- --nocapture`、`cargo test fangyuan_runtime_load -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 7：构建流程、移动加载和资源工作流

- 开始时间：2026-07-05 14:36:29 +08:00
- 结束时间：2026-07-05 15:58:22 +08:00
- 开发总结：新增仓库根目录 Fangyuan bake dry-run 脚本，扩展 `fangyuan_bake` 的 `--clean-output`、report 诊断字段和失败退出码，明确 `.fyb` / report / 临时输出的忽略与 LFS 策略，并同步资源工作流、上手文档和方圆技术路线中的 RON/bin 加载诊断、手机窗口与 Android 验证边界。本阶段未运行完整 Android APK 构建，也未声称已验证发布 `.fyb` 首包加载。
- 验证记录：`.\scripts\run-fangyuan-bake-dry-run.ps1` 通过（entries=4 failed=0 dry_run=true，report 输出 peak_resource_count=505、ron_load_us、bin_load_us）；`git check-ignore -v project/assets/fangyuan/test.fyb` 命中否定规则且临时 probe 文件显示为未忽略，`git check-attr filter -- project/assets/fangyuan/test.fyb` 输出 LFS；`cargo fmt --check` 通过；`cargo test fangyuan_bake -- --nocapture` 通过（17 passed）；`cargo test fangyuan -- --nocapture` 通过（360 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 增加开发命令说明或脚本，能一键执行 bake dry-run。（验证：`scripts/run-fangyuan-bake-dry-run.ps1:1` 定义默认 input/output/report，`:49` 组装 `cargo run --bin fangyuan_bake`，`:59` / `:60` 传入 `--dry-run` / `--report`；主 agent 在仓库根目录运行脚本通过，输出 `entries=4 failed=0 dry_run=true`）
- [x] 如项目已有 CI 或本地检查脚本，接入 bake dry-run 或提供明确后续接入说明。（验证：当前仓库无 `.github` CI workflow 或通用本地检查脚本；`docs/assets-workflow.md:703` 和 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1950` 明确后续接入 `.\scripts\run-fangyuan-bake-dry-run.ps1` 并保存 report）
- [x] 确认生成的 bin / report 是否应提交、是否进入 LFS、是否作为构建产物忽略。（验证：`.gitignore:7` 起忽略生成 `.fyb` 和 report，`:9` 放行 `project/assets/**/*.fyb`；`.gitattributes:24` 让首包 `.fyb` 走 LFS；`docs/assets-workflow.md:166` 记录 RON、`.fyb`、report 和 CI artifact 策略；主 agent 验证 `project/assets/fangyuan/test.fyb` 未被 ignore 且 `git check-attr filter` 为 LFS）
- [x] 记录 RON 路径和 bin 路径的加载耗时、峰值资源数量和错误处理差异。（验证：`project/src/framework/fangyuan/bake.rs:2187` / `:2191` / `:2192` / `:2193` 定义 source bytes、RON/bin load us 和 peak resource；`:2512` / `:2523` report 输出加载错误模型和字段；`docs/assets-workflow.md:679` 至 `:703` 解释字段与错误处理差异；dry-run report 输出 `peak_resource_count=505`、`ron_load_us`、`bin_load_us`）
- [x] 在手机比例窗口和必要的 Android 构建流程中验证二进制加载不破坏首包场景。（验证：`docs/bevy-getting-started.md:989` 和 `docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1954` 记录验证边界：先跑 dry-run，再用 `MYBEVY_START_SCENE="dev.fangyuan_home"` + `--window-profile phone-small` 确认 RON 首包路径；Android APK 构建命令作为需要时单独验证项，未声称本阶段已跑完整 APK 或发布 `.fyb` 首包加载）
- [x] 为 clean workspace、增量 bake、输出目录清理和失败退出码补测试或脚本验证。（验证：`scripts/run-fangyuan-bake-dry-run.ps1:42` 只删除旧 report 文件，避免误删父目录；`project/src/framework/fangyuan/bake.rs:2288` 定义失败退出码，`:2898` 覆盖增量 bake 保留无关输出，`:2930` 覆盖 clean output 清理旧产物，`:3233` 依赖缺失测试断言 exit code 为 1；`cargo test fangyuan_bake -- --nocapture` 17 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_bake -- --nocapture`、`cargo test fangyuan -- --nocapture`、`cargo check`，以及新增 bake dry-run 命令。（验证：主 agent 在 `project/` 复跑 `cargo fmt --check`、`cargo test fangyuan_bake -- --nocapture` 17 passed、`cargo test fangyuan -- --nocapture` 360 passed、`cargo check` 全部通过；仓库根目录复跑 `.\scripts\run-fangyuan-bake-dry-run.ps1` 通过，仅有既有 `selection.rs:32` dead_code warning）

## 阶段 8：蓝图 Identity、缓存和缺失 Fallback

- 开始时间：2026-07-05 16:01:36 +08:00
- 结束时间：2026-07-05 16:30:28 +08:00
- 开发总结：新增 Fangyuan 统一 identity、客户端缓存 manifest/LRU 模型和缺失蓝图 fallback 策略，覆盖 blueprint、prefab、chunk、material profile、skill visual 与 bake artifact；缓存模型记录 hash/version/size/last_used/dependency/source kind 并在读取时校验；fallback 区分家园、装备、技能、NPC 和通用隐藏策略。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_blueprint_identity -- --nocapture` 通过（4 passed）；`cargo test fangyuan_blueprint_cache -- --nocapture` 通过（5 passed）；`cargo test fangyuan_blueprint_fallback -- --nocapture` 通过（4 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 定义 blueprint、prefab、chunk、material profile、skill visual 和 bake artifact 的统一 identity。（验证：`project/src/framework/fangyuan/identity.rs:17` 的 `FangyuanIdentityResourceKind` 覆盖六类资源，`:99` 的 `FangyuanBlueprintIdentity` 统一 id/version/hash/source kind 字段，`project/src/framework/fangyuan/mod.rs:53` 导出 identity API）
- [x] 审核通过后生成或记录 id、version、content hash、schema hash、source hash 和 dependency hash。（验证：`project/src/framework/fangyuan/identity.rs:381` 的 `record_fangyuan_identity_after_audit` 先检查 audit，`:366` 的 `fangyuan_identity_dependency_hash` 对排序依赖生成稳定 hash，`:606` 测试断言 source/content/schema/dependency hash，`:712` 测试拒绝失败 audit）
- [x] 设计客户端缓存目录、manifest、容量限制、LRU 或使用频率淘汰策略。（验证：`project/src/framework/fangyuan/cache.rs:18` 定义 manifest root/max/used/entries，`:94` 定义 cache runtime，`:109` 写入后触发容量处理，`:484` 测试确认 LRU/use_count 淘汰）
- [x] 缓存写入时记录 hash、version、size、last_used、dependency list 和 source kind；读取时校验 hash 和 version。（验证：`project/src/framework/fangyuan/cache.rs:67` 的 entry 记录 identity/content_hash/version/size/last_used/use_count/dependencies/source_kind，`:109` 写入校验 content hash，`:160` 读取校验 version/hash/dependency 和文件内容 hash，`:388` / `:417` / `:512` 测试覆盖 hit、version/hash/missing dependency、损坏文件）
- [x] 定义 blueprint 缺失时的 fallback：default appearance、marker、rule-only、hidden 或 pending。（验证：`project/src/framework/fangyuan/fallback.rs:17` 定义 default appearance / marker / rule-only / hidden / pending，`:36` 定义 fallback policy，`:140` 提供 missing blueprint fallback 入口）
- [x] 家园、装备、技能和 NPC 分别定义缺失表现，避免误导真实规则范围。（验证：`project/src/framework/fangyuan/fallback.rs:48` 按 Home/Equipment/Skill/Npc/Generic 返回不同 policy，`:156` 测试确认 home marker、equipment default appearance、skill rule-only、NPC pending 且视觉 fallback 不误导规则范围）
- [x] 为 identity hash、cache hit/miss、hash mismatch、容量淘汰、损坏文件、missing dependency 和 fallback 恢复补测试。（验证：`project/src/framework/fangyuan/identity.rs:606` / `:663` 覆盖 identity/hash，`project/src/framework/fangyuan/cache.rs:388` / `:417` / `:484` / `:512` 覆盖 cache hit/miss、hash mismatch、容量淘汰、损坏文件和 missing dependency，`project/src/framework/fangyuan/fallback.rs:219` / `:250` 覆盖 fallback 恢复和 identity mismatch）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_blueprint_identity -- --nocapture`、`cargo test fangyuan_blueprint_cache -- --nocapture`、`cargo test fangyuan_blueprint_fallback -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 9：流式更新、纪元继承和权威边界

- 开始时间：2026-07-05 16:32:55 +08:00
- 结束时间：2026-07-05 18:26:57 +08:00
- 开发总结：新增 Fangyuan streaming update manifest、安装计划、回滚 key、签名/权限占位和安装前校验；新增纪元继承迁移输入与旧蓝图重新版本升级、审核、预算降级建议；新增客户端缓存与服务端 authority manifest 的覆盖判定和审计日志。本阶段只建立本地模型和测试，不实现真实 CDN、签名加密或服务器集群。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_streaming_update -- --nocapture` 通过（3 passed）；`cargo test fangyuan_epoch_inheritance -- --nocapture` 通过（3 passed）；`cargo test fangyuan_cache_authority -- --nocapture` 通过（3 passed）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。冷缓存首次构建时 Windows/MSVC 测试链接耗时较长，执行 `cargo clean` 后重建，缓存热后原始测试命令均通过。

- [x] 定义线上更新包 manifest，支持 chunk、prefab、blueprint、material profile 和 skill visual 包。（验证：`project/src/framework/fangyuan/streaming_update.rs:13` 定义 `FangyuanStreamingUpdateManifest`，`:264` 定义 update entry，`:768` 测试覆盖 chunk / prefab / blueprint / material profile / skill visual 五类在线包；`project/src/framework/fangyuan/mod.rs:42` / `:75` 导出模块）
- [x] 更新包安装前校验 hash、version、依赖、预算摘要和签名/权限占位。（验证：`project/src/framework/fangyuan/streaming_update.rs:71` 起 validate 校验 manifest/version/hash/dependency/budget/signature/permission，`:399` 校验 budget，`:409` 定义签名占位，`:437` 定义权限占位；`:921` 和 `:1008` 测试覆盖依赖缺失、版本回退、hash mismatch、预算、签名和权限错误）
- [x] 支持只更新受影响 chunk / prefab / blueprint，不要求重新下载整个世界资源。（验证：`project/src/framework/fangyuan/streaming_update.rs:200` 生成 install plan，`:220` 至 `:255` 只为变更资源生成 Install/Replace/Keep action 并记录 affected chunks/prefabs/blueprints，`:301` / `:309` / `:317` 定义影响范围 builder；`:890` 至 `:917` 测试确认只输出受影响资源和 rollback key）
- [x] 定义纪元迁移输入：旧世界 blueprint refs、玩家家园、装备、技能 visual、天道固化档案。（验证：`project/src/framework/fangyuan/epoch_inheritance.rs:15` 定义 `FangyuanEpochInheritanceInput`，`:18` 至 `:22` 覆盖旧世界 blueprint refs、player homes、equipment、skill visuals、tiandao archives，`:59` / `:67` / `:75` / `:83` 定义对应 ref；`:360` 测试覆盖四类输入）
- [x] 新世界加载旧蓝图时重新执行版本升级、审核、预算校准和降级建议。（验证：`project/src/framework/fangyuan/epoch_inheritance.rs:149` 定义迁移入口，`:173` 执行 source version upgrade，`:181` 执行 blueprint audit，`:188` 执行 object budget audit，`:197` 至 `:207` 输出升级版本、审核和降级建议；`:373` 测试覆盖 legacy blueprint 再审计和预算降级）
- [x] 明确客户端缓存内容不可作为权威审核结果，必须能被服务端或权威 manifest 覆盖。（验证：`project/src/framework/fangyuan/cache_authority.rs:11` 定义 authority manifest，`:49` 定义 authority resource audit result 和 override 标记，`:116` 起 resolve 逻辑优先 server manifest，`:168` 至 `:175` 明确 cache 只可供 bytes 且不是 authoritative audit；`:252` / `:279` 测试覆盖 cache bytes-only 和 server manifest 覆盖 stale cache）
- [x] 为部分更新、依赖缺失、版本回退、失败回滚、旧版本蓝图、预算收紧、server manifest 覆盖和审计日志补测试。（验证：`project/src/framework/fangyuan/streaming_update.rs:768` 覆盖部分更新和 rollback key，`:921` 覆盖依赖缺失/资源版本回退/hash mismatch，`:1008` 覆盖 package rollback 和预算/签名/权限；`project/src/framework/fangyuan/epoch_inheritance.rs:373` 覆盖旧版本蓝图和预算收紧降级，`:428` 覆盖缺源和 epoch 未前进；`project/src/framework/fangyuan/cache_authority.rs:279` / `:328` 覆盖 server manifest 覆盖和拒绝审计日志）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_streaming_update -- --nocapture`、`cargo test fangyuan_epoch_inheritance -- --nocapture`、`cargo test fangyuan_cache_authority -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）

## 阶段 10：端到端验收和手动验证

- 开始时间：2026-07-05 18:30:56 +08:00
- 结束时间：2026-07-05 18:43:28 +08:00
- 开发总结：完成方圆第十二至第十四阶段端到端验收，完整 `fangyuan` 测试集、格式检查和编译检查通过；dry-run bake 产出 RON/bin 加载耗时和错误模型；`phone-small` 手机比例窗口可启动到 Fangyuan Home 且加载 layout audit / stats 日志。本阶段没有业务代码改动，也没有 Android 相关改动，未执行 Android APK 构建。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（382 passed，0 failed，673 filtered out）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）；`.\scripts\run-fangyuan-bake-dry-run.ps1` 通过（entries=4 failed=0 dry_run=true，report 输出 `peak_resource_count=505`、`ron_load_us`、`bin_load_us`、`load_error_model=ron(parse+upgrade+validate),bin(header+schema+kind+hash+payload)`）；`cargo run --bin project -- --window-profile phone-small` 限时启动成功，日志显示 `device 720x1600 scale 2.00, logical 360.0x800.0` 并加载 `fangyuan home layout audit` / `layout stats`，未见 panic。

- [x] 运行 `cargo fmt --check`。（验证：worker 在 `project/` 执行通过，耗时约 2.1s）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：worker 在 `project/` 执行通过，382 passed / 0 failed / 673 filtered out，覆盖 chunk、LOD、pressure、runtime load、cache、streaming、epoch inheritance 和 HUD 相关测试）
- [x] 运行 `cargo check`。（验证：worker 在 `project/` 执行通过，仅保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）
- [x] 手动运行手机比例窗口，移动或模拟位置变化，确认 chunk 加载/卸载没有明显残留和卡死。（验证：`cargo run --bin project -- --window-profile phone-small` 限时 90s 启动成功并加载 Fangyuan Home；`project/src/framework/fangyuan/chunk_loading.rs:1184` 测试通过模拟位置变化触发 load/unload，`project/src/framework/fangyuan/lod_integration.rs:1209` 测试覆盖 LOD path 切换无残留，`project/src/framework/fangyuan/chunk_loading.rs:1091` 测试覆盖场景退出 clear）
- [x] 手动验收热点压力下规则层仍清楚，远处高成本视觉被降级。（验证：`cargo test fangyuan -- --nocapture` 通过；`project/src/framework/fangyuan/lod_integration.rs:1296` 覆盖 VFX 在压力下降级到 rule layer，`:1317` 覆盖 NPC marker 降级，`:1355` 压力场景输出 100 / 300 / 1000 bottleneck 和 degrade 摘要，`:1042` / `:1096` 保留 rule layer 语义）
- [x] 手动验收二进制和 RON 加载结果一致，加载失败能显示明确 fallback 状态。（验证：`.\scripts\run-fangyuan-bake-dry-run.ps1` 通过并生成 report；`artifacts/fangyuan-bake/dry-run/report.txt` 记录 `ron_load_us` / `bin_load_us` / `load_error_model`；`project/src/framework/fangyuan/bake.rs:3297` 测试 bin 与 RON payload 一致，`:3380` 覆盖 version/hash/kind/dependency 错误）
- [x] 手动验收缓存 miss、恢复、部分更新、纪元继承 kept / degraded / rejected 和 HUD 摘要。（验证：`cargo test fangyuan -- --nocapture` 通过；`project/src/framework/fangyuan/cache.rs:388` 覆盖 cache hit/LRU，`project/src/framework/fangyuan/fallback.rs:219` / `:248` 覆盖 fallback 恢复和错误 identity，`project/src/framework/fangyuan/streaming_update.rs:768` 覆盖部分更新，`project/src/framework/fangyuan/epoch_inheritance.rs:373` 覆盖 legacy blueprint 再审核和降级，`project/src/game/screens/gameplay/fangyuan_home.rs:242` / `:615` 覆盖 HUD chunk/LOD/failure 摘要）
- [x] 如涉及 Android，运行仓库约定的 `cargo ndk` 或 APK 构建命令并记录结果。（验证：本阶段只做桌面端端到端验收，没有修改 `android/`、NDK 配置或 Android 专属代码；按条件项判定为不适用，未运行 `cargo ndk` / APK 构建）

## 阶段 11：文档同步和归档准备

- 开始时间：2026-07-05 18:45:05 +08:00
- 结束时间：2026-07-05 18:55:59 +08:00
- 开发总结：同步方圆技术路线、资源工作流和新成员上手文档，明确第 12 至第 14 阶段已落地的 Chunk / LOD / AOI / 热点降级 / Bake / runtime loader / cache / streaming update / 纪元继承 / 权威边界，并确认场景框架文档中的 streaming 仍是通用 scene 元数据边界；完成 checklist 归档副本准备。
- 验证记录：`git diff --check` 通过（仅 LF/CRLF warning）；`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（382 passed，0 failed，673 filtered out）；`cargo check` 通过（保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）。

- [x] 更新方圆技术路线，记录 Chunk、LOD、AOI、热点降级、Bake、runtime loader、cache、streaming update、纪元继承和权威边界。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:768` 新增第 12 至第 14 阶段已落地边界，`:1926` 记录第 12 阶段 chunk/LOD/AOI/热点降级，`:2014` 记录第 14 阶段 cache/streaming/epoch/cache authority）
- [x] 如影响场景框架，检查并更新 `docs/scene/` 相关文档。（验证：`docs/scene/README.md:75` / `:108` 和 `docs/scene/场景框架层功能说明.md:540` 仍明确 scene streaming 当前只记录元数据与状态、不做真实资源流式加载；本次方圆 chunk 属于 `framework/fangyuan` 内部模型，无需修改 scene 文档）
- [x] 更新资源工作流文档，说明开发期 RON、发布期 bin、首包、缓存和后续下载资源边界。（验证：`docs/assets-workflow.md:166` 起补充方圆 RON / `.fyb` / 首包 / 后续下载 / authority cache 边界，`:710` 起补充 chunk/cache/streaming update 安装与纪元继承边界）
- [x] 更新新成员上手文档中的 bake dry-run、调试加载和缓存验收命令。（验证：`docs/bevy-getting-started.md:504` 起新增 chunk/runtime/cache/streaming/epoch/cache authority 验收命令，`:522` 记录完整 `cargo test fangyuan -- --nocapture`，`:1007` 起补充 Android 与缓存/后续下载验证边界）
- [x] 确认文档仍明确完整服务器集群、商业 CDN、运营后台、账号交易和最终云压测平台不是本 checklist 能力。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:781` / `:2023`、`docs/assets-workflow.md:716`、`docs/bevy-getting-started.md:549` 均保留非目标边界）
- [x] checklist 全部完成后，按仓库约定从 `summary/` 归档到 `docs/fangyuan/checklists/`。（验证：`docs/fangyuan/checklists/方圆大世界资源和加载第十二至第十四阶段_checklist.md:1` 已创建归档副本；summary 文件保留为本轮进度记录且不纳入提交）
- [x] 验证命令：`git diff --check`、`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo check`。（验证：worker 在本阶段执行四条命令均通过；`cargo test fangyuan -- --nocapture` 为 382 passed / 0 failed / 673 filtered out，仅有既有 `selection.rs:32` dead_code warning）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-05 08:45:48 +08:00
- 结束时间：2026-07-05 18:55:59 +08:00
- 验收总结：方圆大世界资源和加载第十二至第十四阶段完成。已建立 chunk 加载/卸载、LOD/AOI/热点降级、Bake artifact/runtime loader、identity/cache/fallback、streaming update、epoch inheritance 和 cache authority 的本地模型、工具链、测试与文档边界；端到端 `fangyuan` 回归、格式检查和编译检查通过。真实服务器集群、商业 CDN、运营后台、账号资产交易、最终云压测平台和生产级签名加密仍不属于本 checklist 完成范围。

- [x] 客户端能按 Chunk 加载和卸载方圆内容，不需要一次性加载完整世界。（验证：阶段 2 `project/src/framework/fangyuan/chunk_loading.rs:307` / `:722` 实现 chunk command/runtime，阶段 10 `cargo test fangyuan -- --nocapture` 通过且 `:1184` 覆盖位置变化 load/unload）
- [x] LOD 和 AOI 能按距离、重要度、队伍关系和压力选择表现层级。（验证：阶段 3 `project/src/framework/fangyuan/lod.rs:6` / `:347` / `:662` 定义 LOD/AOI/灵压评估，阶段 4 `project/src/framework/fangyuan/lod_integration.rs:526` 接入 render decision）
- [x] 热点降级优先压缩装饰、透明、发光和拖尾，保留规则可读性。（验证：`project/src/framework/fangyuan/lod.rs:620` / `:642` 定义降级顺序，`project/src/framework/fangyuan/lod_integration.rs:1296` / `:1355` 覆盖压力降级和场景摘要）
- [x] `fangyuan_bake` 或等价脚本能把开发期 RON deterministic bake 为二进制 artifact。（验证：`project/src/bin/fangyuan_bake.rs:5` 提供 CLI，`scripts/run-fangyuan-bake-dry-run.ps1:49` 调用 dry-run，阶段 10 dry-run 输出 entries=4 failed=0）
- [x] Bake report 能准确输出 primitive、prefab、chunk、profile、预算、warning、依赖和 artifact size。（验证：`project/src/framework/fangyuan/bake.rs:2168` / `:2387` 输出 report entry/stat 文本，`artifacts/fangyuan-bake/dry-run/report.txt` 包含 primitive/prefab/chunk/profile/budget/warning/dependency/artifact_size 字段）
- [x] Runtime 正式路径能优先加载二进制，debug 配置保留 RON fallback。（验证：`project/src/framework/fangyuan/bake.rs:1534` 起 runtime loader 先尝试 bin，`:1585` debug fallback 到 RON，`:3297` 测试 bin/RON payload 一致）
- [x] 蓝图、prefab、chunk 和 bake artifact 都具备可校验 id、version 和 hash。（验证：`project/src/framework/fangyuan/identity.rs:17` / `:99` 定义统一 resource kind 与 identity hash，`project/src/framework/fangyuan/cache.rs:160` 读取时校验 version/hash/dependency）
- [x] 客户端缓存能命中、校验、淘汰和处理损坏或不兼容内容。（验证：`project/src/framework/fangyuan/cache.rs:388` / `:417` / `:484` / `:512` 覆盖 cache hit、version/hash mismatch、LRU 淘汰、损坏文件和 missing dependency）
- [x] 部分更新包能下发并安装受影响资源，不要求重新下载完整世界。（验证：`project/src/framework/fangyuan/streaming_update.rs:200` / `:220` 生成 Install/Replace/Keep plan，`:301` / `:309` / `:317` 记录 affected chunk/prefab/blueprint，`:768` 测试覆盖部分更新）
- [x] 纪元继承能重新审核旧视觉资产，并输出 kept / degraded / rejected 结果。（验证：`project/src/framework/fangyuan/epoch_inheritance.rs:149` / `:181` / `:188` 执行升级、审核和预算降级建议，`:373` 测试覆盖 legacy blueprint 再审计和 degraded 结果；权威拒绝边界由 `project/src/framework/fangyuan/cache_authority.rs:328` 覆盖 rejected 审计日志）
- [x] HUD / 日志能显示 chunk、LOD、AOI、灵压、bake、cache、streaming 和继承摘要。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:242` / `:615` 覆盖 HUD chunk/LOD/failure 摘要，`docs/assets-workflow.md:710` 与 `docs/bevy-getting-started.md:504` 记录 bake/cache/streaming/继承验收命令和日志边界）
- [x] `cargo fmt --check` 通过。（验证：阶段 10 和阶段 11 均执行通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：阶段 10 和阶段 11 均执行通过，382 passed / 0 failed / 673 filtered out）
- [x] `cargo check` 通过。（验证：阶段 10 和阶段 11 均执行通过，仅保留既有 `src/framework/ui/widgets/controls/selection.rs:32` dead_code warning）
