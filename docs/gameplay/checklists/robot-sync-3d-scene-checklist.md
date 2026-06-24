# Robot Sync 3D Scene Checklist

## 目标

把当前 Robot Sync 的 2D/UI 场景表现升级为真实 3D 场景：有地板、灯光、3D 摄像机，机器人由点/方块替换为真实人物模型，同时保留现有单机、LAN、MyServer 的 authority 帧同步逻辑。

## 基础原则

- [x] 本次修改只覆盖 MyBevy 客户端表现层，包括场景相机、地板、灯光、机器人可视实体、坐标映射、朝向和后续动画。
- [x] 不修改 server 端、不修改 MyServer room policy、不修改 `project/src/framework/network/` 网络层。
- [x] 不修改 authority 协议、`robot_move` payload 和现有 `RobotSyncReplayState` 同步语义，第一阶段只替换表现层。
- [x] 先保证静态 3D 模型位置同步正确，再做朝向、动画、碰撞和相机跟随。
- [x] 开发过程中只做必要的客户端编译、单元测试和代码检查；单机、LAN、MyServer 由全部开发完成后统一手动验收。
- [x] 新增首包资源继续放在 `project/assets/`，大模型/贴图资源走 Git LFS。

## 阶段 1：3D 场景骨架

- 阶段开始时间：2026-06-23 15:23:52 +08:00
- 阶段结束时间：2026-06-23 15:45:38 +08:00
- 阶段开发总结：已将 Robot Sync arena manifest 切换为 `fixed3d` 透视相机；静态 arena 内容改为 3D primitive 场地底座、网格、边界、贴地 spawn marker 和 `DirectionalLight`，并保留 `SceneOwned(session_id)` 与 runtime root 挂载清理边界。同步更新了相关场景/玩法文档。主审验证通过：`cargo fmt --check`、`cargo test robot_sync_arena --lib`、`cargo check`。按总验收约定，本阶段未单独执行 LAN/MyServer 手动验收。

- [x] 将 `project/assets/scenes/robot_sync_arena/scene.ron` 的相机从 `gameplay2d` 改为 3D 相机。
- [x] 使用 `fixed3d` 或 `gameplay3d` 相机模式，并配置合适的 `position`、`rotation`、`perspective3d` 投影。
- [x] 确认 UI HUD 仍由全局 UI 摄像机显示，不被 3D 摄像机遮挡。
- [x] 在 `project/src/game/scenes/robot_sync_arena.rs` 中生成 3D 场景内容，而不是继续依赖 2D `Sprite` 地面。
- [x] 添加基础光照，至少包含一个 `DirectionalLight`。
- [x] 确认 scene root、layer root、runtime root 下的 3D 实体 `GlobalTransform` 正常传播。

## 阶段 2：地板和出生点

- 阶段开始时间：2026-06-23 15:51:58 +08:00
- 阶段结束时间：2026-06-23 16:36:49 +08:00
- 阶段开发总结：已复用 `floor_tile_large.gltf`，通过 `GltfAssetLabel::Scene(0)` + `BevySceneRoot` 生成 9 个放大地板拼片，覆盖 `-250..250` 逻辑范围并实际扩展到 `-300..300`；primitive base 下沉到地板下方，网格、边界和 Torus 出生点标记抬离地板，避免遮挡和 Z fighting。A/B 出生点继续保持左右两侧。同步更新了入门、场景和移动同步文档。主审验证通过：`cargo fmt --check`、`cargo test robot_sync_arena --lib`、`cargo check`、`git diff --check`。

- [x] 复用现有地板资源：`project/assets/models/scenes/kaykit_dungeon_remastered/floor_tile_large.gltf`。
- [x] 参考 `project/src/game/scenes/sample_dungeon_room.rs` 的 `BevySceneRoot + GltfAssetLabel::Scene(0)` 加载方式。
- [x] 生成一片足够容纳 A/B 出生点和移动范围的地板。
- [x] 确认现有 A/B 出生点仍分别位于左右两侧。
- [x] 将出生点黄色圆环从 2D annulus 升级为贴地 3D 标记，或临时保留为可见的 3D 平面标记。
- [x] 确认出生点不会被地板遮挡或 Z fighting。

## 阶段 3：机器人模型替换

- 阶段开始时间：2026-06-23 16:39:20 +08:00
- 阶段结束时间：2026-06-23 17:22:22 +08:00
- 阶段开发总结：已将 `robot_sync/visual.rs` 的机器人主体从 2D `Sprite` 替换为 `BevySceneRoot` GLB 人物模型；本机玩家使用 `Knight.glb`，远端玩家按 `color_index` 在 `Rogue.glb` / `Mage.glb` 间选择，并在现有 visual 的本机状态或 color index 改变时同步更新 scene handle。保留 `RobotSyncRobotVisual` 作为可视 root 组件，一个玩家对应一个 root entity，离开、切场景和 stale session 清理测试通过。本阶段为让模型落在 3D 地板上，已将渲染坐标映射到 XZ 平面；阶段 4 继续统一比例、HUD 说明和坐标映射常量。主审验证通过：`cargo fmt --check`、`cargo test robot_sync --lib`、`cargo check`、`git diff --check`。

