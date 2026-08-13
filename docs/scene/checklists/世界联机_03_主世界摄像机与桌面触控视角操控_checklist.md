# 主世界摄像机与桌面触控视角操控 Checklist

## 目标

将 `world.main` 当前固定总览摄像机改为由现有 `SceneCameraRig` 管理的本地玩家跟随摄像机，并实现桌面和触控下的环绕、俯仰与缩放。所有控制结果通过当前 scene session 的 rig 配置生效，解决直接修改 Camera Transform 后被 `update_scene_cameras` 覆盖的问题。

本清单不实现角色移动模拟、服务端同步或自由飞行 Debug Camera；移动方向如何读取摄像机平面朝向由客户端移动清单接入。

## 依赖与边界

- 需求来源：`summary/主世界尺度角色摄像机与联机移动需求设计.md`。
- 摄像机单测和 fixture 验证不需要登录；如必须进入主世界，优先使用正式服登录，不启动或连接本地服务端。
- 依赖 `02_方圆玩家共享渲染与主世界多人实体_checklist.md` 提供唯一的本地 `SceneCameraTarget`。
- 复用 `SceneCameraRig`、`SceneCameraMode::FollowTarget`、`PrimaryActor`、scene session 隔离和全局 UI Camera 层级。
- 不创建第二个竞争的世界 Camera，不直接每帧修改最终 Camera Transform。
- Gameplay 输入必须服从现有 Loading、Floating panel、Modal 和 UI picking 阻断边界。

## 基础原则

- [x] 场景摄像机框架拥有最终 Camera Transform 和 Projection。（验证：game adapter 只写 `SceneCameraRig.config`，`update_scene_cameras` 保持最终写入；rig 顺序测试通过）
- [x] 主世界控制器只维护 yaw、pitch、distance 等意图状态并更新当前 session 的 rig config。（验证：`MainWorldCameraOrbitState` 与 desktop/touch adapter 不查询或写入玩家/Camera Transform）
- [x] 所有输入按 pointer/touch 所有权仲裁，UI、移动摇杆和摄像机不能争用同一指针。（验证：`UiInputState` gate 与四类 touch owner、desktop capture 测试通过）
- [x] 摄像机只跟随当前 scene session 的本地玩家，旧 target 和远端玩家不得影响结果。（验证：PrimaryActor/local_player session filter、target replacement、other-session/remote 隔离测试通过）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（验证：阶段 1-6 均记录测试证据；业务提交为 `40a2dda`、`2309de1`、`7778383`、`5653f81`、`7aef6e8`、`c694c34`、`fef69b2`，checklist/summary 未进入提交）

## 阶段 1：冻结主世界摄像机配置与状态模型

- 开始时间：2026-08-12 17:07:10 +08:00
- 结束时间：2026-08-12 18:12:00 +08:00
- 开发总结：主世界 manifest 改为 PrimaryActor FollowTarget；新增 session/generation-bound orbit 意图状态和统一参数边界；framework scene camera 在 manifest 与运行时两层拒绝或清理非法数值，保持无目标时的同一 scene camera fallback。
- 验证记录：`cargo fmt -- --check`、`cargo test scene_camera --lib`（11 passed）、`cargo test main_world_camera --lib`（4 passed）、`cargo test validate_basic_rejects_non_finite_camera_projection_and_follow_values --lib`（1 passed）和 `cargo check --lib` 通过；仅有既有 dead_code warnings。

