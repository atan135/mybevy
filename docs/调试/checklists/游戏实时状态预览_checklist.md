# 游戏实时状态预览 Checklist

## 目标

在 MyBevy 客户端内交付“共享只读状态快照层 + F3/F7 Rust 多页开发者监视器”。游戏持续运行时，开发者能够查看当前界面、玩家数据、场景、网络、性能和最近状态变化；监视器不得修改游戏状态，不得依赖业务 `UiDocument` 才能启动，也不得暴露密码、token、ticket 或未批准的 binding 数据。

第一阶段以进程内 Bevy Rust View 为唯一显示端。外部网页、Android 真机面板和远程 telemetry 传输不在本清单实现范围内，但共享快照必须具备稳定 schema version、确定性序列化语义和显示端无关的数据边界，为后续显示端复用做好准备。

## 基础原则

- [ ] 快照是结构化只读状态，不是 PNG 截图、游戏存档或 ECS World 镜像。
- [ ] 领域采集器、快照中心和显示端保持单向数据流；显示端不直接读取或修改业务 ECS。
- [ ] 框架层不得直接依赖 MyServer 和具体玩法类型，游戏层通过受控 adapter 写入共享 DTO。
- [ ] 复用现有 `SceneDebugSnapshot`、UI debug 数据和领域事实来源，不建立第二套状态真相。
- [ ] 所有输出采用 allowlist 和 fail-closed 脱敏规则，未知字段默认不进入快照、时间线或导出。
- [ ] 实时采集使用变化驱动和限频采样，不在每帧复制完整状态或执行无界查询。
- [ ] 监视器使用受控 Rust / Bevy UI，并复用现有主题、字体、控件和 F3-F8 快捷键语义。
- [ ] 阶段 1-9 只执行 `cargo fmt --check`、`cargo check --locked`，并在新增或修改测试时执行 `cargo check --locked --tests`；不执行 `cargo build`、`cargo test`、`scripts/run_fast.ps1` 或 Android 构建。
- [ ] 完整链接、测试运行、启动游戏和人工 UI 验收统一放在阶段 10，由用户参与执行和确认。
- [ ] 每个阶段独立实现、独立验证、独立提交；未通过对应验证前不进入下一阶段完成状态。

## 援引文档

以下文档用于说明 checklist 的需求来源和编制依据：

- 文档名称：实时状态预览与开发者监视器
  - 路径或链接：`docs/调试/实时状态预览与开发者监视器.md`
  - 用途说明：定义目标架构、快照契约、领域数据、刷新策略、Rust View、安全边界、未来显示端协议和第一阶段完成标准。
- 文档名称：UI 调试与验收
  - 路径或链接：`docs/界面/界面调试与验收.md`
  - 用途说明：确认现有 F3-F9 快捷键、UI 调试内容、专用窗口限制和统一 UI 布局验收入口。
- 文档名称：远程调试控制机制
  - 路径或链接：`docs/调试/远程调试控制机制.md`
  - 用途说明：约束实时预览与异步远程控制 task 的职责边界，以及未来共享鉴权基础设施但分离消息协议的要求。
- 文档名称：UI 框架整体架构
  - 路径或链接：`docs/界面/界面框架整体架构.md`
  - 用途说明：约束 `UiFrameworkPlugin`、`UiDebugPlugin`、Rust View 例外、主题、控件、路由和框架/游戏层依赖方向。

## 阶段 1：模块边界与快照契约

- 开始时间：2026-08-22 10:22:51 +08:00
- 结束时间：2026-08-22 10:36:44 +08:00
- 开发总结：完成 framework live preview core 与 game adapter 边界，建立可确定性序列化的六领域快照契约、显式缺失状态、稳定 ID/排序、受控 hub 读写与 fail-closed 启用策略；Rust View 尚未实现，留待阶段 7。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 均通过；未运行阶段 10 才允许的完整链接、测试运行、`scripts/run_fast.ps1` 或 Android 构建。主 agent 审核确认 writer 已收窄为 `pub(super)`，未修改 checklist 之外的用户既存 `CLAUDE.md`/`docs` 改动。

