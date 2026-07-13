# UI 声明式预览与热更新

本文记录 `UiDocumentPreviewPlugin` 的来源注册、开发期 watch、显式 reload、差异分类、状态迁移、审核入口和人工页面协作边界。底层解析、验证、资源预检、实体生成和清理仍由 `UiDocumentRuntimePlugin` 完成。

## 目录与来源

预览注册不接受裸 `PathBuf`。`UiDocumentSourcePath` 只允许以下逻辑根和小写 ASCII 相对 JSON 路径：

- `Approved`：`project/assets/ui/documents/approved/`，用于首包批准页面。
- `Fixture`：`project/assets/ui/documents/fixtures/`，只用于开发和测试。
- `Authoring`：`project/ui-documents/source/`，只用于人工 authoring，不进入生产加载。

路径拒绝绝对路径、盘符、反斜杠、URI、空 segment、`.`、`..`、换行和非 JSON 扩展名。每次实际 watch 读取前还会分别 canonicalize 项目根、允许根和候选文件，要求允许根仍位于项目根内、候选真实路径仍位于允许根内；symlink、junction 或 reparse point 不能把整个 source root 或字符串合法的候选路径重定向到项目外。F3、reload report 和 audit metadata 只记录逻辑路径，不记录解析后的本机绝对路径。

## 注册、reload 与 watch

游戏层通过 `UiDocumentPreviewCommand::Register` 注册 document ID、owner、逻辑 source、初始 JSON、panel/layer、target profile、page state、host binding schema 和 audit profiles。`open_on_register` 会立即使用初始 JSON 发起一次事务构建。

显式更新使用以下命令：

- `Reload`：重新提交注册表中最近一份 source bytes。
- `ReloadSource`：编辑器或受信宿主显式提交新 JSON；source path 仍来自既有注册，document 不能自行改写路径。
- `Unregister`：停止该 owner/document 的预览与 watch 注册；页面实体仍应通过 runtime `Close` 生命周期关闭。

watch 默认关闭。只有 desktop debug 构建同时满足以下条件时才能启用：

```powershell
Set-Location project
$env:MYBEVY_UI_DOCUMENT_WATCH="1"
cargo run -- --window-profile phone-small
```

release、Android 和未设置环境变量的开发启动不会监控本地路径。运行时 `SetWatchEnabled(true)` 也不能越过编译期平台门。watch 只轮询已经注册并标记 `watch: true` 的安全逻辑路径，不接受 document 内容提供的路径。根不可读、文件缺失、canonicalize/containment、metadata 或读取失败都会产生 source-stage 稳定错误；同一失败状态只报告一次，文件恢复或重新创建后会立即触发一次 reload。

## 事务与机器报告

预览层先运行完整静态校验和 effective document 解析；通过后才发送现有 `UiDocumentRuntimeCommand::Open`。runtime 继续执行 host action/binding、i18n、资源 metadata/预算和 commit 校验。新 generation 成功前旧实例保持可见；任一阶段失败都不会清理旧实例。

`UiDocumentReloadEvent` 的 report version 为 1，包含 reload/request ID、document/owner/source、前后 instance、diff、逐节点状态迁移决定和稳定 error。error 只使用 closed stage、稳定 code、可选 document path/node ID/field path，不包含 parser debug 文本或本机绝对路径。

## 稳定节点差异

`diff_ui_documents` 只比较 validated/effective document，并按稳定 node ID 构造确定性索引。分类如下：

- `no_changes`：canonical 语义一致。
- `in_place`：节点 ID、kind、父级和顺序不变，仅 layout/style 改变。
- `rebuild_subtrees`：节点内容、控件语义、插入、删除、重排、父级或 kind 改变；报告只保留最浅受影响子树根。
- `rebuild_page`：schema/document 根身份、asset/token/style/binding table 或 metadata 改变。

当前 commit 为保证资源和 ECS 原子性，所有三类实际变更都走 Stage 10 的隐藏新树 + 原子 replace。diff 分类用于状态迁移边界、审核报告和后续安全原位 adapter，不允许为了追求局部更新绕过完整验证和资源预检。

