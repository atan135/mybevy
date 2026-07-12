# 01. UI 高保真视觉基础能力 Checklist

## 目标

补齐当前 Bevy UI 框架在参考图高保真复刻方面的通用视觉能力，使页面开发和后续 AI 生成流程能够通过稳定组件、样式 token 和资源描述表达常见游戏 UI，而不需要在每个业务页面重复硬编码底层 Bevy 节点。

本清单只建设视觉与交互基础能力，不包含 AI 模型调用、参考图理解、声明式 UI 文档协议和图像差异审核算法；这些内容由其他清单负责。

## 已有基础与依赖

- 已有 `UiTheme`、`UiMetrics`、宽高分类、方向、安全区数据结构和主题热更新。
- 已有按钮、文本输入、选择控件、数值控件、滚动、图片、Panel Manager、覆盖层和 UI Gallery。
- 已有本地截图、页面审计、设备矩阵和报告能力，不在本清单重复实现。
- 本清单是 `02_UI声明式描述与运行时生成_checklist.md` 和 `03_AI参考图生成UI_checklist.md` 的前置能力之一。

## 基础原则

- [x] 优先扩展 `project/src/framework/ui/` 的通用能力，业务页面只负责组合，不复制框架逻辑。（验证：图片、字体、图标、样式、效果、动画、组件、安全区和审计能力均落在 `framework/ui/`；`game/screens/dev/ui_gallery.rs` 仅组合公共 helper/preset）
- [x] 每项视觉能力同时定义数据模型、运行时行为、失败边界、示例和测试。（验证：阶段 1-10 的逐项记录均包含类型化模型、运行系统或 builder、稳定错误/降级、Gallery state 与 focused tests；最终完整 lib tests 1415/1415）
- [x] 只抽象真实参考图需要的能力，避免建立无法验证的全能样式系统。（验证：`UI高保真视觉能力.md` 的 11 类能力矩阵限定受支持/Direct Bevy/暂不支持边界，九宫格、效果、材质和动画均采用受限配置与显式拒绝）
- [x] 保留直接使用 Bevy UI 原语的逃生口，但需要通过稳定组件或 helper 标记非标准用法。（验证：`UiDirectBevyVisual` 要求 capability 与非空 reason，构造/失败测试通过；综合验收区无需该 marker）
- [x] 新增二进制 UI 资源放入 `project/assets/ui/` 并遵守 Git LFS 约定。（验证：fixture 字体/图片、产品字体和正式图标均位于 `project/assets/ui/`，最终 `git lfs ls-files` 列出对应 PNG/TTF/OTF）
- [x] 每个阶段独立实现、独立验证、独立提交，并同步检查 `docs/ui/`。（验证：`db95cd8` 至 `982a32d` 共 10 个顺序独立 `feat(ui)` 提交分别对应阶段 1-10，各阶段均记录测试/截图并同步 UI 文档）

## 阶段 1：视觉能力模型和验收样例

- 开始时间：2026-07-11 22:00:45 +08:00
- 结束时间：2026-07-11 22:36:24 +08:00
- 开发总结：新增 11 类视觉能力与三态支持模型、Direct Bevy 逃生口 marker、UI Gallery 固定视觉基础区域和 `visual_foundation` 审计 state；加入 4 张确定性图片及 Figtree 400/500/700 真字重 fixture，并记录哈希、固定上游 revision、OFL 许可和 LFS 边界。
- 验证记录：主 agent 运行 `cargo test --lib visual`（76 passed）、`cargo test --lib ui_gallery_audit_recipe_registers_scroll_capture_states`（1 passed）、`cargo fmt --check`、`cargo check`、`.\scripts\run-ui-audit.ps1 -SelfTest`、phone-small `visual_foundation` dry-run 和 `git diff --check` 均通过；dry-run manifest 为 `planned` 且 state 为 `visual_foundation`，7 个二进制 fixture 哈希与 manifest 一致并命中 Git LFS。`cargo check` 仅保留仓库既有 `checkbox` dead-code warning。提交：`db95cd8`。

- [x] 建立参考图视觉能力分类，至少覆盖布局、文字、图片、切片、图标、表面、边框、阴影、渐变、动画和控件状态。（验证：`project/src/framework/ui/visual.rs:5` 定义 11 个稳定 capability ID；`cargo test --lib visual` 通过）
- [x] 为每种能力定义“框架支持、允许直接使用 Bevy、暂不支持”三种状态及判定标准。（验证：`project/src/framework/ui/visual.rs:40` 定义三态模型，`docs/ui/UI高保真视觉能力.md` 记录判定顺序、矩阵和失败边界）
- [x] 在 UI Gallery 规划一组稳定的高保真验收区域，确保后续阶段有固定页面和审计 state。（验证：`project/src/game/screens/dev/ui_gallery.rs:219` 生成首个固定区域，`project/src/game/navigation/mod.rs:256` 注册 `visual_foundation`；phone-small runner dry-run manifest 为 `planned`）
- [x] 准备小型测试资源，覆盖透明边缘、非等比图片、九宫格边框、图集帧和多字重字体。（验证：`project/assets/ui/fixtures/manifest.ron` 登记 4 张 PNG 与 Figtree 400/500/700 三个静态真字重；Gallery manifest/文件测试通过）
- [x] 明确资源版权、来源和 Git LFS 记录要求，不把未知来源的参考图素材当作正式游戏资源。（验证：`project/assets/ui/fixtures/LICENSES.md` 固定 Google Fonts revision、OFL 和自产图片来源；7 个 PNG/TTF 均经 `git check-attr` 确认 `filter=lfs`）
- [x] 更新 `docs/ui/UI组件功能与使用.md` 或新增对应框架说明，记录能力矩阵和非目标。（验证：新增 `docs/ui/UI高保真视觉能力.md`，并由 `docs/ui/README.md`、组件、限制和调试文档交叉引用）
- [x] 运行 `git diff --check`，确认文档和资源清单无路径或格式错误。（验证：主 agent 运行 `git diff --check` 退出码 0，仅显示既有 LF/CRLF 转换提示）

