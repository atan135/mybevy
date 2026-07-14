# UI 参考图生成与正式包边界

本文冻结“参考图 -> `UiDocument` -> 正式游戏页面”流程的工程边界。仓库当前已提供 Stage 1 `tools/ui-generation/` 独立工具、输入契约和工作目录规划；在线 provider、图片预处理、文档生成、预览接入和 `promote` 命令仍未实现。

## 目标

参考图生成流程用于在开发机或 CI 中分析图片、规划布局与视觉 token，并输出可验证的声明式 UI 草稿。页面主体是 `UiDocument` JSON，不是 AI 任意生成的 Rust 布局或业务实现。

正式交付遵循两条独立规则：

- 生成工具实现和未批准运行产物不进入正式游戏依赖图，也不随桌面或 Android 游戏包交付。
- 通过完整校验、素材授权检查和人工批准后明确晋升的 UI JSON、授权资源及必要注册适配属于正式游戏内容，会进入正式目录并随包交付。

“工具不入包”不等于“生成页面不入包”。隔离对象是生成期能力和未批准数据，正式页面仍由现有 `UiDocument` runtime 在游戏中加载。

## 规划目录和依赖方向

生成工具规划为独立 Rust 工具工程：

```text
tools/ui-generation/
  src/                  当前包含输入、状态、目录规划和依赖边界；后续承载 provider、预处理、生成、修复、评测和晋升
  fixtures/             来源明确、允许公开提交的离线 fixture
  Cargo.toml            独立工具 crate，不属于 project workspace 或 target
  Cargo.lock            工具自身依赖锁定

summary/ui-generation/<run-id>/
  input/                原始输入和规范化输入说明
  analysis/             视觉分析、布局计划和不确定项
  draft/                未批准 UiDocument 草稿
  assets/               生成期或待授权素材
  preview/              预览截图和 metadata
  logs/                 脱敏日志
  manifest              输入、版本、hash、决策和产物关系

project/
  src/framework/ui/document/             正式 UiDocument 协议与 runtime
  assets/ui/documents/approved/           已批准、随包交付的 UI JSON
  assets/...                              已批准、随包交付的 UI 资源
  src/game/...                            必要 owner/route/registration 适配
```

`tools/ui-generation/` 已建立独立 Cargo 根。`inspect-task` 当前只严格解析任务、读取图片 bytes、核对声明 metadata/SHA-256、生成高影响问题和返回目录计划，不创建 `summary/ui-generation/`。该目录已由根 `.gitignore` 的 `/summary/*` 规则覆盖。工具自身的公开 fixture 可以提交到 `tools/ui-generation/fixtures/`，但不得为了测试把参考图、模型响应或生成期素材写进 `project/assets/`；Stage 1 仅提交带明确来源说明的文本 fixture，hash 测试使用临时目录。现有 `project/assets/ui/documents/fixtures/` 仍只服务于正式协议/runtime 自身的测试，不作为生成工具的运行目录。

依赖方向只能是：

```text
ui-generation tool -> project 暴露的最小稳定 UiDocument facade
project / Android / 正式构建 -X-> ui-generation tool
```

当前最小 facade 位于 `project::framework::ui::document::tooling`，只暴露协议模型、schema version、validation report、资源预算信息和 canonical JSON 能力。后续只有在真实预览接入需要时才扩展受控预览入口。provider SDK、图片解码/EXIF、prompt、视觉分析、修复、评测、调用成本和生成日志实现只属于工具工程，不能加入 `project/Cargo.toml` 的正式依赖图，也不能注册进 `UiFrameworkPlugin`。

工具提供以下 Stage 1 命令：

```powershell
cargo run --manifest-path tools/ui-generation/Cargo.toml -- inspect-task --task <task.json> --repository-root .
cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .
```

`check-boundary` 不在外层 `cargo run` 中再次启动 Cargo。它用 TOML parser 递归遍历所有普通、target、build、dev、optional、workspace 继承和 patch/replace 的本地 path 依赖，以 canonical manifest 路径检查完整的本地依赖可达性；同时检查两侧 `Cargo.lock` 的 resolved 本地包集合和祖先 workspace，避免直接依赖扫描漏过 `project -> middle -> ui-generation`，也避免依赖网络或构建期文件锁。

