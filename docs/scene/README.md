# MyBevy 场景功能设计文档

这个目录记录 MyBevy 场景框架相关设计。当前 `README.md` 是场景相关总文档；后续如果拆分相机、场景清单、流式加载、触发器等专题文档，应从这里保持索引。

## 专题文档索引

- [场景框架层功能说明](./场景框架层功能说明.md)：说明 `project/src/framework/scene/` 当前已提供的通用 Scene 能力、生命周期、manifest、资源加载、根实体、相机、spawn/anchor、trigger、streaming、authority ready 和调试边界。
- [游戏层场景使用说明](./游戏层场景使用说明.md)：说明 game layer 如何通过场景表 CSV、framework manifest、layout RON、大厅入口和 HUD 接入具体游戏场景，重点面向 `sample.dungeon_room` 样板场景。

## 1. 文档目标

这份文档用于记录 MyBevy 的场景相关能力。当前 `project/src/framework/scene/` 已落地一批框架层基础能力，本文同时说明已实现事实、仍未实现的设计目标、模块职责、数据流、资源组织和后续落地顺序。

设计参考大型游戏的常见机制，但按当前项目规模做轻量化拆分：

- 用统一的场景生命周期管理地图、关卡、副本、房间、测试场景和玩法场景。
- 用清单驱动场景资源，兼容首包资源和后续下载资源。
- 支持异步加载、Loading 过渡、失败回退和资源卸载。
- 预留流式加载、分区、LOD、导航、碰撞、相机、光照、音频、触发器、调试和联网同步边界。
- 保持框架层可复用，具体玩法、关卡内容和业务逻辑留在游戏层。

本文中“已落地能力”描述当前代码事实；其他设计、示例和落地顺序属于后续开发约定和设计目标，不表示已经接入具体游戏内容。

## 2. 当前项目边界

当前仓库约定：

- Rust/Bevy 工程根目录是 `project/`
- 当前 Bevy 版本是 `bevy = "0.18.1"`
- 首包资源目录是 `project/assets/`
- 后续下载资源不放入 `project/assets/`
- 桌面与 Android 都通过同一套 Bevy 代码运行
- UI 框架位于 `project/src/framework/ui/`
- 网络框架位于 `project/src/framework/network/`
- authority 控制机会话位于 `project/src/game/authority/`
- 场景框架边界位于 `project/src/framework/scene/`

场景框架的职责应是：

- 管理场景生命周期。
- 管理场景资源清单解析、首包资源加载跟踪、根实体创建和卸载。
- 提供通用场景相机、根节点、层级、spawn/anchor、trigger、chunk 元数据和调试诊断能力。
- 提供与 UI Loading、网络 authority、资源清单的接口。
- 不依赖具体游戏业务模块。

场景框架不负责：

- 具体玩法规则。
- 具体关卡脚本。
- 具体怪物、玩家、战斗或任务逻辑。
- 具体 UI 页面和 HUD。
- 服务端房间规则或匹配规则。

具体游戏内容优先放在：

- `project/src/game/features/`：具体玩法功能。
- `project/src/game/screens/`：页面和 HUD。
- `project/src/game/scenes/`：具体游戏场景 ID、场景注册适配和场景专属组合逻辑。

### 2.1 已落地能力

当前 `project/src/framework/scene/` 已实现这些框架层能力：

- `ScenePlugin` 统一注册场景命令、事件、运行状态、注册表、加载队列、spawn 查询、streaming 状态、trigger 和 debug 配置。
- `SceneCommand` 支持 `Enter`、`Exit`、`Switch`、`MarkReady`、`Preload`、`Unload`、`ReloadCurrent`、`SetLayerEnabled`、`SetStreamingEnabled`。其中 `Preload` 和 `Unload` 目前只保留命令形态，生命周期系统尚未执行实际预加载或卸载流程。
- `SceneRuntime` 记录 active、pending、ready、生命周期状态和最近错误；生命周期已覆盖纯 UI 场景、世界内容场景、简单切换、退出清理和失败记录。
- `SceneRegistry` 支持注册纯 UI 场景、首包 manifest 场景和 fallback scene，并校验场景 ID、重复注册和空 manifest 路径。
- `SceneManifest` 支持从首包资源路径加载 RON，当前版本固定为 `"1"`，字段覆盖 entry、layers、spawn_points、anchors、triggers 和 chunks，并提供基础静态校验。
- 首包 manifest 路径以 `project/assets/` 为资源根，例如注册路径写 `scenes/.../scene.ron`，代码会在开发期从 `assets` 或 `project/assets` 查找。
- 资源加载跟踪使用 Bevy `AssetServer::load_untyped` 跟踪 manifest 中 layer asset；required asset 未完成前不进入 Active，required 失败会进入 Failed，optional 失败不阻断进入。
- Loading 联动通过 `UiPanelCommand` 打开或关闭全局 Loading，使用 `SceneLoadProgress` 转换基础 Loading 文案和进度计数。
- 世界内容场景会创建 `SceneRoot`、`SceneLayerRoot`、`SceneRuntimeRoot` 和 `SceneOwned`，退出时按 session 清理；纯 UI 场景不会创建 `SceneRoot`。
- 相机能力已提供基础 2D/3D/default config、场景相机标记和复用策略；具体跟随、震动、过场镜头属于游戏层或后续扩展。
- spawn、anchor、trigger、chunk/streaming 已有 manifest 数据结构、运行时索引或状态查询；trigger 只做通用空间检测和事件派发，streaming 当前只记录元数据与状态，不做真实资源流式加载。
- debug 配置可从环境变量读取启动场景、启动 spawn、调试开关、生命周期日志开关、慢加载模拟和失败模拟；当前提供诊断数据接口，不提供成品游戏内调试面板。
- authority ready 已有框架层消息和状态边界，framework 不依赖 `project/src/game/authority/`。

当前 game layer 已接入一个基础样板场景 `sample.dungeon_room`：

