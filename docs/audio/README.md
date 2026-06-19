# MyBevy 音频框架说明

这个目录记录 MyBevy 音频框架的当前实现、使用边界和后续目标。当前项目已经落地独立的 `project/src/framework/audio/` 模块，并在 `GamePlugin` 中接入 `AudioPlugin`。

本文中“已落地能力”描述当前代码事实；“后续目标”只表示设计方向，不应在业务文档或接入说明中写成已完成。

## 1. 文档目标

音频框架给游戏层提供统一的播放、加载、混音、作用域清理和诊断接口，让 UI、场景、战斗和后续下载内容可以通过同一套命令流表达音频意图。

框架层负责：

- 管理音频 ID、catalog、cue、group、播放实例和加载状态。
- 提供 Bevy message 风格的 `AudioCommand` 和 `AudioEvent`。
- 通过 bus 管理音量、静音、暂停，并同步到运行中的 `AudioSink` 和 `SpatialAudioSink`。
- 管理音乐播放、停止、暂停、恢复、淡入淡出和交叉淡入淡出。
- 按 `AudioScope` 停止、暂停、恢复和清理播放实例。
- 提供 UI、场景、战斗和空间音频的轻量 adapter。
- 提供 `MYBEVY_AUDIO_DEBUG` 诊断开关和 `AudioDebugSnapshot`。
- 在移动端生命周期进入后台时按策略暂停 background bus。

框架层不负责：

- 决定具体按钮、角色、技能、怪物、任务或剧情使用哪个音效。
- 编排具体游戏音乐、剧情语音或活动资源下发策略。
- 管理音频授权、美术制作和资源导出流程。
- 处理真实后续下载、CDN、缓存配额、版本哈希校验或内容回滚。
- 提供复杂空间音频能力，例如 HRTF、混响区域、遮挡模拟或高级衰减曲线。

具体映射和业务规则应放在：

- `project/src/game/screens/`：页面、HUD 和控件对音频 cue 的使用。
- `project/src/game/features/`：具体玩法触发音效。
- `project/src/game/scenes/`：具体场景注册、场景音频 adapter 和场景专属组合逻辑。
- `project/src/framework/fight/` 或 game layer 战斗模块：发出战斗事件或 cue 意图。

## 2. 当前项目边界

当前仓库约定：

- Rust/Bevy 工程根目录是 `project/`。
- 当前 Bevy 依赖是 `bevy = { version = "0.18.1", features = ["wav"] }`。
- 首包资源目录是 `project/assets/`。
- 首包音频目录是 `project/assets/audio/`，当前实际样例资源主要是 `.wav`。
- Android Gradle 壳工程会把 `../../project/assets` 打包进 APK assets。
- 后续下载资源不放入 `project/assets/`。

音频框架位于：

```text
project/src/framework/audio/
```

它是 `framework` 下的横向能力，与 `ui`、`scene`、`network`、`fight` 同级。当前 game layer 已在 `project/src/game/scenes/mod.rs` 为 `sample.dungeon_room` 注册了样板场景 ambience cue，使用首包资源：

```text
audio/ambience/light_rain_loop.wav
```

## 3. 已落地模块

当前 `project/src/framework/audio/` 模块结构：

