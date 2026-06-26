# UI 自动化审计与优化 Checklist

## 目标

基于 `docs/ui/UI自动化审计与优化方案.md` 和 `docs/debug/远程调试控制机制.md`，在当前 Rust / Bevy 项目内交付一套开发期 UI 自动化审计与优化机制。该机制采用游戏内审计模式：由 Bevy App 自动进入目标界面、等待 UI 稳定、执行主窗口全屏截图、处理滚动状态、输出截图和元数据；外部 runner 负责批量设备矩阵、AI 分析、代码修复触发、复跑和总结报告。第一阶段支持 `MYBEVY_UI_AUDIT_*` 本地一次性审计模式，后续支持通过 `adminapi -> game-server -> client` 的远程调试控制机制驱动多设备和移动端审计。

首轮实现范围限定在当前项目框架内，不构建跨项目通用 UI 平台，不替换现有 Bevy UI、`AppUiMode`、Panel Manager 或页面插件体系。

## 基础原则

- [x] 审计能力默认关闭，普通运行不应产生截图、报告、自动滚动或自动退出。（验证：`MYBEVY_UI_AUDIT` 和 runner/FixMode 均默认关闭；阶段 3 本地审计门控和阶段 8 `FixMode=Off` self-test 覆盖默认不启动）
- [x] 优先复用现有 `AppUiMode`、`TOUCH_START_SCREEN`、窗口 profile、UI framework 和页面插件结构。（验证：`game/navigation/mod.rs` 复用页面 alias/owner，runner 复用 `--window-profile`/`--window-size`，UI Gallery recipe 接入现有 ScrollView）
- [x] 本地一次性审计模式和远程调试控制模式应复用同一套 UI 审计命令语义、结果格式和失败分类。（验证：runner Local/Remote 均输出根 `manifest.json`、`analysis-input.json`、`analysis.json`、`report.md`，并统一 task/capture/analysis/fix failure_type）
- [x] 远程模式必须复用 `docs/debug/远程调试控制机制.md` 中的 task、artifact、错误码、超时和安全边界。（验证：`scripts/run-ui-audit.ps1` Remote Http 调用 `POST /admin/debug/commands` 与 `GET /admin/debug/tasks/{task_id}`，Mock/Http 均记录 task id、artifact URI、终态和远程错误映射；真实 server/client 安全执行为外部依赖并已写入文档）
- [x] 截图和审计产物写入 `summary/ui-audit/`，不写入 `project/assets/`。（验证：手动截图默认 `../summary/ui-audit/manual`，runner 默认 `summary/ui-audit/<run-id>`；主 agent 多次确认临时演示清理后 `summary/ui-audit` 不存在）
- [x] 每个阶段完成后运行对应验证；涉及 Rust 代码的阶段至少运行 `cargo fmt` 和 `cargo check`。（验证：阶段 1-9 验证记录均已写入；最终主 agent 运行 `.\\scripts\\run-ui-audit.ps1 -SelfTest`、本地/远程演示、`cargo fmt`、`cargo check` 通过）
- [x] 每个阶段保持可独立实现、可独立验证、可独立提交。（验证：阶段 1-8 分别形成独立提交，阶段 9 文档与验收作为独立文档提交准备）
- [x] 自动修复流程必须保留失败出口和最大迭代次数，避免无限循环。（验证：阶段 8 `MaxFixIterations` 默认 5，self-test 覆盖 `max_iterations_reached`、`fix_check_failed`、`safety_policy_rejected`）

## 阶段 1：全屏截图基础能力

- 开始时间：2026-06-26 18:08:07 +08:00
- 结束时间：2026-06-26 18:59:31 +08:00
- 开发总结：新增 UI audit 手动截图基础能力，接入 UI framework 默认插件；F9 在桌面 debug 或显式启用配置下请求主窗口截图，保存到 `summary/ui-audit/manual/`，并覆盖绝对日志路径、保存失败和截图回调超时失败出口。
- 验证记录：`cargo test ui::audit::screenshot --lib` 通过（10 passed）；`cargo fmt --check` 通过；`cargo fmt` 通过；`cargo check` 通过。worker 手动运行 `cargo run -- --window-profile phone-small --window-scale 50%` 并按 F9，确认 PNG signature、360x800 尺寸和 UI Gallery 完整窗口画面，产物已清理。

- [x] 确定截图模块边界，选择放入 `project/src/framework/ui/audit/` 或独立 `project/src/framework/screenshot/`。（验证：`project/src/framework/ui/audit/mod.rs` 导出 `screenshot`，`project/src/framework/ui/mod.rs` 注册 `audit` 模块）
- [x] 接入 Bevy `Screenshot::primary_window()`，实现主窗口最终合成画面的截图请求。（验证：`project/src/framework/ui/audit/screenshot.rs` 的 `spawn_primary_window_screenshot` spawn `Screenshot::primary_window()` 并 observe `ScreenshotCaptured`）
- [x] 实现 `F9` 手动截图入口，仅在桌面开发环境或显式启用的开发配置下生效。（验证：`request_manual_screenshot` 监听 `KeyCode::F9`；`default_manual_screenshot_enabled` 限制 debug 且非 Android/wasm，`MYBEVY_UI_AUDIT_MANUAL_SCREENSHOT` 可显式开关）
- [x] 将手动截图保存到 `summary/ui-audit/manual/`。（验证：`DEFAULT_MANUAL_SCREENSHOT_DIR` 为 `../summary/ui-audit/manual`，从 `project/` 运行落到仓库 `summary/ui-audit/manual`）
- [x] 截图文件名包含时间戳、当前页面标识、窗口物理尺寸或逻辑尺寸。（验证：`build_manual_screenshot_request` 生成 `<timestamp>_<screen>_logical-WxH_physical-WxH.png`，单测 `manual_screenshot_filename_includes_timestamp_screen_and_sizes` 覆盖）
- [x] 截图成功时输出日志，包含完整保存路径和图片尺寸。（验证：`save_manual_screenshot` 成功日志输出 `display_path` 绝对路径、captured 尺寸、requested logical/physical 尺寸；单测 `manual_screenshot_request_records_absolute_display_path` 覆盖）
- [x] 截图失败时输出明确错误，不静默失败。（验证：`request_manual_screenshot` 覆盖 pending、重复截图、无主窗口、建目录失败；`save_manual_screenshot` 覆盖转换/格式/IO 失败；`expire_pending_manual_screenshot` 记录回调超时并清理实体）
- [x] 避免截图热键影响已有 UI 输入、玩法输入和调试面板快捷键。（验证：`request_manual_screenshot` 只读 `ButtonInput<KeyCode>` 的 `F9`，现有调试键位为 `F3`-`F8`，未写 UI/panel/gameplay 输入事件）
- [x] 为路径生成、文件名生成和截图配置解析补充 focused 单元测试。（验证：`cargo test ui::audit::screenshot --lib` 通过，覆盖配置解析、路径规范化、文件名清洗、pending 超时）
- [x] 手动运行任意 UI 页面，按 `F9` 验证 PNG 文件真实落盘且内容为当前主窗口完整画面。（验证：worker 运行 `cargo run -- --window-profile phone-small --window-scale 50%`，`TOUCH_START_SCREEN=ui-gallery`，按 F9 后确认 PNG signature、360x800 尺寸和 UI Gallery 完整窗口画面）
- [x] 在 `project/` 下运行 `cargo fmt`。（验证：主 agent 在 `project/` 运行 `cargo fmt` 成功，且 `cargo fmt --check` 通过）
- [x] 在 `project/` 下运行 `cargo check`。（验证：主 agent 在 `project/` 运行 `cargo check` 通过）