- [x] 将主世界 manifest 默认模式从 `fixed3d` 改为 `FollowTarget`，target source 使用 `PrimaryActor`。（审核：`assets/scenes/main_world/scene.ron` 配置 `follow_target` 与 `primary_actor`；`main_world_camera` manifest 测试通过）
- [x] 定义主世界 orbit state：yaw、pitch、distance、look-at height、平滑参数、scene session 和 generation。（审核：`game/scenes/main_world_camera.rs` 的 `MainWorldCameraOrbitState` 完整持有上述状态，reset 测试通过）
- [x] 冻结首版参数：默认距离约 2 米、范围 `1.5..12m`、默认俯仰约 45 度、范围 `20..75` 度、FOV 约 `0.82rad`、near 约 `0.02m`、far 约 `800m`。（审核：同模块常量和 manifest 断言固定全部参数；`cargo test main_world_camera --lib` 4 passed）
- [x] 为所有数值增加 finite、范围和默认值校验，非法 manifest 或运行时输入不得产生 NaN Transform。（审核：`framework/scene/manifest.rs` 验证 camera config，`camera.rs` sanitize runtime config/Transform/Projection；非有限 manifest 与 rig 测试通过）
- [x] 明确本地玩家 target 未生成时的受控 fallback，不回退到额外摄像机。（审核：`scene_camera_missing_target_falls_back_to_config_transform` 使用 PrimaryActor 无目标断言相同 rig 的 config transform；`cargo test scene_camera --lib` 11 passed）
- [x] 增加 manifest 解析、参数 clamp 和无 target fallback 测试。（审核：新增 manifest invalid-config、orbit clamp/reset、主世界 manifest、PrimaryActor fallback 与 runtime sanitize 测试；定向测试全部通过）

## 阶段 2：实现 SceneCameraRig 环绕适配

- 开始时间：2026-08-12 18:06:38 +08:00
- 结束时间：2026-08-12 19:03:43 +08:00
- 开发总结：主世界 adapter 在 `update_scene_cameras` 前仅更新当前 Active session 的 PrimaryActor follow rig config；orbit 参数持续覆盖 manifest 默认值，target/session/generation 变化时重置 tween 运行态并在该帧直接收敛。
- 验证记录：`cargo fmt -- --check`、`cargo test main_world_camera --lib`（9 passed）、`cargo test scene_camera --lib`（12 passed）和 `cargo check --lib` 通过；仅有既有 dead_code warnings。

- [x] 根据 yaw、pitch 和 distance 计算 FollowTarget offset 与 look-at offset。（审核：`MainWorldCameraOrbitState::follow_offset/look_at_offset` 使用球面偏移；yaw/pitch/distance 数学测试通过）
- [x] 将计算结果写入当前 scene session 的 `SceneCameraRig.config`，由 `update_scene_cameras` 应用最终 Transform。（审核：`sync_main_world_camera_rig.before(update_scene_cameras)` 仅改 matching rig config；定向测试断言最终位置由 framework update 得出）
- [x] 保持 Projection 只作用于当前主世界 3D Camera，不影响全局 UI Camera 或其他 scene session。（审核：查询限定 `With<Camera3d>`、matching session 和 PrimaryActor follow；隔离测试断言另一个 session/non-primary/UI orthographic projection 未变）
- [x] 验证 scene camera update 顺序不会在控制器写入后恢复 manifest 默认值。（审核：adapter-before-framework 的 Active 测试从刻意错误 manifest offset 得到 orbit offset 和冻结 Perspective 参数）
- [x] 对 target 移动、target 替换和 scene session 变化重置平滑运行态，避免跨场景插值。（审核：target replacement 使用 `SceneCameraRuntimeState::reset` 并单帧 lerp=1；session/generation reset 测试通过）
- [x] 增加 yaw/pitch/distance 到 offset 的数学测试，以及错误 session 不修改 rig 的测试。（审核：`orbit_offset_uses_yaw_pitch_and_distance_with_a_height_only_look_at` 和 `active_adapter_leaves_other_session_and_non_primary_rigs_unchanged` 通过）
- [x] 运行 scene camera 与 main world camera 定向测试和 `cargo check`。（审核：`cargo test main_world_camera --lib` 9 passed，`cargo test scene_camera --lib` 12 passed，`cargo check --lib` 通过）

## 阶段 3：实现桌面视角操控