- `project/assets/game/scenes.csv` 是游戏层场景目录表，当前包含 `sample.dungeon_room`，字段覆盖 `scene_id`、启用状态、排序、i18n key/fallback、`kind`、`manifest_path`、`layout_path`、默认 spawn 和 `ui_mode`。
- `project/src/game/scenes/catalog.rs` 在启动时读取首包 CSV，按 `enabled` 过滤并向 `SceneRegistry` 注册 manifest 场景；CSV 解析失败会记录 warning 并保留空 catalog。
- `project/assets/scenes/sample_dungeon_room/scene.ron` 是 framework manifest，声明 dungeon 场景、fixed 3D 相机、blocking required asset Loading、`terrain`/`walls` required layer、`props`/`torches` optional layer、spawn point 和 anchors。
- `project/assets/scenes/sample_dungeon_room/layout.ron` 是 game layer layout，描述 5x5 地板、四周墙体、南侧门、箱桶火把等 prefab 摆放，以及一个方向光和两个点光。
- `project/src/game/scenes/sample_dungeon_room.rs` 监听 `SceneEvent::Entered` 后读取 layout，使用 Bevy glTF `SceneRoot` 实例化 prefab 和 light，把实体挂到 framework layer/runtime root，并统一挂 `SceneOwned(session_id)`。
- `project/src/game/scenes/mod.rs` 当前为 `sample.dungeon_room` 注册了场景 ambience cue；该播放关系走 `SceneAudioAdapterConfig`，不是由 scene manifest 自动驱动。
- 大厅 `game_list` 当前有样板场景固定入口；显示文案使用 `lobby.sample_scene.*`，CSV 中的 `scene.sample_dungeon_room.*` 字段保留给后续 catalog 动态渲染。点击后发送 `SceneCommand::Switch("sample.dungeon_room")`，进入成功后切到 `AppUiMode::SampleScene` 并打开 `project/src/game/screens/gameplay/sample_scene.rs` HUD。
- 样板 HUD 返回按钮会发送 `SceneCommand::Exit` 并路由回大厅；退出时 framework 会清理 `SceneOwned` 场景实体，HUD 由 UI state 退出清理。

### 2.2 未实现设计目标和后续目标

以下内容仍是后续目标，不应在业务文档或接入说明中写成已完成：

- 尚未实现真实后续下载、CDN、内容缓存读取、内容版本哈希校验和 `content_cache://` 场景加载。
- 尚未实现复杂 glTF/additive scene 实例化、碰撞层、导航层、manifest 声明式音频区域、光照环境和 LOD 应用。
- 尚未实现按玩家或相机位置驱动的真实 chunk 加载/卸载；当前 streaming 只保留 chunk 元数据、状态和查询接口。
- 尚未实现任务/剧情/战斗响应、玩家生成、复杂交互、样板场景内容编辑器或 Touch Ripple 业务改造。
- 尚未实现完整远端 authority 协议接入、房间规则、玩家同步、ready 汇总和帧回放 gating；framework 只提供 ready 语义和 adapter 边界。
- 尚未提供完整热重载、可视化调试 overlay、chunk/trigger/anchor 绘制和编辑器工具。

### 2.3 Framework 与 Game Layer 边界

`project/src/framework/scene/` 只提供可复用的场景身份、命令、事件、生命周期、清单、加载、根实体、相机、spawn/anchor、trigger、streaming 元数据、authority ready 边界和诊断数据。

`project/src/game/scenes/` 负责把具体游戏场景 ID 注册进框架、选择 manifest 或纯 UI 定义、组合游戏层系统，并消费 `SceneEvent` 执行业务响应。具体玩法规则、玩家/怪物/战斗/任务/剧情、HUD 文案、页面路由和远端房间协议都留在 game layer。

## 3. 核心术语

### 3.1 SceneRuntime

`SceneRuntime` 是框架层统一的场景概念，表示运行时可进入、可退出、可加载、可卸载的一次场景会话。它可以对应：

- 主城。
- 战斗关卡。
- 副本。
- 房间。
- 新手教学地图。
- UI Gallery 背后的测试环境。
- Touch Ripple 这类单界面玩法场景。
- 登录页、大厅页这类纯 UI 流程。

`SceneRuntime` 不等于 Bevy 的 `Scene` asset，也不等于 glTF 里的 scene。后续框架代码中，场景会话统一命名为 `SceneRuntime`；不要再使用 `GameScene` 或 `SceneSession` 指代框架层场景对象。`session_id` 仍然可以作为 `SceneRuntime` 的字段，用来区分同一个 `SceneId` 的不同运行实例。

### 3.2 SceneRoot

`SceneRoot` 是 `SceneRuntime` 可选拥有的世界内容根实体，用来承载 2D/3D 场景资源、地图、相机、碰撞、触发器、动态玩法实体和 layer roots。

`SceneRuntime` 和 `SceneRoot` 的关系：

- `SceneRuntime` 表示“当前正在运行的场景会话”。
- `SceneRoot` 表示“该会话拥有的世界内容根实体”。
- 纯 UI 场景可以只有 `SceneRuntime`，没有 `SceneRoot`。
- UI 页面、HUD、弹窗和 Loading 仍由 UI 框架管理，不挂到 `SceneRoot` 下。
- 只有当场景需要被统一创建、挂载、卸载的世界实体或场景资源时，才生成 `SceneRoot`。

典型判断：

```text
登录页，纯 UI                  -> SceneRuntime 可选，SceneRoot 无
大厅页，纯 UI                  -> SceneRuntime 可选，SceneRoot 无
大厅页，带 3D 角色或背景       -> SceneRuntime 有，SceneRoot 有
Touch Ripple 玩法              -> SceneRuntime 有，SceneRoot 建议有
战斗、副本、地图、大世界        -> SceneRuntime 有，SceneRoot 有
Loading 过渡页，纯 UI          -> 属于切换流程，SceneRoot 无
```

如果代码中同时使用 Bevy 原生 `SceneRoot` 组件实例化 glTF scene，建议通过模块路径或别名区分，例如把 Bevy 类型导入为 `BevySceneRoot`。本文中的 `SceneRoot` 默认指 MyBevy 场景框架的世界内容根实体。

### 3.3 Scene Id

`SceneId` 是稳定场景 ID。业务和服务端协议只引用 ID，不直接写资源路径。

示例：

```text
touch_ripple.playground
lobby.main
world.capital
dungeon.forest_001
arena.test_small
```

建议规则：

- 小写英文、数字、点号、下划线或短横线。
- 不携带内容版本号。
- 不携带平台后缀。
- 不直接等同于文件路径。

### 3.4 Scene Kind

`SceneKind` 描述场景类型，方便套用加载策略和默认行为：

- `Boot`：启动占位场景。
- `Lobby`：大厅或主界面背景场景。
- `Gameplay`：普通玩法场景。
- `Dungeon`：副本或关卡实例。
- `World`：大世界或开放区域。
- `Arena`：小规模竞技/战斗场景。
- `Dev`：开发测试场景。

### 3.5 SceneRuntime Session

`SceneRuntime` 对应一次场景运行实例。多个玩家进入同一个 `SceneId`，也可能对应不同 session，例如不同副本房间。

建议字段：

- `scene_id`
- `session_id`
- `authority_mode`
- `content_version`
- `spawn_point`
- `seed`
- `entered_at`

单机玩法可以只使用本地 session。联机场景应让服务端或 authority 决定 `session_id`、`seed` 和初始快照。这里的 session 是运行实例标识，不是独立于 `SceneRuntime` 的场景对象。

### 3.6 Zone / Region / Chunk

大型游戏通常会把大场景拆成多级空间单元：

- `Zone`：大的逻辑区域，例如主城、野外区域、副本楼层。
- `Region`：中等空间块，例如街区、战斗房间、山谷。
- `Chunk`：流式加载的最小空间块。

