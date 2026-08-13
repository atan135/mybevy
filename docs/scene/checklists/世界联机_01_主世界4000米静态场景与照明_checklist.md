# 主世界 4000 米静态场景与照明 Checklist

## 目标

将客户端 `world.main` 从当前 `32m x 32m` 最小场景升级为可读的日间 `4000m x 4000m` 静态世界：地面顶面保持在 `Y=0`，按 100 米间隔生成 1681 个直径 1 米的参照球，并将全部参照球合并为一个静态 Mesh、一个材质和一个场景实体。

本清单只负责客户端静态场景、渲染配置、生命周期、性能和视觉验收；不生成玩家，不实现摄像机操控、角色移动、服务端地图或联机同步。

## 依赖与边界

- 需求来源：`summary/主世界尺度角色摄像机与联机移动需求设计.md`。
- 本清单不需要登录；如视觉验收必须进入主世界，优先使用正式服登录，不启动或连接当前正在测试其他功能的本地服务端。
- 复用 `project/src/game/scenes/main_world.rs`、主世界首包 RON、`SceneOwned(session_id)` 和 scene runtime root。
- 地面与参照球不创建 collider，不加入 authority 或网络实体注册表。
- 本阶段不引入地图分块、LOD、浮动原点、复杂地形或程序化地表系统。
- 参照球合并 Mesh 的整体 AABB 覆盖全世界，本阶段接受其整体提交和有限裁剪能力。

## 基础原则

- [x] 冻结 `1 Bevy unit = 1m`、客户端世界中心为原点和 `X/Z=-2000..2000` 的尺度契约。（验证：阶段 1 RON/validate 及阶段 2 球心/AABB 测试）
- [x] 所有可调视觉参数集中在主世界 RON 或明确的场景主题配置，不散落魔法数字。（验证：地面、参照球、灯光、相机渲染参数均在主世界 RON/scene manifest）
- [x] 所有运行时静态内容归当前 scene session 所有，退出或重进不得残留旧实体或全局渲染状态。（验证：SceneOwned/ChildOf 集成测试、资产 `2 -> 0 -> 2`、全局渲染哨兵恢复测试）
- [x] 1681 个参照球只对应一个 `Mesh3d` Entity、一个 Mesh asset 和一个材质。（验证：marker collection=1，合并 Mesh/材质句柄各 1，最终 telemetry marker_entities=1）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（验证：提交 `dac648c`、`0ef7d4e`、`3feb62c`、`f88cf91`、`71814a1` 分别对应阶段 1-5）

## 阶段 1：冻结静态场景配置契约

- 开始时间：2026-08-12 10:17:11 +08:00
- 结束时间：2026-08-12 10:32:35 +08:00
- 开发总结：冻结主世界 4000 米地面与 41 x 41 参照球配置契约，统一通过 RON 解析后的业务校验拒绝非法范围、间隔和半径。
- 验证记录：`cargo test main_world --lib` 通过（91 passed, 0 failed）；`cargo fmt -- --check` 通过。

- [x] 将 `MainWorldLayout` 的地面尺寸调整为可表达 `4000 x 0.4 x 4000m`，顶面固定在 `Y=0`。（验证：`project/assets/scenes/main_world/layout.ron` 声明 `size=[4000.0, 0.4, 4000.0]`、`top_y=0.0`，`main_world.rs` 由顶面推导地面中心变换）
- [x] 用明确的 `distance_markers` 配置替换当前单个 `landmark` 语义，至少声明起止坐标、100 米间隔、0.5 米半径、颜色和质量档位。（验证：`layout.ron` 声明 `start=-2000`、`end=2000`、`spacing=100`、`radius=0.5`、颜色及 `quality=low`）
- [x] 明确配置校验：范围有限且有序、间隔大于零、半径大于零、生成轴数量为 41、总数为 1681。（验证：`MainWorldLayout::validate` 校验有限有序范围、正间隔/半径、整除、41 点/轴及 1681 总数）
- [x] 保留 `scene_id=world.main` 和首包资源加载边界，不改变现有场景注册 ID。（验证：`MAIN_WORLD_SCENE_ID` 与 `layout.ron` 仍为 `world.main`，加载路径仍为 `scenes/main_world/layout.ron`）
- [x] 增加 RON 解析和非法参照球配置拒绝测试。（验证：`main_world_layout_rejects_invalid_distance_marker_contracts` 覆盖非法范围、零/错误/极小间隔、零半径及非有限值）
- [x] 运行 `cargo test main_world --lib` 覆盖配置契约，并在 `project/` 运行 `cargo fmt -- --check`。（验证：91 passed, 0 failed；格式检查通过）

