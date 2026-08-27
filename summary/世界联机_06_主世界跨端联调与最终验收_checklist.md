# 主世界跨端联调与最终验收 清单

## 目标

在客户端静态世界、方圆多人实体、摄像机操控、客户端移动和 MyServer 4000 米 authority 均完成后，按“本地预联调”和“正式服最终验收”两套流程执行主世界跨端集成、双客户端联调、断线恢复、生命周期、视觉、性能和平台验收，确认完整产品结果可以交付。

本地预联调用于尽早发现并修复跨端集成缺口，冻结可部署的客户端与服务端候选版本；正式服最终验收使用同一候选版本验证公网登录、选角、character-bound ticket、game proxy、公共房间和真实网络路径。本地结果不能代替正式服结果，正式服未通过时本清单不得整体完成。

本清单以发现和修复集成缺口为目标，不在没有证据时重做前置模块。若联调暴露独立架构缺陷，应回到对应前置 清单 补充实现和验证，再返回本清单复验。

## 前置条件

- `世界联机_01_主世界4000米静态场景与照明_checklist.md` 已完成。
- `世界联机_02_方圆玩家共享渲染与主世界多人实体_checklist.md` 已完成。
- `世界联机_03_主世界摄像机与桌面触控视角操控_checklist.md` 已完成。
- `世界联机_04_主世界客户端移动预测纠正与远端插值_checklist.md` 已完成。
- `世界联机_05_服务端主世界4000米权威移动_checklist.md` 已完成。
- 现有主世界登录、选角、加房、Scene Ready、Room Ready、退出和家园往返基线保持可用。
- 本地预联调只使用明确记录的本地账号、角色、服务地址和测试数据，不连接或修改正式服数据。
- 启动本地 MyServer 或其 PostgreSQL、Redis、NATS、auth、proxy、game-server 等依赖前，按 MyServer 仓库约定向用户说明服务、端口、数据目录和执行范围并取得确认；不得影响其他正在进行的服务端测试。
- 正式服最终验收必须使用本地预联调通过并已提交的精确客户端与服务端 commit；未部署该候选版本时不得用旧正式服结果代替。

## 基础原则

- [ ] 联调使用 character-bound ticket 和不同 `character_id`，不以账号 `player_id` 替代玩法主体。
- [ ] 客户端与服务端版本必须同时满足同一 4000 米坐标和 movement policy 契约。
- [ ] 本地与正式服日志、截图、性能数据和验证记录分开保存并明确标注环境。
- [ ] 本地预联调通过只代表候选版本可部署，不自动勾选正式服验收项。
- [ ] 自动测试、日志、截图和性能数据只记录可复现证据，不凭观察臆造通过结果。
- [ ] Android 真机项需要满足现有公网部署与真机验收清单的前置条件，不用桌面结果替代。
- [ ] 每个阶段完成后运行对应验证，并按阶段独立提交。

## 阶段 1：跨端契约与两套环境预检

- 开始时间：2026-08-18（本轮开始，未单独记录精确时刻）
- 结束时间：2026-08-18 14:23:44 +08:00
- 开发总结：完成两端 commit、主世界映射、移动策略、坐标、协议快照和本地/正式服 endpoint 预检；确认客户端 vendored authority-core、sim-core 源文件与当前服务端一致，game.proto SHA-256 完全一致。本地精简主链路随后按预检结果启动。
- 验证记录：mybevy=`4ad4742f169157dba066fe4116d4dfaa7fc7512d`，MyServer=`408a65d922c0092315c2b2a4cb4e342ecd42f7c0`；两仓库均无受 Git 跟踪的未提交改动。两端 game.proto SHA-256 均为 `19B8DAAB4DB044BB5134B674885960E9E8A088EA1D6FA06F10B85B5F9BAC3EB6`；authority-core 与 sim-core 源文件逐文件哈希一致，差异仅为服务端 crate 独立 Cargo.lock 不进入客户端快照。vendor README 仍记录旧来源 commit `811a6ba0`，阶段 7 冻结候选版本前需更新来源记录。

