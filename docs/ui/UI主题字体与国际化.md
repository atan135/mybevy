# UI 主题字体与国际化

主题、字体和国际化是框架级资源，由 `UiFrameworkPlugin` 在页面插件之前注册。业务页面应通过资源和 helper 获取这些能力，不应硬编码另一套颜色、字号或文案加载逻辑。

## 主题资源

主题实现位于 `project/src/framework/ui/style/theme.rs`。默认配置文件：

```text
project/assets/ui/themes/default.ron
```

主题配置版本当前是 `version: 1`。主要 token：

- `colors`：屏幕背景、面板背景、边框、Loading/Modal 遮罩、正文、弱化文本、错误色、主按钮色、次按钮色，以及 `icon_tint` 七态单色图标着色。
- `text`：大标题、标题、副标题、章节、正文、说明、按钮字号。
- `layout`：页面 padding、页面间距、面板间距、行间距、内容宽度等。
- `button`：按钮最小宽度、高度、横向 padding、圆角。
- `panel`：面板 padding、边框、圆角。

加载优先级：

1. 环境变量 `MYBEVY_UI_THEME` 指定的路径。
2. `assets/ui/themes/default.ron`。
3. `project/assets/ui/themes/default.ron`。
4. `CARGO_MANIFEST_DIR/assets/ui/themes/default.ron`。
5. 内置 `UiTheme::default()`。

## 作用域样式与组件变体

`version: 1` 主题可选增加 `styles`。默认主题已显式登记 Gallery 验收 token；旧 version 1 文件没有 `styles` 时会迁移到同一份内置兼容样式，现有页面和旧 marker 无需一次性改写。

```ron
styles: (
    tokens: [
        (name: "page.surface", value: Color((r: 0.08, g: 0.19, b: 0.18))),
        (name: "control.radius", value: Scalar(6.0)),
    ],
    variants: [
        (
            name: "page.compact",
            extends: Some("page.base"),
            overrides: [
                SurfaceBackground(role: panel, token: "page.surface"),
                ButtonRadius(role: secondary, token: "control.radius"),
            ],
        ),
    ],
),
```

token 只有 `Color` 和 `Scalar` 两种类型。颜色通道必须是有限的 `0..=1`，尺寸类 Scalar 必须非负，字号必须大于零；框架不依赖 clamp 修正非法配置。override 属性是固定 serde enum，不解析页面自造的属性字符串。当前类型化角色包括：

- `UiSurfaceStyleRole`：screen、panel、elevated、overlay。
- `UiBorderStyleRole`：panel、control、emphasis。
- `UiTextStyleRole`：primary、caption、muted、error、button。
- `UiButtonStyleRole`：primary、secondary。
- `UiInputStyleRole`：standard、error。
- `UiCardStyleRole`：standard、emphasis。
- `UiDialogStyleRole`：standard、destructive。

解析顺序固定为：基础 role -> 请求引用的 variant 继承链 -> 从页面根到最近祖先的 scope 链。后应用者覆盖前者；嵌套 scope 因而胜过父 scope。移除 `UiStyleScope`、把实体移出子树或重新挂到其他父级后，下一次解析会按新祖先链恢复，不保存页面私有副本。

文字 role 同时表达颜色与字号语义：`Primary` 使用正文大小和主文字色，`Caption` 使用说明文字大小和主文字色，`Muted` 使用说明文字大小和弱化文字色。需要紧凑但仍强调的标签必须使用 `Caption`，不能用 `Primary` 再硬改字号，也不能用 `Muted` 冒充颜色语义。scope variant 可以分别覆盖这些 role 的颜色，不会把 Caption 提升为正文大小。

同一实体通常只放一个 composite role。确需组合时，静态字段提交优先级固定为 `Surface/Border/Text -> Button/Input -> Card -> Dialog`，后者胜出；Gallery 不用重叠 composite role 建立视觉样例。按钮和输入框的 resolver 只产生 `UiResolvedButtonStyle` / `UiResolvedInputStyle`，现有控件视觉系统继续唯一负责 Interaction、focused、selected、disabled、loading、error 和输入值，不会由主题刷新重置业务状态。

