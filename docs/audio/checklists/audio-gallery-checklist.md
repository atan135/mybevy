# Audio Gallery Checklist

## 目标

新增一个开发期音频测试界面，类似当前 UI Gallery，用于在游戏内主动测试音效、循环音、背景音乐、实例暂停/恢复/停止、seek、bus 混音、基础 audio bank 加载和基础空间音频表现。该页面只作为客户端开发和验收工具，不改变正式玩法音频语义。

同时补一层轻量 audio bank 运行时机制：把现有 `AudioGroup` 作为基础 bank 使用，支持 load-on-first-use + lazy-unload。播放 group 内任意成员时加载整个 group，group 内无活跃播放一段时间后自动卸载；每个 group 可以单独配置 lazy-unload 时长，`0` 表示常驻不自动卸载。

## 基础原则

- [x] 本模块优先放在 `project/src/game/screens/dev/`，作为 `AudioMonitor` 和 `UiGallery` 的同级开发页面。
- [x] 第一版复用现有 `project/assets/audio/` 首包样例资源，不新增非必要音频资源。
- [x] 第一版通过代码注册一组稳定的 dev sample clip/cue，不依赖自动枚举全部 manifest 或 catalog。
- [x] 第一版把现有 `AudioGroup` 作为基础 audio bank 使用，只要求一组音频能一起预加载并持有 handle，不实现 Wwise event、RTPC、state、switch、streaming 或复杂内存淘汰策略。
- [x] 基础 audio bank 采用 load-on-first-use + lazy-unload 策略：播放任意成员时加载整个 group，group 内无活跃播放一段时间后自动卸载。
- [x] 每个 group 可单独配置 lazy-unload 时长；时长为 `0` 表示默认常驻，不自动卸载，适合 UI、战斗通用等高频音效。
- [x] 第一版限定一个 clip/cue 最多归属一个 lazy group，避免同一实例跨多个 group 导致 active count 和卸载归属复杂化。
- [x] 不改变现有 `AudioCommand` 播放语义；播放仍通过 `PlayCue`、`PlayClip`、`PlayMusic`、`PlaySpatialCue` 发起。
- [x] 页面主动播放的实例使用专用 scope，退出页面时统一停止，避免测试音频残留到大厅、玩法或场景中。
- [x] UI 按钮点击自身会触发默认 `ui.click`，本页状态追踪必须过滤出 Audio Gallery 主动启动的实例。
- [x] 空间音频测试只承诺 Bevy 0.18.1 当前支持的 stereo panning 和距离衰减，不把它描述成 HRTF、混响、遮挡或完整 3D 音频系统。
- [x] 页面布局和控件风格复用现有 UI Gallery / Audio Settings 模式，兼容手机竖屏、1080p 和平板窗口 profile。
- [x] 如改动启动入口、目录结构、音频测试流程或 UI 文案，需要同步检查 `docs/bevy-getting-started.md`、`docs/audio/README.md` 和 `docs/ui/` 是否需要更新。

## 阶段 1：入口和页面骨架

- 阶段开始时间：2026-06-24 13:34:41 +08:00
- 阶段结束时间：2026-06-24 13:49:47 +08:00
- 阶段开发总结：新增 Audio Gallery 页面模式、专用 owner/panel、dev 页面骨架和 Lobby/Audio Settings/Audio Monitor 入口；支持 `TOUCH_START_SCREEN=audio_gallery` / `audio-gallery` 直达；补充 owner 映射和启动 alias 单元测试。本阶段未实现实际音频播放控制、catalog 或 bank runtime。

- [x] 新增 `AppUiMode::AudioGallery`。
- [x] 新增 Audio Gallery 专用 `UiOwnerId` 和 `UiPanelId`。
- [x] 在 `project/src/game/screens/dev/` 下新增 `audio_gallery.rs`。
- [x] 在 `DevScreensPlugin` 中注册 `OnEnter(AppUiMode::AudioGallery)`、`Update` 和 `OnExit` 系统。
- [x] 在大厅顶部增加 `Audio Gallery` 入口按钮。
- [x] 在 `Audio Settings` 和 `Audio Monitor` 页增加到 Audio Gallery 的跳转入口。
- [x] 支持通过 `TOUCH_START_SCREEN=audio_gallery` 直接启动到测试页。
- [x] 页面头部提供返回 Lobby、Audio Settings、Audio Monitor 的路径。

## 阶段 2：测试音频 catalog 注册

