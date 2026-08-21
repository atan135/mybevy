# 主世界客户端移动预测、纠正与远端插值 清单

## 目标

在客户端建立主世界角色移动闭环：采集桌面和触控移动输入，按摄像机平面朝向生成标准化移动意图，复用 `MyServerCommand::SendMoveInput` 以 20Hz 提交权威输入，对本地玩家执行预测和权威纠正，并对远端玩家执行快照插值。

本清单不修改 Protobuf 消息号，不新建平行 JSON 移动协议，不实现角色碰撞、寻路、战斗、浮动原点或服务端 authority 模拟。

## 依赖与边界

- 需求来源：`summary/主世界尺度角色摄像机与联机移动需求设计.md`。
- 单元测试和构造快照 fixture 不需要登录；需要真实进场时优先使用正式服登录，不默认使用本地服务端。
- 依赖 `02_方圆玩家共享渲染与主世界多人实体_checklist.md` 提供玩家实体和注册表。
- 依赖 `03_主世界摄像机与桌面触控视角操控_checklist.md` 提供可读取的摄像机平面方向与输入仲裁。
- 与 `世界联机_05_服务端主世界4000米权威移动_checklist.md` 共同冻结 20Hz、4m/s、0..4000 服务端坐标和 2000 米中心偏移。
- 复用现有 `MoveInputReq`、`MovementSnapshotPush`、`MovementRejectPush` 和 recovery 协议。

## 基础原则

- [x] 输入系统只产生移动意图，不直接宣称客户端 Transform 为权威状态。（验证：`MainWorldMovementIntent` 经 20Hz `SendMoveInput`、预测和权威 snapshot/reject 链路处理；正式服实测仅确认显示行为。）
- [x] 本地玩家使用预测与纠正，远端玩家使用插值，两条显示路径明确分离。（验证：local/remote `MovementSnapshotPush` fixture、33 个 movement 测试和离线双角色截图通过。）
- [x] 网络发送频率由 authority 20Hz 控制，不随 render FPS 增长。（验证：`dispatch_main_world_move_input` 的固定 20Hz timer 与方向/预测定向测试通过。）
- [x] 所有 room、character、scene session、generation 和 frame 先校验后应用。（验证：snapshot/reject/entry lifecycle gate 覆盖及 `cargo test main_world_ --lib -q` 168/168 通过。）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（验证：阶段 1-8 已独立提交；阶段 9 验收提交 `5ac032e`，方向修复将在本次验收记录后单独提交。）

## 阶段 1：冻结坐标、帧和移动契约

- 开始时间：2026-08-13 10:06:28 +08:00
- 结束时间：2026-08-13 10:25:16 +08:00
- 开发总结：将 4000 米坐标、方向、移动参数与帧边界集中到 `main_world_contract`，入口和玩家 fixture 已迁移到居中坐标映射。
- 验证记录：`cargo fmt`、`cargo test main_world_contract --lib`（14/14）、`cargo test main_world_entry --lib`（32/32）、`cargo test main_world_players --lib`（17/17）和 `cargo check` 通过；`git diff --check` 通过。