MyBevy 初期不需要马上实现完整大世界流式加载，但场景清单应预留分区概念，避免后续重构资源格式。

### 3.7 Layer

`Layer` 是同一场景内可独立开关的一组内容：

- `base`：地形、静态建筑、基础光照。
- `collision`：碰撞体和阻挡。
- `navigation`：导航网格或寻路数据。
- `props`：可见道具和装饰。
- `actors`：出生点驱动的动态实体。
- `vfx`：环境特效。
- `audio`：环境音和音乐区域。
- `dev`：开发调试标记。

大型游戏常用 additive scene/layer 管理复杂场景。MyBevy 后续也应按层组织加载和卸载，而不是把所有内容绑死在一个巨大 glTF 中。

### 3.8 Spawn Point

`SpawnPoint` 是玩家、AI、相机或临时对象进入场景的位置定义。

建议支持：

- `default`：默认出生点。
- `return_from:<scene_id>`：从某个场景返回时使用。
- `checkpoint:<id>`：关卡检查点。
- `team:<id>`：队伍或阵营出生点。
- `debug:<id>`：开发期直接跳转点。

### 3.9 Scene Anchor

`SceneAnchor` 是场景内的稳定挂点，用于把动态实体挂到具体位置或逻辑对象上。

示例：

- NPC 站位。
- 传送门。
- 镜头看向点。
- 交互按钮位置。
- 音频区域中心。
- 触发器边界。

Anchor 不应依赖美术节点名字的临时拼写。建议在场景元数据里显式声明。

## 4. 目标能力总览

场景系统最终应覆盖这些能力：

- 场景注册：加载场景目录、解析清单、建立 `SceneId` 到资源包的映射。
- 场景进入：根据命令创建 session，准备资源，显示 Loading，实例化内容。
- 场景退出：清理动态实体、关闭场景 UI、停止音频、释放资源引用。
- 场景切换：支持直接切换、Loading 过渡、淡入淡出和失败回退。
- 场景重载：开发期快速重载当前场景。
- 场景分层：支持基础层、碰撞层、导航层、装饰层等独立加载。
- 流式加载：按玩家位置、相机位置或任务状态加载/卸载 chunk。
- 场景查询：通过 ID 查询 spawn point、anchor、区域、触发器、资源状态。
- 相机管理：进入场景时生成或复用相机 rig，支持 2D/3D/固定镜头/跟随镜头。
- 输入绑定：进入场景后切换 gameplay 输入上下文，并尊重 UI 输入阻断。
- 联机同步：场景切换、场景版本、出生点、快照和玩家输入与 authority 对齐。
- 调试工具：显示场景 ID、加载状态、实体数量、分区边界、触发器和资源引用。

## 5. 模块结构

当前 `project/src/framework/scene/` 已拆成以下模块：