## 阶段 2：图片适配、裁切和焦点定位

- 开始时间：2026-07-11 22:38:00 +08:00
- 结束时间：2026-07-11 23:37:43 +08:00
- 开发总结：将图片能力扩展为 Natural、Stretch、Contain 和带归一化焦点的 Cover；新增结构化尺寸约束、稳定错误码、Loading/Ready/Failed/Invalid 状态、圆角 frame helper 与 PostLayout 运行时系统。Gallery 顶部加入四模式横竖 frame 矩阵并注册 `image_fit` 审计 state；第 1/5 轮返工消除了稳定图片每帧错误触发 ECS Changed 的布局刷新风险。
- 验证记录：主 agent 运行 `cargo test --lib image`（17 passed）、`cargo test --lib ui_gallery_audit_recipe_registers_scroll_capture_states`（1 passed）、`cargo fmt --all -- --check`、`cargo check`、`.\scripts\run-ui-audit.ps1 -SelfTest` 和 `git diff --check` 均通过。真实 `phone-small / image_fit` runner 1/1 passed，生成 720x1600 PNG 与 metadata；主 agent 查看截图确认四种 fit、横竖 frame、Cover 两端焦点、圆角裁切和 fixture 均完整，无重叠、截字或越界。`cargo check` 仅保留仓库既有 `checkbox` dead-code warning。提交：`d857c53`。

- [x] 扩展 `UiImageFit`，支持 Natural、Stretch、Contain 和 Cover，并明确各模式的尺寸计算规则。（验证：`project/src/framework/ui/widgets/image.rs:9` 定义四模式，`:361` 的 `calculate_image_fit` 实现自然尺寸、拉伸、等比包含和裁源覆盖；focused tests 通过）
- [x] 为 Cover 增加水平和垂直焦点参数，使头像、角色和背景图可以控制裁切中心。（验证：`project/src/framework/ui/widgets/image.rs:44` 定义左上坐标系的二维 `UiImageFocus`，Cover 横纵焦点 clamp 与源矩形边界测试通过）
- [x] 支持固定尺寸、百分比尺寸、宽高比和最大/最小尺寸约束的组合校验。（验证：`project/src/framework/ui/widgets/image.rs:110` 定义 `UiImageConstraints`，覆盖 px/percent/auto/aspect/min/max 与非有限、单位冲突、min>max、过约束测试）
- [x] 对零尺寸、非法宽高比、资源未加载和图片加载失败提供可诊断的占位或错误状态。（验证：`project/src/framework/ui/widgets/image.rs:323` 定义 Loading/Ready/Failed/Invalid，`:493` 按资源/布局状态应用不同占位；零尺寸、非法宽高比和 Loading ECS 测试通过）
- [x] 确保图片在圆角与 `Overflow::clip()` 下裁切正确，不污染父节点布局。（验证：`project/src/framework/ui/widgets/image.rs:448` 由 frame 独占圆角与 clip，运行时只提交实际变化的子图片组件；`:1157` 和 `:1177` 证明相同输入第二帧不再标记 Changed，真实截图裁切正确）
- [x] 在 UI Gallery 展示所有 fit 模式和横竖比例组合，并注册稳定审计 state。（验证：`project/src/game/screens/dev/ui_gallery.rs:296` 生成四模式 2x2 卡片且每卡含横竖 frame，`project/src/framework/ui/audit/local.rs:34` 注册 `image_fit`；真实 phone-small audit 通过）
- [x] 为尺寸计算、焦点 clamp 和非法输入补充 focused 单元测试。（验证：主 agent 运行 `cargo test --lib image`，17 passed，覆盖尺寸、横纵裁切、焦点 clamp、非法约束和稳定 ECS change tick）
- [x] 在 `project/` 运行 `cargo fmt`、相关图片测试和 `cargo check`。（验证：主 agent 运行 `cargo fmt --all -- --check`、两组 focused tests 与 `cargo check` 均退出码 0）

## 阶段 3：九宫格、平铺和图集帧

- 开始时间：2026-07-11 23:39:32 +08:00
- 结束时间：2026-07-12 01:07:27 +08:00
- 开发总结：新增可序列化的九宫格、X/Y/Both 平铺、纹理源和图集帧模型，提供边界、缩放、物理像素与重复预算校验，并由统一 advanced builder 映射 Bevy 0.18.1。Gallery 加入九宫格、多尺寸按钮边框、平铺和四个图集帧，审计框架新增通用 child anchor 及 `image_modes`、`image_tiling`、`image_atlas` state。第 1/5 轮返工将 spec path 设为实际纹理 handle 的唯一权威来源，消除同尺寸错误纹理绕过描述的风险。
- 验证记录：主 agent 运行 `cargo test --lib image`（31 passed）、`cargo test --lib scroll`（14 passed）、`cargo test --lib ui_gallery`（7 passed）、`cargo fmt --all -- --check`、`cargo check`、runner self-test 和 `git diff --check` 均通过；worker 完整 `cargo test --lib --no-fail-fast` 1247 passed。真实审计 phone-small 三状态 3/3、tablet-landscape 三状态 3/3、1170x2532/device scale 3.25 的 `image_modes` 1/1 passed；主 agent 查看全部关键截图，确认边框无翻转、三种平铺方向正确、四个 atlas frame 无整图泄漏或重叠。仅保留仓库既有 `checkbox` dead-code warning。提交：`19d3a50`。