- [x] 将 `main_world_bevy_position` 从旧 `0..=16` 规则改为 4000 米非负服务端坐标到居中 Bevy 坐标的唯一转换。（验证：`project/src/game/scenes/main_world_contract.rs` 的 `main_world_bevy_position`；`main_world_entry` 委托该契约。）
- [x] 实现反向转换：`server_x=bevy_x+2000`、`server_y=bevy_z+2000`。（验证：`main_world_server_position` 与回环测试通过。）
- [x] 对方向只做 X/Z 到 x/y 的轴映射，不应用中心偏移。（验证：`main_world_bevy_direction`、`main_world_server_direction` 及 `directions_use_axis_mapping_without_position_offset` 测试。）
- [x] 统一使用 `0 <= coordinate < 4000` 的服务端合法范围，并拒绝 NaN、Infinity 和错误 scene ID。（验证：`is_main_world_server_coordinate`、`main_world_bevy_position_from_authority` 及定向边界测试。）
- [x] 冻结 20Hz、4m/s、方向归一化、`MOVE_DIR` 保活和 `MOVE_STOP` 语义。（验证：共享常量、`MainWorldMoveInputKind` 与 `movement_contract_freezes_cadence_speed_normalization_and_stop_semantics` 测试。）
- [x] 定义移动运行时使用的权威帧、预测帧、已确认帧和渲染帧边界。（验证：`MainWorldAuthorityFrame`、`MainWorldPredictedFrame`、`MainWorldConfirmedFrame`、`MainWorldRenderFrame` 类型与边界测试。）
- [x] 增加中心、四边、排他上界、反向转换和有限值属性测试。（验证：`cargo test main_world_contract --lib` 14/14 通过。）

## 阶段 2：建立客户端移动状态与系统调度

- 开始时间：2026-08-13 10:26:03 +08:00
- 结束时间：2026-08-13 10:57:45 +08:00
- 开发总结：新增独立 `main_world_movement` 插件，集中有界预测/权威/远端状态，并明确 Update、FixedUpdate 与 PostUpdate 调度及主世界生命周期 gate。
- 验证记录：`cargo fmt`、`cargo test main_world_movement --lib`（7/7）、`cargo test main_world_entry --lib`（32/32）、`cargo test main_world_players --lib`（17/17）、`cargo test game::scenes --lib`（170/170）和 `cargo check` 通过；`git diff --check` 通过。

- [x] 新增主世界 movement plugin 或等价独立模块，不将持续模拟堆入 `main_world_entry.rs`。（验证：`project/src/game/scenes/main_world_movement.rs` 的 `MainWorldMovementPlugin` 已由 `GameScenesPlugin` 注册。）
- [x] 定义移动意图、本地预测状态、未确认输入历史、权威基线和远端插值缓存。（验证：独立模块定义 `MainWorldMovementIntent`、`MainWorldPredictedState`、`MainWorldUnconfirmedInput`、`MainWorldAuthorityBaseline` 和 `MainWorldRemoteInterpolationBuffer`。）
- [x] 为预测历史和每个远端玩家快照队列设置明确容量与淘汰规则。（验证：预测历史上限 100、远端队列上限 40；`prediction_history_evicts_oldest_input_at_capacity` 和远端缓存测试通过。）
- [x] 明确 FixedUpdate、网络事件消费、Transform 写入和 Transform propagation 的系统顺序。（验证：Update `Coordinator -> ConsumeAuthority -> CollectIntent -> DispatchInput`，FixedUpdate `Predict`，PostUpdate `WriteTransforms.before(TransformSystems::Propagate)`。）
- [x] 只在 `MainWorldEntryState` 为 Active 且 `input_frozen=false` 时开放本地移动。（验证：`sync_main_world_movement_lifecycle` 与 Active gate 测试。）
- [x] 在断线、退出、scene generation 变化和不可恢复失败时冻结或清理对应状态。（验证：Recovering、Exiting、Failed、无 session 和 generation 切换均调用清理；生命周期测试通过。）
- [x] 增加系统顺序、资源默认值、容量上限和 Active gate 测试。（验证：`cargo test main_world_movement --lib` 7/7 与 `cargo test game::scenes --lib` 170/170 通过。）

## 阶段 3：实现桌面与触控移动意图

- 开始时间：2026-08-13 10:58:36 +08:00
- 结束时间：2026-08-13 11:21:16 +08:00
- 开发总结：接入键盘和左侧触控摇杆的摄像机相对移动意图；抽出摄像机/移动共用的触控起点 owner，统一处理 UI 优先、输入冻结和单次停止转换。
- 验证记录：`cargo fmt`、`cargo test main_world_movement --lib -q`（12/12）、`cargo test main_world_camera --lib -q`（19/19）、`cargo test game::scenes --lib -q`（175/175）和 `cargo check` 通过；`git diff --check` 通过。场景汇总首次出现一次既有 entry 时序性失败，独立复现及第二次汇总均通过。

