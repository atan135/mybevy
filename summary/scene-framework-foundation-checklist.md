# 场景框架基础通用功能开发清单

本文基于 `docs/scene/README.md` 梳理，目标是优先推进 `project/src/framework/scene/` 中可复用的框架层能力，暂不实现具体游戏玩法、关卡逻辑、战斗逻辑、任务逻辑或 Touch Ripple 的业务改造。

## 范围边界

本轮优先实现：

- 场景框架插件、命令、事件、运行状态和生命周期状态机。
- 场景根实体、层根实体和统一清理标记。
- 场景注册表、轻量清单结构、清单校验和首包资源加载流程。
- Loading、错误、相机、spawn、anchor、trigger、debug 等框架接口。
- 为后续内容缓存、流式加载和 authority ready 流程预留数据边界。

本轮暂不实现：

- 具体玩法规则、战斗规则、任务规则、怪物、玩家控制和剧情脚本。
- 具体游戏页面、HUD、弹窗文案和业务 UI 流程。
- 具体关卡内容、美术资源制作、复杂 glTF 场景实例化细节。
- 后续下载服务、真实 CDN、真实内容发布后台。
- 完整大世界流式加载、完整 LOD、完整导航和完整物理系统。

## 设计原则

- 框架层不依赖 `project/src/game/`，只暴露通用类型、命令、事件和查询接口。
- 游戏层通过注册、命令和事件接入场景框架，不直接修改 `SceneRuntime`。
- 先支持纯 UI 场景和简单世界内容场景，再扩展资源清单、相机、spawn、trigger 和 streaming。
- 所有场景拥有的实体必须能通过 `SceneOwned` 或 `SceneRoot` 统一清理。
- 资源路径和场景 ID 的解析集中在 registry/manifest，不在玩法代码里散落。
- 错误要可诊断，不能因为清单缺字段、资源缺失或场景不存在直接 panic。

## 开发任务清单

- [x] 阶段 1：场景框架骨架（开始：2026-06-17 18:17:22 +08:00；结束：2026-06-17 18:28:18 +08:00）
  - [x] 新增 `ScenePlugin` 作为 `project/src/framework/scene/` 的统一入口，负责注册资源、消息和基础系统。
  - [x] 拆分基础模块文件，建议先落地 `plugin.rs`、`command.rs`、`event.rs`、`lifecycle.rs`、`root.rs`、`registry.rs`、`manifest.rs`、`loading.rs`、`camera.rs`、`spawn.rs`、`trigger.rs`、`debug.rs`、`prelude.rs`。
  - [x] 定义框架层 newtype：`SceneId`、`SceneSessionId`、`SceneLayerId`、`SceneAssetId`、`SceneSpawnPointId`、`SceneAnchorId`、`SceneTriggerId`、`SceneChunkId`。
  - [x] 定义 `SceneKind`、`SceneAuthorityMode`、`SceneTransition`、`SceneLoadingPolicy` 等轻量枚举，先覆盖文档中的最小语义。
  - [x] 在 `framework::prelude` 或 `framework::scene::prelude` 中导出游戏层常用但不泄漏内部实现的类型。
  - [x] 明确模块可见性，框架内部实现保持 `pub(crate)`，外部只暴露命令、事件、资源只读查询和注册 API。

- [x] 阶段 2：命令、事件和运行状态（开始：2026-06-17 18:29:47 +08:00；结束：2026-06-17 18:36:09 +08:00）
  - [x] 定义 `SceneCommand`，最小支持 `Enter`、`Exit`、`Switch`、`Preload`、`Unload`、`ReloadCurrent`。
  - [x] 定义请求结构：`SceneEnterRequest`、`SceneExitRequest`、`SceneSwitchRequest`、`ScenePreloadRequest`、`SceneUnloadRequest`、`SceneReloadRequest`。
  - [x] 定义 `SceneEvent`，覆盖 `Resolving`、`LoadProgress`、`Instantiating`、`Entered`、`ExitStarted`、`Exited`、`LayerLoaded`、`LayerUnloaded`、`Failed`。
  - [x] 定义 `SceneRuntime` 资源，包含 `active`、`pending`、`state`、`last_error` 和基础查询方法。
  - [x] 定义 `SceneSessionInfo`，记录 `scene_id`、`session_id`、`authority_mode`、`content_version`、`spawn_point`、`seed`、`entered_at`。
  - [x] 定义 `SceneLifecycleState`，先支持 `Idle`、`Resolving`、`LoadingAssets`、`Instantiating`、`Activating`、`Active`、`Deactivating`、`Unloading`、`Failed`，`Downloading` 和 `Suspending` 可预留。