- [x] 记录参与联调的 mybevy 与 MyServer 精确 commit、配置版本和协议生成来源。（验证：记录两个完整 commit；客户端协议快照来源记录为 811a6ba0，实际源码/协议与 408a65d9 逐文件核对一致）
- [x] 核对 `world.main` 与 `grassland_01 / scene_id=1 / spawn_id=1001` 映射未变化。（验证：客户端 main_world_contract、scene RON 与服务端 SceneTable/SceneSpawnPoint 一致）
- [x] 核对公共房间为 `main-world-public / movement_demo`，两端 tick 为 20Hz、速度为 4m/s。（验证：客户端集中契约与服务端 room policy/room 常量一致）
- [x] 核对服务端坐标 `0..4000`、客户端中心偏移 2000 米和默认出生点 `(2002,2002) -> (2,0,2)`。（验证：服务端 world_contract/CSV 与客户端 main_world_contract/layout 一致）
- [x] 核对 MoveInput、EntityTransform、MovementSnapshot、MovementReject 和 recovery Protobuf schema 一致。（验证：两端 game.proto SHA-256 完全一致）
- [x] 确认服务端 movement AOI 关闭，客户端 full sync 按完整房间实体集合处理。（验证：服务端 movement_demo policy 为 AOI disabled，客户端 full snapshot 实体集合处理代码与既有测试存在）
- [x] 分别记录本地和正式服的 auth、game、chat、mail endpoint；确认本地环境选择不会持久化或污染正式服 session。（验证：本地 auth=127.0.0.1:3000、game KCP=4000/TCP=14000；正式服 descriptor 继续按仓库约定使用 game/chat/mail 公网地址；环境切换清理 session 的既有客户端契约未改）
- [x] 检查两个仓库工作区状态，保留并记录与本轮无关的既有改动，不混入联调修复提交。（验证：两仓库受跟踪工作区均干净；客户端 summary 按仓库约定被忽略）

## 阶段 2：本地服务端环境与单客户端基线

- 开始时间：2026-08-18 14:32:44 +08:00
- 结束时间：2026-08-19 16:41:40 +08:00
- 开发总结：本地 MyServer 精简主链路和真实 Bevy 客户端已启动；完成游客登录、已有角色选择、character-bound ticket 鉴权、公共房间进入、固定窗口主世界基线和出生点坐标复核。8 月 19 日新增 HUD 角色名、服务端坐标和移动方向诊断，后续复验可直接观察出生坐标和方向。
- 验证记录：auth `/healthz` 通过；game proxy 接受 account `plr_27he19p00040`、character `chr_27he4t440040` 并绑定 game-server；真实客户端与 `127.0.0.1:14000` 保持 TCP 会话，服务端 `main-world-public` 以 20Hz 持续发送 FrameBundle/MovementSnapshot，online_count=1。用户确认主世界 HUD 显示“游戏会话已连接”，默认出生点服务端坐标为 `(2002,2002)`，对应客户端 `(2,0,2)`；滚轮缩放、右键环绕、摄像机跟随、移动停止和参照球阵列可见；本地截图保存在 `summary/local-test-evidence/`，本地 A/B 客户端日志保存在 `logs/main-world-debug/`。新增提交 `b0b9d46` 接入 HUD 坐标与方向诊断。

- [x] 在启动前列出所需 MyServer 服务、端口、数据目录和测试范围，经用户确认后再启动。（验证：用户确认允许启动本地服务；启动 PostgreSQL 5432、Redis 6379、NATS 4222、auth 3000、game-server 7000、game-proxy TCP 14000/KCP 4000、match-service 9002，使用本地开发数据）
- [x] 检查端口占用和现有服务状态，确认不会重启、覆盖或占用其他测试中的 MyServer 实例。（验证：启动前检查监听端口；按用户确认停止并重启本地实例）
- [x] 使用隔离的本地测试账号、角色和 character-bound ticket 完成注册或登录、选角和 game proxy 鉴权。（验证：真实 Bevy 客户端本地游客登录并选择 `chr_27he4t440040`，proxy session 12 鉴权及 upstream 绑定成功）
- [x] 加入本地 `main-world-public`，验证 RoomJoin、初始 movement snapshot、Scene Ready、Room Ready 和 Active 顺序。（验证：真实客户端进入主世界并显示游戏会话已连接，服务端持续产生 20Hz FrameBundle 和周期 MovementSnapshot）
- [x] 确认默认出生坐标映射为客户端 `(2,0,2)`，本地方圆玩家只生成一次且脚底贴地。（验证：用户确认出生点服务端坐标为 `(2002,2002)`，对应客户端 `(2,0,2)`；本轮确认玩家只生成一次且脚底贴地，`b0b9d46` 已提供 HUD 坐标诊断）
- [x] 确认 4000 米地面、单 Mesh 参照球和日间照明正常显示。（验证：本地客户端可见球阵列与主世界画面）
- [x] 确认摄像机默认跟随本地玩家，环绕、俯仰和缩放参数不会被下一帧覆盖。（验证：用户确认右键视角、滚轮缩放和摄像机跟随正常）
- [x] 保存本地单客户端日志、关键状态和固定窗口截图作为后续对照基线，并记录环境为 local。（验证：`summary/local-test-evidence/bevy-local-a.png`、`bevy-local-a-main-world.png`、`bevy-local-real-entry.png`；本地运行日志在 `logs/main-world-debug/`）