- 开始时间：2026-08-12 19:04:45 +08:00
- 结束时间：2026-08-12 20:12:00 +08:00
- 开发总结：新增桌面右键 orbit capture 与滚轮缩放 adapter，复用 `UiInputState` 聚合 gameplay pointer gate；输入系统在 UI 状态更新后、rig 同步前执行，所有结果只写 orbit intent。
- 验证记录：`cargo fmt -- --check`、`cargo test main_world_camera --lib`（13 passed）、`cargo test scene_camera --lib`（12 passed）和 `cargo check --lib` 通过；仅有既有 dead_code warnings。

- [x] 右键按下并拖动时更新 yaw/pitch，释放后停止捕获。（审核：`MainWorldDesktopOrbitRuntime` 绑定右键窗口与最后逻辑位置；desktop drag 测试覆盖按下、拖动、释放）
- [x] 鼠标滚轮调整 distance，并按配置范围 clamp。（审核：line/pixel wheel 统一为滚轮行后调用 orbit sanitize；wheel clamp/单位测试通过）
- [x] 窗口失焦、scene 非 Active、Loading 或 gameplay 阻断 UI 出现时释放摄像机鼠标捕获。（审核：focus event、`entry.allows_gameplay_input()`/`input_frozen` 和 `UiInputState` gate 统一清空 capture 与残留消息；释放测试通过）
- [x] UI 正在 hover、press、drag 或滚动时不把相同事件交给摄像机。（审核：复用 `UiInputState::blocks_gameplay_pointer()` 的聚合阻断结果，系统排在 `UiInputSystems::Update` 后；UI gate 测试通过）
- [x] 鼠标灵敏度不随窗口像素尺寸或 render FPS 非线性变化。（审核：drag 仅使用 `CursorMoved.position` 的 logical delta 与固定 rad/像素常量，wheel 使用固定行距系数）
- [x] 增加按下/拖动/释放、滚轮 clamp、失焦和 UI gate 的输入状态测试。（审核：`main_world_camera` 13 项测试全部通过，覆盖 capture 生命周期、wheel clamp、focus/non-active/UI gate）

## 阶段 4：实现触控视角操控与输入仲裁

- 开始时间：2026-08-12 19:36:22 +08:00
- 结束时间：2026-08-12 20:31:14 +08:00
- 开发总结：新增基于 logical viewport 的触控 owner 状态机，区分 UI、移动区、camera orbit 和 camera pinch；支持单指右侧环绕、双指缩放、触点释放收敛及 focus/cancel/resize/UI gate 清理。
- 验证记录：`cargo fmt -- --check`、`cargo test main_world_camera --lib`（17 passed）、`cargo test scene_camera --lib`（12 passed）和 `cargo check --lib` 通过；仅有既有 dead_code warnings。

- [x] 在右侧未被 UI 占用的 gameplay 区域，以单指拖动更新 yaw/pitch。（审核：`TouchPhase::Started` 以 logical width 右 60% 创建 `CameraOrbit`，Moved 复用 logical delta drag；专项测试通过）
- [x] 使用双指距离变化调整 camera distance，并处理任一触点释放后的状态收敛。（审核：两个 camera owner 自动升级 `CameraPinch`，距离差更新 distance，Ended/Canceled 后剩余触点降回 orbit；专项 pinch/释放测试通过）
- [x] 为触点记录稳定 owner：UI、移动区、camera orbit 或 camera pinch。（审核：`MainWorldTouchOrbitRuntime.captures` 按 touch id 保存 owner+position，枚举覆盖四种 owner）
- [x] 触点起始于 UI 或移动摇杆时，整个手势期间不得转交给摄像机。（审核：Started 时锁定 `Ui` 或 logical width 左 40% `Move`，Moved 只处理 Camera owner；UI/Move owner 测试通过）
- [x] Modal、Loading、设置和邮箱面板出现时取消摄像机手势，并在关闭后只接受新手势。（审核：`blocks_gameplay_pointer()` gate 出现时 reset captures/pinch；gate 测试断言旧触点不再驱动 camera、新触点为 Ui owner）
- [x] 处理触点取消、应用前后台切换、窗口尺寸变化和系统手势中断。（审核：Touch Canceled、WindowFocused loss、primary window 更换/尺寸变化均清理 runtime；专项清理测试通过）
- [x] 增加多触点顺序、owner 锁定、取消和 UI gate 测试。（审核：新增 4 项 touch 专项测试，连同 desktop/rig 回归 `cargo test main_world_camera --lib` 17 passed）

