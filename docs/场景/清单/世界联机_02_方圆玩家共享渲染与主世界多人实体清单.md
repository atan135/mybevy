# 方圆玩家共享渲染与主世界多人实体 清单

## 目标

从现有方圆玩家预览中提取可复用的方圆玩家运行时渲染能力，在 `world.main` 中根据 MyServer 权威 `EntityTransform` 生成本地和远端玩家，并建立以 `character_id` 为主键、`entity_id` 为重建判据、受 scene session 管理的多人实体生命周期。

当前所有联网玩家暂时复用 `fangyuan/avatars/minimal_player.ron`。本清单不实现移动输入、预测、插值算法或摄像机操控，只提供这些系统所需的稳定玩家实体和权威状态入口。

## 依赖与边界

- 需求来源：`summary/主世界尺度角色摄像机与联机移动需求设计.md`。
- 共享渲染和 fixture 测试不需要登录；如必须进入主世界验证，优先使用正式服登录，不默认使用本地服务端。
- 依赖现有 `FangyuanAvatar`、`FangyuanPrimitiveSet`、`FangyuanRenderAssetCache` 和 blueprint 加载能力。
- 依赖主世界进场协调器提供 active scene session、本地 `character_id`、generation 和 gameplay ready 门槛。
- 可以使用构造的 `MovementSnapshotPush` fixture 完成客户端开发，不阻塞于服务端 4000 米配置清单。
- 本阶段不定义正式角色外观协议，不按账号 `player_id` 创建玩法实体。

## 基础原则

- [x] 共享方圆渲染只解释 blueprint 和 primitive，不感知房间加入、网络重连或主世界 UI。（验证：`framework/fangyuan/runtime.rs` 无 game/MyServer 依赖，主世界生命周期全部位于 game scene adapter）
- [x] Preview 和主世界必须复用同一视觉生成路径，不保留两套 primitive 解释实现。（验证：Preview 与主世界均调用 `spawn_fangyuan_player`，26 项 Preview 与 17 项玩家测试通过）
- [x] 一个 scene session 内，同一 `character_id` 最多对应一个玩家根实体。（验证：`MainWorldPlayerRegistry` 以 character ID 为 key，唯一更新和重复快照测试通过）
- [x] 玩家根实体承载世界位置和身份，primitive 子实体只保留 blueprint 内部相对 Transform。（验证：根持有 `MainWorldPlayer`/权威 Transform，runtime parenting/局部 Transform 测试通过）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（验证：阶段 1-7 均记录测试证据并产生独立业务提交，summary/清单 未进入提交）

## 阶段 1：定义共享方圆玩家运行时边界

- 开始时间：2026-08-12 13:19:42 +08:00
- 结束时间：2026-08-12 13:44:31 +08:00
- 开发总结：抽取 framework 级方圆玩家运行时，共享玩家根组件、primitive 视觉生成、Transform 同步和 RenderAssetCache，Preview 改为使用共享插件。
- 验证记录：`cargo test fangyuan_player_preview --lib` 通过（24 passed, 0 failed）；`cargo check --lib` 与 `cargo fmt -- --check` 通过。

- [x] 确定共享模块路径和公开给 game layer 的最小组件、命令或生成 API。（验证：新增 `framework/fangyuan/runtime.rs` 并由 `fangyuan/mod.rs` 导出 `FangyuanPlayerRuntimePlugin`、共享组件及 `spawn_fangyuan_player`）
- [x] 将 `FangyuanPlayer`、玩家状态、视觉生成标记和 RenderAssetCache 的职责从 Preview 私有语义中分离。（验证：相关组件与 `FangyuanPlayerRenderAssets` 已迁入共享 runtime，Preview 删除私有实现）
- [x] 定义玩家根实体与 primitive 视觉子树的所有权、可见性和 Transform 同步契约。（验证：共享系统将 render-only primitive 作为玩家根 child，子实体 `Visibility::Visible`，根 `FangyuanPlayerPosition/ObjectState/Transform` 在 Propagate 前同步）
- [x] 定义共享 API 如何接收 blueprint ID、显示名、初始 Transform 和额外 game-layer bundle。（验证：`spawn_fangyuan_player<B: Bundle>` 接收上述参数，runtime 测试核验 avatar、Transform 与 extra component）
- [x] 保持 framework fangyuan 数据模型不依赖主世界或 MyServer 类型。（验证：runtime 仅导入 framework Fangyuan 类型与 Bevy，无 game-layer 协议依赖）
- [x] 增加模块边界测试，确保共享运行时不导入 navigation、main_world_entry 或 MyServer 协议。（验证：`shared_runtime_does_not_reference_game_or_network_modules` 检查无 `crate::game`、`MyServer`、`main_world` 引用）
- [x] 运行方圆玩家定向测试和 `cargo fmt -- --check`。（验证：Preview 相关 24 tests passed；格式检查通过）