## 阶段 3：本地双客户端实体与移动同步

- 开始时间：2026-08-19 14:17:50 +08:00（本地 A/B 双客户端复验轮次）
- 结束时间：2026-08-19 17:27:40 +08:00
- 开发总结：已使用两个不同本地 character 完成 A/B 客户端同房间进场、双向移动/停止和关键联调数据汇总。新增提交修复首次进场等待错误快照、运行中公共房间重复启动、远端同实体更新覆盖插值空间状态、本地预测固定帧回退、远端停止和空闲转移动插值窗口问题。
- 验证记录：`logs/main-world-debug/client-a10.err.log`、`client-b10.err.log` 记录 `chr_29tj3zv00020` 与 `chr_27he4t440040` 同房间；A 端收到 B 端 `moving=true/false` 远端快照并看到位置变化，B 端记录 20Hz 移动输入帧。用户确认 A 端移动时 B 端远端显示正常；`logs/main-world-debug/client-a11.err.log`、`client-b11.err.log` 复验不同角色进场。A10 共记录 B 的 4,875 条远端快照（4,873 个唯一 frame，`81104..153390`），B10 共记录 A 的 4,819 条远端快照（4,819 个唯一 frame，`81128..153390`）；两端远端快照诊断的中位帧步长为 15、最大为 15，连续记录无超过 1.215 秒的间隔。该诊断为节流采样，不能替代服务端 20Hz tick 计数；由 B10 的 145 条 input dispatch 证明输入按约 50.15ms 中位间隔派发，且 `frame_id - latest_room_frame` 固定为 2。两端 `presentation_lag_frames` 中位数均为 18（A 平均 17.908、B 平均 17.994；最大分别为 18.305 与 18.312），A/B 各观察 1 个对端角色，与各自本地角色合计可见 2 个。相关提交：`770b262`、`4bec7c0`、`c8ba7fe`。当前 HEAD 验证：`cargo test remote_ --lib` 21 passed；`cargo test --lib game::scenes::main_world_movement::tests` 44 passed。

- [x] 使用两个不同 `character_id` 的本地客户端加入同一公共房间。（验证：A=`chr_29tj3zv00020`、B=`chr_27he4t440040`；A/B 日志均进入 `main-world-public` 并 `main world entry active`）
- [x] 两端分别确认恰好生成一个本地玩家和一个对应远端玩家，无重复或身份交换。（验证：A/B 日志的远端快照 character 均指向对端；回归测试 `local_ticket_character_gets_camera_target_but_remote_does_not`、`snapshot_applies_all_characters_incrementally_and_full_sync_removes_missing`、`remote_same_entity_update_preserves_interpolated_spatial_state` 通过）
- [x] 客户端 A 移动和停止时，客户端 B 看到对应远端角色连续位移并收敛停止。（验证：用户确认 A 客户端移动时，B 客户端查看正常）
- [x] 客户端 B 移动和停止时，客户端 A 得到对称结果。（验证：`client-a10.err.log` 记录对端 `chr_27he4t440040` 多段 `moving=true/false` 快照与位置变化；`client-b10.err.log` 同期记录 B 端连续 `move input dispatched`）
- [x] 验证本地角色采用预测与纠正，远端角色采用插值，没有互换显示策略。（验证：`4bec7c0` 保留本地预测基线并改用连续展示帧渲染远端快照；`c8ba7fe` 修复远端停止和空闲转移动插值；对应 movement 定向测试通过）
- [x] 验证两端摄像机只跟随各自本地 character，远端玩家不会抢占 target。（验证：`local_ticket_character_gets_camera_target_but_remote_does_not`、`plugin_recovery_preserves_visuals_freezes_camera_then_restores_unique_target` 通过）
- [x] 验证 full sync、普通增量、重复和旧帧不会重复生成实体或回退位置。（验证：`targeted_recovery_snapshot_registers_all_visible_remote_entities`、`remote_full_sync_retains_visible_players_and_removes_missing_players`、`remote_buffers_replace_duplicates_reject_old_frames_and_evict_oldest`、`remote_late_snapshot_does_not_rewind_or_rebase_presentation` 通过）
- [x] 记录双客户端网络帧、快照频率、可见实体数和关键延迟数据。（验证：A10/B10 的远端 frame 范围、采样步长、无长间隔、B10 输入约 20Hz、2 帧输入领先、两端约 18 帧呈现滞后和各端 2 个可见玩家已写入阶段验证记录）