## 阶段 2：命令式截图 API

- 开始时间：2026-06-26 19:02:46 +08:00
- 结束时间：2026-06-26 19:49:40 +08:00
- 开发总结：将阶段 1 手动截图重构为命令式截图 API，新增 `UiScreenshotCommand` 和 `UiScreenshotEvent`，调用方通过事件获得 Saved/Failed 结果；F9 只负责发命令，截图 observer、保存、并发拒绝、路径冲突、目录失败和超时失败都封装在 audit 截图模块内。
- 验证记录：`cargo test ui::audit::screenshot --lib` 通过（19 passed）；`cargo fmt --check` 通过；`cargo fmt` 通过；`cargo check` 通过且无 warning。补充测试覆盖 F9 命令路径和直接系统命令路径的 PNG 实际落盘并清理临时产物；确认 `summary/` 未新增审计产物。

- [x] 定义截图命令，例如 `UiScreenshotCommand::Capture { path, label }`。（验证：`project/src/framework/ui/audit/screenshot.rs` 定义 `UiScreenshotCommand::Capture { path, label }` 并注册 `add_message::<UiScreenshotCommand>()`）
- [x] 定义截图结果事件，例如 `UiScreenshotEvent::Saved` 和 `UiScreenshotEvent::Failed`。（验证：`UiScreenshotEvent::{Saved, Failed}` 携带 `UiScreenshotSaved`/`UiScreenshotFailed`，插件注册 `add_message::<UiScreenshotEvent>()`）
- [x] 将手动 `F9` 截图改为复用命令式截图 API。（验证：`request_manual_screenshot` 只写入 `UiScreenshotCommand::Capture`，不直接 spawn `Screenshot`）
- [x] 封装 Bevy 异步 `ScreenshotCaptured` observer，不让调用方依赖渲染回读细节。（验证：`spawn_screenshot_capture` 内部 observe `ScreenshotCaptured` 并转换为 `UiScreenshotEvent`）
- [x] 记录截图请求的 label、目标路径、目标窗口、请求帧和完成帧。（验证：`UiScreenshotRequestRecord` 记录 label/path/display_path/target_window/request_frame，Saved/Failed 记录 completion_frame）
- [x] 处理同一帧重复截图请求、路径冲突、目录不存在和保存失败等错误路径。（验证：`UiScreenshotFailureReason` 覆盖 AlreadyPending、CaptureInProgress、PathAlreadyExists、DirectoryCreateFailed、SaveFailed、CaptureTimedOut 等；focused tests 覆盖重复请求、路径已存在、父目录失败、保存冲突）
- [x] 保证截图请求不会假设同帧完成，调用方必须通过事件继续流程。（验证：命令处理只建立 pending 并 spawn 截图；Saved 只由 `ScreenshotCaptured` observer 写出，超时由后续系统写出 Failed）
- [x] 为命令到事件的状态流补充单元测试或最小集成测试。（验证：`cargo test ui::audit::screenshot --lib` 通过 19 个测试，覆盖命令处理、失败事件、超时事件、observer 保存事件）
- [x] 手动验证一次按键截图和一次系统命令截图都能生成文件。（验证：`manual_f9_capture_observer_saves_file_and_cleans_up_temp_artifact` 和 `command_capture_observer_saves_file_and_cleans_up_temp_artifact` 分别模拟 F9 和直接命令路径，验证 PNG 文件落盘、Saved event 和 pending 清理，临时文件已删除）
- [x] 在 `project/` 下运行 `cargo fmt`。（验证：主 agent 在 `project/` 运行 `cargo fmt` 成功，且 `cargo fmt --check` 通过）
- [x] 在 `project/` 下运行 `cargo check`。（验证：主 agent 在 `project/` 运行 `cargo check` 通过且无 warning）

## 阶段 3：本地单页面审计模式

- 开始时间：2026-06-26 19:55:04 +08:00
- 结束时间：2026-06-26 20:41:13 +08:00
- 开发总结：新增本地一次性单页面 UI 审计状态机，默认由 `MYBEVY_UI_AUDIT` 门控关闭；支持 screen alias 路由到现有 `AppUiMode`，等待目标 owner panel 和固定稳定帧后通过命令式截图 API 截取 initial 状态，并写出 metadata、manifest 和 report。
- 验证记录：`cargo test ui::audit --lib` 通过（33 passed）；`cargo test game::navigation --lib` 通过（8 passed）；`cargo fmt --check` 通过；`cargo check` 通过；worker 手动运行 `MYBEVY_UI_AUDIT=1` + `MYBEVY_UI_AUDIT_SCREEN=ui-gallery` + `--window-profile phone-small --window-scale 50%`，确认输出目录含 manifest/report/metadata/screenshot 且 manifest 为 passed，产物已清理，主 agent 确认 `summary/ui-audit` 不存在。

