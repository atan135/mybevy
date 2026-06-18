# MyBevy 音频框架设计文档

这个目录记录 MyBevy 音频框架相关设计。当前项目尚未实现独立 `framework::audio` 模块，本文描述后续补齐音频框架时建议采用的边界、模块结构、数据流和落地顺序。

本文中的设计目标应按阶段落地；除非代码已经接入，否则不要在业务文档中写成已完成能力。

## 1. 文档目标

音频框架的目标是给游戏层提供统一、可复用的播放和管理接口，让 UI、场景、战斗、活动和后续下载内容都能通过同一套命令流播放声音。

框架层负责：

- 管理音频资源 ID、路径、加载状态和播放配置。
- 提供音效、音乐、环境音、空间音频和战斗音效的统一命令接口。
- 管理音量总线、静音、暂停、淡入淡出和运行中音量更新。
- 管理音频播放实例、作用域和生命周期清理。
- 提供调试、诊断和基础性能约束。

框架层不负责：

- 决定具体按钮、技能、角色、怪物或剧情使用哪个音效。
- 编写具体游戏音乐编排规则。
- 处理音频资源授权、美术制作和导出规范以外的内容生产流程。
- 处理运营活动的具体资源下发策略。
- 直接依赖 game layer、具体 UI 页面、具体战斗系统或远端房间协议。

具体映射和业务规则应放在：

- `project/src/game/screens/`：页面和 HUD 对音频 cue 的使用。
- `project/src/game/features/`：玩法功能触发音效。
- `project/src/game/scenes/`：具体场景的音乐、环境音和空间音源配置。
- `project/src/framework/fight/` 或 game layer 战斗模块：发出战斗事件或 cue 意图，但不直接控制底层播放实体。

## 2. 当前项目边界

当前仓库约定：

- Rust/Bevy 工程根目录是 `project/`
- 当前 Bevy 版本是 `bevy = "0.18.1"`
- 首包资源目录是 `project/assets/`
- 后续下载资源不放入 `project/assets/`
- `docs/assets-workflow.md` 已经约定音频资源可放在 `project/assets/audio/`
- `project/src/framework/scene/manifest.rs` 已能识别 `audio` 和 `sound` 资源类型，但只用于场景资源加载跟踪，尚未接入音频播放框架

音频框架建议位于：

```text
project/src/framework/audio/
```

并作为 `framework` 下的横向能力，与 `ui`、`scene`、`network`、`fight` 同级。不要把通用音频能力塞进 UI、场景或战斗模块内部。

## 3. 核心术语

### 3.1 AudioClipId

`AudioClipId` 是稳定音频文件 ID，指向一个具体音频 asset。

示例：

```text
ui.click.default
ui.modal.open
music.lobby.main
ambience.dungeon.room
battle.hit.light_01
```

`AudioClipId` 应稳定，不建议携带内容版本号、平台后缀或临时路径。

### 3.2 AudioCueId

`AudioCueId` 是播放语义 ID。游戏层通常播放 cue，而不是直接播放某个文件。

示例：

```text
button.primary.click
panel.open
scene.dungeon.enter
battle.skill.cast
battle.weapon.hit
```

一个 cue 可以映射到：

- 单个 clip。
- 多个随机 clip。
- 带音量、音高随机的 clip 列表。
- 带冷却、并发限制和优先级的播放规则。

### 3.3 AudioBus

`AudioBus` 是音量和静音控制分组。

建议基础分组：

- `Master`：总音量。
- `Music`：背景音乐。
- `Sfx`：通用音效。
- `Ui`：UI 音效。
- `Ambience`：环境循环音。
- `Battle`：战斗音效。
- `Voice`：语音和对白。

最终音量通常是 `Master * Bus * Cue * Instance` 的组合结果。

### 3.4 AudioScope

`AudioScope` 表示播放实例的生命周期归属。

建议 scope：

- `Global`：全局常驻，例如主菜单音乐控制器。
- `Ui`：页面、弹窗、Toast、按钮等 UI 音效。
- `Scene(session_id)`：随场景 session 清理。
- `Entity(entity)`：跟随某个实体或空间音源。
- `Battle(battle_id)`：随战斗实例清理。