- [x] 封装 Bevy `NodeImageMode::Sliced`，定义可序列化的九宫格边距、缩放模式和中心区域策略。（验证：`project/src/framework/ui/widgets/image.rs:211`、`:285` 定义 serde insets、中心/边缘策略、角缩放和 slice 预算，`:919` 校验后映射 `TextureSlicer`）
- [x] 封装 `NodeImageMode::Tiled`，支持横向、纵向和双向平铺，并校验重复阈值。（验证：`project/src/framework/ui/widgets/image.rs:348` 定义轴向、stretch 与预算，`:991` 计算重复数并拒绝超预算；X/Y/Both focused tests 与真实截图通过）
- [x] 建立正式图集帧描述，支持源纹理、像素区域、原始尺寸和可选 pivot。（验证：`project/src/framework/ui/widgets/image.rs:175` 定义 `UiAtlasFrame`；`:1113` 仅从已校验 spec path 加载 handle，同尺寸错误纹理不可注入，路径/边界/original/pivot 测试通过）
- [x] 明确九宫格与图集帧组合的支持边界；不支持的组合必须在构建前返回错误。（验证：`project/src/framework/ui/widgets/image.rs:443` 的 advanced spec 拒绝 atlas + NineSlice/Tiled；测试确认失败前后 AssetServer 均未注册非法 source path）
- [x] 验证切片边框在小于最小尺寸、非整数缩放和高 DPI 下不会翻转或明显变形。（验证：小目标/物理像素 focused tests 通过；1170x2532、device scale 3.25 真实 `image_modes` 截图边框稳定）
- [x] 在 UI Gallery 增加面板边框、按钮多尺寸、平铺背景和图集帧示例。（验证：`project/src/game/screens/dev/ui_gallery.rs:366` 生成高级图片区域，`:1695`/`:1748` 注册平铺与 atlas anchor；phone/tablet 三状态真实审计通过）
- [x] 为切片参数、帧边界、资源尺寸和失败路径补充测试。（验证：主 agent `cargo test --lib image` 31 passed，覆盖 RON、insets、预算、source mismatch、安全路径、构建前拒绝、atlas 边界和稳定 ECS change tick）
- [x] 在 `project/` 运行 `cargo fmt`、相关测试和 `cargo check`。（验证：主 agent 运行格式、image/scroll/Gallery focused tests 与 `cargo check` 均退出码 0）

## 阶段 4：字体注册、多字重和文本排版

- 开始时间：2026-07-12 01:09:20 +08:00
- 结束时间：2026-07-12 03:56:46 +08:00
- 开发总结：将兼容用 `UiFontAssets::regular` 扩展为 family/weight/role/face/coverage 注册表，产品 CJK Regular 与 Figtree fixture 400/500/700 使用真实静态 face；新增整节点 fallback、加载/失败/缺字状态、完整文本样式 token、字素簇 ellipsis、父 frame clip 和受控静态字图描述。公共标题/标签 helper、i18n/theme 刷新和 UI Gallery 已接入；四轮返工修复了 Bevy 同实体 overflow 无法裁字、tablet 嵌套文本 auto-height 遗漏、稳定文本重复 Changed 和静态字图路径校验边界。
- 验证记录：主 agent 运行 `cargo test --lib fonts`（15 passed）、`cargo test --lib typography`（6 passed）、i18n/theme focused tests（各 1 passed）、`cargo test --lib ui_gallery`（12 passed）、audit state test（1 passed）、`cargo fmt --all -- --check`、`cargo check`、`.\scripts\run-ui-audit.ps1 -SelfTest` 和 `git diff --check` 均通过；真实审计 run `summary/ui-audit/20260712-032630-836e6d` 的 phone-small / tablet-landscape 各 2 张截图人工复核通过，确认三字重、混排、长文本、父裁切、ellipsis、显式缺字替换和三个相邻 panel 均无截断、越界或重叠。字体文件全部命中 Git LFS，CJK 文件 SHA-256 与说明一致；仅保留仓库既有 `checkbox` dead-code warning。提交：`d13fa92`。

- [x] 将单一 `UiFontAssets::regular` 扩展为字体家族和字重注册表，至少支持 Regular、Medium 和 Bold。（验证：`project/src/framework/ui/style/fonts.rs` 注册 ProductCjk 与 FigtreeFixture face，Figtree 400/500/700 均解析到独立静态 handle；fonts focused 15/15 通过）
- [x] 为每个字体角色定义主字体、fallback、覆盖字符范围和资源缺失行为。（验证：`UiFontRoleSpec` 为 Display/Heading/Body/Caption/Control/LatinFixture 声明 primary、fallback、coverage、Loading/Failed/missing-glyph policy；失败与整节点 coverage fallback 测试通过）
- [x] 扩展文本样式 token，支持字体家族、字重、字号、行高、对齐、换行和截断策略。（验证：`UiTextStyleToken` 可序列化并映射 `TextFont`、`LineHeight`、`TextLayout`，支持 clip 与 grapheme ellipsis；RON/非法值/稳定 ECS change-tick 测试通过）
- [x] 明确当前 Bevy 文本能力无法表达的字距、复杂富文本或排版效果，并提供资源化文字图片的受控替代方案。（验证：`docs/ui/UI主题字体与国际化.md` 和 `UI当前限制.md` 记录边界；`UiRasterizedTextSpec` 校验 ui 资源路径、可访问文案、i18n key 和来源，非法路径在 AssetServer 注册前失败）
- [x] 验证中英文混排、数字、标点、长单词、超长中文和字体缺字场景。（验证：fonts tests 覆盖整节点 CJK fallback 与 emoji 显式 `?`；最新 phone/tablet `typography_overflow` 截图确认全部固定边界文案正确显示）
- [x] 保证 i18n 热更新后文本仍保留正确字体角色和布局约束。（验证：字体同步排在 `UiI18nSystems::Refresh` 与 `UiThemeSystems::Refresh` 后；`i18n_refresh_preserves_font_role_layout_and_node_constraints` 通过）
- [x] 在 UI Gallery 展示全部文字角色、字重、换行和溢出状态。（验证：Gallery 注册 `typography`、`typography_overflow` 两个 anchor/state，展示六角色、三真字重、混排、长文本、clip、ellipsis、居中和缺字；双 profile 四图人工通过）
- [x] 为字体选择、fallback、样式解析和缺失资源补充测试。（验证：主 agent `cargo test --lib fonts` 15 passed，覆盖三字重、coverage/资源失败 fallback、缺字、RON、grapheme、clip、路径失败和稳定 Changed）
- [x] 在 `project/` 运行 `cargo fmt`、文本相关测试和 `cargo check`。（验证：主 agent `cargo fmt --all -- --check`、fonts/typography/i18n/theme/Gallery/audit focused tests 与 `cargo check` 均退出码 0）