```text
project/src/framework/audio/
|-- mod.rs
|-- battle.rs
|-- catalog.rs
|-- catalog_config.rs
|-- command.rs
|-- debug.rs
|-- event.rs
|-- id.rs
|-- lifecycle.rs
|-- loading.rs
|-- mixer.rs
|-- music.rs
|-- playback.rs
|-- plugin.rs
|-- prelude.rs
|-- scene.rs
|-- scope.rs
|-- spatial.rs
`-- ui.rs
```

职责说明：

- `plugin.rs`：`AudioPlugin` 入口，注册 messages、resources 和系统顺序。
- `id.rs`：`AudioClipId`、`AudioCueId`、`AudioGroupId`、`AudioScopeId`、`AudioInstanceId`。
- `scope.rs`：`AudioBus` 和 `AudioScope`。
- `command.rs`：播放、停止、scope 控制、bus 控制、音乐和 group 加载命令。
- `event.rs`：播放开始、跳过、停止、加载失败、加载进度、音乐变化和 bus 变化事件。
- `catalog.rs`：内存版 clip、cue、group 目录和解析逻辑。
- `catalog_config.rs`：RON catalog 结构、路径安全校验和首包 catalog 文件读取 helper。
- `playback.rs`：cue/clip/spatial/battle 播放实例、冷却、并发、优先级、淡入淡出和 scope 清理。
- `mixer.rs`：bus 音量、静音、暂停和运行中 sink 同步。
- `music.rs`：`MusicController` 和音乐切换、暂停恢复、停止、淡入淡出、交叉淡入淡出。
- `loading.rs`：`PreloadGroup`、`UnloadGroup`、group 加载进度和失败事件。
- `ui.rs`：消费 `UiButtonEvent` 播放默认或覆盖的 UI click cue，并做短冷却。
- `scene.rs`：消费 `SceneEvent`，按 `SceneAudioAdapterConfig` 在进入场景时播放、退出时按 scene scope 停止。
- `spatial.rs`：基础空间 cue、listener 绑定、固定或跟随实体音源、简单距离衰减和空间 sink 同步。
- `battle.rs`：`BattleAudioCue` helper，把战斗 cue 映射成 `PlayBattleCue`。
- `lifecycle.rs`：应用后台/恢复时按策略暂停或恢复 `Music`、`Sfx`、`Battle` bus。
- `debug.rs`：`MYBEVY_AUDIO_DEBUG`、最近 cue/失败记录、活跃实例和加载 group 快照。

## 4. 核心类型

### 4.1 AudioClipId

`AudioClipId` 是稳定音频文件 ID，指向一个具体音频 asset。

示例：

```text
ui.click_wood_01
music.menu_loop
ambience.sample_dungeon_room
battle.sword_hit_01
voice.line_01
```

`AudioClipId` 不应携带内容版本号、平台后缀或临时路径。

### 4.2 AudioCueId

`AudioCueId` 是播放语义 ID。游戏层优先播放 cue，而不是直接散落资源路径。

示例：

```text
ui.button.click
scene.sample_dungeon_room.ambience
battle.weapon.hit
```

一个 cue 当前可以映射到单个或多个 clip，支持权重、默认 bus、默认 scope、looped、音量、音高、冷却时间、最大并发和优先级。当前变体选择是确定性的加权轮转，不是随机播放。

### 4.3 AudioBus

当前已实现的 bus：

- `Master`：总音量。
- `Music`：背景音乐。
- `Sfx`：通用音效和当前场景 ambience。
- `Ui`：UI 音效。
- `Battle`：战斗音效。

当前还没有独立 `Ambience` 或 `Voice` bus。环境音和语音可以先按业务需要映射到现有 bus，后续如果需要更细粒度设置再扩展。

有效音量由 `Master`、具体 bus 和实例音量共同计算。运行中修改 bus 音量、静音或暂停会同步到已经存在的非空间和空间 audio sink。

### 4.4 AudioScope

当前已实现的 scope：

- `Global`：全局常驻实例。
- `Ui`：UI 音效。
- `Scene(AudioScopeId)`：随场景 session 清理。
- `Entity(Entity)`：跟随某个实体或空间音源。
- `Battle(AudioScopeId)`：随战斗实例清理。

`StopByScope`、`PauseByScope` 和 `ResumeByScope` 会按 scope 控制实例。场景退出时，`SceneAudioAdapterConfig` 会按当前 scene session scope 停止场景音频。

### 4.5 AudioInstanceId

`AudioInstanceId` 表示一次播放实例，用于停止、淡出、调试和事件关联。短音效通常不需要业务保存 instance ID；音乐、循环音、空间音源和语音更适合保留或通过 scope 管理。

## 5. 命令和事件

音频框架使用 Bevy message 风格。常用命令：

```text
AudioCommand::PlayCue
AudioCommand::PlayBattleCue
AudioCommand::PlaySpatialCue
AudioCommand::PlayClip
AudioCommand::PlayMusic
AudioCommand::CrossfadeMusic
AudioCommand::StopMusic
AudioCommand::PauseMusic
AudioCommand::ResumeMusic
AudioCommand::StopInstance
AudioCommand::StopByScope
AudioCommand::PauseByScope
AudioCommand::ResumeByScope
AudioCommand::SetBusVolume
AudioCommand::SetBusMuted
AudioCommand::SetBusPaused
AudioCommand::PreloadGroup
AudioCommand::UnloadGroup
```

当前没有 `AudioCommand::SetListener`。空间音频 listener 使用 `AudioSpatialListenerBinding` resource 绑定目标实体。

常用事件：

```text
AudioEvent::CueStarted
AudioEvent::ClipStarted
AudioEvent::CueSkipped
AudioEvent::InstanceStopped
AudioEvent::LoadProgress
AudioEvent::LoadFailed
AudioEvent::MusicChanged
AudioEvent::BusChanged
```

`CueSkipped` 当前能表达冷却、最大并发、低优先级替换失败、缺 cue/clip、bus 暂停和 scope 暂停等原因。

## 6. Catalog 和资源路径

首包音频目录：

```text
project/assets/audio/
|-- ambience/
|-- battle/
|-- common/
|-- music/
|-- spatial/
|-- ui/
`-- voice/
```