配置在应用前完整编译。稳定错误码包括 `ui_style_unknown_token`、`ui_style_unknown_variant`、`ui_style_variant_cycle`、`ui_style_duplicate_token`、`ui_style_duplicate_variant`、`ui_style_duplicate_override`、`ui_style_token_type_mismatch` 和 `ui_style_invalid_value`。任一错误都会让文件热更新失败并保留 last-known-good `UiTheme`，不会部分应用或 panic。

带 `UiStyleBinding` 的实体会得到只读 `UiResolvedStyleDebugSnapshot`，记录 scope 链、请求 role/variant、来源链、最终关键 token、fallback 和稳定错误码。UI audit metadata 的 `style_resolutions` 会收集这些快照；`style_scopes` capture state 对齐 Gallery 固定区域。

## 主题热更新

如果主题从文件加载成功，`UiThemePlugin` 会约每 0.8 秒轮询文件修改时间。基础字段和全部 styles 配置解析、引用与循环校验都成功后才替换 `UiTheme`；失败则保留 last-known-good 主题并记录 warning。主题或 metrics 更新会重新解析当前祖先 scope，稳定输入的第二帧不会重复标记 resolved component、Node 或颜色 Changed。

主题刷新依赖组件 marker：

- `UiThemeBackgroundRole`
- `UiThemeBorderRole`
- `UiThemeTextColorRole`
- `UiThemeTextStyleRole`
- `UiThemeButtonNodeRole`
- `UiThemePanelNodeRole`
- `UiThemeRootNodeRole`

页面和控件应保留这些 marker，这样主题、视口或 metrics 变化时节点会自动刷新。图标按钮的 `icon_tint` 由专用视觉系统读取 `UiTheme`，不需要额外 marker；全彩图标会忽略该 tint 并保持原色。

## 字体注册表

字体加载和选择位于 `project/src/framework/ui/style/fonts.rs`。`UiFontAssets` 保留 `regular` 作为旧页面兼容句柄，同时提供 family / weight / role / face / coverage 注册表。新公共 helper 不直接读取 `regular`，而是解析 `UiTextStyleToken`。

当前注册 face：

| family | weight | 路径 | 定位 |
| --- | --- | --- | --- |
| `ProductCjk` | Regular 400 | `ui/fonts/MyBevyUiCjk-Regular.otf` | 产品 UI CJK Regular；9,207,028 bytes |
| `FigtreeFixture` | Regular 400 | `ui/fixtures/fonts/FigtreeFixture-Regular.ttf` | Latin 开发验收 fixture；40,096 bytes |
| `FigtreeFixture` | Medium 500 | `ui/fixtures/fonts/FigtreeFixture-Medium.ttf` | Latin 开发验收 fixture；40,120 bytes |
| `FigtreeFixture` | Bold 700 | `ui/fixtures/fonts/FigtreeFixture-Bold.ttf` | Latin 开发验收 fixture；40,076 bytes |

Figtree 三个 face 是来自同一固定 Google Fonts revision 的真实静态实例，不是复制 Regular 或运行时合成粗体；但它们仍是开发 fixture，业务页面不得把 `FigtreeFixture` 当正式产品 family。来源、OFL、hash 和 coverage 见 `project/assets/ui/fixtures/`。产品 CJK face 的 hash、覆盖边界和历史 provenance 缺口见 `project/assets/ui/fonts/README.md`。

字体角色包括 `Display`、`Heading`、`Body`、`Caption`、`Control` 和 `LatinFixture`。每个角色明确声明 primary face、fallback face、预期 coverage，以及 Loading、Failed 和缺字行为。产品角色目前都以 CJK Regular 为 primary；请求 Medium/Bold 时会明确落到已注册 Regular，并报告 `WeightUnavailable`，不会合成笔画。

## 字体选择和 fallback

字体选择以整个 Text 节点为单位：

1. 请求 token 中的 family + weight face。
2. 请求 face 不存在时尝试角色 primary。
3. coverage 不足或资源加载失败时按角色 fallback 顺序选择。
4. 只选择能够覆盖整个节点内容的单一 face。

因此 `FigtreeFixture Bold` 的纯 Latin 文本会使用真实 700 face；同一 token 的中英文混排会将整个节点切到 CJK Regular。当前框架没有自动按字符拆分 `TextSpan`，也不声称具备逐字 fallback。