```text
project/src/framework/scene/
|-- mod.rs
|-- authority.rs
|-- camera.rs
|-- plugin.rs
|-- command.rs
|-- debug.rs
|-- event.rs
|-- id.rs
|-- lifecycle.rs
|-- manifest.rs
|-- loading.rs
|-- registry.rs
|-- root.rs
|-- spawn.rs
|-- streaming.rs
|-- trigger.rs
`-- prelude.rs
```

职责说明：

- `mod.rs`：场景框架模块边界，只导出通用 framework 能力。
- `authority.rs`：framework 层 authority ready 请求、状态和 adapter 边界，不依赖 game authority。
- `camera.rs`：通用相机 rig、相机配置和默认 2D/3D 相机 helper。
- `plugin.rs`：场景框架插件入口。
- `command.rs`：场景命令定义，例如进入、退出、切换、ready、重载、预加载、layer 和 streaming 开关。
- `debug.rs`：开发期环境变量、诊断快照和实体/layer 调试信息。
- `event.rs`：场景事件和错误分类，例如加载进度、进入完成、ready、输入重置、失败。
- `id.rs`：场景、session、layer、asset、spawn、anchor、trigger、zone、region、chunk 等 newtype。
- `lifecycle.rs`：运行状态资源、生命周期状态机、进入/退出/切换、失败清理和 ready 处理。
- `manifest.rs`：场景清单结构、首包 RON 加载、路径安全检查和基础校验。
- `loading.rs`：资源加载跟踪、Loading 进度和 UI Loading adapter。
- `registry.rs`：场景注册表，维护 `SceneId` 到定义或首包 manifest 的映射。
- `root.rs`：场景根实体、层根实体和统一清理标记。
- `spawn.rs`：出生点、挂点、查询索引和 Transform helper。
- `streaming.rs`：Zone、Region、Chunk 元数据、状态记录和查询；真实流式加载仍未实现。
- `trigger.rs`：触发器 manifest、组件、命令、空间检测和通用事件派发。
- `prelude.rs`：对游戏层暴露常用类型。

当前没有独立 `partition.rs`；分区和 chunk 元数据暂由 `streaming.rs` 承载。后续如果分区查询变复杂，再拆独立模块。

## 6. 场景生命周期

### 6.1 状态模型

建议每个场景 session 使用明确状态：

```text
Idle
Resolving
Downloading
LoadingAssets
Instantiating
Activating
Active
Suspending
Deactivating
Unloading
Failed
```

含义：

- `Idle`：没有活动场景。
- `Resolving`：解析 `SceneId`，查注册表和内容版本。
- `Downloading`：后续下载资源缺失，正在下载或校验。
- `LoadingAssets`：通过 `AssetServer` 加载资源。
- `Instantiating`：需要世界内容时生成 `SceneRoot`、Bevy 实体和场景层级。
- `Activating`：设置相机、输入、音频、spawn、HUD 联动。
- `Active`：场景运行中。
- `Suspending`：临时挂起，例如打开全屏 UI 或进入后台。
- `Deactivating`：退出前停止系统、关闭触发器、冻结动态对象。
- `Unloading`：despawn 场景实体并释放 handle。
- `Failed`：进入或运行过程中失败。

### 6.2 进入流程

标准进入流程：

1. 游戏层发送 `SceneCommand::Enter`。
2. 场景框架查 `SceneRegistry`。
3. 校验场景清单版本、平台、资源依赖和本地缓存。
4. 如果资源缺失，进入下载流程。
5. 显示 Loading UI。
6. 加载场景所需 asset handles。
7. 等待关键资源到达可实例化状态。
8. 如果场景声明了世界内容，生成 `SceneRoot` 和 layer roots。
9. 实例化 glTF、sprite、tilemap、碰撞、触发器等内容。
10. 等 `SceneInstanceReady` 或自定义 ready 条件满足。
11. 如果场景需要世界相机，创建或配置相机。
12. 根据 spawn point 放置玩家和初始对象。
13. 切换输入上下文。
14. 发送 `SceneEvent::Entered`。
15. 关闭 Loading UI。

纯 UI 场景可以跳过第 8 到第 12 步，只保留 `SceneRuntime` 状态、Loading、输入上下文和 UI 路由联动。

### 6.3 退出流程

标准退出流程：

1. 游戏层或框架发送 `SceneCommand::Exit`。
2. 标记当前 scene session 为 `Deactivating`。
3. 停止场景内可重复触发逻辑。
4. 停止或淡出场景音频。
5. 关闭场景专属 HUD 或通知 UI 路由清理。
6. 发送业务层退出事件，让玩法保存必要状态。
7. despawn 带 `SceneOwned` 的实体。
8. 清理未完成的流式加载请求。
9. 释放场景 handle 引用。
10. 发送 `SceneEvent::Exited`。

### 6.4 切换流程

大型游戏通常不会直接从 A 场景硬切到 B 场景，而是通过过渡流程控制体验和资源：

```text
Active(A)
-> Deactivating(A)
-> Loading(B)
-> Instantiating(B)
-> Activating(B)
-> Active(B)
-> Unloading(A)
```

MyBevy 初期可先使用简单模式：

```text
Exit(A)
-> Enter(B)
```

后续如果要做无缝切换，可以保留旧场景一段时间，让新场景加载完成后再卸载旧场景。需要注意内存峰值会显著上升。

### 6.5 失败回退

进入场景失败时不应让应用崩溃。建议回退策略：

- 当前仍有旧场景：保持旧场景运行，显示错误 Toast 或 Confirm。
- 没有旧场景：进入首包 fallback 场景。
- 联机场景失败：通知 authority 或房间层离开当前 session。
- 内容版本不匹配：提示更新或重新拉取内容清单。
- 资源校验失败：删除损坏缓存，尝试重新下载。

## 7. 命令与事件设计

### 7.1 SceneCommand

建议框架层提供消息式接口，类似现有 network 和 authority：

```rust
pub enum SceneCommand {
    Enter(SceneEnterRequest),
    Exit(SceneExitRequest),
    Switch(SceneSwitchRequest),
    Preload(ScenePreloadRequest),
    Unload(SceneUnloadRequest),
    ReloadCurrent(SceneReloadRequest),
    SetLayerEnabled(SceneLayerCommand),
    SetStreamingEnabled(bool),
}
```

建议字段：

```rust
pub struct SceneEnterRequest {
    pub scene_id: SceneId,
    pub session_id: Option<SceneSessionId>,
    pub spawn_point: Option<SpawnPointId>,
    pub content_version: Option<String>,
    pub transition: SceneTransition,
    pub authority: SceneAuthorityMode,
}
```

### 7.2 SceneEvent

建议事件覆盖状态、进度和错误：

```rust
pub enum SceneEvent {
    Resolving(SceneId),
    DownloadProgress(SceneLoadProgress),
    LoadProgress(SceneLoadProgress),
    Instantiating(SceneId),
    Entered(SceneEntered),
    ExitStarted(SceneId),
    Exited(SceneId),
    LayerLoaded(SceneLayerId),
    LayerUnloaded(SceneLayerId),
    ChunkLoaded(SceneChunkId),
    ChunkUnloaded(SceneChunkId),
    Failed(SceneFailure),
}
```

`SceneLoadProgress` 应区分：

- 下载进度。
- Bevy asset 加载进度。
- 实例化进度。
- 必需资源和可选资源。

UI Loading 不应猜测加载状态，而应读取这些事件或资源。

### 7.3 SceneRuntime 资源

建议提供一个当前运行状态资源：

```rust
pub struct SceneRuntime {
    pub active: Option<SceneSessionInfo>,
    pub pending: Option<SceneSessionInfo>,
    pub state: SceneLifecycleState,
    pub last_error: Option<SceneFailure>,
}
```

游戏层只读 `SceneRuntime` 用于显示信息或判断当前场景，不应直接修改它。

## 8. 场景清单

### 8.1 设计目标

场景不应只靠硬编码加载路径。大型游戏通常使用场景表或资源清单把逻辑 ID、资源包、分区、出生点和运行参数绑定起来。

当前已实现首包 RON 场景清单加载，版本固定为 `"1"`。JSON、后续下载 manifest 和从内容平台生成 manifest 仍是后续目标。

### 8.2 清单示例

当前样板场景 manifest 示例：

```ron
(
    version: "1",
    scene_id: SceneId("sample.dungeon_room"),
    kind: "dungeon",
    entry: (
        default_spawn: Some(SceneSpawnPointId("spawn.default")),
        camera: Some((
            id: Some("camera.room_overview"),
            mode: "fixed3d",
            position: Some((0.0, 8.0, 10.0)),
            rotation: Some((-38.0, 0.0, 0.0)),
            projection: Some((kind: "perspective3d", fov_y: 0.9, near: 0.1, far: 200.0)),
        )),
        loading_policy: "blocking_required_assets",
    ),
    layers: [
        (
            id: SceneLayerId("terrain"),
            required: true,
            assets: [
                (
                    id: SceneAssetId("terrain.floor.large"),
                    kind: "gltf_scene",
                    path: "models/scenes/kaykit_dungeon_remastered/floor_tile_large.gltf",
                ),
            ],
        ),
        (
            id: SceneLayerId("walls"),
            required: true,
            assets: [
                (
                    id: SceneAssetId("walls.straight"),
                    kind: "gltf_scene",
                    path: "models/scenes/kaykit_dungeon_remastered/wall.gltf",
                ),
            ],
        ),
        (
            id: SceneLayerId("props"),
            required: false,
            assets: [
                (
                    id: SceneAssetId("props.chest"),
                    kind: "gltf_scene",
                    path: "models/props/kaykit_dungeon_remastered/chest.gltf",
                ),
            ],
        ),
    ],
    spawn_points: [
        (
            id: SceneSpawnPointId("spawn.default"),
            position: (0.0, 0.0, 0.0),
            rotation: (0.0, 0.0, 0.0),
            tags: ["default", "player"],
        ),
    ],
    anchors: [
        (
            id: SceneAnchorId("anchor.exit"),
            position: (0.0, 0.0, -4.0),
            rotation: (0.0, 180.0, 0.0),
            tags: ["exit"],
        ),
    ],
    triggers: [],
    chunks: [],
)
```

### 8.3 必需字段

当前 framework manifest 至少包含：

- `version`：清单格式版本。
- `scene_id`：稳定场景 ID。
- `kind`：场景类型。
- `layers`：资源层。

如果 `entry.default_spawn` 非空，它必须引用 `spawn_points` 中已声明的 spawn point。

### 8.4 可选字段

当前已支持：

- `entry.default_spawn`
- `entry.camera`
- `entry.loading_policy`
- `layers[].assets[].label`
- `spawn_points`
- `anchors`
- `triggers`
- `chunks`

后续可能扩展但尚未实现的 manifest 字段包括 display name、description、平台过滤、画质档位、光照环境、音频区域、天气、导航、碰撞、LOD 和开发备注。这些不要在当前业务文档里写成已可用字段。

### 8.5 校验规则

解析清单时应做静态校验：

- `scene_id` 不为空且格式合法。
- layer ID 唯一。
- asset ID 唯一。
- required layer 至少有一个 asset 或生成策略。
- 默认 spawn point 必须存在。
- 路径不能跳出资源根目录。
- 清单版本必须被当前客户端支持。
- `entry.default_spawn` 如果存在，必须能在 `spawn_points` 中找到。

后续下载资源必须能在内容清单中找到，这仍属于后续下载流程目标。

清单校验失败应输出明确错误，方便策划和美术定位。

## 9. 资源组织

### 9.1 首包场景资源

首包只放启动必需和 fallback 必需内容：

```text
project/assets/
|-- game/
|   `-- scenes.csv
|-- scenes/
|   |-- boot/
|   |   `-- boot_scene.ron
|   |-- fallback/
|   |   `-- fallback_scene.ron
|   `-- sample_dungeon_room/
|       |-- scene.ron
|       `-- layout.ron
|-- models/
|   |-- props/
|   `-- scenes/
|-- textures/
|-- audio/
`-- ui/
```