## 阶段 4：本地移动、视角和边界验收

- 开始时间：2026-08-18 15:17:00 +08:00（约，本轮人工 UI 验收开始）
- 结束时间：
- 开发总结：完成本地人工验证并修正首次进入主世界的同角色重登录竞态；方向反向为参照球/跟随相机造成的误判，固定参照球复验后移动方向正常。8 月 19 日补充 HUD 坐标/方向诊断，并修复本地预测与远端插值抖动。
- 验证记录：用户确认 `W/A/S/D`、触控摇杆、松键停止、摄像机跟随、右键视角、滚轮缩放、阵列显示均正常，无跳动、回弹或持续滑行。切换角色/重新登录后首次点击“进入主世界”已一次成功；服务端新增发送通道归属保护，自动回归测试通过。当前 HEAD 验证：`cargo test --lib game::scenes::main_world_movement::tests` 44 passed，覆盖键盘/方向键、触控摇杆、20Hz 派发、释放/失焦/UI 阻断、预测固定帧保持和远端插值。双指缩放需要 Android 真机，本地无法覆盖，转入正式服平台验收。

- [x] 验证 WASD、方向键和触控摇杆均按当前摄像机平面方向移动。（验证：用户确认 WASD、方向键和触控摇杆本地交互复验正常；测试 `keyboard_axes_normalize_diagonals_and_use_wasd_or_arrows`、`camera_relative_mapping_matches_view_forward_and_right_at_cardinal_yaws`、`left_touch_stick_has_dead_zone_continuous_axis_and_locked_ownership` 通过）
- [x] 验证斜向速度与单轴速度一致，服务端权威速度保持 4m/s。（验证：用户确认 W+D 与单轴移动速度表现一致）
- [x] 验证移动期间按 20Hz 提交输入，释放、失焦和 UI 阻断只产生必要停止语义。（验证：服务端此前持续输出 20Hz movement snapshot；本轮确认组合视角操作不打断移动，失焦后无持续滑行）
- [x] 验证桌面右键拖动、滚轮和触控右侧拖动不干扰移动输入；双指缩放转入正式服 Android 真机验收。（验证：本轮桌面右键拖动、滚轮与移动互不干扰；触控摇杆本地交互已复验；测试 `right_and_ui_touches_never_claim_movement_and_camera_uses_same_owner_rule` 通过；双指缩放本地无法测试，已从本地验收范围移至阶段 12）
- [ ] 使用测试或调试定位验证服务端 0、中心、接近 4000 和越界坐标，不人工步行穿越全图。（部分通过：客户端坐标映射和 4000 排他上界已有单元测试；本地服务端调试定位仍待覆盖全组坐标）
- [ ] 确认合法边界位置能同步，负值、4000 排他上界、NaN 和 Infinity 被拒绝。（部分通过：`coordinate_mapping_rejects_exclusive_upper_bound_and_non_finite_values`、`fixed_prediction_preserves_exclusive_server_upper_boundary_and_converges_on_stop` 通过；服务端拒绝链路仍待本地构造复验）
- [x] 验证 100 米参照球可以辅助判断方向和位移，不要求摄像机一次显示完整地图。（验证：用户确认参照球阵列可见并用于方向复验）

## 阶段 5：本地纠正、拒绝与异常网络

