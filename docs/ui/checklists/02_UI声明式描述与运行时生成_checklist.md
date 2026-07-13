# 02. UI 声明式描述与运行时生成 Checklist

## 目标

建立一套版本化、可校验、可诊断的声明式 UI 文档协议，并实现从文档到 Bevy ECS UI 实体的确定性运行时构建。该协议作为 AI 生成、人工编辑、预览、截图审核和自动修复的共同中间表示，避免 AI 直接输出任意 Rust 代码成为主要工作流。

AI 交换格式以 JSON 为主，首包或人工维护资源可以在复用同一 Serde 数据模型的前提下支持 RON。本清单不负责识别参考图，也不负责调用 AI 服务。

## 已有基础与依赖

- 复用现有 `UiFrameworkPlugin`、`UiTheme`、`UiMetrics`、widgets、Panel Manager、i18n、binding、focus 和 audit 能力。
- 依赖 `01_UI高保真视觉基础能力_checklist.md` 暴露稳定、可描述的视觉组件；协议可以先覆盖现有能力，再按版本增量扩展。
- 复用仓库已有 `serde`、`serde_json` 和 `ron`，新增依赖必须说明必要性和 Android 构建影响。
- 运行时生成的页面仍遵循 `project/src/game/screens/` 与游戏层路由边界，不把业务命令实现塞入 framework。

## 基础原则

- [x] 文档必须先完整解析和校验，再以事务方式生成实体，失败时不留下半棵 UI 树。（验证：`project/src/framework/ui/document/runtime.rs:568` 的 runtime pipeline 在 commit 前完成 static/host/resource 校验；`ui_document_runtime_resource_failure_and_cancel_never_spawn_partial_page` 和 28 项 runtime tests 通过）
- [x] 协议只表达白名单布局、样式、资源、组件、绑定和动作，不允许任意 Rust 类型名、脚本、shell 或文件访问。（验证：closed Serde 模型和 deny-by-default action/resource/source 校验已落地；`binding_action.rs:1148` 与 `preview.rs:1905` 覆盖 shell/URL/path/绝对路径逃逸拒绝）
- [x] 每个节点使用稳定 ID，所有错误、截图 metadata 和 AI 修复建议都能定位到文档路径和节点 ID。（验证：`project/src/framework/ui/document/runtime.rs:277`、`:283` 定义稳定 node/audit marker，validation report 与四 profile metadata 均携带 node ID 和 document path）
- [x] 协议版本、默认值、兼容策略和迁移规则必须显式，不依赖当前代码的隐式行为。（验证：`docs/ui/UI声明式文档协议.md` 冻结 v1、默认值、兼容 code 和相邻版本迁移规则；JSON Schema 由 Rust 类型派生且漂移测试通过）
- [x] 相同文档、资源和 viewport 必须生成确定性一致的 UI 结构。（验证：canonical JSON round-trip、effective merge、concurrent open last-request-wins、稳定 node index/diff/audit 排序测试通过，四 profile audit 使用固定 capture state）
- [x] 每个阶段独立实现、独立验证、独立提交，并补充可长期保留的 fixture。（验证：阶段 1-11 分别形成 `86e6ded`、`dc0cf6f`、`7a123e5`、`91f6281`、`2cf2283`、`023e8fa`、`97acfa0`、`08ade2e`、`dbdea2d`、`d555357`、`63bfca7`，fixtures/approved Gallery 随对应阶段提交）

## 阶段 1：协议边界和版本策略

- 开始时间：2026-07-13 09:29:13 +08:00
- 结束时间：2026-07-13 09:38:01 +08:00
- 开发总结：冻结声明式 UI 文档 v1 的职责、可信边界、JSON/RON 定位、版本兼容、目录来源、安全预算和最小 fixture；本阶段未引入 Rust 运行时实现。
- 验证记录：PowerShell `ConvertFrom-Json` 验证 fixture 通过；引用资源存在；独立运行 `git diff --check` 通过。

- [x] 定义 `UiDocument` 的职责、非目标、可信边界和与业务页面 Rust 代码的协作方式。（验证：`docs/ui/UI声明式文档协议.md:5`、`:31`、`:46`、`:61` 分别定义目标、非目标、分层协作和不可信输入处理顺序）
- [x] 确认 JSON 为 AI 交换格式，并明确 RON 是否只用于人工维护和首包资源。（验证：`docs/ui/UI声明式文档协议.md:79` 规定 JSON 为唯一 AI/工具交换格式，RON 仅用于人工维护的首包批准资源）
- [x] 定义 schema version、最低支持版本、未知未来版本和废弃字段的处理规则。（验证：`docs/ui/UI声明式文档协议.md:99` 定义当前/最低版本、未来版本拒绝、废弃字段 warning 和逐版本迁移规则）
- [x] 定义兼容等级：无损读取、需要迁移、拒绝加载，并为每种结果提供错误码。（验证：`docs/ui/UI声明式文档协议.md:113` 的兼容表定义三档结果 code，后续 reason 表细分拒绝原因）
- [x] 规定文档、生成草稿、批准资源和运行时产物的目录边界，禁止从任意绝对路径加载资源。（验证：`docs/ui/UI声明式文档协议.md:150` 定义 source/draft/approved/fixture/runtime 目录和 loader source，并拒绝绝对路径、父目录、URI 与符号链接逃逸）
- [x] 设计最小可运行示例，包含页面根、文本、图片、按钮和一个响应式变体。（验证：`project/assets/ui/documents/fixtures/minimal_page.v1.json:20`、`:45`、`:55`、`:69`、`:81` 覆盖 Container 根、Text、Image、Button 和 Compact Portrait override；JSON 解析通过）
- [x] 记录协议安全模型和资源预算边界，完成文档评审。（验证：`docs/ui/UI声明式文档协议.md:190`、`:205`、`:255` 记录 deny-by-default 安全模型、`mobile_baseline_v1` 硬预算和阶段评审结论）
- [x] 运行 `git diff --check`。（验证：2026-07-13 主 agent 独立运行 `git diff --check` 退出码 0）

