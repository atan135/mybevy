# 摄像机管理系统第一阶段 Checklist

## 目标

基于 [docs/scene/摄像机管理系统设计.md](../docs/scene/摄像机管理系统设计.md)，完成 MyBevy 第一阶段摄像机管理系统从文档到实现的闭环。交付内容至少支持固定视角、跟随视角、可配置摄像机动画或过渡，并在 Robot Sync 玩法中完成实际接入和验收；同时明确镜头特效与渲染特效边界，不在第一阶段实现完整后处理渲染栈。

## 基础原则

- [ ] 保持 `project/src/framework/scene/` 作为通用摄像机能力边界，具体玩法策略留在 `project/src/game/`。
- [ ] 复用现有 `SceneCameraRig`、scene manifest、`SceneOwned(session_id)` 和全局 UI 相机层级，不另起不受 scene lifecycle 管理的相机系统。
- [ ] 不改变 Robot Sync 的 authority、replay、network、MyServer、checksum 和 HUD snapshot 语义。
- [ ] Robot Sync 跟随视角必须保留远端玩家可见性或提供可回退总览视角。
- [ ] 每个阶段完成后运行对应验证，并记录验证结果后再提交。

## 阶段 1：文档和边界同步

- 开始时间：2026-06-26 12:31:06 +08:00
- 结束时间：2026-06-26 12:37:14 +08:00
- 开发总结：同步摄像机管理第一阶段文档边界，明确阶段 1 只做文档和范围确认，后续实现再落地 framework/runtime 与 Robot Sync 接入。
- 验证记录：`git diff --check -- docs/scene/摄像机管理系统设计.md docs/scene/场景框架层功能说明.md docs/scene/游戏层场景使用说明.md` 通过，仅有 LF/CRLF 换行提示；审查 diff 确认只修改允许的 3 个 scene 文档。

- [x] 复核 `docs/scene/摄像机管理系统设计.md`，确认第一阶段范围只包含固定、跟随、动画/过渡和 transform/projection 类镜头效果。（验证：docs/scene/摄像机管理系统设计.md 阶段 1 文档同步范围列出固定、跟随、动画/过渡和 transform/projection 类镜头效果）
- [x] 更新 `docs/scene/场景框架层功能说明.md`，把当前“基础相机生成”说明调整为待实现或已实现的摄像机管理能力说明。（验证：docs/scene/场景框架层功能说明.md 将 `camera.rs` 描述为基础生成已实现，并列出固定、跟随和动画/过渡为后续扩展边界）
- [x] 更新 `docs/scene/游戏层场景使用说明.md`，补充 Robot Sync 摄像机接入策略、默认总览和跟随视角验收方式。（验证：docs/scene/游戏层场景使用说明.md 新增 Robot Sync 摄像机接入策略和默认总览、FollowLocal、过渡、退出验收口径）
- [x] 如实现影响新成员上手流程，检查并更新 `docs/bevy-getting-started.md` 中与场景相机相关的说明。（验证：阶段 1 只同步 scene 文档、不改变启动命令或上手流程，docs/bevy-getting-started.md 无需修改）
- [x] 明确镜头特效归属：transform/projection 类归摄像机系统，后处理、屏幕空间、材质和粒子归渲染/VFX 系统。（验证：docs/scene/摄像机管理系统设计.md 和 docs/scene/场景框架层功能说明.md 均记录该边界）
- [x] 确认本阶段不实现完整后处理栈、复杂 cutscene timeline、摄像机剪辑资源格式。（验证：docs/scene/摄像机管理系统设计.md 阶段 1 范围和 docs/scene/场景框架层功能说明.md 当前未实现清单均明确排除这些内容）

## 阶段 2：框架层配置和 Manifest 支持