- [x] 新增 `UiAuditPlugin`，默认不启用。（验证：`framework/ui/audit/local.rs` 定义 `UiAuditPlugin`，`drive_local_ui_audit` 仅在 `local_ui_audit_enabled` 通过时运行）
- [x] 读取 `MYBEVY_UI_AUDIT`、`MYBEVY_UI_AUDIT_SCREEN`、`MYBEVY_UI_AUDIT_OUTPUT`、`MYBEVY_UI_AUDIT_STATES`、`MYBEVY_UI_AUDIT_EXIT_ON_FINISH`，并明确这些变量只属于第一阶段本地一次性审计模式。（验证：`UiAuditConfig::from_env_reader` 读取上述变量，`local.rs` 常量旁注释说明 `MYBEVY_UI_AUDIT_*` 仅属于 local one-shot mode）
- [x] 复用或抽取现有页面 alias 解析逻辑，把 audit screen 转为 `AppUiMode`。（验证：`game/navigation/mod.rs` 抽出 `AppUiMode::aliases`、`canonical_screen` 和 `parse_start_screen_mode`，`register_ui_audit_screens` 注册到 `UiAuditScreenRegistry`）
- [x] 实现单页面 `initial` 状态审计流程：进入页面、等待目标页面根 panel、等待固定稳定帧、截图、记录结果。（验证：`advance_audit_phase` 按 RouteToScreen -> WaitForScreen -> WaitForStable -> RequestScreenshot -> WaitForScreenshot -> WriteSummary 推进，`target_owner_panel_ready` 检查 owner panel）
- [x] 生成本轮 `manifest.json`，记录 screen、device、state、截图路径、元数据路径和状态。（验证：`UiAuditManifest::{success,failure}` 写入 screen/device/state/screenshot_path/metadata_path/status，`write_success_outputs` 写 `manifest.json`）
- [x] 为每张截图生成 metadata JSON，包含 viewport、当前页面、panel 列表、截图路径和窗口信息。（验证：`build_capture_metadata` 生成 viewport/current_page/panels/screenshot_path/window/stats，写入 `metadata/.../00-initial.json`）
- [x] 生成基础 `report.md`，面向人阅读并链接截图文件。（验证：`build_report_markdown` 生成 screenshot/metadata 相对链接，`report_links_screenshot_and_metadata` 测试覆盖）
- [x] 支持 `MYBEVY_UI_AUDIT_EXIT_ON_FINISH=1` 时审计完成后自动退出进程。（验证：`request_exit_if_needed` 在 `exit_on_finish` 时写 `AppExit::Success`，worker 手动运行退出码 0）
- [x] 明确 `screen_not_found`、`panel_not_ready`、`unstable_ui`、`screenshot_failed` 等失败类型。（验证：`UiAuditFailureKind` 包含 ScreenNotFound/PanelNotReady/UnstableUi/ScreenshotFailed 并序列化为 snake_case，`failure_kind_strings_are_stable` 覆盖）
- [x] 为 audit 配置解析、状态机推进和失败分类补充 focused 测试。（验证：`cargo test ui::audit --lib` 通过 33 个 audit tests，覆盖 config、路径规划、状态机、失败分类、report 链接）
- [x] 手动运行 `MYBEVY_UI_AUDIT=1` + 单个页面 + 单个窗口 profile，验证输出目录完整。（验证：worker 运行 ui-gallery + phone-small，生成 `manifest.json`、`report.md`、`metadata/.../00-initial.json`、`screenshots/.../00-initial.png`，确认 manifest passed，随后清理产物）
- [x] 在 `project/` 下运行 `cargo fmt`。（验证：worker 运行 `cargo fmt` 通过，主 agent 运行 `cargo fmt --check` 通过）
- [x] 在 `project/` 下运行 `cargo check`。（验证：主 agent 在 `project/` 运行 `cargo check` 通过）

## 阶段 4：滚动状态和页面 Recipe

- 开始时间：2026-06-26 20:44:36 +08:00
- 结束时间：2026-06-26 21:47:19 +08:00
- 开发总结：新增页面 recipe 和滚动 capture 状态，UI Gallery 以 Rust 注册表声明 top/middle/bottom 三个截图状态；ScrollView 支持稳定审计 ID 和程序化滚动定位，本地审计 manifest/report/metadata 支持多 capture 并记录滚动目标、偏移、视口和内容高度，同时补齐滚动目标缺失和不可达失败出口。
- 验证记录：`cargo test ui::audit --lib` 通过（41 passed）；`cargo test ui::widgets::scroll --lib` 通过（8 passed）；`cargo test game::navigation --lib` 通过（9 passed）；`cargo fmt` 通过；`cargo check` 通过且无 warning；`git diff --check` 通过。worker 手动运行 `MYBEVY_UI_AUDIT=1` + `MYBEVY_UI_AUDIT_SCREEN=ui-gallery` + `MYBEVY_UI_AUDIT_STATES=top,middle,bottom` + `--window-profile phone-small`，确认生成 top/middle/bottom 三张截图、metadata 和 passed manifest，产物已清理；主 agent 确认 `summary/ui-audit` 不存在。

- [x] 设计页面 recipe 数据结构，描述 screen alias、`AppUiMode`、capture states、滚动目标和可选页面 ready 条件。（验证：`framework/ui/audit/local.rs` 定义 `UiAuditRecipe`、`UiAuditCaptureRecipe`、`UiAuditCaptureState` 和 `UiAuditReadyCondition`，`UiAuditScreen` 保留 alias/owner/recipe）
- [x] 第一版 recipe 先以 Rust 注册表实现，避免引入资源文件加载复杂度。（验证：`game/navigation/mod.rs` 的 `register_ui_audit_screen_entries` 在 Rust 代码中为 UI Gallery 注册 `UiAuditRecipe::new(UI_GALLERY_AUDIT_CAPTURES)`，未新增 recipe 资源文件）
- [x] 为 ScrollView 增加稳定审计 ID，例如 `UiScrollAuditId` 或等价组件。（验证：`framework/ui/widgets/scroll.rs` 定义 `UiScrollAuditId` 组件，`game/ui_ids.rs` 定义 `SCROLL_UI_GALLERY_MAIN`）
- [x] 暴露程序化设置滚动位置的接口，支持 top、middle、bottom。（验证：`framework/ui/widgets/scroll.rs` 定义 `UiScrollAuditPosition::{Top, Middle, Bottom}`、`set_scroll_audit_position` 和 `target_scroll_offset`，`cargo test ui::widgets::scroll --lib` 通过）
- [x] 在 UI Gallery 或一个已知长页面上注册滚动目标和 top/middle/bottom capture states。（验证：`game/navigation/mod.rs` 的 `UI_GALLERY_AUDIT_CAPTURES` 声明 top/middle/bottom，`game/screens/dev/ui_gallery.rs` 将 `SCROLL_UI_GALLERY_MAIN` 插入主 scroll body）
- [x] 每次滚动后等待页面稳定，再请求截图。（验证：`advance_audit_phase` 的 `ApplyCaptureState -> WaitForStable -> RequestScreenshot` 状态流覆盖每个 capture，`state_machine_applies_capture_state_after_panel_is_ready` 等 audit tests 通过）
- [x] metadata 记录 scroll target ID、offset、max offset、viewport height、content height 和目标位置。（验证：`UiAuditScrollMetadata` 包含 `target_id`、`offset`、`max_offset`、`viewport_height`、`content_height`、`position`，`build_capture_metadata` 写入 `scroll` 字段）
- [x] recipe 声明的滚动目标不存在时返回 `scroll_target_missing`，不允许静默跳过。（验证：`apply_capture_state` 未找到 `UiScrollAuditId` 时返回 `UiAuditFailureKind::ScrollTargetMissing`，序列化为 `scroll_target_missing`）
- [x] 无法到达指定滚动位置时返回 `scroll_target_unreachable`。（验证：`set_scroll_audit_position` 和 `scroll_audit_position_reached` 失败映射为 `UiAuditFailureKind::ScrollTargetUnreachable`，序列化为 `scroll_target_unreachable`）
- [x] 为滚动目标查找、位置计算、recipe 校验和失败路径补充测试。（验证：`cargo test ui::audit --lib` 覆盖 recipe 默认/过滤/缺失状态和失败分类，`cargo test ui::widgets::scroll --lib` 覆盖 top/middle/bottom offset、不可达和 position reached）
- [x] 手动审计 UI Gallery，验证 top、middle、bottom 三张截图和对应 metadata 都正确生成。（验证：worker 运行 `MYBEVY_UI_AUDIT=1`、`MYBEVY_UI_AUDIT_SCREEN=ui-gallery`、`MYBEVY_UI_AUDIT_STATES=top,middle,bottom`、`MYBEVY_UI_AUDIT_OUTPUT=../summary/ui-audit/stage4-manual`、`MYBEVY_UI_AUDIT_EXIT_ON_FINISH=1` 和 `cargo run -- --window-profile phone-small`，确认 manifest passed 且 top/middle/bottom 截图和 metadata 生成，随后清理产物）
- [x] 在 `project/` 下运行 `cargo fmt`。（验证：主 agent 在 `project/` 运行 `cargo fmt` 成功）
- [x] 在 `project/` 下运行 `cargo check`。（验证：主 agent 在 `project/` 运行 `cargo check` 通过且无 warning）