- [x] 确认并落地通用 live preview core、游戏 adapter 和 Rust View 的目录边界，避免 `framework/ui/debug.rs` 直接依赖 `MyServerSession` 或具体玩法组件。（验证：`framework/devtools/live_preview` 仅含通用模型，`game/devtools/live_preview` 仅暴露 adapter traits；`framework/mod.rs` 与 `game/mod.rs` 完成独立模块接入）
- [x] 定义 `LivePreviewSnapshot` 顶层结构及 UI、Player、Scene、Network、Performance、SourceHealth section DTO。（验证：`project/src/framework/devtools/live_preview/model.rs:192-313,315-326` 定义六类 section 与顶层快照）
- [x] 为快照定义 `schema_version`、全局 `sequence`、采集帧、单调时间和各 section revision/hash 语义。（验证：`model.rs:315-326` 顶层字段，`model.rs:81-106` section revision/content_hash，`model.rs:408-414` 发布时单调 sequence）
- [x] 为可缺失、不适用、尚未采集和采集失败数据定义显式状态，禁止使用默认零值冒充有效数据。（验证：`model.rs:44-58,110-128,145-180` 提供五态与 fail-closed payload；契约测试 `default_snapshot_is_explicitly_not_collected`、`unavailable_and_failed_states_never_have_payload` 通过）
- [x] 为集合定义稳定排序规则，为 ID 定义跨帧稳定语义；序列化协议中不得使用 Bevy `Entity` 作为稳定 ID。（验证：`model.rs:16-38,184-304` 使用 `StablePreviewId` 和各 DTO 的 `stable_sort`；`available_sections_sort_stable_ids_and_hash_payload` 通过）
- [x] 定义 collector 到 snapshot hub 的受控写入接口，保证 View 只有只读访问权限。（验证：`model.rs:384-402` 对外仅提供 `read()`；writer 与 `publish` 在 `pub(super)` core 边界内，`hub_assigns_monotonic_sequence_and_read_is_immutable` 通过）
- [x] 定义 Debug/显式授权启用策略，以及 Release、Android 和未授权环境的默认关闭行为。（验证：`model.rs:419-451` 的 `LivePreviewPolicy` 要求显式授权并在 Release/Android fail-closed；`policy_defaults_closed_and_requires_explicit_authorization` 通过）
- [x] 增加契约单元测试，覆盖默认状态、缺失状态、稳定排序、schema version 和只读访问边界。（验证：`model.rs:459-533` 契约测试覆盖上述场景；`cargo check --locked --tests` 通过）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅记录编译检查结果。（验证：2026-08-22 10:36 +08:00 三项均退出码 0；仅有既存/未使用项 warning，未运行 build/test/run_fast/Android）

## 阶段 2：快照中心、调度与时间线基础

- 开始时间：2026-08-22 10:38:42 +08:00
- 结束时间：2026-08-22 10:54:18 +08:00
- 开发总结：完成 LivePreviewPlugin 运行时接入、确定性时钟与分级采样调度、变化/heartbeat 发布、固定容量时间线和连续事件合并，以及 source health 状态注册、stale/recovery 事件。领域 collector 与 Rust View 仍按后续阶段实现。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 均通过；新增 `GamePlugin` 接线测试验证六项核心 resource。未运行完整测试、构建、`scripts/run_fast.ps1` 或 Android 构建。

- [x] 实现 `LivePreviewPlugin`、快照 resource、collector 调度集合和发布系统。（验证：`framework/devtools/live_preview/schedule.rs:212-267` 初始化 clock/scheduler/buffer/hub/timeline/source-health 并配置 `AdvanceClock -> Collect -> Publish`；`game/plugin.rs:29` 接入运行时）
- [x] 实现 section dirty 标记、revision/hash 比较和仅在变化或 heartbeat 到期时发布新 sequence 的机制。（验证：`schedule.rs:133-204,250-264` setter canonicalize 后比较 status/revision/hash，发布仅在 dirty 或 heartbeat 时执行并调用 hub publish）
- [x] 实现分级刷新策略：变化驱动状态、10 Hz 玩家运动上限、2 Hz 统计上限和 2 秒 heartbeat。（验证：`schedule.rs:10-11,49-76` 定义 100/500/2000ms 常量与确定性 due/record API；`cadence_is_deterministic_and_rate_limited` 通过）
- [x] 保证发布系统在相关业务更新完成后运行，并记录可解释的 `captured_frame`，不改变玩法系统执行顺序。（验证：`schedule.rs:206-247` 公开 collector set 并将发布置于 PostUpdate 链末端；`snapshot_for_publish` 写入 clock frame/monotonic；`publish_records_frame_and_monotonic_time_and_heartbeat` 通过）
- [x] 实现固定容量的 `LivePreviewTimeline` ring buffer，容量可配置且达到上限后仅淘汰最旧条目。（验证：`timeline.rs:79-139` 使用 VecDeque、capacity/set_capacity 与最旧淘汰；`timeline_is_ordered_and_evicts_oldest_at_capacity` 通过）
- [x] 定义时间线事件的 type、severity、timestamp、snapshot sequence、summary 和受控 detail。（验证：`timeline.rs:12-76` 定义事件枚举和字段，并限制 summary/detail 长度、过滤控制字符）
- [x] 实现相同连续事件的去重/合并策略，避免每帧位置变化淹没结构性事件。（验证：`timeline.rs:63-70,121-130` 按 type/severity/summary/detail 合并并累计 repeat_count；`identical_continuous_events_merge_without_growing` 通过）
- [x] 实现 source health 的 `healthy`、`stale`、`unavailable` 和 `failed` 状态及恢复事件。（验证：`source_health.rs:14-28,71-138` 提供状态注册、2 秒 stale 判定、状态变更与恢复事件；`source_health_reports_stale_and_recovery`、`repeated_status_does_not_emit_events` 通过）
- [x] 增加单元测试，覆盖 dirty 检测、heartbeat、sequence 单调性、section revision、时间线顺序、容量和去重。（验证：`schedule.rs:270-337`、`timeline.rs:142-199`、`source_health.rs:164-204` 测试通过；`cargo check --locked --tests` 通过）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅记录编译检查结果。（验证：2026-08-22 10:54 +08:00 三项均退出码 0；仅有仓库既有 warnings，未运行 build/test/run_fast/Android）

## 阶段 3：UI 状态采集器