切场景、关弹窗、结束战斗时，框架可以按 scope 停止或淡出音频，避免残留。

### 3.5 AudioInstanceId

`AudioInstanceId` 是一次播放实例的 ID，用于停止、暂停、淡出或查询状态。

短音效通常不需要业务保存 instance ID；背景音乐、循环环境音、语音和空间音源更需要可追踪实例。

## 4. 目标能力总览

音频框架建议覆盖：

- 音效资源管理：ID、路径、分组、首包和后续下载来源。
- 音效资源加载：预加载、加载组、加载状态、失败回退。
- 游戏内 UI 音效：按钮、弹窗、切页、输入、错误提示。
- 游戏内背景音乐：播放、停止、暂停、恢复、淡入淡出、交叉淡入淡出。
- 游戏内场景声音：环境循环音、空间音源、区域切换和触发器联动。
- 游戏内战斗音效：技能、命中、格挡、暴击、死亡、倒计时和并发控制。
- 音量总线：总音量、音乐、音效、UI、环境、战斗和语音。
- 播放实例管理：暂停、恢复、停止、淡出、作用域清理。
- 调试诊断：活跃实例数、bus 状态、最近 cue、失败资源和静音开关。

## 5. 建议模块结构

建议后续新增：

```text
project/src/framework/audio/
|-- mod.rs
|-- prelude.rs
|-- plugin.rs
|-- id.rs
|-- command.rs
|-- event.rs
|-- catalog.rs
|-- loading.rs
|-- playback.rs
|-- mixer.rs
|-- music.rs
|-- spatial.rs
|-- scope.rs
`-- debug.rs
```

职责说明：

- `mod.rs`：音频框架模块边界。
- `prelude.rs`：对 game layer 暴露常用类型。
- `plugin.rs`：`AudioPlugin` 入口，注册消息、资源和系统。
- `id.rs`：`AudioClipId`、`AudioCueId`、`AudioGroupId`、`AudioInstanceId` 等 newtype。
- `command.rs`：播放、停止、预加载、bus 控制和音乐切换命令。
- `event.rs`：播放开始、完成、失败、加载状态变化等事件。
- `catalog.rs`：音频资源目录、cue 规则、group 和路径映射。
- `loading.rs`：音频预加载和加载状态跟踪。
- `playback.rs`：普通音效播放实例管理。
- `mixer.rs`：bus 音量、静音、暂停和运行中音量同步。
- `music.rs`：背景音乐状态机、淡入淡出和交叉淡入淡出。
- `spatial.rs`：空间音源、监听器和场景音频区域。
- `scope.rs`：播放实例归属和清理规则。
- `debug.rs`：环境变量、诊断快照和调试开关。

## 6. 命令和事件

音频框架应优先使用 Bevy message 风格，保持和当前 `SceneCommand`、`UiOverlayCommand` 一致。

建议命令：

```text
AudioCommand::PlayCue
AudioCommand::PlayClip
AudioCommand::PlayMusic
AudioCommand::CrossfadeMusic
AudioCommand::StopInstance
AudioCommand::StopByScope
AudioCommand::PauseByScope
AudioCommand::ResumeByScope
AudioCommand::PreloadGroup
AudioCommand::UnloadGroup
AudioCommand::SetBusVolume
AudioCommand::SetBusMuted
AudioCommand::SetBusPaused
AudioCommand::SetListener
```

建议事件：

```text
AudioEvent::CueStarted
AudioEvent::CueSkipped
AudioEvent::InstanceFinished
AudioEvent::InstanceStopped
AudioEvent::LoadProgress
AudioEvent::LoadFailed
AudioEvent::MusicChanged
AudioEvent::BusChanged
```

`CueSkipped` 应能表达被冷却、并发限制、静音策略或资源缺失跳过，方便调试。

## 7. 资源目录和 Catalog

首包音频建议目录：

```text
project/assets/audio/
|-- ui/
|-- music/
|-- ambience/
|-- battle/
|-- voice/
`-- common/
```

代码中路径从 `project/assets/` 下一级开始写：

```text
audio/ui/click.ogg
audio/music/lobby_main.ogg
```

后续下载音频继续遵守 `docs/assets-workflow.md`：

