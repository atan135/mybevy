# MyBevy 音频框架开发清单

本文记录 MyBevy 音频框架的开发任务清单。目标是基于 `docs/audio/README.md` 补齐 `framework/audio/` 横向能力，让 UI、场景、战斗和后续下载内容都能通过统一命令流播放、管理和调试音频。

## 设计结论

- 新增音频框架建议位于 `project/src/framework/audio/`，与 `ui`、`scene`、`network`、`fight` 同级，不挂到 UI、场景或战斗模块内部。
- 游戏层只发送播放意图，例如 `AudioCommand::PlayCue`、`PlayMusic`、`StopByScope`，不直接操作底层 Bevy 音频实体。
- 第一版使用 `AudioClipId` 表示具体音频资源，使用 `AudioCueId` 表示播放语义；业务优先播放 cue，而不是到处写资源路径。
- 音量总线第一版至少支持 `Master`、`Music`、`Sfx`、`Ui`，后续扩展 `Ambience`、`Battle`、`Voice`。
- 播放实例必须带生命周期归属，建议支持 `Global`、`Ui`、`Scene(session_id)`、`Entity(entity)`、`Battle(battle_id)` 等 scope。
- UI 音效通过 UI 事件 adapter 接入，场景音频通过 `SceneEvent` 和 scene scope 接入，战斗音效通过战斗事件或 cue adapter 接入。
- 运行中调整音量时不能只依赖 Bevy 全局音量，应追踪活跃实例并同步到 `AudioSink` 或空间音频 sink。
- 短 UI 音效和关键反馈音可进首包；背景音乐、活动语音和大体积音频优先后续下载。
- 高频 UI 和战斗音效必须有冷却、最大并发或优先级策略，避免同一帧无限创建播放实例。
- 空间音频、后续下载 catalog、调试面板和复杂战斗音频可分阶段落地，第一版先做最小播放闭环。

## 数据和模块约定

- 设计文档：`docs/audio/README.md`
- 框架模块目录：`project/src/framework/audio/`
- 框架导出入口：`project/src/framework/mod.rs`、`project/src/framework/prelude.rs`
- 首包音频目录：`project/assets/audio/`
- 建议首包子目录：`audio/ui/`、`audio/music/`、`audio/ambience/`、`audio/battle/`、`audio/voice/`、`audio/common/`
- 后续下载音频路径：`content_cache://<version>/audio/...`
- 建议核心类型：`AudioPlugin`、`AudioCommand`、`AudioEvent`、`AudioClipId`、`AudioCueId`、`AudioGroupId`、`AudioInstanceId`、`AudioBus`、`AudioScope`
- 建议核心资源：`AudioCatalog`、`AudioMixer`、`AudioPlaybackState`、`MusicController`、`AudioDebugConfig`
- 建议模块：`plugin.rs`、`id.rs`、`command.rs`、`event.rs`、`catalog.rs`、`loading.rs`、`playback.rs`、`mixer.rs`、`music.rs`、`spatial.rs`、`scope.rs`、`debug.rs`、`prelude.rs`

## 音频运行模型

1. `AudioPlugin` 注册命令、事件、资源和系统。
2. UI、场景、战斗或 game layer 发送 `AudioCommand`。
3. `AudioCatalog` 将 `AudioCueId` 解析为 clip、bus、scope 和播放规则。
4. 加载系统确认音频资源 handle 可用，必要时记录加载状态或失败。
5. 播放系统生成 Bevy 音频播放实体，并记录 `AudioInstanceId`、bus、scope 和 sink 关联。
6. mixer 系统根据 bus 音量、静音和暂停状态同步活跃实例。
7. music 系统处理背景音乐状态、淡入淡出和交叉淡入淡出。
8. scope 清理系统响应 UI、场景、战斗生命周期，停止或淡出对应音频。
9. debug 系统收集活跃实例、bus 状态、最近 cue 和失败资源。

## 验收标准