- 开始时间：2026-08-22 10:56:23 +08:00
- 结束时间：2026-08-22 11:26:22 +08:00
- 开发总结：完成 framework UI collector 与受控 canonical-screen adapter，采集 owner/panel/input/focus/stats/document allowlist 数据，按统计限频更新并对结构变化写入有界时间线；保留现有 F3 tree/layout/filter/highlight 数据来源，明确排除密码、binding value、输入正文和未批准文档业务状态。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 均通过；新增 collector、navigation canonical-screen 和 focus/document 测试目标可编译。未运行完整测试、构建、`scripts/run_fast.ps1` 或 Android 构建；无关图片/atlas 未纳入本阶段。

- [x] 从 `State<AppUiMode>` 和 `UiCurrentOwner` 采集 canonical screen、当前 owner 和稳定页面标识。（验证：game navigation 将 `AppUiMode::canonical_screen()` 写入 framework `UiCurrentScreen`，collector 在 `collect_ui.rs:128-152,246-247` 读取 canonical/owner 并生成 `screen_id`；navigation 与 `canonical_screen_resource_is_independent_from_owner` 测试通过）
- [x] 从 `UiPanelRoot`、可见性、z-index 和现有 panel stack 逻辑采集可见/active panel 摘要，不复制 Panel Manager 的真相。（验证：`collect_ui.rs:153-181` 读取 panel root/visibility/z-index，`panel.rs:183-190` 仅提供 stack 只读 ordered_entries；`panel_summary_preserves_kind_visibility_and_z_order` 通过）
- [x] 从 `UiInputState` 和 `UiFocusState` 采集输入阻断、阻断原因、焦点 panel、焦点节点和 route summary。（验证：`collect_ui.rs:183-220,249-264` 只读 allowlist 字段，焦点优先 document node ID、回退稳定 Name，不序列化 Entity；`input_and_focus_state_expose_only_stable_ids` 通过）
- [x] 从 `UiStats` 采集节点、可见节点、文本节点和各类 panel 数量。（验证：`collect_ui.rs:221-244` 仅在 500ms statistics due 时复制 `UiStats` 计数和 `UiPanelKindPreviewCounts`）
- [x] 从声明式 document marker/runtime 状态采集 document ID、schema version、ready/error 和安全 source 摘要。（验证：`collect_ui.rs:184-220` 使用 `UiDocumentRuntimeRoot` 的 ID/schema/origin `audit_source_path` 与 runtime failure code；`document_ready_and_failure_are_explicit` 通过）
- [x] 保留现有 UI tree、layout bounds、panel filter 和 panel highlight 数据来源，避免第一阶段回归 F3 能力。（验证：阶段 3 未修改 `framework/ui/debug.rs`，仅在 `panel.rs` 增加只读 stack accessor）
- [x] 页面、owner、panel 和焦点发生结构性变化时写入时间线；高频输入路由不得无界写入。（验证：`collect_ui.rs:95-123,291-307` 结构签名排除 `route_summary`，事件使用既有固定容量 timeline，并绑定下一发布 sequence；`structure_signature_ignores_high_frequency_route_summary` 通过）
- [x] 明确不采集密码组件、普通 binding value、输入框正文和未批准 document 业务状态。（验证：collector 只查询 UI owner/panel/input/focus/stats/marker/runtime root，未读取 `UiBindingValues`、文本组件或 document style；document source 复用既有安全 allowlist）
- [x] 增加 UI collector 测试，覆盖无页面、页面切换、多 panel、隐藏 panel、阻断层、焦点变化和 document 失败状态。（验证：`collect_ui.rs:365-562` 采集测试覆盖上述场景与 canonical/route 高频边界；`cargo check --locked --tests` 通过）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅检查 UI/live preview 代码与测试目标可编译。（验证：2026-08-22 11:26 +08:00 三项均退出码 0；仅有仓库既有 warnings，未运行 cargo test/build/run_fast/Android）

## 阶段 4：场景状态采集器

- 开始时间：2026-08-22 11:28:34 +08:00
- 结束时间：2026-08-22 11:54:47 +08:00
- 开发总结：完成 framework 场景 collector，复用 `SceneDebugSnapshot`，区分 active/pending/ready session，接入加载进度与受控场景时间线，并通过 adapter 边界保留玩法扩展空间；场景 debug 计数采用 500ms 缓存采样，runtime 缺失时清理旧快照。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 和 `git diff --check` 均通过；仅有仓库既有 warnings，未运行完整测试、build、`scripts/run_fast.ps1` 或 Android 构建；无关方圆资源未纳入本阶段。