## 阶段 2：核心文档模型和稳定标识

- 开始时间：2026-07-13 09:39:10 +08:00
- 结束时间：2026-07-13 10:19:48 +08:00
- 开发总结：实现声明式 UI v1 核心 Rust/Serde 模型、五类稳定 ID、节点索引和 ECS/audit marker、canonical JSON、由 Rust 类型派生的 JSON Schema 及合法/非法 fixtures。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（7 passed）、`cargo fmt -- --check`、`cargo check`、正常依赖树检查和 `git diff --check`，全部通过；`schemars` 仅存在于 dev-dependencies。

- [x] 定义 `UiDocument`、metadata、asset table、token table、root node 和可选状态集合。（验证：`project/src/framework/ui/document/model.rs:12` 定义核心文档及 metadata/assets/tokens/root/states/responsive，阶段 1 最小 fixture 可完整解析）
- [x] 定义受格式约束的 `UiDocumentId`、`UiNodeId`、`UiAssetId`、`UiStyleId` 和 `UiActionId` 新类型。（验证：`project/src/framework/ui/document/id.rs:127` 至 `:151` 定义五类 newtype，并在构造和 Serde 反序列化时强制 ASCII segment、长度和 namespace 规则）
- [x] 保证单个文档内节点 ID 唯一，且 ID 能稳定映射到 ECS marker 和审核 metadata。（验证：`project/src/framework/ui/document/validation.rs:160` 建立唯一节点路径索引，`:179`、`:185`、`:191` 定义 document/node ECS marker 与审核 metadata）
- [x] 为所有字段定义明确默认值，禁止依赖 Serde 缺省后出现平台差异。（验证：`project/src/framework/ui/document/model.rs` 的可省略字段均绑定显式 `Default`/`serde(default)`；canonical golden 测试验证空集合、null、默认 enum 和零值布局被稳定写出）
- [x] 生成或维护与 Rust 类型一致的 JSON Schema，供 AI structured output 和外部校验使用。（验证：`project/src/framework/ui/document/tests.rs:153` 从 `UiDocument` 派生 Draft 2020-12 schema 并与 `project/assets/ui/documents/schema/ui_document.v1.schema.json` 完整比对，包含 closed object、ID regex 和版本范围）
- [x] 提供 canonical JSON 序列化规则，保证字段顺序、数值格式和默认值策略可用于快照测试。（验证：`project/src/framework/ui/document/canonical.rs:5` 递归排序 object key、保持数组顺序并输出 LF 结尾；`tests.rs:118` golden/round-trip 测试通过）
- [x] 为合法文档、重复 ID、非法 ID、未知版本和缺失根节点补充测试。（验证：`project/src/framework/ui/document/tests.rs:35` 至 `:116` 覆盖合法解析、重复节点、五类非法 ID、未来/非法版本、缺失根和未知字段）
- [x] 在 `project/` 运行 `cargo fmt`、协议模型测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（7 passed）和 `cargo check` 均通过）

## 阶段 3：布局和值类型协议

- 开始时间：2026-07-13 10:21:09 +08:00
- 结束时间：2026-07-13 11:06:02 +08:00
- 开发总结：实现正式布局和值类型协议、Flex/Grid/Absolute/隐藏/滚动到 Bevy Node 与 ZIndex 的确定性映射、字段级静态验证和布局 golden fixture；第 1 轮审核补齐 responsive/state patch 的矛盾约束与 Grid 字段适用性校验。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（14 passed）、`cargo fmt -- --check`、`cargo check` 和 `git diff --check`，全部通过。

- [x] 定义受限的长度类型，覆盖 Auto、Px、Percent、Vw、Vh 和必要的 Min/Max 约束。（验证：`project/src/framework/ui/document/layout.rs:14` 定义五类 `UiLength`，`:271` 的 `UiLayout` 包含宽高及四个 min/max 字段）
- [x] 定义 Flex、Grid、Absolute、隐藏和滚动布局字段，并映射到 Bevy `Node`。（验证：`project/src/framework/ui/document/layout.rs:271` 定义受限布局字段，`:406` 在完整校验后映射 `Node` 和可选 `ZIndex`）
- [x] 支持宽高、宽高比、margin、padding、gap、border、对齐、换行、overflow 和 z-index。（验证：`project/src/framework/ui/document/layout.rs:271` 覆盖所列字段；`tests.rs:207` 表驱动断言 Flex、尺寸、间距、边框、对齐、换行、滚动和 z-index 映射）
- [x] 为 Grid 定义列、行、repeat 和 span 的可序列化表示，限制最大轨道和重复数量。（验证：`project/src/framework/ui/document/layout.rs:762` 校验定义数 32、repeat 16、展开轨道 64、span 32；`tests.rs:282` 验证 Bevy Grid 映射）
- [x] 明确 Absolute 节点的包含块、锚点和尺寸推导规则，防止参考图坐标被无约束硬编码。（验证：`project/src/framework/ui/document/layout.rs:843` 规定父 border box 且逐轴要求单锚点+尺寸或双锚点推导；`tests.rs:336` 覆盖合法、欠约束和过约束）
- [x] 对负尺寸、NaN/Infinity、非法百分比、矛盾约束和过大 z-index 返回字段级错误。（验证：`project/src/framework/ui/document/tests.rs:397` 覆盖完整布局错误，`:458` 覆盖 document、responsive/state patch 的稳定 code 与完整字段路径；第 1/5 轮审核修复通过）
- [x] 建立布局模型到 Bevy Node 的表驱动测试和 JSON golden fixture。（验证：`project/src/framework/ui/document/tests.rs:189` 至 `:395` 覆盖长度/Flex/Grid/Absolute 映射，`:548` 比对 `layout_protocol.v1.canonical.json` golden）
- [x] 在 `project/` 运行 `cargo fmt`、布局协议测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（14 passed）和 `cargo check` 均通过）

