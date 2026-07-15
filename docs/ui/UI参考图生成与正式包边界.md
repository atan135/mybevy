# UI 参考图生成与正式包边界

本文冻结“参考图 -> `UiDocument` -> 正式游戏页面”流程的工程边界。仓库当前已提供 Stage 1-7 `tools/ui-generation/` 独立工具、输入契约、工作目录规划、provider 无关调用协议、图片预处理、结构化视觉分析、确定性页面规划、素材策略和受控 `UiDocument` 生成验证；在线供应商适配、生成 CLI 落盘、有限修复、预览接入和 `promote` 命令仍未实现。

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
  src/                  当前包含输入、状态、目录、依赖边界、provider、预处理、分析、页面规划和素材策略；后续承载文档生成、修复、评测和晋升
  assets/               工具侧只读、版本化的正式 UI asset ID metadata；不包含正式资源副本
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

`tools/ui-generation/` 已建立独立 Cargo 根。`inspect-task` 当前只严格解析任务、读取图片 bytes、核对声明 metadata/SHA-256、生成高影响问题和返回目录计划，不创建 `summary/ui-generation/`。该目录已由根 `.gitignore` 的 `/summary/*` 规则覆盖。工具自身的公开 fixture 可以提交到 `tools/ui-generation/fixtures/`，但不得为了测试把参考图、模型响应或生成期素材写进 `project/assets/`；当前只提交带明确来源说明的文本任务/provider fixture，hash 测试使用临时目录。现有 `project/assets/ui/documents/fixtures/` 仍只服务于正式协议/runtime 自身的测试，不作为生成工具的运行目录。

依赖方向只能是：

```text
ui-generation tool -> project 暴露的最小稳定 UiDocument facade
project / Android / 正式构建 -X-> ui-generation tool
```

当前最小 facade 位于 `project::framework::ui::document::tooling`，只暴露协议模型、schema version、validation report、资源预算信息和 canonical JSON 能力。后续只有在真实预览接入需要时才扩展受控预览入口。provider SDK、图片解码/EXIF、prompt、视觉分析、修复、评测、调用成本和生成日志实现只属于工具工程，不能加入 `project/Cargo.toml` 的正式依赖图，也不能注册进 `UiFrameworkPlugin`。

工具当前提供以下输入、预处理和边界命令：

```powershell
cargo run --manifest-path tools/ui-generation/Cargo.toml -- inspect-task --task <task.json> --repository-root .
cargo run --manifest-path tools/ui-generation/Cargo.toml -- preprocess-task --task <task.json> --options <preprocess.options.json> --repository-root .
cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .
```

`check-boundary` 不在外层 `cargo run` 中再次启动 Cargo。它用 TOML parser 递归遍历所有普通、target、build、dev、optional、workspace 继承和 patch/replace 的本地 path 依赖，以 canonical manifest 路径检查完整的本地依赖可达性；同时检查两侧 `Cargo.lock` 的 resolved 本地包集合和祖先 workspace，避免直接依赖扫描漏过 `project -> middle -> ui-generation`，也避免依赖网络或构建期文件锁。

`inspect-task` 的输入是 `deny_unknown_fields` 的 serde JSON。它包含页面用途、主参考图、按显式 priority 排序的多状态/多尺寸/局部参考图、目标逻辑 viewport、可见文字、必须保留内容和允许修改范围。每张图声明原始尺寸、方向、色彩空间、SHA-256、来源与授权状态；Stage 1 只校验声明和文件 bytes/hash，不解码像素或推断 EXIF。缺失的装饰处理可确定性回落到项目主题，高影响的用途、文案、保留内容、修改范围、方向、色彩空间、授权和状态转换证据会进入结构化问题，目标 viewport 缺失则直接失败。

run ID 只允许安全的小写 ASCII 标识，不接受绝对路径、`..`、路径分隔符或 Windows 保留名。目录计划固定包含 `input/`、`analysis/`、`draft/`、`assets/`、`preview/`、`logs/` 和 `manifest.json`；已有目标和通过符号链接逃逸仓库的根会被拒绝。状态模型区分 pending、输入校验、ready、running、completed、failed 和 cancelled，取消是幂等终态且在图片读取边界检查。

