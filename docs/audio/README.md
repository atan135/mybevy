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
- 以 `AudioGroup` 为基础提供轻量 audio bank runtime，支持 load-on-first-use 和 lazy-unload。
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
|-- bank.rs
|-- catalog.rs
|-- catalog_config.rs
|-- command.rs
|-- debug.rs
|-- event.rs
|-- id.rs
|-- lifecycle.rs
|-- loading.rs
|-- metadata.rs
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
- `bank.rs`：基于 `AudioGroup` 的轻量 audio bank runtime、播放触发预加载、活跃实例归属和 lazy-unload。
- `catalog_config.rs`：RON catalog 结构、路径安全校验和首包 catalog 文件读取 helper。
- `playback.rs`：cue/clip/spatial/battle 播放实例、冷却、并发、优先级、淡入淡出和 scope 清理。
- `mixer.rs`：bus 音量、静音、暂停和运行中 sink 同步。
- `music.rs`：`MusicController` 和音乐切换、暂停恢复、停止、淡入淡出、交叉淡入淡出。
- `loading.rs`：`PreloadGroup`、`UnloadGroup`、group 加载进度和失败事件。
- `metadata.rs`：首包音频元数据 manifest 读取、时长缓存和按 clip/path 查询 helper。
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
AudioCommand::PauseInstance
AudioCommand::ResumeInstance
AudioCommand::SeekInstance
AudioCommand::QueryInstanceProgress
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
AudioEvent::InstanceProgress
AudioEvent::InstanceControlFailed
AudioEvent::LoadProgress
AudioEvent::LoadFailed
AudioEvent::MusicChanged
AudioEvent::BusChanged
```

`CueSkipped` 当前能表达冷却、最大并发、低优先级替换失败、缺 cue/clip、bus 暂停和 scope 暂停等原因。

长音频、语音或循环环境音如果需要提前停止，继续复用 `AudioCommand::StopInstance`。单实例暂停和恢复使用 `PauseInstance` / `ResumeInstance`，框架会更新 `AudioPlaybackState`，并在 mixer 同步阶段调用运行中 `AudioSink` 或 `SpatialAudioSink` 的 `pause()` / `play()`。`QueryInstanceProgress` 会返回 `InstanceProgress`，包含 `instance_id`、clip/cue、scope、bus、`position_seconds`、暂停状态和是否空间音频。

`SeekInstance { instance_id, seconds }` 会把负数归零；非有限值会发送 `InstanceControlFailed(InvalidPosition)`。如果实例不存在，发送 `MissingInstance`；如果实例已经在停止/淡出，发送 `StoppedInstance`。如果命令到达时 Bevy sink 尚未创建，框架先记录 `pending_seek_seconds` 和估算 `position_seconds`，等 sink 出现后调用 Bevy 0.18.1 的 `AudioSinkPlayback::try_seek()`。如果底层解码器或 source 不支持 seek，发送 `InstanceControlFailed(SeekUnsupported)`；其他 seek 错误按 `SinkNotReady` 上报。普通和空间实例都走同一套 sink 能力。

`PlayClip`、`PlayCue`、`PlayBattleCue`、`PlaySpatialCue` 和 `PlayMusic` 请求支持可选 `start_seconds`。框架会写入 Bevy `PlaybackSettings.start_position`，同时在实例状态中记录初始 `start_seconds` / `position_seconds`。循环播放时，Bevy 的行为是每轮从该起点开始。

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

当前已落地首包音频时长预处理：

- `scripts/update-audio-manifest.ps1` 会扫描 `project/assets/audio/` 下的首包音频资源。
- 脚本当前用 WAV 文件头中的 RIFF、`fmt ` 和 `data` chunk 计算原始 `duration_seconds`，不依赖 Bevy 运行时 `AudioSource` 加载。
- 输出文件是 `project/assets/audio/audio_manifest.ron`，运行时资源路径从 `project/assets/` 下一级开始写，例如 `audio/music/stealth_bass_loop.wav`。
- manifest 中的 `id` 由资源路径生成，例如 `audio/music/stealth_bass_loop.wav` 对应 `music.stealth_bass_loop`；game layer 仍可以用语义 `AudioClipId` 指向同一个资源路径。
- `AudioPlugin` 启动时读取 `audio/audio_manifest.ron`，加载到 `AudioMetadata` resource。读取失败不会阻断启动，会记录 warning 并保留空 metadata。
- 运行时查询优先通过 `AudioCatalog` 找到 clip 的资源路径，再从 `AudioMetadata` 按 path 查询时长；因此语义 clip ID 只要指向 manifest 中存在的 path，也能查到时长。
- 可直接使用 `AudioMetadata::clip_duration_seconds_by_path("audio/...")` 查询资源路径时长，或使用 `AudioCatalog::clip_duration_seconds(&clip_id, &metadata)` 查询 catalog clip 时长。
- 查询不会触发音频 bytes 或 `AudioSource` 加载；metadata 缺失、catalog clip 缺失或 path 不在 manifest 中时返回 `None`。

新增首包音频时：

- 放入 `project/assets/audio/<category>/...`。
- 使用小写英文、数字、下划线或短横线命名。
- 代码路径写 `audio/...`，不要写 `project/assets/audio/...`。
- Android 路径大小写必须和文件完全一致。
- 二进制音频文件按 Git LFS 规则提交。
- 新增或替换首包音频后运行 `.\scripts\update-audio-manifest.ps1` 更新 `audio/audio_manifest.ron`。

格式边界：

- 当前时长预处理和 Bevy 运行时已验证的是 `.wav`。
- `scripts/update-audio-manifest.ps1` 已为 OGG、MP3、FLAC 留出扩展点，但这些格式当前没有 parser；脚本遇到这些扩展名会报错，避免生成不完整时长。
- 文档或测试里可能仍有 `.ogg` 示例路径，但当前依赖只显式开启 `wav` feature；新增 OGG/Vorbis、MP3、FLAC 等格式前，需要补预处理 parser，并确认 Bevy feature 和目标平台支持。
- 大体积音乐、活动语音和可替换内容仍应走后续下载设计，不应直接放进首包。

## 8. UI 音效

已落地能力：

- `AudioPlugin` 注册 `UiAudioAdapterConfig` 和 `UiAudioCooldowns`。
- `play_ui_button_audio` 消费 `UiButtonEvent`。
- 默认 cue 是 `DEFAULT_UI_CLICK_CUE_ID`。
- 控件可挂 `UiAudioCueOverride` 覆盖 cue。
- UI click 有短冷却，避免高频连点无限堆叠。
- UI bus 或 Master bus 静音、暂停时会跳过 UI 音效并发送 skip 事件。
- Lobby 头部提供“音频设置”页面入口，当前可调整 `Master`、`Music`、`Sfx`、`Ui`、`Battle` bus 的 0-100 音量，并可切换 Master 静音；页面通过 `AudioCommand::SetBusVolume` / `SetBusMuted` 驱动 mixer，不直接操作 sink。

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
- `PlayMusic` 支持 `start_seconds`，适合长音乐或语音从指定时间点起播。
- 已有音乐实例也可以通过通用 `PauseInstance`、`ResumeInstance`、`SeekInstance`、`QueryInstanceProgress` 和 `StopInstance` 做单实例控制。
- `CrossfadeMusic` 支持旧音乐淡出、新音乐淡入。
- `MusicController` 记录当前和淡出中的 track。
- bus 音量和暂停状态会影响当前播放音乐。

边界：

- 具体哪个页面或场景播放哪首音乐由 game layer 注册。
- 当前没有复杂音乐状态机、分层音乐、节拍同步、stinger 或过场音乐编排。
- seek 和进度读取依赖 Bevy/rodio sink；框架会记录估算快照，但不承诺所有音频格式、平台或后续自定义 source 都支持精确 seek。

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

### 13.1 当前 group 加载能力

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

### 13.2 基础 Audio Bank 和 Lazy Unload

已落地能力：

- 播放 group 内任意 cue/clip 时，自动确保整个 group 处于 loading 或 loaded 状态。
- 当前这次播放请求不等待整个 group 完成加载，继续允许现有 lazy play 兜底，避免第一次点击无响应。
- group 中其他音频异步加载，并由 `AudioLoadingState` 持有 `AudioSource` handle。
- 只要 group 内仍有任意活跃播放实例，就保持 group 加载状态。
- group 内所有实例停止后，启动 lazy-unload 计时器。
- 计时器到期前如果再次播放 group 内成员，取消本轮自动卸载。
- 计时器到期且 group 内仍无活跃实例时，发送 `AudioCommand::UnloadGroup`，释放框架持有的 group handles。
- `AudioBankRuntime` 记录 group 配置、加载状态、活跃实例集合、实例归属、idle countdown 和重复映射诊断。
- `AudioBankGroupConfig` 可为每个 group 单独配置 lazy-unload 时长。

- `0` 表示 resident group，默认不自动卸载，适合 UI、战斗通用、常用脚步等高频音效。
- 大于 `0` 表示 idle 超时自动卸载，适合开发测试页、场景局部 ambience、低频玩法音效或临时活动资源。

当前约束：

- 一个 cue/clip 最多归属一个 lazy group，避免同一实例跨多个 group 导致 active count 和卸载归属复杂化。
- 手动 `PreloadGroup` 仍可提前加载 group。
- 手动 `UnloadGroup` 只释放 group 持有的 handles，不负责停止正在播放的实例；停止播放仍使用 `StopInstance`、`StopMusic` 或 `StopByScope`。
- paused、looped、music crossfade 中的实例只要仍活跃，就继续阻止 group 自动卸载。
- `UnloadGroup` 仍不承诺 Bevy 全局 asset 缓存立即释放内存；如果其他 strong handle 仍存在，资源可以继续留在 Bevy asset 系统中。

Audio Gallery 是该机制的第一批验证页面：

- 注册 `bank.audio_gallery`，包含 SFX、loop、music、spatial 和 voice 样例。
- 配置一个非 0 lazy-unload group，用于验证 idle countdown 和自动卸载。
- 配置一个 lazy-unload 时长为 `0` 的 resident group 样例，用于验证常驻行为。
- 页面显示 group 状态：not loaded、loading、loaded、idle countdown、resident。
- Audio Monitor 显示 group 加载进度、最近 started/skipped cue 和 load failed，辅助验证 bank 行为。

仍不属于第一版目标：

- Wwise event、RTPC、state、switch 或 soundbank 运行时。
- LRU、全局内存预算、优先级淘汰和跨场景资源热迁移。
- 音频 streaming、远端 catalog 热更新和后续下载闭环。
- 自动扫描 manifest 生成 bank 或 UI。

### 13.3 Audio Gallery 开发测试页

Audio Gallery 是开发期音频测试页面，位于 `AppUiMode::AudioGallery`。它只用于客户端开发和验收，不改变正式玩法音频语义，也不作为成品游戏音频配置入口。

入口：

- 大厅顶部的 `Audio Gallery` 按钮。
- `Audio Settings` 和 `Audio Monitor` 页面内的跳转入口。
- 桌面开发可用环境变量直达：

```powershell
$env:TOUCH_START_SCREEN="audio_gallery"
cargo run
```

`audio-gallery` 是同等可用的 alias。

页面能力：

- SFX、Loop/Ambience、普通 clip、长音频 seek/progress、Music、Spatial、Mixer/Loading、Rules/Stress 和 Diagnostics 分区。
- 播放入口覆盖 `PlayCue`、`PlayClip`、`PlayMusic`、`CrossfadeMusic` 和 `PlaySpatialCue`。
- 实例控制覆盖 pause、resume、stop、fade-out stop、seek 和 progress query。
- Mixer 区可发出 bus volume、mute、pause/resume 命令，验证运行中普通实例和空间实例跟随 bus 状态。
- Loading 区可手动 `PreloadGroup` / `UnloadGroup` `bank.audio_gallery`，并显示 lazy group 和 resident group 状态。
- Rules / Stress 区提供 cooldown、max_concurrent、missing cue/clip 样例，用于观察 skipped 和 load failed。

页面清理策略：

- Audio Gallery 主动播放的普通、空间和音乐实例统一使用 `dev.audio_gallery` scope。
- 退出 `AppUiMode::AudioGallery` 时发送 `StopByScope(dev.audio_gallery)`，移除页面 state、空间 listener binding、listener proxy 和 helper entities。
- 退出时只清理 `bank.audio_gallery` 这类非 resident dev bank 的 runtime/idle 状态并发起 unload，不误卸载 `bank.audio_gallery.resident`。
- 页面状态追踪会过滤默认 `ui.click`，避免按钮点击音效覆盖 Gallery 主动启动的实例状态。

边界：

- 当前空间音频只验证 Bevy 0.18.1 的基础 stereo panning 和距离衰减，不承诺 HRTF、混响、遮挡或完整 3D 音频。
- 页面复用首包 `project/assets/audio/` 样例和 `dev.audio.*` clip/cue ID；不要把这些 ID 当作正式玩法语义。
- Audio Gallery 不自动扫描全部 manifest 或 catalog，不负责音频资源授权、格式转换或远端下载验收。
- `bank.audio_gallery` 的 lazy-unload 只表示框架释放 group handles；不保证 Bevy 全局 asset 缓存立即释放真实内存。

## 14. 调试和诊断

非页面诊断仍支持环境变量：

```powershell
$env:MYBEVY_AUDIO_DEBUG="true"
cargo run
```

支持的真值包括 `1`、`true`、`on`、`yes`、`enabled`，假值包括 `0`、`false`、`off`、`no`、`disabled`。

进入游戏内音频性能监控页面时会自动启用采集；使用上述环境变量时，未进入页面也会持续采集。启用后，`AudioDebugSnapshot` 会记录：

- 是否启用 debug。
- 当前活跃实例总数和按 bus 统计。
- 暂停、停止中或淡出中、空间音频和循环播放实例数量。
- 音频资源 bytes 估算：资源数量、总 bytes、按目录 bytes、最大资源路径和大小。
- 当前播放实例引用资源的估算 bytes。
- 活跃实例详情：instance、clip、cue、scope、bus、路径、暂停、停止中、失败、空间音频标记。
- 当前加载 group 进度。
- 最近开始的 cue。
- 最近跳过的 cue。
- 最近加载失败资源。
- 基础阈值提示，例如实例数过高、同 cue 并发过高、资源过大和 required 加载失败。

实例详情中还包含 `looped`、`start_seconds`、`position_seconds`、`duration_seconds` 和 `pending_seek_seconds`，用于排查长音频起播、暂停和 seek 状态。资源 bytes 来自首包音频文件 metadata 扫描，是音频资源 bytes 或估算内存，不等同真实进程内存。

游戏内音频性能监控页面位于 `AppUiMode::AudioMonitor`，可从大厅或音频设置页进入，进入后会自动开启 `AudioDebugConfig.enabled`。也可通过启动环境变量直接打开：

```powershell
$env:TOUCH_START_SCREEN="audio_monitor"
cargo run
```

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
- 音频资源热重载、远端 catalog 热更新和授权检查工具。
- 所有格式和平台上的精确 seek 保证；当前 seek 依赖 Bevy/rodio sink 能力，不支持时会发失败事件。

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