## 阶段 4：样式、视觉效果和资源引用协议

- 开始时间：2026-07-13 11:07:28 +08:00
- 结束时间：2026-07-13 12:15:24 +08:00
- 开发总结：实现 token/component/inline 样式解析与字段级优先级合并、canonical sRGB 颜色、白名单资源表和高级图片 presentation、材质 allowlist 与视觉/资源预算；第 1 轮审核补齐文字深层合并、严格路径字符白名单和全模式 focus 校验。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（19 passed）、`cargo fmt -- --check`、`cargo check` 和 `git diff --check`，全部通过且无编译 warning。

- [x] 定义 token 引用、组件 style 引用和节点 inline override 的优先级。（验证：`project/src/framework/ui/document/style.rs:197`、`:294` 实现 component extends 与文字字段级 merge，`:750` 按 component 后 inline 解析；`tests.rs:582` 验证 token、继承和 inline 优先级）
- [x] 覆盖背景、边框、圆角、文字样式、透明度、阴影、渐变和受支持材质参数。（验证：`project/src/framework/ui/document/style.rs:179` 的白名单属性和 resolved 类型覆盖全部所列视觉能力，材质参数仅支持 `frosted_panel_v1`）
- [x] 定义颜色格式和色彩空间，统一十六进制、sRGB 数值和透明度的 canonical 表示。（验证：`project/src/framework/ui/document/style.rs:8` 严格解析 hex/sRGB 并序列化为小写 `#rrggbbaa`；`tests.rs:582` 验证 sRGB 输出 `#ff800080`）
- [x] 定义图片、字体、图标、图集和材质资源条目，所有运行时路径必须来自文档 asset table 和允许目录。（验证：`project/src/framework/ui/document/asset.rs:13` 定义 typed asset entry，`:290` 及安全 path helper 限制 `ui/` 小写 ASCII 白名单相对路径、扩展名和 built-in material）
- [x] 支持九宫格、平铺、Contain、Cover、焦点裁切和图集帧描述。（验证：`project/src/framework/ui/document/asset.rs:98` 定义各 presentation 并适配现有 widget image mode；`tests.rs:582` 验证四类映射）
- [x] 对未知 token、循环引用、错误资源类型和不允许的 shader 返回稳定错误。（验证：`project/src/framework/ui/document/tests.rs:727`、`:779` 覆盖未知/循环 token、style cycle、kind mismatch、路径逃逸、未允许材质及 shader 字段拒绝）
- [x] 限制阴影层数、渐变 stop 数、材质数和资源尺寸声明，防止 AI 生成过度复杂视觉树。（验证：document 层复用 effects 的 3 层阴影/6 stop 和 widgets 图片预算，并限制 4 材质、4096px、16MiB 单资源、64MiB 总声明；`tests.rs:843` 覆盖超限）
- [x] 为样式合并、资源解析、循环引用和预算限制补充测试。（验证：`project/src/framework/ui/document/tests.rs:582` 至 `:953` 的 5 组阶段测试覆盖合并、canonical、资源、循环、安全、预算与 focus；第 1/5 轮审核修复通过）
- [x] 在 `project/` 运行 `cargo fmt`、样式协议测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（19 passed）和 `cargo check` 均通过）

## 阶段 5：文本、图片和内容节点协议

- 开始时间：2026-07-13 12:17:14 +08:00
- 结束时间：2026-07-13 13:09:35 +08:00
- 开发总结：正式定义五类基础内容节点、互斥 text source 与 typed format、排版适配、i18n catalog 校验和图片 tint/placeholder/failure 协议；第 1 轮审核恢复 16 KiB literal / 4 KiB fallback 冻结预算并修正文档合并语义。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（25 passed）、`cargo fmt -- --check`、`cargo check` 和 `git diff --check`，全部通过且无编译 warning。

