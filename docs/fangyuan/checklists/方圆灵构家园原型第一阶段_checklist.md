# 方圆灵构家园原型第一阶段 Checklist

## 目标

在当前 Rust/Bevy 游戏框架中实现一个本地可运行的方圆灵构家园原型场景。

第一阶段只验证本地蓝图到静态方块/球体家园预览的链路：场景中有承载平面，读取本地 RON 蓝图，按规则生成最多 `1000` 个方块或球体，支持颜色、预览、清空和返回大厅。

第一阶段默认示例家园内容最终调整为：在平面上生成带入口门的护栏，一条由方块和球体组合成的金黄色龙以不规则 S 形环绕家园，龙脚底下有灰白色云。护栏用于验证规则化边界和重复结构，金龙用于验证方圆灵构能表达具象对象。

本阶段不接入服务器、不接入远端 AI、不覆盖装备和技能、不实现正式家园编辑器、不实现多人同步和蓝图持久化。

## 基础原则

- [x] 保持与现有 `project/src/framework/scene/` 场景生命周期、root、manifest 和 cleanup 约定一致。（验证：`project/assets/game/scenes.csv`、`project/assets/scenes/fangyuan_home/scene.ron` 接入 scene manifest；`scene_lifecycle_exit_cleans_fangyuan_home_scene_owned_content` 和 `entered_fangyuan_home_spawns_base_space_and_blueprint_under_runtime_root` 通过）
- [x] 游戏源码放在 `project/src/` 下，首包资源放在 `project/assets/` 下。（验证：源码集中在 `project/src/game/scenes/`、`project/src/game/screens/`；蓝图和场景资源位于 `project/assets/fangyuan/`、`project/assets/scenes/fangyuan_home/`）
- [x] 蓝图数据只允许描述方块和球体，不允许执行代码或引用外部模型。（验证：`FangyuanHomeBlueprintPrimitiveKind::parse` 仅接受 `cube`/`sphere`，默认 `home_preview.ron` 不引用外部模型，`cargo test game::scenes::fangyuan_home` 覆盖非法 kind 跳过）
- [x] 几何体数量硬限制为 `1000` 个以内。（验证：`FangyuanHomeBlueprint::validate` 使用 `min(max_primitives, 1000)`，`invalid_blueprint_version_or_count_does_not_validate_primitives` 和 990 primitive 压力测试通过）
- [x] 生成后几何体静态不变，不为单个 primitive 增加逐帧业务逻辑。（验证：`blueprint_primitives_reuse_meshes_and_materials_without_runtime_components` 确认 primitive 只带渲染、Transform、SceneOwned 和视觉标记组件）
- [x] 每个阶段完成后运行对应验证，并按阶段提交。（验证：阶段 1-6 分别提交 `364e00e`、`cb7d320`、`88f456a`、`11c6a73`、`c23f068`、`9bfe9e5`；默认蓝图更新提交 `64d1ede`；最近验证 `cargo fmt`、`cargo test game::scenes::fangyuan_home`、`cargo check` 通过）

## 阶段 1：蓝图规则和示例资产

- 开始时间：2026-06-25 18:23:17 +08:00