- 开始时间：2026-08-19 14:06:05 +08:00（客户端纠正与插值回归验证）
- 结束时间：
- 开发总结：客户端侧已修复小误差回放、固定帧边界预测回退、远端快照延迟窗口、远端停止和空闲转移动表现；MovementReject corrected transform、迟到拒绝、重复/旧快照和 full sync 处理已有回归测试。用户确认本轮尚未执行受控异常网络、服务端强 correction 构造和日志敏感信息审查。
- 验证记录：相关提交 `4bec7c0`、`c8ba7fe`；当前 HEAD `cargo test remote_ --lib` 21 passed，`cargo test --lib game::scenes::main_world_movement::tests` 44 passed。人工异常网络与强 correction 构造未测试。

- [x] 构造小幅客户端预测误差，确认使用平滑纠正且不持续振荡。（验证：`local_snapshot_event_confirms_history_and_replays_remaining_inputs_idempotently`、`authority_replay_waits_for_fixed_ticks_before_applying_pending_inputs`、`authority_replay_preserves_applied_fixed_step_interpolation_endpoints`、`local_visual_correction_decays_by_render_delta` 通过）
- [ ] 构造超过阈值的漂移，确认服务端下发权威 correction 并使客户端收敛。（部分通过：客户端 strong/recovery correction 应用测试通过；仍需本地服务端构造权威 correction）
- [x] 构造非法方向或越界移动，确认 `MovementRejectPush` 的 corrected transform 被正确应用。（验证：`movement_reject_applies_corrected_baseline_and_stops_invalid_prediction`、`late_movement_reject_cannot_rewind_a_newer_authority_snapshot` 通过）
- [x] 验证乱序、重复、丢失和迟到快照不会回退已应用权威状态。（验证：`remote_buffers_replace_duplicates_reject_old_frames_and_evict_oldest`、`remote_late_snapshot_does_not_rewind_or_rebase_presentation`、`remote_snapshot_push_is_idempotent_and_resets_on_identity_or_large_jump`、`remote_continuous_movement_keeps_the_normal_delay_and_monotonic_time` 通过）
- [ ] 验证移动控制包短暂缺失后服务端按 control timeout 停止角色。
- [ ] 检查客户端和服务端日志包含必要 room、character、frame 和 reason，同时不包含 ticket、密码或 token。
- [ ] 记录正常网络和受控异常网络下的预测误差、纠正频率和远端插值表现。

## 阶段 6：本地断线恢复与场景生命周期

- 开始时间：2026-08-19 11:53:05 +08:00（首次进场与生命周期回归验证）
- 结束时间：
- 开发总结：新增提交修复首次入场消费房间状态内嵌 `movement_demo` 快照、运行中公共房间重复启动、真实房间帧纪元重置后的预测/派发/远端插值清理。断线、重连、redirect、退出和家园往返的客户端状态机已有单元测试覆盖，但本地断网和家园往返人工联调仍未整体验收；本轮已补客户端侧修复，使房间/帧包内嵌完整实体快照可以移除缺失远端玩家，并同步清理远端插值缓存。P6-01 确认失败：B 在游戏内退出登录并回到登录页后，A 仍保留 B。客户端 `LeaveRoom` 已抵达服务端，但服务端将明确退出与意外断线同样标记为离线并保留 120 秒，未立即调用 `on_character_leave` 移除 movement 实体；因此需要在 MyServer 区分可恢复断线和明确离房。
- 验证记录：相关提交 `770b262`、`ce8ea93`；既有状态机测试覆盖 `disconnect_freezes_input_keeps_the_scene_and_requests_ticket_reconnect`、`redirect_ignores_old_snapshots_until_reconnect_then_restores_ready`、`reconnect_membership_expiry_rejoins_the_fixed_public_room`、`active_exit_leaves_room_exits_its_scene_and_returns_to_lobby`、`home_transition_leaves_the_public_room_and_return_restarts_main_world_entry`。本轮新增 `complete_room_snapshot_removes_missing_remote_player_without_full_sync`、`complete_room_snapshot_removes_missing_remote_interpolation_without_rebase` 覆盖客户端远端 despawn/缓存清理，相关测试通过。窗口关闭复验：B 日志 `logs/main-world-debug/phase6-B-current.stderr.log` 在 10:22:52 仅记录 `No windows are open, exiting`，A 在 5 秒内仍显示 B，B 重登后仍为 `chr_29tj3zv00020` 且无重复实体，符合可恢复断线语义。P6-01（2026-08-19，游戏内退出登录）失败：用户确认 B 已回到登录页、A 等待 5 秒仍显示 B；服务端 `H:\project\MyServer\logs\dev-stack\game-server.out.log` 在 10:43:08 记录 B 的 `leave_room called`，紧接着 `movement demo player stopped after offline` 与 `player marked offline, members count: 2`。`movement_demo` 策略 `offline_ttl_secs = 120`，TTL 到期才执行 `on_character_leave`。新启动的 `logs/main-world-debug/phase6-B.log` 只有编译输出，未保留本次客户端运行时日志；服务端日志已足以确认请求到达及错误语义。