## 状态保留边界

reload 发出前按旧 instance 的稳定 node ID 快照，commit 后只向同 document/owner 的新 instance 恢复。当前支持：

- 可聚焦协议控件的焦点。
- TextInput 值、UTF-8 边界安全的光标和 selection。
- ScrollPosition。
- Slider、Stepper 和 Select 当前值。
- Checkbox、Toggle、Segmented 和 Tab 选择状态。

新节点必须 ID 和 kind 一致，且输入长度、数值范围、Select/Segmented option、Tab value、Focusable marker 等约束仍兼容。缺失、改 kind、max chars 收紧、越界、option 删除或 Tab value 变化会拒绝迁移，并在 `state_decisions` 写入稳定 reason。IME composition 和 native keyboard session 不迁移，并作为独立的 `preserved=false` decision 明确报告，不能被 TextInput 值迁移的成功结果掩盖。状态不会跨 owner、document 或旧 instance 泄漏；local binding 继续由 runtime replace 语义保留。

## F3、audit 与 Gallery

F3 的 `declarative documents` 区域和每份 audit metadata 的 `document_nodes` 都包含 document ID、schema version、node ID、source field path、安全逻辑 source path 和完整 effective style。节点按 document ID/node ID 稳定排序，可从截图 finding 直接定位回 JSON。

每次 preview `Register` 自动在 `UiDocumentAuditRecipeRegistry` 生成 `document_<document_id>` recipe，并按 `(document_id, owner)` 隔离保存 source 和受 allowlist 限制的 phone/tablet profiles；注销一个 owner 不影响同 document 的其他 owner。游戏路由仍负责把可访问页面注册到现有 `UiAuditScreenRegistry`，因为 framework 不应自行构造业务路由。直接发送 Stage 10 `UiDocumentRuntimeCommand::Open` 不会创建 preview recipe，也不会自动获得 game route。

完整示例位于：

- JSON：`project/assets/ui/documents/approved/gallery/declarative_gallery.v1.json`
- 游戏适配：`project/src/game/screens/dev/ui_document_gallery.rs`
- screen alias：`ui_document_gallery`、`document-gallery`、`declarative_gallery`

示例覆盖资源、滚动/组合布局、文本与输入、选择/数值/状态控件、local binding、白名单 action 及 compact portrait/expanded landscape 响应式 override。

多 profile 审核命令：

```powershell
.\scripts\run-ui-audit.ps1 -Screens ui-document-gallery -Devices phone-small,phone-portrait,tablet-portrait,tablet-landscape -States initial -DryRun
.\scripts\run-ui-audit.ps1 -Screens ui-document-gallery -Devices phone-small,phone-portrait,tablet-portrait,tablet-landscape -States initial
```

## 迁移与人工页面协作

preview 不提供另一套 schema migration。旧版本先按 `UI声明式文档协议.md` 的相邻版本纯数据迁移生成当前模型，再进入相同 validation/reload 流程。迁移工具不能直接覆盖 approved 文件，输出仍需经过批准流程。

声明式页面适合结构稳定、白名单控件足够、需要 AI/人工共同编辑和批量审核的页面。复杂业务状态机、特殊手势、3D/场景绑定或协议暂不支持的组件继续放在 `game/screens/` 的 Rust 页面。Rust 适配层拥有 owner、路由、action registry、host binding 和生命周期；framework document 不导入业务 enum、system 或命令实现。

## 当前限制

- diff 已冻结分类和测试，但实际 ECS commit 仍采用整页事务 replace，没有开放局部实体 patch API。
- watch 使用文件 metadata 变化检测，面向单进程开发预览，不是跨进程编辑协议或生产内容分发机制。
- TextInput 当前保留值、焦点、光标和 selection；IME composition 与 native keyboard session 不迁移，但会在 reload report 中单独记录拒绝原因。
- Scroll offset 在 commit 时先恢复非负有限值，后续 Bevy layout 会按新内容范围约束；结构显著缩短时应结合 report 和截图复核。
- 自动 recipe registry 是 framework 元数据；实际 audit screen 的可路由性仍由游戏层注册。