## 阶段 2：实现参照球单 Mesh 生成器

- 开始时间：2026-08-12 10:33:30 +08:00
- 结束时间：2026-08-12 10:57:37 +08:00
- 开发总结：实现 8 sectors x 6 stacks 的低模球模板与 41 x 41 单 Mesh 合并生成器，冻结单球及合并后顶点/三角形预算。
- 验证记录：`cargo test main_world::tests::distance_marker_mesh --lib` 通过（2 passed, 0 failed）；`cargo check` 与 `cargo fmt -- --check` 通过。

- [x] 选择 Bevy 0.18.1 支持的低模球拓扑，并冻结单球三角形预算。（验证：`main_world.rs` 使用 8 sectors x 6 stacks UV 球，常量冻结为单球 63 顶点、80 三角形）
- [x] 只生成一次基础球顶点、法线、UV 和索引模板，再按 `41 x 41` 坐标复制进一个 Mesh。（验证：`build_main_world_marker_template` 单次生成模板，`build_main_world_distance_marker_mesh` 复制到一个 `Mesh`）
- [x] 使用 `u32` 索引，避免合并后超过 `u16` 顶点上限。（验证：生成器写入 `Indices::U32`；测试确认总顶点 105903 大于 `u16::MAX`）
- [x] 保证每个球心位于 `Y=0.5m`，X/Z 坐标精确覆盖 `-2000..2000` 且间隔为 `100m`。（验证：测试从每个 63 顶点块恢复并逐一核验全部 1681 个球心）
- [x] 为生成结果增加纯数据测试：球数、边界坐标、顶点/索引数量、索引范围、attribute 长度和 AABB 均合法。（验证：`distance_marker_mesh_merges_all_markers_with_valid_attributes_and_bounds` 验证 105903 顶点、403440 索引、134480 三角形及 AABB `[-2000.5,0,-2000.5]..[2000.5,1,2000.5]`）
- [x] 验证生成函数不创建 ECS Entity、不创建 collider，也不依赖网络状态。（验证：生成器签名仅接收 `MainWorldDistanceMarkers` 并返回纯 `Mesh + stats`，实现无 `Commands`、collider 或 network/authority 引用）
- [x] 运行参照球 Mesh 定向测试和 `cargo check`。（验证：定向测试 2 passed, 0 failed；`cargo check` 通过）

## 阶段 3：接入主世界场景生命周期

- 开始时间：2026-08-12 10:58:37 +08:00
- 结束时间：2026-08-12 11:17:48 +08:00
- 开发总结：将合并参照球 Mesh 接入 `world.main` 的 `SceneOwned` 生命周期，补齐重复 Entered、Exit、失败和重进的实体/资产边界测试。
- 验证记录：`cargo test main_world --lib` 通过（94 passed）；主世界模块测试 7 passed；`cargo check` 与 `cargo fmt -- --check` 通过。