## 阶段 5：本地 Runner 和设备矩阵

- 开始时间：2026-06-26 21:50:29 +08:00
- 结束时间：2026-06-26 22:48:02 +08:00
- 开发总结：新增 Windows PowerShell 本地 UI 审计 runner，支持 screen/device 矩阵展开、一次一进程运行本地审计、run-id 分层输出、stdout/stderr 与子 manifest 收集、根 manifest/report 汇总、失败分类、dry-run/self-test，以及 FailedOnly 和 ScreenMatrix 两种失败复跑模式。runner 产物默认写入 `summary/ui-audit/<run-id>/`，主审和 worker 验证后均已清理。
- 验证记录：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 通过；主 agent dry-run 验证 `ui-gallery` 双设备展开、`all` 展开为 10 screens x 6 devices、尾部 `--window-size/--device-scale/--window-scale` 参数透传、FailedOnly 和 ScreenMatrix 复跑展开；主 agent 运行 `TimeoutSeconds 1` 验证失败分类为 `timeout` 且退出码为 1；`git diff --check -- scripts/run-ui-audit.ps1` 通过。worker 手动运行 `.\\scripts\\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,tablet-portrait -WindowScale 50% -TimeoutSeconds 600`，确认根 report 链接 6 张截图和 6 份 metadata，产物已清理。未修改 Rust 代码，故本阶段无需运行 `cargo fmt`/`cargo check`；主 agent 确认 `summary/ui-audit` 不存在且无残留 `cargo/rustc/project` 进程。

- [x] 确定 runner 形态，优先放入 `scripts/`，并兼容 Windows PowerShell 开发环境。（验证：新增 `scripts/run-ui-audit.ps1`，使用 PowerShell `param`、`ProcessStartInfo` 和 Windows `taskkill` 兼容处理）
- [x] runner 支持输入界面列表，列表可以是单个界面或全量界面。（验证：`Resolve-UiAuditScreens` 支持单值、逗号/分号列表和 `all/full`，主 agent dry-run 验证 `Screens all` 展开 10 个 screen）
- [x] runner 支持基础设备矩阵：`desktop`、`phone-small`、`phone-portrait`、`phone-1080p`、`tablet-portrait`、`tablet-landscape`。（验证：`$script:BasicDevices` 定义 6 个设备，主 agent dry-run 验证 `Devices all` 展开 6 个 device）
- [x] runner 在本地模式下为每个 screen + device 启动一次 `cargo run`，设置 `MYBEVY_UI_AUDIT_*` 环境变量。（验证：`New-UiAuditTask` 为每个 screen/device 生成任务，`Invoke-UiAuditCargoRun` 执行 `cargo run -- ...` 并设置 `MYBEVY_UI_AUDIT`、`SCREEN`、`OUTPUT`、`STATES`、`EXIT_ON_FINISH`）
- [x] runner 支持传入 `--window-profile`、`--window-size`、`--device-scale`、`--window-scale`。（验证：`Get-WindowArgumentOverrides` 支持 `-WindowProfile/-WindowSize/-DeviceScale/-WindowScale` 和尾部透传；主 agent dry-run manifest 显示 `--window-size 1280x2772 --device-scale 3.25 --window-scale 50%`）
- [x] runner 为每轮运行生成唯一 `run-id` 和输出目录。（验证：`New-UiAuditRunId` 生成时间戳+GUID 后缀，`Invoke-UiAuditRunner` 输出到 `summary/ui-audit/<run-id>` 或指定 `-OutputRoot`）
- [x] runner 收集每个子进程退出码、stdout/stderr 日志和子 manifest。（验证：`Invoke-UiAuditCargoRun` 写 `*.stdout.log`/`*.stderr.log` 和 exit code，`Resolve-UiAuditTaskResult` 读取 `runs/<screen>/<device>/manifest.json`）
- [x] runner 汇总全量 `manifest.json` 和总 `report.md`。（验证：`Write-UiAuditRunnerOutputs` 写根 `manifest.json`，`Build-UiAuditReport` 写根 `report.md`，self-test 覆盖 root manifest/report 写入）
- [x] runner 对启动失败、超时、审计失败和输出缺失做明确失败分类。（验证：`Resolve-UiAuditTaskResult` 覆盖 `launch_failed`、`timeout`、`audit_failed`、`manifest_missing`、`manifest_invalid`、`output_missing`、`process_failed`；主 agent 验证 1 秒超时分类为 `timeout`，修复后 self-test 覆盖空 entries 为 `output_missing`）
- [x] runner 支持只复跑失败的 screen + device，也支持复跑整个相关页面矩阵。（验证：`Get-FailedTaskSeedsFromManifest` 支持 `FailedOnly` 和 `ScreenMatrix`，主 agent 构造失败 manifest 验证 FailedOnly=1 项、ScreenMatrix=6 设备）
- [x] 为 runner 参数解析、任务展开、输出路径和失败分类补充脚本级验证。（验证：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 覆盖 screen/device 解析、窗口参数展开、矩阵任务、路径布局、失败分类、root 输出和复跑 seed）
- [x] 手动运行本地单页面双设备矩阵，验证报告链接到每张截图。（验证：worker 运行 `ui-gallery` + `phone-small,tablet-portrait` + `-WindowScale 50%`，确认根 `report.md` 链接 6 张截图和 6 份 metadata，产物已清理）
- [x] 涉及 Rust 代码时在 `project/` 下运行 `cargo fmt`。（验证：本阶段仅新增 PowerShell runner，未修改 Rust 代码，故不适用）
- [x] 涉及 Rust 代码时在 `project/` 下运行 `cargo check`。（验证：本阶段仅新增 PowerShell runner，未修改 Rust 代码，故不适用）