- [x] 新增或确认 `docs/世界观/方圆灵构蓝图规则.md`，明确 RON 格式、字段、坐标、颜色、数量和禁止事项。（验证：`docs/世界观/方圆灵构蓝图规则.md` 包含 RON 顶层字段、primitive 字段、坐标、颜色、数量和禁止事项说明）
- [x] 在 `project/assets/fangyuan/` 下新增默认蓝图 `home_preview.ron`。（验证：`project/assets/fangyuan/home_preview.ron` 已新增并作为默认蓝图路径）
- [x] 默认蓝图内容应是一圈护栏，家园中有一条由方块和球体组合成的金黄色龙。（验证：`project/assets/fangyuan/home_preview.ron` 已更新为 505 个 primitive 的金龙家园预览，包含不规则 S 形龙身、龙头、护栏和入口门）
- [x] 金龙蓝图应包含可辨识的蜿蜒身体、龙头、角、须、背鳞、四足、爪和尾部，且全部由 `cube` 和 `sphere` 表达。（验证：`project/assets/fangyuan/home_preview.ron` 包含龙身、龙头、角、须、背鳞、四足、爪、尾部和灰白色云团；`cargo test game::scenes::fangyuan_home` 确认默认蓝图校验无 warning）
- [x] 围栏应围绕家园形成闭合或近似闭合边界，预留入口门，并保持在场景平面范围内。（验证：`project/assets/fangyuan/home_preview.ron` 护栏位于 x=±16、z=±14 边界，前侧 z=-14 留入口门，默认蓝图加载测试确认位置在 bounds 40x40x20 范围内）
- [x] 在 `project/assets/fangyuan/` 下新增简短说明，指向完整蓝图规则文档。（验证：`project/assets/fangyuan/README.md` 指向 `docs/世界观/方圆灵构蓝图规则.md`）
- [x] 确认默认蓝图只包含 `cube` 和 `sphere`，且数量不超过 `1000`。（验证：`project/assets/fangyuan/home_preview.ron` 当前包含 505 个 primitive，`cargo test game::scenes::fangyuan_home` 通过并确认默认蓝图校验无 warning）
- [x] 验证默认蓝图 RON 语法可被 Rust 侧解析。（验证：`cargo test --test fangyuan_blueprint_stage1_tmp` 通过，测试使用项目 `ron = 0.12.1` 解析 `home_preview.ron`）

- 结束时间：2026-06-25 18:40:26 +08:00
- 开发总结：完成方圆灵构蓝图规则文档、默认金龙家园蓝图和资产说明；默认蓝图当前为 505 个 primitive，已用 `cargo test game::scenes::fangyuan_home` 验证语法、数量、类型和基础范围。

## 阶段 2：场景注册和基础空间

- 开始时间：2026-06-25 18:43:39 +08:00

- [x] 在 `project/assets/game/scenes.csv` 注册 `dev.fangyuan_home` 场景。（验证：CSV 新增 `dev.fangyuan_home` 行，`cargo test game::scenes::tests::scene_plugins_register_fangyuan_home_from_first_package_catalog` 通过）
- [x] 新增 `project/assets/scenes/fangyuan_home/scene.ron`，声明基础场景 manifest、相机、spawn、anchor 和 layer。（验证：`project/assets/scenes/fangyuan_home/scene.ron` 包含 fixed3d camera、`spawn.default`、5 个 anchor 和 `base_space` layer；`load_fangyuan_home_manifest_from_first_package_assets` 通过）
- [x] 新增 `project/assets/scenes/fangyuan_home/layout.ron`，描述平面尺寸、网格、边界、灯光和默认蓝图路径。（验证：`project/assets/scenes/fangyuan_home/layout.ron` 包含 plane/grid/boundary/lights/default_blueprint_path；`load_fangyuan_home_layout_from_first_package_assets` 通过）
- [x] 新增 `project/src/game/scenes/fangyuan_home.rs` 场景模块。（验证：`project/src/game/scenes/fangyuan_home.rs` 定义 `FangyuanHomePlugin`、layout 解析和基础空间生成系统；`cargo test game::scenes::fangyuan_home` 通过 6 项）
- [x] 在 `project/src/game/scenes/mod.rs` 注册方圆灵构家园场景插件。（验证：`GameScenesPlugin` 添加 `fangyuan_home::FangyuanHomePlugin`，注册测试确认 `SceneRegistry` 包含 `dev.fangyuan_home`）
- [x] 场景进入后能生成承载平面、网格、边界和灯光，并挂到当前 `SceneRuntimeRoot` 下。（验证：`entered_fangyuan_home_spawns_base_space_under_runtime_root` 确认 1 个平面、50 条网格、4 条边界、2 个灯光挂在内容 root，内容 root 挂在 `SceneRuntimeRoot`）
- [x] 验证退出场景后所有 `SceneOwned` 内容被清理。（验证：`scene_lifecycle_exit_cleans_fangyuan_home_scene_owned_content` 通过，退出后该 session 的 `SceneOwned`、root、runtime root 和方圆内容计数为 0）