## 阶段 2：重构 Preview 使用共享渲染能力

- 开始时间：2026-08-12 13:45:34 +08:00
- 结束时间：2026-08-12 14:03:18 +08:00
- 开发总结：Preview 通过共享 `spawn_fangyuan_player` 创建玩家根实体，使用 Preview owner marker 实现实例级去重和页面生命周期。
- 验证记录：`cargo test fangyuan_player_preview --lib` 通过（25 passed, 0 failed）；`cargo check --lib` 与 `cargo fmt -- --check` 通过。

- [x] 将 Preview 的 primitive Mesh/Material 创建迁移到共享方圆玩家运行时。（验证：Preview plugin 添加 `FangyuanPlayerRuntimePlugin`，不再保留私有 primitive spawn 系统）
- [x] 保留 Preview 专属的 OnEnter adapter、`DespawnOnExit`、预览摄像机和预览灯光。（验证：`spawn_fangyuan_preview_player` 仍由 OnEnter 调用，额外 bundle 挂 `DespawnOnExit(AppUiMode::FangyuanPlayerPreview)`；相机/灯光模块未改）
- [x] 移除“全局存在任一 FangyuanPlayer 就不生成”的单例假设，改为 Preview owner 或实例级去重。（验证：查询改为 `FangyuanPlayerPreviewOwner`；`existing_non_preview_player_does_not_block_preview_owned_player` 验证总玩家 2、Preview owner 1）
- [x] 保证 Preview 当前最小玩家外观、primitive 数量、颜色、透明度和 Transform 不发生行为回退。（验证：25 项 Preview 测试覆盖默认 primitive、Transform、颜色/alpha、材质和子实体数量）
- [x] 验证共享 Mesh/Material 缓存在相同 primitive 和颜色下复用 asset handle。（验证：Preview 测试核验相同 cube/sphere mesh handle 和相同颜色 material handle）
- [x] 运行 `cargo test fangyuan_player_preview --lib`、相关方圆测试和 `cargo check`。（验证：25 passed；相关 render_assets 测试在同一过滤套件通过；`cargo check --lib` 通过）

## 阶段 3：定义主世界玩家实体与注册表

- 开始时间：2026-08-12 13:56:27 +08:00
- 结束时间：2026-08-12 14:10:15 +08:00
- 开发总结：定义 session/generation-bound 主世界玩家注册表，按 character_id 唯一维护玩家根并在 server entity_id 变化时受控替换。
- 验证记录：`cargo test main_world_players --lib` 通过（5 passed, 0 failed）；`cargo check --lib`、`cargo fmt -- --check`、`git diff --check` 通过。

- [x] 新增主世界玩家根组件，保存 `character_id`、服务端 `entity_id`、本地/远端归属、scene session 和最近权威帧。（验证：`MainWorldPlayer` 定义完整字段）
- [x] 新增按 `character_id` 查询的 session-bound 注册表，支持创建、查找、替换和清理。（验证：`MainWorldPlayerRegistry` 绑定 session/generation 并提供 `register/get/len/clear`）
- [x] 以当前 character-bound ticket 对应的 `character_id` 唯一标记本地玩家。（验证：registry constructor 接收 `local_character_id`，注册时仅字符串相等者为 `Local`）
- [x] 发现同一 `character_id` 的 `entity_id` 变化时受控替换或重置旧实体，不保留陈旧身份。（验证：`changed_server_entity_id_replaces_stale_root` 断言旧 Entity 已 despawn、新根进入注册表）
- [x] 拒绝空 character ID、错误 scene ID、非有限 Transform 和旧 generation 数据。（验证：非法输入测试覆盖空/空白 ID、错误 scene、generation mismatch、NaN Transform；旧 frame 另有定向测试）
- [x] 为本地玩家挂接当前 scene session 的 `SceneCameraTarget`，tag 使用框架已有 `local_player` 或 `primary_actor`。（验证：本地根挂 session-bound `SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG`）
- [x] 保证远端玩家不会获得本地控制或摄像机目标标记。（验证：远端 root 无 `SceneCameraTarget` 且 ownership 为 `Remote`；本阶段尚未定义控制组件）
- [x] 增加注册表唯一性、身份替换和 session 隔离测试。（验证：5 项测试覆盖唯一更新、替换、本地身份、非法输入、旧帧与 clear 不影响无关 Entity）