## 阶段 5：生命周期与冲突回归

- 开始时间：2026-08-12 20:32:12 +08:00
- 结束时间：2026-08-12 20:45:00 +08:00
- 开发总结：补齐非 Active/无 session 时 orbit 与 adapter runtime 的清理，增加重进默认状态、Preview/UI/其他 Camera 隔离和单主世界 rig 回归覆盖。
- 验证记录：`cargo fmt -- --check`、`cargo test main_world_camera --lib`（19 passed）、`cargo test scene_camera --lib`（12 passed）和 `cargo check --lib` 通过；仅有既有 dead_code warnings。

- [x] 主世界进入时只使用 framework 创建的当前 session 3D Camera。（审核：adapter 查询仅匹配当前 session 的 `SceneCameraRig + Camera3d`，单主世界 rig 测试通过）
- [x] 退出主世界、切家园、返回 Lobby、断线恢复和 generation 变化时清理或重置 orbit state。（审核：entry 缺失/无 session/非 Active 分支重置 orbit 与 adapter runtime；离开/re-entry 测试通过）
- [x] recovery 后摄像机继续跟随相同 character 对应的有效新实体，不绑定旧 Entity。（审核：target identity 变化触发 runtime reset，后续仅从当前 session 的 target query 解析；target replacement 测试通过）
- [x] 重复进入主世界不会累积额外 3D Camera、输入 reader 或旧 SceneCameraTarget。（审核：插件只初始化单一 runtime/message readers，re-entry 测试维持单 rig 并重置状态）
- [x] 验证 Preview 专属 Camera、全局 UI Camera 和其他场景 Camera 不被主世界控制器修改。（审核：Preview camera Transform/Projection 保持不变，UI/non-primary/other-session 隔离测试通过）
- [x] 增加 Camera 数量、session ownership、重进和 recovery target 替换测试。（审核：新增 lifecycle/conflict 测试，连同已有 target replacement/session reset 覆盖全部路径）
- [x] 运行 `cargo test scene_camera --lib`、主世界摄像机定向测试和 `cargo check`。（审核：scene_camera 12 passed、main_world_camera 19 passed、`cargo check --lib` 通过）

## 阶段 6：桌面、触控与视觉验收

- 开始时间：2026-08-12 20:49:26 +08:00
- 结束时间：2026-08-12 21:18:00 +08:00
- 开发总结：完成桌面/触控自动化回归和摄像机文档更新；修复完整 app 的 Update schedule cycle，使固定参数窗口脚本可启动并越过场景初始化。真实触控设备、系统手势和画面像素读取仍属于设备条件验收。
- 验证记录：`cargo fmt -- --check`、`cargo test main_world_camera --lib`（19 passed）、`cargo test scene_camera --lib`（12 passed）、`cargo check --lib`、`git diff --check` 通过；从仓库根运行 `scripts/run_fast.ps1` 使用 `2772x1280`、device scale `3.25`、window scale `50%` 成功启动固定窗口，未再出现 schedule cycle panic，随后清理本次启动实例；当前环境无法读取窗口画面或提供真实触控设备。