- [x] 将当前 `MainWorldFloatingSphere` 替换为合并参照球 Mesh 实体。（验证：`MainWorldDistanceMarkerCollection` 持有阶段 2 生成的单个 `Mesh3d`，代码中已无旧 `MainWorldFloatingSphere`）
- [x] 为地面、参照球和方向光使用当前 `MainWorldContent` 父实体及一致的 `SceneOwned(session_id)`。（验证：集成测试断言三类视觉实体的 `ChildOf` 均指向同一 content，且 `SceneOwned` session 一致）
- [x] 确保单次 Scene Entered 只实例化一份主世界内容，重复 Entered 事件不重复生成 Mesh 实体。（验证：重复 Entered 后 content/参照球集合均为 1，`Assets<Mesh>` 保持 2、材质保持 2）
- [x] 确保 Scene Exit、加载失败和重新进入会释放旧场景实体，不残留上一个 session 的内容。（验证：Exit/重进测试断言旧 session `SceneOwned=0`、新 session content/集合各 1；缺失 manifest 失败测试断言无 active/content/session-owned 残留）
- [x] 更新场景实例化日志，使对象统计反映地面、参照球集合和灯光，而不逐球输出日志。（验证：单条实例化日志输出 `marker_count`、`marker_vertices`、`marker_triangles` 及三个集合级对象）
- [x] 增加集成测试，断言当前 session 只有一个参照球 `Mesh3d` Entity，且不存在 1681 个球体实体。（验证：`entered_session_owns_one_marker_collection_without_duplicate_entities_or_assets` 断言一个集合实体、总计两个场景 Mesh 实体）
- [x] 运行 `cargo test main_world --lib` 和 `cargo check`。（验证：94 passed；`cargo check` 通过）

## 阶段 4：建立日间照明与材质基线

- 开始时间：2026-08-12 11:18:31 +08:00
- 结束时间：2026-08-12 11:42:25 +08:00
- 开发总结：建立主世界日间材质、方向光和相机级环境渲染基线；通过 session-camera 隔离避免修改或恢复全局渲染资源。
- 验证记录：`cargo test main_world --lib` 通过（95 passed, 0 failed）；`cargo check` 与 `cargo fmt -- --check` 通过。

- [x] 调整地面为非金属、高粗糙度和足够中间调明度的材质。（验证：`layout.ron` 配置地面 `[0.24,0.43,0.27]`、`metallic=0`、`perceptual_roughness=0.94`，集成测试核验实际材质）
- [x] 调整参照球颜色，使其与地面有稳定的色相和明度差；如使用 emissive，仅保持轻微识别度。（验证：参照球 `[0.12,0.62,0.96]`、`emissive_strength=0.08`；测试校验亮度差至少 0.1 且 emissive 不超过 0.1）
- [x] 为主世界配置方向光角度、颜色和日间照度，并明确是否启用玩家附近所需阴影。（验证：RON 配置角度、暖白颜色、85000 lux 和当前静态首包 `shadows_enabled=false`；集成测试核验实际灯光；近场对象接入时再按范围启用）
- [x] 增加环境补光，确保背光面和地面不接近纯黑。（验证：当前 session 相机挂载蓝天色 `AmbientLight`，亮度 180 cd/m2）
- [x] 配置与主世界匹配的 clear color、曝光和色调映射，避免依赖预览页面的灯光或摄像机。（验证：当前 session 相机使用自定义日间 clear color、EV100 8、`TonyMcMapface`；run_fast 截图验证日间层次）
- [x] 如果使用全局环境光 Resource，保存进入前状态，并在 scene session 退出、失败或切换时可靠恢复。（验证：实现未修改 `GlobalAmbientLight`/全局 `ClearColor`，全部配置挂当前 `SceneOwned` 相机；非默认全局哨兵在进入、退出、失败、重进全程不变）
- [x] 增加环境光进入/退出和重复进入的状态恢复测试。（验证：framework 集成测试断言旧 session 相机销毁、新 session 仅一个配置相机，且全局资源无污染）
- [x] 运行主世界渲染配置定向测试和 `cargo check`。（验证：95 passed, 0 failed；`cargo check` 通过）

## 阶段 5：性能、视觉与回归验收

- 开始时间：2026-08-12 11:44:50 +08:00
- 结束时间：2026-08-12 13:09:26 +08:00
- 开发总结：完成固定视口离线视觉验收、资源清理、帧时间分布 telemetry 和阴影性能优化；更新主世界场景说明。
- 验证记录：`scripts/run_fast.ps1` 固定视口 DX12 离线直入主世界通过；最终采样 warmup 2.014s、sample 5.030s、293 frames、平均 application FPS 58.25、平均帧 17.167ms、p95 21.335ms、最大 48.568ms；`cargo test main_world --lib` 95 passed；`cargo fmt -- --check`、`cargo check`、`git diff --check` 通过。

