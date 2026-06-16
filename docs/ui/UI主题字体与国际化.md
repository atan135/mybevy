# UI 主题字体与国际化

主题、字体和国际化是框架级资源，由 `UiFrameworkPlugin` 在页面插件之前注册。业务页面应通过资源和 helper 获取这些能力，不应硬编码另一套颜色、字号或文案加载逻辑。

## 主题资源

主题实现位于 `project/src/game/ui/style/theme.rs`。默认配置文件：

```text
project/assets/ui/themes/default.ron
```

主题配置版本当前是 `version: 1`。主要 token：

- `colors`：屏幕背景、面板背景、边框、Loading/Modal 遮罩、正文、弱化文本、错误色、主按钮色、次按钮色。
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

## 主题热更新

如果主题从文件加载成功，`UiThemePlugin` 会约每 0.8 秒轮询文件修改时间。解析成功后替换 `UiTheme`，解析失败则保留当前主题并记录 warning。

主题刷新依赖组件 marker：

- `UiThemeBackgroundRole`
- `UiThemeBorderRole`
- `UiThemeTextColorRole`
- `UiThemeTextStyleRole`
- `UiThemeButtonNodeRole`
- `UiThemePanelNodeRole`
- `UiThemeRootNodeRole`

页面和控件应保留这些 marker，这样主题、视口或 metrics 变化时节点会自动刷新。

## 字体资源

字体加载位于 `project/src/game/ui/style/fonts.rs`。当前唯一字体资源：

```text
project/assets/ui/fonts/MyBevyUiCjk-Regular.otf
```

运行时 AssetServer 路径是：

```text
ui/fonts/MyBevyUiCjk-Regular.otf
```

字体句柄保存在 `UiFontAssets { regular }`。文本 helper 默认使用这份 regular 字体。

当前字体定位是 CJK 常用 UI 字体子集，不覆盖扩展 CJK、emoji、日文、韩文、繁体专用字形，也没有粗体/斜体等多字重资源。

## 国际化资源

i18n 实现位于 `project/src/game/ui/i18n.rs`。默认目录：

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

i18n 文件成功加载后同样约每 0.8 秒轮询。热更新成功会替换 `UiI18n`，并刷新带 `UiI18nText` 的文本节点。热更新失败保留当前 i18n。

需要注意：输入框 helper 文案、校验消息等部分状态在组件里保存的是已解析字符串，并不一定都能在 locale 热更新后自动回刷。需要完全动态切换语言的表单，应优先保留 key 或在 locale 改变时重新生成对应页面。