- [ ] 移动中断开本地 game proxy，确认客户端立即冻结输入且不无限延伸本地预测。（部分通过：`disconnect_freezes_input_keeps_the_scene_and_requests_ticket_reconnect`、`connection_loss_sends_nothing_resets_the_timer_and_preserves_next_frame_order` 通过；真实 proxy 断开仍待联调）
- [ ] 完成重鉴权和 RoomReconnect，确认 recovery snapshot 恢复位置、方向、moving 和玩家集合。（部分通过：`recovery_snapshot_rebuilds_local_baseline_and_clears_remote_history`、`reconnect_membership_expiry_rejoins_the_fixed_public_room` 通过；真实重鉴权仍待联调）
- [ ] 确认恢复后本地摄像机跟随有效新实体，未绑定旧 scene Entity。（部分通过：`plugin_recovery_preserves_visuals_freezes_camera_then_restores_unique_target` 通过；真实恢复后摄像机仍待本地观察）
- [ ] 验证 server redirect 期间旧连接迟到快照不覆盖新 generation 权威状态。（部分通过：`redirect_ignores_old_snapshots_until_reconnect_then_restores_ready` 通过；真实 redirect 仍待联调）
- [x] 断线恢复子测：直接关闭 B 窗口后 5 秒内，A 保留 B 的静止角色；B 重登后复用同一 `character_id`，且 A 不出现重复实体。（验证：B 未发送 `LeaveRoom`，服务端按 `movement_demo` 的 120 秒 offline TTL 保留可恢复角色；日志见 `logs/main-world-debug/phase6-A-current.stderr.log`、`logs/main-world-debug/phase6-B-current.stderr.log`。）
- [ ] P6-01：B 游戏内退出登录并回到登录页后，A 应在 5 秒内移除 B；当前失败。（服务端已收到 `LeaveRoom`，却把 B 标记 offline 并保留实体 120 秒；须修复 MyServer 明确离房的立即删除与完整快照/离场事件广播，再进行 A/B 复验。）
- [ ] 返回 Lobby 后确认停止发送移动输入，并清理玩家、参照球、摄像机目标和运行时缓存。（待复验：游戏内退出登录已确认对端保留离开角色；客户端完整房间快照触发远端 despawn 与插值缓存清理已修复，但依赖服务端对明确离房移除实体；部分通过：`active_exit_leaves_room_exits_its_scene_and_returns_to_lobby`、`plugin_scene_exit_or_lobby_teardown_clears_players_visuals_and_targets` 通过）
- [ ] 进入家园再返回主世界，确认重新加入公共房间且不残留旧实体或旧预测历史。（部分通过：`home_transition_leaves_the_public_room_and_return_restarts_main_world_entry`、`plugin_same_generation_reentry_after_teardown_uses_fresh_snapshot_and_target` 通过；真实家园往返仍待复验）
- [ ] 重复进入/退出主世界，确认 SceneOwned Entity、3D Camera、环境光和输入 reader 不累积。（部分通过：generation/session 清理测试通过；3D Camera、环境光和输入 reader 累积仍待运行时测量）
- [ ] 验证 session kick 和不可恢复鉴权失败沿现有路径清理并返回登录页。（部分通过：房间错误和场景失败测试覆盖到可操作 Lobby；session kick 仍待本地构造）
- [x] 验证切换角色/重新登录后首次进入主世界不会被旧连接延迟清理误伤。（验证：服务端以房间成员当前发送通道校验断线归属；回归测试 `delayed_disconnect_does_not_mark_rejoined_character_offline` 通过；真实本地 UI 首次点击进入成功）

## 阶段 7：本地性能、视觉与候选版本收口