## 阶段 5：图标资源和图片化按钮状态

- 开始时间：2026-07-12 03:59:15 +08:00
- 结束时间：2026-07-12 06:14:01 +08:00
- 开发总结：新增稳定 `UiIconId` 注册表、单色/全彩 tint 边界、加载与缺失占位状态，以 9 张 96 x 96 正式 PNG 替换文本符号；新增纯图标、左右图标文字和固定尺寸图片按钮，统一 idle/hovered/pressed/focused/selected/disabled/loading 优先级与贴图/tint/background override。根按钮显式维护 AccessKit 可访问节点，i18n 更新同步隐藏 label 与根 label；主审核第 1/5 轮返工修复了主题刷新整值覆盖业务 `Node` 布局字段、Lucide 许可误注和 manifest 弱哈希映射校验。
- 验证记录：主 agent 运行完整 `cargo test --lib --no-fail-fast`（1289 passed）、`cargo check --tests`、`cargo check`、`cargo fmt --all -- --check`、runner self-test 和 `git diff --check` 均通过；9 张 PNG 全部命中 Git LFS，实际 SHA-256 与 typed RON manifest 一一匹配。真实审计 `summary/ui-audit/20260712-045809-d6c8d1` 的 phone-small / tablet-landscape 各覆盖 `icons` 和 `icon_states`，2/2 task、4/4 capture passed；人工查看确认图标、全彩样例、缺失占位和七态矩阵无重叠、越界或布局抖动。仅保留仓库既有 `checkbox` dead-code warning。提交：`c2d0c05`。

- [x] 用正式图标资源替代图标按钮中的文本符号，并保留可访问名称和 i18n label。（验证：`UiIconBundle` 使用 PNG `ImageNode`；根 `AccessibilityNode` 初始 label 为 `Add`，替换 i18n 后隐藏 Text 与根 label 均为“添加”）
- [x] 定义稳定图标 ID、资源路径、默认尺寸、着色能力和缺失图标占位行为。（验证：`widgets/icon.rs` 注册 9 个 descriptor，typed manifest 精确校验 ID/path/policy/hash/dimensions，未知 ID 与资源失败均进入 `missing.png` 状态）
- [x] 支持纯图标、图标加文字、左/右图标和固定尺寸图片按钮。（验证：`icon_button_key`、`icon_label_button_key` 的 Leading/Trailing 与 `image_button_key` 在 Gallery 真实截图中均可见）
- [x] 支持 idle、hovered、pressed、focused、selected、disabled、loading 的贴图或样式覆盖。（验证：`UiIconButtonVisuals` 提供七态 override，focused 测试覆盖优先级和自定义 icon/tint/background）
- [x] 明确单色可着色图标与全彩图标的处理边界，避免错误 tint。（验证：`MonochromeTintable` 使用主题 `icon_tint`，`FullColor` 强制 `Color::WHITE`；单元测试与 Gallery 全彩样例通过）
- [x] 确保状态图片切换不改变按钮布局尺寸或点击区域。（验证：状态稳定帧不改变 Node/Children 且不产生无意义 Changed；主题刷新仅更新 helper 拥有字段，业务 margin/inset/grid 等全部保留）
- [x] 在 UI Gallery 覆盖鼠标、键盘焦点和触控按下状态。（验证：`icon_states` 稳定 anchor 展示 hovered/focused/pressed 及其余四态，phone/tablet 四张真实截图 passed）
- [x] 为图标解析、状态优先级和缺失资源路径补充测试。（验证：完整 lib tests 1289/1289，其中 icon 18/18、button 61/61，覆盖注册、unsafe path、tint、manifest、missing fallback、七态优先级和 ECS 稳定性）
- [x] 在 `project/` 运行 `cargo fmt`、按钮相关测试和 `cargo check`。（验证：主 agent 运行 `cargo fmt --all -- --check`、完整 lib tests、`cargo check --tests` 和 `cargo check` 均退出 0）

## 阶段 6：作用域样式、组件变体和主题 token

- 开始时间：2026-07-12 06:16:55 +08:00
- 结束时间：2026-07-12 09:19:29 +08:00
- 开发总结：建立类型化 Color/Scalar token、可继承 component variant 和页面/子树 scope 解析器；接入表面、边框、文字、按钮、输入框、卡片与弹窗 role，保留现有控件交互状态源；完成热更新 last-known-good、旧 v1 主题迁移、解析快照 metadata 和 Gallery `style_scopes` 对照区。
- 验证记录：主 agent 运行完整 lib tests 1320/1320、`cargo fmt --all -- --check`、`cargo check --tests`、`cargo check`、audit runner SelfTest 和 `git diff --check` 均通过；`summary/ui-audit/20260712-091122-512669` 的 phone-small/tablet-landscape `style_scopes` 2/2 通过，人工确认无重叠、截字、越界或状态丢失，metadata 含 10 份稳定解析快照。仅保留既有 `checkbox` dead-code warning。