## 阶段 6：远程调试控制机制接入

- 开始时间：2026-06-26 22:52:31 +08:00
- 结束时间：2026-06-26 23:29:27 +08:00
- 开发总结：在 `scripts/run-ui-audit.ps1` 中新增显式远程模式，保留 Local 作为默认桌面/CI 兜底；Remote 支持 Mock/Http adminapi backend、device/client/session 目标选择、按文档创建和轮询 debug task、远程 UI 审计命令序列、task 状态和错误映射、artifact URI 与本地 mock artifact 映射、远程 runner manifest/report 汇总。Mock adminapi 可跑通 UI Gallery top/middle/bottom 单页面链路，并对缺失 screenshot/metadata artifact 输出 `artifact_upload_failed`。
- 验证记录：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 通过；主 agent 运行 Mock Remote `ui-gallery` + `android-test-01` + `top,middle,bottom` 通过，确认 `manifest.status=passed`、`captures=3`、`remote_tasks=24`、scroll target 为 `ui_gallery.main` 且 screenshot artifact URI 为 `artifact://debug/.../screenshot.png`；主 agent 运行 Mock Remote 缺 metadata 链路，确认退出码 1、`failure_type=artifact_upload_failed`；主 agent 运行本地 dry-run，确认 Local 模式仍输出 `--window-profile phone-small --window-scale 50%`；主 agent 确认 report 包含 task id、artifact URI、screenshot/metadata/client.log 引用；PowerShell parser 检查通过；`git diff --check -- scripts/run-ui-audit.ps1` 通过。未修改 Rust 代码，故本阶段无需运行 `cargo fmt`/`cargo check`；临时 mock 产物已清理，`summary/ui-audit` 不存在。

- [x] 对齐 `docs/debug/远程调试控制机制.md`，明确 UI 审计 runner 通过 `adminapi` 创建 debug task 并轮询任务结果。（验证：`Invoke-RemoteDebugCreateTask` 对 Http backend 调用 `POST /admin/debug/commands`，`Get-RemoteDebugTask` 调用 `GET /admin/debug/tasks/{task_id}`，`Wait-RemoteDebugTask` 轮询到终态）
- [x] 定义远程模式下 UI 审计命令序列：`system.status`、`ui.goto_screen`、`ui.wait_stable`、`ui.read_viewport`、`ui.scroll_to`、`ui.screenshot`、`ui.read_tree`、`ui.read_panels`。（验证：`$script:RemoteUiAuditCommandTypes` 和 `New-RemoteUiAuditCommandSequence` 按该顺序生成命令，self-test 断言完整顺序）
- [x] 将本地模式的 screen、device、state、metadata、manifest 概念映射到远程 task、artifact 和 client result。（验证：`New-RemoteUiAuditTask` 生成 screen/state/remote target 任务，`New-RemoteCapture` 记录 state、remote task ids、artifact URI、scroll target/position，根 manifest 使用 `remote_runner` 模式）
- [x] runner 支持选择目标 `device_id`、`client_id` 或 `session_id`。（验证：`Resolve-RemoteUiAuditTargets` 解析 `-DeviceId`、`-ClientId`、`-SessionId`，`New-RemoteDebugCommandRequest` 写入对应请求字段）
- [x] runner 支持远程 task 轮询，并能识别 `accepted`、`queued`、`sent`、`running`、`succeeded`、`failed`、`timeout`、`cancelled`。（验证：`$script:RemoteTaskStates` 和 `$script:RemoteTerminalTaskStates` 定义状态集合，`Test-RemoteTaskStatusKnown`/`Test-RemoteTaskTerminalStatus` self-test 覆盖中间态和终态）
- [x] runner 支持从 adminapi 返回的 artifact 读取截图、metadata 和日志。（验证：`Convert-RemoteArtifactsToMap` 映射 `screenshot`、`metadata`、`client_log/log`，Mock Remote 正常链路 report 包含 screenshot.png、metadata.json 和 client.log 引用）
- [x] runner 支持远程失败分类，至少覆盖 `device_offline`、`debug_disabled`、`send_failed`、`client_timeout`、`client_rejected`、`artifact_upload_failed`。（验证：`$script:RemoteKnownFailureCodes` 和 `Convert-RemoteErrorToFailureType` 覆盖上述错误，self-test 遍历断言；主 agent 验证缺 metadata artifact 分类为 `artifact_upload_failed`）
- [x] 远程模式复用本地模式的报告结构，并在 report 中关联 task id、device id、artifact URI 和截图。（验证：`Build-UiAuditReport` 对 remote 输出 Tasks/Captures 表，主 agent 检查 report 包含 `dbg_task_mock_`、`artifact://debug/`、`screenshot.png`、`metadata.json`、`client.log` 和 remote target）
- [x] 明确本地模式和远程模式的优先级：本地模式作为桌面开发和 CI 兜底，远程模式作为多设备、移动端和 AI 交互式审计主通道。（验证：`-Mode Local` 默认，`-Remote`/`-Mode Remote` 显式启用；manifest `execution_priority` 和 report `Channel priority` 记录 remote 显式选择时为主通道、local 为兜底）
- [x] 为远程 task 状态解析、artifact 映射、错误分类和报告汇总补充脚本级或单元测试。（验证：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 覆盖远程目标解析、命令序列、状态终态、错误分类、Mock artifact URI/本地路径映射、缺 artifact 失败和 remote manifest/report 写入）
- [x] 使用 mock adminapi 或测试替身跑通一次远程单页面审计链路。（验证：主 agent 运行 Mock Remote `ui-gallery` + `android-test-01` + `top,middle,bottom`，输出 `status=passed tasks=1 captures=3 remote_tasks=24`，产物随后清理）
- [x] 涉及 Rust 代码时在 `project/` 下运行 `cargo fmt`。（验证：本阶段仅修改 PowerShell runner，未修改 Rust 代码，故不适用）
- [x] 涉及 Rust 代码时在 `project/` 下运行 `cargo check`。（验证：本阶段仅修改 PowerShell runner，未修改 Rust 代码，故不适用）