- [x] 使用 `W/S/A/D` 和方向键生成二维输入轴。（验证：`keyboard_movement_axis` 与键盘组合定向测试。）
- [x] 归一化多键组合，确保斜向输入不会提高移动速度。（验证：使用 `main_world_normalized_direction`；键盘斜向测试断言单位长度。）
- [x] 根据当前摄像机在 XZ 平面的 forward/right 将输入转换为世界移动方向。（验证：`main_world_camera_relative_direction` 基于 `MainWorldCameraOrbitState.yaw_radians`；相机相对方向测试通过。）
- [x] 为触控左侧 gameplay 区实现虚拟摇杆或等价连续二维输入，并提供稳定死区。（验证：左 40% 区的单 capture、80px 半径和 12px dead zone；摇杆连续输入测试通过。）
- [x] 复用摄像机清单定义的 pointer owner，移动区、摄像机区和 UI 不争用触点。（验证：摄像机导出 `main_world_touch_owner` 和 `MainWorldTouchOwner`；起点锁定且 UI 优先的测试通过。）
- [x] 按键或触点释放、窗口失焦、应用进入后台和 UI 阻断时产生一次停止意图。（验证：`stop_sequence` 与 `request_stop`；释放、`WindowFocused`、`AppLifecycle`、UI gate 定向测试通过。）
- [x] 增加方向组合、相机相对转换、死区、释放、失焦和 UI gate 测试。（验证：`cargo test main_world_movement --lib -q` 12/12 和 `cargo test main_world_camera --lib -q` 19/19 通过。）

## 阶段 4：接入 20Hz MoveInput 发送

- 开始时间：2026-08-13 11:22:09 +08:00
- 结束时间：2026-08-13 12:33:08 +08:00
- 开发总结：在 movement 的 DispatchInput set 中接入既有 `SendMoveInput`，以精确 20Hz cadence 发送方向/停止请求，回传服务端坐标和同帧 client state，并以 entry/session 身份和连接状态 gate。
- 验证记录：第 1 轮修复了测试时间推进与 50ms timer 边界；`cargo fmt`、`cargo test main_world_movement --lib -q`（16/16）、`cargo test game::scenes --lib -q`（179/179）、`cargo test myserver --lib -q`（161/161）、`cargo check` 和 `git diff --check` 通过。

- [x] 移动期间按 authority 帧持续发送 `MyServerCommand::SendMoveInput(MOVE_DIR)`。（验证：`dispatch_main_world_move_input` 的 20Hz `Timer`；持续发送定向测试通过。）
- [x] 方向变化在下一可用 authority frame 生效，同一 frame 不重复发送互相冲突的方向。（验证：每个 tick 仅构造一个 `SendMoveInput`；方向更新测试断言 frame 42/43 的正确方向。）
- [x] 停止时发送一次 `MOVE_STOP`，并避免空闲期间每帧重复停止包。（验证：`stop_sequence`/`observed_stop_sequence` 消费一次；停止去重测试通过。）
- [x] 仅改变朝向且不移动时按需要发送现有 `FACE_TO`。（验证：当前没有独立仅朝向意图来源，因此不伪造 `FACE_TO`；现有输入仅生成 `MOVE_DIR`/`MOVE_STOP`，满足可选语义。）
- [x] 每个请求携带反向转换后的预测 `client_x/client_y` 和匹配的预测 frame ID。（验证：使用 `main_world_server_position` 填充既有 `MovementClientState`，测试断言 `(2001.25, 1997.5)` 和相同 frame。）
- [x] gameplay 非 Active、room 不匹配、输入冻结或连接不可用时不发送移动请求。（验证：`main_world_movement_send_gate` 校验 entry/session/generation/room/character/认证连接；gate 定向测试通过。）
- [x] 确保移动发送不复用 `ui_touch` action，也不通过通用 `PlayerInputReq` 重复编码。（验证：仅写入既有 `MyServerCommand::SendMoveInput`，由 MyServer plugin 编码 `MoveInputReq`。）
- [x] 增加 20Hz 限频、保活、停止去重、frame 连续性和连接 gate 测试。（验证：`cargo test main_world_movement --lib -q` 16/16、场景 179/179、MyServer 161/161 通过。）

