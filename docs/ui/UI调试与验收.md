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
$env:MYBEVY_UI_AUDIT_STATES="image_fit,visual_foundation,visual_acceptance,image_modes,image_tiling,image_atlas,typography,typography_overflow,icons,icon_states,style_scopes,effects,animations,components,component_checkboxes,component_toggles,component_segmented,component_overlays,component_tooltip,middle,bottom"
$env:MYBEVY_UI_AUDIT_EXIT_ON_FINISH="1"
cargo run -- --window-profile phone-small --window-scale 50%
```

输出目录会包含 `manifest.json`、`report.md`、`screenshots/` 和 `metadata/`。`ui_gallery` 的默认 runner 状态包含 `visual_acceptance` 以及图片、字体、图标、样式、效果、动画、组件专题和滚动状态；`image_fit` / `visual_foundation` 固定指向顶部，其余专题状态对齐稳定 child anchor，最后两个组件状态会确定性打开 Dropdown 与 Tooltip，兼容状态 `top` 仍可显式请求。其他页面默认只覆盖 `initial`。

metadata 的 `style_resolutions` 收集带 `UiStyleBinding` 实体的 `UiResolvedStyleDebugSnapshot`，包含 scope 链、请求 role/variant、来源链、最终关键 token、fallback 和错误码。它用于 F3/AI 审核边界读取最终解析结果，不包含可写业务状态。

metadata 的 `effect_resolutions` 收集带 `UiEffectBinding` 实体的 `UiResolvedEffectDebugSnapshot`，包含请求/最终 preset、实际组件、材质 reason、fallback 和 draw-call/overdraw 规划预算。材质结果是受控策略状态；当前没有 adapter 时不会把 fallback 记录成 shader 渲染成功。

metadata 的 `image_snapshots`、`font_snapshots` 和 `visual_summary` 汇总图片 mode/status、样式 scope/variant、字体 role/status、效果、动画与控件状态；`visual_budget` 记录节点、解码图片 payload、render primitive、额外 effect draw、材质和 overdraw 的开发期估算。统计口径与 profile 阈值见 [UI安全区与视觉预算.md](UI安全区与视觉预算.md)。

常规批量验收优先使用仓库根目录 runner：

```powershell
.\scripts\run-ui-audit.ps1 -SelfTest
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,phone-portrait,tablet-portrait,tablet-landscape -States visual_acceptance -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,tablet-landscape -States auto
```

`-DryRun` 只验证矩阵、报告和分析输入，不启动游戏、不生成真实截图。真实本地运行会为每个 screen + device 启动一次 `cargo run`，并把子进程输出写入本轮 `logs/`。

本地 capture 在滚动完成后固定等待 30 个渲染帧再请求截图。该窗口用于让首次使用的 Bevy UI gradient / box-shadow pipeline 完成准备；只检查 ECS metadata 或等待 5 帧可能得到“组件已存在但首张 PNG 仍是纯色”的假通过。

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
- `ui_document_gallery`, `ui-document-gallery`, `document_gallery`, `document-gallery`, `declarative_gallery`
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
- F3 的 `declarative documents` 是否能按 document/node 定位 source field path，并显示安全逻辑 source 和 effective style。

## 审计报告验收重点

成功报告需要确认：

- `manifest.json` 的 `status` 为 `passed` 或 dry-run 时为 `planned`。
- `report.md` 中每个 capture 都有对应 screenshot 和 metadata 链接；远程模式至少有 screenshot / metadata artifact URI。
- `analysis-input.json` 中每条 capture 的 `capture_id`、`screen`、`device`、`state` 能对应回 `manifest.json` 和 `report.md`。`Provider` 模式可读取 `ui_ai_visual_analysis_v1` 报告中的结构化 image evidence，并保留 region、reference element 和 node ID；原有 `Fixture`、`Auto` 与 `Off` 行为保持兼容。
- 有滚动 recipe 的 `ui_gallery` 默认覆盖 `visual_acceptance`、全部专题 state、六个 `component*` 状态、`middle` 和 `bottom`，并记录 `scroll_target_id = ui_gallery.main`；综合区、图片、文字、图标、样式、效果、动画和通用组件均使用 child anchor，不要依赖会随页面总高度变化的 `middle`。组件 metadata 还会输出稳定排序的 `control_snapshots`。
- 每个核心 capture 的 `visual_budget.status` 不应为 `exceeded`；`warning` 需要结合 finding 和截图决定是否接受。图片内存和 draw/overdraw 都是估算，不得写成 GPU 实测通过。
- 任意 UI Gallery capture state 第一次应用时，全部动画样例都应 seek 到 `0.625` 并 pause；后续 30 帧稳定等待内 scroll geometry、目标值、player 和 debug snapshot 不应继续 Changed。
- metadata 的 `motion_policy` 应与当前 `UiMotionPolicy` 一致；`animation_snapshots` 应按 Name/Entity 稳定排序，并记录 target、raw/eased progress、pause 与 `causes_layout_reflow`。
- 声明式页面 metadata 的 `document_nodes` 应按 document ID/node ID 稳定排序，并包含 schema version、source path、document path 和 effective style；不得出现盘符或本机绝对路径。
- capture metadata 的 `semantic_tree.schema_version = 3` 使用 `logical_pixels`、半开矩形和 1/64 像素规范化；`target_root_id` 只覆盖目标页面、同 owner 可见覆盖层和全局可见 Toast，不得混入其他 owner。
- `semantic_tree.nodes[].stable_id` 不包含 Bevy Entity。声明式节点使用 owner/panel/document/node 身份，传统节点使用稳定命名层级或同语义 sibling ordinal；节点同时输出父级、深度、可选实体 `Name` 和 `ComputedNode::stack_index()`，`capture_entity` 仅用于单次 capture 诊断。
- `semantic_tree.panels[]` 输出 panel/Toast root 的真实 `capture_entity`、可选实体 `Name` 和按 panel ID/kind 定位的 `likely_files`；overlay finding 不得用占位 Entity 或统一 overlays 目录代替真实定位。
- 节点 `clip_bounds` 是自身 bounds、viewport 和全部裁切祖先的交集。不可见、完全裁切和无语义纯布局节点不进入常规告警；可见语义节点的明显零尺寸在裁切跳过前检查。
- `semantic_tree.panels` 应能证明 active focus scope、focus suppression、Pickable 和 `UiInputState` 阻断。Loading 无可聚焦控件时焦点必须清空；Modal 上方只允许受控 Dropdown/Tooltip transient 层；Toast 不得阻断或捕获焦点。
- `ui_semantic_audit_v1` 的 hard failure 与视觉相似度、局部分数分开，高相似度不得抵消裁切、不可点击、滚动不可达或覆盖层输入错误；总四态门禁仍由后续聚合阶段负责。
- `ui_ai_visual_analysis_v1` 的每个 capture 必须同时绑定 reference、actual、overlay、heatmap、diff report、区域指标、semantic report schema v3、原始 UI metadata hash、允许差异和 privacy rect；diff artifact hash 与 region binding 任一错配都必须在 provider 调用前失败。图片先读取 header 并预留解码像素/字节预算，再对同一快照执行受限完整解码。在线 provider 只接收语义文本框与显式 privacy rect 遮罩后的内存图片副本；可见且未完全裁切的文本缺少有效 measured bounds 时拒绝上传。敏感 metadata 值按固定数量、单值字节和总字节上限去重收集，ASCII echo 不区分大小写、非 ASCII echo 精确匹配，结构 ID/路径保持可追踪。HTTP 禁止重定向并限制输出 token。AI 引用的 capture/image/region/node/file 必须可反查；报告没有 pass/降级 deterministic hard failure 的字段，且生成模型与审核模型独立记录。
- `ui_visual_gate_v1` 只消费由 path + SHA-256 绑定并通过版本、尺寸、aligned image hash、reference binding 和 hard-failure 保留校验的上游报告；capture 必须沿用唯一的 `capture_id == screen.device.state`。四态为 `passed`、`failed`、`needs_review` 和 `invalid`：证据无效优先于尺寸/语义 hard failure，随后依次保留 critical、AI severe、normal、AI medium 和 decorative review。critical/normal、AI severe/medium 阻断，decorative-only 进入人工复核，AI minor 仅报告。每个 reference profile 可覆盖完整六项区域阈值，未匹配时使用保守默认；Stage 6 local status/violations 作为独立 upstream diagnostics，不参与 selected profile 的 Stage 9 决策。每个区域的正式 profile 指标、阈值、违规项和决定都独立输出，不生成可掩盖局部失败的全局平均分。

失败报告需要能定位：

- task 级：`screen`、`device` 或远程目标、`states`、`failure_type`、stdout/stderr 或 remote task id。
- capture 级：`screen`、`device`、`state`、失败类型、截图/metadata/log 路径或 artifact URI。
- analysis 级：issue 必须包含 `screen`、`device`、`state`、severity、blocking、problem、evidence、likely cause 和 suggested files。

常见失败类型包括：

- 本地启动与输出：`launch_failed`、`timeout`、`manifest_missing`、`manifest_invalid`、`output_missing`、`process_failed`。
- 游戏内审计：`screen_not_found`、`panel_not_ready`、`unstable_ui`、`scroll_target_missing`、`scroll_target_unreachable`、`screenshot_failed`、`config_invalid`。
- 远程任务：`device_offline`、`debug_disabled`、`send_failed`、`client_timeout`、`client_rejected`、`artifact_upload_failed`、`remote_error`、`remote_failed`。
- AI 与修复：`ai_blocking_issue`、`deterministic_hard_failure`、`ai_missing_capture_metadata`、`ai_result_invalid`、`safety_policy_rejected`、`fix_check_failed`、`max_iterations_reached`。

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
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --release --lib --crate-type cdylib
Set-Location ..\android
.\gradlew.bat assembleDebug
```

Android 真机或模拟器上需要额外确认：

- `libproject.so` 能加载，首屏能进入预期页面。
- UI 字体能加载中文和英文文本。
- 触控按下、拖动、松开和 UI 按钮点击不冲突。
- 输入框能唤起软键盘，并能同步 native text input 状态。
- `viewport.safe_area.source` 在原生回调后为 `android_window_insets`，状态栏、display cutout、手势/三键导航区域没有遮挡关键 UI；IME 显隐不应改变全局 safe area。
- 横竖屏或窗口尺寸变化后 `UiViewport` 与 `UiMetrics` 正常刷新。

如果本机没有 `adb`，Android 设备级安装、渲染、触控、字体、图片切片和效果降级检查需要在有设备连接的环境完成。当前 2026-07-12 的开发环境即为此状态，真机项未完成；所需设备、API 和记录步骤见 [UI安全区与视觉预算.md](UI安全区与视觉预算.md)。

当前 UI 审计还不支持 Android 真机截图和系统 UI 截图。Android 页面仍需要人工或未来远程 client 接入后，通过 `adminapi` 执行 `ui.screenshot`、`ui.read_viewport`、`ui.read_panels` 等命令验收。