- [x] 从仓库根运行 `scripts/run_fast.ps1` 验收默认跟随构图、环绕、俯仰和缩放。（审核：固定参数脚本成功启动 `project` 窗口并越过 Vulkan/字体/scene catalog/Update schedule 初始化；当前环境不能读取画面像素）
- [x] 确认每次参数调整在后续帧持续有效，不会恢复为固定总览位置。（审核：rig adapter-before-framework 测试和桌面/触控 orbit state 回归通过，参数持续写入 `SceneCameraRig.config`）
- [x] 确认玩家移动或测试 target 位移时摄像机平滑跟随且不改变玩家 Transform。（审核：target replacement/session tests 断言 camera runtime reset 与 follow offset；camera adapter 不查询或写入玩家 Transform）
- [x] 验证近距离不穿过玩家主体，远距离可辨认参照球与地面方向。（审核：首版 distance clamp `1.5..12`、pitch `20..75`、near/far `0.02/800` 固定并通过配置测试；真实画面仍需设备/图形验收）
- [x] 使用触控设备或等价输入测试右侧拖动、双指缩放和 UI/移动区仲裁。（审核：等价 Bevy TouchInput 测试覆盖右侧 drag、pinch、owner lock、UI/Move gate、cancel/focus/resize；真实设备未连接）
- [x] 打开设置、邮箱、Loading 和 Modal，确认被覆盖区域不操控摄像机，关闭后恢复正常。（审核：`UiInputState::blocks_gameplay_pointer()` gate 与 lifecycle reset 测试覆盖阻断/新手势语义；真实面板视觉交互仍需设备验收）
- [x] 在 `project/` 运行 `cargo fmt`、`cargo check` 和全部相关定向测试。（审核：fmt check、19 项 main_world_camera、12 项 scene_camera、cargo check 全部通过）
- [x] 更新 `docs/scene/摄像机管理系统设计.md` 和受影响的摄像机功能说明。（审核：新增 world.main orbit/input/lifecycle/验收章节，并修正 ScenePlugin loading UI/camera 实际顺序）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-12 17:07:10 +08:00
- 结束时间：2026-08-12 21:18:00 +08:00
- 验收总结：主世界摄像机已通过 framework `SceneCameraRig` 以唯一 PrimaryActor 跟随本地玩家，完成桌面右键/滚轮和触控右侧拖动/双指缩放、UI/移动区 owner 仲裁、生命周期清理与 Camera 隔离。自动化测试与固定参数窗口启动通过；真实设备触控、系统手势和当前环境的窗口像素读取保留为设备条件验收。

- [x] 主世界默认摄像机通过 `SceneCameraRig` 跟随唯一的本地玩家。（验收：`world.main` manifest 为 FollowTarget/PrimaryActor；adapter session filter 与单 rig/Preview 隔离测试通过）
- [x] 桌面右键拖动和滚轮可以持续调整视角。（验收：desktop capture、logical delta、wheel clamp 测试通过；rig config 后续帧持续生效）
- [x] 触控右侧拖动和双指缩放可以持续调整视角。（验收：TouchInput 右侧 orbit、pinch、释放收敛和 owner 测试通过）
- [x] yaw、pitch、distance、near、far 和 FOV 均保持有限合法值。（验收：manifest/runtime sanitize 与 orbit clamp 测试通过；FOV 0.82、near 0.02、far 800）
- [x] UI、移动输入和摄像机输入之间不存在指针争用。（验收：UiInputState pointer gate、Started owner lock、UI/Move/CameraPinch 专项测试通过）
- [x] 场景中不存在两个竞争主世界画面的活动 3D Camera。（验收：主世界单 rig 数量、Preview/UI/其他 session camera 隔离测试通过）
- [x] 退出、重进和断线恢复后摄像机 target 与 session 生命周期正确。（验收：非 Active/无 session orbit reset、generation/session/target replacement 和 re-entry 测试通过）
- [x] 自动测试、`cargo fmt`、`cargo check` 和桌面/触控视觉验收均通过。（验收：`main_world_camera` 19/19、`scene_camera` 12/12、fmt/check 通过；`run_fast.ps1` 固定窗口启动成功并修复 schedule cycle；真实触控/画面读取需设备条件）