- 开始时间：2026-06-26 12:39:54 +08:00
- 结束时间：2026-06-26 12:53:35 +08:00
- 开发总结：扩展 framework scene camera 配置和 manifest 解析，新增 FollowTarget、follow/animation 配置结构、easing 解析和 prelude 导出，并保留旧相机模式默认映射兼容。
- 验证记录：在 `project/` 下运行 `cargo fmt --check`、`cargo test scene --lib`（113 passed）、`cargo check` 均通过；`git diff --check -- project/src/framework/scene/camera.rs project/src/framework/scene/manifest.rs project/src/framework/scene/prelude.rs` 通过，仅有 LF/CRLF 换行提示。

- [x] 在 `project/src/framework/scene/camera.rs` 中扩展 `SceneCameraMode`，加入跟随视角模式并保持现有 `UiOnly2d`、`Gameplay2d`、`Gameplay3d`、`Fixed3d`、`DebugFree` 兼容。（验证：project/src/framework/scene/camera.rs:172 定义 `FollowTarget`，`cargo test scene --lib` 通过）
- [x] 扩展 `SceneCameraConfig`，支持 follow 配置和 animation 配置，并保持现有构造函数默认行为兼容。（验证：project/src/framework/scene/camera.rs:34 增加 `follow` 和 `animation` 字段，既有构造函数均补默认值；`cargo check` 通过）
- [x] 增加 `SceneCameraFollowConfig`，覆盖目标来源、offset、look_at_offset、position_lerp、rotation_lerp 和多人可见性保底参数。（验证：project/src/framework/scene/camera.rs:197 定义这些字段，project/src/framework/scene/manifest.rs:1354 的跟随解析测试通过）
- [x] 增加 `SceneCameraAnimationConfig` 和 easing 配置，至少支持 `linear`、`smooth_step`、`ease_in_out`。（验证：project/src/framework/scene/camera.rs:231 定义动画配置和 easing，project/src/framework/scene/manifest.rs:650 解析 `linear`、`smoothstep`、`easeinout`）
- [x] 扩展 `project/src/framework/scene/manifest.rs` 的 manifest 解析，支持 `mode: "follow_target"`、`follow` 和 `animation` 字段。（验证：project/src/framework/scene/manifest.rs:408 增加 manifest 字段，project/src/framework/scene/manifest.rs:1354 和 :1399 测试覆盖跟随与动画解析）
- [x] 更新 `SceneCameraRef`、字符串引用和默认相机模式映射，保证旧 manifest 中 `fixed3d`、`gameplay3d` 等写法不破坏。（验证：project/src/framework/scene/manifest.rs:669 和 :680 更新映射，project/src/framework/scene/manifest.rs:1318、:1433 测试覆盖 fixed3d/gameplay3d 兼容和未知模式默认）
- [x] 增加 manifest 单元测试，覆盖固定相机兼容、跟随相机解析、动画配置解析、未知或缺省字段的默认行为。（验证：project/src/framework/scene/manifest.rs:1318、:1354、:1399、:1433 新增测试，`cargo test scene --lib` 113 passed）

## 阶段 3：框架层运行时管理

- 开始时间：2026-06-26 12:55:29 +08:00
- 结束时间：2026-06-26 13:13:54 +08:00
- 开发总结：实现 framework scene camera runtime，新增目标标记、目标解析、固定/跟随 transform 更新、轻量 tween 和 ScenePlugin 调度注册；projection 目前按配置立即应用到 scene camera，不实现 FOV tween。
- 验证记录：在 `project/` 下运行 `cargo fmt --check`、`cargo test scene_camera --lib`（9 passed）、`cargo test scene --lib`（119 passed）、`cargo check` 均通过；`git diff --check -- project/src/framework/scene/camera.rs project/src/framework/scene/plugin.rs project/src/framework/scene/prelude.rs` 通过，仅有 LF/CRLF 换行提示。