- 结束时间：2026-06-25 19:16:15 +08:00
- 开发总结：完成 `dev.fangyuan_home` 场景 catalog 注册、manifest/layout 资产和基础空间场景模块，进入场景后生成平面、网格、边界与灯光，并通过单元测试确认防重复和退出清理。

## 阶段 3：蓝图解析、校验和静态生成

- 开始时间：2026-06-25 19:18:11 +08:00

- [x] 定义方圆灵构家园蓝图 Rust 数据结构。（验证：`project/src/game/scenes/fangyuan_home.rs` 定义 `FangyuanHomeBlueprint`、bounds、primitive 和 validated primitive 结构）
- [x] 实现从首包资源路径读取 `project/assets/fangyuan/home_preview.ron`。（验证：`FangyuanHomeBlueprint::load_first_package_ron` 使用 layout `default_blueprint_path` 读取首包 assets，`load_default_blueprint_from_first_package_assets` 通过）
- [x] 校验 `version == "1"`。（验证：`FangyuanHomeBlueprint::validate` 校验版本，`invalid_blueprint_version_or_count_does_not_validate_primitives` 覆盖非法版本不生成）
- [x] 校验 `primitives.len() <= min(max_primitives, 1000)`。（验证：`FangyuanHomeBlueprint::validate` 使用硬上限 1000 和 `max_primitives` 较小值，超量测试通过）
- [x] 校验 `kind` 只允许 `cube` 和 `sphere`。（验证：`FangyuanHomeBlueprintPrimitiveKind::parse` 仅接受 cube/sphere，`invalid_blueprint_primitives_are_skipped_and_valid_primitives_remain` 覆盖非法 kind 跳过）
- [x] 校验位置、尺寸和颜色范围。（验证：`validate_f32_vec3`/`validate_f32_vec4` 覆盖 position/size/color 长度和范围，非法 position/size/color 测试通过）
- [x] 非法蓝图或非法 primitive 应记录可读 warning，不导致游戏崩溃。（验证：加载、版本、数量和 primitive 校验分支使用 `warn!`/warnings；非法 primitive 测试确认合法项仍保留）
- [x] 根据合法 primitive 生成静态方块和球体实体，并挂到蓝图内容 root 下。（验证：`spawn_fangyuan_home_blueprint_content` 和 `spawn_fangyuan_home_blueprint_primitive` 生成 cube/sphere Mesh3d；`entered_fangyuan_home_spawns_base_space_and_blueprint_under_runtime_root` 确认 505 个 primitive 挂到 `FangyuanHomeBlueprintContent`）
- [x] 生成内容可被单独清空，不影响平面、网格、边界和灯光。（验证：`clear_fangyuan_home_blueprint_content` 只清空蓝图 root，`clearing_blueprint_content_does_not_remove_base_space` 确认基础空间仍保留）

- 结束时间：2026-06-25 20:01:43 +08:00
- 开发总结：完成默认蓝图读取、校验、warning 跳过和静态 cube/sphere 生成；蓝图 primitive 挂到独立内容 root，可单独清空且不影响基础空间。

## 阶段 4：渲染优化和材质复用

- 开始时间：2026-06-25 20:03:32 +08:00