## 阶段 4：实现 0.25 米方圆玩家生成

- 开始时间：2026-08-12 14:11:06 +08:00
- 结束时间：2026-08-12 14:31:18 +08:00
- 开发总结：主世界注册表接入真实 minimal blueprint，按 bounds 计算 0.25m footprint 缩放，生成多个 session-owned 玩家并复用共享渲染资产。
- 验证记录：`cargo test main_world_players --lib` 通过（8 passed）；runtime 测试 2 passed；`cargo check --lib` 与 `cargo fmt -- --check` 通过。

- [x] 从 blueprint bounds 计算 `uniform_scale = 0.25 / max(width, depth)`，不硬编码当前模型缩放值。（验证：`main_world_player_uniform_scale` 使用真实 `FangyuanBlueprintBounds`）
- [x] 验证当前 `2 x 2 x 3` bounds 得到 `0.125` 缩放和 `0.25 x 0.25 x 0.375m` 逻辑尺寸。（验证：`minimal_blueprint_scales_to_quarter_meter_footprint_and_grounded_height`）
- [x] 计算脚底基准并让玩家根实体落在地面 `Y=0`，不逐 primitive 写入世界坐标。（验证：根 Transform translation 使用权威脚底位置，primitive 仍来自 blueprint 局部 Transform；尺寸测试断言 Y=0）
- [x] 在当前 scene runtime root 下生成玩家根实体和方圆视觉子树，并附加 `SceneOwned(session_id)`。（验证：registry 创建路径 parent 到传入 runtime root，根插入 `SceneOwned`；多玩家测试核验 parent）
- [x] 允许同一场景创建多个玩家，所有玩家暂时复用最小方圆 blueprint 和共享渲染缓存。（验证：多玩家测试生成 2 根/4 primitive，按 kind 复用 Mesh handle、按颜色复用 Material handle）
- [x] 为尺寸、脚底对齐、多玩家 primitive 数量和 asset handle 复用增加测试。（验证：主世界 8 项测试及 runtime rotation 保留测试通过）
- [x] 运行共享方圆玩家和主世界玩家生成定向测试。（验证：`cargo test main_world_players --lib` 8 passed；`cargo test framework::fangyuan::runtime --lib` 2 passed）

## 阶段 5：接入权威快照与 Active 门槛

- 开始时间：2026-08-12 14:32:40 +08:00
- 结束时间：2026-08-12 15:02:35 +08:00
- 开发总结：接入独立 `MainWorldPlayersPlugin`，缓存早到权威快照并在 scene/room ready 后建立全部玩家，持续处理增量与 full sync。
- 验证记录：`cargo test main_world_players --lib` 通过（13 passed）；`cargo test main_world_entry --lib` 32 passed；`cargo check --lib` 与 `cargo fmt -- --check` 通过。