- [x] 定义 Container、Text、Image、Icon 和 Spacer 等基础节点类型。（验证：`project/src/framework/ui/document/model.rs:61` 定义五类正式基础节点并保留 Button 种子，统一 ID/layout/style 遍历覆盖全部节点）
- [x] 文本节点支持 literal、i18n key、fallback、绑定 path 和格式化策略的互斥校验。（验证：`project/src/framework/ui/document/content.rs:12`、`:69` 使用 closed enum 定义三类互斥 source 与 plain/number/percent/bytes format，JSON shape preflight 提供稳定互斥错误；`tests.rs:1043` 通过）
- [x] 文本节点支持字体角色、字重、行高、对齐、换行、最大行数和溢出策略。（验证：`project/src/framework/ui/document/content.rs:156` 定义排版协议并适配现有 `UiTextStyleToken`/Bevy `TextLayout`；`tests.rs:1153` 验证映射与缺字策略）
- [x] 图片节点支持 asset 引用、fit、焦点、tint、可选 placeholder 和加载失败表现。（验证：`project/src/framework/ui/document/model.rs:79` 扩展 Image 字段，`content.rs:272` 定义失败表现，asset validator 校验主图/placeholder/failure kind；`tests.rs:1215` 验证状态解析）
- [x] 明确富文本、混合 span、emoji 和不支持排版特性的首版边界。（验证：`docs/ui/UI声明式文档协议.md:337` 的内容章节明确不支持 markup/混合 span/内联图/竖排等，并记录 emoji 与 grapheme fallback 边界）
- [x] 对超长文字、空资源 ID、非法格式化表达式和缺失 i18n key 提供诊断。（验证：`project/src/framework/ui/document/content.rs:387` 区分 16 KiB literal 与 4 KiB fallback 并返回精确 path；`tests.rs:1043`、`:1095`、`:1189` 覆盖空 ID、非法 format、长度边界和 catalog 缺键；第 1/5 轮审核修复通过）
- [x] 为中英文、长文本、缺字、图片未加载和错误资源类型建立 fixture。（验证：`project/assets/ui/documents/fixtures/content_protocol.v1.json` 覆盖中英文、长文本、emoji、loading/failure、Icon/Spacer，`invalid/content_wrong_asset_type.v1.json` 覆盖类型错误；`tests.rs:987` 与 `:1215` 读取验证）
- [x] 在 `project/` 运行 `cargo fmt`、内容节点测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（25 passed）和 `cargo check` 均通过）

## 阶段 6：控件、组件变体和状态协议

- 开始时间：2026-07-13 13:11:13 +08:00
- 结束时间：2026-07-13 14:06:39 +08:00
- 开发总结：为 15 类控件定义统一 variant/size/state/slot/children contract、稳定节点诊断和现有 widgets adapter，并新增逐组件合法/非法与完整状态 fixtures；第 1 轮审核移除无法映射的 Slider step、补数值控件直接 adapter、TextInput 初值和 Checkbox/Toggle state 一致性。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（30 passed）、`cargo fmt -- --check`、`cargo check` 和 `git diff --check`，全部通过且无编译 warning。

- [x] 为 Button、TextInput、Checkbox、Toggle、Segmented、Slider、Stepper、Scroll、Modal 入口等现有控件定义声明式节点。（验证：`project/src/framework/ui/document/model.rs:118` 至 `:237` 定义九类节点，Button 保留阶段 1 兼容字段）
- [x] 按视觉基础清单进度增加 ImageButton、Badge、Progress、Tab、Tooltip 和 Select 等组件。（验证：`project/src/framework/ui/document/model.rs:238` 至 `:307` 定义六类扩展控件，Select 映射现有 Dropdown）
- [x] 定义组件 variant、size、state 和 slot，避免协议复制控件内部实体结构。（验证：`project/src/framework/ui/document/control.rs:20` 至 `:148` 定义统一 closed contract 和声明级 children，不暴露 widgets 内部 ECS 子树）
- [x] 定义 normal、hovered、pressed、focused、selected、disabled、loading、empty 和 error 状态覆盖。（验证：`project/src/framework/ui/document/control.rs:49` 定义九态，`:384` 按现有 kind 检查合法状态与 override；`tests.rs:1523` 验证现有状态优先级）
- [x] 保证控件节点映射到现有 widgets 公共 API，而不是在文档构建器中复制交互系统。（验证：`project/src/framework/ui/document/control.rs:214` 的 adapter 复用 `resolve_control_state`、`UiControlFlags`、`UiScrollViewConfig`、`UiPanelKind` 及现有 `UiSlider::new`/`UiStepper::new`；`tests.rs:1319`、`:1572` 验证；第 1/5 轮审核修复通过）
- [x] 对不兼容 slot、无效状态、缺失 label 和不支持的嵌套返回节点级错误。（验证：`project/src/framework/ui/document/control.rs:384`、`:438`、`:536`、`:562` 返回含 code/path/node ID 的 state/slot/label/nesting/value 错误，15 个 invalid fixtures 表驱动通过）
- [x] 为每个组件维护最小合法 JSON、完整状态 JSON 和非法 fixture。（验证：`project/assets/ui/documents/fixtures/controls/minimal/` 和 `invalid/` 各含 15 个 fixture，`complete_states.v1.json` 覆盖全部合法状态；`tests.rs:1284`、`:1319`、`:1430` 逐目录读取并校验精确计数）
- [x] 在 `project/` 运行 `cargo fmt`、控件协议测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（30 passed）和 `cargo check` 均通过）

## 阶段 7：绑定和动作白名单

- 开始时间：2026-07-13 14:08:26 +08:00
- 结束时间：2026-07-13 15:32:08 +08:00
- 开发总结：完成 typed binding、document/owner/local 作用域存储与清理语义、四类 closed action、宿主注册表及游戏层适配；第 1/5 轮审核补齐 source-node 动作归属、Node allowlist、descriptor 不变量、五项 dispatch 身份匹配和 opaque string 误拒绝回归。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（38 passed）、`cargo test binding_ --lib`（25 passed）、游戏 action adapter 定向测试（1 passed）、`cargo fmt -- --check`、`cargo check` 和 `git diff --check`，全部通过。