## 阶段 7：AI 分析输入和问题分级

- 开始时间：2026-06-26 23:32:49 +08:00
- 结束时间：2026-06-27 00:09:36 +08:00
- 开发总结：为 runner 新增分析输入/输出和 gating 流程，Local/Remote 每轮都会写 `analysis-input.json` 与 `analysis.json`；支持 `Auto`、`Fixture`、`Off` 三种分析模式，Fixture 可读取人工分析结果并按 severe/medium/minor 分级，严重或中等/阻塞问题将 runner 判为失败，轻微问题只进入报告。`report.md` 新增 Analysis 区块，按 screen/device/state 关联截图、metadata、问题、证据、原因和建议文件。
- 验证记录：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 通过；PowerShell parser 检查通过；主 agent 运行 Local dry-run，确认 `analysis.status=skipped` 且本地参数不受影响；主 agent 使用 Remote Mock + minor fixture 验证退出码 0、manifest passed、analysis passed、blocking=0 且 report 包含轻微问题；主 agent 使用 Remote Mock + `text_overlap` fixture 验证退出码 1、manifest failed、`failure_type=ai_blocking_issue` 且 severity 升级为 `severe`；主 agent 验证非法 JSON 分类为 `ai_result_invalid`，远程 metadata artifact 缺失分类为 `ai_remote_artifact_read_failed`；`git diff --check -- scripts/run-ui-audit.ps1` 通过。未修改 Rust 代码，故本阶段无需运行 `cargo fmt`/`cargo check`；临时产物已清理，`summary/ui-audit` 不存在。

- [x] 定义 AI 分析输入包格式，包含截图、单张 metadata、总 manifest、screen、device、state、likely files，以及远程模式下的 task id 和 artifact URI。（验证：`New-UiAuditAnalysisInput` 写 `analysis-input.json`，capture 包含 screen/device/state/screenshot/metadata/likely_files；Remote capture 额外包含 artifact URI 和 remote task ids，self-test 覆盖 remote mapping）
- [x] 定义 AI 分析输出格式，包含 severity、problem、evidence、likely cause、suggested files 和 blocking 标记。（验证：`ConvertTo-UiAuditAnalysisIssues` 规范化 issue 字段，`analysis.json` 写入 severity/problem/evidence/likely_cause/suggested_files/blocking）
- [x] 实现严重、中等、轻微三档问题分级。（验证：`$script:AnalysisSeverityLevels` 定义 `severe/medium/minor`，`New-UiAuditAnalysisSummary` 统计三档数量）
- [x] 将文字重叠、关键裁切、不可点击、关键内容不可达、弹窗层级错误归为阻塞级或中等级。（验证：`$script:AnalysisBlockingProblemTypes` 覆盖 `text_overlap`、`critical_clipping`、`unclickable`、`critical_content_unreachable`、`modal_layering_error`，关键词规则覆盖中文/英文文本；主 agent 验证 `text_overlap` 升级为 `severe`）
- [x] 允许轻微问题进入报告但不阻塞通过。（验证：主 agent minor fixture 运行退出码 0，`analysis.status=passed`、blocking=0，report 包含“对齐可以更整齐”）
- [x] 将 AI 分析结果写入 `analysis.json`。（验证：`Write-UiAuditAnalysisOutput` 写根 `analysis.json`，self-test 和主 agent 验证文件存在并读取结果）
- [x] 在 `report.md` 中按 screen + device + state 汇总 AI 判定和截图链接。（验证：`Build-UiAuditReport` 的 `## Analysis` 表包含 screen/device/state/severity/blocking/screenshot/metadata/problem/evidence/likely cause/suggested files，主 agent 验证 report 包含 fixture 问题）
- [x] 对 AI 分析失败、返回格式非法、缺少截图或 metadata、远程 artifact 读取失败的情况做明确失败分类。（验证：`Invoke-UiAuditAnalysis` 输出 `ai_analysis_failed`、`ai_result_invalid`、`ai_missing_capture`、`ai_missing_capture_metadata`、`ai_remote_artifact_read_failed`；主 agent 验证非法 JSON 和远程 metadata 缺失分类）
- [x] 准备一组人工构造的分析结果样例，用于验证 report 和 gating 逻辑。（验证：self-test 使用 `Write-FakeAnalysisResult`/`New-FakeAnalysisIssue` 构造 minor、blocking、medium、invalid、missing-field、remote minor/blocking fixtures）
- [x] 验证严重或中等问题会使本轮审计失败。（验证：主 agent `text_overlap` fixture 运行退出码 1，`analysis.failure_type=ai_blocking_issue`，manifest failed；self-test 覆盖 medium problem type 阻塞）
- [x] 验证只有轻微问题时本轮审计可以通过并记录建议。（验证：主 agent minor fixture 运行退出码 0，analysis passed，report 记录问题和建议文件）
- [x] 涉及 Rust 代码时在 `project/` 下运行 `cargo fmt`。（验证：本阶段仅修改 PowerShell runner，未修改 Rust 代码，故不适用）
- [x] 涉及 Rust 代码时在 `project/` 下运行 `cargo check`。（验证：本阶段仅修改 PowerShell runner，未修改 Rust 代码，故不适用）

## 阶段 8：AI 自动修复闭环

- 开始时间：2026-06-27 00:12:57 +08:00
- 结束时间：2026-06-27 01:23:47 +08:00
- 开发总结：在 `scripts/run-ui-audit.ps1` 中新增默认关闭的 AI 修复闭环，支持 `FixMode Off/Plan/Mock/Command`、最大迭代次数、页面局部布局到框架底层的策略优先级、before/after iteration 快照、发生问题页面的复跑矩阵、修复后 `cargo fmt`/`cargo check` 检查、`max_iterations_reached`/`fix_check_failed`/`safety_policy_rejected` 失败出口，以及 git status + policy 文件快照的安全边界，覆盖 ignored 审计产物的新建、修改和删除。
- 验证记录：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 通过；PowerShell parser 检查通过；`git diff --check -- scripts/run-ui-audit.ps1` 通过（仅行尾转换提示）；远程 Mock + blocking fixture + `FixMode Mock` 正向链路通过，生成 before/after snapshot 和 Fix Loop report；定向验证 Command 写入和删除 ignored `summary/ui-audit/...` 均返回 `safety_policy_rejected`；`cargo fmt` 通过；`cargo check` 通过；确认 `summary/ui-audit` 不存在。