- 能通过 `AudioCommand::PlayCue` 播放一个首包 UI 音效。
- 能通过 `AudioCommand::PlayMusic` 播放、停止和切换一首背景音乐。
- `Master`、`Music`、`Sfx`、`Ui` bus 的音量和静音能影响当前播放实例。
- UI 按钮点击能通过 adapter 播放默认点击音，且连点不会无限堆叠。
- 场景进入后可以播放 scene scope 的环境音，场景退出后无残留音频。
- 背景音乐支持基础淡入、淡出和交叉淡入淡出。
- 战斗或高频 cue 支持最大并发、冷却或优先级，避免同一帧创建无限实例。
- 资源缺失、cue 缺失或播放失败时记录可读错误，不导致主流程崩溃。
- Android 资源路径大小写正确，首包音频路径遵守 `project/assets/` 相对路径规则。
- `cargo fmt --check`、`cargo check`、`cargo test`、`git diff --check` 通过。

## 开发和派发规则

- 任务串行执行，一次只派发一个二级任务，避免并发修改同一框架边界。
- 每个二级任务派发后填写“开始时间”。
- 每个二级任务通过主 agent 审核和验收后填写“结束时间”。
- 审核不通过时保留该任务未完成状态，打回修复，不进入下一个二级任务。
- 涉及 Rust 代码的任务完成后至少执行相关单测或 `cargo check`；阶段验收任务执行完整检查。
- 不把尚未落地的音频能力写成已实现能力。
- 新增首包二进制音频资源时，确认命中 Git LFS 规则。

## 二级任务清单

- [x] 1. 确认 Bevy 0.18.1 音频 API 边界
  - 开始时间：2026-06-18 17:05:26 +08:00
  - 结束时间：2026-06-18 17:10:42 +08:00
  - [x] 确认 `AudioPlayer`、`PlaybackSettings`、`AudioSink`、空间音频 sink 的当前用法。
  - [x] 确认运行中修改音量、暂停、恢复和停止实例的可行方式。
  - [x] 确认循环播放、播放结束检测和实体清理策略。
  - [x] 确认 Android 目标平台支持的音频格式和 Bevy feature 需求。
  - [x] 输出第一版底层 API 使用结论，作为实现依据。

  API 边界结论：
  - 当前项目 `project/Cargo.toml` 使用 `bevy = "0.18.1"` 且未关闭默认 features；Bevy 默认 feature 集包含 audio 和 vorbis，因此当前首选可验证格式是 OGG/Vorbis。
  - Bevy 0.18.1 通过给实体插入 `AudioPlayer::new(handle)` 和 `PlaybackSettings` 播放音频；音频资源加载完成后，Bevy 在 `PostUpdate` 自动为实体插入 `AudioSink` 或 `SpatialAudioSink`。
  - `PlaybackSettings` 只作为启动参数；运行中修改 `PlaybackSettings`、`GlobalVolume` 不会影响已经播放的实例。后续框架必须追踪活跃实例，并通过 `AudioSinkPlayback` 同步运行中音量、静音、暂停、恢复、停止和变速。
  - 非空间音频运行中控制使用 `AudioSink`；空间音频使用 `SpatialAudioSink`。两者都实现 `AudioSinkPlayback`，支持 `set_volume`、`mute`、`unmute`、`pause`、`play`、`stop`、`empty`、`position`、`try_seek`、`set_speed`。
  - 停止实例可调用 sink 的 `stop()`；停止后不能恢复。暂停和恢复分别使用 `pause()`、`play()`。停止或自然播放结束后的框架状态清理需要由实例追踪系统或 Bevy 自动清理模式配合完成。
  - 循环播放使用 `PlaybackSettings::LOOP` 或 `PlaybackMode::Loop`；一次性音效可用 `PlaybackSettings::DESPAWN` 在播放结束后 despawn 实体，或用 `PlaybackSettings::REMOVE` 在结束后移除音频组件。若使用 `ONCE`，Bevy 不会自动清理实体，框架需通过 `sink.empty()` 检测结束并清理。
  - `AudioPlayer` 指向尚未加载完成的资源时不会立即播放；资源可用后才开始播放并插入 sink。因此 `AudioInstanceId` 到 sink 的绑定需要允许“已创建播放实体但 sink 尚未出现”的 pending 状态。
  - 空间音频通过 `PlaybackSettings::with_spatial(true)` 启用，需要音源实体有 `GlobalTransform`；监听器使用 `SpatialListener`，同一时间只应有一个。Bevy 0.18.1 空间音频是简单左右声道 panning，不提供 HRTF、混响或遮挡。
  - Android 构建沿用 Bevy/rodio/cpal 输出链路；默认 feature 中包含 `android_shared_stdcxx`，项目当前无需额外开启即可走默认 Android 支持。若以后关闭默认 features，需要显式保留 `bevy_audio`、`vorbis` 和 Android 相关 feature。
  - Bevy 音频 loader 的可选格式由 features 决定：默认 OGG/Vorbis；`.mp3` 需 `bevy/mp3`，`.flac` 需 `bevy/flac`，`.wav` 需 `bevy/wav`，也可走对应 `symphonia-*` features。第一版首包资源建议统一 OGG/Vorbis，减少 Android 包体和 feature 风险。