- [x] 将 `project/src/game/features/robot_sync/visual.rs` 中的 `Sprite` 机器人替换为 GLB 人物模型实体。
- [x] 复用现有角色资源，例如：
  - [x] `project/assets/models/characters/kaykit_adventurers/Knight.glb`
  - [x] `project/assets/models/characters/kaykit_adventurers/Rogue.glb`
  - [x] `project/assets/models/characters/kaykit_adventurers/Mage.glb`
- [x] 本机玩家和远端玩家使用可区分的模型，或使用脚底绿色/红色标记区分。
- [x] 保留 `RobotSyncRobotVisual` 组件，继续用它记录 `player_id`、`session_id`、`is_local_player`。
- [x] 继续保证一个玩家只生成一个可视实体。
- [x] 玩家离开、切场景、session 变化时，模型实体能正确清理。

## 阶段 4：坐标映射

- 阶段开始时间：2026-06-23 17:26:18 +08:00
- 阶段结束时间：2026-06-23 18:17:22 +08:00
- 阶段开发总结：已新增 Robot Sync 坐标映射模块，明确 fixed -> sync -> world3d 的表现层换算：`sync.x -> world3d.x`，`sync.y -> world3d.z`，统一比例为 `0.1 world3d units / sync unit`，人物脚底高度固定为 `world3d.y = 0.05`。机器人 GLB visual、arena base、grid、boundary、spawn marker 和地板覆盖都改为同一坐标体系，HUD 同时显示 fixed/sync/world3d，telemetry 与 checksum 仍只使用 fixed 坐标。同步更新了入门、场景和移动同步文档。主审验证通过：`cargo fmt --check`、`cargo test robot_sync --lib`、`cargo check`、`git diff --check`。按总验收约定，本阶段未单独执行 LAN/MyServer 手动验收。

- [x] 明确同步坐标到 3D 坐标的映射规则：`sync.x -> world.x`，`sync.y -> world.z`，`world.y -> 高度`。
- [x] 增加统一比例常量，避免当前 `-250..250` 同步范围对 KayKit 模型过大。
- [x] 确认人物脚底落在地板上，不悬空、不陷入地板。
- [x] 更新 HUD 中的坐标显示说明，必要时同时显示 sync 坐标和 3D world 坐标。
- [x] 保留单元测试覆盖固定位置到 `Transform.translation` 的映射。

## 阶段 5：朝向

- 阶段开始时间：2026-06-23 18:20:25 +08:00
- 阶段结束时间：2026-06-23 18:35:59 +08:00
- 阶段开发总结：已为 Robot Sync GLB visual root 增加表现层 yaw：使用 replay 层已接受的 `dir_x` / `dir_y` 计算 XZ 平面朝向，`dir_x -> world3d.x`、`dir_y -> world3d.z`，KayKit 默认 `+Z` 方向对应零 yaw。零方向不会覆盖现有 `Transform.rotation`，因此移动后停止会保留上一有效朝向；新生成且静止的实体保持稳定默认朝向。该 rotation 不写回 replay state，不修改 authority payload、网络层、MyServer policy 或 checksum 语义。同步更新了移动同步设计文档。主审验证通过：`cargo fmt --check`、`cargo test robot_sync --lib`、`cargo check`、`git diff --check`。按总验收约定，本阶段未单独执行 LAN/MyServer 手动验收。

- [x] 根据 `RobotState.dir_x` / `RobotState.dir_y` 计算模型朝向。
- [x] 将二维方向映射为 3D XZ 平面的 yaw。
- [x] 静止时保留上一次有效朝向，避免模型抖动或瞬间转回默认方向。
- [x] 增加测试覆盖不同方向输入对应的旋转结果。

## 阶段 6：动画

- 阶段开始时间：2026-06-23 18:37:49 +08:00
- 阶段结束时间：2026-06-23 19:05:32 +08:00
- 阶段开发总结：已在 Robot Sync GLB visual 上接入表现层 Idle / Run 两态动画。动画状态只从 replay 层已接受的 `speed + dir_x/dir_y` 派生：有速度且方向非零播放 Run，否则播放 Idle；Idle 使用 `Rig_Medium_General.glb#Animation6` (`Idle_A`)，Run 使用 `Rig_Medium_MovementBasic.glb#Animation5` (`Running_A`)。实现通过 `AnimationGraph::from_clips`、`AnimationGraphHandle` 和 `AnimationTransitions` 绑定到 SceneRoot 加载出的子 `AnimationPlayer`，并沿父链关联回 `RobotSyncRobotVisual`，不会改变 player visual cleanup/session 语义，也不写回 replay state、authority payload、网络层、MyServer policy 或 checksum。同步更新了移动同步和场景使用文档。主审验证通过：`cargo fmt --check`、`cargo test robot_sync --lib`、`cargo check`、`git diff --check`。按总验收约定，本阶段未单独执行 LAN/MyServer 手动验收。