- [x] 阶段 3：最小生命周期流程（开始：2026-06-17 18:36:47 +08:00；结束：2026-06-17 18:48:48 +08:00）
  - [x] 实现 `Enter` 流程：接收命令、查 registry、生成 pending session、进入 Resolving/Loading/Instantiating/Activating/Active。
  - [x] 实现纯 UI 场景进入能力：只创建 `SceneRuntime`，不创建 `SceneRoot`。
  - [x] 实现世界内容场景进入能力：按请求或 manifest 配置创建 `SceneRoot` 和默认 layer root。
  - [x] 实现 `Exit` 流程：发送退出事件、标记 Deactivating、清理 `SceneOwned` 实体、清空 active session。
  - [x] 实现 `Switch` 流程的简单版本：先 Exit 当前场景，再 Enter 目标场景。
  - [x] 处理重复命令和非法状态，例如 Idle 时 Exit、Active 时重复 Enter、切换中再次 Switch。
  - [x] 确保失败时进入 `Failed` 并写入 `SceneFailure`，而不是留下半初始化状态。

- [x] 阶段 4：场景实体组织和清理（开始：2026-06-17 18:49:52 +08:00；结束：2026-06-17 19:03:59 +08:00）
  - [x] 定义 `SceneRoot` 组件，记录 `scene_id` 和 `session_id`。
  - [x] 定义 `SceneLayerRoot` 组件，记录 `layer_id`、加载状态和是否 required。
  - [x] 定义 `SceneOwned` 组件，所有随场景销毁的运行时实体都可挂载。
  - [x] 定义 `SceneRuntimeRoot` 或等价组件，用于动态实体、临时特效和测试对象的统一父节点。
  - [x] 提供创建 root/layer root 的 helper，避免游戏层手写不一致的层级结构。
  - [x] 提供按 `session_id` 清理 `SceneOwned` 的系统，并保证子实体递归清理。
  - [x] 提供实体计数和残留检测辅助，用于后续 debug 面板和测试。

- [x] 阶段 5：场景注册表（开始：2026-06-17 19:05:08 +08:00；结束：2026-06-17 19:13:46 +08:00）
  - [x] 定义 `SceneRegistry` 资源，维护 `SceneId` 到场景定义或 manifest 路径的映射。
  - [x] 提供注册 API：注册纯 UI 场景、注册首包 manifest 场景、注册 fallback 场景。
  - [x] 支持场景定义中的基础元数据：`scene_id`、`kind`、`has_world_root`、`default_spawn`、`manifest_path`、`loading_policy`。
  - [x] 提供查询 API：按 `SceneId` 获取定义、判断场景是否存在、列出已注册场景。
  - [x] 对重复注册、空 ID、非法 ID 格式给出明确错误。
  - [x] 预留内容版本和内容缓存来源字段，但初期只实现首包路径。
  - [x] 增加 fallback scene 的注册约定，用于场景不存在或加载失败后的回退。

- [x] 阶段 6：场景清单结构和校验（开始：2026-06-17 19:14:38 +08:00；结束：2026-06-17 19:21:17 +08:00）
  - [x] 定义 `SceneManifest`，包含 `version`、`scene_id`、`kind`、`entry`、`layers`、`spawn_points`、`anchors`、`triggers`。
  - [x] 定义 `SceneManifestEntry`，包含 `default_spawn`、`camera`、`loading_policy`。
  - [x] 定义 `SceneLayerManifest` 和 `SceneAssetRef`，描述 layer、required 标记、asset ID、kind、path、label。
  - [x] 定义 `SceneSpawnPointManifest` 和 `SceneAnchorManifest`，先支持 position、rotation、tags。
  - [x] 定义 `SceneTriggerManifest`，先支持 box/circle 这类简单形状和 event 名称。
  - [x] 实现 manifest 静态校验：必填字段、ID 唯一性、默认 spawn 是否存在、路径是否跳出资源根、required layer 是否有效。
  - [x] 明确 manifest 版本支持策略，当前只接受 `"1"` 或一个固定常量。

- [x] 阶段 7：首包资源加载和 Loading 进度（开始：2026-06-17 19:22:01 +08:00；结束：2026-06-17 19:55:12 +08:00）
  - [x] 支持从 `project/assets/scenes/...` 加载 RON 场景清单。
  - [x] 通过 Bevy `AssetServer` 请求 required asset handles，先覆盖通用 handle 跟踪，不急于实例化所有资产类型。
  - [x] 定义 `SceneLoadProgress`，区分 required、optional、loaded、failed、phase 和可展示 message key。
  - [x] 在加载过程中发送 `SceneEvent::LoadProgress`。
  - [x] required 资源未完成前不进入 `Active`。
  - [x] optional 资源失败不阻断进入，但要写入 warning 或事件。
  - [x] required 资源失败时发送 `SceneEvent::Failed`，并根据策略保持旧场景或进入 fallback。