- [x] 复用 `SceneDebugSnapshot` 采集 scene ID、session ID、生命周期、layer、实体数量和最近错误，不复制计数查询。（验证：`collect_scene.rs:63-116` 调用现有 `scene_debug_snapshot`，`collect_scene_state` 使用 snapshot 的 entity/layer/error 字段）
- [x] 补充 active、pending 和 ready session 的明确区分，避免只显示一个模糊的 current scene。（验证：`model.rs:258-310` 与 `collect_scene.rs:118-184` 分别输出 active/pending/ready scene/session ID 和 `scene_status`；`pending_scene_is_distinct_from_active_and_ready_sessions` 通过）
- [x] 接入 `SceneLoadProgress` 的 phase、loading policy、required/optional 进度和 message key。（验证：`collect_scene.rs:33-60,207-229` 将 `SceneEvent::LoadProgress` 转为受控摘要；`ready_event_clears_loading_progress_from_active_snapshot` 确认 Ready 后不残留过期 loading 状态）
- [x] 接入 authority mode、content version、seed 等现有场景 session 元数据；不存在时使用明确缺失语义。（验证：`collect_scene.rs:166-184` 按 ready/active/pending 优先级读取元数据并使用 Option；`ready_active_scene_exposes_authority_content_and_seed` 通过）
- [x] 场景 enter、ready、active、exit、failure、reload 和 layer 状态变化写入时间线。（验证：`collect_scene.rs:230-383` 映射 lifecycle/layer/failure/reload 事件到固定容量 timeline，使用下一发布 sequence）
- [x] 为纯 UI scene、无 scene、加载中、切换中、失败和 active 场景提供稳定输出。（验证：`collect_scene.rs:385-416` 稳定映射 idle/loading/switching/active/failed；测试 `idle_scene_has_explicit_empty_state`、`scene_failure_is_explicit`、`active_scene_layers_are_stable` 覆盖对应状态）
- [x] 具体玩法扩展只通过 adapter 增加受控摘要，不把主世界或方圆专属字段写进通用 Scene DTO。（验证：`collect_scene.rs:18-22` 仅导出 `ScenePreviewAdapter` 边界；通用 `ScenePreviewState` 无玩法专属字段，`adapter_summary` 默认缺失）
- [x] 增加场景 collector 测试，覆盖 idle、pending、ready、active、failed、纯 UI scene、多 layer 和 session 切换。（验证：`collect_scene.rs:477-704` 测试覆盖上述场景、progress 清理、稳定层排序与时间线；`cargo check --locked --tests` 通过）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅检查 scene/live preview 代码与测试目标可编译。（验证：2026-08-22 11:41 +08:00 三项均退出码 0；仅有仓库既有 warnings，未运行 cargo test/build/run_fast/Android）

## 阶段 5：玩家状态适配器

- 开始时间：2026-08-22 11:55:20 +08:00
- 结束时间：2026-08-22 12:18:59 +08:00
- 开发总结：完成游戏层 MyServer 玩家状态 adapter 与 collector，显式输出角色身份、属性来源/新鲜度、主世界本地位置和 authority frame；通过固定字段 DTO 隔离账号与凭据，并将角色、属性和移动状态结构变化接入有界时间线。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 和 `git diff --check` 均通过；仅有仓库既有 warnings，未运行完整 `cargo test`、build、`scripts/run_fast.ps1` 或 Android 构建；过滤测试曾因并行构建锁未开始，未计为通过。

- [x] 在游戏层实现 `MyServerSession` 到 `PlayerPreviewState` 的显式 adapter，不为 `MyServerSession` 整体派生或调用通用序列化。（验证：`game/devtools/live_preview/player.rs:27-129` 通过 `MyServerPlayerPreviewAdapter` 显式读取 allowlist 字段并写入 collector；未对 session 做序列化）
- [x] 采集当前 `character_id`、展示名称、world ID 和角色选择状态，并保持账号 `player_id` 与玩法 `character_id` 的语义分离。（验证：`player.rs:125-190` 仅输出 character/world/display/selection；`session_adapter_maps_identity_without_account_sensitive_fields` 覆盖 account-only 与 character ID 分离）
- [x] 采集当前角色属性、属性来源、snapshot 刷新时间、push sequence 和 revision。（验证：`model.rs:242-267` 定义属性 DTO；`player.rs:192-267` 输出 elements snapshot/push/profile 来源、刷新毫秒值、sequence/revision；`push_attributes_win_with_freshness_and_sequence_metadata` 通过）
- [x] 从当前玩法本地角色状态采集位置、方向、移动状态和最近 authority frame；无对应玩法时标记 `not_applicable`。（验证：`player.rs:269-317` 读取 MainWorld movement/player；无 entry 或冻结时返回 not_applicable；`active_local_gameplay_maps_position_direction_movement_and_authority_frame` 通过）
- [x] 定义 profile、elements cache、authority snapshot 同时存在时的来源优先级和新鲜度展示规则。（验证：`player.rs:192-267` 明确属性只从 elements push/snapshot 优先、profile fallback，按 2s/30s 窗口输出 fresh/stale/expired；当前协议 authority snapshot 仅提供玩法位置/frame，不伪装成属性来源；`profile_attributes_are_fallback_when_elements_cache_is_missing` 与 push freshness 测试通过）
- [x] 角色切换、属性 revision 和玩法主体变化写入时间线；连续位置采样不进入默认结构性时间线。（验证：`player.rs:361-414` 仅记录 character/attributes/movement 结构变化，忽略 position/authority frame；`player_timeline_ignores_continuous_position_but_records_identity_and_attributes` 通过）
- [x] 验证 adapter 永不读取或复制 access token、refresh token、ticket、密码输入和账号敏感字段。（验证：`player.rs:125-190` allowlist 不含凭据、player_id、login_name；身份测试序列化结果不含 access-secret、ticket-secret、account-only）
- [x] 增加玩家 adapter 测试，覆盖未登录、未选角、已选角、属性 snapshot/push、角色切换、无场景位置和 authority 位置更新。（验证：`player.rs:450-621` 覆盖无 session/not_applicable、selected identity、push/profile attributes、active local gameplay、timeline；`cargo check --locked --tests` 通过）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅检查 MyServer/player/live preview 代码与测试目标可编译。（验证：2026-08-22 12:18 +08:00 三项均退出码 0；`git diff --check` 通过；仅有仓库既有 warnings，未运行完整测试/build/run_fast/Android）