- 开始时间：2026-08-19 16:56:41 +08:00
- 结束时间：
- 开发总结：已收口本轮新增客户端修复提交和定向测试结果，并补充退出登录对端残留的客户端侧远端 despawn 修复；本地视觉、性能、MyServer 32 人快照、服务端定向测试和候选版本冻结仍未完成。
- 验证记录：本轮本地修复提交：`770b262 fix(main-world): 修正首次入场移动同步`、`b0b9d46 feat(main-world-hud): 增加角色位置和移动方向诊断`、`4bec7c0 fix(main-world): 消除权威移动预测与插值抖动`、`c8ba7fe fix(main-world): 修复移动端远端移动插值问题`。当前工作区新增远端 despawn 修复尚未提交；已运行 `cargo fmt`、`cargo check`、`cargo test --lib game::scenes::main_world_contract::tests` 15 passed、`cargo test --lib game::scenes::main_world_players::tests` 19 passed、`cargo test --lib game::scenes::main_world_movement::tests` 45 passed、`cargo test remote_ --lib` 23 passed；尚未运行服务端定向测试。

- [ ] 从仓库根运行 `scripts/run_fast.ps1`，按固定设备尺寸和缩放完成桌面视觉验收。
- [ ] 记录客户端单人和双人时的稳定帧率、内存、ECS Entity、Mesh/Material asset 和主要系统耗时。
- [ ] 确认 1681 个参照球仍为一个 Entity，静态 Mesh 不在运行时重复重建。
- [ ] 确认默认画面亮度、玩家轮廓、地面和参照球在出生点及移动过程中清晰可辨。
- [ ] 记录 MyServer 最大 32 人全房间快照的序列化大小、发送频率和处理开销证据。
- [ ] 在 `project/` 运行 `cargo fmt`、`cargo check` 和相关客户端测试；按 MyServer 规则经确认后运行服务端定向测试。（部分通过：本轮已运行 `cargo fmt`、`cargo check`、main_world_contract/main_world_players/main_world_movement/remote_ 定向测试；服务端测试仍待执行）
- [ ] 汇总本地阶段发现的问题、修复提交和复验结果，不保留无归属的临时改动。（部分通过：本轮已记录 4 个 mybevy 修复提交和客户端测试；仍需整理本地日志、截图、性能与 MyServer 复验）
- [ ] 冻结准备部署的 mybevy commit、MyServer commit、配置版本和 protobuf 来源；存在未解决阻塞项时不得进入正式服验收。

## 阶段 8：正式服部署与版本预检

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 确认阶段 7 冻结的 MyServer 候选 commit 已部署到正式服，记录部署版本和时间。
- [ ] 确认正式服客户端使用阶段 7 冻结的 mybevy commit 和同源 protobuf。
- [ ] 核对 auth 下发的 game、chat 和 mail descriptor 符合生产配置，客户端不读取 registry 或直连内部服务。
- [ ] 核对正式服 `grassland_01`、spawn 1001、`main-world-public / movement_demo`、20Hz、4m/s、0.05m 和 AOI disabled 契约。
- [ ] 对比本地与正式服配置差异，确认差异均有明确用途且不会改变移动和坐标契约。
- [ ] 正式服预检失败时停止后续验收，修复并重新部署同一候选链路后再继续。

## 阶段 9：正式服单客户端基线

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 使用正式服有效账号、角色和 character-bound ticket 完成登录、选角和 game proxy 鉴权。
- [ ] 加入 `main-world-public`，验证 RoomJoin、初始 movement snapshot、Scene Ready、Room Ready 和 Active 顺序。
- [ ] 确认默认出生坐标映射为客户端 `(2,0,2)`，本地方圆玩家只生成一次且脚底贴地。
- [ ] 确认 4000 米地面、单 Mesh 参照球、日间照明和摄像机操控正常。
- [ ] 验证键盘和触控移动输入、停止、预测及权威纠正在正式服链路可用。
- [ ] 保存正式服单客户端日志、关键状态和固定窗口截图，所有证据明确标注为 production。