## 阶段 5：实现本地预测

- 开始时间：2026-08-13 12:34:05 +08:00
- 结束时间：2026-08-13 13:01:51 +08:00
- 开发总结：实现由已发送 authority 输入驱动的固定 20Hz 本地预测、可重放输入历史与 local-only 渲染插值；排他上界按服务端坐标浮点精度安全映射。
- 验证记录：上界首次测试发现 `next_down(2000)+2000` 可舍入回非法 4000，已改为 `next_down(4000)-2000` 后通过；`cargo test main_world_movement --lib -q`（21/21）、`cargo test game::scenes --lib -q`（184/184）、`cargo test myserver --lib -q`（161/161）、`cargo check` 和 `git diff --check` 通过。

- [x] 使用固定 20Hz 步长和 4m/s 速度推进本地预测位置。（验证：`predict_main_world_movement_fixed` 及 `main_world_predicted_after_input` 每帧推进 0.2m；速度测试通过。）
- [x] 将预测位置限制在客户端映射后的 `-2000..2000` 世界范围，保持与服务端排他上界一致。（验证：从 `next_down(4000)-2000` 计算上界；边界测试断言 `main_world_server_position` 合法。）
- [x] 为每个已发送输入保存 frame、方向、预测前后状态和确认状态。（验证：`MainWorldUnconfirmedInput` 记录 frame/direction/before/after/confirmed；发送-预测关联测试通过。）
- [x] 渲染 Transform 在固定预测状态之间平滑，不把渲染插值结果写回预测基线。（验证：PostUpdate 使用 `Time<Fixed>::overstep_fraction`；local-only 视觉测试断言预测基线不变。）
- [x] 本地视觉即时响应输入，不等待 `MovementSnapshotPush` 才开始移动。（验证：SendMoveInput 后 pending FIFO 在下一固定帧直接推进，关联测试断言无需快照即可移动。）
- [x] 停止意图使预测速度和 moving 状态在确定帧收敛。（验证：`MOVE_STOP` 的零方向预测输入清零 direction/moving，停止测试通过。）
- [x] 增加速度、斜向、边界、停止、不同 render delta 和确定性重放测试。（验证：`cargo test main_world_movement --lib -q` 21/21 与场景 184/184 通过。）

## 阶段 6：实现本地权威纠正与输入重放

- 开始时间：2026-08-13 13:02:54 +08:00
- 结束时间：2026-08-13 13:43:30 +08:00
- 开发总结：消费本地 `MovementSnapshotPush`，按 room/character/scene/session/generation/frame 校验后建立权威基线，区分小误差视觉平滑与强制 rebase，并重放未确认输入。
- 验证记录：首轮生命周期测试发现普通 Recovering 错误保留 session，已修复为仅 reconnect_requested 恢复路径保留；第 1 轮补充真实 snapshot event fixture 后，`cargo test main_world_movement --lib -q`（23/23）、`cargo test game::scenes --lib -q`（186/186）、`cargo check` 和 `git diff --check` 通过。