- [x] 2. 建立 `framework/audio/` 模块骨架
  - 开始时间：2026-06-18 17:12:09 +08:00
  - 结束时间：2026-06-18 17:15:10 +08:00
  - [x] 新增 `project/src/framework/audio/` 目录。
  - [x] 新增 `mod.rs`、`prelude.rs`、`plugin.rs`。
  - [x] 在 `project/src/framework/mod.rs` 暴露 audio 模块。
  - [x] 按需在 `project/src/framework/prelude.rs` 导出常用 audio 类型。
  - [x] 确认新增模块不依赖 game layer。

- [x] 3. 定义音频 ID、bus 和 scope 类型
  - 开始时间：2026-06-18 17:16:00 +08:00
  - 结束时间：2026-06-18 17:33:55 +08:00
  - [x] 定义 `AudioClipId`。
  - [x] 定义 `AudioCueId`。
  - [x] 定义 `AudioGroupId`。
  - [x] 定义 `AudioInstanceId`。
  - [x] 定义 `AudioBus`，至少包含 `Master`、`Music`、`Sfx`、`Ui`。
  - [x] 定义 `AudioScope`，至少包含 `Global`、`Ui`、`Scene`、`Entity`、`Battle`。
  - [x] 为 ID 类型补充基础校验、Display、From<&str> 或等价构造能力。

- [x] 4. 定义音频命令和事件
  - 开始时间：2026-06-18 17:35:11 +08:00
  - 结束时间：2026-06-18 17:43:03 +08:00
  - [x] 定义 `AudioCommand::PlayCue`。
  - [x] 定义 `AudioCommand::PlayClip`。
  - [x] 定义 `AudioCommand::PlayMusic` 和 `CrossfadeMusic`。
  - [x] 定义 `StopInstance`、`StopByScope`、`PauseByScope`、`ResumeByScope`。
  - [x] 定义 `SetBusVolume`、`SetBusMuted`、`SetBusPaused`。
  - [x] 定义 `PreloadGroup`、`UnloadGroup`。
  - [x] 定义播放、跳过、停止、加载失败、音乐切换和 bus 变化事件。

- [x] 5. 接入 `AudioPlugin`
  - 开始时间：2026-06-18 17:44:08 +08:00
  - 结束时间：2026-06-18 17:51:36 +08:00
  - [x] 在 `AudioPlugin` 中注册 `AudioCommand`。
  - [x] 在 `AudioPlugin` 中注册 `AudioEvent`。
  - [x] 初始化 catalog、mixer、playback、music 和 debug 资源。
  - [x] 配置 audio 相关 system set，保证命令处理、sink 同步和清理顺序稳定。
  - [x] 在 `GamePlugin` 或上层插件中接入 `AudioPlugin`。

- [x] 6. 实现内存版 `AudioCatalog`
  - 开始时间：2026-06-18 17:52:57 +08:00
  - 结束时间：2026-06-18 18:08:29 +08:00
  - [x] 支持注册 clip ID 到资源路径的映射。
  - [x] 支持注册 cue ID 到 clip 或 clip 列表的映射。
  - [x] cue 支持默认 bus、scope、音量、音高和循环标记。
  - [x] cue 支持冷却、最大并发和优先级字段的结构定义。
  - [x] 提供 cue 和 clip 缺失时的明确错误。
  - [x] 编写 catalog 基础单元测试。

- [x] 7. 新增基础首包音频资源和资源检查
  - 开始时间：2026-06-18 18:09:28 +08:00
  - 结束时间：2026-06-18 18:15:16 +08:00
  - [x] 新增 `project/assets/audio/` 子目录结构。
  - [x] 放入最小 UI 点击音和测试音乐资源，或使用明确占位资源策略。
  - [x] 确认资源命名使用小写英文、数字、下划线或短横线。
  - [x] 使用 `git check-attr filter -- <path>` 确认二进制音频命中 Git LFS。
  - [x] 记录资源授权来源或确认只使用可发布资源。