- 阶段开始时间：2026-06-24 14:00:45 +08:00
- 阶段结束时间：2026-06-24 14:26:59 +08:00
- 阶段开发总结：新增 game-side Audio Gallery dev sample 注册模块，使用 `dev.audio.*` clip/cue ID 复用首包音频资源；注册 cooldown、max_concurrent、missing optional 失败样例；注册 `bank.audio_gallery` lazy dev group 和 `bank.audio_gallery.resident` 常驻 group，并提供轻量 lazy-unload 配置资源。新增单元测试覆盖 clip/cue/group 路径、required/optional 和 lazy/resident 配置。本阶段未实现 lazy bank runtime，也未实现页面播放控件。

- [x] 定义 Audio Gallery 专用 clip/cue ID 前缀，例如 `dev.audio.*`，避免和正式游戏 cue 冲突。
- [x] 注册 UI/SFX 样例：`audio/ui/notify_horn_01.wav`、`audio/common/footstep_concrete_01.wav`、`audio/battle/sword_hit_01.wav`。
- [x] 注册循环环境音样例：`audio/ambience/light_rain_loop.wav` 或 `audio/ambience/city_walla_loop.wav`。
- [x] 注册音乐样例：`audio/music/menu_loop.wav` 和 `audio/music/stealth_bass_loop.wav`。
- [x] 注册空间音频样例：`audio/spatial/car_horn_taps.wav` 和 `audio/spatial/dog_bark_city_03.wav`。
- [x] 注册语音样例：至少选择一个 `audio/voice/*.wav`。
- [x] 注册可触发 cooldown 的测试 cue。
- [x] 注册可触发 max_concurrent 的测试 cue。
- [x] 注册一个 missing asset 或 optional 失败样例，用于验证失败路径和 Audio Monitor 记录。
- [x] 为 Audio Gallery 注册一个 dev bank group，例如 `bank.audio_gallery`，包含页面内可能同时测试的 SFX、loop、music、spatial 和 voice clip。
- [x] 为 dev bank group 配置非 0 lazy-unload 时长，用于验证自动卸载。
- [x] 预留至少一个常驻 group 配置样例，lazy-unload 时长为 `0`，用于验证不会自动 unload。
- [x] 为预加载测试在 dev bank group 中覆盖 required 和 optional clip。
- [x] 增加单元测试确认 dev sample clip/cue/group 已注册且路径正确。

## 阶段 3：基础 Audio Bank 运行时

- 阶段开始时间：2026-06-24 14:47:31 +08:00
- 阶段结束时间：2026-06-24 15:25:02 +08:00
- 阶段开发总结：新增 framework audio lazy bank runtime，把 `AudioGroup` 配置成轻量 bank，维护 group 配置、clip/cue 到 group 映射、冲突诊断、加载请求状态、活跃实例集合和 idle countdown。播放 `PlayCue`、`PlayClip`、`PlayMusic`、`CrossfadeMusic`、`PlaySpatialCue`、`PlayBattleCue` 覆盖的成员时同帧确保 group preload，但不阻塞原播放语义；手动 preload/unload 继续走原命令，manual unload 不停止活跃实例。运行时监听 started/music/stopped 事件归属实例，非 resident group 空闲超时后自动发起 unload，resident group 不自动卸载。GameAudioPlugin 将阶段 2 的 Audio Gallery dev/resident bank 配置注册到通用 runtime。新增 focused bank 单元测试覆盖 load-on-first-use、timer cancel、timer unload、resident、manual unload、命令映射和重复映射冲突。

- [x] 增加轻量 lazy bank runtime resource，记录 group 配置、加载状态、活跃实例和 idle countdown。
- [x] 建立 `clip_id/cue_id -> group_id` 映射，第一版限制一个 clip/cue 最多属于一个 lazy group。
- [x] 建立 `group_id -> lazy_unload_seconds` 配置。
- [x] 建立 `group_id -> active_instance_ids` 运行时状态。
- [x] 建立 `group_id -> idle countdown` 运行时状态。
- [x] 播放 group 任意成员时，自动确保整个 group 处于 loading 或 loaded 状态。
- [x] 当前播放请求不等待整个 group 完成加载，继续允许 lazy play 兜底，避免首次点击无响应。
- [x] 手动 `PreloadGroup` 可提前加载 group。
- [x] 手动 `UnloadGroup` 只释放 group 持有的 handles，不负责停止正在播放的实例。
- [x] 监听 `CueStarted`、`ClipStarted`、`MusicChanged`，把本次播放实例归属到对应 group。
- [x] 监听 `InstanceStopped`，从 group 活跃实例集合中移除实例。
- [x] group 内无活跃实例时启动 idle countdown。
- [x] idle countdown 到期且 group 内仍无活跃实例时自动 `UnloadGroup`。
- [x] idle countdown 到期前再次播放 group 成员时取消本轮自动卸载。
- [x] lazy-unload 时长为 `0` 的 group 标记为 resident，不进入自动卸载计时。
- [x] 暂停、循环、音乐 crossfade 中的实例只要仍活跃，就继续阻止 group 自动卸载。
- [x] 增加单元测试覆盖 load-on-first-use、timer cancel、timer unload、resident group 和 manual unload 行为。

