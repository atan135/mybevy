# 主世界进场与 Authority 闭环 清单

## 目标

复用 MyServer 已有 `grassland_01` 场景和 `movement_demo` 房间策略，建立从 Lobby 加入固定公共房间、加载客户端主世界、完成 Scene/Room ready、消费权威场景状态、断线恢复并退出回 Lobby 的完整链路。

首版固定公共房间 ID 为 `main-world-public`。本清单不新增服务端主世界 SceneTable 记录，不实现匹配、动态分线、正式战斗、美术地形或完整角色玩法。

## 依赖与边界

- 依赖 `01_注册与登录入口适配_checklist.md` 或现有登录/选角链路提供有效 character-bound ticket。
- 依赖 `project/src/framework/scene/` 的 `SceneRuntime`、`SceneCommand`、`SceneEvent`、Loading、根实体和 ready 边界。
- 依赖 `project/src/game/myserver/` 的 game proxy 鉴权、房间命令、重连和移动快照事件。
- 主世界 HUD 和场景内按钮由 `03_主世界界面与场景内面板_checklist.md` 负责。
- 服务端权威场景为 `scene_code=grassland_01`、`scene_id=1`、默认出生点 `spawn_id=1001`。
- 本清单的网络链路验收统一使用本机 Windows `cargo run` 客户端连接本地 MyServer；Android 真机验收集中转移至 `07_公网部署后Android真机验收_checklist.md`，只在目标版本完成公网部署后启动。

## 基础原则

- [x] 客户端场景负责资源和表现生命周期，服务端房间负责角色身份、权威场景 ID、位置和移动状态。（审核：`world.main` 实体归属 SceneOwned/session root，MyServer movement snapshot 提供 scene 1 和权威坐标。）
- [x] 不新增与 `SceneRuntime` 重复的通用场景状态机；游戏层协调器只编排业务意图和网络前置条件。（审核：协调器仅发送 generation-bound SceneCommand 并消费 SceneEvent，加载/清理仍由 SceneRuntime 完成。）
- [x] 房间、移动、输入、重连和退出全部使用 ticket 绑定的 `character_id`，不以账号 `player_id` 作为玩法主体。（审核：三轮本地 KCP 实跑均以当前 character 完成 Auth/Join/Snapshot/Ready/Active。）
- [x] UI 不直接发送底层网络包或拼装完整进场流程。（审核：Lobby 仅发 MainWorldEntryIntent，协调器和 MyServer adapter 负责完整链路。）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（审核：阶段 1-8 已分阶段提交；阶段 9 已拆分 MyServer 0dad9a5/500cb7f 与 MyBevy 9fde471/d4a3f91/17c3d62。）

## 阶段 1：主世界和公共房间契约冻结

- 开始时间：2026-08-04 13:40:21 +08:00
- 结束时间：2026-08-04 13:54:14 +08:00
- 开发总结：新增集中主世界 authority contract，冻结客户端/服务端场景、出生点、公共房间和 policy 映射。
- 验证记录：worker 运行 cargo fmt --check、cargo test main_world_contract --lib（6 passed）和 cargo check；主审核复跑该定向测试 6 passed。

- [x] 确认复用服务端 `grassland_01`，对应数值 ID `1` 和默认出生点 `1001`。
- [x] 确认复用服务端 `movement_demo` policy，不新增重复主场景策略。
- [x] 确认首版采用固定公共房间模型，房间 ID 为 `main-world-public`，不经过匹配或动态分配。
- [x] 在客户端集中定义主世界逻辑 ID、服务端 scene code/数值 ID、spawn ID、room ID 和 policy ID 映射，禁止散落硬编码。（审核：`project/src/game/scenes/main_world_contract.rs` 定义 MainWorldAuthorityContract，定向测试通过。）
- [x] 明确 `RoomStatePush.game_state`、`MovementSnapshotPush.entities[].scene_id` 与客户端主世界 scene session 的权威关系。（审核：contract 明确 room game_state 仅为兼容性断言，ticket-bound entity snapshot scene_id 是活动场景最终权威。）
- [x] 将匹配、动态分线、跨区传送、正式玩家模型和战斗标记为本清单非目标。（审核：contract 的 first-scope non-goals 常量及单元测试明确排除。）

## 阶段 2：服务端现有场景和策略准入验证