- [x] 增加通用 `SceneCameraTarget` 组件或等价目标标记，包含 `session_id`、`tag`、`priority` 等目标解析信息。（验证：project/src/framework/scene/camera.rs:32 定义 `SceneCameraTarget { session_id, tag, priority }`）
- [x] 增加相机目标解析逻辑，支持当前 session 内按 anchor、target tag 或本地玩家目标查找。（验证：project/src/framework/scene/camera.rs:510 解析 Anchor、SceneTarget、PrimaryActor、AllParticipants；project/src/framework/scene/camera.rs:788 和 :817 测试覆盖 tag 与 anchor）
- [x] 增加 scene camera update system，按 `SceneCameraRig` 应用固定视角、跟随视角和动画过渡。（验证：project/src/framework/scene/camera.rs:443 定义 `update_scene_cameras`，project/src/framework/scene/plugin.rs:52 注册系统）
- [x] 实现跟随视角的 position offset、look_at_offset、位置平滑和旋转平滑。（验证：project/src/framework/scene/camera.rs:484 应用 offset/look_at_offset 和 lerp/slerp，`cargo test scene_camera --lib` 通过）
- [x] 实现固定与跟随之间的轻量 tween，duration 为 0 时立即完成，duration 大于 0 时按 easing 插值。（验证：project/src/framework/scene/camera.rs:607 实现 `scene_camera_apply_animation`，project/src/framework/scene/camera.rs:660 实现 easing sample，project/src/framework/scene/camera.rs:860 测试覆盖插值）
- [x] 如支持 FOV 过渡，保证只影响当前 scene camera 的 projection，不影响全局 UI 相机。（验证：本阶段未实现 FOV tween；project/src/framework/scene/camera.rs:459 仅对查询到的 `SceneCameraRig` 相机应用 projection，`cargo test scene_camera --lib` 中 `global_ui_camera_order_is_above_scene_cameras` 通过）
- [x] 将相机更新系统注册到 `ScenePlugin` 的合适阶段，并避免与 lifecycle 创建/清理顺序冲突。（验证：project/src/framework/scene/plugin.rs:44-54 将 `update_scene_cameras` 放在 lifecycle/trigger/streaming 后、loading UI sync 前的 chain 中）
- [x] 处理目标缺失、stale session、目标被 despawn、无世界 root 等失败路径，要求不 panic 并可回退默认视角。（验证：project/src/framework/scene/camera.rs:515 缺失 session 回退，:895 和 :911 测试覆盖目标缺失与 session 隔离）
- [x] 增加框架层单元测试，覆盖目标跟随、动画插值、目标缺失回退、session 隔离和场景退出清理。（验证：project/src/framework/scene/camera.rs:788、:860、:895、:911、:940 测试覆盖对应行为，`cargo test scene_camera --lib` 9 passed）

## 阶段 4：Robot Sync 玩法接入

- 开始时间：2026-06-26 13:15:57 +08:00
- 结束时间：2026-06-26 13:29:32 +08:00
- 开发总结：Robot Sync visual 为本地玩家挂载 scene camera target，新增临时 `C` 键在默认总览和 FollowLocal 间切换；默认 manifest fixed3d 总览保持不变，切换只修改当前 scene camera rig 配置。
- 验证记录：在 `project/` 下运行 `cargo fmt --check`、`cargo test robot_sync --lib`（88 passed）、`cargo check` 均通过；`git diff --check -- project/src/game/features/robot_sync/plugin.rs project/src/game/features/robot_sync/visual.rs` 通过，仅有 LF/CRLF 换行提示；`project/assets/scenes/robot_sync_arena/scene.ron` 无 diff。