`inspect-task` 的输入是 `deny_unknown_fields` 的 serde JSON。它包含页面用途、主参考图、按显式 priority 排序的多状态/多尺寸/局部参考图、目标逻辑 viewport、可见文字、必须保留内容和允许修改范围。每张图声明原始尺寸、方向、色彩空间、SHA-256、来源与授权状态；Stage 1 只校验声明和文件 bytes/hash，不解码像素或推断 EXIF。缺失的装饰处理可确定性回落到项目主题，高影响的用途、文案、保留内容、修改范围、方向、色彩空间、授权和状态转换证据会进入结构化问题，目标 viewport 缺失则直接失败。

run ID 只允许安全的小写 ASCII 标识，不接受绝对路径、`..`、路径分隔符或 Windows 保留名。目录计划固定包含 `input/`、`analysis/`、`draft/`、`assets/`、`preview/`、`logs/` 和 `manifest.json`；已有目标和通过符号链接逃逸仓库的根会被拒绝。状态模型区分 pending、输入校验、ready、running、completed、failed 和 cancelled，取消是幂等终态且在图片读取边界检查。

## 产物分类

| 内容 | 生成期位置 | 能否进入正式目录 | 是否随正式包交付 |
| --- | --- | --- | --- |
| Provider、图片预处理、prompt、分析、修复、评测和成本代码 | `tools/ui-generation/` | 否 | 否 |
| 原始参考图、模型原始响应、日志、草稿、source map、生成期素材 | `summary/ui-generation/<run-id>/` | 默认禁止 | 否 |
| 公开离线 fixture | `tools/ui-generation/fixtures/` | 仅留在工具工程 | 否 |
| 已批准 `UiDocument` JSON | 晋升前位于 run draft | `project/assets/ui/documents/approved/` | 是 |
| 已授权并批准的图片、字体和其他 UI 资源 | 晋升前位于 run assets | 对应 `project/assets/` 正式目录 | 是 |
| 经审阅的 i18n/theme 变更 | 由晋升计划生成 | 对应正式配置目录 | 是 |
| 确定性 owner/route/registration 适配 | 由封闭模板生成 | 对应 `project/src/game/` 文件 | 是，会编译进正式游戏 |

生成期 source map 用于把参考图证据、分析元素、草稿节点和预览结果关联起来，默认保留在 run 目录，不作为正式资源打包。晋升后的 `UiDocument` 仍保留稳定 document/node ID，并继续通过既有 validation report 与 audit metadata 定位正式页面问题。

## 生成和验证流程

规划流程按以下顺序执行：

1. 工具读取参考图和用户输入，在隔离 run 目录记录 hash、来源、授权状态和目标 viewport。
2. Provider 和本地处理步骤生成结构分析、布局计划、token 候选、素材计划与不确定项。
3. 工具输出 `UiDocument` JSON 草稿、source map 和必要的素材候选，不直接修改 `project/`。
4. 草稿依次通过 JSON Schema、语义、能力、action/binding、资源来源和预算校验。
5. 后续工具通过最小 facade 复用现有声明式 preview/runtime，在开发进程中生成确定性预览；Stage 1 尚未接入预览，生成器和 provider 不进入正式插件。
6. 人工处理高影响不确定项、授权问题、未知业务行为和框架能力缺口。
7. 只有验证和批准均完成的 run 才能进入受控 `promote` 流程。

参考图只能证明可见状态。隐藏交互、业务权限、响应式规则和不可见页面状态不能由模型静默猜测；无法由白名单能力表达的内容必须保留为阻塞问题或人工工作项。

## 受控晋升

后续 `promote` 子命令是唯一允许生成工具写入正式目录的入口。普通分析、生成、修复、预览和评测命令不得写入 `project/src/`、`project/assets/` 或 approved 目录。

晋升必须满足以下前置条件：

- run manifest 完整，输入、模型、prompt、schema、参数、hash、修复轮次和人工决定可追溯。
- 最终草稿通过 Schema、语义、能力、action/binding、资源和预算校验。
- 所有高影响不确定项已有明确决定；拒绝、未知授权和未知业务行为不能被默认接受。
- 目标文件、页面 owner、document ID、route 和资源 ID 的所有权明确。
- 目标 schema version 可由当前正式 runtime 读取，canonical hash 与待晋升内容一致。

`promote` 在写入前必须输出完整、可审阅的变更计划，至少列出：

- 将写入 `project/assets/ui/documents/approved/` 的 JSON。
- 将写入正式 assets 的每个资源、目标路径、hash、许可证和 Git LFS 规则。
- i18n key、theme token、owner、route、page registration 和 action/binding 注册要求。
- 与现有文件、ID、页面所有权或 schema 的冲突。
- 不会进入正式目录的 run 产物。