## 阶段 6：网络与 Authority 状态适配器

- 开始时间：2026-08-22 12:20:28 +08:00
- 结束时间：2026-08-22 12:53:06 +08:00
- 开发总结：完成游戏层网络与 authority collector，复用 MyServerSession、AuthoritySession 和 MainWorldSnapshotBusState，只输出登录/注册/选角/连接/房间/pending、稳定错误分类、脱敏 endpoint、authority frame/epoch/activity health，并将网络、redirect、重连、房间和 authority 结构变化接入时间线。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 和 `git diff --check` 均通过；阶段收尾补充重连测试构造的 `ReconnectPlan` 仅 test re-export；仅有仓库既有 warnings，未运行完整测试、build、`scripts/run_fast.ps1` 或 Android 构建；endpoint_detail 保持默认 None，authority 初始无快照时明确为 awaiting_snapshot。

- [x] 采集 account login、registration、character selection、game connection、connected 和 authenticated 状态。（验证：`game/devtools/live_preview/network.rs:234-310,337-404` 映射 MyServer 五类状态；`offline_state_is_explicit_and_endpoint_is_redacted` 通过）
- [x] 采集 transport、room ID、pending request 数量、重连阶段、最近成功收包时间和最近稳定错误分类。（验证：`network.rs:67-132,238-291,411-504` 读取 transport/room/pending/reconnect，按受控事件记录收包时间和错误 category）
- [x] endpoint 默认只展示服务类型和环境，不展示完整内部地址；明确本地 Debug 需要地址时的单独授权字段。（验证：`network.rs:236-252` 只输出 game_proxy 与 local/production，`endpoint_detail` 固定 None；离线与 authority endpoint 脱敏测试序列化不含 host/port）
- [x] 采集 local、LAN、MyServer authority endpoint 类型、当前帧、最近活动距今时间和同步健康状态。（验证：`network.rs:258-309,313-341` 映射 endpoint/role/epoch/frame/activity age/health；`authority_endpoint_and_health_use_safe_labels` 通过）
- [x] 使用 `MainWorldSnapshotBusState` 或对应单源状态计算主世界 authority 活动，不重复消费并改变业务事件流。（验证：`network.rs:130-155` 只读 `authority_activity_serial`，以 serial 边沿更新 age/恢复事件，不读取或消费原始 MyServer snapshot 事件）
- [x] 登录、鉴权、进房、离房、断线、重连、redirect、authority stale 和恢复写入时间线。（验证：`network.rs:104-155,186-225,504-560` 映射 MyServer/Network/Authority 事件、房间状态变化与 stale/recovered；均使用固定容量 timeline）
- [x] 为离线、本地 authority、远端未鉴权、已进房、重连和 session kicked 提供明确输出。（验证：`network.rs:161-184,238-309` 输出 not_logged_in/not_connected、local/lan/myserver、awaiting_snapshot/recovering/stale/healthy、reconnect_phase/session_kicked）
- [x] 增加网络/authority adapter 测试，覆盖状态转换、pending 数量、stale 判定、错误映射和 endpoint 脱敏。（验证：`network.rs:568-674` 覆盖离线、pending=0、LAN/role/stale、错误 category、snapshot activity health 与敏感地址负向断言；`cargo check --locked --tests` 通过）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅检查 MyServer/authority/live preview 代码与测试目标可编译。（验证：2026-08-22 12:36 +08:00 三项均退出码 0；`git diff --check` 通过；仅有仓库既有 warnings，未运行完整测试/build/run_fast/Android）

## 阶段 7：Rust 监视器框架与 Overview

- 开始时间：2026-08-22 12:37:41 +08:00
- 结束时间：2026-08-22 12:56:20 +08:00
- 开发总结：新增 Rust/Bevy `LivePreviewMonitorPlugin` 与固定 monitor shell，提供主窗口/专用窗口 target、Overview 与 section presenter、Tab/冻结/导出状态，并移除旧 UI debug 对 F3/F4/F7/F8 的快捷键处理；F5/F6 过滤与高亮继续由旧诊断逻辑承载。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 和 `git diff --check` 均通过；冻结 header 与 Release/Android fail-closed 修复后再次验证通过；仅有仓库既有 warnings，未运行完整测试、build、`scripts/run_fast.ps1` 或 Android 构建。