- [x] 8. 实现普通音效播放
  - 开始时间：2026-06-18 18:16:21 +08:00
  - 结束时间：2026-06-18 18:43:42 +08:00
  - [x] 处理 `AudioCommand::PlayCue`。
  - [x] 处理 `AudioCommand::PlayClip`。
  - [x] 根据 catalog 解析资源路径并加载 `AudioSource` handle。
  - [x] 生成播放实体并记录 `AudioInstanceId`、bus、scope 和 cue 信息。
  - [x] 支持短音效自然播放结束后的实例清理。
  - [x] 播放失败时发送 `AudioEvent::LoadFailed` 或等价错误事件。

- [x] 9. 实现 mixer 和运行中音量同步
  - 开始时间：2026-06-18 18:44:46 +08:00
  - 结束时间：2026-06-18 19:13:53 +08:00
  - [x] 保存 `Master`、`Music`、`Sfx`、`Ui` bus 音量。
  - [x] 支持 bus 静音和暂停状态。
  - [x] 处理 `SetBusVolume`、`SetBusMuted`、`SetBusPaused`。
  - [x] 将 bus 变化同步到已存在的 `AudioSink`。
  - [x] 验证运行中修改 Music 或 Ui bus 能影响当前播放实例。
  - [x] 为音量 clamp 和 muted 计算增加单元测试。

- [x] 10. 实现背景音乐控制器
  - 开始时间：2026-06-18 19:15:05 +08:00
  - 结束时间：2026-06-18 19:40:26 +08:00
  - [x] 定义 `MusicController` 当前状态。
  - [x] 支持 `PlayMusic`。
  - [x] 支持停止当前音乐。
  - [x] 支持暂停和恢复当前音乐。
  - [x] 支持淡入和淡出。
  - [x] 支持 `CrossfadeMusic`。
  - [x] 确认切音乐后旧音乐实体能清理。

- [x] 11. 接入 UI 音效 adapter
  - 开始时间：2026-06-18 19:41:32 +08:00
  - 结束时间：2026-06-18 19:59:15 +08:00
  - [x] 消费 `UiButtonEvent` 并播放默认点击 cue。
  - [x] 支持按钮或 action 覆盖 cue 的扩展点。
  - [x] 为 UI cue 增加短冷却，避免连点无限叠音。
  - [x] 确认 UI 音效默认走 `Ui` bus。
  - [x] 确认静音 `Ui` bus 后按钮点击不再发声。

- [x] 12. 接入场景音频生命周期
  - 开始时间：2026-06-18 20:00:31 +08:00
  - 结束时间：2026-06-18 20:36:39 +08:00
  - [x] 监听 `SceneEvent::Entered`，为目标场景播放基础音乐或环境音。
  - [x] 监听 `SceneEvent::Exited` 或退出开始事件，停止或淡出 `Scene(session_id)` scope 音频。
  - [x] 确认重复进入和退出场景不会残留音频实例。
  - [x] 为样板场景准备最小 ambience 或测试 cue。
  - [x] 不在 scene framework 中硬编码具体业务音乐规则。

- [x] 13. 实现空间音频最小能力
  - 开始时间：2026-06-18 20:37:42 +08:00
  - 结束时间：2026-06-18 21:16:16 +08:00
  - [x] 定义 listener 绑定方式，优先支持相机或玩家 Transform。
  - [x] 定义 `PlaySpatialCue` 或等价命令。
  - [x] 支持空间音源跟随实体或固定 Transform。
  - [x] 支持最大距离和基础衰减参数。
  - [x] 场景退出或实体销毁后清理空间音频实例。
  - [x] 记录 Bevy 空间音频能力限制，不承诺复杂 HRTF 或混响。

- [x] 14. 实现战斗音效播放策略
  - 开始时间：2026-06-18 21:17:27 +08:00
  - 结束时间：2026-06-18 21:47:51 +08:00
  - [x] 定义 battle cue 的基础映射和 bus。
  - [x] 支持 cue 最大并发。
  - [x] 支持 cue 冷却。
  - [x] 支持 cue 优先级或跳过策略。
  - [x] 支持随机变体。
  - [x] 支持 `Battle(battle_id)` scope 清理。
  - [x] 编写高频 cue 不无限创建实例的测试。

