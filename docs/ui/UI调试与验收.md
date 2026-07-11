# UI 调试与验收

UI 调试能力集中在 `project/src/framework/ui/debug.rs`。调试面板用于观察 viewport、metrics、输入路由、面板栈、UI 树和布局边界。

## F3 调试面板

快捷键：

- `F3`：打开或关闭调试面板。
- `F4`：冻结或恢复当前调试内容。
- `F5`：切换面板过滤模式：all、active panels only、blocking panels only。
- `F6`：打开或关闭 panel 高亮。
- `F7`：在主窗口调试层和专用调试窗口之间切换。Android 不支持专用窗口，会保持主窗口。
- `F8`：把当前调试文本复制到内部 buffer，并输出日志预览。

调试面板内容包括：

- `UiViewport`：逻辑尺寸、设备尺寸、缩放、preview scale、窗口尺寸、宽高分类、方向。
- `UiMetrics`：内容最大宽度、弹窗最大宽度、padding、gap。
- `UiInputState`：是否阻断、阻断原因、route summary、route history。
- `UiFocusState`：当前焦点实体。
- `UiStats`：UI 节点、可见节点、文本节点、各类 panel 数量。
- panel 列表和 active panel 栈。
- UI tree 根节点摘要。
- layout bounds 摘要。

## F9 手动截图

桌面 debug 构建默认启用 `F9` 主窗口截图。进入任意页面后按 `F9`，当前 primary window 的最终合成画面会保存为 PNG，并在日志中输出保存路径。

默认目录：

```text
summary/ui-audit/manual/
```

文件名包含 Unix 秒级时间戳、当前 UI owner、逻辑尺寸和物理尺寸。可用环境变量覆盖：

```powershell
Set-Location project
$env:MYBEVY_UI_AUDIT_MANUAL_SCREENSHOT="1"
$env:MYBEVY_UI_AUDIT_MANUAL_OUTPUT="..\summary\ui-audit\manual"
cargo run -- --window-profile phone-small --window-scale 50%
```

Android 和 wasm 默认不启用手动截图。当前截图只覆盖 primary window，不覆盖 F7 专用调试窗口、系统 UI 或 offscreen render target。

## 启动目标页面

可用 `TOUCH_START_SCREEN` 直接进入目标页面：

```powershell
Set-Location project
$env:TOUCH_START_SCREEN="gallery"
cargo run -- --window-profile phone-small --window-scale 50%
```

支持值见 [UI模式与面板层级.md](UI模式与面板层级.md)。

## UI 审计模式

本地一次性审计模式通过 `MYBEVY_UI_AUDIT_*` 环境变量驱动，适合确认单个页面在单个窗口 profile 下能自动进入、稳定、滚动和截图：

```powershell
Set-Location project
$env:MYBEVY_UI_AUDIT="1"
$env:MYBEVY_UI_AUDIT_SCREEN="ui-gallery"
$env:MYBEVY_UI_AUDIT_OUTPUT="..\summary\ui-audit\manual-local-check"
$env:MYBEVY_UI_AUDIT_STATES="image_fit,visual_foundation,image_modes,image_tiling,image_atlas,middle,bottom"
$env:MYBEVY_UI_AUDIT_EXIT_ON_FINISH="1"
cargo run -- --window-profile phone-small --window-scale 50%
```

输出目录会包含 `manifest.json`、`report.md`、`screenshots/` 和 `metadata/`。`ui_gallery` 的默认 runner 状态为 `image_fit`、`visual_foundation`、`image_modes`、`image_tiling`、`image_atlas`、`middle`、`bottom`；前两项固定指向主滚动容器顶部，三个高级图片状态分别对齐稳定 child anchor，兼容状态 `top` 仍可显式请求。其他页面默认只覆盖 `initial`。

常规批量验收优先使用仓库根目录 runner：

```powershell
.\scripts\run-ui-audit.ps1 -SelfTest
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,tablet-landscape -States "image_fit,visual_foundation,image_modes,image_tiling,image_atlas,middle,bottom" -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,tablet-landscape -States auto
```

`-DryRun` 只验证矩阵、报告和分析输入，不启动游戏、不生成真实截图。真实本地运行会为每个 screen + device 启动一次 `cargo run`，并把子进程输出写入本轮 `logs/`。

远程 Mock 验收用于确认 adminapi 任务模型、artifact 汇总和报告关联：

```powershell
.\scripts\run-ui-audit.ps1 -Remote -RemoteBackend Mock -DeviceId android-test-01 -Screens ui-gallery -States top
```

远程 Http 验收会调用外部 `adminapi`；真实能否控制设备取决于外部 server / client 是否已经接入调试命令：

```powershell
.\scripts\run-ui-audit.ps1 -Remote -RemoteBackend Http -AdminApiBaseUrl http://127.0.0.1:8080 -AdminApiToken <token> -DeviceId android-test-01 -Screens ui-gallery -States top
```

审计产物默认写入 `summary/ui-audit/<run-id>/`。该目录被 Git 忽略，不应作为常规代码提交内容。清理旧产物时删除具体 run 目录或整个 `summary/ui-audit/`，保留 `summary/.gitkeep`。

当前 runner 支持的基础设备矩阵：

- `desktop`
- `phone-small`
- `phone-portrait`
- `phone-1080p`
- `tablet-portrait`
- `tablet-landscape`

当前支持的 screen alias：