- [x] 将当前单一全局主题扩展为基础 token 加组件变体，避免每个参考页面复制完整主题。（验证：`style/scopes.rs` 的 `UiStyleSheet::compile` 编译类型化 token/variant/override，`assets/ui/themes/default.ron` 仅声明 Gallery 所需差异 token 和变体）
- [x] 定义可组合的表面、边框、文字、按钮、输入框、卡片和弹窗样式角色。（验证：`UiStyleBinding` 类型化组合 7 类 role；Caption 保持紧凑字号与主文字色，Button/Input 有真实最终渲染 consumer）
- [x] 支持页面或子树作用域的样式覆盖，并定义继承、优先级和恢复规则。（验证：解析顺序为 base -> request variant -> 根到最近 scope；`ecs_resolver_inherits_nested_scope_and_restores_after_removal` 验证嵌套覆盖及移除恢复）
- [x] 保证主题热更新能够刷新作用域样式，同时不覆盖业务运行时状态。（验证：theme/input/button/icon 生产 ECS 回归验证热更新当帧收敛，且保留 Interaction、focus、selected、disabled、loading 和输入值）
- [x] 对未知 token、循环引用、重复变体和类型不匹配返回稳定错误。（验证：scopes 编译器回归覆盖 unknown token/variant、cycle、duplicate token/variant/override、type mismatch、越界颜色和非法尺寸，错误码为稳定 `ui_style_*`）
- [x] 提供样式解析后的只读调试信息，供 F3 面板和 AI 审核 metadata 使用。（验证：`UiResolvedStyleDebugSnapshot` 记录 scope/request/source/final token/fallback/error；audit `style_resolutions` 稳定排序收集，解绑后清理无 stale snapshot）
- [x] 为默认主题建立向后兼容迁移，现有页面无需一次性重写即可运行。（验证：`UiThemeConfig.styles` 使用 serde default，缺少 `styles` 的旧 version 1 配置加载内置兼容表；packaged/default 主题测试均通过）
- [x] 为继承、覆盖、热更新和旧配置迁移补充测试。（验证：scopes 23/23、fonts 17/17、theme 17/17、controls 47/47、Gallery 15/15，并覆盖首帧收敛、第二帧 Changed 稳定性和同帧解绑/重绑）
- [x] 在 `project/` 运行 `cargo fmt`、主题相关测试和 `cargo check`。（验证：主 agent 运行 `cargo fmt --all -- --check`、完整 lib tests 1320/1320、`cargo check --tests` 和 `cargo check` 均退出 0）

## 阶段 7：阴影、渐变、描边和自定义材质边界

- 开始时间：2026-07-12 09:23:00 +08:00
- 结束时间：2026-07-12 11:12:11 +08:00
- 开发总结：新增受限视觉效果 catalog 与 `UiEffectBinding`，覆盖多层盒阴影、原生文字阴影、背景/边框线性渐变、独立边宽/圆角、Outline、圆角裁切，以及静态材质白名单、参数/平台/GPU/shader/adapter 校验和可见 fallback；逐字段 baseline + last-applied 所有权模型可识别外部主题/业务写入，预设切换和解绑不会恢复陈旧值。Gallery 增加 5 个效果对照，audit metadata 记录稳定解析与预算，并补齐效果/材质边界文档。
- 验证记录：主 agent 审核第 1/5 轮返工后运行 `cargo test --lib`（1343/1343）、`cargo fmt --all -- --check`、`cargo check --tests`、`cargo check`、`.\scripts\run-ui-audit.ps1 -SelfTest` 和 `git diff --check` 均通过；focused effects 23/23，覆盖外部更新恢复、预设切换、unknown fallback、非法 effects 热更新 LKG 与 7 类材质失败可见降级。真实审计 `summary/ui-audit/20260712-103325-ce1016`（phone-small）和 `summary/ui-audit/20260712-103458-fdc29b`（tablet-landscape）均 passed，人工复核 5 个效果示例无重叠、截字或越界，metadata 均含 5 个稳定 effect resolution 和 1 个预期材质 fallback。仅保留仓库既有 `checkbox` dead-code warning。

- [x] 封装 Bevy `BoxShadow` 和 `TextShadow`，支持颜色、偏移、扩散、模糊和多层阴影的受限配置。（验证：盒阴影支持颜色、XY 偏移、spread、blur 和最多 3 层；Bevy 0.18.1 文字阴影只接受单层颜色/偏移，对 blur、spread、多层返回稳定 `ui_effect_text_shadow_unsupported`）
- [x] 封装背景渐变和边框渐变，至少覆盖线性渐变、角度、色标和透明度。（验证：背景/边框各支持一层线性渐变、规范化角度、2..=6 个有序透明色标；非法角度、色标与颜色有稳定错误）
- [x] 支持独立边宽、圆角和轮廓样式，并验证圆角、裁切、渐变和阴影的组合结果。（验证：`gallery.composite` 同时应用四边宽、四圆角、clip、Outline、背景/边框渐变与阴影，phone/tablet 截图及 ECS 回归通过）
- [x] 定义何时允许使用 `UiMaterial`，并为自定义材质设置 shader 白名单、参数上限和平台兼容要求。（验证：仅静态 allowlist `frosted_panel_v1` 可请求固定 shader 路径，限制 4 scalar、2 color、0 texture 与 native/Android 平台；未交付 adapter 时明确降级）
- [x] 对 GPU 不支持、shader 加载失败和材质参数非法提供降级样式。（验证：参数、平台、GPU、shader missing/loading/failed、adapter unavailable 7 条路径均应用声明的背景/边框 fallback 并写入稳定原因码）
- [x] 在 UI Gallery 增加阴影、渐变和材质降级对照示例。（验证：`effects` anchor/state 展示多层盒阴影、文字阴影、背景/边框渐变、圆角裁切组合和材质降级共 5 个示例，中英文文案与自动 recipe 完整）
- [x] 记录新增效果的 draw call、过度绘制和移动端性能预算。（验证：编译器硬限制 8 个额外 draw primitive、5 层 overdraw，并在 debug/audit snapshot 记录请求/应用上界、阴影层和渐变色标；文档给出移动端建议与真机分析边界）
- [x] 为配置解析和降级路径补充测试，并进行至少一个手机 profile 的截图验收。（验证：effects 23/23、theme/audit/Gallery 回归通过；phone-small `20260712-103325-ce1016` 与 tablet-landscape `20260712-103458-fdc29b` 真实审计通过）
- [x] 在 `project/` 运行 `cargo fmt`、相关测试和 `cargo check`。（验证：主 agent 运行格式检查、1343 项 lib tests、`cargo check --tests`、`cargo check`、runner SelfTest 和 diff 检查均退出 0）