- [ ] 15. 实现加载组和预加载接口
  - 开始时间：
  - 结束时间：
  - [ ] 支持 `AudioCommand::PreloadGroup`。
  - [ ] 支持 `AudioCommand::UnloadGroup`。
  - [ ] 记录 group 中 clip 加载进度。
  - [ ] 发送 `AudioEvent::LoadProgress`。
  - [ ] 资源缺失时记录 clip ID、路径和 group。
  - [ ] 确认 optional 音频失败不阻塞主流程。

- [ ] 16. 支持 catalog 外部配置预留
  - 开始时间：
  - 结束时间：
  - [ ] 设计 RON 或 JSON catalog 数据结构。
  - [ ] 支持从首包资源读取 catalog 的解析函数。
  - [ ] 保留 `content_cache://...` 路径字段兼容。
  - [ ] 校验路径不能包含绝对路径、反斜杠、盘符或 `..`。
  - [ ] 解析失败时保留内置 fallback catalog。
  - [ ] 编写 catalog 配置解析测试。

- [ ] 17. 增加调试和诊断能力
  - 开始时间：
  - 结束时间：
  - [ ] 定义 `AudioDebugConfig`。
  - [ ] 支持 `MYBEVY_AUDIO_DEBUG`。
  - [ ] 支持记录最近播放 cue 和最近跳过 cue。
  - [ ] 支持统计活跃实例数量和按 bus 分组数量。
  - [ ] 支持记录最近加载失败资源。
  - [ ] 预留 UI debug 面板读取的 snapshot 结构。

- [ ] 18. 移动端和后台行为检查
  - 开始时间：
  - 结束时间：
  - [ ] 确认 Android APK 能读取 `project/assets/audio/` 内音频。
  - [ ] 确认音频格式在 Android Debug 包中可播放。
  - [ ] 确认移动端窗口配置或触控流程下 UI 音效可用。
  - [ ] 评估进入后台、息屏或应用暂停时的 bus 暂停策略。
  - [ ] 确认同屏音频实例数量有上限或可诊断。

- [ ] 19. 测试覆盖
  - 开始时间：
  - 结束时间：
  - [ ] 测试 ID 校验和 Display 行为。
  - [ ] 测试 catalog cue 到 clip 解析。
  - [ ] 测试 bus 音量、静音和暂停计算。
  - [ ] 测试 scope 清理筛选。
  - [ ] 测试 cue 冷却和最大并发。
  - [ ] 测试音乐状态切换和淡入淡出状态推进。
  - [ ] 测试资源路径安全校验。

- [ ] 20. 文档维护
  - 开始时间：
  - 结束时间：
  - [ ] 更新 `docs/audio/README.md`，把已落地能力和未实现能力区分清楚。
  - [ ] 如新增资源目录或格式约定，检查 `docs/assets-workflow.md` 是否需要同步。
  - [ ] 如新增启动或调试环境变量，检查 `docs/bevy-getting-started.md` 是否需要同步。
  - [ ] 如 audio 和 scene manifest 对接，检查 `docs/scene/` 是否需要补充音频边界。
  - [ ] 不把后续下载、空间音频或战斗音频高级能力写成已完成，除非代码已经落地。

- [ ] 21. 阶段验收
  - 开始时间：
  - 结束时间：
  - [ ] 执行 `cargo fmt --check`。
  - [ ] 执行 `cargo check`。
  - [ ] 执行 `cargo test`。
  - [ ] 执行 `git diff --check`。
  - [ ] 运行桌面客户端，验证 UI 点击音和音乐播放。
  - [ ] 进入并退出样板场景，验证场景音频无残留。
  - [ ] 修改 bus 音量，验证当前播放实例同步变化。

- [ ] 22. 最终验收和提交
  - 开始时间：
  - 结束时间：
  - [ ] 主 agent 做代码审核，重点检查 framework/game 边界、scope 清理、运行中音量同步和测试覆盖。
  - [ ] 完整执行 `cargo fmt --check`、`cargo check`、`cargo test`、`git diff --check`。
  - [ ] 确认 checklist 所有二级任务有开始时间和结束时间。
  - [ ] 使用 `$mygit-skill` 检查改动拆分和提交信息。
  - [ ] 提交通过验收的代码、资源和文档。