- [x] 从仓库根运行 `scripts/run_fast.ps1`，按 `2772x1280`、device scale `3.25`、window scale `50%` 验收主世界。（验证：DX12 离线直入使用 `MYBEVY_SCENE_DEBUG=on`、`MYBEVY_START_SCENE=world.main`、`MYBEVY_UI_AUDIT=1`、`SCREEN=main_world` 和固定脚本视口）
- [x] 记录 Mesh 数、材质数、参照球 Entity 数、合并 Mesh 顶点数、三角形数、生成耗时和稳定帧率。（验证：最终 telemetry：全局 Mesh=5、材质=3，参照球 Entity=1，105903 顶点、134480 三角形，生成 1.8483ms；2.014s warmup 后 5.030s sample，平均 application FPS 58.25、平均帧 17.167ms、p95 21.335ms、最大 48.568ms；指标明确不代表 GPU present FPS）
- [x] 视觉确认地面、参照球和背景层次清晰，默认画面不接近纯黑。（验证：最终截图 `C:\Users\defaultuser0.DESKTOP-1LG9IK4\AppData\Local\Temp\mybevy-main-world-final-visual.png` 显示蓝天、绿色地面、清晰地平线与可辨识参照球）
- [x] 在出生点附近确认原点参照球、相邻 100 米方向和地面尺度关系可辨认。（验证：固定观察相机截图中中心原点球及横向/纵深 100m 规则网格可读）
- [x] 验证场景反复进入/退出后实体数和全局环境光状态不累积。（验证：集成测试断言主世界资产 `2 -> 0 -> 2`、marker Entity=1，非默认全局 `ClearColor`/`GlobalAmbientLight` 在退出、失败、重进全程不变）
- [x] 验证 Preview、Lobby、家园和其他已注册 3D 场景的光照与相机没有被主世界全局状态污染。（验证：主世界仅插入 session camera 组件，不修改全局渲染 Resource；framework 集成哨兵验证全局资源保持原值）
- [x] 在 `project/` 运行 `cargo fmt`、`cargo check` 和全部相关定向测试。（验证：`cargo fmt -- --check`、`cargo check`、`cargo test main_world --lib` 通过）
- [x] 更新 `docs/scene/` 中受影响的主世界静态场景说明；如未影响上手流程，记录无需修改 `docs/bevy-getting-started.md`。（验证：`docs/scene/游戏层场景使用说明.md` 已补 4000m、单 Mesh 预算、camera-scoped 渲染、资源清理和阴影策略；本阶段未改变上手流程，`docs/bevy-getting-started.md` 无需修改）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-12 10:17:11 +08:00
- 结束时间：2026-08-12 13:09:26 +08:00
- 验收总结：主世界完成 4000m 静态地面、1681 个合并参照球、日间 session-camera 渲染基线、生命周期清理和固定视口视觉/性能验收；阴影默认关闭以控制静态首包长尾，后续近场对象按范围启用。

- [x] 主世界地面为 `4000 x 0.4 x 4000m`，顶面位于 `Y=0`。（验证：阶段 1 配置契约测试与最终布局 RON）
- [x] 参照球数量严格为 1681，直径 1 米，间隔 100 米，覆盖两个轴的 `-2000..2000`。（验证：阶段 1/2 纯数据测试逐一核验 41 x 41 球心、半径和 AABB）
- [x] 全部参照球由一个静态 Mesh、一个材质和一个 `Mesh3d` Entity 渲染。（验证：阶段 3 集成测试 marker collection=1；最终 telemetry marker_entities=1）
- [x] 参照球不包含独立 Entity、collider、authority 状态或网络同步。（验证：合并生成器和场景查询仅存在集合实体，无 collider/authority/network 注册）
- [x] 主世界默认日间画面能清楚区分地面、参照球和空间层次。（验证：固定 run_fast DX12 截图通过，蓝天/绿色地面/地平线/中心球/网格清晰可辨）
- [x] 退出、失败和重进时场景实体及全局渲染状态均正确清理或恢复。（验证：实体与资产 `2 -> 0 -> 2`，session camera 销毁，全局渲染哨兵不变）
- [x] 自动测试、`cargo fmt`、`cargo check` 和 `scripts/run_fast.ps1` 视觉验收均通过。（验证：95 项主世界定向测试、格式/编译检查及规定固定视口视觉验收通过）