Stage 2 的 provider 协议用 `visual_analysis` 和 `structured_generation` 两种供应商无关请求隔离模型名称、SDK 请求和原始响应。请求、图片 bytes、prompt 和结构化输入没有序列化实现；普通 trace 只允许记录 run/prompt/schema 版本、图片数量/总字节数、尝试结果、耗时和经过字符校验的服务端 request ID。统一 runner 强制单次超时、外部取消、本地最小请求间隔和最多 10 次的有限重试，并只重试 timeout、rate limit 和 service unavailable。凭据只由环境变量或注入的系统安全存储读取，secret 的 `Debug`/`Display` 恒为 `[REDACTED]`。当前没有在线供应商 SDK 或网络适配；离线 `FixtureProvider` 和 `MockProvider` 用于本地与 CI。

Stage 3 的 `preprocess-task` 只接受内容识别为 PNG/JPEG、编码体积不超过 64 MiB、单边不超过 16384 px 且解码像素不超过 2400 万的参考图。工具读取编码尺寸、原始/解码色彩类型、alpha、EXIF 方向和 ICC profile 的长度/hash；任务声明方向与实际 EXIF 冲突会失败，无 EXIF 时才使用已确认的任务声明。标准副本固定输出确定性 RGBA8 PNG，但当前不执行或声称未经验证的 ICC 色彩转换，manifest 会同时保留声明色彩空间和嵌入 profile 证据。

裁切、安全区和系统 UI 排除区均来自严格 options JSON，坐标统一使用完整 EXIF 归一化图的左上原点像素边界，不根据内容猜测。每张参考图最多允许 64 个系统 UI 排除区，超限在解码和绘制前失败，避免 options 放大辅助图绘制成本。manifest 明确记录原图像素、EXIF 归一化像素、预览像素、目标 logical px 和 device physical px 的尺寸、比例、裁切偏移与舍入规则；Raw 与 EXIF 空间保留全图往返，预览、logical 和 physical 空间只接受 crop 内坐标。超大图按固定 max edge/像素预算等比缩小，同时保留原尺寸和映射。网格、区域编号和高对比图是带 `auxiliary_only` 的独立 artifact，不能替代原图或标准预览作为后续证据。

预处理 cache key 绑定输入 SHA-256、预处理协议/实现版本、声明 metadata、reference ID、目标 viewport、页面/局部验证 profile 和全部输出选项。cache 与 run 都先写同卷 staging 目录再 rename，已有 run 不覆盖，损坏 cache 不静默复用；所有输出固定在被忽略的 `summary/ui-generation/` 下。页面/状态/viewport 参考图会拒绝无可见变化的空白页；局部 detail 允许纯色素材，避免把合法色块误判为空白。

Stage 4 的 `UiReferenceAnalysis` 只存在于工具 crate，是独立于正式 `UiDocument` 的不可信中间协议。每份分析绑定 reference ID、原图 hash、预处理 cache key、manifest hash、标准预览 hash/尺寸和明确的 `standard_preview_pixel` 左上原点坐标语义；区域和元素的 bounding box 不能脱离这些证据单独出现。元素图记录单根父子层级、区域、视觉种类、固定锚定/内容流/比例伸缩/滚动/绝对装饰布局行为、对齐线索、重复模式、候选组件、置信度和可回指 reference 区域、provider request ID 或人工输入的证据。纯 Schema/语义解析只证明输出内部一致，不赋予来源信任；provider 集成必须使用 `ProviderExecution` 的实际 provider ID、response request ID、request prompt version、原始 `GenerationTask` 和 Stage 3 manifest 证据构造不可反序列化的 trusted context，并逐项交叉校验。provider 未返回 request ID 时不能建立可追溯分析来源，集成校验会明确失败。

分析协议单独保留 OCR/model 原始文字候选、候选置信度、人工文字、`human_input_id` 和采用策略。可信人工输入由 `GenerationTask.visible_text` 按顺序确定性派生为 `task.visible_text.XXXX -> text`，分析中的人工文字必须绑定该 ID、通过元素 evidence 链接，并与 trusted context 中的原文逐字相同；模型自填一组互相匹配的文字和 human evidence 仍会被集成校验拒绝。人工文字存在时必须保持权威；冲突必须产生关联的 `text_conflict` uncertainty。遮挡、模糊、裁切、未知字体和隐藏交互都有显式、可关联的 uncertainty 类型，不得通过模型默认值静默消失。Rust 类型通过 `schemars` 生成 Draft 2020-12 JSON Schema，输入先受 2 MiB、结构深度、节点和字符串预算约束，再执行 Schema 与语义校验；语义阶段检查 ID 唯一性、单根无环图、最大 24 层、坐标边界、引用闭包、重复序号和文字权威性。公开预算常量由生成 Schema 的精确 constraint 测试和边界值测试共同锁定。当前只用仓库自编 fixture/Mock 验证协议，不调用真实 OCR/在线模型，也不生成 `UiDocument`。

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