- [x] 在 Robot Sync 本地玩家 visual 生成或更新时挂相机目标标记，确保目标 `session_id` 与当前 scene session 一致。（验证：project/src/game/features/robot_sync/visual.rs:304 插入 `SceneCameraTarget::new(session_id.clone())`，project/src/game/features/robot_sync/visual.rs:724 测试校验 session/tag/priority）
- [x] 确保 stale session 的 robot visual 或目标标记不会被当前相机跟随。（验证：project/src/game/features/robot_sync/visual.rs:317 远端或非本地 visual 移除 target，project/src/game/features/robot_sync/visual.rs:1144 测试覆盖 stale session target 清理）
- [x] 为 Robot Sync 增加总览与本地跟随的切换入口，入口可以是调试配置、环境变量或临时按键。（验证：project/src/game/features/robot_sync/plugin.rs:35 定义 `C` 键，project/src/game/features/robot_sync/plugin.rs:71 切换当前 session camera rig）
- [x] 保持 `project/assets/scenes/robot_sync_arena/scene.ron` 默认总览视角可用，避免双客户端联调默认丢失远端可见性。（验证：project/assets/scenes/robot_sync_arena/scene.ron 无 diff，仍为 `mode: "fixed3d"`、`anchor.camera_target` 默认总览）
- [x] 增加或配置 Robot Sync 跟随相机参数，使用高俯视 offset 和合理 FOV，保留远端可见性或可快速回退总览。（验证：project/src/game/features/robot_sync/plugin.rs:104 配置 FollowLocal offset `(0,42,52)`、FOV `0.78` 和 0.25s 过渡，project/src/game/features/robot_sync/plugin.rs:90 可切回 overview）
- [x] 确认相机系统不读取或写入 authority replay state，不改变 `robot_move` payload、frame apply、checksum 或 HUD snapshot。（验证：改动仅在 plugin camera toggle 和 visual target marker；project/src/game/features/robot_sync/plugin.rs:1184 测试 `C` 键不发送 `AuthorityCommand`，`cargo test robot_sync --lib` 中 checksum/HUD/payload 相关测试通过）
- [x] 补充 Robot Sync 相关测试，覆盖本地玩家目标标记、stale session 隔离、退出清理和相机切换命令。（验证：project/src/game/features/robot_sync/visual.rs:724、:753、:1144、:1235 和 project/src/game/features/robot_sync/plugin.rs:1093、:1184 覆盖对应行为，`cargo test robot_sync --lib` 88 passed）

## 阶段 5：验收测试和平台检查

- 开始时间：2026-06-26 13:31:32 +08:00
- 结束时间：2026-06-26 14:14:34 +08:00
- 开发总结：自动化验收全部通过；受控窗口启动 phone/tablet Robot Sync 并截图确认 3D 场景、HUD 层级、phone 相机切换、大屏构图和返回大厅；受控 LAN 双客户端启动确认两名机器人同屏可见并清理进程。
- 验证记录：`cargo fmt --check`、`cargo test robot_sync --lib`（88 passed）、`cargo test scene_camera --lib`（10 passed）、`cargo test scene --lib`（120 passed）、`cargo check`、`git diff --check` 均通过；曾因中断 `cargo run` 后 debug 增量产物损坏导致 `cargo build` linker 失败，执行 `cargo clean -p project` 后 `cargo build` 通过；受控截图包含 `%TEMP%/mybevy-camera-realhwnd-check-20260626-140302/hidden-overview.png`、`hidden-follow.png`、`hidden-back-overview.png`、`%TEMP%/mybevy-camera-tablet-check-20260626-135540/tablet-landscape-direct.png`、`%TEMP%/mybevy-two-client-camera-check-20260626-140737/two-clients-screen.png` 和 `%TEMP%/mybevy-lobby-visible-click-20260626-141301/after-visible-click.png`，结束后确认无 `project/cargo/rustc/link` 残留进程。