## 阶段 8：通用属性动画和过渡

- 开始时间：2026-07-12 11:13:51 +08:00
- 结束时间：2026-07-12 13:10:59 +08:00
- 开发总结：将兼容用 alpha 播放器扩展为类型化通用属性动画，覆盖透明度、Transform 位移/缩放、布局位置/尺寸和背景/文字颜色；补齐延迟、easing、方向、有限/无限重复、完成、继续当前值、取消、seek 和暂停语义。控件过渡、页面入场、弹窗入退场与 loading 循环均接入统一模型，并提供 Full/Reduced/Disabled 动态策略、主题和页面清理行为、稳定事件及审计快照；Gallery 新增 8 个固定进度示例。
- 验证记录：主 agent 运行 `cargo test --lib`（1363/1363）、`cargo fmt --all -- --check`、`cargo check --tests`、`cargo check`、`.\scripts\run-ui-audit.ps1 -SelfTest` 和 `git diff --check` 均通过；worker focused animation tests 31/31。真实审计 `summary/ui-audit/20260712-125321-8ee4bf`（phone-small）和 `summary/ui-audit/20260712-125539-052521`（tablet-landscape）均 passed，人工复核 8 个示例无重叠、截字或越界；metadata 含 8 份暂停于 raw progress `0.625` 的稳定快照，且仅布局尺寸标记 reflow。审核返工修复 Disabled 策略未按实际重复次数和方向选择最终端点的问题；仅保留仓库既有 `checkbox` dead-code warning。

- [x] 在现有 alpha 动画基础上定义通用动画目标，至少支持透明度、位置、尺寸、缩放和颜色。（验证：`core/animation.rs` 的 `UiAnimationTarget` 覆盖 Alpha、TransformTranslation、LayoutPosition、LayoutSize、TransformScale、BackgroundColor 和 TextColor；属性应用回归通过）
- [x] 定义 from/to、duration、delay、easing、播放方向、重复次数和完成行为。（验证：`UiAnimationSpec` 类型化声明全部时序字段，校验非法时长、延迟、重复和值类型；方向与实际 repeat 终点测试通过）
- [x] 支持控件状态过渡、页面入场、弹窗入退场和 loading 循环动画的稳定组合。（验证：Gallery 展示控件过渡与页面入场，`overlays/modal.rs` 接入入退场，`overlays/loading.rs` 接入无限 pulse；双 profile 截图通过）
- [x] 明确布局属性动画与 Transform 动画的选择规则，避免每帧重排造成不必要开销。（验证：`UiAnimationTarget::causes_layout_reflow` 仅标记 LayoutPosition/LayoutSize，新增文档优先推荐 Transform；audit metadata 仅 layout-size 示例为 reflow）
- [x] 处理动画被打断、目标实体销毁、主题热更新和页面切换时的取消语义。（验证：Start/ContinueFromCurrent/Cancel 与 Replaced/Cancelled/Rejected 事件稳定；销毁目标、主题变更和 Gallery 页面清理回归通过）
- [x] 提供禁用或减少动态效果的全局设置，并保证关闭动画后直接到达最终状态。（验证：`UiMotionPolicy` 提供 Full/Reduced/Disabled；Disabled 同帧到达按方向与 repeat 计算的最终端点，Reduced 收敛无限循环，相关回归通过）
- [x] 在 UI Gallery 注册可重复截图的动画静止点或指定进度 state。（验证：`animations` recipe/state 在截图前 seek 并暂停于 raw progress 0.625；双 profile metadata 均稳定收集 8 个快照）
- [x] 为 easing、插值、取消、完成和零时长补充测试。（验证：animation focused tests 31/31，覆盖 easing clamp、标量/向量/颜色插值、取消行为、完成事件、零时长、seek/pause、repeat/direction 与 ECS change stability）
- [x] 在 `project/` 运行 `cargo fmt`、动画相关测试和 `cargo check`。（验证：主 agent 运行格式检查、1363 项 lib tests、`cargo check --tests` 与 `cargo check` 均退出码 0）

## 阶段 9：复刻常用组件和完整交互状态

- 开始时间：2026-07-12 13:12:42 +08:00
- 结束时间：2026-07-12 19:09:41 +08:00
- 开发总结：新增可复用 Badge、Progress、Tab、Tooltip 和 Dropdown，建立九态支持矩阵、稳定 `UiControlEvent` 与 owner/control ID/value/reason 载荷；Checkbox、Toggle、Segmented 改为 box/mark、track/thumb、indicator 正式结构。Tooltip/Dropdown 接入 Panel Manager、焦点、键盘、滚动、边缘避让、click-away/Escape/owner 清理与 Modal 阻断；Dropdown 固定高度并对长文本使用字素簇省略，option 保持面板内换行。新增不透明 Popover 主题 role、确定性 Tooltip pin、6 个 Gallery 审计 state 和 63 份稳定控件快照；正式 Lucide chevron-down PNG 替代会在滚动裁切边界泄漏的旋转线段。
- 验证记录：主 agent 两轮返工审查后运行 `cargo test --lib -- --test-threads=1`（1398/1398）、`cargo fmt --all -- --check`、`cargo check --tests`、`cargo check`、`.\scripts\run-ui-audit.ps1 -SelfTest` 和 `git diff --check` 均通过且无 Rust warning。最终真实审计 `summary/ui-audit/20260712-185958-04b879`（phone-small）与 `summary/ui-audit/20260712-190029-5d7d86`（tablet-landscape）各 6/6 passed；主 agent 人工复核 12 张图，确认中文终态、三类选择控件八态、固定 Dropdown 尺寸/省略号、长 option 换行、Popover 边缘避让和实色 Tooltip 无重叠、越界或裁切碎片。两设备每态 63 个稳定 snapshot，panel 序列为 page-only、page+dropdown、page+tooltip；transition run `20260712-174139-93d277` 验证 Tooltip -> middle -> bottom panel_count 为 2/1/1。`chevron-down.png` SHA-256 与 manifest 一致并命中 Git LFS；已知 AMD Vulkan 首帧 swap-chain timeout 复跑相同 binary 成功，不属于业务失败。