- [ ] 阶段 8：Loading 与 UI 框架接口
  - [ ] 在场景框架内定义 UI 联动边界，避免场景系统直接依赖具体游戏页面。
  - [ ] 使用现有 `UiPanelCommand` 和 `UiLoading` 打开/关闭全局 Loading，或提供可选 adapter 系统。
  - [ ] 将 `SceneLoadProgress` 转换为 Loading 标题、说明和进度。
  - [ ] `SceneEvent::Entered` 后关闭 Loading。
  - [ ] `SceneEvent::Failed` 后关闭 Loading，并只发送通用错误事件，具体 Toast/Confirm 文案留给 UI/i18n。
  - [ ] 确认全屏 Loading 打开时复用 UI 输入阻断，不新增独立输入遮罩。
  - [ ] 支持 Loading 策略：`None`、`Spinner`、`Progress`、`Blocking`、`NonBlocking` 的最小数据表达。

- [ ] 阶段 9：相机基础能力
  - [ ] 定义 `SceneCameraMode`，先支持 `UiOnly2d`、`Gameplay2d`、`Gameplay3d`、`Fixed3d`、`DebugFree`。
  - [ ] 定义 `SceneCameraRig` 或 `SceneCameraConfig`，记录 mode、transform、projection 参数和可选 target。
  - [ ] 场景进入时根据 manifest 或注册定义决定是否需要世界相机。
  - [ ] 提供创建默认 2D/3D 相机的 helper。
  - [ ] 支持复用已有相机或标记场景相机，避免重复生成无用相机。
  - [ ] 场景退出时只清理场景拥有的相机，不误删全局 UI 相机。
  - [ ] 预留相机跟随和调试自由相机接口，但不实现具体玩法跟随策略。

- [ ] 阶段 10：Spawn、Anchor 和查询接口
  - [ ] 定义运行时查询资源或索引，用于按 ID 查找 spawn point 和 anchor。
  - [ ] 场景激活时将 manifest 中的 spawn point 和 anchor 写入当前 session 的查询索引。
  - [ ] 提供 `default_spawn` 查询方法。
  - [ ] 提供按 tag 查询 spawn point/anchor 的方法。
  - [ ] 缺失 spawn point 时返回 `SceneFailure::SpawnPointMissing`，不 panic。
  - [ ] 提供将 spawn/anchor 转为 `Transform` 的 helper。
  - [ ] 预留 anchor 可视化调试接口。

- [ ] 阶段 11：通用 Trigger 框架
  - [ ] 定义 `SceneTrigger` 组件，记录 trigger ID、shape、event、enabled、session_id。
  - [ ] 定义 `SceneTriggerShape`，先支持 2D circle/box 或 3D box，按当前项目实际维度选择最小实现。
  - [ ] 定义 `SceneTriggerEvent`，包含 trigger ID、activator、action、session_id。
  - [ ] 提供从 manifest 生成 trigger 实体的 helper。
  - [ ] 提供启用/禁用 trigger 的通用命令或 API。
  - [ ] 只做空间检测和事件派发，不处理任务、剧情、传送、战斗等业务响应。
  - [ ] 为 trigger debug 绘制预留 shape 和 label 数据。

- [ ] 阶段 12：错误分类和诊断日志
  - [ ] 定义 `SceneFailure` 和 `SceneFailureKind`，覆盖 `SceneNotFound`、`ManifestLoadFailed`、`ManifestParseFailed`、`ManifestVersionUnsupported`、`RequiredAssetMissing`、`AssetLoadFailed`、`SpawnPointMissing`、`CameraSetupFailed`。
  - [ ] 每个错误至少携带 `scene_id`、`session_id`、`content_version`、`state` 和可选 asset path。
  - [ ] 为错误提供面向日志的详细描述和面向 UI 的 message key。
  - [ ] 所有 lifecycle 分支使用统一失败入口，避免散落 `warn!` 后继续运行。
  - [ ] 场景不存在、清单无效、资源失败和 root 创建失败分别产出可区分错误。
  - [ ] 失败后清理 pending session 和临时实体，避免半场景残留。
  - [ ] 保留最近错误用于 debug 面板查询。