```text
content_cache://2026.06.09.1/audio/music/event_theme.ogg
```

建议引入音频 catalog，记录：

- clip ID。
- 资源路径。
- 所属 group。
- 默认 bus。
- 是否循环。
- 默认音量和音高。
- 是否首包必需。
- 内容版本或来源。

cue 规则记录：

- cue ID。
- 可选 clip 列表。
- 随机权重。
- 音量随机范围。
- 音高随机范围。
- 冷却时间。
- 最大并发。
- 优先级。
- 默认 scope。

## 8. 加载策略

建议分组加载：

- `audio.ui.core`：基础 UI 点击、确认、取消、错误提示，适合首包预加载。
- `audio.music.lobby`：大厅音乐，按页面或场景加载。
- `audio.scene.<scene_id>`：场景音乐、环境音和空间音源。
- `audio.battle.common`：通用战斗命中、释放、受击和倒计时音效。
- `audio.voice.<content_id>`：对白或语音，优先后续下载。

基础原则：

- 短 UI 音效和关键反馈音可进首包。
- 背景音乐、活动语音、大体积音频优先走后续下载。
- 音频加载失败通常不应阻塞关键流程，除非资源被显式标记为 required。
- 场景 manifest 中的 audio 资源可先作为加载依赖跟踪，后续再映射到 audio catalog 和播放策略。

## 9. UI 音效

UI 音效应通过事件或 adapter 接入，不应让每个按钮直接操作 Bevy 音频实体。

建议接入点：

- `UiButtonEvent`：按钮按下、点击、取消。
- `UiModalResult`：确认、取消、关闭。
- `UiPanelCommand` 或 panel 状态变化：打开、关闭、切换。
- 文本输入提交、错误提示、Toast 显示。

建议策略：

- 提供默认点击 cue，例如 `ui.button.click`。
- 支持具体控件覆盖 cue。
- 对高频 UI cue 增加短冷却，避免连点造成刺耳堆叠。
- UI 音效默认走 `Ui` bus，不受 `Battle` bus 影响。
- 打开全屏 Loading 时可继续播放 UI 提示音，但 gameplay 音效应按场景或输入状态控制。

## 10. 背景音乐

背景音乐建议由独立 `MusicController` 管理。

基础能力：

- 播放一首音乐。
- 停止当前音乐。
- 暂停和恢复。
- 淡入。
- 淡出。
- 交叉淡入淡出。
- 记录当前音乐 ID 和播放状态。

典型场景：

- 登录页播放登录音乐。
- 大厅播放大厅音乐。
- 进入副本时切副本音乐。
- 触发战斗时切战斗音乐。
- 战斗结束后恢复场景音乐。

音乐通常走 `Music` bus，并使用 `Global` 或 `Scene(session_id)` scope。场景音乐建议随场景退出淡出；全局音乐可跨场景持续。

## 11. 场景声音

场景声音包括：

- 背景音乐。
- 环境循环音，例如风声、水声、地下城底噪。
- 空间音源，例如火把、瀑布、机关和 NPC。
- 区域音频，例如进入洞穴后切换 ambience。
- 过场音频。

建议与 scene framework 的关系：

- 场景进入后，根据 `SceneEvent::Entered` 播放场景音乐和基础 ambience。
- 场景退出或切换时，按 `Scene(session_id)` scope 停止或淡出。
- 场景 trigger 可发送业务事件，由 game layer 决定是否切换音乐或环境音。
- 空间音源应挂到实体或 anchor，并使用 `Entity(entity)` 或 `Scene(session_id)` scope。

当前 `SceneManifest` 已能识别 `audio` 和 `sound` asset kind。后续可以扩展 manifest 或 game layer layout，让场景声明音频资源和音源位置，但 framework scene 不应硬编码具体播放规则。

## 12. 战斗音效

战斗音效应由战斗事件驱动，音频框架只处理播放策略。

常见 cue：

- 技能开始。
- 技能释放。
- 命中。
- 暴击。
- 格挡。
- 受击。
- 护盾破裂。
- 死亡。
- 回合开始。
- 倒计时。

战斗音效需要额外约束：