### Stage 5 规划协议

`tools/ui-generation/src/planning.rs` 将已通过 Stage 4 校验的分析结果转换为有预算、顺序稳定的页面规划。它聚类可见几何得到字号、间距、重复尺寸、圆角和边框候选，按重复 pattern 保留组件实例到参考元素的 source mapping，并输出父子、尺寸、锚点、对齐、间距、伸缩和滚动约束。计划步骤固定按结构、视觉、装饰排序。

规划器通过 `UiDocument` tooling facade 的只读 catalog 匹配现有 theme token 和正式协议实际支持的 widget variant。variant catalog 与 `UiDocument` semantic validator 共用支持矩阵；label、card、list、list item 等没有正式 component variant 的候选不会被标记为全局复用。匹配成功才建议复用全局项；未匹配 token 默认限制为页面作用域，未匹配的重复组件限制为组件作用域。

每个 token 都携带稳定的 `origin`：`observed_geometry` 仅表示来自 bounding box/父子位置的几何值，`existing_catalog_suggestion` 表示按视觉角色或控件类型提出的现有主题建议，`heuristic_assumption` 表示字号比例、默认阴影等启发式假设。颜色、圆角、边框和阴影目前不是像素测量结果；后续阶段不得把 catalog 建议或启发式值当作参考图视觉证据。在线视觉测量仍不属于当前能力。

规划诊断会稳定报告同轴矛盾对齐、固定宽度双边锚定、过度绝对定位和子元素不可能最小尺寸。规划器本身不会写入 `project/assets/`；Stage 6 素材策略以其输出和同一份 analysis ID 为后续输入。

### Stage 6 素材策略

`tools/ui-generation/src/asset_strategy.rs` 为每个分析元素确定性记录 `existing_asset`、`programmatic`、`authorized_crop`、`recreate`、`generate` 或 `placeholder` 六类处置。未获得显式资源匹配的图片类元素不会猜路径或静默消失，而是生成带诊断的 placeholder；表面、文字、边框和状态等可表达内容默认归入程序化表现。重制和生成规格固定记录像素尺寸、alpha、nine-slice 边距、sRGB 要求和用途。生成 provenance 只允许受控 subject/style tag 摘要，记录工具 ID/版本、许可证和人工审核状态，不保存完整 prompt，新草稿也不能把自身标记为已批准。

工具侧 `assets/ui_asset_catalog.v1.json` 为当前正式 `project/assets/ui/{atlas,icons,images,fonts}` 建立稳定 asset ID、hash、尺寸、alpha、许可证和检索 tag。加载 catalog 时会递归复验生产资源全覆盖、文件 hash/metadata、许可文件、任意层大小写碰撞、重复 ID、路径逃逸和符号链接，并对目录深度和遍历条目设置显式预算。查询结果只返回稳定 ID；策略匹配不接受模型提供的 packaged path。图标、背景、内容图、装饰和 nine-slice 还必须分别匹配 catalog 的用途 tag，程序化替代与输出规格用途也必须和分析元素 kind 兼容；无法证明兼容时拒绝该决定，由调用方显式保留 placeholder/review。既有背景和 atlas 中尚无许可记录的文件明确保留 `unknown` 并产生审核诊断，不把“文件已经在包内”等同于许可已确认。

局部裁切采用 fail-closed 授权：只有任务明确标记 `derivatives_allowed` 且带许可/授权记录时才能建立 crop；`analysis_only`、`distribution_allowed`、`unknown`、`denied` 和缺许可引用全部拒绝。素材策略入口会先复验 task 和 analysis 语义、planning protocol/analysis ID，重算 Stage 3 cache key 和完整坐标映射，再交叉核对 source/manifest/标准预览 hash、实现版本、尺寸和坐标约定，将 Stage 4 preview bbox 按向外取整映射到 EXIF-normalized 坐标。catalog JSON、manifest、artifact 数量/字节/像素和目录遍历都在解析或分配前受显式预算约束。实际像素只从 run 内精确匹配 hash 的标准预览读取，经同目录 staging 和文件 flush/sync 后，以 hard-link create-if-absent 进行原子 no-clobber 提交，再删除 staging；已有目标或竞争窗口内新建目标都不会被覆盖。draft asset ID 使用带固定前缀的无填充 base32 无损映射为跨平台文件名，`a.b` 与 `a_b` 不碰撞。规格与裁切尺寸不一致时不隐式缩放，写前写后还会复核正式 `project/assets/` 文件 hash 快照。