- 开始时间：2026-08-04 13:55:02 +08:00
- 结束时间：2026-08-04 14:28:49 +08:00
- 开发总结：核验并修复 MyServer 固定主城房间的严格 policy 准入，未知或不一致策略不再静默回退。
- 验证记录：静态核对 scene CSV/grid/policy；MyServer 定向 lifecycle 测试 9 passed，worker 的 room_policy 8 passed、factory 2 passed、cargo check 通过；服务端提交 e4a31d8。

- [x] 验证 `SceneTable.csv` 中 `grassland_01` 的数值 ID、GridFile、尺寸、AOI 和 DefaultSpawnId。（验证：MyServer SceneTable.csv:3 为 id=1、grassland_01.grid.json、16x16、AOI 4、spawn 1001。）
- [x] 验证 `SceneSpawnPoint.csv` 中 `1001` 属于场景 `1`，且默认坐标在 grid 上可行走。（验证：SceneSpawnPoint.csv:3 为 scene 1 的 (2,2)，grid walkable=1/block=0，服务端 validator 校验。）
- [x] 验证 `movement_demo` 使用 `grassland_01`，并记录 max members、允许中途加入、离线 TTL、快照频率和移动修正参数。（验证：max 32、mid-join false、TTL 120s、snapshot 15 frames、correction 3/.35，AOI radius 16。）
- [x] 确认 `main-world-public` 首位玩家可以创建房间，后续玩家以相同 policy 加入；错误 policy 不得静默回退成其他玩法。（验证：MyServer e4a31d8 拒绝未知 policy 与 existing-room mismatch；lifecycle 测试 9 passed。）
- [x] 明确房间满员、房间暂不可用和服务端策略拒绝的稳定错误码及客户端恢复入口。（验证：ROOM_FULL、ROOM_*UNAVAILABLE、SERVER_DRAINING_REJECT_NEW_ROOM、ROOM_POLICY_UNSUPPORTED/MISMATCH；客户端后续协调器按 MyServer DisplayError/RoomJoinFailed 消费。）
- [x] 如验证发现配置缺口，只修复服务端现有配置或策略；不得无依据新增第三套主世界场景。（验证：仅修复服务端 policy/factory 准入，无新增场景；MyServer e4a31d8。）

## 阶段 3：客户端主世界场景资源和注册

- 开始时间：2026-08-04 14:29:31 +08:00
- 结束时间：2026-08-04 14:53:53 +08:00
- 开发总结：新增 world.main 主世界客户端场景、首包 manifest/layout 和 session-owned 最小渲染环境。
- 验证记录：cargo fmt、cargo test main_world --lib（9 passed）和 cargo check 通过。

- [x] 在 `project/assets/game/scenes.csv` 增加独立主世界客户端场景记录和专用 `GameSceneUiMode`。（验证：world.main/main_world catalog 测试通过。）
- [x] 新增首包主世界 manifest/layout，声明稳定 scene ID、默认 spawn、相机和必要 required layer。（验证：main_world scene.ron/layout.ron 解析测试通过。）
- [x] 在 `project/src/game/scenes/` 增加主世界注册与组合模块，不把具体场景逻辑堆入 `main.rs`。（验证：main_world.rs 注册于 scenes/mod.rs。）
- [x] 首版生成可见方形平地、悬浮球体、基础相机和光照，并全部挂接到当前 scene session 的根或 owned 边界。（验证：Entered 生成 RuntimeRoot/SceneOwned 子树，9 项场景测试通过。）
- [x] 退出、失败和重新进入时完整清理主世界实体、相机、加载句柄和临时资源。（验证：framework SceneOwned/session root 清理契约及 re-enter 去重测试通过。）
- [x] 验证客户端场景字符串与服务端数值场景通过集中映射关联，不要求两端使用同一种 ID 类型。（验证：main_world 仅使用 MAIN_WORLD_CLIENT_SCENE_ID，numeric ID 保留 main_world_contract。）

## 阶段 4：游戏层进场协调器

- 开始时间：2026-08-04 14:54:42 +08:00
- 结束时间：2026-08-04 15:07:18 +08:00
- 开发总结：新增 generation 隔离的主世界进场协调器，Lobby 以单一意图入口驱动网络前置校验与中止边界。
- 验证记录：cargo fmt、cargo check、cargo test main_world_entry --lib（3 passed）。