- [x] 从本地 character 的 `MovementSnapshotPush` 读取权威位置、方向、moving 和 last input frame。（验证：`consume_main_world_local_authority_snapshots` 读取真实事件并建立 `MainWorldLocalAuthoritySnapshot`。）
- [x] 丢弃旧于已应用权威帧的快照，并对重复帧幂等处理。（验证：`last_applied_authority_frame` 单调 gate；真实 event 测试覆盖重复快照。）
- [x] 将权威位置与对应预测历史对比，区分小误差平滑纠正和强制新基线。（验证：0.5m 阈值、0.1s visual offset 和 correction 测试。）
- [x] full sync、recovery、entity ID 变化、scene change 和超出阈值时直接建立权威基线。（验证：`force_rebase` 覆盖 full_sync、FullSync/Strong/Recovery、reconnect、identity/scene 变化、无 anchor/大误差；集成测试通过。）
- [x] 应用权威基线后重放尚未被 authority 确认的本地输入，不重放已确认或旧 generation 输入。（验证：确认帧清理 history、以 authority position 重建 pending replay；真实 event 测试通过。）
- [x] 清理已确认预测历史并保持队列有界。（验证：`unconfirmed_inputs`/`pending_prediction` 按 confirmed frame 清理，并沿用容量 100；history 集成测试通过。）
- [x] 增加小误差、强纠正、输入重放、旧快照、重复快照和 history 截断测试。（验证：`cargo test main_world_movement --lib -q` 23/23、场景 186/186 通过。）

## 阶段 7：实现远端玩家插值

- 开始时间：2026-08-13 13:44:28 +08:00
- 结束时间：2026-08-13 14:34:46 +08:00
- 开发总结：为远端 character 建立 identity-scoped 有界快照缓存，按 2 个 authority frame 延迟和 fixed overstep alpha 插值，并处理停止、零方向、强制重置与实体移除。
- 验证记录：原实现测试覆盖不足，替代 worker 补充 4 个真实 `MovementSnapshotPush` 集成测试；最终 `cargo test main_world_movement --lib -q`（28/28）、`cargo test game::scenes --lib -q`（191/191）、`cargo check` 和 `git diff --check` 通过。工作区另有无关 UI/docs 改动，未纳入本阶段提交。

- [x] 为每个远端 character 按 frame ID 缓存有序权威状态。（验证：`MainWorldRemoteInterpolationBuffer` 按 frame 排序、重复替换、容量 40；乱序/重复集成测试通过。）
- [x] 普通快照在有界延迟窗口内插值位置和朝向，不执行本地输入预测。（验证：2 authority-frame delay + `Time<Fixed>::overstep_fraction`；远端 Transform 集成测试通过，ownership gate 排除本地。）
- [x] `moving=false` 时收敛到最终权威位置并停止残余滑动。（验证：停止样本集成断言最终 Transform 位置。）
- [x] 零方向向量保留有效旧朝向或使用稳定默认朝向，不生成 NaN rotation。（验证：零方向继承 Vec2::X/旧方向，rotation 使用合法 `Quat::from_rotation_y`；测试通过。）
- [x] full sync、recovery、entity ID 变化和超出最大距离时重置插值基线。（验证：FullSync/Strong/Recovery、identity 变化和 >8m 大跳变测试断言 buffer 仅保留新基线。）
- [x] 远端实体被 full sync 移除或 scene exit 时同步释放插值缓存。（验证：full sync visible retain/remove 与 Exiting 生命周期测试通过。）
- [x] 增加乱序、重复、丢帧、停止、强制重置和实体移除测试。（验证：新增 4 个真实 `MovementSnapshotPush` fixture；movement 28/28、scene 191/191 通过。）

## 阶段 8：处理 Reject、断线与退出

- 开始时间：2026-08-13 14:36:07 +08:00
- 结束时间：2026-08-13 15:22:00 +08:00
- 开发总结：接入 `MovementRejectPush` 校验与强制纠正；对非法/越界/碰撞/超时停止本地预测；支持 reconnect recovery 快照重建本地基线和远端缓存，并明确迟到 reject 丢弃规则。
- 验证记录：首轮测试发现 corrected reject 后残留旧输入重放 0.2m，已清理 prediction queue 修复；审核又补充过期 reject frame gate 和真实迟到事件测试。最终 `cargo test main_world_movement --lib -q`（31/31）、entry（32/32）、players（17/17）、`cargo test game::scenes --lib -q`（193/193）、`cargo check` 和 `git diff --check` 通过。