- [x] 定义 typed binding 值，至少覆盖 String、Bool、Number、Visibility 和受限枚举。（验证：`project/src/framework/ui/document/binding_action.rs:33`、`:49` 定义 closed 类型和值；`project/src/framework/ui/core/binding.rs:720` 覆盖运行时存取）
- [x] 为 binding path 定义作用域、默认值、缺失值和生命周期清理语义。（验证：`project/src/framework/ui/document/binding_action.rs:15` 定义 document/owner/local scope 及声明语义；`project/src/framework/ui/core/binding.rs:190`、`:221`、`:246`、`:253` 实现 scoped 存取和 owner/document 清理，`:739` 测试通过）
- [x] 定义 UI 动作描述，只允许路由、关闭面板、发送已注册业务命令和更新允许的局部状态。（验证：`project/src/framework/ui/document/binding_action.rs:143` 的 `UiRegisteredActionKind` 仅含四类 closed kind，`ui_document_action_registry_validates_four_closed_kinds_and_host_bindings` 测试通过）
- [x] 建立 `UiActionRegistry`，由游戏层注册动作 ID 到具体命令，framework 不持有业务策略。（验证：`project/src/framework/ui/document/binding_action.rs:203`、`:224` 提供通用 registry/dispatch；`project/src/game/navigation/mod.rs:72`、`:96` 在 game 层注册并适配 route）
- [x] 禁止文档指定 Rust 类型、系统函数、任意消息名称、文件路径、网络地址或命令行。（验证：action 使用 tagged typed value 和 unknown-field rejection，`project/src/framework/ui/document/binding_action.rs:835` 拒绝路径/URL/网络地址/shell 能力；`:1148` 覆盖 selector、路径、URL、地址和命令行拒绝及 opaque string 正例）
- [x] 校验动作参数类型、目标节点、权限范围和当前 owner，拒绝跨页面越权操作。（验证：`project/src/framework/ui/document/binding_action.rs:554`、`:661` 校验 document/owner/参数 schema、Node allowlist 和当前文档节点；`:1003` 覆盖未知参数、错误类型、跨页和同页未授权目标）
- [x] 对未知动作、参数错误、绑定类型不匹配和 owner 已销毁补充测试。（验证：`project/src/framework/ui/document/binding_action.rs:904`、`:1003`、`:1103`、`:1210` 覆盖 binding/default/enum、unknown action/param/权限、owner 销毁及 registry 非法配置；游戏层 `project/src/game/navigation/mod.rs:690` 覆盖五类 spoof dispatch）
- [x] 在 `project/` 运行 `cargo fmt`、绑定与动作测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（38 passed）、`cargo test binding_ --lib`（25 passed）、游戏 adapter 测试（1 passed）和 `cargo check` 均通过）

## 阶段 8：响应式变体和多状态文档

- 开始时间：2026-07-13 15:34:41 +08:00
- 结束时间：2026-07-13 17:17:56 +08:00
- 开发总结：实现与 runtime viewport 共享分类 policy 的 target profile、closed safe-area/input/platform 条件、节点字段级 responsive/state patch、确定性优先级与联合条件冲突检测，以及可序列化 effective document；第 1/5 轮审核修复联合几何可满足性误报、`UiPageState` schema 边界和 viewport 双事实源。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（45 passed）、viewport 测试（9 passed）、responsive 定向测试（4 passed）、`cargo fmt -- --check`、`cargo check` 和 `git diff --check`，全部通过。