Loading 时保留所选 handle，等待 Bevy 完成加载；Failed 时尝试声明的 fallback；所有候选资源都失败时节点 fail-closed 为空并暴露 `UiFontResolutionStatus::Unavailable`。没有任何候选 coverage 的 grapheme 会显式替换为 `?` 并报告 `GlyphReplacement`，不会先显示 tofu 再把它记为成功。

## 文本样式 token

`UiTextStyleToken` 包含：

- `font_role`、`font_family`、`font_weight`
- `font_size`
- 相对或像素 `line_height`
- 左、中、右和 justified `alignment`
- word、character、word-or-character 和 no-wrap `wrap`
- none、clip 和 ellipsis `truncation`

这些字段分别映射到 Bevy 0.18.1 的 `TextFont`、`LineHeight` 和 `TextLayout`。`screen_title`、`screen_title_key`、`screen_label`、`screen_label_key` 已自动附加 token、字体解析状态和同步状态，旧调用签名不变。

`Clip` 不改原字符串。Bevy 的 overflow 裁切作用于子节点，因此调用方必须用 `try_ui_text_clip_frame(width, height)` 建立有明确尺寸的父 frame，再把 no-wrap Text 作为子节点；不能把 `Overflow::clip()` 直接放在 Text 自身并声称已裁切。`Ellipsis { max_graphemes }` 的预算包含省略号，按 Unicode grapheme cluster 截断，不按 UTF-8 字节或单个 code point 截断。非法字号、行高、零 ellipsis 预算和非法 clip frame 尺寸会返回稳定错误。

Bevy 0.18.1 当前公共映射不提供通用字距 token。框架也没有稳定的复杂富文本、自动逐字 font fallback、文字沿路径或高级排版效果。设计必须依赖这些效果时，先调整设计；必须资源化时可使用 `UiRasterizedTextSpec`，但路径必须位于 `project/assets/ui/`（运行时 `ui/`），并携带 accessible fallback、i18n fallback key 和 `ProjectOwned` 或完整授权来源。该入口只用于受控静态字图，不是通用声明式文字协议。

## 国际化资源

i18n 实现位于 `project/src/framework/ui/i18n.rs`。默认目录：

```text
project/assets/ui/i18n/
```

当前已有：

- `zh_cn.ron`
- `en_us.ron`

默认 locale 是 `zh_cn`。可用环境变量：

- `MYBEVY_UI_LOCALE`：指定 locale，会把 `-` 规范化为 `_`，例如 `en-US` -> `en_us`。
- `MYBEVY_UI_I18N`：直接指定 i18n RON 文件路径。

加载顺序：

1. `MYBEVY_UI_I18N` 指定路径。
2. 当前 locale 的 `assets/ui/i18n/<locale>.ron`、`project/assets/ui/i18n/<locale>.ron`、`CARGO_MANIFEST_DIR/assets/ui/i18n/<locale>.ron`。
3. 如果 locale 不是 `zh_cn`，再尝试 `zh_cn`。
4. 内置中文 fallback。

## 文案解析

业务代码通过 `i18n.tr(key, fallback)` 解析文案。如果当前 locale 缺 key，会优先使用内置中文 fallback；仍没有则使用调用点传入的 fallback；fallback 为空时显示 key。

文本节点如果带 `UiI18nText { key, fallback }`，`UiI18n` 变化后会自动刷新 `Text` 内容。控件 helper 的 `_key` 版本会自动附加这个组件。

## i18n 热更新

i18n 文件成功加载后同样约每 0.8 秒轮询。热更新成功会替换 `UiI18n`，并刷新带 `UiI18nText` 的文本节点。字体同步在 i18n 和主题刷新之后运行：locale 变化会重新解析全文 coverage 和 truncation，但保留字体角色、weight、line height、wrap、truncation 和 Node 约束。主题刷新只更新主题拥有的字号，不覆盖其余运行时排版字段。热更新失败保留当前 i18n。

需要注意：输入框 helper 文案、校验消息等部分状态在组件里保存的是已解析字符串，并不一定都能在 locale 热更新后自动回刷。需要完全动态切换语言的表单，应优先保留 key 或在 locale 改变时重新生成对应页面。