- [x] 从 `MovementSnapshotPush.entities` 读取主世界 scene 对应的全部 character，而不是只读取本地角色。（验证：`apply_main_world_snapshot` 预验证并遍历全部 entities，系统测试生成 local+remote）
- [x] 初始快照早于 Scene Ready 时缓存最新有效版本，不提前生成到未就绪的世界根。（验证：plugin 缓存 generation-bound snapshot；系统测试 early snapshot 后玩家数为 0）
- [x] Scene Ready、Room Ready 和初始权威快照门槛满足后，根据最新快照一次性建立玩家实体集合。（验证：同一系统测试设置 matching root/scene_ready/room_ready_acknowledged 后不发新 snapshot 即生成 2 玩家）
- [x] 增量快照只创建缺失实体和更新已存在实体的权威状态，不删除未列出的玩家。（验证：系统测试增量更新/新增并保留未列出 remote）
- [x] AOI 关闭后将 full sync 解释为完整可见实体集合，并清理集合中不再存在的玩家。（验证：`full_sync=true` 调用 `remove_missing`，系统测试删除缺失角色）
- [x] 保持 `main_world_entry` 只负责编排进场和初始上下文，持续实体维护进入独立玩家运行时模块。（验证：`MainWorldPlayersPlugin` 独立监听 MyServerEvent 并读取 entry context，已接入 `GameScenesPlugin`；entry 不维护玩家实体）
- [x] 增加快照早到、重复快照、增量快照、full sync 删除和旧帧忽略测试。（验证：13 项玩家测试覆盖全部时序、错误原子拒绝及 generation/session 切换；entry 32 项回归通过）

## 阶段 6：完成退出、重连与重进生命周期

- 开始时间：2026-08-12 15:03:30 +08:00
- 结束时间：2026-08-12 16:04:30 +08:00
- 开发总结：完善主世界玩家运行时的退出、恢复、redirect/generation 隔离和同 generation 重进逻辑；Recovering 保留视觉但冻结摄像机目标，Active 恢复唯一本地 target，完整 teardown 清理注册表、primitive、缓存和恢复状态。
- 验证记录：`cargo test main_world_players --lib` 16 passed；`cargo test main_world --lib` 112 passed；`cargo test fangyuan_player_preview --lib` 25 passed；`cargo fmt -- --check`、`cargo check --lib` 通过；未联网、未启动或停止 server。

- [x] Room Leave、scene exit、切家园、返回 Lobby、切环境和不可恢复鉴权失败时清空对应玩家注册表。（验证：`maintain_main_world_players` 对非视觉 session phase 清理 registry/cache/frame/error/recovery 状态；`plugin_scene_exit_or_lobby_teardown_clears_players_visuals_and_targets` 通过）
- [x] 短线恢复期间保留当前可恢复视觉并冻结本地控制资格，不生成重复本地玩家。（验证：Recovering 分支保留 registry、移除全部 `SceneCameraTarget`；`plugin_recovery_preserves_visuals_freezes_camera_then_restores_unique_target` 通过）
- [x] recovery/full snapshot 到达后以权威实体集合重建或复用玩家，并重置旧权威帧基线。（验证：Recovering -> Active 清空 cached snapshot/last frame 并归零 registry frame 基线，低 frame full snapshot 重建测试通过）
- [x] server redirect 和 scene generation 变化时拒绝旧连接或旧 session 的迟到快照。（验证：generation/session 切换清理玩家和缓存；`plugin_generation_session_switch_clears_old_players_and_cached_snapshot` 与 `redirect_ignores_old_snapshots_until_reconnect_then_restores_ready` 通过）
- [x] 重复进入同一主世界时确认旧玩家、primitive 子树和摄像机 target 均已清理。（验证：`plugin_same_generation_reentry_after_teardown_uses_fresh_snapshot_and_target` 断言旧实体清理、新 session 仅建立唯一本地 target）
- [x] 增加断线、重连、redirect、scene exit 和重复进入的生命周期测试。（验证：主世界玩家新增 recovery、teardown、same-generation re-entry 测试；主世界 entry redirect/recovery 测试均通过）
- [x] 运行 `cargo test main_world --lib`、`cargo test fangyuan_player_preview --lib` 和 `cargo check`。（验证：112、25 passed；`cargo check --lib` 通过）

## 阶段 7：视觉回归与文档