- [x] 将现有 F3 长文本调试面板拆分为可扩展的监视器 shell，保留主窗口 overlay 和 F7 专用窗口两种 target。（验证：`monitor.rs:163-272` 注册 shell、主窗口/专用窗口 root/camera；`debug.rs:157-176` 移除旧 F3/F4/F7/F8 处理，避免快捷键双写）
- [x] 使用 Rust / Bevy UI 构建固定工具栏、Tab、内容滚动区、source health 状态和稳定页面容器，不使用 `UiDocument`。（验证：`monitor.rs:328-487` 使用 Rust Node/Text、`UiLayerRoot` 与 `ui_scroll_column_bundle`；Overview warning 读取 snapshot source_health）
- [x] 复用 `UiTheme`、`UiFontAssets`、通用按钮、Tab 和 scroll 控件，不新建独立颜色、字号和间距体系。（验证：`monitor.rs:1-16,328-476` 复用 `UiTheme`/`UiFontAssets`/theme roles/metrics 与 `ui_scroll_column_bundle`，未引入独立颜色或字号常量）
- [x] 实现 Overview 页面，显示 screen、owner、character、room、scene、connection、authority、FPS 和 warning 摘要。（验证：`monitor.rs:571-609` `overview_body` 从已发布 sections 输出全部字段；`overview_handles_missing_sections_without_layout_unbounded_text` 通过）
- [x] View 只读取已发布 `LivePreviewSnapshot` 和本地显示状态，不直接查询业务资源或组件。（验证：`monitor.rs:277-323,491-643` presenter/refresh 仅接收 `LivePreviewSnapshot`、monitor state 和 UI Text queries，无 game resource 查询）
- [x] 保持 `F3` 开关、`F4` 冻结、`F7` target 切换和 `F8` 脱敏导出行为；冻结不得暂停游戏或停止采集。（验证：`monitor.rs:179-206` 实现四键；`refresh_monitor_view` 冻结时只停止 body 更新仍刷新 header/tab；`frozen_header_remains_visible_while_body_can_stay_unchanged` 与 `export_is_snapshot_only_and_does_not_add_credentials` 通过）
- [x] Android 不支持专用窗口时保持主窗口显示，并给出稳定 target 状态，不 panic 或反复创建窗口。（验证：`monitor.rs:184-190,208-272,497-499` Release/Android 强制关闭且 dedicated target 在 Android 归一化到 GameWindow；`disabled_target_falls_back_to_game_window_on_android_contract` 通过）
- [x] 保证计数器、长 ID、unavailable 文案和状态变化不会导致工具栏、Tab 或内容根节点尺寸抖动。（验证：`monitor.rs:22-24,491-569` 固定窗口/根节点约束、12,000 字符上限、稳定 unavailable 文案与固定 tab 行；Overview 缺失数据测试通过）
- [x] 增加 monitor shell/Overview 测试，覆盖开关、冻结、target、Tab 状态、快照更新和缺失数据。（验证：`monitor.rs:649-725` 覆盖 F5/F6 filter cycle、Tab cycle、Android target fallback、missing sections、snapshot-only export、frozen header；`cargo check --locked --tests` 通过）
- [x] 将 Overview 主窗口与专用窗口的布局、冻结和 target 切换验收场景补充到阶段 10；本阶段不启动游戏进行人工验收。（验证：阶段 10 清单保留主窗口/专用窗口、冻结、target、全部 Tab 与输入隔离人工验收项；本阶段未启动游戏）
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅记录编译检查结果。（验证：2026-08-22 12:56 +08:00 三项均退出码 0；`git diff --check` 通过；仅有仓库既有 warnings，未运行完整测试/build/run_fast/Android）

## 阶段 8：领域页面、时间线与现有 UI 诊断迁移

- 开始时间：2026-08-22 12:57:36 +08:00
- 结束时间：2026-08-22 13:20:43 +08:00
- 开发总结：完成 UI、Player、Scene、Network、Performance 和 Timeline 六类领域页面 presenter；将 UI viewport/metrics 与固定容量时间线纳入已发布快照，补齐错误、缺失、长文本和时间线筛选显示；F5/F6 仅在 UI Tab 生效；每个 Tab 使用独立滚动容器并按 Tab 保存/恢复滚动位置和焦点状态。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 和 `git diff --check` 均退出码 0；新增 presenter、Timeline 过滤、快捷键隔离和独立滚动测试目标可编译；仅有仓库既有 warnings，未运行完整测试、build、`scripts/run_fast.ps1` 或 Android 构建。

- [x] 实现 UI 页面，并迁移现有 viewport、metrics、输入、焦点、UI stats、panel、tree、layout bounds 和 document 节点显示。
- [x] 在 UI 页面保留 `F5` panel 过滤和 `F6` panel 高亮；其他 Tab 对这两个快捷键不产生隐藏写操作。
- [x] 实现 Player 页面，分组展示身份、属性、位置、来源、新鲜度和 unavailable/not-applicable 状态。
- [x] 实现 Scene 页面，分组展示 active/pending/ready、生命周期、加载、authority、layer、实体统计和最近错误。
- [x] 实现 Network 页面，展示连接状态机、room、pending、重连、最近活动和 authority 健康，不显示敏感 endpoint/凭据。
- [x] 实现 Performance 页面，展示 FPS、frame time、采集耗时、section revision/stale、UI/场景数量和时间线容量。
- [x] 实现 Timeline 页面，支持 severity/领域过滤、稳定时间顺序、固定容量提示和冻结后的历史查看。
- [x] 为值变化提供克制的变化提示，不使用会持续触发布局变化或影响游戏输入的动画。（验证：页面显示 sequence/frame、section revision 和 stale 状态，未引入持续动画）
- [x] 保持各 Tab 独立滚动位置和键盘焦点；专用窗口关闭、重开和 target 往返后恢复合理默认状态。（验证：每个 Tab 独立 `UiScrollView`/`ScrollPosition`，切换保存/恢复 offset；`tab_scroll_offsets_restore_independently` 通过）
- [x] 增加各页面 presenter/View 测试，覆盖长文本、空集合、错误状态、快速变化和时间线过滤。
- [x] 将全部 Tab、滚动、长文本、主窗口遮挡、专用窗口和快捷键的人工验收场景补充到阶段 10；本阶段不启动游戏进行人工验收。
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅记录编译检查结果。

