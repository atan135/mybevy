# UI 响应式布局

响应式布局由 `UiViewport` 和 `UiMetrics` 驱动。页面不直接根据窗口像素写大量分支，而是读取框架推导出的尺寸分类、方向、安全区和指标。

## UiViewport

`project/src/framework/ui/core/viewport.rs` 定义 `UiViewport`：

```rust
pub struct UiViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub window_logical_width: f32,
    pub window_logical_height: f32,
    pub device_width: f32,
    pub device_height: f32,
    pub device_scale: f32,
    pub preview_scale: f32,
    pub width_class: UiWidthClass,
    pub height_class: UiHeightClass,
    pub orientation: UiOrientation,
    pub input_mode: UiInputMode,
    pub safe_area: UiSafeArea,
}
```

桌面端如果使用窗口 profile，会优先使用启动配置推导逻辑设备尺寸和确定性 safe-area fixture；Android 使用运行时窗口尺寸和原生 `WindowInsetsCompat`。

## 尺寸分类

宽度分类：

- `Compact`：`logical_width < 480`
- `Medium`：`480 <= logical_width < 840`
- `Expanded`：`logical_width >= 840`

高度分类：

- `Short`：`logical_height < 600`
- `Regular`：`600 <= logical_height < 800`
- `Tall`：`logical_height >= 800`

方向：

- `Portrait`：`logical_height >= logical_width`
- `Landscape`：否则为横屏

## UiMetrics

`UiMetrics::from_viewport_and_theme` 根据视口和主题推导常用 UI 指标：

- `page_padding`
- `panel_padding`
- `control_gap`
- `section_gap`
- `button_height`
- `input_height`
- `icon_size`
- `touch_target_min`
- `font_body`
- `font_button`
- `font_title`
- `content_max_width`
- `dialog_max_width`

触控/鼠标混合模式下 `touch_target_min` 当前是 44，鼠标键盘模式是 40。按钮、输入框和图标按钮应至少满足这个最小触控目标。

## 安全区

`UiSafeArea` 提供 `left/right/top/bottom`，并支持 `viewport.safe_area_padding(base)`。主题 root marker 会自动把安全区加入页面和覆盖层 padding。

Android Activity 将状态栏、display cutout、导航栏和手势区域的物理 inset 通过 JNI 发布，viewport 按当前 device scale 转为逻辑像素。首个回调前 source 为 `unavailable`；有效回调后为 `android_window_insets`。IME 不进入安全区，避免软键盘弹出导致整页 padding 跳变。

phone/tablet 桌面 profile 带有明确的 `desktop_profile_fixture`，也可用 `--safe-area-insets LEFT,RIGHT,TOP,BOTTOM` 覆盖逻辑像素。fixture 只用于确定性布局审计，不冒充真机结果。完整数据链和失败边界见 [UI安全区与视觉预算.md](UI安全区与视觉预算.md)。

## 布局 helper 使用原则

页面内容优先使用：

- `ui_content_container(metrics)`：限制正文最大宽度并居中。
- `ui_responsive_grid(metrics, viewport.width_class, UiResponsiveGridColumns::new(...))`：按宽度分类切换列数。
- `ui_adaptive_grid(metrics, viewport, UiResponsiveGridColumns::new(...))`：同时考虑宽度、方向和 `Short` 高度；Short 与 Expanded 竖屏最多两列。
- `ui_action_row(metrics, viewport.width_class)`：紧凑屏从左侧开始并允许换行，宽屏靠右。
- `ui_scroll_column_with_max_height(...)`：长内容限制高度并启用滚动。

固定格式区域应设置稳定尺寸，例如按钮高度、图标按钮宽高、步进器值宽度、弹窗最大宽度，避免 hover、loading、文本变化引起布局跳动。

## 桌面窗口验收命令

常用窗口 profile：

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
cargo run -- --window-profile phone-small --safe-area-insets 0,0,30,24
```

可配合 `TOUCH_START_SCREEN` 直接进入页面：

```powershell
$env:TOUCH_START_SCREEN="gallery"
cargo run -- --window-profile phone-small --window-scale 50%
```

## 当前验收关注点

- Login、Lobby、UI Gallery、Touch Ripple HUD 在紧凑竖屏下不应有文字重叠或不可触达按钮。
- Confirm 和 Loading 打开后下层页面不应响应。
- Floating 在紧凑屏下不应超出安全边界。
- `visual_acceptance` 在 Compact、Medium、Expanded、横竖屏和 Short 高度策略下不应出现固定控件缩小、文字覆盖或横向越界。
- UI Gallery 的长列表和 Confirm 正文应可滚动。
- Touch Ripple 页面在 UI 未阻断时应能正常按下、拖动并生成水波纹；UI 弹层打开时 gameplay 输入应被阻断。