代码和 catalog 中的首包路径从 `project/assets/` 下一级开始写：

```text
audio/ui/click_wood_01.wav
audio/music/menu_loop.wav
audio/ambience/light_rain_loop.wav
```

当前 `AudioCatalog` 可以由代码注册，也可以通过 `AudioCatalogConfig` 从 RON 构建。RON catalog 支持：

- `clips`：clip ID 到资源路径的映射。
- `cues`：cue ID、clip 列表、权重、playback 和 rules。
- `groups`：预加载分组和 required/optional 标记。
- `playback.bus`：`master`、`music`、`sfx`、`ui`、`battle`。
- `playback.scope`：`global`、`ui`、`scene`、`battle`。
- `rules.volume`、`rules.pitch`、`rules.cooldown_seconds`、`rules.max_concurrent`、`rules.priority`。

路径安全规则：

- 使用正斜杠。
- 不允许空路径、绝对路径、Windows 盘符、反斜杠、`..`。
- 只允许普通 assets 相对路径或 `content_cache://...`。

`content_cache://...` 目前只是 catalog 路径校验层允许的 URI 形式。真实下载、缓存注册、版本哈希校验和 Android 私有缓存读取还没有完整落地，不能把后续下载音频写成已完成能力。

## 7. 当前首包资源和格式

当前首包音频样例在 `project/assets/audio/`，主要用于音频框架开发和内部测试。该目录下的 `readme.md` 已说明这些文件是开发期占位资源，不是最终游戏内容。

当前实际文件是 `.wav`，并且 `project/Cargo.toml` 已开启 Bevy `wav` feature：

```toml
bevy = { version = "0.18.1", features = ["wav"] }
```

新增首包音频时：

- 放入 `project/assets/audio/<category>/...`。
- 使用小写英文、数字、下划线或短横线命名。
- 代码路径写 `audio/...`，不要写 `project/assets/audio/...`。
- Android 路径大小写必须和文件完全一致。
- 二进制音频文件按 Git LFS 规则提交。

格式边界：