- [x] 定义进场意图、状态资源和 typed event，至少覆盖 Lobby idle、校验、加房、加载、等待 ready、active、退出、恢复和失败。（审核：main_world_entry typed phases/intents/events 已定义。）
- [x] 为每次进场分配稳定 request/generation，拒绝旧网络响应或旧 SceneEvent 修改新请求状态。（验证：stale signal 定向测试通过。）
- [x] 校验当前环境、账号会话、选中角色、ticket、game proxy 鉴权和目标映射后才允许加房。（验证：missing auth 无 JoinRequested 测试通过。）
- [x] 同一帧多条命令和跨帧重复点击只形成一个有效进场请求；请求进行中按钮状态由协调器权威驱动。（验证：duplicate enter 单 generation 测试通过。）
- [x] 复用 Scene framework 的 Loading 和生命周期事件，清理 Lobby 当前手写 Loading 与 framework Loading 的重复所有权。（审核：Lobby 不启用 UiLoading，SceneEvent 预留给后续 ready 绑定。）
- [x] 对取消、退出应用、切环境、切角色和退出登录定义明确的中止与清理顺序。（审核：typed abort intents 和状态变化自动中止路径已定义。）

## 阶段 5：固定公共房间加入和权威场景确认

- 开始时间：2026-08-04 15:08:09 +08:00
- 结束时间：2026-08-04 15:20:42 +08:00
- 开发总结：协调器已将固定公共房间加入、权威 scene/角色快照确认、位置转换与可恢复错误闭环接入。
- 验证记录：cargo fmt、cargo check、cargo test main_world_entry --lib（4 passed）；主审核复跑 4 passed。

- [x] game proxy 鉴权成功后发送 `JoinRoom(room_id=main-world-public, policy_id=movement_demo)`。（审核：JoinRequested adapter 发送固定 contract 的 MyServerCommand::JoinRoom。）
- [x] 区分 `RoomJoinRes` 成功与权威 `RoomStatePush`/移动快照到达，不把仅收到 join ack 当作场景已可操作。（审核：RoomJoined 仅记录 ack，匹配角色 MovementSnapshot 才进入 LoadingScene。）
- [x] 校验权威场景 ID 必须映射到 `grassland_01`；未知或不一致场景进入受控失败，不加载错误客户端内容。（审核：room/entity scene_id 必须为 contract server_scene_id=1。）
- [x] 保存 room ID、policy、权威 scene ID、当前角色位置和最近快照 generation，供重连与诊断使用。（审核：entry state 保存 authority fields 与 snapshot generation。）
- [x] 定义服务端二维位置到 Bevy 三维世界坐标的唯一转换，并增加边界、方向和非法数值校验。（验证：固定 x->X/y->Z、0..=16、finite 校验，坐标测试通过。）
- [x] 对固定公共房间满员、policy mismatch、scene mismatch、join timeout 和迟到快照提供可重试错误。（审核：RoomEntryFailure 映射 ROOM_FULL、policy、unavailable、timeout、scene、stale/invalid snapshot。）

## 阶段 6：Scene Ready 与 Room Ready 闭环

- 开始时间：2026-08-04 15:21:35 +08:00
- 结束时间：2026-08-04 15:42:55 +08:00
- 开发总结：权威快照、Scene Ready 与 Room Ready ack 已形成 generation/session 隔离的 active 三门槛闭环。
- 验证记录：cargo fmt、cargo check、cargo test main_world_entry --lib（7 passed）；主审核复跑 7 passed。

- [x] 收到权威场景确认后才向 `SceneCommand` 提交主世界进入请求。（审核：matching movement snapshot 创建 generation-bound session 后发送 SceneCommand::Enter。）
- [x] 客户端 required 资源、根实体、相机和出生点完成后等待 `SceneEvent::Ready`，不得只以 `SceneEvent::Entered` 显示可操作 HUD。（验证：Entered 不触发 ready，matching SceneEvent::Ready 集成测试通过。）
- [x] 当前 generation 的 `SceneEvent::Ready` 到达后发送 `RoomReadyReq(ready=true)`，并关联当前房间与场景 session。（验证：matching session Ready 仅发送一次 MyServerCommand::SetReady(true)。）
- [x] 明确首版进入 active 的门槛：服务端 ready ack、初始权威快照和客户端 Scene ready 均已满足。（验证：authority snapshot + Scene Ready + ReadyChanged 三门槛集成测试通过。）
- [x] ready 前阻断 gameplay 输入和主世界业务按钮；失败时关闭 Loading 并返回可操作 Lobby。（验证：SceneEvent::Failed 映射 SceneLoadFailed 并解除 Lobby 阻断测试通过。）
- [x] 重复 ready、旧 session ready 和迟到 ready ack 不得激活错误场景。（验证：旧 session、duplicate Ready、cancelled late ack 集成测试通过。）