- [x] 支持按 Compact、Medium、Expanded、Short、Regular、Tall 和横竖屏选择变体。（验证：`project/src/framework/ui/core/viewport.rs:378` 提供共享分类 policy，`project/src/framework/ui/document/responsive.rs:206` 构造 document target profile；`tests.rs:159` 覆盖 480/840、600/800、正方形方向和 runtime parity）
- [x] 定义基础节点加条件 override 的合并规则，禁止复制整棵页面树表达微小差异。（验证：`project/src/framework/ui/document/responsive.rs:561` 仅按稳定 node ID 应用 `UiLayoutPatch`/`UiStylePatch` 字段，不提供新增、删除或替换树能力；`tests.rs:258` 验证合并且 source 不变）
- [x] 支持安全区、输入模式和可选平台条件，但不允许根据任意环境变量改变文档。（验证：`project/src/framework/ui/document/model.rs` 的 `UiResponsiveCondition` 仅使用 closed safe-area/input/platform enum；`project/src/framework/ui/document/tests.rs:592` 验证 environment 和开放 platform selector 被 closed parse 拒绝）
- [x] 支持页面 initial、loading、empty、error 和业务命名状态，状态切换保留稳定节点 ID。（验证：`project/src/framework/ui/document/responsive.rs:126` 定义四个标准状态和 namespaced 业务状态；`project/src/framework/ui/document/tests.rs:327` 覆盖五类状态切换及稳定 root/child node ID）
- [x] 定义多个条件同时命中时的优先级，并检测互相冲突的 override。（验证：`project/src/framework/ui/document/responsive.rs:521` 按 priority/specificity/source order 确定性应用，`:406`/`:797` 使用共享几何可满足性，`:922` 输出字段冲突；`tests.rs:413`、`:493` 覆盖同级冲突、互斥条件不误报和死条件拒绝，第 1/5 轮审核修复通过）
- [x] 对每种目标 profile 输出解析后的 effective document，供调试和审核使用。（验证：`project/src/framework/ui/document/responsive.rs:442`、`:495` 输出含 source/profile/state/applied evidence 和二次验证文档的 `UiEffectiveDocument`；`tests.rs:258` 验证稳定 JSON、证据顺序和非修改性）
- [x] 为宽高边界值、方向变化、状态切换和冲突条件补充测试。（验证：`project/src/framework/ui/document/tests.rs:159`、`:327`、`:413`、`:493`、`:556` 覆盖边界、旋转、状态、冲突/可满足性、未知节点及合并后二次校验）
- [x] 在 `project/` 运行 `cargo fmt`、响应式协议测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt -- --check`、`cargo test ui_document_ --lib`（45 passed）、viewport 测试（9 passed）、responsive 定向测试（4 passed）和 `cargo check` 均通过）

## 阶段 9：完整验证器、诊断和资源预算

- 开始时间：2026-07-13 17:19:20 +08:00
- 结束时间：2026-07-13 18:52:25 +08:00
- 开发总结：实现 bytes/UTF-8、syntax/duplicate key、structure、reference、capability、budget 分层验证，稳定可序列化报告和 `mobile_baseline_v1` 可执行预算；补齐多错误聚合、100 条截断、重复 slot、图结构边界和确定性畸形输入测试。第 1/5 轮审核修复 typed 默认字段展开被误当原始 source bytes 的入口不一致。
- 验证记录：主 agent 独立运行 `cargo test validation_tests --lib`（11 passed）、`cargo test ui_document_ --lib`（56 passed）、`cargo fmt --all -- --check`、`cargo check` 和 `git diff --check`，全部通过。

- [x] 实现语法解析、结构验证、引用解析、能力检查和预算检查的分层验证流程。（验证：`project/src/framework/ui/document/validation.rs:145`、`:149` 提供字符串/bytes 入口，`:186` 起按 bytes、syntax/duplicate key、structure、reference/capability 和 budget 顺序聚合，legacy API 保持兼容）
- [x] 为错误定义稳定 code、severity、document path、node ID、字段路径、说明和修复提示。（验证：`project/src/framework/ui/document/report.rs:13`、`:20`、`:30` 定义 closed severity/phase 及完整 diagnostic 协议，`:77` 定义稳定 report；多错误 fixture 验证路径、node ID、message/suggestion）
- [x] 限制文档字节数、节点数、树深度、子节点数、字符串长度、资源数、动画数和效果复杂度。（验证：`project/src/framework/ui/document/budget.rs:11` 至 `:25` 冻结 bytes/node/depth/children/assets/string/animation/effect 等常量，`:78`、`:216`、`:256`、`:317` 分析 source、树、patch 和 resolved style 使用；`validation_tests.rs:153`、`:227`、`:383` 覆盖）
- [x] 检测不可达节点、重复 slot、无效 action、资源循环、样式循环和明显越界布局。（验证：`project/src/framework/ui/document/report.rs:184` 原始 JSON Visitor 拒绝全部 duplicate object key；`budget.rs:505` 检测极端 px；现有 structured style/token 环与 action capability 验证被统一映射；`validation_tests.rs:49`、`:94`、`:536` 覆盖真实错误及内嵌 node/无 asset-edge 的结构不变量）
- [x] 支持一次返回多个独立错误，同时设置最大错误数避免恶意文档消耗资源。（验证：`project/src/framework/ui/document/report.rs:9`、`:104`、`:141` 实现稳定排序去重、100 条上限和 truncated；`validation_tests.rs:49` 聚合六类独立错误，`:112` 覆盖 duplicate/conflict bomb 截断）
- [x] 为 AI 提供机器可读 validation report，不把 Rust debug 字符串作为协议。（验证：`project/src/framework/ui/document/report.rs:30`、`:77` 的 Serde closed structs 只输出稳定字段，`validation_tests.rs:49` 验证 JSON report 不含 `UiDocumentError` debug 文本）
- [x] 对随机和畸形输入增加 property/fuzz 风格测试，保证验证器不 panic。（验证：`project/src/framework/ui/document/validation_tests.rs:493` 使用确定性随机 bytes、非法 UTF-8、Unicode、深嵌套、畸形 number/path 和 `catch_unwind` 验证不 panic）
- [x] 在 `project/` 运行 `cargo fmt`、验证器测试和 `cargo check`。（验证：2026-07-13 主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test validation_tests --lib`（11 passed）、`cargo test ui_document_ --lib`（56 passed）和 `cargo check` 均通过）

## 阶段 10：事务式运行时构建和生命周期

- 开始时间：2026-07-13 18:54:08 +08:00
- 结束时间：2026-07-14 00:38:16 +08:00
- 开发总结：实现声明式 UI 文档到 Bevy ECS 的事务式运行时、资源预检、完整控件树、动态状态样式、图片 fallback、实例级资源预算账本、绑定动作和生命周期清理；经 3 轮主审修复控件完整映射、可信本地状态写入、动态 state override、协议 layout 边界和 late-ready 资产唯一计账。
- 验证记录：主 agent 独立运行 `cargo test ui_document_runtime --lib`（28 passed）、`cargo test ui_document_ --lib`（84 passed）、`cargo test binding_ --lib`（29 passed）、`cargo test framework::ui::widgets::controls --lib`（64 passed）、`cargo fmt --all -- --check`、`cargo check` 和 `git diff --check`，全部通过且无编译 warning。