PNG/JPEG 草稿检查会在解码前拒绝超编码字节或超 Android 尺寸/像素预算，并通过受限解码器约束分配；检查结果稳定报告 8-bit Android 颜色类型、alpha 要求、透明边缘和透明 RGB bleed、APNG、ICC/sRGB 证据，以及 JPEG 有损压缩审核项。无法证明颜色转换时输出 `review_required`，不会声称已经完成 ICC 转换；超出 Android 纹理预算、格式/解码失败或规格不符则拒绝。

### Stage 7 结构化 UiDocument 生成

`tools/ui-generation/src/generation.rs` 将已验证的 `UiReferenceAnalysis`、确定性布局计划、token/组件建议和 Stage 6 素材表组成 provider `structured_generation` 请求。请求携带固定版本的输出契约、目标 `UiDocument` schema 版本、literal-only 文本策略、stable asset allowlist 和由参考元素 ID 确定性派生的 node ID/source map；不从 Markdown 代码块或自然语言中截取 JSON。

provider 响应先受独立的 envelope 字节、深度、节点、容器和字符串预算约束，再交给游戏工程 `document::tooling` facade 执行正式 canonical JSON、Schema、语义、能力和资源预算验证。工具不复制正式 `UiDocument` 协议验证器；正式验证后的 canonical 结构还必须精确覆盖 source map 的节点集合与父子层级。所有可见文字只允许使用分析阶段明确采用的 literal，未解决文字保持 unsupported；生成阶段禁止新增 i18n key、binding 或 action。参考图中看似可交互但没有受信任业务注册的元素只产生未实现项，不会凭空绑定行为。

素材只允许使用 Stage 6 已明确选中的 stable asset ID，并逐项复核 catalog 中的 packaged path 和类型；placeholder、待生成、待重制或待授权裁切项不能偷偷引用正式资源。输出独立记录 assumptions、unimplemented states、required new components 和 unsupported capabilities；合并预算由 provider、analysis、planning 和 asset strategy 的公开最大预算推导，排序去重后不得截断明细，协议预算漂移导致越界时稳定失败。trace 只保留经过安全 label 约束的 provider/model/prompt/schema 标识、组合输入 hash、受控生成参数、server request ID 和 canonical 文档 hash，不保存完整 prompt 或原始响应。

当前 Stage 7 是工具库 API 和离线 fixture 验证链路：最小页面、复杂嵌套页面、正式协议非法输出、不支持能力、注册组件 variant、stable asset path、source map 碰撞和 Markdown fence 均有测试。它尚未提供生成 CLI、在线模型适配、草稿目录原子落盘、有限修复或进程内预览；这些能力分别留给后续阶段。

截至本文更新时：

- 现有 `UiDocument` 协议、验证器、事务 runtime、preview/reload 和 audit metadata 已可供正式游戏与开发预览使用；`document::tooling` 提供不含游戏业务内部实现的最小验证/canonical facade。
- 独立 `tools/ui-generation/` 工具工程已实现 Stage 1 任务输入与依赖方向检查、Stage 2 provider 安全协议、Stage 3 受限图片解码与坐标/缓存、Stage 4 结构化视觉分析 Schema/语义校验/离线 fixture、Stage 5 确定性 token/组件/布局规划、Stage 6 稳定 asset ID catalog、六类素材策略、授权裁切与草稿质量检查，以及 Stage 7 结构化 `UiDocument` 生成、正式 facade 验证、source map 和生成追踪。
- `inspect-task` 默认不创建用户运行产物；`preprocess-task` 会创建被忽略的 `summary/ui-generation/<run-id>/input/preprocessed/` 和 `.cache/preprocess/`，不会写入正式游戏目录。
- 在线 provider/OCR/图片生成适配、生成 CLI/草稿原子落盘、有限修复/评测、预览接入和 `promote` 命令尚未实现；当前结构化生成能力通过工具 API 和离线 fixture 验证，不会自动写入正式目录。
- 目前不能宣称已经具备从参考图自动生成并落盘、批准或晋升正式 UI 的端到端流程；实现进度以对应 checklist 和代码验证结果为准。