- [x] 消费 `MovementRejectPush` 前校验 room、character、scene、generation 和 reference frame。（验证：reject consumer 校验 room/character/session/generation/scene/reference frame，非零过期 reject frame 丢弃；真实 gate/迟到事件测试通过。）
- [x] 对 corrected transform 建立权威基线，并按 correction kind/reason 决定平滑或强制纠正。（验证：corrected transform 走 `reconcile_main_world_local_authority(force_rebase=true)`；reject 集成测试断言 baseline。）
- [x] 非法方向、越界、碰撞拒绝和控制超时停止当前本地预测。（验证：reason/error-code 识别后清空 history/pending、停止 intent 并将 predicted 对齐 corrected；Reject 测试通过。）
- [x] 断线时停止增长未确认预测，保留可恢复视觉并冻结输入。（验证：既有 connection gate/reset 与 lifecycle `Recovering` 冻结；发送/生命周期定向测试通过。）
- [x] recovery snapshot 到达后重建本地预测基线和全部远端插值缓存。（验证：reconnect_requested 的 Recovering 接收 Recovery/full_sync，本地 baseline 强制重建、远端缓存清空后重建；集成测试通过。）
- [x] Room Leave、家园切换、返回 Lobby 和 session kick 后不再发送主世界移动输入。（验证：entry Active/session/room/connection send gate 及 Exiting lifecycle 清理；现有 movement/entry/players 测试通过。）
- [x] 日志只记录必要 room、character、frame、reason 和误差，不记录 ticket 或敏感字段。（验证：reject `debug!` 仅包含 room_id、character_id、frame_id、reference_frame_id、reason_code。）
- [x] 增加 reject、断线、recovery、退出和迟到事件测试。（验证：movement 31/31、entry 32/32、players 17/17、scene 193/193 通过。）

## 阶段 9：客户端移动验收与文档

- 开始时间：2026-08-13 15:23:05 +08:00
- 结束时间：2026-08-13 17:22:48 +08:00
- 开发总结：完成固定窗口离线 fixture、正式服游客登录与用户创建测试角色后的主世界实测；实测发现相机 orbit 的 actor-to-camera 偏移被误用为 view-forward，修正其反向并将触控屏幕 Y 翻转后，键盘与触控均与当前视图方向一致，松开均可靠停止。
- 验证记录：`cargo test main_world_ --lib -q`（168/168）、`cargo test main_world_movement --lib -q`（33/33）、`cargo test main_world_camera --lib -q`（19/19）、`cargo test game::scenes --lib -q`（196/196）、`cargo fmt -- --check`、`cargo check` 与 `git diff --check` 通过；根目录 `scripts/run_fast.ps1` 固定使用 2772x1280/device 3.25/window 50%。离线 fixture 生成 `target/stage9-main-world-players-fixture.png`（1386x640），local/remote 两角色可见且分离。正式服游客会话进入主世界后，用户确认修复版 W/D 与左侧摇杆方向正确，键盘与触控释放后均立即停止。