- 开始时间：2026-08-12 16:05:50 +08:00
- 结束时间：2026-08-12 16:46:19 +08:00
- 开发总结：新增显式 opt-in 的离线双玩家固定视口截图 fixture，经真实权威消息、主世界玩家插件和共享 Fangyuan runtime 生成 local/remote；补充 Preview 退出清理、稳定 Entity/asset 增量测量测试和正式场景使用文档。
- 验证记录：主 agent 从仓库根以 `PROJECT_MAIN_WORLD_PLAYERS_FIXTURE_SCREENSHOT=H:\project\mybevy\summary\main-world-players-fixture-main-review.png` 运行 `scripts/run_fast.ps1`，固定视口截图成功且客户端自动退出；原图 705,939 bytes，确认两名玩家清晰分离、完整可见且底面贴地。`cargo test main_world_players --lib` 17 passed，`cargo test main_world --lib` 113 passed，`cargo test fangyuan_player_preview --lib` 26 passed，`cargo fmt -- --check`、`cargo check --lib` 通过；全程离线，未启动或停止 server。

- [x] 从仓库根运行 `scripts/run_fast.ps1`，确认主世界本地玩家清晰可见且脚底贴地。（验证：opt-in fixture 在固定 `2772x1280`、device scale `3.25`、window scale `50%` 下生成 `main-world-players-fixture-main-review.png` 并自动退出；原图确认角色完整可见且底面落在同一地面）
- [x] 使用至少两个构造 character 验证场景内可以同时显示多个独立方圆玩家。（验证：fixture 注入 `fixture-local` 和 `fixture-remote` 的真实 `MovementSnapshotPush`；截图显示两个空间分离实体，玩家测试断言 registry=2、visual=4）
- [x] 验证 Preview 页面仍能正常进入、显示和退出，不受主世界玩家注册表影响。（验证：`leaving_preview_mode_cleans_owned_player_and_primitive_visuals` 覆盖 OnExit 后 owner/primitive 为 0；Preview 过滤套件 26 passed）
- [x] 记录单玩家和多玩家下的 Entity 数、Mesh/Material asset 增量和生成耗时。（验证：主 agent `--nocapture` 实测空到单 roots +1/visuals +2/Mesh +2/Material +2/2.944ms，单到双 +1/+2/+0/+0/0.371ms；计数由测试断言）
- [x] 更新 `docs/方圆灵构/` 或 `docs/场景/` 中共享玩家渲染、尺寸和生命周期说明。（验证：`docs/场景/游戏层场景使用说明.md` 记录共享 runtime、尺寸/Y=0、character/session/generation、快照/recovery/teardown、fixture 和测量口径）
- [x] 在 `project/` 运行 `cargo fmt`、`cargo check` 和全部相关定向测试。（验证：格式检查与 `cargo check --lib` 通过；玩家 17、主世界 113、Preview 26 passed）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-12 13:19:42 +08:00
- 结束时间：2026-08-12 16:46:19 +08:00
- 验收总结：共享 Fangyuan 玩家视觉、主世界权威多人实体、恢复/退出生命周期和固定视口离线视觉回归均完成；7 个阶段分别提交业务改动，清单 与截图留在未提交的 summary 验收区。

- [x] Preview 和主世界复用同一方圆玩家 primitive 生成与渲染缓存。（验证：两者均调用 framework `spawn_fangyuan_player` 并使用 `FangyuanPlayerRuntimePlugin` cache）
- [x] 主世界 Active 后，本地 `character_id` 对应玩家只生成一次。（验证：character-keyed registry 唯一更新测试、重复快照和 fixture 均保持唯一本地根）
- [x] 当前最小玩家逻辑尺寸为 `0.25 x 0.25 x 0.375m`，脚底位于 `Y=0`。（验证：bounds 缩放单测通过，固定视口截图确认地面对齐）
- [x] 同一 scene session 可生成多个不同 `character_id` 的独立玩家实体。（验证：双玩家测试和离线 local/remote fixture 均生成 2 根、4 visual）
- [x] full sync、重连、退出和重进能够正确维护或清理玩家集合。（验证：玩家 17 项与主世界 113 项覆盖 full sync、recovery、redirect、teardown、same-generation re-entry）
- [x] 本地玩家拥有唯一 SceneCameraTarget，远端玩家不会抢占摄像机目标。（验证：local/remote ownership、recovery 恢复和重进测试均断言唯一 local target）
- [x] 自动测试、`cargo fmt`、`cargo check` 和固定窗口视觉验收均通过。（验证：玩家 17、主世界 113、Preview 26 passed；fmt/check 通过；run_fast fixture 截图通过）