- [x] 实现 `UiDocument` 到 Bevy ECS 的构建器，优先调用现有 layout、widgets、style 和 panel 公共能力。（验证：`project/src/framework/ui/document/runtime.rs:1304` 事务生成文档树，`:1390` 起按节点复用 layout/style、现有 widgets 和 panel/layer marker；`:6636` 验证动态完整样式、控件 preset 与协议 layout 边界）
- [x] 构建前完成全部静态验证和资源预解析；失败时不得生成可见的部分页面。（验证：`project/src/framework/ui/document/runtime.rs:612` 先完成静态/host/i18n/effective validation，`:1034` 预检 typed 资源和实际图片 metadata，`:1304` 仅在 ready 后隐藏生成并原子显示；`:5467`、`:5491` 断言失败/取消不生成页面）
- [x] 为生成根节点添加 document ID、版本、owner、panel、layer 和 source metadata marker。（验证：`project/src/framework/ui/document/runtime.rs:214` 定义完整 root metadata，`:1327` 在根实体写入 document/version/owner/panel/layer/origin 及 framework layer/panel marker）
- [x] 为每个生成实体添加稳定 node ID marker，并维护 document node 到 Entity 的索引。（验证：`project/src/framework/ui/document/runtime.rs:227`、`:233` 定义稳定 node/audit marker，`:252` 定义实例索引，`:1377` 为每个协议节点写 marker 并登记 `UiNodeId -> Entity`；`:5423` 验证索引与 marker）
- [x] 支持关闭页面、切换 owner、资源加载失败和构建取消时清理全部实体与索引。（验证：`project/src/framework/ui/document/runtime.rs:4227`、`:4250`、`:4288` 实现 key/panel/owner 关闭，`:4332`、`:4372` 清理替换/活动实例及 binding，`:4427` 取消 pending；`:5919` 验证命令和外部 despawn 清理）
- [x] 定义重复打开、同 ID 文档替换、并发构建和部分资源异步到达的行为。（验证：`project/src/framework/ui/document/runtime.rs:612` 实现 request 去重、latest generation 和幂等打开，`:1238` 成功后替换旧实例；`:5544`、`:5590`、`:7098` 验证旧页保留、last-request-wins 和 late-ready 实例级唯一资产账本）
- [x] 统计构建时间、节点数、资源数和失败阶段，接入现有 debug/audit metadata。（验证：`project/src/framework/ui/document/runtime.rs:195` 定义机器可读 build record，`:233` 定义节点 audit metadata，`:4476` 统计 elapsed/node/asset/entity/failure stage 并通过 runtime event 发布）
- [x] 为成功构建、验证失败、资源失败、取消和清理补充集成测试。（验证：`project/src/framework/ui/document/runtime.rs:5423`、`:5467`、`:5491`、`:5919`、`:6388` 分别覆盖成功、验证失败、资源失败/取消、清理和替换清理；runtime 测试共 28 项通过）
- [x] 在 `project/` 运行 `cargo fmt`、运行时构建测试和 `cargo check`。（验证：2026-07-14 主 agent 独立运行 `cargo fmt --all -- --check`、`cargo test ui_document_runtime --lib`（28 passed）和 `cargo check` 均通过）

## 阶段 11：增量预览、热更新和审核集成

- 开始时间：2026-07-14 00:40:39 +08:00
- 结束时间：2026-07-14 04:13:06 +08:00
- 开发总结：实现安全逻辑 source、desktop debug opt-in watch、显式 reload、稳定 ID diff、事务失败回退、局部状态迁移、F3/audit metadata、owner 隔离 recipe 和完整声明式 Gallery；第 1/5 轮主审补齐 canonical containment、watch 删除/恢复诊断、recipe 多 owner 生命周期、TextInput/Segmented/Tab 状态迁移及失败/取消 snapshot 隔离。
- 验证记录：主 agent 独立运行 `cargo test ui_document_ --lib`（95 passed）、`cargo test binding_ --lib`（29 passed）、控件测试（64 passed）、audit local 测试（40 passed）、navigation 测试（14 passed）、`cargo fmt --all -- --check`、`cargo check`、audit runner SelfTest、四 profile DryRun 和 `git diff --check`，全部通过；真实 audit `stage11-real` 为 4/4 passed，逐张检查手机/平板截图无重叠或横向溢出，四份 metadata 各含 21 个声明式节点且 visual budget passed。