- 高频命中音应有最大并发。
- 同类音效可做随机变体，减少重复感。
- 重要音效应有更高优先级。
- 同一帧大量事件不应无限创建播放实例。
- 战斗结束时按 `Battle(battle_id)` scope 清理循环音和延迟音。

具体角色、武器、技能和怪物映射哪个 cue，属于 game layer 或 fight adapter，不属于 audio framework。

## 13. 音量和运行时控制

音量控制建议分两层：

- 配置层：玩家设置中的 bus 音量、静音和开关。
- 实例层：当前正在播放的实体实际音量。

Bevy 的全局音量适合初始化总量，但运行中修改音量时，应由框架追踪活跃播放实例并同步到 `AudioSink` 或空间音频 sink。不要假设修改全局音量会自动影响所有已经播放的音频实例。

建议保存：

- `master_volume`
- `music_volume`
- `sfx_volume`
- `ui_volume`
- `ambience_volume`
- `battle_volume`
- `voice_volume`
- 各 bus muted/paused 状态

设置页只应发送 `AudioCommand::SetBusVolume` 或 `AudioCommand::SetBusMuted`，不直接访问底层 sink。

## 14. 空间音频

空间音频适用于 2D/3D 场景中的位置声音。

建议能力：

- 设置 listener 实体或 Transform。
- 播放空间 cue。
- 跟随实体更新音源位置。
- 设置最大距离、衰减曲线和基础音量。
- 随场景或实体清理。

Bevy 当前空间音频能力偏轻量，适合左右声道 panning 和基础距离感。复杂 HRTF、混响区域和遮挡模拟应作为后续扩展，不作为第一阶段目标。

## 15. 失败处理

音频失败不应让游戏主流程崩溃。

建议错误类型：

- `ClipNotFound`
- `CueNotFound`
- `GroupNotFound`
- `AssetLoadFailed`
- `InvalidVolume`
- `InstanceNotFound`
- `PlaybackUnavailable`
- `UnsupportedFormat`

错误日志应包含：

- cue ID。
- clip ID。
- 资源路径。
- bus。
- scope。
- 原始错误信息。

玩家侧通常不需要展示音频错误，开发期通过日志和 debug 面板查看。

## 16. 调试和诊断

建议提供调试信息：

- 当前 master 和各 bus 音量。
- bus 静音和暂停状态。
- 当前音乐 ID。
- 活跃音频实例数量。
- 按 bus 统计的实例数量。
- 最近播放 cue。
- 最近跳过 cue。
- 最近加载失败资源。
- 当前 listener 位置。

建议环境变量：

```powershell
$env:MYBEVY_AUDIO_DEBUG="true"
$env:MYBEVY_AUDIO_MUTE="music"
$env:MYBEVY_AUDIO_LOG_CUES="true"
$env:MYBEVY_AUDIO_DISABLE_SPATIAL="true"
```

这些变量只用于开发期。正式设置应来自玩家设置或游戏配置。

## 17. 移动端注意事项

移动端音频应控制：

- 同时播放实例数。
- 短音效高频创建。
- 大体积音乐首包占用。
- 未使用音频 handle 长期保留。
- 音频文件格式和平台兼容性。
- 后台、息屏、来电时的暂停和恢复策略。

建议：

- 短音效优先使用 OGG 或经验证的平台格式。
- 音乐和语音优先后续下载。
- UI 核心音效保持小体积并进首包。
- 战斗高频 cue 必须有并发限制。
- 进入后台时暂停 `Music`、`Ambience` 和 `Battle` bus，按产品需求决定 UI 和 Voice。

## 18. 与现有系统的关系

### 18.1 与资源流程

音频资源遵守 `docs/assets-workflow.md`：

- 首包资源放在 `project/assets/audio/`。
- 代码路径从 `audio/...` 开始写。
- 后续下载资源走内容清单、缓存和 `content_cache://...`。
- 音频二进制文件通过 Git LFS 提交。

### 18.2 与 UI 框架

UI 框架负责 UI 事件、面板和输入阻断。音频框架通过 adapter 消费 UI 事件并播放 cue，不直接管理 UI 层级。

### 18.3 与场景框架

