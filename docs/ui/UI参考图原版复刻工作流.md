# UI 参考图原版复刻工作流

本文定义把一张 UI 参考图落地为可运行 Bevy 页面的方法。目标不是“风格相近”，而是在指定 viewport 下复刻构图、素材、线框、文字位置和控件状态，并使可点击区域成为真实 UI 控件。

`ai_login_reference` 是本流程的可运行样例：`project/src/game/screens/dev/ai_login_reference.rs`。

## 适用范围

适用于登录页、主菜单、角色选择、活动入口等具有强美术装饰的游戏页面，尤其是以下情况：

- 参考图中的背景、主体图标、装饰线框需要保持原画细节。
- 参考图中存在可点击文字或图块，但暂时不需要业务跳转。
- 页面需要在 hover、pressed、focus 等状态下提供真实反馈。
- 声明式 `UiDocument` 无法自然表达页面专属素材组合或局部动效时，需要使用受控的页面级 Bevy UI。

普通表单、列表或后台工具页面优先使用现有组件和 `UiDocument`。声明式 Grid、card/frame、文字角色、控件 slot/state、TextInput 和高级图片的具体配置见 [UI生成式组件配置参考.md](UI生成式组件配置参考.md)。不要为了一个常规按钮重写一整页原生 Bevy 节点。

## 完成定义

只有同时满足以下条件，页面才可称为“原版复刻”：

| 项目 | 必须满足的条件 |
| --- | --- |
| 构图 | 在目标 viewport 中，主视觉、面板、标题和可点击项的位置与参考图逐项对齐。 |
| 美术 | 背景、主体图、框线、纹理等使用有授权的原始或派生位图；不得用近似纯色线框替代复杂原图装饰。 |
| 控件 | 每一个可操作项都是 Bevy `Button`，不是背景图里不可点击的文字。 |
| 状态 | 至少有 idle、hovered、pressed；按下状态有颜色/边框变化和缩放或等效反馈。 |
| 干净图层 | 运行时不能看见参考图中烘焙的旧文字、旧按钮、旧水印或重复边框。 |
| 验收 | 基线、hover、pressed 三张运行时截图均经人工核对；窗口尺寸与参考图目标尺寸一致。 |

## 输入约束

开始前固定以下信息，缺失时不要先写布局：

1. 参考图文件、原始像素尺寸、目标逻辑 viewport 和目标平台。
2. 可保留、必须移除、必须可点击的元素清单。
3. 参考图和派生素材的来源、授权和是否允许进入正式包。
4. 是否只做交互反馈，或需要接入路由、账号、网络等业务动作。

本流程中的参考图仅在其来源和授权允许时，才能派生为 `project/assets/` 中的正式资源。来源不明的图片只允许放在本地 `artifacts/` 或被忽略的 `summary/` 中作对照，不能提交到游戏包。

## 分层与素材拆分

不要把完整参考图直接作为最终背景。完整截图中的文字和按钮无法独立交互，也会与新控件重叠。应按下表拆分。

| 层 | 产物 | 处理规则 |
| --- | --- | --- |
| 背景层 | `*_background.png` | 保留场景、光照和地面等大面积美术；清除水印及会被 UI 覆盖的烘焙控件。 |
| 主体层 | `*_sigil.png`、角色图或徽记 | 裁切主体，保留透明通道或与背景一致的边缘过渡；作为独立 `ImageNode` 放置。 |
| 面板层 | `*_panel_surface.png` | 从原图裁切装饰框和面板表面，移除原文字、旧分隔线与旧交互标记；保留复杂线框和纹理。 |
| 交互层 | Rust `Button` 和 `Text` | 新建真实控件、文字和状态视觉，覆盖在面板层上。 |

对于复杂线框，优先裁切并清理原图的面板区域，而不是重新用几条 `BorderColor` 近似绘制。九宫格只适用于边角和边线能独立拉伸的素材；带有连续纹理、亮度变化或不对称装饰的面板，应使用完整面板位图并按参考图比例放置。

### 坐标换算

所有裁切和布局先以原图像素为准，再换算到目标 viewport。原图尺寸为 `(source_width, source_height)`、目标 viewport 为 `(viewport_width, viewport_height)` 时：

```text
left_percent = source_left / source_width * 100
top_percent = source_top / source_height * 100
width_percent = source_width_of_node / source_width * 100
height_percent = source_height_of_node / source_height * 100
```

先按此结果建立绝对定位节点，再用运行时截图微调。不要依据肉眼猜测百分比，也不要把原图强行拉到不同宽高比来制造“相似”。如果目标 viewport 的宽高比不同，必须明确设计裁切、留黑或单独适配规则。

## 实现步骤

### 1. 建立页面模式和所有权

为页面添加专用 `AppUiMode`、`UiOwnerId`、`UiPanelId` 和开发页 setup。页面根节点需使用现有 `game_panel_root`、`UiLayerRoot` 和 `DespawnOnExit`，避免页面切换后遗留实体。

### 2. 将素材放入正式资源目录并记录来源

资源放入 `project/assets/ui/images/`，代码路径从 `ui/images/` 开始写。每个正式派生 PNG 都需要：

- 在同目录 provenance Markdown 中记录原图、处理目的和授权说明。
- 在 `tools/ui-generation/assets/ui_asset_catalog.v1.json` 中记录路径、SHA-256、字节数、尺寸、alpha 和 license reference。
- 让二进制资源命中仓库 Git LFS 规则。