- 当前已验证和启用的是 `.wav`。
- 文档或测试里可能仍有 `.ogg` 示例路径，但当前依赖只显式开启 `wav` feature；新增 OGG/Vorbis、MP3、FLAC 等格式前，需要先确认 Bevy feature 和目标平台支持。
- 大体积音乐、活动语音和可替换内容仍应走后续下载设计，不应直接放进首包。

## 8. UI 音效

已落地能力：

- `AudioPlugin` 注册 `UiAudioAdapterConfig` 和 `UiAudioCooldowns`。
- `play_ui_button_audio` 消费 `UiButtonEvent`。
- 默认 cue 是 `DEFAULT_UI_CLICK_CUE_ID`。
- 控件可挂 `UiAudioCueOverride` 覆盖 cue。
- UI click 有短冷却，避免高频连点无限堆叠。
- UI bus 或 Master bus 静音、暂停时会跳过 UI 音效并发送 skip 事件。

应用层接入约定：

- 默认按钮点击音效使用确认语义，监听 `UiButtonEventKind::Click`，也就是按下后在按钮内松开并真正触发按钮动作时播放。
- 不把默认点击音效放在 `Down` 按压瞬间播放，避免用户按下后拖出按钮、取消或被输入阻断时出现“没触发动作但已经响了”的假反馈。
- 普通按钮不需要挂额外组件，默认播放 `UiAudioAdapterConfig.default_click_cue_id`。
- 需要特殊音效的按钮挂 `UiAudioCueOverride` 指向自定义 cue，例如确认、警告或特殊入口按钮。
- 默认点击 cue 和特殊点击 cue 的 clip 注册由 game layer 或初始化代码负责。

边界：

- 当前 adapter 只覆盖按钮点击事件，不自动覆盖所有弹窗、Toast、输入错误或页面切换。
- 具体 UI cue 的 catalog 注册仍由 game layer 或初始化代码负责。

## 9. 背景音乐

已落地能力：

- `AudioCommand::PlayMusic` 播放音乐，默认走 `Music` bus。
- `StopMusic` 支持立即停止或淡出停止。
- `PauseMusic` 和 `ResumeMusic` 更新 `MusicController` 和当前实例暂停状态。
- `CrossfadeMusic` 支持旧音乐淡出、新音乐淡入。
- `MusicController` 记录当前和淡出中的 track。
- bus 音量和暂停状态会影响当前播放音乐。

边界：

- 具体哪个页面或场景播放哪首音乐由 game layer 注册。
- 当前没有复杂音乐状态机、分层音乐、节拍同步、stinger 或过场音乐编排。

## 10. 场景音频

已落地能力：

- `SceneAudioAdapterConfig` 可按 `SceneId` 注册 `SceneAudioEntry`。
- `SceneAudioEntry` 支持 `on_enter` 播放 cue 或 music。
- 进入场景后，adapter 消费 `SceneEvent::Entered`，把播放命令写入 scene session scope。
- `SceneEvent::ExitStarted` 可按配置淡出 scene scope。
- `SceneEvent::Exited` 会再次按 scene scope 停止，避免残留。
- 当前 `sample.dungeon_room` 已在 game layer 注册一个循环 ambience cue：`scene.sample_dungeon_room.ambience`。

应用层接入约定：

- 场景背景音乐由 game layer 的场景表配置，而不是硬编码在 scene framework 中。
- 场景表应能表达当前场景的背景音乐 clip ID、资源路径、音量、淡入和退出淡出参数。
- 注册场景表时同步把音乐资源注册进 `AudioCatalog`，并把对应 `SceneAudioMusic` 注册进 `SceneAudioAdapterConfig`。
- 场景 BGM 默认循环播放，进入场景时随 `SceneEvent::Entered` 播放，退出场景时按 `Scene(session_id)` scope 停止或淡出。
- 当前可先使用 `project/assets/audio/music/` 下已有的首包音乐，例如 `audio/music/stealth_bass_loop.wav` 或 `audio/music/menu_loop.wav`。

重要边界：