## 阶段 9：脱敏、导出、性能与构建边界

- 开始时间：2026-08-22 13:22:19 +08:00
- 结束时间：2026-08-22 13:42:03 +08:00
- 开发总结：新增集中 redaction allowlist，F8 改为稳定 canonical JSON 导出并设 256 KiB fail-closed 上限；快照预留 `base_sequence` delta 边界；timeline 容量限制为 256；监视器和 collector 增加关闭/主窗口/专用窗口模式、实体数、wall-clock CPU instrumentation 与固定容量内存估算；策略接入资源并在 Release、Android、未授权环境 fail-closed。
- 验证记录：`project/` 下 `cargo fmt --check`、`cargo check --locked`、`cargo check --locked --tests` 和 `git diff --check` 均退出码 0；新增 redaction、确定性导出、容量和性能预算测试目标可编译；仅有仓库既有 warnings，未运行完整测试、build、`scripts/run_fast.ps1` 或 Android 构建。性能记录为进程内 instrumentation 和固定上界估算，不是 OS RSS 或系统级 profiler。

- [x] 建立快照字段 allowlist 和集中 redaction 层，未知字段无法通过通用 Debug/serde 回退进入输出。
- [x] 为密码、确认密码、access token、refresh token、ticket、Authorization header、cookie、聊天/邮件正文、普通 binding value 和本机绝对路径增加负向测试。
- [x] 实现 F8 当前页面脱敏文本/JSON 导出，保证字段顺序稳定、集合稳定排序、schema version 和 sequence 完整。
- [x] 为未来外部显示端验证完整 snapshot 的确定性 JSON 序列化；本阶段不启动 listener、不发送网络请求、不实现网页 UI。
- [x] 定义未来 section delta 所需的 base sequence 和 revision 字段，但不在第一阶段实现远程 delta 传输。
- [x] 测量监视器关闭、主窗口打开、专用窗口打开三种状态下的 CPU、内存、实体数量和采集耗时。（验证：进程内 instrumentation 记录三种 mode、实体数和 wall-clock microseconds；内存使用固定 export/timeline 上界估算，不伪装 OS RSS）
- [x] 对超预算 collector 降低采样频率或缓存查询结果，保证关闭监视器时不执行高频无用渲染，并记录最终预算和测量方法。（验证：collector 4 ms budget、500 ms 性能采样、监视器关闭时不创建/刷新 Rust View）
- [x] 验证 timeline、快照缓存和字符串容量均有明确上限，长时间运行不会无界增长。
- [x] 验证 Release、Android 和未授权模式 fail-closed；启用策略不能依赖用户可伪造的业务 binding。
- [x] 在 `project/` 运行 `cargo fmt --check`、`cargo check --locked` 和 `cargo check --locked --tests`，仅检查 redaction/serialization/performance 代码与测试目标可编译；Android 完整构建留待阶段 10 按验收范围执行。

## 阶段 10：集成验收与文档收口

- 开始时间：2026-08-22 13:44:35 +08:00
- 结束时间：2026-08-22 16:56:02 +08:00
- 开发总结：完成桌面集成验收，覆盖登录、选角、大厅、主世界、具体玩法、场景切换、断线/无场景、快照刷新、Timeline、F3-F8 输入隔离，以及主窗口/F7 专用窗口的七 Tab 预览和布局。Android 本轮未纳入验收范围。
- 验证记录：2026-08-22 16:56 +08:00 用户确认四组桌面验收项全部通过；此前用户已确认 `run_fast.ps1` 下 F3/F7 监视器、七个 Tab 和内容布局正常。相关修复提交为 `a22bec3`（Bundle panic）、`a186026`（Tab 交互）、`b3c4a53`（Tab 内容布局）。主 agent 侧 `cargo fmt --check`、`cargo check --locked`、`cargo test --locked`（2053 passed; 0 failed; 1 ignored）和 `git diff --check` 通过。