场景框架负责场景生命周期、manifest 和 `SceneOwned` 清理。音频框架按 `Scene(session_id)` scope 管理场景音频，并响应场景进入、退出或 trigger 事件。

### 18.4 与战斗框架

战斗框架或 game layer 发出战斗事件或音频 cue。音频框架负责并发、优先级、bus、scope 和实际播放。

### 18.5 与网络和 Authority

联机玩法中，关键 gameplay 音效可以由 authority 帧或已确认事件驱动，避免客户端预测和回滚造成明显错音。纯 UI 音效仍可本地即时播放。

## 19. 推荐落地顺序

### 阶段 1：最小播放闭环

目标：让 game layer 通过命令播放首包音效和音乐。

实现：

1. 新增 `framework/audio/` 模块和 `AudioPlugin`。
2. 定义 `AudioCommand`、`AudioEvent`、`AudioCueId`、`AudioClipId`。
3. 支持 `PlayCue`、`PlayClip`、`StopInstance`。
4. 支持 `Master`、`Music`、`Sfx`、`Ui` bus。
5. 用内存 catalog 注册少量首包音频。

验收：

- 能通过命令播放一个 UI 音效。
- 能播放并停止一首背景音乐。
- 能调整 UI 和 Music bus 音量。

### 阶段 2：UI 音效接入

目标：基础 UI 交互自动播放音效。

实现：

1. 接入 `UiButtonEvent`。
2. 提供默认按钮点击 cue。
3. 支持控件或 action 覆盖 cue。
4. 增加 UI cue 冷却。

验收：

- 普通按钮点击有反馈音。
- 连点不会无限堆叠音效。
- 静音 UI bus 后 UI 音效停止，其他 bus 不受影响。

### 阶段 3：音乐控制器

目标：支持页面和场景音乐切换。

实现：

1. 增加 `MusicController`。
2. 支持淡入、淡出和交叉淡入淡出。
3. 支持按 scope 停止场景音乐。

验收：

- 大厅和样板场景能切换不同音乐。
- 切场景时旧音乐淡出，新音乐淡入。
- 修改 Music bus 音量能影响当前播放音乐。

### 阶段 4：场景音频

目标：场景拥有环境循环音和基础空间音源。

实现：

1. 按 `SceneEvent::Entered` 播放场景 ambience。
2. 按 `SceneEvent::Exited` 清理 `Scene(session_id)` 音频。
3. 支持 `PlaySpatialCue`。
4. 支持 listener 绑定相机或玩家。

验收：

- 进入样板场景播放环境音。
- 退出场景后环境音停止。
- 空间音源随相机或玩家位置变化有左右声道变化。

### 阶段 5：战斗音效和并发限制

目标：高频战斗事件不会造成音频失控。

实现：

1. 支持 cue 最大并发。
2. 支持 cue 冷却和优先级。
3. 支持随机变体。
4. 支持 `Battle(battle_id)` scope。

验收：

- 同一帧大量命中事件不会创建无限音频实例。
- 重要技能音效不会被普通命中音完全盖住。
- 战斗结束时循环音和残留音能清理。

### 阶段 6：后续下载和调试面板

目标：音频资源接入内容清单，并提供可视化诊断。

实现：

1. catalog 支持 `content_cache://...` 路径。
2. 支持 `PreloadGroup` 和加载进度。
3. 增加 debug snapshot。
4. 在 UI debug 面板显示 audio 状态。

验收：

- 下载音频资源后可播放。
- 资源缺失时能记录明确错误。
- debug 面板能看到 bus、音乐和实例状态。

## 20. 验收清单

新增或修改音频能力时至少检查：

- 首包资源路径符合 `project/assets/` 相对路径规则。
- Android 路径大小写正确。
- 音量设置能影响当前播放实例。
- 静音和暂停按 bus 生效。
- 场景退出后场景音频无残留。
- 战斗高频音效有并发限制。
- 资源加载失败不导致主流程崩溃。
- 后续下载资源没有误放进 `project/assets/`。
- `cargo fmt` 和 `cargo check` 通过。

只改文档时，至少确认：

- 文档没有把未实现能力写成已实现。
- 路径符合当前仓库结构。
- 和 `docs/assets-workflow.md` 的资源约定一致。