- [x] 在 `project/` 下运行 `cargo fmt --check`。（验证：2026-06-26 在 project/ 执行通过）
- [x] 在 `project/` 下运行 `cargo test robot_sync --lib`。（验证：2026-06-26 在 project/ 执行通过，88 passed）
- [x] 在 `project/` 下运行覆盖 scene camera/manifest 的相关单元测试。（验证：2026-06-26 在 project/ 执行 `cargo test scene_camera --lib` 10 passed、`cargo test scene --lib` 120 passed）
- [x] 在 `project/` 下运行 `cargo check`。（验证：2026-06-26 在 project/ 执行通过）
- [x] 使用 `cargo run -- --window-profile phone-portrait` 手动检查 Robot Sync 默认总览、跟随视角、过渡动画和 HUD 层级。（验证：受控 phone 窗口截图 `%TEMP%/mybevy-camera-realhwnd-check-20260626-140302/hidden-overview.png`、`hidden-follow.png`、`hidden-back-overview.png` 显示 HUD 可隐藏、按 `C` 进入近距跟随、再按 `C` 回到较远视角，进程清理为 0）
- [x] 使用 `cargo run -- --window-profile tablet-landscape` 手动检查大屏构图、远端玩家可见性和相机切换。（验证：`%TEMP%/mybevy-camera-tablet-check-20260626-135540/tablet-landscape-direct.png` 显示 tablet-landscape 下 3D 场景、机器人和 HUD 层级；双客户端截图 `%TEMP%/mybevy-two-client-camera-check-20260626-140737/two-clients-screen.png` 显示两名机器人同屏可见，phone 相机切换由上一项截图覆盖）
- [x] 使用 `MYBEVY_START_SCENE="arena.robot_sync"` 直接进入 Robot Sync 场景，验证返回大厅后相机和目标不残留。（验证：受控 direct-start 截图 `%TEMP%/mybevy-lobby-visible-click-20260626-141301/before-visible-click.png` 进入 Robot Sync，点击“大厅”后 `%TEMP%/mybevy-lobby-visible-click-20260626-141301/after-visible-click.png` 显示退出到桌面/大厅路径，进程清理为 0；`cargo test robot_sync --lib` 覆盖 visual/camera target 退出清理）
- [x] 如环境允许，运行双客户端 Robot Sync 脚本，确认 LAN/MyServer 联调中跟随视角不会破坏远端玩家观察和同步表现。（验证：按 `scripts/start-robot-sync-two-clients.ps1` 的 LAN env 等价受控启动两个 `project.exe` 到临时日志，`client-a.log`/`client-b.log` 均出现 `robot_count=2` frame applied，截图 `%TEMP%/mybevy-two-client-camera-check-20260626-140737/two-clients-screen.png` 显示远端玩家同屏可见；未使用脚本本身的 `-NoExit` 窗口以避免残留）
- [x] 运行 `git diff --check`，确认没有空白错误。（验证：2026-06-26 在仓库根目录执行通过）

## 阶段 6：文档归档和最终同步

- 开始时间：2026-06-26 14:16:16 +08:00
- 结束时间：2026-06-26 14:29:27 +08:00
- 开发总结：根据最终实现同步摄像机设计、框架层说明、游戏层 Robot Sync 使用说明和上手文档，明确已完成能力、未完成项、`C` 键临时切换入口和后续正式命令/profile 方向；随后归档 checklist。
- 验证记录：`git diff --check -- docs/scene/摄像机管理系统设计.md docs/scene/场景框架层功能说明.md docs/scene/游戏层场景使用说明.md docs/bevy-getting-started.md` 通过，仅有 LF/CRLF 换行提示；归档前检查阶段 1-6 均已有开始时间、结束时间、开发总结和验证记录。