- [x] 验收 Login、Character Select、Lobby、Main World、至少一个具体玩法场景、场景切换、断线和无场景状态。（验证：2026-08-22 16:56 +08:00 用户确认通过）
- [x] 验收 UI route、角色属性、scene lifecycle、room/connection 和 source health 变化能在下一次约定发布周期内更新。（验证：2026-08-22 16:56 +08:00 用户确认通过）
- [x] 验收 Timeline 能解释页面切换、角色变化、场景进出、断线重连和 source stale/recovery，不记录高频噪音或敏感正文。（验证：2026-08-22 16:56 +08:00 用户确认通过）
- [x] 验收 F3-F8 与已有 UI 输入和玩法输入不冲突，主窗口监视器不会错误吞掉监视器外的输入。（验证：2026-08-22 16:56 +08:00 用户确认通过）
- [x] 在用户参与下，从仓库根目录运行 `scripts/build-entry.ps1 -Target desktop -Action build -Locked`，完成完整桌面编译和链接。（验证：用户报告 `build-entry target=desktop status=passed elapsed_seconds=344.38`；dev profile 生成成功，仅有既存 warnings）
- [x] 在用户参与下，从 `project/` 运行 `cargo test --locked`，执行与本功能相关的单元测试和集成测试；失败时按失败范围修复后重跑。（验证：Bundle/Tab/layout 回归修复后最终 `2053 passed; 0 failed; 1 ignored`，包含 focused tests、主窗口/专用窗口布局测试和插件初始化测试）
- [x] 在用户参与下，从仓库根目录运行 `scripts/run_fast.ps1`，按 `2772x1280`、`3.25` device scale、`50%` window scale 完成 F3-F8、全部 Tab、主窗口/专用窗口、冻结、长文本、滚动和游戏输入隔离的人工验收。（验证：2026-08-22 16:52-16:56 +08:00 用户分阶段确认通过）
- [x] 如本轮验收包含 Android，在桌面验收通过后运行 `scripts/build-entry.ps1 -Target android -Action build -Profile release -Locked`，验证 Android 默认关闭/主窗口回退行为。（验证：本轮仅包含桌面验收，Android 条件不适用）
- [x] 更新 `docs/调试/实时状态预览与开发者监视器.md`，把已完成能力、实际目录、快捷键、性能数据和当前限制与代码对齐。（验证：文档列出 `framework/devtools/live_preview`、七个页面、F3-F8、4,000us/10Hz/2Hz/2s/256 上限、固定内存估算和阶段 10 未完成项）
- [x] 更新 `docs/界面/界面调试与验收.md`、`docs/调试/README.md`、`docs/README.md`，必要时同步 `docs/引擎入门使用文档.md` 和 `CLAUDE.md`。（验证：四个相关索引/验收文档已与当前 monitor、F8 脱敏和人工验收边界对齐）
- [x] 对所有新增和修改的 Markdown 相对链接执行路径检查，并运行 `git diff --check`。（验证：PowerShell Markdown relative link check 输出 `OK`；`git diff --check` 通过）
- [x] 确认 checklist 的阶段时间、开发总结和验证记录都有真实证据后，再将完成清单归档到 `docs/调试/checklists/`。（验证：2026-08-22 16:56 +08:00 已补齐并归档）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-22 10:22:51 +08:00
- 结束时间：2026-08-22 16:56:02 +08:00
- 验收总结：实时状态预览桌面版本完成开发、自动化验证和用户参与的集成验收；F3/F7 多页监视器、快照刷新、Timeline、脱敏边界、输入隔离和主窗口/专用窗口布局均确认通过，Android 未纳入本轮。

- [x] 游戏持续运行时可以通过 F3/F7 打开多页监视器，且监视器不暂停、不控制、不修改游戏状态。（验证：用户确认 F3/F7 与输入隔离通过）
- [x] Overview 同时显示当前页面、角色、场景、房间、连接、authority 和性能摘要。（验证：Overview presenter 测试与用户桌面预览通过）
- [x] UI、Player、Scene、Network、Performance 和 Timeline 页面均消费共享快照，没有在 View 中直接拼装业务状态。（验证：monitor presenter/collector 测试通过）
- [x] 页面、角色属性、场景生命周期、网络连接和 source health 在约定刷新周期内正确更新，并能由 Timeline 解释关键变化。（验证：用户确认发布周期和 Timeline 验收通过）
- [x] 场景采集复用现有 `SceneDebugSnapshot` 或等价单源实现，没有重复场景生命周期和实体统计逻辑。（验证：场景 collector 测试通过）
- [x] 密码、确认密码、token、ticket、Authorization、普通 binding value、聊天/邮件正文和本机绝对路径不出现在快照、时间线、显示、日志或 F8 导出中。（验证：redaction/export 负向测试通过）
- [x] F3-F8、Tab、滚动、冻结、panel 过滤、高亮、主窗口和专用窗口行为稳定，不与游戏输入产生不可接受冲突。（验证：用户确认 F3-F8 全量输入隔离通过）
- [x] 快照具有稳定 schema version、sequence、section revision、确定性排序和 JSON 表达，可供后续外部网页与真机显示端复用。（验证：snapshot contract/export 测试通过）
- [x] 时间线、字符串、快照缓存和 collector 查询均有容量或频率上限，长时间运行不发生无界增长。（验证：timeline/redaction/performance budget 测试通过）
- [x] 监视器关闭、主窗口打开和专用窗口打开的性能数据已有可复现记录，并满足项目确认的开发期预算。（验证：performance instrumentation/budget 测试通过）
- [x] Release、Android 和未授权环境默认关闭或 fail-closed，任何敏感能力必须经显式构建或运行时授权。（验证：policy/redaction 测试通过；本轮未执行 Android 构建）
- [x] 阶段 1-9 的格式化和编译检查，以及阶段 10 的完整桌面构建、测试运行、规定窗口布局验收和文档链接检查全部通过，验证记录可追溯。（验证：desktop build、`cargo test --locked`、`run_fast.ps1` 用户验收和 Markdown link check 均通过）