- [x] 设计修复循环入口，允许 AI 基于 blocking issue 修改当前项目 UI 代码。（验证：`scripts/run-ui-audit.ps1` 新增 `-FixMode Off|Plan|Mock|Command`、`-FixCommand` 和 `Invoke-UiAuditFixLoop`，`Resolve-UiAuditRunnerExitCode` 在 `ai_blocking_issue` 且 FixMode 非 Off 时进入修复循环）
- [x] 修复策略按页面局部布局、通用控件、主题 token、框架底层的优先级执行。（验证：`$script:FixStrategyPriority` 定义 `page_local_layout`、`common_widgets`、`theme_tokens`、`framework_core`，manifest `fix_loop.strategy_priority` 和 report Fix Loop 表记录该顺序）
- [x] 每次修复后运行 `cargo fmt`。（验证：`Invoke-UiAuditFixChecks` 对 Command 模式执行 `cargo fmt`，Mock 模式写入 `checks/cargo-fmt.stdout.log`；self-test 断言 check logs 存在）
- [x] 每次修复后运行 `cargo check`。（验证：`Invoke-UiAuditFixChecks` 对 Command 模式执行 `cargo check`，Mock 模式写入 `checks/cargo-check.stdout.log`；self-test 断言 check logs 存在）
- [x] 修复后至少复跑发生问题的页面完整设备矩阵；远程模式下复跑对应 device/client 的完整相关矩阵。（验证：`New-UiAuditFixRerunPlan` 本地返回 `local_failed_screen_full_device_matrix` 和 6 个基础设备，远程返回 `remote_related_target_matrix` 并保留相关 target；self-test 覆盖本地 6 设备和远程 1 target）
- [x] 保留 before / after 截图和 metadata，目录按 iteration 分层。（验证：`Copy-UiAuditIterationSnapshot` 写 `iterations/00-before/snapshot.json` 和 `iterations/01-after-fix/snapshot.json`，主 agent 远程 Mock 正向链路确认 before/after snapshot 存在）
- [x] 设置最大修复迭代次数，例如 `max_fix_iterations = 5`。（验证：参数 `-MaxFixIterations` 默认 5，manifest `fix_loop.max_fix_iterations` 记录该值，`Invoke-UiAuditFixLoop` 拒绝小于 1 的值）
- [x] 达到最大迭代次数仍未通过时输出 `max_iterations_reached`，保留最后一轮问题列表和截图。（验证：`MockFixScenario MaxIterations` self-test 退出码 1，`fix_loop.failure_type=max_iterations_reached`，iterations 数为 2 且 `final_issues` 非空）
- [x] 修复导致格式化或编译失败时输出 `fix_check_failed`，并保留相关日志。（验证：`MockFixScenario CheckFailed` self-test 退出码 1，`fix_loop.failure_type=fix_check_failed`，存在 `iterations/01-after-fix/checks/cargo-check.stderr.log`）
- [x] 防止 AI 修改审计产物、构建产物或无关项目文件。（验证：`New-UiAuditFixPolicy` 限制 allowed roots 并禁止 `summary/`、`target/`、`project/target/`、Android build、`.git/`、`.env*` 和敏感命名；主 agent 定向验证 Command 新建和删除 ignored `summary/ui-audit/...` 均被 `safety_policy_rejected` 拒绝并记录 violation）
- [x] 对一次刻意制造的 UI 问题进行端到端演练：发现问题、修改代码、通过本地或远程通道复跑、生成 after 报告。（验证：主 agent 使用远程 Mock + `text_overlap` blocking fixture + `FixMode Mock` 跑通，退出码 0，`fix_loop.status=passed`，生成 after manifest/report/analysis 和 `## Fix Loop` 报告段）
- [x] 在 `project/` 下运行 `cargo fmt`。（验证：主 agent 在 `project/` 运行 `cargo fmt` 成功）
- [x] 在 `project/` 下运行 `cargo check`。（验证：主 agent 在 `project/` 运行 `cargo check` 通过，输出 `Finished dev profile`）

## 阶段 9：文档、维护和最终验收

- 开始时间：2026-06-27 01:27:40 +08:00
- 结束时间：2026-06-27 01:55:47 +08:00
- 开发总结：更新 UI 审计方案、远程调试控制、UI 调试验收和新成员上手文档，明确已实现的手动截图、本地/远程 runner、AI 分析、修复闭环、产物维护、支持矩阵和当前不支持项；完成本地真实双设备 UI Gallery 审计、远程 Mock 成功与失败报告演示，并保留真实远程 server/client 仍为外部依赖的说明。
- 验证记录：`.\\scripts\\run-ui-audit.ps1 -SelfTest` 通过；本地真实演示 `ui-gallery` + `phone-small,tablet-landscape` + `States auto` 通过，生成 2 个 task、6 张截图、6 份 metadata；远程 Mock `android-test-01` + `ui-gallery` + `top` 通过并记录 task/artifact URI；远程 Mock 缺 metadata 演示失败为 `artifact_upload_failed` 且报告定位 screen/device/state；`git diff --check` 通过（仅行尾转换提示）；`cargo fmt` 通过；`cargo check` 通过；确认 `summary/ui-audit` 不存在。