## 阶段 10：正式服双客户端实体与移动同步

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 使用两个不同 `character_id` 的客户端通过正式服加入同一公共房间。
- [ ] 两端分别确认恰好生成一个本地玩家和一个对应远端玩家，无重复或身份交换。
- [ ] A、B 双向移动和停止均能在对端连续显示并收敛到权威状态。
- [ ] 验证本地预测、远端插值、摄像机本地 target 和 full sync 实体集合均未互换或回退。
- [ ] 验证重复、旧帧和正常网络抖动不会重复生成实体或回退已应用位置。
- [ ] 记录正式服双客户端快照频率、可见实体数、往返延迟、预测误差和远端插值表现。

## 阶段 11：正式服拒绝、断线恢复与生命周期

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 使用安全且受控的测试方式验证越界或非法输入得到 `MovementRejectPush` 并应用 corrected transform。
- [ ] 移动中断开 game proxy，确认客户端冻结输入并停止无限预测。
- [ ] 完成重鉴权和 RoomReconnect，确认 recovery 恢复位置、方向、moving 和玩家集合。
- [ ] 确认恢复后本地摄像机跟随有效新实体，未绑定旧 scene Entity。
- [ ] 验证 server redirect 期间旧连接迟到快照不覆盖新 generation 权威状态。
- [ ] 验证 Lobby、家园往返、重复进入/退出、session kick 和不可恢复鉴权失败的清理路径。
- [ ] 检查生产日志和客户端日志不包含 ticket、密码、token 或其他敏感 payload。

## 阶段 12：正式服性能、视觉与平台最终验收

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 按固定设备尺寸和缩放完成正式服桌面视觉验收，保存与本地基线可对照的截图。
- [ ] 记录正式服单人和双人时的稳定帧率、内存、ECS Entity、资源数量和主要系统耗时。
- [ ] 记录正式服最大 32 人全房间快照的序列化大小、发送频率和服务端处理开销证据。
- [ ] 对比本地与正式服延迟、预测误差、纠正频率和远端插值表现，解释显著差异。
- [ ] 如目标版本已完成公网部署，按现有 Android 真机验收清单使用正式服验证触控、前后台、断线和恢复。
- [ ] Android 未满足设备或部署前置条件时明确记录为外部验收阻塞项，不以桌面或本地结果替代。
- [ ] 正式服最终验收完成后复跑客户端检查与关键测试；服务端检查按 MyServer 规则经确认后执行。

## 阶段 13：文档同步与清单归档准备

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 汇总两端最终坐标、速度、tick、纠正、AOI、出生点和协议兼容契约。
- [ ] 分别汇总本地预联调与正式服最终验收的版本、环境、日志、截图和性能证据。
- [ ] 更新 `docs/场景/` 中主世界场景、摄像机、玩家生命周期和移动同步说明。
- [ ] 更新 `docs/服务端/` 中客户端接线、移动恢复和服务端配置说明。
- [ ] 检查 `docs/界面/` 是否需要补充移动摇杆、触控区域和 gameplay 输入阻断边界。
- [ ] 检查 `CLAUDE.md` 与 `docs/引擎入门使用文档.md` 是否因命令、目录、初始化或上手流程变化需要同步。
- [ ] 将所有日志、截图和性能数据路径写入对应阶段验证记录，不提交临时敏感数据或无关构建产物。
- [ ] 所有拆分 清单 完成后，按仓库约定将其从 `summary/` 归档到对应 `docs/<领域>/checklists/`。

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 主世界呈现清晰可读的日间 `4000m x 4000m` 地面和单实体参照球 Mesh。
- [ ] 本地阶段 1～7 全部完成，并冻结已提交、可复现的客户端与服务端候选版本。
- [ ] 正式服阶段 8～12 使用同一候选版本完成部署和复验，本地结果未被直接当作正式服结果。
- [ ] 当前登录 character 生成尺寸正确、脚底贴地的方圆玩家。
- [ ] 桌面和触控均可自由移动角色并环绕、俯仰、缩放查看场景。
- [ ] 摄像机参数通过 `SceneCameraRig` 持续生效且只跟随本地玩家。
- [ ] MyServer 对 4000 米范围保持移动 authority，客户端预测和纠正行为稳定。
- [ ] 两个正式服客户端能够看到彼此角色连续移动、停止和权威纠正。
- [ ] Reject、断线恢复、redirect、退出、家园切换和旧 generation 隔离在正式服全部通过。
- [ ] 性能、日志、安全、生命周期和文档验收均有可复现记录。
- [ ] 本阶段未引入地图分块、LOD、浮动原点、复杂地形或重复移动协议。
