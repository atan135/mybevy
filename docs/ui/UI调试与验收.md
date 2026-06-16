# UI 调试与验收

UI 调试能力集中在 `project/src/game/ui/debug.rs`。调试面板用于观察 viewport、metrics、输入路由、面板栈、UI 树和布局边界。

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

## 启动目标页面

可用 `TOUCH_START_SCREEN` 直接进入目标页面：

```powershell
Set-Location project
$env:TOUCH_START_SCREEN="gallery"
cargo run -- --window-profile phone-small --window-scale 50%
```

支持值见 [UI模式与面板层级.md](UI模式与面板层级.md)。

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

## 当前已知窗口验收状态

近期窗口级检查覆盖过：

- Login、Lobby、UI Gallery、Touch Ripple 在 `phone-small --window-scale 50%` 下视觉可用。
- UI Gallery 在平板横屏 profile 下视觉可用。
- Confirm 打开后下层页面输入被阻断。
- F3 调试面板可显示 viewport、metrics、input route、panels、stats、tree 和 layout 信息。
- Touch Ripple 拖动可生成水波纹拖尾。

桌面运行时如果出现 Vulkan validation layer 缺失 warning，属于本机图形环境提示，不是项目 UI 逻辑错误。

## Android 验收关注点

Android 打包命令：

```powershell
Set-Location project
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs build --release
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