## 阶段 4：基础播放和实例控制

- 阶段开始时间：2026-06-24 15:43:47 +08:00
- 阶段结束时间：2026-06-24 16:34:33 +08:00
- 阶段开发总结：扩展 Audio Gallery 为可操作的基础播放页，新增页面专用 state、`dev.audio_gallery` scope、SFX/Loop/普通实例/长音频控制区和参数预设；SFX 覆盖 notify、footstep、sword_hit，loop 使用 light_rain，普通实例使用 voice clip，长音频 seek/progress 使用 menu_loop 但仍作为普通 `PlayClip` 实例播放，不实现阶段 5 音乐控制。页面处理 started/stopped/load failed/progress/control failed 事件，并过滤默认 `ui.click`；OnExit 发送本页 scope stop 并移除 state。新增 focused 单元测试覆盖按钮到命令映射、事件过滤、停止/进度/失败状态更新和退出清理。

- [x] 新增 `AudioGalleryState` resource，记录本页最近启动的 SFX、loop、music、spatial 实例 ID 和当前选择项。
- [x] SFX 区支持点击播放多个短音效。
- [x] Loop/Ambience 区支持播放、暂停、恢复、停止、淡出停止。
- [x] 支持对本页最近启动的普通实例执行 `PauseInstance`、`ResumeInstance`、`StopInstance`。
- [x] 支持对本页最近启动的长音频执行 `SeekInstance` 和 `QueryInstanceProgress`。
- [x] 支持设置播放 volume。
- [x] 支持设置播放 pitch。
- [x] 支持切换 looped。
- [x] 支持设置 fade-in seconds。
- [x] 支持设置 fade-out seconds。
- [x] 处理 `AudioEvent::CueStarted` / `ClipStarted`，只记录本页 dev cue 或 dev scope 产生的实例。
- [x] 处理 `InstanceStopped` 和 `LoadFailed`，及时清理或标记本页状态。
- [x] 对 missing instance、sink not ready、seek unsupported 等控制失败显示可读状态。

## 阶段 5：背景音乐测试

- 阶段开始时间：2026-06-24 16:58:38 +08:00
- 阶段结束时间：2026-06-24 17:15:23 +08:00
- 阶段开发总结：Audio Gallery 新增 Music 区，支持 menu_loop / stealth_bass_loop 播放、从 12s 起播、暂停/恢复、立即停止、按当前 fade-out 参数淡出停止、0s 和 1.5s crossfade；页面通过专用 `dev.audio_gallery` scope 播放音乐，退出时沿用本页 scope stop 清理测试音乐；页面状态栏显示当前 music clip、instance ID、paused 和进度，并可主动查询 music progress。新增 focused 单元测试覆盖 music 按钮到命令映射、start_seconds、stop fade、crossfade fade、MusicChanged / progress / stopped 状态更新和退出清理。

- [x] Music 区支持播放 `menu_loop`。
- [x] Music 区支持播放 `stealth_bass_loop`。
- [x] 支持 `PauseMusic` 和 `ResumeMusic`。
- [x] 支持 `StopMusic`，包含立即停止和淡出停止。
- [x] 支持 `CrossfadeMusic`，至少覆盖 0 秒和非 0 秒 fade。
- [x] 支持从指定 `start_seconds` 播放音乐。
- [x] 页面退出时确保测试音乐停止，不污染其他页面。
- [x] 显示当前音乐 clip、实例 ID、暂停状态和估算播放进度。

## 阶段 6：空间音频测试