## 阶段 7：退出、断线重连和恢复

- 开始时间：2026-08-04 15:43:58 +08:00
- 结束时间：2026-08-04 16:12:03 +08:00
- 开发总结：补齐主世界退出、本地清理、短线恢复、redirect、成员失效重加和不可恢复登录失败路径。
- 验证记录：cargo fmt、cargo check、cargo test main_world_entry --lib（13 passed）；底层 MyServerEvent 暂无 connection generation，依赖 current-connection filtering 与 RoomReconnected 门槛隔离旧 push。

- [x] 从主世界返回 Lobby 时停止 gameplay 输入，发送 `RoomLeaveReq`，退出 scene session，再切换 Lobby owner/HUD。（验证：active exit 状态机测试通过。）
- [x] `RoomLeaveReq` 超时或连接已断开时仍能完成本地场景清理，同时记录服务端状态未知以供后续重连处理。（验证：lost leave response 测试记录 departure Unknown 并回 Lobby。）
- [x] game proxy 短线时冻结权威输入并保留可恢复场景，使用最新 ticket 执行 game auth 和 `RoomReconnectReq`。（验证：disconnect 冻结 input、保留 scene、ReconnectWithTicket 测试通过。）
- [x] 重连成功后重新应用权威快照、修正场景/位置并恢复 ready；成员已失效时重新加入固定公共房间。（验证：redirect recovery 与 member expiry rejoin 测试通过。）
- [x] session kick、封禁、维护、版本不兼容和不可恢复鉴权失败必须清理房间/场景并回登录页。（验证：fatal account event scene cleanup/logout/login route 测试通过。）
- [x] server redirect 期间保持 generation 隔离，不让旧连接 push 覆盖新 authority 状态。（验证：redirect 在 RoomReconnected 前忽略旧 snapshot 测试通过；底层 event 无 connection generation 的限制已记录。）

## 阶段 8：主世界与家园切换边界

- 开始时间：2026-08-04 16:13:03 +08:00
- 结束时间：2026-08-04 16:38:26 +08:00
- 开发总结：家园作为本地场景运行；进入前离开主城，返回时直接重新加入 main-world-public，失败统一回 Lobby。
- 验证记录：cargo fmt、cargo check、main_world_entry 17 passed、fangyuan_home 52 passed；主审核复跑同结果。

- [x] 确认进入现有 `FANGYUAN_HOME_SCENE_ID` 前是否离开 `main-world-public`，以及家园首版是否完全本地运行。（验证：EnterHome 先 LeaveRoom/Exit 主城，再 Enter 本地家园。）
- [x] 确认从家园返回时直接重新加入主世界，还是先回 Lobby；在决定前不实现隐式返回逻辑。（验证：已确认并实现 ReturnFromHome -> Enter，直接重新加入 main-world-public。）
- [x] 选择可实现的失败回退方案：预加载后切换、失败后重进主世界或统一回 Lobby。（验证：家园加载/返回失败统一回 Lobby。）
- [x] 不依赖当前先卸载旧场景的 `SceneCommand::Switch` 实现“失败后原地恢复”这一不可满足语义。（审核：仅使用 generation session-bound SceneCommand::Enter/Exit。）
- [x] 为快速重复点击家园/返回、切换中断网和家园加载失败增加状态机测试。（验证：main_world_entry 17 passed 覆盖 duplicate、disconnect 和 failure。）

## 阶段 9：测试、性能和文档

- 开始时间：2026-08-04 16:39:24 +08:00
- 结束时间：2026-08-05 09:45:58 +08:00
- 开发总结：补齐真实公共主城 StartRoom、in-game late join recovery、Android 首包 catalog、Scene framework 集成清理和可观测性；修复 KCP/TCP 持续入站流量下的控制请求饥饿、Lobby 覆盖与误退出。Windows 本地网络与 DX12 视觉验收已完成；Android 真机项已转移至 07 清单。
- 验证记录：Windows `cargo run` + 本地 MyServer TCP 完成 Auth/Join/Start/Snapshot/Scene Ready/Room Ready/Active，relay 短断后 connection 3 重鉴权、RoomReconnect、snapshot/ready 恢复 Active，再单次 Leave/scene cleanup 回 Lobby；5.006s 应用更新采样 55.33 FPS、31 ECS entities、9 SceneOwned，Windows 内存 WS 859.84 MiB/Peak WS 952.17 MiB/Private 866.75 MiB。DX12 截图显示平地、球体、相机与光照；当前 AMD R7 360 Vulkan 白屏为仓库已知验收环境限制。最新工作区 `cargo fmt --check`、`cargo check`、main_world 38/38、main_world_entry 24/24、auto_client 2/2、Lobby cleanup 3/3、TCP/KCP 首包和跨帧重连定向测试通过。Android arm64 Release 与 Debug APK 仅作为 07 清单的包体准入证据。