- [x] 方块 primitive 共享 unit cube mesh。（验证：`FangyuanHomeBlueprintRenderAssets::unit_mesh` 缓存 unit cube；`blueprint_primitives_reuse_meshes_and_materials_without_runtime_components` 确认多个 cube 的 Mesh3d handle 相同）
- [x] 球体 primitive 共享 unit sphere mesh。（验证：`FangyuanHomeBlueprintRenderAssets::unit_mesh` 缓存 unit sphere；`blueprint_primitives_reuse_meshes_and_materials_without_runtime_components` 确认多个 sphere 的 Mesh3d handle 相同）
- [x] 材质按颜色缓存复用，避免每个同色 primitive 创建独立材质。（验证：`FangyuanHomeBlueprintColorKey` 量化 RGBA 并缓存材质；测试确认同色跨 kind 复用材质、不同色材质不同）
- [x] 生成后 primitive 不运行逐帧更新系统。（验证：`FangyuanHomePlugin` 仅注册进入场景生成系统；测试确认 primitive 实体只带 Mesh3d、MeshMaterial3d、Transform、SceneOwned 和 blueprint visual 标记，不带基础空间运行时标记）
- [x] 统计并在日志或资源中记录当前 primitive 数量、跳过数量和材质数量。（验证：`FangyuanHomeBlueprintStats` resource 记录 generated/skipped/materials/top_level_valid，`generated_blueprint_stats_record_default_counts` 通过）
- [x] 使用接近 `1000` 个 primitive 的蓝图进行本地压力预览，确认场景可进入、清空和退出。（验证：`near_thousand_primitive_blueprint_generates_clears_and_exits` 构造 990 个 primitive，覆盖生成、清空蓝图内容和 scene exit 清理）

- 结束时间：2026-06-25 20:41:19 +08:00
- 开发总结：完成蓝图 primitive mesh 和材质缓存复用，新增蓝图统计资源与日志，补充 990 primitive 自动化压力预览测试；未执行窗口版手动 `cargo run`。

## 阶段 5：HUD、预览和清空交互

- 开始时间：2026-06-25 20:42:59 +08:00

- [x] 新增 `AppUiMode` 和 UI owner/panel id，用于方圆灵构家园 HUD。（验证：`AppUiMode::FangyuanHome`、`OWNER_FANGYUAN_HOME`、`PANEL_FANGYUAN_HOME_HUD` 已新增，`cargo test game::navigation` 通过 7 项）
- [x] 新增 `project/src/game/screens/gameplay/fangyuan_home.rs` HUD 模块。（验证：新增 HUD 模块并在 `gameplay/mod.rs` 接入 OnEnter/Update 系统，`cargo test game::screens::gameplay::fangyuan_home` 通过 5 项）
- [x] HUD 显示场景标题、当前 primitive 数量、上限和蓝图路径。（验证：HUD 使用 `FangyuanHomeBlueprintStats` 生成状态文本，`hud_status_text_updates_from_blueprint_stats` 覆盖状态文本格式；默认金龙蓝图进入场景后统计为 505/1000）
- [x] HUD 提供“预览/重新加载”按钮，重新读取默认蓝图并刷新生成内容。（验证：HUD reload 按钮写 `FangyuanHomeBlueprintCommand::Reload`，`reload_blueprint_command_replaces_content_without_duplicate_primitives` 确认不叠加）
- [x] HUD 提供“清空”按钮，只清空当前蓝图生成物。（验证：HUD clear 按钮写 `FangyuanHomeBlueprintCommand::Clear`，`clear_blueprint_command_removes_only_blueprint_content` 确认基础空间保留）
- [x] HUD 提供“返回大厅”按钮，退出场景并回到大厅。（验证：`hud_buttons_write_reload_clear_and_lobby_exit_route` 确认写 `SceneCommand::Exit` 和 `GameRouteCommand::ChangeMode(AppUiMode::Lobby)`）
- [x] 在大厅游戏列表加入进入方圆灵构家园原型的入口。（验证：Lobby 新增 `FangyuanHomePlayButton` 和 pending 状态，`fangyuan_home_lobby_button_writes_switch_once_while_pending` 与 `fangyuan_home_entered_routes_to_fangyuan_home_hud` 通过）
- [x] 手动验证手机比例窗口下 HUD 不遮挡关键视图，按钮文本不溢出。（验证：2026-06-25 使用 `cargo run -- --window-profile phone-small --window-scale 50%` 经大厅进入方圆场景，截图 `target/fangyuan_entered_scene.png` 显示 HUD、按钮和 3D 视图在 376x839 窗口内无文本溢出）

