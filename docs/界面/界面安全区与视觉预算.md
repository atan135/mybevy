# UI 安全区与视觉预算

本文记录 UI 框架的 Android 安全区数据链、桌面确定性模拟、响应式验收范围和开发期视觉预算。这里的预算用于发现明显超限，不替代 Android GPU 分析器或真机截图。

## Android 生产安全区

生产数据链如下：

1. `MainActivity.onApplyWindowInsets` 接收 GameActivity 已注册的 `WindowInsetsCompat` 回调，并先调用 `super` 保留 GameActivity 的原生事件和 IME 行为。
2. Activity 合并 `systemBars`、`displayCutout`、`mandatorySystemGestures` 和当前 `systemGestures`，排除 IME；`getInsetsIgnoringVisibility` 保证沉浸式系统栏临时显隐时页面不跳动。
3. `nativeOnWindowInsetsChanged(left, right, top, bottom)` 通过 JNI 将物理像素写入 Rust 线程安全邮箱。相同值不会增加 revision。
4. `UiViewportPlugin` 每帧按当前 primary window 的物理尺寸和 `scale_factor` 转为逻辑像素。旋转、resize 或 density 变化即使没有新 revision，也会重新换算。
5. 首个原生回调到达前，`UiSafeAreaStatus.source` 为 `unavailable` 且 inset 为零；回调后为 `android_window_insets`。非法 scale/零窗口不会产生非有限值，超出窗口的 inset pair 会按比例收敛到窗口边界。

Activity 在 `onCreate` 使用 edge-to-edge 和 `LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES`，并在 focus 恢复、`onResume` 和 `onConfigurationChanged` 后请求重新分发 insets。

## 桌面模拟

桌面模拟与 Android 生产来源分开记录：

- `desktop_profile_fixture`：phone/tablet profile 自带确定性逻辑像素 fixture，用于截图审计安全区 padding。
- `desktop_command_line_override`：`--safe-area-insets LEFT,RIGHT,TOP,BOTTOM` 显式覆盖，单位为逻辑像素；允许全零以测试无安全区布局。
- `unavailable`：desktop profile 或没有显式 override 时为零。

示例：

```powershell
Set-Location project
cargo run -- --window-profile phone-small
cargo run -- --window-size 720x1600 --device-scale 2 --safe-area-insets 0,0,24,20
cargo run -- --window-profile tablet-landscape --safe-area-insets 24,24,0,16
```

桌面 fixture 只能验证布局策略，不能证明 OEM cutout、三键导航、手势导航或系统栏控制器行为正确。

## 响应式策略

宽度仍由 `Compact / Medium / Expanded` 分类，高度由 `Short / Regular / Tall` 分类，方向由 `Portrait / Landscape` 分类。`ui_adaptive_grid` 在宽度列数基础上增加两条约束：

- `Short` 高度最多使用两列，减少横向内容在低高度窗口中的不可达风险。
- `Expanded + Portrait` 最多使用两列，避免宽平板竖屏出现过宽的四列内容。

UI Gallery 的 `visual_acceptance` state 在一个固定 anchor 内组合 Cover 背景、九宫格、Regular/Medium/Bold fixture、正式图标、阴影与渐变、Selected/Loading/Disabled 控件。业务模块只组合公共 helper，不复制图片计算、字体 fallback、效果降级或按钮状态优先级。

## 开发期预算

`UiVisualBudgetReport` 按 viewport 宽度选择 profile。达到限制的 80% 产生 `warning`，大于限制产生 `exceeded`；finding 按固定 metric 顺序输出。

| 指标 | Compact | Medium | Expanded | 统计口径 |
| --- | ---: | ---: | ---: | --- |
| UI node | 1800 | 2000 | 2200 | `UiStats.ui_node_count` |
| 图片解码内存估算 | 64 MiB | 80 MiB | 96 MiB | 当前 UI `ImageNode` handle 去重后，累加已解析 `Assets<Image>::data.len()` |
| render primitive 估算 | 1800 | 2000 | 2200 | 可见 UI node 加额外 effect draw 上界的保守值 |
| 额外 effect draw-call 上界 | 32 | 40 | 48 | 所有 `UiResolvedEffectDebugSnapshot.applied_draw_call_upper_bound` 之和 |
| material 数估算 | 4 | 6 | 8 | 一个标准 UI material 加去重后的自定义 material request ID |
| 单 effect overdraw layer 上界 | 4 | 5 | 6 | 所有效果预算中的最大 `overdraw_layers` |

图片字段不是 GPU VRAM，未包含驱动 row padding、mipmap、纹理副本或压缩格式差异；draw-call 和 overdraw 字段也不是 GPU 实测。发布前仍需在目标 Android GPU 上用平台分析器确认 batching、透明层和峰值显存。

## 审计 metadata

每个 capture 除原有 style/effect/animation/control 快照外，还输出：

- `viewport.safe_area`：逻辑/物理 inset、source 和 revision。
- `image_snapshots`：请求 presentation、Bevy image mode、状态和单资源解码字节估算。
- `font_snapshots`：字体 role、请求/解析 family/weight 和 fallback 状态，不输出文本内容。
- `visual_summary`：稳定排序的图片 mode/status、style scope/variant、font role/status、effect/material、animation policy/state 和 control kind/state 计数。
- `visual_budget`：profile、口径、limits、usage、status 和稳定 findings。

四个核心 profile 的综合审计命令：

```powershell
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small,phone-portrait,tablet-portrait,tablet-landscape -States visual_acceptance
```

Runner 成功只代表进程、截图与 metadata 产出成功；仍需逐图检查文字、裁切、触控尺寸和浮层边界，并确认 `visual_budget.status` 不是 `exceeded`。

## Android 真机验收记录

截至 2026-07-12，当前开发环境执行设备探测时 `adb` 不在 `PATH`，没有可用的授权 Android 设备。因此以下真机项尚未完成，桌面 fixture、Rust 转换测试、Android Rust 编译和 APK 构建都不能替代它们：

- 状态栏、display cutout、手势导航和三键导航的物理 inset。
- 横竖屏、后台恢复、临时显示系统栏和 density 变化后的 revision/逻辑像素更新。
- IME 显隐不改变全局安全区。
- CJK/Latin 字体实际渲染、触控按下/拖动/松开、九宫格和 tiled 图片。
- 阴影/渐变显示以及自定义材质在 Android 上的可见降级。

补验环境至少需要 API 31、一个有 cutout 或手势导航的授权设备、可用 `adb`、本轮 arm64 `libproject.so` 和 Debug APK。安装后进入 UI Gallery，分别在横竖屏打开 `visual_acceptance`、`image_modes`、`effects` 和组件浮层，记录设备型号/API、APK 构建、截图与 metadata safe-area source/revision。