- `SceneManifest` 当前能把 `audio` / `sound` asset kind 当作资源依赖加载跟踪。
- manifest 的 `audio` / `sound` asset 不会自动注册 catalog，也不会自动播放。
- 实际场景音频播放由 `SceneAudioAdapterConfig` 和 game layer 注册决定。
- 当前还没有 manifest 声明式音频区域、自动空间音源、区域混响或触发器自动切换音频。

## 11. 空间音频

已落地能力：

- `AudioCommand::PlaySpatialCue`。
- `AudioSpatialSource::Fixed` 和 `AudioSpatialSource::FollowEntity`。
- `AudioSpatialListenerBinding` 绑定 listener 到实体。
- 空间 emitter Transform 跟随实体更新。
- 目标实体消失时停止对应空间实例并发送停止事件。
- 基于 listener 和 emitter 距离做简单线性/指数衰减计算。
- 空间 sink 同步 bus 音量和暂停状态。

边界：

- Bevy 0.18.1 空间音频是基础 stereo panning。
- 当前不提供 HRTF、混响、遮挡、区域声学、复杂曲线或自动 Audio LOD。
- 当前没有 `MYBEVY_AUDIO_DISABLE_SPATIAL` 环境变量。

## 12. 战斗音效

已落地能力：

- `AudioCommand::PlayBattleCue` 使用 `Battle(AudioScopeId)` scope。
- 如果 cue 默认 bus 是 `Sfx`，战斗 cue 会默认映射到 `Battle` bus。
- `BattleAudioCue` helper 可生成 `PlayBattleCue` 命令。
- cue rules 支持冷却、最大并发和优先级。并发已满时，高优先级 cue 可替换低优先级实例；同优先级或更低优先级会跳过。
- `StopByScope(AudioScope::Battle(...))` 可清理指定 battle scope。

边界：

- 具体技能、武器、怪物、命中类型和战斗状态到 cue 的映射仍属于 game layer 或 fight adapter。
- 当前没有完整战斗音频编排、混音 ducking、技能优先级矩阵、语音抢占规则或音频事件表。

## 13. 加载和后续下载边界

已落地能力：

- `AudioGroupEntry` 可声明 group 中 clip 的 required/optional。
- `AudioCommand::PreloadGroup` 会为 group 中的 clip 创建 `AudioSource` handle。
- `AudioLoadingState` 记录 group 加载状态。
- `AudioEvent::LoadProgress` 报告 loaded/failed/required 计数。
- optional clip 失败不影响 required 进度统计。
- `AudioCommand::UnloadGroup` 会移除框架保存的 group 加载状态。

边界：

- 当前 group 加载只管理 `AssetServer` handle 和框架状态，不实现磁盘缓存下载。
- `UnloadGroup` 不保证 Bevy 全局 asset 缓存立即释放内存，只移除 audio loading state 中的引用。
- `content_cache://...` 路径已可通过 catalog 路径校验，但真实内容缓存源注册、下载和校验仍是后续目标。

## 14. 调试和诊断

当前支持的环境变量：

```powershell
$env:MYBEVY_AUDIO_DEBUG="true"
cargo run
```

支持的真值包括 `1`、`true`、`on`、`yes`、`enabled`，假值包括 `0`、`false`、`off`、`no`、`disabled`。

启用后，`AudioDebugSnapshot` 会记录：

- 是否启用 debug。
- 当前活跃实例总数和按 bus 统计。
- 活跃实例详情：instance、clip、cue、scope、bus、路径、暂停、停止中、失败、空间音频标记。
- 当前加载 group 进度。
- 最近开始的 cue。
- 最近跳过的 cue。
- 最近加载失败资源。

当前没有实现这些旧设计变量：

```powershell
$env:MYBEVY_AUDIO_MUTE="music"
$env:MYBEVY_AUDIO_LOG_CUES="true"
$env:MYBEVY_AUDIO_DISABLE_SPATIAL="true"
```