首包适合：

- 启动画面。
- Loading 背景。
- 错误 fallback 场景。
- 小型测试场景。
- 当前 `sample.dungeon_room` 样板场景的 CSV、manifest、layout 和 KayKit dungeon glTF 资源。
- 当前 Touch Ripple 单界面玩法所需最小资源。

大体积 3D 场景、活动地图、皮肤化场景和关卡包应走后续下载。

当前已落地的样板场景资源路径：

```text
project/assets/game/scenes.csv
project/assets/scenes/sample_dungeon_room/scene.ron
project/assets/scenes/sample_dungeon_room/layout.ron
project/assets/models/scenes/kaykit_dungeon_remastered/*.gltf
project/assets/models/props/kaykit_dungeon_remastered/*.gltf
project/assets/licenses/kaykit_dungeon_remastered_license.txt
```

`scene.ron` 属于 framework manifest，只声明资源依赖、相机、spawn/anchor 和 layer 元数据；manifest 中的 glTF 不会由 framework 自动摆放。`layout.ron` 属于 game layer，当前由 `sample_dungeon_room.rs` 读取后实例化 prefab 和 light。CSV、manifest 和 layout 都使用 `project/assets/` 相对路径。

### 9.2 后续下载场景资源

后续下载内容建议：

```text
content_dist/
`-- 2026.06.17.1/
    |-- manifest.json
    |-- scenes/
    |   `-- forest_001/
    |       |-- scene.ron
    |       |-- collision.ron
    |       |-- navmesh.bin
    |       |-- streaming.ron
    |       `-- lightmap.ron
    |-- models/
    |   `-- scenes/
    |       `-- forest_001/
    |           |-- base.glb
    |           |-- props.glb
    |           `-- vfx.glb
    |-- textures/
    `-- audio/
```

客户端加载路径：

```text
content_cache://2026.06.17.1/scenes/forest_001/scene.ron
content_cache://2026.06.17.1/models/scenes/forest_001/base.glb
```

### 9.3 内容清单关系

`docs/assets-workflow.md` 已规定后续下载资源由内容清单管理。场景清单应和内容清单配合：

- 内容清单负责版本、下载 URL、大小和哈希。
- 场景清单负责场景语义、层、出生点、触发器和资源用途。
- 游戏逻辑引用 `SceneId`。
- 场景框架通过注册表把 `SceneId` 解析到内容版本和场景清单路径。

不要让游戏逻辑直接拼接 glTF 路径。

### 9.4 glTF 使用建议

运行时推荐 `.glb`：

- 单文件便于下载、校验和缓存。
- 相对依赖少，Android 上路径问题少。
- 适合 Bevy 的 `GltfAssetLabel::Scene(0)` 加载方式。

复杂场景不要导出成一个超大 `.glb`。建议拆成：

- 地形或静态建筑。
- 大型装饰。
- 可交互物。
- 动态角色。
- VFX。
- 碰撞和导航数据。

碰撞、导航、触发器和 gameplay 数据不建议只埋在美术 glTF 节点名里。可以从 DCC 工具导出后生成单独 RON/JSON 数据，运行时由框架解析。

## 10. 实体层级与清理

### 10.1 根实体

`SceneRoot` 只在 `SceneRuntime` 需要世界内容时生成。纯 UI 场景没有 `SceneRoot`，其页面、HUD、弹窗和 Loading 都由 UI 框架管理。

带世界内容的场景生成统一根实体：

```text
SceneRootEntity
|-- LayerRoot(base)
|-- LayerRoot(collision)
|-- LayerRoot(navigation)
|-- LayerRoot(props)
|-- LayerRoot(audio)
`-- RuntimeRoot(dynamic)
```

建议组件：

```rust
pub struct SceneRoot {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

pub struct SceneLayerRoot {
    pub layer_id: SceneLayerId,
}

pub struct SceneOwned {
    pub session_id: SceneSessionId,
}
```

退出场景时，通过 `SceneOwned` 或父子层级统一 despawn。没有 `SceneRoot` 的纯 UI 场景只需要关闭对应 UI panel、清理输入上下文和更新 `SceneRuntime` 状态。

### 10.2 静态与动态实体

建议区分：

- 静态实体：来自场景资源，例如地形、建筑、装饰。
- 运行时实体：玩家、AI、投射物、临时特效、触摸反馈。
- 逻辑实体：触发器、区域、出生点、交互点。

静态实体由场景层管理。运行时实体由玩法系统管理，但应挂 `SceneOwned`，避免切场景后残留。

### 10.3 跨场景实体

有些实体不随场景销毁：

- 全局 UI。
- 网络连接和 authority session。
- 全局音频控制器。
- 玩家账号状态。
- 内容清单和缓存索引。

这些实体不要挂 `SceneOwned`。如果玩家角色需要跨场景保留，建议保留数据资源，实体本体在新场景重新生成。

## 11. 相机系统

### 11.1 相机职责

场景框架应提供通用相机能力：

- 2D 正交相机。
- 3D 透视相机。
- 固定镜头。
- 跟随目标镜头。
- 轨道调试镜头。
- 过场镜头。
- Loading 或 UI 背景镜头。

具体跟随策略、镜头震动和战斗镜头可以在游戏层扩展。

### 11.2 Camera Rig

建议用 `CameraRig` 表达相机配置：

```rust
pub enum SceneCameraMode {
    UiOnly2d,
    Gameplay2d,
    Gameplay3d,
    Fixed3d,
    FollowTarget,
    DebugFree,
}
```

进入场景时：

1. 读取场景清单默认 camera。
2. 如果当前没有合适相机，生成相机实体。
3. 如果已有相机可复用，切换投影、Transform 和目标。
4. 如果场景有 `SceneRoot`，把相机绑定到 `SceneRoot`；否则交给全局 camera manager 或 UI 相机管理。