- [x] 根据最终实现更新 `docs/scene/摄像机管理系统设计.md`，把已完成能力、未完成能力和后续目标区分清楚。（验证：docs/scene/摄像机管理系统设计.md:5 记录已完成 FollowTarget、SceneCameraTarget、Transform tween 和 `C` 键，:459 记录 FOV tween、后处理、timeline、clip、profile、SceneCameraCommand 未完成）
- [x] 根据最终实现更新 `docs/scene/场景框架层功能说明.md` 的当前能力清单、模块职责和系统调度说明。（验证：docs/scene/场景框架层功能说明.md:72 更新 camera.rs 职责，:120 记录 `update_scene_cameras` 调度位置，:473 记录目标标记和 session 隔离）
- [x] 根据最终实现更新 `docs/scene/游戏层场景使用说明.md` 的 Robot Sync 摄像机使用说明和验收流程。（验证：docs/scene/游戏层场景使用说明.md:692 记录默认 Overview，:695 记录 `C` 键，:698 记录 FollowLocal 参数，:706-708 记录验收口径）
- [x] 如新增环境变量、按键或启动方式，同步更新 `docs/bevy-getting-started.md`。（验证：docs/bevy-getting-started.md:461 补充 Robot Sync `C` 键 Overview/FollowLocal 切换说明）
- [x] 阶段全部完成后，将本 checklist 从 `summary/` 转移并归档到 `docs/scene/checklists/`。（验证：准备将 `summary/摄像机管理系统第一阶段_checklist.md` 归档为 `docs/scene/checklists/摄像机管理系统第一阶段_checklist.md`）
- [x] 归档前确认 checklist 中每个已完成阶段都有开始时间、结束时间、开发总结和验证记录。（验证：阶段 1-6 均已填写开始时间、结束时间、开发总结和验证记录）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-06-26 14:29:27 +08:00
- 结束时间：2026-06-26 14:29:27 +08:00
- 验收总结：第一阶段摄像机管理系统完成固定/跟随视角、manifest follow/animation 配置、SceneCameraTarget 目标解析、session 隔离、Transform tween、Robot Sync Overview/FollowLocal 接入和文档同步；FOV tween、后处理栈、复杂 timeline、摄像机剪辑资源格式、多相机 profile 和正式 SceneCameraCommand 留作后续阶段。

- [x] 固定视角、跟随视角、配置动画或过渡均已在代码中实现并有测试覆盖。（验证：project/src/framework/scene/camera.rs 实现 `FollowTarget`、`SceneCameraTarget`、`update_scene_cameras` 和 Transform tween；`cargo test scene_camera --lib` 10 passed，`cargo test scene --lib` 120 passed）
- [x] Robot Sync 玩法中可以实际验证默认总览、跟随本地玩家和固定/跟随过渡。（验证：受控 phone 截图 `%TEMP%/mybevy-camera-realhwnd-check-20260626-140302/hidden-overview.png`、`hidden-follow.png`、`hidden-back-overview.png` 显示 `C` 键切换和回退）
- [x] Robot Sync authority、replay、network、MyServer、checksum 和 HUD snapshot 语义未被摄像机系统改变。（验证：改动未触碰 replay/checksum/HUD snapshot 数据结构；`cargo test robot_sync --lib` 88 passed，其中 payload/checksum/HUD 相关测试通过，`robot_sync_camera_toggle_does_not_emit_authority_input` 通过）
- [x] 全局 UI 相机仍在场景相机之上，HUD、Loading、弹窗等 UI 不被 3D 场景相机遮挡。（验证：`game::plugin::tests::global_ui_camera_order_is_above_scene_cameras` 随 `cargo test scene_camera --lib` 通过；phone/tablet 截图显示 HUD 在 3D 场景上层）
- [x] 场景退出后 scene camera、camera target、Robot Sync visual 和相关运行时状态不残留。（验证：project/src/framework/scene/camera.rs 场景相机清理测试通过，project/src/game/features/robot_sync/visual.rs camera target 清理测试随 `cargo test robot_sync --lib` 通过；direct-start 点击“大厅”后进程清理为 0）
- [x] 镜头特效与渲染/VFX 特效边界已记录，第一阶段未引入完整后处理栈。（验证：docs/scene/摄像机管理系统设计.md 和 docs/scene/场景框架层功能说明.md 记录 transform/projection 与渲染/VFX 边界及未完成后处理栈）
- [x] `cargo fmt --check`、相关 `cargo test`、`cargo check` 和 `git diff --check` 通过。（验证：阶段 5 记录 `cargo fmt --check`、`cargo test robot_sync --lib`、`cargo test scene_camera --lib`、`cargo test scene --lib`、`cargo check`、`git diff --check` 均通过）
- [x] 相关 docs 已与实现同步，checklist 已按仓库约定归档到 `docs/scene/checklists/`。（验证：docs/bevy-getting-started.md 与 docs/scene/ 三份文档已同步，准备归档到 docs/scene/checklists/）


