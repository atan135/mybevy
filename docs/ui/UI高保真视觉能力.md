# UI 高保真视觉能力

本文定义参考图复刻时使用的视觉能力分类、支持状态和逃生口。它描述当前公共能力，不是阶段任务清单。能力实现变化时，需要同步更新本文、UI Gallery 样例和对应测试。

## 支持状态

| 状态 | 稳定 ID | 判定标准 | 页面侧规则 |
| --- | --- | --- | --- |
| 框架支持 | `framework_supported` | `project/src/framework/ui/` 已提供稳定数据模型或 helper、运行时行为和已知失败边界，并有测试或 Gallery 样例 | 业务页面只组合公共能力，不复制底层计算或状态优先级 |
| 允许直接使用 Bevy | `direct_bevy_allowed` | Bevy 0.18.1 已有足够稳定的原语，但框架尚未形成可复用约定；该用法能限制在单个页面或试验区域 | 根实体必须附加 `UiDirectBevyVisual::new(capability, reason)`，并在页面测试或 Gallery 中留有可复现样例 |
| 暂不支持 | `unsupported` | Bevy 原语、平台能力、性能预算或当前框架边界不足以给出可验证结果 | 不得静默近似；应调整设计、使用有明确许可的资源化结果，或等待公共能力补齐 |

判定顺序固定为：先查公共 API，再判断是否满足受控的 Direct Bevy 条件，最后才归为暂不支持。`direct_bevy_allowed` 是可追踪的临时边界，不等于框架兼容承诺。

视觉能力和状态的稳定代码模型位于 `project/src/framework/ui/visual.rs`。Direct Bevy 逃生口示例：

```rust
commands.spawn((
    bevy_visual_bundle,
    UiDirectBevyVisual::new(
        UiVisualCapability::Shadow,
        "screen-local comparison before a shared shadow token exists",
    ),
));
```

`reason` 不能为空。暂不支持的能力不能通过该 marker 绕过边界。

## 能力矩阵

| 分类 | 当前能力 | 状态 | 当前边界或公共入口 |
| --- | --- | --- | --- |
| 布局 `layout` | 响应式列、网格、滚动、视口分类和安全区 padding | 框架支持 | `widgets/layout.rs`、`widgets/scroll.rs`、`core/viewport.rs` |
| 布局 `layout` | 页面专属绝对定位或 Transform 组合 | 允许直接使用 Bevy | 仅限局部组合，必须标记；不得复制通用响应式计算 |
| 文字 `typography` | Regular 字体、主题字号角色、颜色角色、换行和 i18n 刷新 | 框架支持 | `style/fonts.rs`、`style/theme.rs`、`i18n.rs` |
| 文字 `typography` | Medium/Bold 注册、字体 fallback、字距、截断策略和复杂富文本 | 暂不支持 | 多字重 fixture 已准备，但尚未进入运行时字体注册表 |
| 图片 `image` | `Natural`、`Stretch` 和固定/百分比/宽高比容器 | 框架支持 | `widgets/image.rs` |
| 图片 `image` | `Contain`、`Cover`、焦点裁切和统一加载失败占位 | 暂不支持 | 不得在业务页面复制裁切计算 |
| 切片 `slice` | `NodeImageMode::Sliced`、`Tiled` 的页面级试验 | 允许直接使用 Bevy | 必须标记；尚无序列化边距、最小尺寸和组合校验 |
| 图标 `icon` | 使用 `ImageNode` 的页面级正式图片图标 | 允许直接使用 Bevy | 必须保留可访问名称并标记；文本符号只用于当前开发样例 |
| 图标 `icon` | 稳定图标 ID、着色边界和缺失图标占位 | 暂不支持 | 尚无图标注册表 |
| 表面 `surface` | 主题色驱动的纯色页面、面板和按钮背景 | 框架支持 | `UiThemeBackgroundRole` 和组件 helper |
| 边框 `border` | 统一边宽、纯色边框和圆角 | 框架支持 | `UiThemeBorderRole` 和组件 helper |
| 边框 `border` | 独立边宽、复杂轮廓或页面专属组合 | 允许直接使用 Bevy | 必须标记并验证裁切；渐变边框不属于此逃生口 |
| 阴影 `shadow` | 页面级 `BoxShadow` / `TextShadow` 对照试验 | 允许直接使用 Bevy | 必须标记并记录移动端验证；尚无共享 token 或降级规则 |
| 渐变 `gradient` | 页面级 Bevy 渐变对照试验 | 允许直接使用 Bevy | 必须标记；尚无色标限制、边框组合或平台降级规则 |
| 动画 `animation` | 框架现有 alpha 动画和覆盖层入场 | 框架支持 | `core/animation.rs` |
| 动画 `animation` | 页面级 Transform 动画试验 | 允许直接使用 Bevy | 必须标记；不得冒充具备取消、主题刷新或减少动态效果语义的公共动画 |
| 动画 `animation` | 通用位置、尺寸、缩放、颜色过渡协议 | 暂不支持 | 尚无统一目标、easing 和取消模型 |
| 控件状态 `control_state` | 按钮 idle/hovered/pressed/focused/selected/disabled/loading 视觉优先级 | 框架支持 | `widgets/controls/button.rs` |
| 控件状态 `control_state` | Checkbox、Toggle、Segmented 的当前轻量状态结构 | 框架支持 | 当前仍是按钮式视觉，限制见 `UI当前限制.md` |
| 控件状态 `control_state` | 状态贴图、Badge、Progress、Tab、Tooltip 和下拉选择 | 暂不支持 | 不在业务页面建立私有状态优先级协议 |

## 稳定验收区域

UI Gallery 的第一个内容面板是固定的 `visual foundation` 区域，代码 marker 为 `GalleryVisualFoundationRegion`。该区域只展示阶段 1 fixture，不把未来能力伪装成已实现能力。

- 页面：`ui_gallery`，别名 `ui-gallery`、`gallery`
- 审计 state：`visual_foundation`
- 滚动目标：`ui_gallery.main`
- 位置：主滚动容器顶部
- fixture 清单：`project/assets/ui/fixtures/manifest.ron`

批量 runner 的 `-States auto` 会为 UI Gallery 选择 `visual_foundation,middle,bottom`。仍可显式请求兼容 state `top`。

```powershell
.\scripts\run-ui-audit.ps1 -Screens ui-gallery -Devices phone-small -States visual_foundation -DryRun
```

后续能力应扩展此固定区域或增加新的命名 state；不要依赖内容总高度计算出的 `middle` 作为唯一高保真基线。

## 资源与许可

- 阶段 fixture 放在 `project/assets/ui/fixtures/`，正式业务资源不要依赖 fixture ID 或路径。
- 图片、字体等二进制继续命中仓库 `.gitattributes` 中的 Git LFS 规则；RON、Markdown 和许可文本保持普通 Git 文件。
- 每项 fixture 必须在 `manifest.ron` 和 `LICENSES.md` 中记录用途、尺寸或字重、来源、许可和固定上游版本。
- 仓库自产几何 fixture 不含外部参考图像素；外部字体随附原始许可文本。
- 来源或许可不明确的参考图只能用于本地对照，不能进入 `project/assets/`、APK 或正式游戏资源。

## 非目标

本能力模型不定义 AI 模型调用、参考图理解、声明式 UI 文档协议、图像差异算法、完整样式语言或自定义 shader 白名单。fixture 也不是正式美术资产，不用于建立产品视觉风格。