如需这些能力，应先补代码，再更新文档。

## 15. 移动端和 Android

当前 Android 工程在 `android/app/build.gradle` 中把 `../../project/assets` 加入 APK assets，因此 `project/assets/audio/...` 会随 APK 打包。运行时首包路径仍写 `audio/...`。

当前移动端相关能力：

- Bevy/rodio/cpal 音频链路沿用 Bevy 0.18.1 默认行为。
- `AudioLifecyclePausePolicy` 默认在 `WillSuspend` / `Suspended` 暂停 `Music`、`Sfx`、`Battle` bus。
- `WillResume` / `Running` 时只恢复由策略暂停的 bus，不覆盖用户或业务原本暂停的 bus。
- `Ui` bus 默认不随后台策略暂停。

移动端新增音频时仍需关注：

- 同时播放实例数量和高频短音效堆叠。
- `.wav` 体积较大，大体积音乐和语音不要盲目进首包。
- Android 路径大小写。
- 资源授权是否允许随 APK 分发。

## 16. 与现有系统的关系

### 16.1 与资源流程

音频资源遵守 `docs/assets-workflow.md`：

- 首包资源放在 `project/assets/audio/`。
- 代码和 catalog 路径从 `audio/...` 开始写。
- Android 当前会把整个 `project/assets` 打进 APK assets。
- 后续下载资源设计使用 `content_cache://<version>/audio/...`，但真实下载和缓存加载还未完整接入。
- 音频二进制文件通过 Git LFS 提交。

### 16.2 与 UI 框架

UI 框架负责 UI 事件、面板和输入阻断。音频框架通过 adapter 消费 UI 事件并播放 cue，不直接管理 UI 层级。

### 16.3 与场景框架

场景框架负责场景生命周期、manifest 资源加载跟踪和 `SceneOwned` 清理。音频框架通过 `SceneAudioAdapterConfig` 响应 `SceneEvent`，按 `Scene(session_id)` scope 管理场景音频。

manifest 中的 `audio` / `sound` 当前只表示“这个资源需要被场景加载流程跟踪”，不是自动播放声明。

### 16.4 与战斗框架

战斗框架或 game layer 发出战斗事件或音频 cue。音频框架负责 cue 解析、并发、优先级、bus、scope 和实际播放。

### 16.5 与网络和 Authority

联机玩法中，关键 gameplay 音效可以由 authority 帧或已确认事件驱动，避免客户端预测和回滚造成明显错音。纯 UI 音效仍可本地即时播放。

## 17. 当前未实现能力

以下内容仍是后续目标，不要写成当前已完成：

- 真实后续下载、CDN、缓存配额、版本哈希校验和 `content_cache://` 音频加载闭环。
- 独立 `Ambience`、`Voice` bus 和玩家设置持久化。
- manifest 声明式音频区域、自动音源生成、自动触发器切换音乐。
- 复杂空间音频：HRTF、混响、遮挡、环境区域、复杂衰减曲线和 Audio LOD。
- 完整战斗音频编排：ducking、优先级矩阵、角色语音抢占、按技能表自动映射。
- UI 全量音效 adapter：弹窗、Toast、输入错误、页面切换等。
- 游戏内 audio debug overlay 或设置页面。
- 音频资源热重载、远端 catalog 热更新和授权检查工具。

## 18. 基础验收

修改音频框架代码后至少执行：

```powershell
Set-Location project
cargo fmt --check
cargo check
cargo test
Set-Location ..
git diff --check
```

只改文档时，至少执行：

```powershell
git diff --check
```

验收时还应确认：

- 文档没有把未实现能力写成已实现。
- 首包资源路径符合 `project/assets/` 相对路径规则。
- Android 路径大小写正确。
- `.wav` 支持和 `Cargo.toml` feature 描述一致。
- 后续下载资源没有误放进 `project/assets/`。