- [x] 第一版先不接动画，只验收静态模型移动。
- [x] 静态模型稳定后，再接入现有动画资源：`project/assets/models/animations/kaykit_adventurers/Rig_Medium_MovementBasic.glb`。
- [x] 为人物增加 Idle / Run 两种基础状态。
- [x] 根据输入方向或速度切换 Idle / Run。
- [x] 确认本机和远端玩家动画状态在 LAN / MyServer 下表现一致。

## 阶段 7：相机体验

- 阶段开始时间：2026-06-23 19:07:34 +08:00
- 阶段结束时间：2026-06-23 19:26:33 +08:00
- 阶段开发总结：已将 Robot Sync arena 固定俯视 3D 相机从旧同步坐标尺度调整到 Stage 4 后的 world3d 尺度：相机位置改为 `(0.0, 110.0, 136.0)`，保持 `-38.9` 度俯视和 `fov_y=0.78`，far 缩到 `300.0`。新增 frustum 单元测试，按 phone portrait 宽高比验证固定相机覆盖 `-25..25` world3d arena、边界墙高度和左右出生点。文档说明本机玩家跟随相机属于后续，且必须保留远端玩家可见性。本阶段未实现跟随相机，也未修改 authority/replay/network/MyServer 语义。主审验证通过：`cargo fmt --check`、`cargo test robot_sync_arena --lib`、`cargo check`、`git diff --check`。按总验收约定，本阶段未单独执行 GUI 窗口 profile、LAN 或 MyServer 手动验收。

- [x] 第一版使用固定俯视 3D 相机，便于同时观察 A/B 两个玩家。
- [x] 后续再增加本机玩家跟随相机。
- [x] 跟随相机需要保留远端玩家可见性，避免联调时看不到 B 端。
- [x] 检查不同窗口 profile 下模型、地板、HUD 不重叠、不出画。

## 阶段 8：最终手动验收

阶段开始时间：2026-06-23 19:29:13 +08:00
阶段结束时间：2026-06-23 19:40:35 +08:00
阶段验收总结：Robot Sync 3D 场景最终手动验收已完成。用户在可交互窗口环境确认：单机模式下单 client 移动人物模型时位置和 HUD 数据一致；LAN 双客户端下 A/B 两端都能看到本机和对方人物模型，A 端移动可同步到 B 端，B 端移动可同步到 A 端；MyServer 双客户端通过中心服连接后表现与 LAN 一致，并确认当前环境使用 `17002` 端口；断开、重连、切场景后未残留旧模型实体。开发环境曾尝试以 `MYBEVY_START_SCENE=arena.robot_sync` 启动非交互式 GUI 验收，默认 Vulkan 与 `WGPU_BACKEND=dx12` 均能初始化窗口和 GPU adapter，但停留于渲染初始化/首次 shader 输出阶段，故最终 GUI 观察以用户可交互环境验收结论为准。

以下项目不要求每个开发阶段都执行，由全部客户端表现层开发完成后统一手动验收。

- [x] 单机模式：单 client 移动人物模型，位置和 HUD 数据一致。
- [x] LAN 模式：A/B 两端都能看到本机和对方人物模型。
- [x] LAN 模式：A 端移动，B 端看到 A 的模型移动；B 端移动，A 端看到 B 的模型移动。
- [x] MyServer 模式：两个 client 通过中心服连接后，表现与 LAN 一致。
- [x] MyServer 模式：确认使用正确端口，例如当前环境的 `17002`。
- [x] 断开、重连、切场景后，不残留旧模型实体。

## 阶段 9：测试和提交

- 阶段开始时间：2026-06-23 19:41:15 +08:00
- 阶段结束时间：2026-06-23 19:43:41 +08:00
- 阶段开发总结：Robot Sync 3D 表现层开发已按阶段拆分提交完成，相关单元测试覆盖 3D 坐标映射、模型 visual、朝向、动画状态/绑定、arena 静态内容、固定相机 frustum、HUD 和 authority replay 边界。最终代码验证通过：`cargo fmt --check`、`cargo test robot_sync --lib`、`cargo check`、`git diff --check`；最终手动验收已在 Stage 8 记录完成。

- [x] 更新或新增 `robot_sync` 相关单元测试。
- [x] 客户端代码完成后至少运行：
  - [x] `cargo fmt --check`
  - [x] `cargo test robot_sync --lib`
  - [x] `cargo check`
- [x] 如果修改了场景使用方式或启动方式，同步更新 `docs/bevy-getting-started.md` 或相关场景文档。
- [x] 提交时拆分清楚：场景 3D 化、人物模型替换、动画接入不要混成一个过大的提交。

## 建议提交拆分

- [x] `feat(robot-sync): 将同步场景切换为 3D 地板`
- [x] `feat(robot-sync): 使用人物模型显示同步玩家`
- [x] `feat(robot-sync): 根据移动方向更新人物朝向`
- [x] `feat(robot-sync): 接入人物移动动画`
- [x] `docs(robot-sync): 补充 3D 同步场景测试流程`