### 11.3 移动端注意事项

移动端场景相机应避免：

- 过高远裁剪距离。
- 无限制实时阴影。
- 大量透明物体覆盖全屏。
- 频繁创建销毁相机。

摄像机切换优先改配置和 Transform，少做实体重建。

## 12. Loading 与 UI 联动

场景系统不直接绘制业务 UI，但应发出足够事件让 UI 框架显示 Loading。

建议联动：

- `SceneCommand::Enter` 后，场景框架发送 `UiPanelCommand::Open(Loading)`。
- `SceneEvent::LoadProgress` 更新 Loading 文案和进度。
- `SceneEvent::Entered` 后关闭 Loading。
- `SceneEvent::Failed` 后关闭 Loading 并显示 Confirm 或 Toast。

场景 Loading 应支持不同模式：

- `None`：开发期或极短加载不显示。
- `Spinner`：未知进度。
- `Progress`：可计算下载和加载进度。
- `Blocking`：阻断输入。
- `NonBlocking`：后台预加载，不阻断当前场景。

当全屏 Loading 打开时，UI 输入框架应阻断 gameplay 输入。当前 UI 框架已经有面板层级和输入阻断能力，场景系统应复用它，而不是自己写一套输入遮罩。

## 13. 场景输入上下文

场景进入后通常要切换输入上下文：

- UI 页面输入。
- 玩法触控输入。
- 角色移动输入。
- 调试相机输入。
- 观战输入。

当前 Touch Ripple 通过 authority 帧同步回放 `ui_touch` 输入。后续场景系统接入时应遵守：

- 原始本地输入先经过 UI 输入阻断判断。
- 需要同步的 gameplay 输入通过 authority 发送。
- 场景内系统消费 authority 帧或本地 fallback 帧。
- 切场景时清理未完成的输入状态，例如按住、拖拽、蓄力。

输入上下文不应由场景美术资源决定，而应由场景类型、游戏模式和 UI 状态共同决定。

## 14. 碰撞与物理

当前项目没有明确物理库。设计上先预留碰撞数据边界：

- 静态阻挡。
- 触发区域。
- 相机边界。
- 投射物碰撞。
- 交互范围。
- 地面高度或导航面。

如果后续接入物理库，应让场景框架负责从场景清单生成通用 collision entities，玩法层只关心碰撞事件或查询结果。

碰撞数据来源建议：

- 简单 2D：RON/JSON 直接定义矩形、圆、多边形。
- 简单 3D：RON/JSON 定义盒、球、胶囊。
- 复杂静态场景：离线烘焙 collision mesh。
- 复杂导航：离线生成 navmesh。

不要把高精度渲染 mesh 直接当运行时碰撞 mesh，移动端成本太高。

## 15. 导航与寻路

大型游戏会把导航作为场景核心数据之一。MyBevy 后续可按需求分阶段：

### 15.1 初期

- 手写几个 navigation zone。
- 用简单路径点或网格寻路。
- 只支持测试 AI 或点击移动原型。

### 15.2 中期

- 支持 tile/grid 导航。
- 支持阻挡动态更新。
- 支持区域代价，例如水、草地、危险区。

### 15.3 后期

- 接入 navmesh。
- 支持分区 navmesh。
- 支持跨 chunk 导航。
- 支持服务端权威寻路或校验。

导航数据应挂在场景清单或独立导航文件里，而不是散落在 AI 代码中。

## 16. 触发器与场景脚本

触发器用于把空间事件转换成游戏事件：

- 玩家进入区域。
- 玩家离开区域。
- 点击交互物。
- 到达检查点。
- 进入传送门。
- 触发剧情。
- 开启战斗。
- 切换音乐。
- 开启或关闭场景 layer。

建议框架层只提供通用触发检测和事件派发：

```text
SceneTriggerEvent {
    trigger_id,
    activator,
    action,
}
```

具体处理留给游戏层：

- 任务系统决定是否推进任务。
- 战斗系统决定是否开战。
- 导航系统决定是否允许传送。
- UI 系统决定是否弹交互按钮。

不要在场景框架中硬编码任务、剧情或战斗逻辑。

## 17. 流式加载

### 17.1 适用场景

流式加载适合：

- 大世界。
- 大型主城。
- 大型 3D 关卡。
- 资源很重的活动地图。
- 需要控制移动端内存峰值的场景。

小型战斗房间和当前 Touch Ripple 玩法不需要一开始接流式加载。

### 17.2 基本策略

按玩家或相机位置维护加载半径：

```text
active radius: 必须加载并显示
warm radius: 后台预加载
cold radius: 可卸载
```

典型流程：

1. 玩家进入 chunk A。
2. A 周围一圈进入 active。
3. 更远一圈进入 warm，后台加载。
4. 远离的 chunk 进入 cold。
5. cold chunk 没有动态实体引用时卸载。

### 17.3 Chunk 数据

每个 chunk 建议记录：

- `chunk_id`
- `bounds`
- `required_layers`
- `optional_layers`
- `neighbor_chunks`
- `asset_refs`
- `memory_budget`
- `priority`

### 17.4 动态实体处理

动态实体不能简单跟随 chunk 卸载。建议：

- 服务器权威实体由 authority 或同步系统管理。
- 纯客户端装饰实体可随 chunk 卸载。
- 掉落物、任务物、战斗对象需要明确归属策略。
- 玩家附近对象优先保留。

## 18. LOD 与性能预算

场景清单应预留不同质量档位：

- `low`
- `medium`
- `high`

移动端优先目标：

- 控制 draw call。
- 控制透明 overdraw。
- 控制纹理尺寸和数量。
- 控制实时阴影。
- 控制骨骼动画数量。
- 控制场景实体数量。
- 控制同时加载的 glTF 数量。

LOD 建议分层：

- Mesh LOD：不同模型精度。
- Material LOD：简化材质。
- Texture LOD：不同贴图尺寸。
- Actor LOD：远处 AI 降低更新频率。
- VFX LOD：降低粒子数量或关闭。
- Audio LOD：远处音源不播放或混合。

初期不必实现完整 LOD 系统，但资源命名和清单字段要避免锁死单档位。

## 19. 光照、环境和后处理

场景应能配置：

- 环境色。
- 主光源方向和颜色。
- 雾。
- 天空盒或背景。
- 反射或环境贴图。
- Lightmap。
- 后处理 profile。

移动端建议：

- 优先使用烘焙或简单光照。
- 谨慎使用实时阴影。
- 后处理按设备档位开关。
- 不要让每个场景随意创建过多光源。

框架层可以提供 `SceneEnvironment` 资源，进入场景时应用；游戏层或设置页可以按画质档位覆盖。

## 20. 音频场景

当前项目已有 `project/src/framework/audio/`，场景音频和 scene framework 的边界如下：