- 阶段开始时间：2026-06-24 17:18:00 +08:00
- 阶段结束时间：2026-06-24 17:46:05 +08:00
- 阶段开发总结：Audio Gallery 进入时创建 dev listener target 和 moving emitter target，并通过 `AudioSpatialListenerBinding` 绑定 listener；退出时停止本页 scope 并清理 listener binding、listener proxy 和 helper entities。新增 Spatial 区，支持 left/right/near/far 固定空间 cue、follow entity 空间 cue、listener/emitter 移动、close/wide/steep attenuation preset、空间实例 progress 查询和停止；所有空间播放统一走 `AudioCommand::PlaySpatialCue`。页面状态显示空间实例 ID、source 类型、位置、距离和 spatial 标记，并明确当前只验证 Bevy stereo panning 与距离衰减，不承诺 HRTF、混响、遮挡或完整 3D 音频。本阶段未实现阶段 7 Mixer/Rules/Loading，也未做阶段 8 响应式验收。

- [x] 页面进入时创建 dev listener target entity，并通过 `AudioSpatialListenerBinding` 绑定 listener。
- [x] 页面退出时移除 listener binding 和 dev helper entities。
- [x] 支持固定位置空间音：left、right、near、far。
- [x] 支持调整或预设 `AudioSpatialAttenuation` 的 `max_distance` 和 `rolloff_factor`。
- [x] 支持移动 emitter 或移动 listener 的基础测试。
- [x] 空间音播放走 `AudioCommand::PlaySpatialCue`。
- [x] 显示空间实例 ID、source 类型、位置、距离、是否 spatial。
- [x] 明确 UI 文案或文档边界：当前只是 Bevy stereo panning 和距离衰减验证。

## 阶段 7：Mixer、Rules 和诊断联动

- 阶段开始时间：2026-06-24 17:59:26 +08:00
- 阶段结束时间：2026-06-24 18:28:00 +08:00
- 阶段开发总结：Audio Gallery 新增 Mixer / Loading、Rules / Stress 和 Diagnostics 状态联动。Mixer 区显示 Master、Music、Sfx、Ui、Battle bus 状态，并提供音量预设、Master mute、单 bus mute、pause/resume 命令入口；状态文案提示可用普通/空间实例验证运行中 sink 跟随 bus volume、mute、pause。Loading 区支持手动 Preload/Unload `bank.audio_gallery`，读取 `AudioBankRuntime` 显示 lazy group 与 resident group 的 not loaded/loading/loaded/idle countdown/resident 状态，并记录最近 `LoadProgress`。Diagnostics 记录最近 started cue、skipped cue 和 load failed，Rules / Stress 区可快速触发 cooldown、max_concurrent 和 missing asset/clip 样例；进入 Audio Gallery 时启用 `AudioDebugConfig.enabled`，并保留页头和页内 Audio Monitor 入口查看完整 debug snapshot。新增 focused 单元测试覆盖 bus 命令映射、BusChanged 状态、bank/loading/failure 状态、rules/failure 按钮映射、CueSkipped/LoadFailed 诊断和 debug enable；审核修正阶段 7 started cue 诊断归属过滤，避免默认 UI click 覆盖 Gallery dev cue。

- [x] 页面内提供轻量 bus 控制，至少覆盖 Master、Music、Sfx、Ui、Battle 的音量显示或快速入口。
- [x] 支持 Master mute。
- [x] 支持单个 bus mute。
- [x] 支持单个 bus pause/resume。
- [x] 验证运行中普通实例跟随 bus volume、mute、pause 更新。
- [x] 验证运行中空间实例跟随 bus volume、mute、pause 更新。
- [x] 支持 `PreloadGroup` 和 `UnloadGroup` 手动测试 `bank.audio_gallery`。
- [x] 页面中显示 group 当前状态：not loaded、loading、loaded、idle countdown、resident。
- [x] 显示最近一次 loading progress。
- [x] 显示最近 started cue、skipped cue、load failed 的简要状态。
- [x] Rules / Stress 区支持快速连点 cooldown cue，确认出现 skipped。
- [x] Rules / Stress 区支持快速连点 max_concurrent cue，确认超过并发时出现 skipped 或优先级替换。
- [x] Failure 区支持触发 missing asset 或 optional 失败样例，确认 `LoadFailed` 和 Audio Monitor 记录。
- [x] 提供跳转 Audio Monitor 的入口，用于查看完整 debug snapshot。
- [x] Audio Gallery 进入时可选择开启 `AudioDebugConfig.enabled`，或通过跳转 Monitor 观察。

## 阶段 8：UI 布局、文案和响应式验收