- `login`
- `lobby`, `game_list`, `game-list`, `list`
- `audio_settings`, `audio-settings`, `audio`, `settings`
- `audio_monitor`, `audio-monitor`, `audio_debug`, `audio-debug`
- `audio_gallery`, `audio-gallery`
- `wanfa_touch_ripple`, `wanfa-touch-ripple`, `touch`, `touch_ripple`, `touch-ripple`
- `ui_gallery`, `ui-gallery`, `gallery`
- `sample_scene`, `sample-scene`, `sample`
- `robot_sync_scene`, `robot-sync-scene`, `robot`
- `fangyuan_home`, `fangyuan-home`, `fangyuan`

## 窗口级验收命令

常用命令：

```powershell
Set-Location project
cargo run -- --window-profile phone-portrait
cargo run -- --window-profile phone-1080p
cargo run -- --window-profile phone-small
cargo run -- --window-profile tablet-portrait
cargo run -- --window-profile tablet-landscape
cargo run -- --window-size 1280x2772
cargo run -- --window-profile phone-portrait --window-scale 50%
cargo run -- --window-size 1280x2772 --device-scale 3.25 --window-scale 50%
```

窗口验收重点：

- 页面根节点是否带 `UiPanelRoot` 和合理 `UiLayerRoot`。
- 文本是否在紧凑屏、平板横屏和高 DPI preview 下不重叠、不裁切关键内容。
- 按钮、输入框、图标按钮是否保持稳定尺寸，并满足最小触控目标。
- Confirm、Loading 打开后下层页面和 gameplay 输入是否被阻断。
- Floating 是否在紧凑屏保持可见边界。
- ScrollView 是否能滚轮和拖拽滚动。
- F3 中 `pointer_blocked` 和 `block_reason` 是否符合当前交互状态。
- F3 中 panel stack 是否符合当前覆盖层顺序。

## 审计报告验收重点

成功报告需要确认：

- `manifest.json` 的 `status` 为 `passed` 或 dry-run 时为 `planned`。
- `report.md` 中每个 capture 都有对应 screenshot 和 metadata 链接；远程模式至少有 screenshot / metadata artifact URI。
- `analysis-input.json` 中每条 capture 的 `screen`、`device`、`state` 能对应回 `manifest.json` 和 `report.md`。
- 有滚动 recipe 的 `ui_gallery` 默认覆盖 `image_fit`、`visual_foundation`、`image_modes`、`image_tiling`、`image_atlas`、`middle`、`bottom`，并记录 `scroll_target_id = ui_gallery.main`；图片适配固定基线使用 `image_fit`，完整 fixture 使用 `visual_foundation`，九宫格/平铺/图集分别使用三个 child anchor 状态，不要依赖会随页面总高度变化的 `middle`。

失败报告需要能定位：

- task 级：`screen`、`device` 或远程目标、`states`、`failure_type`、stdout/stderr 或 remote task id。
- capture 级：`screen`、`device`、`state`、失败类型、截图/metadata/log 路径或 artifact URI。
- analysis 级：issue 必须包含 `screen`、`device`、`state`、severity、blocking、problem、evidence、likely cause 和 suggested files。

常见失败类型包括：

- 本地启动与输出：`launch_failed`、`timeout`、`manifest_missing`、`manifest_invalid`、`output_missing`、`process_failed`。
- 游戏内审计：`screen_not_found`、`panel_not_ready`、`unstable_ui`、`scroll_target_missing`、`scroll_target_unreachable`、`screenshot_failed`、`config_invalid`。
- 远程任务：`device_offline`、`debug_disabled`、`send_failed`、`client_timeout`、`client_rejected`、`artifact_upload_failed`、`remote_error`、`remote_failed`。
- AI 与修复：`ai_blocking_issue`、`ai_missing_capture_metadata`、`ai_result_invalid`、`safety_policy_rejected`、`fix_check_failed`、`max_iterations_reached`。

## 当前已知窗口验收状态

近期窗口级检查覆盖过：

- Login、Lobby、UI Gallery、Touch Ripple 在 `phone-small --window-scale 50%` 下视觉可用。
- UI Gallery 在平板横屏 profile 下视觉可用。
- Confirm 打开后下层页面输入被阻断。
- F3 调试面板可显示 viewport、metrics、input route、panels、stats、tree 和 layout 信息。
- F9 手动主窗口截图可写入 `summary/ui-audit/manual/`。
- UI audit runner 可生成本地 dry-run 报告和远程 Mock 报告。
- Touch Ripple 拖动可生成水波纹拖尾。

桌面运行时如果出现 Vulkan validation layer 缺失 warning，属于本机图形环境提示，不是项目 UI 逻辑错误。

## Android 验收关注点

Android 打包命令：

```powershell
Set-Location project
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --release --lib -- --crate-type cdylib
Set-Location ..\android
.\gradlew.bat assembleDebug
```

Android 真机或模拟器上需要额外确认：

- `libproject.so` 能加载，首屏能进入预期页面。
- UI 字体能加载中文和英文文本。
- 触控按下、拖动、松开和 UI 按钮点击不冲突。
- 输入框能唤起软键盘，并能同步 native text input 状态。
- 状态栏、刘海、手势导航区域没有遮挡关键 UI。当前 safe area 仍是零值，需要真机观察。
- 横竖屏或窗口尺寸变化后 `UiViewport` 与 `UiMetrics` 正常刷新。

如果本机没有 `adb`，Android 设备级安装、渲染、触控和字体检查需要在有设备连接的环境完成。

当前 UI 审计还不支持 Android 真机截图和系统 UI 截图。Android 页面仍需要人工或未来远程 client 接入后，通过 `adminapi` 执行 `ui.screenshot`、`ui.read_viewport`、`ui.read_panels` 等命令验收。