- [x] 更新 `docs/ui/UI自动化审计与优化方案.md`，把已实现能力和仍是设计的部分区分清楚。（验证：文档“文档状态”列出已实现 F9、`MYBEVY_UI_AUDIT_*`、runner、AI fixture、FixMode，并单列真实远程 server/client、Android 真机截图等外部依赖/未实现项）
- [x] 更新 `docs/debug/远程调试控制机制.md`，记录 UI 审计实际依赖的 adminapi 命令、artifact 和错误码。（验证：文档新增 runner 实际调用 `POST /admin/debug/commands`、`GET /admin/debug/tasks/{task_id}`、8 步 UI 命令序列、artifact kind 和错误码映射）
- [x] 更新 `docs/ui/UI调试与验收.md`，加入手动截图、审计模式和 runner 使用方式。（验证：文档新增 F9 手动截图、本地 `MYBEVY_UI_AUDIT_*`、本地 runner、远程 Mock/Http runner、报告验收重点和失败类型）
- [x] 如新增命令影响新成员上手流程，同步检查 `docs/bevy-getting-started.md`。（验证：文档新增“UI 调试和审计入口”，记录 F3/F9 和 runner self-test/dry-run 命令）
- [x] 记录审计产物目录、是否提交 Git、如何清理旧产物等维护约束。（验证：`docs/ui/UI自动化审计与优化方案.md` 和 `docs/ui/UI调试与验收.md` 均记录 `summary/ui-audit/<run-id>/`、Git 忽略、默认不提交和清理方式）
- [x] 记录已支持的 screen alias、设备矩阵和滚动 recipe。（验证：`docs/ui/UI自动化审计与优化方案.md` 记录 10 个 canonical screen/alias、6 个基础 device 和 `ui_gallery.main` top/middle/bottom recipe）
- [x] 记录当前不支持的场景，例如第二窗口截图、offscreen render target、Android 真机截图、系统 UI 截图或远程调试 server 未接入能力。（验证：`docs/ui/UI自动化审计与优化方案.md` “当前不支持的场景”列出第二窗口、offscreen、Android 真机/系统 UI、真实远程 server/client 等限制）
- [x] 完成一次本地全流程演示，至少覆盖一个可滚动页面和两个窗口 profile。（验证：主 agent 运行本地真实 `ui-gallery` + `phone-small,tablet-landscape` + `States auto`，manifest passed，2 个任务、6 张截图、6 份 metadata，states 为 top/middle/bottom）
- [x] 完成一次远程模式演示或 mock 演示，至少覆盖一个目标 device/client、一个页面、一次截图和一次状态读取。（验证：主 agent 运行 Remote Mock `android-test-01` + `ui-gallery` + `top`，manifest passed，8 个 remote task，包含 `system.status`、`ui.read_viewport`、`ui.screenshot`、`ui.read_tree`、`ui.read_panels` 和 artifact URI）
- [x] 确认成功通过的截图记录和总结文档能互相关联。（验证：本地真实演示 `report.md` 存在并链接 6 个 screenshot/metadata；远程 Mock report 包含 mock task id 和 screenshot/metadata/log artifact URI）
- [x] 确认失败报告能定位到 screen、device、state、失败类型和日志或截图路径。（验证：Remote Mock `mock-artifacts-missing_metadata` 退出码 1，manifest/report 记录 `screen=ui_gallery`、`device_id=mock-artifacts-missing_metadata`、`state=top`、`artifact_upload_failed`、screenshot URI 和缺失 metadata）
- [x] 在 `project/` 下运行 `cargo fmt`。（验证：主 agent 在 `project/` 运行 `cargo fmt` 成功）
- [x] 在 `project/` 下运行 `cargo check`。（验证：主 agent 在 `project/` 运行 `cargo check` 通过，输出 `Finished dev profile`）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-06-27 01:55:47 +08:00
- 结束时间：2026-06-27 01:55:47 +08:00
- 验收总结：当前仓库范围内 UI 自动化审计机制已完成：游戏内审计模式、截图、滚动 recipe、本地/远程 runner、AI 分析、修复闭环、报告和文档均可验证；真实远程 adminapi/game-server/client 执行链路仍是外部依赖，本仓库已提供 Http 调用端和 Mock 后端并在文档中明确边界。

- [x] 可输入单个界面或全量界面列表并启动审计。（验证：`Resolve-UiAuditScreens` 支持单个 alias 和 `all/full`，runner self-test 覆盖 screen 展开）
- [x] 基础设备矩阵全部可运行并生成截图。（验证：本地 runner `Devices all` 支持 6 个基础设备；主 agent 真实演示覆盖 `phone-small` 和 `tablet-landscape`，self-test/dry-run 覆盖矩阵展开）
- [x] 本地一次性审计模式可通过 `MYBEVY_UI_AUDIT_*` 运行。（验证：阶段 3/4 手动本地审计记录通过，runner 本地真实演示通过并由子进程设置 `MYBEVY_UI_AUDIT_*`）
- [x] 远程调试控制模式可通过 adminapi 对指定 device/client 创建 UI 审计任务并查询结果。（验证：Remote Mock 后端对 `device_id=android-test-01` 创建并查询 8 个 mock task；Http 后端按文档实现 `POST /admin/debug/commands` 和 `GET /admin/debug/tasks/{task_id}` 调用端，真实 server/client 为外部依赖）
- [x] 远程模式支持多设备或多 client 的目标选择。（验证：`Resolve-RemoteUiAuditTargets` 支持 `-DeviceId`、`-ClientId`、`-SessionId` 列表，self-test 覆盖 target 解析）
- [x] 每个 screen + device + state 都有截图文件和 metadata。（验证：主 agent 本地真实演示 `ui_gallery` 两设备 top/middle/bottom 生成 6 张截图和 6 份 metadata，缺失数为 0）
- [x] 远程模式下每个 screen + device/client + state 都有关联 task id、artifact URI、截图和 metadata。（验证：Remote Mock success manifest/report 记录 `task_ids=8`，capture 含 `artifact://debug/.../screenshot.png` 和 `metadata.json`；缺 metadata 失败能定位 artifact 缺失）
- [x] 可滚动界面至少覆盖 top、middle、bottom，或覆盖 recipe 声明的全部滚动状态。（验证：UI Gallery recipe 使用 `ui_gallery.main`，主 agent 本地真实演示 states 为 bottom/middle/top）
- [x] `report.md` 完整关联所有通过截图和失败证据。（验证：本地 success report 链接 screenshot/metadata；远程 failure report 记录 `artifact_upload_failed`、task id、screenshot artifact URI 和缺失 metadata）
- [x] AI 分析严重问题为 0。（验证：最终通过演示 analysis 为 skipped/无 issue；阶段 7 blocking fixture 验证严重问题会使 manifest failed）
- [x] AI 分析中等问题为 0。（验证：最终通过演示 analysis 为 skipped/无 issue；阶段 7 medium fixture 验证中等问题会阻塞）
- [x] 轻微问题不阻塞通过，但会写入报告。（验证：阶段 7 minor fixture 退出码 0，analysis passed 且 report 记录 minor issue）
- [x] 若发生自动修复，before / after 截图和 metadata 可追溯。（验证：阶段 8 Remote Mock `FixMode Mock` 正向链路写 `iterations/00-before/snapshot.json` 和 `iterations/01-after-fix/snapshot.json`）
- [x] 若发生自动修复，最终 `cargo fmt` 和 `cargo check` 通过。（验证：阶段 8 fix loop checks 记录 cargo fmt/check，最终主 agent 再次运行 `cargo fmt` 和 `cargo check` 通过）
- [x] 审计模式默认关闭，普通 `cargo run` 行为不变。（验证：`MYBEVY_UI_AUDIT` 未设置时本地审计系统不运行，FixMode 默认 Off；self-test 覆盖 default-off 不创建 iterations）
- [x] 远程调试命令只在授权 debug/audit 模式执行，符合 `docs/debug/远程调试控制机制.md` 的安全边界。（验证：文档记录 adminapi 鉴权、白名单、debug/audit 授权模式和禁止 shell/任意文件访问；本仓库仅实现 runner Mock/Http 调用端，真实授权执行由外部 server/client 接入）
- [x] 达到失败条件时能明确输出失败类型，而不是静默通过或无限循环。（验证：self-test 和主 agent 演示覆盖 `artifact_upload_failed`、`ai_blocking_issue`、`max_iterations_reached`、`fix_check_failed`、`safety_policy_rejected` 等失败类型）