- [x] 增加场景 ID 映射、坐标转换、generation、重复请求、join/ready/leave 和重连状态机单元测试。（验证：main_world_entry 21 passed，含 RoomState 帧不得丢弃 late-join recovery 快照回归。）
- [x] 增加 Scene framework 集成测试，覆盖加载成功、required asset 失败、scene mismatch 和退出清理。（验证：main_world framework manifest 进入/退出测试验证实例化与清理；lifecycle 24 passed 和主世界状态机覆盖 required failure/scene mismatch。）
- [x] 在 MyServer 运行 movement_demo、场景表和房间相关定向测试前确认依赖与执行范围。（验证：启动本地完整 dev stack，公共主城 lifecycle 与 recovery snapshot 定向测试各1 passed。）
- [x] 运行 `cargo fmt`、`cargo check` 和相关客户端定向测试。（验证：cargo fmt -- --check、cargo check、cargo build、main_world_entry 21 passed、Lobby cleanup 3 passed、KCP loopback 1 passed。）
- [x] 使用本机 Windows `cargo run` 客户端连接本地 MyServer，验证鉴权、加入公共房间、权威快照、Scene/Room ready、断线恢复和退出链路。（验证：`logs/stage9/desktop-recovery-final-v3.log` 记录首次 Active、relay 断线、重鉴权、RoomReconnect、recovered Active 和单次退出 Lobby。）
- [x] Windows `cargo run` 客户端验证平地、球体、相机、光照和 Loading，并记录可取得的帧率、实体数和内存数据。（验证：DX12 截图 `logs/stage9/desktop-main-world-dx12-screen.png`；55.33 application updates/s、31 ECS、9 SceneOwned、WS 859.84 MiB/Peak WS 952.17 MiB。）
- [x] 更新 `docs/场景/`、`docs/服务端/` 和必要的上手文档，说明固定公共房间进场链路。（验证：17c3d62 记录 StartRoom/late join/ready/家园语义及 Windows-only 网络验收边界。）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-04 13:40:21 +08:00
- 结束时间：2026-08-05 09:45:58 +08:00
- 验收总结：固定公共主城进场、Authority ready、断线恢复、退出清理和家园往返边界已通过 Windows 本地 MyServer 实跑、客户端自动化、MyServer 定向测试及 DX12 视觉/性能验收；Android 真机公网验收已按范围拆分至 07 清单，不再阻塞本清单归档。

- [x] 已登录并选角的用户可以从 Lobby 加入 `main-world-public / movement_demo` 并进入客户端主世界。（验证：Windows 本地 MyServer 实跑进入 Active，DX12 截图显示主世界内容。）
- [x] 客户端使用服务端 `grassland_01 / scene_id=1 / spawn_id=1001` 权威映射，不新增重复服务端主场景。（验证：final-v3 快照为 scene 1，MyServer 定向 recovery 快照验证 spawn 1001。）
- [x] Scene ready、Room ready 和初始快照均完成后才显示可操作主世界 HUD。（验证：main_world_entry 24/24 与 final-v3 实跑三门槛后 Active。）
- [x] 重复点击、房间满员、策略不一致、加载失败、断网、重连和 session kick 均有确定结果。（验证：main_world_entry 24/24、TCP/KCP 饥饿回归、跨帧旧连接 disconnect 回归及 relay 断线恢复实跑通过。）
- [x] 返回 Lobby 会释放主世界场景、HUD、输入和房间状态，不退出账号或丢失有效角色会话。（验证：final-v3 单次 Leave/scene cleanup 完成并回 Lobby，会话保持。）
- [x] Windows `cargo run` + 本地 MyServer 网络链路、客户端自动化和必要的 MyServer 定向验证全部通过。（验证：详见阶段 9 运行、测试与视觉记录；Android 真机验收不再属于本清单完成门槛。）