显示计划后必须获得显式确认，不能把模型成功、校验通过或先前的普通运行确认当作晋升授权。实现应先在临时目录构造完整结果并复验，再以事务方式落盘；任一步失败都必须回滚，不得留下部分 JSON、部分资源或未配套注册代码，也不得覆盖计划外文件。

## Rust 适配边界

页面结构、样式和控件树由 `UiDocument` JSON 表达。后续工具只允许从封闭、版本化模板产生确定性的 owner、route 和 registration 适配，并将结果作为晋升计划中的普通代码 diff 交给开发者审阅。

工具和模型不得：

- 生成任意 Rust 业务 system、命令处理器、网络调用或权限逻辑。
- 通过字符串指定 Rust 类型、函数、system、message 或反射组件。
- 为绕过校验自动扩大 action/binding allowlist 或资源预算。
- 猜测不存在的业务 action、binding path、owner 或 route。

未知 action 或 binding 必须阻塞晋升。开发者应先在游戏层实现并注册受控业务能力，再重新校验文档。即使模板适配已经晋升，正式 runtime 仍要执行现有 action、binding、owner、资源和预算检查。

## 素材、隐私和安全

- 未确认许可的参考图只能用于本地分析，不能裁切后晋升为正式资源。
- 每个候选资源都要记录来源、原图 hash、裁切或生成步骤、许可证、批准决定和最终 hash。
- 二进制正式资源继续遵守仓库 Git LFS 规则；文本 JSON、RON 和授权说明按现有约定提交。
- 凭据只从环境变量或系统安全存储读取，不写入 prompt 快照、manifest、普通日志或正式资源。
- 日志与报告要对账号文字、个人信息、访问令牌和 provider 敏感内容脱敏。
- 模型输出始终是不可信输入，structured output、人工批准和 `approved` 路径都不能替代正式 runtime 校验。

## 正式构建隔离验证

后续实现验收不能只依赖 Rust dead-code elimination。至少应提供以下证据：

- `check-boundary` 的结构化 manifest/path 完整可达图和 lockfile 检查证明 `project` 不包含 `tools/ui-generation`，且工具单向可达 `project`；发布验收仍可用 `cargo metadata`/`cargo tree` 交叉确认 provider、图片预处理和评测专属依赖未进入正式图。
- `project/Cargo.toml`、Android 壳和正式构建脚本没有反向引用工具 crate，也没有通过默认 feature 或隐式 workspace 把工具带入构建。
- 正式桌面构建和 Android `cargo ndk ... --lib` 只构建游戏 target；构建记录中没有生成工具 target。
- 包内容不包含原始参考图、模型响应、run 日志、草稿、source map 或工具 fixture。
- 已晋升 JSON、授权资源和必要注册适配能由正式游戏加载，并在桌面与 Android 包中按预期交付。

## 与预览和视觉审核的关系

后续生成阶段可以复用 [UI声明式预览与热更新.md](UI声明式预览与热更新.md) 中现有的安全 source、事务 reload、状态迁移和 audit metadata，但只能通过开发期边界调用，不能把 provider 或 generator 注册进正式 `UiFrameworkPlugin`。Stage 1 tooling facade 尚未暴露预览入口；预览成功也不会构成晋升批准或改变草稿的不可信状态。

本轮设计负责从参考图生成可验证的 `UiDocument` 及其受控晋升。参考图与渲染结果的视觉相似度判定、差异分区和审核阈值属于本地后续开发计划 `04_UI参考图视觉审核_checklist.md`，不能用“能够预览”替代视觉审核通过。

## 当前状态

截至本文更新时：

- 现有 `UiDocument` 协议、验证器、事务 runtime、preview/reload 和 audit metadata 已可供正式游戏与开发预览使用；`document::tooling` 提供不含游戏业务内部实现的最小验证/canonical facade。
- 独立 `tools/ui-generation/` 工具工程已实现 Stage 1 任务输入、metadata/hash 校验、问题列表、目录规划、状态/取消、文本 fixture 和依赖方向检查。
- `summary/ui-generation/<run-id>/` 的结构和忽略边界已有可执行规划，但 Stage 1 默认不创建用户运行产物。
- provider、参考图像素预处理、视觉分析、生成/修复/评测、预览接入和 `promote` 命令尚未实现。
- 目前不能宣称能够从参考图自动生成、批准或晋升正式 UI；实现进度以对应 checklist 和代码验证结果为准。