- [x] 补充参考图中高频但当前缺失的 Badge、Progress、Tab、Tooltip 和下拉选择组件。（验证：`widgets/controls/components.rs` 提供五类类型化数据与公共 helper，Popover 实现在 `overlays/popover.rs`；components focused 12/12 通过）
- [x] 将 Checkbox、Toggle 和 Segmented 从文本符号外观升级为可换肤的正式视觉结构。（验证：`selection.rs` 使用固定 box/mark、track/thumb、segment indicator 子节点，文案不再编码 `[x]` 或 ON/OFF；phone/tablet 独立 state 截图通过）
- [x] 为新增控件定义 normal、hovered、pressed、focused、selected、disabled、loading、empty 和 error 状态。（验证：`UiControlKind::supports_state` 定义适用矩阵，`UiControlFlags` 与 `resolve_control_state` 固定优先级；audit metadata 每态稳定收集 63 个 snapshot）
- [x] 为控件事件定义稳定消息，不让业务系统依赖内部实体层级或直接扫描 marker。（验证：`UiControlEvent` 返回根 entity、owner、control ID/kind、ValueChanged/Opened/Closed、Bool/Text 值和触发 reason；选择与 Dropdown ECS 回归通过）
- [x] 验证鼠标、键盘、触控、滚动容器和 Modal 内的焦点与输入阻断行为。（验证：UiButton pointer/touch 路径、Enter/Space/Tab/方向/Home/End、UiScrollView、disabled/loading flags 直接门控及 Modal 内三种关闭焦点回返均有生产调度测试；focus 6/6 通过）
- [x] 为 Tooltip 和下拉层定义层级、屏幕边缘避让、关闭和 owner 清理语义。（验证：Popover 使用 Floating layer、safe-area clamp 和上下翻转；click-away/Escape/CloseTop/owner despawn/页面 owner 清理及 trigger focus return 回归通过，transition panel_count 2/1/1）
- [x] 在 UI Gallery 为每个组件展示全部可达状态和长文本边界。（验证：注册 `components`、`component_checkboxes`、`component_toggles`、`component_segmented`、`component_overlays`、`component_tooltip` 六个 state；双设备 12 张最终截图人工通过）
- [x] 为状态转换、事件、焦点和清理路径补充测试。（验证：focused 包含 components 12/12、Popover 8/8、Theme 19/19、Gallery 24/24、audit local 35/35、selection 4/4、panel 4/4，并覆盖第二帧 change stability）
- [x] 在 `project/` 运行 `cargo fmt`、控件相关测试和 `cargo check`。（验证：主 agent 运行单线程全库 1398 项、格式检查、`cargo check --tests` 与 `cargo check` 均退出码 0）

## 阶段 10：安全区、响应式、性能和整体文档验收

- 开始时间：2026-07-12 19:12:35 +08:00
- 结束时间：2026-07-12 21:34:13 +08:00
- 开发总结：接入 Android `WindowInsetsCompat -> JNI -> UiSafeAreaStatus` 生产链和桌面 safe-area fixture/override；新增 Compact/Medium/Expanded 类型化视觉预算、图片/字体/样式/效果/动画审计汇总，以及只组合框架公共 helper 的 `visual_acceptance` 综合验收区。响应式 helper 同时覆盖方向和 Short 高度，Compact 效果区调整为两列以完整显示五个样例。第 1/5 轮返工修正 `--safe-area-insets` 缺值时误称 window size 的诊断，并补充不吞后续 flag 的回归测试。
- 验证记录：主 agent 运行单线程完整 lib tests 1415/1415、返工后 config focused tests 14/14、`cargo fmt --all -- --check`、`cargo check --tests`、`cargo check`、UI audit SelfTest 和 `git diff --check` 均通过；Gradle `compileDebugJavaWithJavac --rerun-tasks` 成功，`llvm-nm -D` 确认导出 `Java_com_mybevy_project_MainActivity_nativeOnWindowInsetsChanged`。真实审计 `summary/ui-audit/20260712-210022-e38020` 覆盖四个 profile、4/4 task 与 24/24 capture，全部预算 passed（1200 nodes、30.02 MiB 解码 payload、14 额外 effect draw 上界、2 个材质估算、2 层 overdraw、0 unresolved），主 agent 人工复核综合区、效果区和浮层关键截图无文字重叠、关键裁切或越界。worker 完成 Android arm64 动态库和 Debug APK 构建；当前环境返回 `ADB_NOT_FOUND`，因此真机安全区、字体、触控、图片切片和效果降级按条件保留未验。