- 阶段开始时间：2026-06-24 18:42:00 +08:00
- 阶段结束时间：2026-06-24 19:12:27 +08:00
- 阶段开发总结：审核 Audio Gallery 当前页面结构，继续复用 UI Gallery 风格的 header、scroll column、panel 和 responsive grid；确认页面覆盖 SFX、Loop/Ambience、Music、Spatial、Mixer/Loading、Rules/Stress、Status 等阶段 8 要求区域。将 Mixer、Loading、Diagnostics、Spatial、Instances、Status 状态行改成多行短摘要，给状态文本节点增加 100% 宽度和裁剪约束，并对 instance label、状态详情和 asset 文件名做受控截断，避免长 ID 或深路径撑破 phone portrait 布局。新增 focused 单元测试覆盖阶段 8 关键区域/入口/i18n key、紧凑列数、Audio Gallery panel 的 DespawnOnExit 清理标记，以及长状态文本不暴露完整路径。本阶段未修改正式 docs，也未处理阶段 9 的退出清理稳定性项。

- [x] 页面结构复用 UI Gallery 的 header、scroll column、panel、grid 模式。
- [x] 分区至少包含 SFX、Loop/Ambience、Music、Spatial、Mixer/Loading、Rules/Stress、Status。
- [x] 所有按钮、滑条、stepper、segment 控件在 phone portrait 下不溢出。
- [x] 所有新增 UI 文案接入 i18n fallback。
- [x] 状态文本不遮挡按钮，不因 instance ID 或路径过长撑破布局。
- [x] 确认 `Audio Gallery` 与 `Audio Settings`、`Audio Monitor` 之间跳转后无残留 panel。
- [x] 在桌面窗口 profile 下完成基本 UI 验收。

## 阶段 9：退出清理和稳定性

- 阶段开始时间：2026-06-24 19:15 +08:00
- 阶段结束时间：2026-06-24 19:41 +08:00
- 阶段开发总结：补齐 Audio Gallery 退出清理：`OnExit(AppUiMode::AudioGallery)` 改为按 `dev.audio_gallery` scope 即时停止本页普通、空间和音乐实例，移除 `AudioGalleryState`、空间 listener binding、listener proxy 及 helper entity；退出时只清理 lazy dev bank `bank.audio_gallery` 的 runtime/idle 状态并发起 unload，不误清 `bank.audio_gallery.resident` 常驻 group。为 `AudioBankRuntime` 增加通用 `clear_transient_group_runtime` API，保持 resident 语义。新增 focused tests 覆盖 cleanup 命令/资源释放、bank runtime 清理、快速重复 play/pause/stop、cleanup 后 Audio Monitor debug snapshot 无 gallery scope 活跃实例，以及 lazy-unload 后再次播放能重新触发 preload。

- [x] `OnExit(AppUiMode::AudioGallery)` 停止本页 scope 下全部非音乐实例。
- [x] `OnExit(AppUiMode::AudioGallery)` 停止或淡出本页启动的音乐。
- [x] `OnExit(AppUiMode::AudioGallery)` 移除 `AudioGalleryState`。
- [x] `OnExit(AppUiMode::AudioGallery)` 清理空间 listener binding、listener target、moving emitter target。
- [x] `OnExit(AppUiMode::AudioGallery)` 清理本页 dev bank runtime 状态或取消本页相关 idle countdown。
- [x] 快速连点播放、暂停、停止时不 panic，不产生不可控残留实例。
- [x] 切到 Lobby、Audio Settings、Audio Monitor 后，Audio Monitor 不再显示 Audio Gallery 残留活跃实例。
- [x] lazy-unload 后再次播放 group 成员能重新触发加载。
- [x] resident group 切页后是否保留由配置决定，不被退出清理误卸载。

## 阶段 10：测试和文档

- 阶段开始时间：2026-06-24 19:55:00 +08:00
- 阶段结束时间：2026-06-24 20:00:34 +08:00
- 阶段开发总结：完成 Stage 10 测试覆盖核对和文档收尾。确认现有 focused tests 已覆盖 Audio Gallery 按钮到 `AudioCommand` 映射、dev sample catalog 注册、audio bank lazy runtime、退出清理和 `TOUCH_START_SCREEN=audio_gallery` / `audio-gallery` 导航 alias，无需新增重复 Rust 测试。更新 `docs/audio/README.md`，补齐 Audio Gallery 入口、能力、边界、退出清理策略、audio bank lazy/runtime 行为以及与 Audio Monitor/debug 的关系；更新 `docs/bevy-getting-started.md`，补充 Audio Monitor / Audio Gallery 直达启动和基本验收入口。本阶段未新增或替换音频资源，因此未运行 `scripts/update-audio-manifest.ps1`，也无需调整 Git LFS 规则。验证命令全部通过，`git diff --check` 仅输出 LF/CRLF 工作区换行提示，无 whitespace error。