`ai_login_reference` 的背景、水滴和面板分别使用 `ai_login_background.png`、`ai_login_sigil.png`、`ai_login_panel_surface.png`，来源记录在 `project/assets/ui/images/ai_login_reference_assets.md`。

### 3. 按固定 z-index 组装视觉层

根节点裁切溢出；背景、水滴、面板和按钮容器按固定层级创建。图片节点必须 `Pickable::IGNORE`，防止装饰层抢走按钮命中。

```rust
root.spawn((
    Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() },
    ImageNode::new(background).with_mode(NodeImageMode::Stretch),
    Pickable::IGNORE,
    ZIndex(0),
));

root.spawn((
    Node { width: percent(24.5), height: percent(87.5), position_type: PositionType::Absolute,
           right: percent(0.0), top: percent(6.25), ..default() },
    ZIndex(10),
));
```

面板素材必须完全覆盖原背景中相同位置的烘焙菜单。若截图中仍能看到旧文字，先修正面板的范围、透明度或背景清理，不要通过降低新文字透明度掩盖问题。

### 4. 用真实按钮覆盖交互项

无业务逻辑不等于没有控件。每个可点击项至少使用 `Button`、`FocusableButton`、页面专属 marker 和 `UiTransform`。视觉状态由 `Interaction` 驱动，按下和松开由既有 `UiButtonEvent` 驱动动画命令。

```rust
commands.spawn((
    Button,
    FocusableButton,
    ReferenceButton,
    UiTransform::default(),
    Node {
        width: percent(100),
        height: px(68),
        min_height: px(60),
        border: UiRect::vertical(px(1)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    },
    BackgroundColor(Color::NONE),
    BorderColor::all(Color::NONE),
));
```

推荐状态：idle 保持原画视觉；hover 使用克制的表面 tint 和亮化边线；pressed 使用更明确的 tint、亮边和 `UiAnimationSpec::transform_scale`。本项目登录样例采用 `1.0 -> 0.96`、`0.07s` 的按下缩放，以及 `0.96 -> 1.0`、`0.13s` 的松开回弹。没有业务需求时，`Click` 不路由、不联网、不修改账号状态。

若使用能力矩阵中标为“允许直接使用 Bevy”的特性，按 [UI高保真视觉能力.md](UI高保真视觉能力.md) 的规则附加 `UiDirectBevyVisual` 及原因；不要借此绕过框架已有的公共控件、主题或动画能力。

### 5. 添加最小可回归测试

为按下/松开动画的 target、时长和缩放值写 focused unit test。页面新增 mode 时，应同时测试 alias 解析和 owner 归属。测试不替代视觉检查，但能防止后续重构移除交互反馈。

## 预览与验收

在 `project/` 下以固定窗口启动页面：

```powershell
$env:TOUCH_START_SCREEN = "ai_login_reference"
cargo run -- --window-size 1280x720
```

运行窗口中按 `F9` 会将截图写入 `summary/ui-audit/manual/`。至少保存并人工比较三种状态：

1. idle：检查素材位置、原图线框、水滴和文字，没有旧烘焙 UI 残留。
2. hovered：鼠标进入每个按钮，检查高亮只影响当前项，布局和可点击区域不跳动。
3. pressed：按住鼠标，检查表面/边线变化和缩放；松开后必须恢复 idle 或 hover 状态。

提交前执行：

```powershell
Set-Location project
cargo fmt --check
cargo test --lib ai_login_reference # 替换为当前页面模块名
cargo check

Set-Location ..
cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .
pwsh -NoProfile -File scripts/test-ui-supply-chain.ps1
git diff --check
```

## 常见失败与修复顺序

| 症状 | 原因 | 修复 |
| --- | --- | --- |
| 新文字与旧文字重叠 | 完整参考图仍含烘焙菜单，或面板遮罩半透明/尺寸不足 | 清理背景或使用不透明的清理后面板素材，按原图坐标扩大覆盖区域。 |
| 线框看起来“像但不对” | 用代码近似重画了复杂装饰 | 从有授权的原图裁切并清理线框，保留原像素细节；必要时改为完整面板贴图。 |
| 按钮没有 hover 或按下反馈 | 文字只是图片/`Text`，或装饰层参与拾取 | 使用真实 `Button` 和 `FocusableButton`；装饰图加 `Pickable::IGNORE`。 |
| 按下后尺寸回不来 | 没有处理 `Up`/`Cancel`，或动画从固定起点重新开始 | 用 `continue_from_current` 衔接 release 动画，覆盖 `Up` 和 `Cancel`。 |
| 不同尺寸画面错位 | 直接使用像素常量或拉伸了不同宽高比参考图 | 先按原图比例换算，再为不同 viewport 建立明确的响应式或独立适配规则。 |
| 资源无法进入正式包 | 未记录来源/hash/license 或资源目录错误 | 补齐 catalog 与 provenance，确认资源位于 `project/assets/` 并命中 LFS。 |

## 后续任务模板

后续请求可以按以下格式提交，避免实现目标不明确：

```text
参考图：<本地路径或已授权素材>
目标 viewport：<例如 1280x720>
页面用途：<登录 / 主菜单 / 弹窗>
必须原样保留：<背景、水滴、右侧线框>
必须拆成真实控件：<选择服务器、开始游戏、账号管理>
交互：hover 高亮、pressed 缩放回弹；暂不接业务逻辑
允许修改：<仅可移除旧文字/水印和分隔线>
验收：idle、hover、pressed 截图均与参考图逐项核对
资源授权：<来源和可进入正式包的确认>
```

满足该模板并完成上面的验收后，页面才可以进入后续业务接线或正式内容审批。