- [x] 建立开发期文档 watch 和显式 reload 命令，生产构建默认不监控任意本地路径。（验证：`project/src/framework/ui/document/preview.rs:36`、`:145`、`:286`、`:344`、`:521` 定义 closed source、reload/watch 命令、desktop debug 平台门、canonical containment 和去重恢复状态机；source/watch 定向测试包含在 95 项声明式测试中）
- [x] 使用稳定节点 ID 计算文档差异，区分可原位更新、需要重建子树和需要重建页面的改动。（验证：`project/src/framework/ui/document/preview.rs:1599` 基于 validated/effective document 的稳定 ID 索引生成四类 diff；`ui_document_diff_has_stable_in_place_subtree_and_page_classes` 通过）
- [x] 在安全范围内保留输入焦点、滚动位置和局部控件状态；无法保留时记录原因。（验证：`project/src/framework/ui/document/preview.rs:1050`、`:1323` 快照并恢复 focus、TextInput value/cursor/selection、scroll、Slider、Stepper、Select、Checkbox、Toggle、Segmented、Tab，逐状态输出 preserved/reason；完整迁移与拒绝测试通过）
- [x] reload 失败时继续显示上一份有效文档，并输出机器可读错误报告。（验证：`project/src/framework/ui/document/preview.rs:810`、`:1247` 串联 validation/runtime transaction 与 versioned report；资源 preflight 失败、validation/host 失败和 cancel 测试均断言旧 instance 保留）
- [x] 将 document ID、schema version、node ID、effective style 和 source path 加入 F3 或 audit metadata。（验证：`project/src/framework/ui/debug.rs:511` 输出 F3 声明式节点信息，`project/src/framework/ui/audit/local.rs:1678` 收集稳定 metadata；四 profile 实际 metadata 各含 21 个节点、schema v1 和安全逻辑 source）
- [x] 为每个声明式页面自动注册或生成可配置的 audit screen/recipe 入口。（验证：`project/src/framework/ui/document/preview.rs:262`、`:676` 在 preview registration 时按 `(document_id, owner)` 生成可配置 profiles 的 recipe；多 owner 注册和逆序注销测试通过，实际业务路由边界记录于预览文档）
- [x] 提供至少一个完整声明式 Gallery 页面，覆盖布局、资源、控件、绑定、动作和响应式状态。（验证：`project/assets/ui/documents/approved/gallery/declarative_gallery.v1.json` 覆盖批准资源、组合布局、15 类内容/控件、local binding、白名单 action 和两档 responsive override；`project/src/game/screens/dev/ui_document_gallery.rs:20` 接入路由/runtime，完整性测试通过）
- [x] 更新 `docs/ui/`，记录协议、目录、调试、限制、迁移和人工页面协作方式。（验证：`docs/ui/UI声明式预览与热更新.md:1` 的九节覆盖 source、命令、事务、diff、迁移、audit、人工协作和限制，并同步 README、协议、调试验收及当前限制）
- [x] 运行多 profile UI audit、`git diff --check`、`cargo fmt`、相关测试和 `cargo check`。（验证：`stage11-real` 在 phone-small、phone-portrait、tablet-portrait、tablet-landscape 4/4 passed；主 agent 独立完成 95/29/64/40/14 项 focused tests、format、check、SelfTest、DryRun 和 diff check）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有阶段完成后统一验收。

- 开始时间：2026-07-14 04:14:59 +08:00
- 结束时间：2026-07-14 04:16:35 +08:00
- 验收总结：声明式 UI v1 已形成从 JSON/RON 模型、schema、验证、预算和白名单，到响应式 effective document、事务 ECS runtime、绑定动作、preview/reload、状态迁移与 audit 的完整闭环；批准 Gallery 在手机和平板四档真实 audit 中 4/4 通过，所有 focused tests、格式和编译检查通过。

- [x] 一个不包含 Rust 页面布局代码的声明式 UI 文档可以生成可交互、可路由、可审核的 Bevy 页面。（验证：`declarative_gallery.v1.json` 独立描述布局和控件，`ui_document_gallery.rs:20` 仅负责 owner/profile/路由生命周期适配；action、控件及真实 audit 通过）
- [x] JSON Schema、Rust 数据模型、验证器和文档示例保持一致，并有自动测试防止漂移。（验证：`ui_document_schema_matches_rust_model`、canonical round-trip、完整 Gallery validate 测试包含在 95 项声明式测试中并通过）
- [x] 任意非法、超预算或越权文档在生成实体前被拒绝，且返回节点级机器可读诊断。（验证：11 项 `validation_tests` 覆盖聚合诊断、全部 collection/payload/effect 预算和畸形输入；28 项 runtime tests 覆盖 validation/host/resource 失败零残留）
- [x] 文档不能触发任意代码、shell、网络请求、绝对路径读取或未注册业务动作。（验证：closed action descriptor/registry、asset/source logical root 与 canonical containment 测试通过；binding 29 项及声明式安全输入测试通过）
- [x] 相同文档、viewport、主题和资源生成确定性一致的节点树和截图状态。（验证：canonical/effective merge、runtime index、diff 与 metadata 稳定排序测试通过；audit runner 固定 profile/state 并输出可复核 manifest）
- [x] 热更新失败不会破坏上一份有效 UI，页面退出后无生成实体或索引残留。（验证：preview validation/host/resource/cancel 测试断言旧 instance 保留；`runtime.rs:5990` 覆盖 Close 与外部 despawn 后实体和索引清理）
- [x] 手机与平板 profile 下的声明式示例页面无重叠、关键裁切或不可达交互。（验证：真实 audit `stage11-real` 的 phone-small、phone-portrait、tablet-portrait、tablet-landscape 4/4 passed；主 agent 逐张检查截图，页面由 Scroll 根承载纵向内容且无横向溢出或控件互相覆盖）
- [x] audit metadata 能从截图问题定位到 document ID、node ID 和源字段路径。（验证：`project/src/framework/ui/audit/local.rs:1678` 收集 document metadata；四份实际 metadata 各含 21 个按 ID 排序节点、schema v1、document path、safe source 和 effective style）
- [x] `cargo fmt`、协议/验证器/运行时 focused tests、`cargo check` 和 UI audit 全部通过。（验证：主 agent 独立运行声明式 95、validator 11、runtime 28、binding 29、控件 64、audit 40、navigation 14 项测试，format/check/diff/SelfTest/DryRun 通过，真实 audit 4/4 passed）
- [x] `docs/ui/` 完整记录协议版本、兼容策略、安全边界和当前限制。（验证：协议、预览热更新、调试验收、当前限制和 README 已同步，覆盖 v1 兼容、目录/source、deny-by-default、迁移、人工协作及整页事务 replace 等限制）