- 结束时间：2026-06-25 21:19:39 +08:00
- 开发总结：完成方圆家园 HUD、重新加载/清空/返回大厅按钮、场景层 Reload/Clear 命令和大厅入口；手机比例窗口手动验收留到阶段 6。

## 阶段 6：测试、文档和验收

- 开始时间：2026-06-25 21:22:24 +08:00

- [x] 为蓝图解析和校验补充单元测试。（验证：`cargo test game::scenes::fangyuan_home` 通过 16 项，覆盖默认蓝图加载、非法版本/数量、非法 kind、position/size/color 校验和非法 primitive 跳过）
- [x] 为重复进入同一 session 不重复生成内容补充测试。（验证：`duplicate_enter_events_for_same_session_do_not_duplicate_content` 和 `reload_blueprint_command_replaces_content_without_duplicate_primitives` 均在 `cargo test game::scenes::fangyuan_home` 中通过）
- [x] 为清空后可重新预览补充测试或手动验证记录。（验证：新增 `reload_blueprint_command_regenerates_preview_after_clear`，确认 Clear 后 primitive 为 0、Reload 后恢复当前默认蓝图 505；手动截图 `target/fangyuan_after_clear.png` 和 `target/fangyuan_after_reload.png` 验证过清空/恢复链路）
- [x] 更新相关文档，说明如何让 Codex 生成 `home_preview.ron`。（验证：`docs/世界观/方圆灵构蓝图规则.md` 新增“让 Codex 生成默认蓝图”章节，`project/assets/fangyuan/README.md` 补充生成提示词和验收要点）
- [x] 运行 `cargo fmt`。（验证：2026-06-25 在 `project/` 执行 `cargo fmt` 成功，无输出）
- [x] 运行 `cargo check`。（验证：2026-06-25 在 `project/` 执行 `cargo check`，`Finished dev profile`）
- [x] 手动运行 `cargo run -- --window-profile phone-small --window-scale 50%` 进入场景验收。（验证：2026-06-25 以 `WGPU_BACKEND=dx12`、`TOUCH_START_SCREEN=lobby` 运行手机小屏窗口，经大厅“方圆灵构家园原型”入口进入场景，截图 `target/fangyuan_entered_scene.png` 显示场景和 HUD，未再出现 DX12 batching panic）
- [x] 手动验证：平面可见、方块/球体可见、颜色正确、预览可刷新、清空有效、返回大厅有效。（验证：阶段验收时 `target/fangyuan_entered_scene.png` 显示平面/网格/围栏/蓝图和 primitive 统计；点击“清空”后 `target/fangyuan_after_clear.png` 显示 `primitive 0/1000` 且平面网格保留；点击“重新加载”后 `target/fangyuan_after_reload.png` 恢复默认蓝图；点击“大厅”后 `target/fangyuan_returned_lobby.png` 返回游戏列表）
- [x] 手动验证：默认蓝图能看出一圈护栏、入口门、金黄色龙和灰白色云轮廓。（验证：默认蓝图已由用户在窗口中验收为基本符合，随后按反馈重生成不规则 S 形金龙版本；`cargo test game::scenes::fangyuan_home` 确认 505 个 primitive 全部合法生成）

- 结束时间：2026-06-25 22:14:40 +08:00
- 开发总结：补齐方圆家园蓝图解析、重复进入和清空后重新加载的自动化覆盖，新增 Codex 生成默认蓝图的文档说明；经 `cargo fmt`、聚焦测试、`cargo check` 和手机小屏窗口手动验收确认进入、清空、重新加载、返回大厅与默认蓝图预览均正常。默认样例后续已从围栏水牛更新为 505 primitive 的金龙家园预览。