- [x] 使用 fixture 快照验证至少一个本地玩家和一个远端玩家的移动显示路径。（验证：`shared_snapshot_fixture_drives_local_correction_and_remote_visual_path` 使用同一 `MovementSnapshotPush` 断言 local authority baseline、remote cache 及两实体 Transform；`scripts/run_fast.ps1` fixture 截图显示两角色。）
- [x] 从仓库根运行 `scripts/run_fast.ps1` 验证键盘和触控移动、停止及摄像机相对方向。（验证：修复版由固定窗口脚本启动并正式服进场；用户实测 W/D 和左侧摇杆均相对当前视图方向正确，键盘/触控释放均停止；`main_world_camera_relative_direction` 与摇杆 yaw 定向测试同步通过。）
- [x] 使用调试位置或自动测试验证世界中心和四个边界，不要求人工步行穿越全图。（验证：`cargo test main_world_ --lib -q` 168/168 包含 `main_world_contract` 的中心、四边、排他上界和反向映射定向测试。）
- [x] 记录移动输入发送频率、预测历史容量、插值缓存容量和逐帧系统耗时。（验证：`MainWorldMovementDiagnostics` 记录 Update、FixedUpdate、PostUpdate 的最近 wall-time 与采样数；文档冻结 20Hz、100 条预测历史、40 条远端缓存和 2-frame 延迟，`movement_diagnostics_record_update_fixed_and_presentation_samples` 通过。）
- [x] 验证设置、邮箱、Loading、Modal、断线和窗口失焦均能可靠停止移动。（验证：entry 的 `input_frozen` / modal-blocking panel gate、`release_focus_background_and_ui_gates_emit_one_stop_and_require_rearm` 与 `generation_change_disconnect_exit_and_failure_clear_scoped_runtime`；`cargo test main_world_ --lib -q` 168/168 通过。）
- [x] 在 `project/` 运行 `cargo fmt`、`cargo check` 和全部客户端移动定向测试。（验证：`cargo fmt -- --check`、`cargo check`、`cargo test main_world_ --lib -q` 168/168、`cargo test main_world_movement --lib -q` 33/33、`cargo test game::scenes --lib -q` 196/196 通过。）
- [x] 更新 `docs/场景/` 或玩法文档中的坐标、输入、预测、纠正和远端插值说明。（验证：`docs/场景/游戏层场景使用说明.md` 新增 `world.main` 客户端移动章节，覆盖坐标、20Hz 输入、预测、纠正、Reject、recovery、远端插值、容量和诊断。）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-13 15:23:05 +08:00
- 结束时间：2026-08-13 17:22:48 +08:00
- 验收总结：客户端主世界移动闭环完成。正式服游客测试角色进入主世界后，修复版键盘和左侧触控均产生正确的摄像机相对方向，释放后停止；自动化覆盖 20Hz 发送、预测、权威纠正、远端插值、Reject、recovery、边界和 UI/生命周期 gate。固定窗口离线双角色 fixture 与正式服手工输入证据均已记录。无未解决的已知阻塞项。

- [x] 桌面和触控输入都能产生摄像机相对、归一化的移动方向。（验证：正式服修复版 W/D、左侧摇杆实测；yaw=0/+90 度与触控定向测试通过。）
- [x] `MOVE_DIR`、`MOVE_STOP` 和可选 `FACE_TO` 通过现有 MoveInput 协议按 20Hz 发送。（验证：20Hz timer、方向/停止去重测试和正式服移动后释放停止实测；当前无独立仅朝向意图，未伪造 `FACE_TO`。）
- [x] 本地玩家具有即时预测响应，并能按权威快照平滑或强制纠正。（验证：本地预测、snapshot rebase/replay、reject 测试与真实进场移动显示通过。）
- [x] 远端玩家使用有界快照缓存插值，停止和强制重置行为正确。（验证：40 条缓存、2-frame 插值、停止/重置/实体移除测试及离线双角色 snapshot fixture 通过。）
- [x] Reject、断线、recovery、退出和旧 generation 均有确定处理结果。（验证：reject、disconnect/recovery、exit/generation gate 定向测试通过。）
- [x] 客户端可在 4000 米逻辑范围内移动并正确约束边界。（验证：中心、四边、排他上界、反向转换和有限值定向测试通过。）
- [x] 自动测试、`cargo fmt`、`cargo check` 和固定窗口输入验收均通过。（验证：movement 33/33、camera 19/19、scenes 196/196、`main_world_` 168/168、fmt/check/diff check 与固定窗口正式服手工输入验收通过。）