- [ ] 阶段 13：调试与开发期能力
  - [ ] 定义 `SceneDebugConfig`，支持通过环境变量或资源开关启用。
  - [ ] 支持启动场景环境变量：`MYBEVY_START_SCENE`、`MYBEVY_START_SPAWN`。
  - [ ] 支持慢加载模拟和加载失败模拟的配置边界。
  - [ ] 提供当前场景诊断数据：scene ID、session ID、state、layer 状态、实体数量、最近错误。
  - [ ] 提供 debug overlay 所需的只读数据接口，具体 UI 面板可后续在游戏层或 UI debug 中接入。
  - [ ] 提供命令式 reload current 能力，用于开发期快速重载 manifest。
  - [ ] 确保 debug 能力默认关闭，不影响正式运行性能。

- [ ] 阶段 14：分层加载预留
  - [ ] 定义 layer 状态：`Registered`、`Loading`、`Loaded`、`Active`、`Unloading`、`Failed`。
  - [ ] 实现 `SetLayerEnabled` 命令的数据结构，初期可以只更新状态和发送事件。
  - [ ] 支持 required layer 和 optional layer 的不同失败策略。
  - [ ] 为每个 layer 建立独立 layer root，便于后续独立卸载。
  - [ ] 提供 layer 查询 API：按 ID 查状态、列出当前场景 layer。
  - [ ] 预留 layer asset handles 的引用管理。
  - [ ] 暂不实现复杂 additive glTF、碰撞层、导航层的实际业务处理。

- [ ] 阶段 15：Streaming、Partition 和 Chunk 预留
  - [ ] 定义 `SceneZoneId`、`SceneRegionId`、`SceneChunkId` 的基础类型和 manifest 数据结构。
  - [ ] 定义 chunk bounds、neighbor、required layers、optional layers、asset refs、priority、memory budget 字段。
  - [ ] 定义 `SceneStreamingState` 和 `SetStreamingEnabled` 命令。
  - [ ] 提供 chunk 查询接口：按位置查 chunk、按 chunk ID 查 bounds、列出 active/warm/cold chunk。
  - [ ] 初期只实现元数据解析和状态记录，不做真实流式加载。
  - [ ] 为后续按相机或玩家位置驱动加载半径预留系统入口。
  - [ ] 明确动态实体不随 chunk 自动卸载的框架约束。

- [ ] 阶段 16：Authority Ready 接口预留
  - [ ] 在 `SceneEnterRequest` 和 `SceneSessionInfo` 中保留 `authority_mode`、`session_id`、`content_version`、`seed`。
  - [ ] 定义场景 ready 状态，区分本地 Active 和可开始消费 gameplay frame 的 Ready。
  - [ ] 提供 `SceneEvent::Entered` 和可选 `SceneEvent::Ready` 的语义边界。
  - [ ] 预留加载完成后通知 authority 的 adapter 接口，但不在 framework 依赖 game authority 模块。
  - [ ] 预留版本不一致错误：`ContentVersionMissing`、`ContentHashMismatch`、`AuthorityRejected`。
  - [ ] 切场景时提供清理输入状态的通用事件，具体输入类型由游戏层处理。
  - [ ] 暂不实现远端协议、房间规则、玩家同步和帧回放逻辑。

- [ ] 阶段 17：测试和验收
  - [ ] 为 ID 校验、manifest 校验、registry 注册冲突写单元测试。
  - [ ] 为 lifecycle 状态迁移写最小测试或 Bevy app 测试。
  - [ ] 验证纯 UI 场景进入后没有 `SceneRoot`。
  - [ ] 验证世界内容场景进入后存在 `SceneRoot`、layer root 和 session 信息。
  - [ ] 验证退出后 `SceneOwned` 实体被清理，重复进入退出不残留。
  - [ ] 验证 manifest 缺字段、默认 spawn 缺失、required asset 失败能产出明确错误。
  - [ ] 每个实现阶段结束执行 `cargo fmt` 和 `cargo check`。

- [ ] 阶段 18：文档同步
  - [ ] 更新 `docs/scene/README.md` 中“未实现设计目标”和“已落地能力”的表述，避免把规划写成事实。
  - [ ] 如新增资源路径或 manifest 示例，检查 `docs/assets-workflow.md` 是否需要同步。
  - [ ] 如新增开发启动环境变量，更新 `docs/bevy-getting-started.md`。
  - [ ] 如拆分 `project/src/framework/scene/` 模块结构，检查 `CLAUDE.md` 的目录约定是否需要同步。
  - [ ] 在文档中明确 framework 和 game layer 的职责边界。
  - [ ] 记录首批验收命令和预期结果。
  - [ ] 避免记录尚未实现的具体游戏接入流程。