- [x] 接入 Android 状态栏、刘海和手势导航 inset，使 `UiSafeArea` 不再固定为零。（验证：`MainActivity.onApplyWindowInsets` 合并 system bars/display cutout/mandatory+system gestures 并排除 IME，`core/safe_area.rs` 通过线程安全 mailbox 接收物理像素，`viewport.rs` 按实时窗口与 scale 转逻辑像素；Java 编译通过且 `llvm-nm -D` 确认 JNI symbol 已导出）
- [x] 验证 Compact、Medium、Expanded、横竖屏和 Short 高度下新增组件的布局策略。（验证：四 profile metadata 分别覆盖 Compact/Medium/Expanded 与横竖屏，24 张真实截图人工通过；`responsive_columns_for_viewport` focused test 覆盖 Short、Expanded Portrait 和各宽度列数，Gallery profile test 覆盖 360x568 Short 触控尺寸）
- [x] 为高保真效果建立节点数、图片内存、draw call、材质数和 overdraw 的开发期预算。（验证：`visual.rs` 定义三档 limits、稳定 metric/finding/status，`audit/local.rs` 对图片 handle 去重并汇总 effect/material/overdraw；四 profile metadata 均为 passed，实际值 1200/30.02 MiB/14/2/2 且 unresolved=0）
- [x] 扩展 F3 或审计 metadata，显示图片模式、样式变体、字体角色、效果数量和动画状态。（验证：capture JSON 输出 `image_snapshots`、`font_snapshots`、稳定 `visual_summary` 和 `visual_budget`，四 profile 24 份 metadata 均可解析且字段完整）
- [x] 使用 UI audit runner 覆盖 `phone-small`、`phone-portrait`、`tablet-portrait` 和 `tablet-landscape`。（验证：`summary/ui-audit/20260712-210022-e38020` 为 4/4 task、24/24 capture passed，覆盖综合验收、效果、动画、组件和两类浮层状态）
- [ ] 在可用 Android 设备上验收安全区、字体、触控、图片切片和效果降级；无设备时保留未完成并记录条件。（条件记录：主 agent 与 worker 均得到 `ADB_NOT_FOUND`；arm64 `libproject.so`、JNI 导出、Java 编译和约 181.5 MB Debug APK 已验证，但不能替代 API 31+ 授权真机上的 cutout/手势/三键导航、IME、旋转恢复、字体、触控、切片和效果验收）
- [x] 更新 `docs/ui/`、`docs/assets-workflow.md` 和 `docs/bevy-getting-started.md` 中受影响的说明。（验证：新增 `docs/ui/UI安全区与视觉预算.md`，并同步 UI README、响应式、限制、调试、高保真、效果、assets workflow 与 getting started；文档明确 fixture/估算不冒充真机或 GPU 实测）
- [x] 运行 UI audit self-test、相关页面真实截图审计和 `git diff --check`。（验证：主 agent 运行 SelfTest 与 `git diff --check` 均退出 0，并人工复核 `20260712-210022-e38020` 的四档综合区、窄屏效果区及关键浮层截图）
- [x] 在 `project/` 运行 `cargo fmt`、相关测试和 `cargo check`。（验证：主 agent 完整 lib tests 1415/1415、返工后 config tests 14/14、格式检查、`cargo check --tests` 与 `cargo check` 均退出码 0）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有阶段完成后统一验收。

- 开始时间：2026-07-12 21:37:15 +08:00
- 结束时间：2026-07-12 21:38:26 +08:00
- 验收总结：阶段 1-10 已以 10 个独立提交完成，框架形成图片适配/切片/图集、多字重排版、正式图标、作用域样式、受限效果与材质降级、通用动画、完整控件状态、Android 安全区和类型化视觉预算闭环。`visual_acceptance` 仅通过公共能力组合背景图、九宫格、多字重、正式图标、阴影/渐变和按钮状态；四个核心 profile 共 24 张最终截图与 metadata 通过，所有预算低于阈值。完整 Rust/Java/runner/Android 构建证据成立；唯一保留条件是当前环境 `ADB_NOT_FOUND`，Android 真机安全区、字体、触控、图片切片和效果降级需在 API 31+ 授权设备补验，已在清单与文档明确记录，未冒充完成。

- [x] 一张包含背景图、九宫格面板、多字重文字、正式图标、阴影、渐变和状态按钮的验收页面可以只通过框架公共能力实现。（验证：UI Gallery `visual_acceptance` 区组合 Cover 正式图片、advanced nine-slice、Regular/Medium/Bold、Help 图标、composite effect 与 Selected/Loading/Disabled 按钮；四 profile 截图通过）
- [x] 验收页面不需要在业务模块复制图片计算、状态优先级、字体 fallback 或效果降级逻辑。（验证：综合区调用 `ui_image`、`try_ui_advanced_image`、`try_ui_styled_text`、图标/状态按钮 helper 和 `UiEffectBinding` preset，计算、fallback 与优先级仍由 `framework/ui/` 所有）
- [x] 所有新增能力在 UI Gallery 中有稳定示例、状态覆盖和可自动进入的审计 recipe。（验证：导航 recipe 注册 22 个稳定 capture state，runner `auto` 包含 visual foundation/acceptance、图片、字体、图标、样式、效果、动画及组件专题 state）
- [x] 手机与平板核心 profile 无文字重叠、关键裁切、触控目标不足或弹层越界。（验证：最终四 profile 24 张截图人工复核通过，包含 phone-small/phone-portrait/tablet-portrait/tablet-landscape 的综合区、效果、动画、组件、Dropdown 与 Tooltip；profile/Short 触控最小尺寸测试通过）
- [x] Android 安全区有真实设备验证记录，或明确记录尚未满足的设备阻塞条件。（验证：满足后半条件；`ADB_NOT_FOUND` 已记录，`UI安全区与视觉预算.md` 列出 API 31+ 设备、cutout/导航/IME/旋转恢复与视觉触控补验清单，阶段真机执行项保持 `[ ]`）
- [x] 主题热更新、i18n 更新和页面切换不会破坏新增组件状态。（验证：各阶段 theme/style/font/icon/animation/control/page cleanup 回归均通过，最终 1415 项完整 lib tests 覆盖 last-known-good、状态保留、i18n 更新和 route/owner 清理）
- [x] 新增视觉效果满足已定义的节点、图片内存和渲染预算。（验证：四 profile 24 份 metadata 的 `visual_budget.status` 全部 passed，usage 为 1200 nodes、30.02 MiB、14 额外 effect draw 上界、2 个材质估算、2 层 overdraw、0 unresolved）
- [x] `cargo fmt`、相关 focused tests、`cargo check` 和 UI audit 验收全部通过。（验证：主 agent 运行 1415/1415 lib tests、返工后 14/14 config tests、格式检查、`cargo check --tests`、`cargo check`、runner SelfTest、4/4 task 与 24/24 capture 均通过）
- [x] `docs/ui/` 与实际公共 API、限制和资源工作流一致。（验证：UI README 与高保真、图片、字体、图标、样式、效果、动画、组件、响应式、调试、限制及安全区/预算文档已随阶段同步；assets workflow/getting started 明确 LFS、预算口径、Android 构建和真机边界）