- Scene manifest 当前可用 `audio` / `sound` asset kind 跟踪资源加载依赖。
- 这些 asset kind 只参与场景资源加载状态，不会自动注册 audio catalog，也不会自动播放。
- 实际播放由 `SceneAudioAdapterConfig` 和 game layer 注册决定；当前 `sample.dungeon_room` 的 ambience 就在 `project/src/game/scenes/mod.rs` 注册。
- 进入场景时，audio adapter 响应 `SceneEvent::Entered` 写入播放命令；退出时按 `Scene(session_id)` scope 停止或淡出。

后续目标中的场景音频包括：

- 背景音乐。
- 环境循环音。
- 区域音源。
- 混响区域。
- 战斗音乐切换。
- 过场音频。

设计建议：

- 场景进入时加载基础音频。
- `SceneEvent::Entered` 后淡入背景音乐。
- 离开场景时淡出。
- 触发器可切换音乐状态。
- 大体积音乐走后续下载。
- 音频失败不应阻塞关键场景进入，除非该资源被标记为 required。

当前尚未实现 manifest 声明式音频区域、自动空间音源、区域混响、触发器自动切换音乐或按场景清单自动生成音频实体。

## 21. 联机与 Authority 场景同步

当前项目已有 `AuthorityCommand` / `AuthorityEvent`。场景系统接入联机时应把 authority 作为权威来源之一。

### 21.1 场景进入

联机场景进入建议流程：

1. 客户端请求加入房间或 session。
2. 服务端或 host 返回 `scene_id`、`session_id`、`content_version`、`spawn_point`、`seed`。
3. 客户端校验本地内容版本。
4. 客户端加载场景。
5. 加载完成后发送 ready。
6. authority 等待所有必要 peer ready，或按规则超时开始。
7. authority 下发初始快照。
8. 客户端开始回放权威帧。

### 21.2 场景版本

联机场景必须处理版本一致性：

- `scene_id` 一致。
- `content_version` 一致或兼容。
- 关键碰撞和导航数据哈希一致。
- 玩法配置版本一致。

如果美术资源不同但不影响玩法，可以允许不同 visual 版本；如果碰撞、出生点或玩法参数不同，必须拒绝进入或强制更新。

### 21.3 输入和快照

场景系统不直接处理具体输入，但要提供上下文：

- 当前 session。
- 当前 frame。
- 当前场景状态。
- 当前 spawn point。
- 场景是否 ready。

玩法系统只有在场景 ready 后才应消费 authority frame。切场景期间应暂停或丢弃旧场景输入。

### 21.4 兴趣管理

大世界联机需要 interest management：

- 玩家只接收附近实体。
- 场景 chunk 影响同步范围。
- 服务器按区域下发快照。
- 客户端按 chunk 加载资源。

MyBevy 初期可以不实现，但 `SceneChunkId`、`bounds` 和 session 概念应预留。

## 22. 存档与恢复

单机场景需要保存：

- 当前 `scene_id`。
- 当前 spawn 或坐标。
- 检查点。
- 已触发的场景事件。
- 已开启的门、机关或宝箱。
- 当前任务状态。

场景框架只应保存通用场景状态，具体任务、背包、战斗等由游戏层保存。

恢复流程：

1. 读取存档。
2. 找到 `scene_id` 和 `content_version`。
3. 加载场景。
4. 应用检查点或坐标。
5. 让游戏层恢复动态状态。

## 23. 开发期工作流

当前已提供的开发期能力：

- 从命令行指定启动场景。
- 从命令行指定 spawn point。
- 通过 `SceneCommand::ReloadCurrent` 重载当前场景。
- 列出当前场景实体数量。
- 读取场景生命周期日志开关。
- 读取模拟资源加载失败和慢加载的配置边界。

仍未落地的开发期能力：

- 文件监听式热重载场景清单。
- 可视化显示场景边界、触发器、anchor 和 chunk。
- 完整列出当前 asset handle 和内容缓存来源。

当前支持的环境变量：

```powershell
$env:MYBEVY_START_SCENE="sample.dungeon_room"
$env:MYBEVY_START_SPAWN="spawn.default"
$env:MYBEVY_SCENE_DEBUG="true"
$env:MYBEVY_SCENE_LOG_LIFECYCLE="true"
$env:MYBEVY_SCENE_SLOW_LOADING_SECONDS="1.5"
$env:MYBEVY_SCENE_SIMULATE_FAILURE="asset_load"
cargo run
```

变量含义：

- `MYBEVY_START_SCENE`：启动后自动发送 `SceneCommand::Enter` 的场景 ID。该场景仍必须先由 game layer 注册。
- `MYBEVY_START_SPAWN`：启动场景使用的 spawn point ID。
- `MYBEVY_SCENE_DEBUG`：启用场景 debug 配置；接受 `1`、`true`、`on`、`yes`、`enabled` 等布尔值。
- `MYBEVY_SCENE_LOG_LIFECYCLE`：生命周期日志开关；未设置时随 `MYBEVY_SCENE_DEBUG` 开启。
- `MYBEVY_SCENE_SLOW_LOADING_SECONDS`：慢加载模拟秒数配置，必须是正数。
- `MYBEVY_SCENE_SIMULATE_FAILURE`：失败模拟类型，当前可解析 `manifest_load`、`asset_load`、`camera_setup`。

这些变量只用于开发期，正式版本应由登录、房间、存档或服务端协议决定场景入口。

## 24. 调试面板

场景调试信息建议显示：

- 当前 `scene_id`
- 当前 `session_id`
- 生命周期状态
- 内容版本
- Loading 进度
- 当前 layer 列表和状态
- 当前 chunk 列表和状态
- 场景实体数量
- 动态实体数量
- asset handle 数量
- 最近错误
- 相机模式和位置
- 当前 spawn point
- authority 场景状态

调试绘制建议：

- spawn point：小坐标轴或图标。
- anchor：带 ID 的标记。
- trigger：半透明边界。
- chunk：网格边界。
- navigation：线框或热力色。
- collision：线框。
- camera bounds：矩形或锥体。

调试显示应能在运行时开关，默认不影响正式包性能。

## 25. 错误分类

建议定义明确错误类型：

- `SceneNotFound`
- `ManifestLoadFailed`
- `ManifestParseFailed`
- `ManifestVersionUnsupported`
- `ContentVersionMissing`
- `ContentHashMismatch`
- `RequiredAssetMissing`
- `AssetLoadFailed`
- `SceneInstanceFailed`
- `SpawnPointMissing`
- `CameraSetupFailed`
- `AuthorityRejected`
- `NetworkTimeout`
- `OutOfMemoryRisk`

错误日志应至少包含：

- `scene_id`
- `session_id`
- `content_version`
- 资源 ID 或路径
- 当前生命周期状态
- 原始错误信息

面向玩家的错误文案由 UI/i18n 负责，不应直接显示内部路径和堆栈。