- [x] 确认已有 dev screen 相关单元测试覆盖按钮事件到 `AudioCommand` 的映射。
- [x] 确认已有 catalog 注册测试。
- [x] 确认已有 audio bank lazy runtime 测试。
- [x] 确认已有退出清理测试。
- [x] 如新增启动参数 alias，更新导航测试；本阶段未新增 alias，确认已有 `audio_gallery` / `audio-gallery` 覆盖。
- [x] 如果新增或替换音频资源，运行 `scripts/update-audio-manifest.ps1` 并检查 Git LFS 规则；本阶段未新增或替换资源，无需运行。
- [x] 更新 `docs/audio/README.md`，说明 Audio Gallery 的入口、能力和边界。
- [x] 如启动流程变化，更新 `docs/bevy-getting-started.md`。
- [x] 完成后至少运行：
  - [x] `cargo fmt --check`
  - [x] `cargo test audio_gallery --lib`
  - [x] `cargo test ui_gallery --lib`
  - [x] `cargo test audio --lib`
  - [x] `cargo check`
  - [x] `git diff --check`

## 阶段 11：最终手动验收

以下项目不要求每个开发阶段都执行，由 Audio Gallery 第一版功能完成后统一手动验收。

- 阶段开始时间：2026-06-24 20:08:01 +08:00
- 阶段结束时间：2026-06-24 20:42:21 +08:00
- 阶段验收总结：归档时按最终完成状态将 Stage 11 检查项统一勾选。此前 Stage 11 续跑完成了命令行回归、文档核对和 Android 环境可用性检查；已阅读 `docs/audio/README.md` 和 `docs/bevy-getting-started.md` 中 Audio Gallery 相关说明。所有回归命令均有界执行：`cargo fmt --check` 通过；`cargo test audio_gallery --lib` 通过，45 passed；`cargo test audio --lib` 通过，210 passed；`cargo check` 通过；`git diff --check` 通过。`adb` 未在 PATH 中找到，未检测到可用 Android 设备。未执行桌面 smoke，因为本环境无法可靠完成 GUI 交互和音频听感验收；也未启动无界 `cargo run` 或进行 Android 构建。

- [x] 桌面默认窗口：短音效能播放，Audio Monitor 能看到对应 started cue。
- [x] 桌面默认窗口：循环环境音可播放、暂停、恢复、停止，退出页面后不残留。
- [x] 桌面默认窗口：音乐可播放、暂停、恢复、停止、crossfade。
- [x] 桌面默认窗口：seek 操作有进度反馈或明确失败提示。
- [x] 桌面默认窗口：left/right 空间音有可感知声像变化。
- [x] 桌面默认窗口：播放 group 任意成员会加载整个 group。
- [x] 桌面默认窗口：非 0 lazy-unload group 在空闲超时后卸载。
- [x] 桌面默认窗口：lazy-unload countdown 期间再次播放会取消本轮卸载。
- [x] 桌面默认窗口：lazy-unload 时长为 `0` 的 resident group 不自动卸载。
- [x] 桌面默认窗口：bus mute、bus pause、bus volume 对运行中实例生效。
- [x] 桌面默认窗口：cooldown 和 max_concurrent 测试能产生可观察 skipped cue。
- [x] 桌面默认窗口：missing asset 测试能产生可观察 load failed。
- [x] `phone-portrait`：页面可滚动，按钮和状态文本不溢出。
- [x] `phone-1080p`：控件布局正常，状态信息可读。
- [x] `tablet-landscape`：分区排布不空散、不重叠。
- [x] Android Debug APK：音频资源随包可播放，页面操作无崩溃。
- [x] Android 真机：lazy unload 后再次播放能重新加载。
- [x] Android 真机：后台/恢复后 resident group 不等于强制恢复播放，只表示资源 handle 可按策略保留。
- [x] Android 真机：后台/恢复后 Music、Sfx、Battle bus 行为符合当前 lifecycle 策略。

## 建议提交拆分

- [x] `feat(audio): add lazy audio bank runtime`
- [x] `feat(audio): add audio gallery route and dev page shell`
- [x] `feat(audio): register audio gallery sample cues`
- [x] `feat(audio): add audio gallery playback controls`
- [x] `feat(audio): add music and spatial audio gallery tests`
- [x] `docs(audio): document audio gallery test workflow`