## 26. 与现有系统的关系

### 26.1 与资源系统

依赖 `docs/assets-workflow.md`：

- 首包场景使用 `project/assets/` 相对路径。
- 后续下载场景使用 `content_cache://<version>/...`。
- 大体积场景资源不放入首包。
- 资源路径由内容清单和场景清单解析，不在玩法代码里散落。

### 26.2 与 UI 框架

场景系统通过 UI 框架显示：

- Loading。
- 错误 Toast。
- 确认弹窗。
- 场景调试面板。

场景系统不直接管理 UI 层级，应复用 `UiPanelCommand`、`UiOverlayCommand` 和现有输入阻断机制。

### 26.3 与网络框架

场景系统可通过网络框架下载清单或请求远端场景元数据，但不应直接依赖具体 HTTP/TCP/KCP 实现细节。下载和校验仍应归内容管理流程负责。

### 26.4 与 Authority

联机场景以 authority 下发的场景 session 为准：

- 场景 ID。
- 玩家 ID。
- 出生点。
- 初始快照。
- 帧号。
- 版本要求。

玩法层消费 `AuthorityEvent`，场景层提供 ready 状态和上下文。

### 26.5 与 Gameplay Screens

`project/src/game/screens/gameplay/` 负责页面和 HUD。场景进入后，HUD 可以根据 `SceneEvent::Entered` 打开；场景退出时，应跟随页面路由或场景事件关闭。

不要把 HUD 节点挂到 `SceneRoot` 下。HUD 属于 UI 层，应由 UI 框架管理。纯 UI 场景可以完全没有 `SceneRoot`。

## 27. 推荐落地顺序

### 阶段 1：最小场景生命周期

目标：验证统一场景命令、运行状态和实体清理边界，不绑定具体玩法接入。

实现：

1. 增加 `ScenePlugin`。
2. 定义 `SceneId`、`SceneCommand`、`SceneEvent`、`SceneRuntime`。
3. 支持 `Enter`、`Exit`、`Switch`。
4. 支持纯 UI 场景只创建 `SceneRuntime`，不创建 `SceneRoot`。
5. 支持带世界内容的场景按需生成 `SceneRoot`。
6. 退出时清理 `SceneOwned` 实体。
7. 提供 game layer 可调用的注册入口，用于注册纯 UI 或世界内容测试场景。

验收：

- 能通过命令进入一个已注册的测试场景 ID。
- 纯 UI 测试场景进入后没有 `SceneRoot`。
- 带世界内容的测试场景进入后有 `SceneRoot`。
- 能退出并清理场景实体。
- 重复进入退出不残留实体。

### 阶段 2：场景清单和资源加载

目标：用清单而不是硬编码路径加载场景。

实现：

1. 增加 `SceneManifest`。
2. 支持从 `project/assets/scenes/...` 加载 RON。
3. 支持 required asset handles。
4. 支持 Loading 进度事件。
5. 支持加载失败回退。

验收：

- 清单缺字段能报明确错误。
- required 资源加载完成前不进入 Active。
- 资源失败时能回到 fallback 或保持旧场景。

### 阶段 3：相机、spawn 和触发器

目标：让场景拥有可复用的运行基础。

实现：

1. 支持默认 spawn point。
2. 支持 anchor 查询。
3. 支持基础相机配置。
4. 支持简单 box/circle trigger。
5. 触发器发出通用事件。

验收：

- spawn point 查询能返回稳定 `Transform`，game layer 可据此生成运行时实体。
- 相机按场景配置定位。
- 进入 trigger 后能收到事件。

### 阶段 4：内容缓存和后续下载

目标：场景资源接入内容清单。

实现：

1. 注册 `content_cache` asset source。
2. 场景注册表支持首包和内容缓存两种来源。
3. 场景清单 asset refs 能映射到内容清单。
4. 支持版本和哈希校验失败处理。

验收：

- 首包 fallback 场景可离线进入。
- 下载场景资源后可从 `content_cache://` 加载。
- 缺失资源不会导致崩溃。

### 阶段 5：分层和流式加载

目标：支持大型场景。

实现：

1. 支持 layer 独立启停。
2. 支持 chunk 元数据。
3. 根据框架提供的位置源或相机位置加载附近 chunk。
4. 远离 chunk 自动卸载。
5. 调试显示 chunk 状态。

验收：

- 位置源穿过 chunk 边界时能加载新内容。
- 远离内容能卸载。
- 动态实体不会被错误卸载。

### 阶段 6：联机场景 ready 流程

目标：场景加载状态与 authority 对齐。

实现：

1. authority 下发或确认 `scene_id`、`session_id`、`content_version`。
2. 客户端场景加载完成后发送 ready。
3. ready 前不消费 gameplay frame。
4. 版本不匹配时拒绝进入。

验收：

- 两个客户端进入同一 session 时场景版本一致。
- 慢客户端未 ready 时不会提前开始关键 gameplay。
- 切场景时旧输入不会污染新场景。

## 28. 编码约定

建议：

- 框架层类型放在 `project/src/framework/scene/`。
- 游戏层具体场景注册放在 `project/src/game/scenes/`。
- `SceneId`、`SceneLayerId`、`SceneChunkId` 使用 newtype，避免和普通字符串混用。
- 命令和事件使用 Bevy message/event 模式。
- 资源加载不阻塞 Bevy 主线程。
- 所有场景实体都能通过 root 或 `SceneOwned` 清理。
- 清单解析失败不 panic。
- 场景系统不依赖具体 UI 页面。
- 场景系统不依赖具体 game feature。

避免：

- 在玩法代码里到处写资源路径。
- 用 glTF 节点名承载所有逻辑。
- 切场景只靠手写 despawn 某几个实体。
- Loading UI 自己猜资源是否加载完。
- 联机场景绕过 authority 自行决定出生和开始时间。
- 后续下载资源放进 `project/assets/`。

## 29. 验收清单

新增或修改场景能力时至少检查：

- 场景能进入、退出、重复进入。
- 退出后实体无残留。
- Loading 状态正确关闭。
- 资源缺失时错误可读。
- Android 大小写路径正确。
- 首包资源和后续下载资源来源清晰。
- 清单版本兼容。
- 相机不会重复生成无用实体。
- UI 输入阻断仍然生效。
- authority ready 状态不被绕过。
- `cargo fmt` 和 `cargo check` 通过。

首批基础验收命令：

```powershell
Set-Location project
cargo fmt --check
cargo check
cargo test
Set-Location ..
git diff --check
```

预期结果：

- `cargo fmt --check` 无格式化 diff。
- `cargo check` 编译通过。
- `cargo test` 单元测试和 Bevy app 测试通过。
- `git diff --check` 不报告空白错误。

只改文档时，至少确认：

- 文档没有把未实现能力写成已实现。
- 路径符合当前仓库结构。
- 和 `docs/assets-workflow.md` 的资源约定一致。
