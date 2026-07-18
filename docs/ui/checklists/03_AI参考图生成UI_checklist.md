# 03. AI 参考图生成 UI Checklist

## 目标

实现一套以参考图为主要输入、以经过严格验证的 `UiDocument` 和受控 UI 素材为输出的 AI 生成流程。流程应能够拆解页面结构、提取视觉特征、生成响应式布局和控件状态，并在本地预览中给出可追溯结果。

生成器默认不直接修改 Rust 源码，也不自动覆盖正式 UI 文档或游戏资源。生成草稿进入隔离工作目录；通过验证和明确批准后，由受控晋升命令把 `UiDocument`、授权资源和必要的确定性注册适配写入正式游戏目录，并随游戏构建打包。

AI 生成器本身是仓库级桌面/CI 开发工具，不属于正式游戏运行时。生成、provider、图片预处理、评测和修复代码统一放在独立的 `tools/ui-generation/` Rust 工具工程；正式游戏只保留已有 `UiDocument` 运行时和经过人工批准后明确晋升的 JSON/资源。

## 已有基础与依赖

- 依赖 `01_UI高保真视觉基础能力_checklist.md` 提供可复用视觉原语和组件。
- 依赖 `02_UI声明式描述与运行时生成_checklist.md` 提供 JSON Schema、验证器和运行时预览。
- 可复用现有 UI audit 截图、设备矩阵、metadata 和报告结构，但视觉相似度审核由 `04_UI参考图视觉审核_checklist.md` 负责。
- AI 服务应通过可替换 provider 接口接入，不在协议和业务页面中绑定单一供应商。
- 工具工程可以单向依赖游戏工程公开的最小 `UiDocument` 校验/预览 API；`project`、Android 壳和正式构建脚本不得反向依赖工具工程。

## 正式包隔离与工程边界

- [x] 在 `tools/ui-generation/` 建立独立 Rust 工具工程，确保它不是 `project` 的依赖、Android `--lib` 构建目标或正式游戏插件。
- [x] provider、图片解码/EXIF、prompt、评测和其他生成期依赖只声明在工具工程中，不加入 `project/Cargo.toml` 的正式依赖图。
- [x] 依赖方向保持为 `ui-generation tool -> project UiDocument public facade`；公开 facade 只暴露复用所需的稳定协议、校验和预览能力，不暴露游戏业务内部实现。
- [x] 参考图、模型响应、日志、草稿、source map 和生成期素材不写入 `project/assets/`；可提交 fixture 放在工具目录，私有运行产物放在被忽略的 `summary/ui-generation/`。
- [x] 正式桌面/Android 构建必须通过依赖图和构建记录证明未编译、链接或打包 `tools/ui-generation/`；只有人工批准并明确晋升的 JSON、资源和必要注册适配可以进入正式包。

## 基础原则

- [x] AI 输出只能落入受控 staging 目录，默认不得修改 `project/src/`、正式 assets 或已批准 UI 文档。
- [x] 所有模型输出必须通过结构化 Schema、语义验证和资源预算检查，不能直接信任自然语言结果。
- [x] 参考图只能证明可见状态；隐藏交互、响应式规则和业务行为必须标记为假设或由额外输入提供。
- [x] 未确认授权的参考图素材不得直接裁切并作为正式游戏资源提交。
- [x] 模型、prompt、schema、输入图片和生成参数必须可追溯，支持复现和问题定位。
- [x] 在线模型不可用时，fixture 和 mock 流程仍能完成本地测试与 CI 验证。

## 阶段 1：生成任务输入契约和工作目录

- 开始时间：2026-07-14 10:18:11 +08:00
- 结束时间：2026-07-14 14:09:41 +08:00
- 开发总结：建立独立 `tools/ui-generation/` Rust CLI、严格任务输入/参考图 metadata 与 hash 校验、高影响问题列表、安全 run 目录规划、稳定生命周期/失败码，以及游戏侧最小 `UiDocument` tooling facade 和结构化反向依赖防回归检查；Stage 1 仅检查与规划，不创建运行目录或修改正式游戏文件。
- 验证记录：`cargo test --manifest-path tools/ui-generation/Cargo.toml` 20/20 通过；工具 crate `cargo fmt --all -- --check`、`cargo check` 通过；`check-boundary` 报告五项依赖/lock/workspace 边界均为 `true`；`project/` 的 `cargo test ui_document_tooling_facade --lib` 1/1、`cargo fmt --all -- --check`、`cargo check` 通过；仓库 `git diff --check` 通过。

- [x] 创建 `tools/ui-generation/` 独立 Rust crate 和 CLI 入口，明确它不属于 `project` 的正式 target、feature 或依赖。（验证：`tools/ui-generation/Cargo.toml` 使用独立 package/lock/target，`src/main.rs` 提供 `inspect-task` 与 `check-boundary`；边界报告确认独立 workspace）
- [x] 为游戏工程提供最小、稳定的 `UiDocument` 工具 facade，保持工具到游戏的单向依赖并添加反向依赖防回归检查。（验证：`project/src/framework/ui/document/tooling.rs` 仅导出协议/校验/canonical API；`boundary.rs` 递归检查 manifest path 图、lockfile 与 workspace，真实 `check-boundary` 五项均通过）
- [x] 定义生成任务输入，至少包含页面用途、参考图、目标 viewport、可见文字、必须保留内容和允许修改范围。（验证：`contract.rs::GenerationTask` 定义全部必需字段并使用 `deny_unknown_fields`；严格解析测试通过）
- [x] 支持一张主参考图和可选的多状态、多尺寸、局部细节参考图，并定义它们的优先级。（验证：`primary_reference`、`additional_references`、`AdditionalReferenceRole` 和 `ordered_references` 实现主图优先及数值/角色/ID 确定性排序；排序测试通过）
- [x] 为输入图片记录原始尺寸、方向、色彩空间、文件 hash、来源和素材使用授权状态。（验证：`ImageInputMetadata`/`ImageProvenance` 覆盖全部字段，`verify_reference_files` 分块计算 SHA-256；metadata/hash 测试通过）
- [x] 定义不完整输入的默认规则，只对低风险视觉细节使用默认值，高影响不确定项必须进入问题列表。（验证：`GenerationTask::assess` 仅对装饰/表面细节使用主题默认值，并为用途、文案、保留项、修改范围、方向、色彩空间、授权和状态转换证据生成结构化问题；空白证据与精确 JSONPath 回归测试通过）
- [x] 规划 `summary/ui-generation/<run-id>/` 工作目录，包含 input、analysis、draft、assets、preview、logs 和 manifest。（验证：`RunDirectoryPlan::plan` 固定生成 8 个受控路径且不创建目录；目录规划测试通过）
- [x] 确保工作目录默认被 Git 忽略，不把用户参考图、模型日志或临时素材意外提交。（验证：根 `.gitignore` 的 `/summary/*` 命中 `summary/ui-generation/test/input/reference.png`；`summary_generation_outputs_are_git_ignored` 测试通过）
- [x] 规定可提交 fixture 位于 `tools/ui-generation/fixtures/`，不得为了测试把参考图或生成期资源放入 `project/assets/`；二进制 fixture 仍需记录来源并遵守 Git LFS 约定。（验证：`fixtures/task.valid.json` 与 `fixtures/README.md` 仅提供有来源说明的文本契约 fixture，图片 hash 测试使用临时目录，未新增 `project/assets/` 生成期资源）
- [x] 定义任务状态、失败类型和取消语义，包含输入非法、图片不可读、目标尺寸缺失和输出目录冲突。（验证：`lifecycle.rs` 定义稳定 `TaskStatus`、`TaskFailureKind`/code 和幂等终态取消；状态、失败码和取消边界测试通过）
- [x] 为任务解析、hash、目录规划和失败分类补充测试。（验证：工具 crate 共 20 个测试，覆盖严格解析、优先级、问题路径、hash、不可读图片、目录冲突/逃逸、失败码、生命周期和依赖边界，20/20 通过）
- [x] 在工具 crate 运行 `cargo fmt --manifest-path tools/ui-generation/Cargo.toml --all -- --check`、任务输入测试和 `cargo check --manifest-path tools/ui-generation/Cargo.toml`；在 `project/` 运行 facade 相关测试、`cargo fmt --all -- --check` 和 `cargo check`，并运行 `git diff --check`。（验证：2026-07-14 14:09 +08:00 全部命令退出码 0；工具测试 20/20、project facade 测试 1/1）

## 阶段 2：AI Provider、凭据和离线 Fixture

- 开始时间：2026-07-14 14:11:19 +08:00
- 结束时间：2026-07-14 15:15:31 +08:00
- 开发总结：实现供应商无关的视觉分析/结构化生成 provider 协议、能力检查、安全请求 metadata、环境变量/安全存储凭据边界、单次超时与取消、本地限速、最多 10 次有限重试、稳定错误映射与 request ID trace；加入可提交的文本 Fixture provider 和覆盖故障场景的 Mock provider，不引入在线 SDK 或正式游戏依赖。
- 验证记录：第 1/5 轮审核复现限速墙钟测试不稳定（36/37），返修为注入确定性时钟并精确断言 12ms 等待；返修后主 agent 独立运行工具测试 37/37、工具 `cargo fmt --all -- --check`、工具/`project` `cargo check`、`check-boundary` 和 `git diff --check` 全部通过，且 `project/Cargo.toml`、`project/Cargo.lock`、Android 工程无 diff。

- [x] 定义 provider 无关的视觉分析与结构化生成接口，隔离模型名称、请求格式和响应细节。（验证：`provider/mod.rs` 定义 `Provider` trait、`VisualAnalysis`/`StructuredGeneration` 请求和结构化输出契约；公共协议不含供应商模型名、SDK 类型或原始响应格式）
- [x] 支持超时、取消、有限重试、速率限制、错误分类和服务端 request ID 记录。（验证：`provider/runner.rs` 强制单次超时、取消轮询、最多 10 次重试和按 provider 限速；`ProviderErrorKind`/`TaskFailureKind` 稳定映射并在每次 trace 保留受校验 request ID；runner 测试通过）
- [x] 通过环境变量或系统安全存储读取凭据，禁止写入仓库、manifest、prompt 快照和普通日志。（验证：`credentials.rs` 的 `CredentialResolver` 支持环境变量与注入式 `SecureCredentialStore`；`SecretString` 无 Serialize，`Debug`/`Display` 恒为 `[REDACTED]`；凭据测试通过）
- [x] 定义允许记录的请求 metadata，不默认持久化完整敏感图片或模型原始响应。（验证：`RequestLogMetadata` 只记录受控标识、operation、prompt/schema 版本、图片数量/总字节数和输入类型；请求无 Serialize 且脱敏 Debug，trace 不包含 prompt、图片 bytes、结构化输入或响应 value；脱敏测试通过）
- [x] 实现 Fixture provider，能够从固定输入读取合法、非法、超预算和中断响应。（验证：`provider/fixture.rs` 严格读取 `fixtures/providers/{valid,invalid,over_budget,interrupted}.json`，校验 fixture 来源/敏感标记/类型对应；fixture 测试通过）
- [x] 实现 Mock provider，覆盖超时、限流、鉴权失败、服务异常和格式错误。（验证：`provider/mock.rs::MockScenario` 覆盖五类故障并支持确定性场景队列；错误覆盖测试通过）
- [x] 建立 provider capability 检查，确认是否支持图片输入、结构化输出和所需图片数量。（验证：`ProviderCapabilities::validate_request` 检查 operation、image_input、structured_output 与 max_image_count；缺能力/超图片数测试通过）
- [x] 为 provider 选择、重试、脱敏和错误映射补充测试。（验证：37 个工具测试覆盖注册/选择、重试上限/request ID、响应契约、超时/取消、确定性 12ms 限速、请求与 secret 脱敏和六类错误映射；主 agent 复跑 37/37）
- [x] 确认 provider SDK、在线请求适配和凭据读取代码只存在于工具 crate，`project` 正式依赖图不包含这些生成期能力。（验证：本阶段仅修改 `tools/ui-generation/` 与说明文档，未新增在线 SDK；`project/Cargo.toml`/lock 与 Android 无 diff，真实 `check-boundary` 五项均为 `true`）
- [x] 在工具 crate 运行格式检查、provider 测试和 `cargo check`；在 `project/` 运行 `cargo check` 并复核正式依赖图未变化。（验证：2026-07-14 15:15 +08:00 工具测试 37/37，工具 fmt/check、`project` check、`check-boundary` 和仓库 `git diff --check` 全部退出码 0）

## 阶段 3：参考图预处理和统一坐标系

- 开始时间：2026-07-14 15:17:26 +08:00
- 结束时间：2026-07-14 17:47:44 +08:00
- 开发总结：实现受限 PNG/JPEG 读取、EXIF/ICC/alpha 证据、八方向标准化、显式 crop/safe/system 区域、原图/EXIF/预览/logical/physical 五坐标系映射、确定性下采样与辅助图，以及 hash+版本+完整选项绑定的原子 cache/run；预处理 CLI 只写被忽略的 `summary/ui-generation/`。
- 验证记录：第 1/5 轮代码审核发现未显式 flush、crop 外 Raw/EXIF 不能全图往返、exclusion 数量无上限，返修后显式 write/flush/sync、cache 提交前复验、恢复全图 Raw/EXIF 语义并限制每图 64 个 exclusion；第 2 轮主 agent 独立运行工具测试 46/46、工具 fmt/check、`project` check、`check-boundary`、`git diff --check` 全部通过，`project` 依赖树不含 `zune-jpeg` 且正式依赖/Android 无 diff。

- [x] 读取参考图尺寸、EXIF 方向、透明通道和色彩信息，并输出标准化预览副本。（验证：`preprocess.rs` 受限解码 PNG/JPEG，记录 encoded/decoded color、alpha、EXIF 与 ICC 长度/hash，声明/嵌入方向冲突失败；输出显式 flush/sync 的确定性 RGBA8 PNG）
- [x] 定义参考图像素、目标逻辑像素和设备物理像素之间的转换关系。（验证：`CoordinateMapping` 覆盖 Raw、完整 EXIF-normalized、Preview、TargetLogical、DevicePhysical 五空间并记录比例/偏移/half-up 舍入；全图 Raw/EXIF 与 crop 内下游往返测试通过）
- [x] 支持明确的裁切区域、安全区和系统 UI 排除区域，不自动猜测并删除关键内容。（验证：严格 options JSON 只接受完整 EXIF 归一化像素边界坐标，区域必须在图内且 safe/system 必须位于 crop；每图 exclusion 上限 64，文档明确不推断内容）
- [x] 为超大图片建立下采样策略，同时保留原图尺寸和坐标映射用于最终审核。（验证：预览同时受 max edge 与 4,194,304 像素预算约束，manifest 保留 source raw size、方向、crop 和完整映射；8000x8000 策略计算覆盖通过）
- [x] 生成可选的结构辅助图，例如网格、区域编号和高对比预览，但不得替代原图输入。（验证：可选 `structure.png`/`high-contrast.png` 独立生成并标记 `auxiliary_only`，manifest 的首项固定为非辅助 `preview.png` 且 `original_remains_authoritative=true`）
- [x] 对损坏图片、空白图片、过小图片、异常长宽比和不支持格式返回明确失败。（验证：`TaskFailureKind` 增加格式、损坏、过小、危险尺寸、异常比例、空白、metadata 冲突和 cache 损坏稳定码；综合失败分类测试通过，纯色 detail 降级规则有回归）
- [x] 缓存预处理结果，cache key 包含图片 hash 和预处理版本。（验证：cache key 绑定 SHA-256、协议/实现版本、声明 metadata、reference ID、viewport、profile 与全部有效 options；staging 写后复验 manifest/hash 再 rename，cache 命中/选项变更/损坏测试通过）
- [x] 图片解码、EXIF 和预处理依赖只加入工具 crate，并验证不会改变 `project` 的 Android 依赖与构建产物。（验证：仅 `tools/ui-generation/Cargo.toml` 启用 `image` PNG/JPEG；`project` Cargo/Android 无 diff，`cargo tree -i zune-jpeg` 无正式依赖，`check-boundary` 五项为 `true`）
- [x] 为方向、缩放、裁切和坐标往返补充测试。（验证：9 个阶段 3 测试覆盖八方向、JPEG EXIF、五坐标系、crop 外 Raw/EXIF、下采样、区域、cache、错误分类、flush 失败和端到端运行目录；完整工具测试 46/46）
- [x] 在工具 crate 运行格式检查、预处理测试和 `cargo check`；在 `project/` 运行 `cargo check` 并复核 Android 正式依赖图未变化。（验证：2026-07-14 17:47 +08:00 阶段测试 9/9、工具全量 46/46、工具 fmt/check、`project` check、boundary 与 diff 检查全部退出码 0）

## 阶段 4：参考图结构化视觉分析

- 开始时间：2026-07-14 17:49:35 +08:00
- 结束时间：2026-07-15 10:00:32 +08:00
- 开发总结：建立独立于正式 `UiDocument` 的 `UiReferenceAnalysis` 不可信中间协议，覆盖参考证据、区域/元素层级、布局行为、视觉角色、文字与图片候选及显式不确定性；使用生成的 Draft 2020-12 Schema、输入结构预算、语义图校验和可信 provider/task/preprocess 上下文交叉验证，并加入常规页面、长列表、HUD、弹窗和异常响应离线 fixture。
- 验证记录：中断恢复后 worker 逐项复核 9/9 子项且无需返修；主 agent 独立运行分析协议测试 12/12、工具全量测试 58/58、工具 `cargo fmt --all -- --check`/`cargo check`、`project` `cargo check`、`check-boundary` 五项和仓库 `git diff --check`，全部退出码 0。

- [x] 定义独立于最终 UI Schema 的 `UiReferenceAnalysis`，记录区域、层级、候选组件、文字、图片和装饰元素。（验证：`tools/ui-generation/src/analysis.rs` 定义工具 crate 专属 `UiReferenceAnalysis`、`AnalysisRegion`、`AnalysisElement`、候选组件及文字/图片/装饰类型，未加入正式 runtime facade）
- [x] 为每个识别元素记录 bounding box、父子关系、对齐线索、重复模式、置信度和证据来源。（验证：`AnalysisElement` 覆盖所需字段，语义校验检查 ID 唯一、引用闭包、单根无环、坐标边界和重复序号；fixture/图错误测试通过）
- [x] 区分固定锚定、内容流、比例伸缩、可滚动和绝对装饰元素，避免把所有坐标直接固化为绝对定位。（验证：`LayoutBehaviorKind` 明确定义五类布局行为，`validate_layout` 校验对应 anchor/axis/元素类型；页面与列表 fixture 覆盖通过）
- [x] 识别背景、表面、边框、图标、状态标识和可能的九宫格区域。（验证：`VisualElementKind` 覆盖六类视觉角色，图片协议保留 `likely_nine_slice` 与证据；Schema/语义测试通过）
- [x] OCR 或模型识别的文字必须保留原始候选、置信度和人工提供文字，不静默覆盖用户文本。（验证：`TextRecognition` 保存候选、置信度、人工文字/输入 ID/采用策略；trusted context 逐字核对 `GenerationTask.visible_text`，人工文字权威与冲突 uncertainty 回归测试通过）
- [x] 对遮挡、模糊、裁切、无法识别字体和隐藏交互生成显式 uncertainty 列表。（验证：`UncertaintyKind` 覆盖 occlusion、blur、cropping、unknown_font、hidden_interaction 等并强制 subject/evidence 关联；modal fixture 覆盖所需类型）
- [x] 用 JSON Schema 校验分析结果，并限制元素数、层级深度和文本长度。（验证：`schemars` 生成 Draft 2020-12 Schema，`jsonschema` 执行运行时校验；2 MiB/结构预算、512 元素、24 层、1024 字节文字及 Schema 漂移/边界测试通过）
- [x] 为常规页面、长列表、HUD、弹窗和异常模型响应准备 fixture。（验证：`fixtures/analysis/` 包含四个合法场景及 unknown-field、over-budget、graph-invalid 三个异常 fixture，场景语义和稳定诊断测试通过）
- [x] 在工具 crate 运行格式检查、视觉分析协议测试、Schema 漂移测试和 `cargo check`；在 `project/` 运行 `cargo check`。（验证：2026-07-15 10:00 +08:00 分析测试 12/12、工具全量 58/58、工具 fmt/check、project check、boundary 和 diff 检查全部通过）

## 阶段 5：视觉 token 和布局规划

- 开始时间：2026-07-15 10:01:47 +08:00
- 结束时间：2026-07-15 11:13:55 +08:00
- 开发总结：实现确定性 `UiGenerationPlan`，从可信分析生成带来源分类的页面候选 token、重复组件映射、布局约束图和结构/视觉/装饰三阶段步骤；通过正式 `UiDocument` tooling facade 的只读 theme/variant catalog 复用真实现有能力，并稳定诊断矛盾布局、过度绝对定位和不可能尺寸。规划输出预算覆盖 Stage 4 最大合法输入，不静默截断，组件 ID 使用无损编码避免碰撞。
- 验证记录：第 1/5 轮审核发现虚假 widget variant、组件 ID 碰撞、输出静默截断、token 来源不明和 catalog 漂移风险，返修为共享正式 variant 支持矩阵、无损 ID、上游最大预算、`TokenOrigin` 和真实 theme 防漂移测试；第 2-3 轮在最大输入并发测试下复现 provider 非超时用例的调度型假超时，隔离为通用 10s 测试 policy 与 timeout 专项 20ms policy。最终主 agent 默认并行工具全量连续 5 次 66/66，planning 8/8、provider runner 7/7、project tooling 3/3，以及工具/project fmt/check、boundary、diff 检查全部通过。

- [x] 从分析结果中归并颜色、字号、间距、圆角、边框、阴影和重复尺寸，形成页面级候选 token。（验证：`tools/ui-generation/src/planning.rs` 的 `CandidateTokenKind` 覆盖七类 token，1px 容差聚类与稳定排序生成候选；`TokenOrigin` 明确区分几何观测、catalog 建议和启发式假设）
- [x] 将重复区域归并为组件实例，保留与参考图元素之间的映射。（验证：`derive_components` 按 `pattern_id` 归并并保留排序后的 `source_element_ids`；组件 ID 对 source key 做无损 UTF-8 十六进制编码，碰撞回归和 512 组件唯一性测试通过）
- [x] 生成布局约束图，记录父子尺寸、锚点、对齐、间距和可伸缩关系。（验证：`ConstraintKind`/`derive_constraints` 覆盖 parent、width、height、anchor、align、gap、flex、scroll，约束覆盖测试通过）
- [x] 按“结构优先、视觉其次、微小装饰最后”的顺序输出生成计划。（验证：`PlanPhase` 与 `derive_steps` 固定按 Structure、Visual、Decoration 生成并稳定排序，canonical 输出与 phase 顺序测试通过）
- [x] 检测互相矛盾的布局约束、过度绝对定位和不可能满足的最小尺寸。（验证：稳定诊断覆盖同轴矛盾对齐、固定宽度双边锚定、超过 35% 绝对装饰定位和子元素超出父尺寸，诊断代码/顺序测试通过）
- [x] 将项目现有 theme token 和 widget variant 纳入候选匹配，优先复用而不是创建近似重复项。（验证：`document::tooling` 暴露只读 token/variant catalog；theme 值逐项对照 `UiTheme::default()`，variant 与正式 semantic validator 共用 `component_variant_supported`，不支持的 list_item/label 不再误报全局复用）
- [x] 对新 token 和新组件建议标记作用域，避免一次参考图污染全局主题。（验证：`RecommendationScope` 区分 ExistingGlobal、Page、Component；仅真实匹配 catalog 的项可全局复用，未匹配 token 默认页面作用域、重复新组件为组件作用域）
- [x] 为 token 聚类、布局冲突和组件复用决策补充确定性测试。（验证：8 个 planning 测试覆盖确定性 JSON、聚类/来源、组件复用/映射/ID 碰撞、约束类型、稳定诊断、预算和超过旧截断上限的 512 元素输入）
- [x] 在工具 crate 运行格式检查、规划器确定性测试和 `cargo check`；在 `project/` 运行 `cargo check`。（验证：2026-07-15 11:13 +08:00 工具默认并行全量连续 5 次均 66/66，planning 8/8、provider runner 7/7、project tooling 3/3，两边 fmt/check、boundary 和 diff 检查全部通过）

## 阶段 6：素材分类、提取和替代策略

- 开始时间：2026-07-15 11:15:20 +08:00
- 结束时间：2026-07-15 13:51:55 +08:00
- 开发总结：实现独立 Stage 6 素材策略协议，为正式 UI 资源建立稳定 asset ID catalog，并确定性覆盖现有资源、程序化表现、授权裁切、重制、生成和占位六类处置；授权裁切绑定可信任务、Stage 3 cache/manifest/坐标证据，只向受控 run assets 原子 no-clobber 写入，生成规格/provenance 和 PNG/JPEG/Android 质量报告保持授权、色彩与人工审核边界。
- 验证记录：中断恢复后的 worker 首轮 14/14 测试虽通过，但审查发现 catalog/manifest 解析前缺少字节预算、目录枚举先收集后限额、超大图片仍进入解码以及 TrustedAssetSource 未完整复验 task/cache/坐标映射；返修后新增预算和伪造可信来源回归。主 agent 独立运行 Stage 6 测试 15/15、工具全量 81/81、工具 fmt/check、project fmt/check、project cargo check、check-boundary 五项和 git diff --check，全部通过；project/assets、正式 Cargo 依赖与 Android 无 diff。额外严格 Clippy 仅发现本阶段一处可省略 lifetime 和三处既有模块非验收警告，未扩大范围处理。

- [x] 将参考图元素分类为现有资源、可程序化表现、授权可裁切、需要重制、需要生成和临时占位。（验证：`tools/ui-generation/src/asset_strategy.rs` 的 `AssetDisposition`/`AssetDecision` 定义六类处置，`build_asset_strategy` 确定性生成策略和显式降级诊断；分类测试通过）
- [x] 对现有 `project/assets/ui/` 建立可检索 metadata，使用稳定 asset ID 匹配而不是让模型拼接路径。（验证：`tools/ui-generation/assets/ui_asset_catalog.v1.json` 覆盖 atlas/icons/images/fonts 的 22 个正式资源及 hash/尺寸/alpha/license/tags；catalog 递归复验全覆盖、重复/大小写碰撞、路径/符号链接和 metadata，搜索与策略只返回/接收稳定 asset ID）
- [x] 仅在授权允许时提取参考图局部素材，并记录原图 hash、裁切区域和处理步骤。（验证：`TrustedAssetSource` 重算 task/Stage 3 cache key/坐标映射并复验 manifest；仅 `derivatives_allowed` 且有许可记录可建立 `CropSourceRecord`，记录 source/preview/manifest hash、preview/EXIF crop 和处理步骤；授权拒绝与伪造来源测试通过）
- [x] 对需要重制或生成的素材输出规格，包括尺寸、透明背景、切片边距、色彩和用途。（验证：`AssetSpecification` 记录宽高、alpha、nine-slice insets、sRGB 和 usage，并校验 4096 单边、16777216 像素、中心区与用途兼容；重制/生成测试通过）
- [x] 生成素材必须记录生成工具、提示词摘要、版本、授权信息和人工批准状态。（验证：`GenerationProvenance`/`GenerationPromptSummary` 使用受控 tag 记录 tool ID/version、许可与审批，新草稿强制 `pending_human_review` 且不能自批；provenance 测试通过）
- [x] 所有草稿素材写入 `summary/ui-generation/<run-id>/assets/`，不直接覆盖 `project/assets/`、正式资源或现有 Git LFS 文件。（验证：`extract_authorized_crop` 复验 run/symlink/direct-child 边界，以 flush/sync、hard-link create-if-absent 原子 no-clobber 提交，并对正式 assets 做写前写后 hash 快照；冲突/竞争测试和 project/assets 零 diff 复核通过）
- [x] 检查透明边缘、压缩伪影、颜色空间、尺寸上限和 Android 纹理兼容性。（验证：`inspect_asset_bytes` 在解码前限制 16 MiB、4096 单边和 16777216 像素，受限 decode allocation，并报告 8-bit color、alpha/透明边缘/透明 RGB、APNG、ICC/sRGB 与 JPEG 有损审核；质量测试通过）
- [x] 为资源匹配、授权拒绝、裁切映射和占位降级补充测试。（验证：`asset_strategy::tests` 15 项覆盖 catalog 匹配/全覆盖/逃逸/碰撞/符号链接、授权 fail-closed、可信 manifest/cache/坐标伪造、裁切映射/no-clobber、六类策略/占位和质量预算；主 agent 复跑 15/15）
- [x] 在工具 crate 运行资源清单/授权/裁切测试、格式检查和 `cargo check`；复核 `project/assets/` 未被草稿流程修改，并运行 `project` 的 `cargo check` 与仓库 `git diff --check`。（验证：2026-07-15 13:51 +08:00 工具 Stage 6 15/15、全量 81/81、fmt/check，project fmt/check、boundary 五项和 diff 检查全部退出码 0；project/assets、project Cargo/lock 与 Android 无 diff）

## 阶段 7：结构化 UiDocument 生成

- 开始时间：2026-07-15 13:53:25 +08:00
- 结束时间：2026-07-15 14:47:21 +08:00
- 开发总结：实现 Stage 7 严格结构化 `UiDocument` 生成 API，将可信分析、确定性规划和素材策略组成 provider 请求，并通过正式 tooling facade 完成 canonical/Schema/语义/能力/预算验证；生成结果提供节点精确覆盖的 source map、literal-only 文本决策、stable asset allowlist、结构化风险披露和不含敏感 payload 的可复现 trace，阶段 7 明确禁止提前猜测 action/binding/i18n/state/responsive。
- 验证记录：第 1/5 轮主审核发现 provider 与本地派生 disclosure 合并超过 512 时按优先级静默 truncate，可能丢失隐藏交互、假设和不支持证据；返修删除截断，以 Stage 4/5/6 公开最大预算推导 7168 合并上限，未来漂移超限稳定失败，并增加超过旧上限仍完整保留六类尾项的回归。最终主 agent 独立运行生成相关过滤 12/12（Stage 7 自身 10 项）、工具全量 91/91、project `cargo test ui_document_ --lib` 98/98、两边 fmt/check、boundary 五项和 git diff --check，全部通过；正式 assets、project Cargo/lock 与 Android 无 diff。

- [x] 使用 `UiReferenceAnalysis`、布局计划、token 和素材表生成符合指定版本的 `UiDocument` JSON。（验证：`tools/ui-generation/src/generation.rs::prepare_generation_request` 交叉验证 analysis/确定性 plan/Stage 6 strategy，将完整结构化输入和目标 schema 版本交给 provider；合法 fixture 经 `validate_generation_execution` 产出 canonical document）
- [x] 通过 provider 的 structured output 或等价强约束机制生成，不从 Markdown 代码块截取 JSON。（验证：固定 `StructuredOutputContract` 和 deny-unknown-fields envelope 约束输出，响应先过字节/深度/节点/容器/字符串预算；Markdown fence 字符串测试稳定返回 malformed）
- [x] 将可见元素映射到稳定 node ID，并保留 reference element ID 到 node ID 的 source map。（验证：`derive_source_map` 对全部 analysis elements 确定性映射，正式 canonical 文档必须精确匹配节点集合与父子层级，并回填 document path；确定性/无碰撞与复杂层级测试通过）
- [x] 优先使用已注册组件和样式变体；只有协议明确支持时才使用 inline override。（验证：请求携带 Stage 5 正式 catalog 匹配的 component/variant，正式 facade 再校验协议字段和 variant；已注册 badge/default 正向测试通过，未注册组件进入 required_new_components）
- [x] 对文本使用 literal 或 i18n key 的策略作显式选择，不生成不存在的业务绑定和动作。（验证：`TextSourceStrategy` 显式区分 Literal/Unresolved；canonical 节点 literal 必须逐项匹配 analysis adopted text，递归拒绝 action/on_click/binding/i18n_key，且禁止非空 Stage 9 states/responsive；非法行为测试通过）
- [x] 输出生成假设、未实现状态、所需新组件和不受支持能力列表。（验证：`GenerationDisclosures` 分四类合并 provider 与可信本地证据，排序去重后不截断；7168 上限由 512 provider、analysis uncertainties/elements、4096 tokens、512 components/assets 推导，超旧 512 回归证明 provider 尾项及 heuristic/uncertainty/hidden/component/asset 全保留）
- [x] 记录 model、prompt version、schema version、输入 hash、生成参数和响应 request ID。（验证：`GenerationTrace` 记录受安全 label 约束的 provider/model/prompt/output schema/document schema、组合输入 SHA-256、参数、最后成功 server request ID 和 canonical document hash；trace/预算/伪造 request ID 测试通过）
- [x] 生成器通过游戏工程公开的最小 facade 复用 `UiDocument` canonical JSON、Schema/语义验证和预算规则，不复制协议实现，也不让游戏工程依赖生成器。（验证：generation 仅调用 `project::framework::ui::document::tooling::{validate_json_bytes,canonicalize_json,...}`；正式 facade 无改动，check-boundary 五项 true，project Cargo/lock 无 diff）
- [x] 为最小页面、复杂页面、非法输出和不支持能力补充 fixture 测试。（验证：`tools/ui-generation/fixtures/generation/` 四个仓库自有文本 fixture 覆盖最小、嵌套复杂、正式协议非法和不支持能力；Stage 7 10 项测试另覆盖 Markdown、source map/hash、registered variant、stable asset path、trace/预算和披露完整性）
- [x] 在工具 crate 运行格式检查、生成器/fixture/source map 测试和 `cargo check`；在 `project/` 运行 `ui_document_` focused tests、格式检查和 `cargo check`。（验证：2026-07-15 14:47 +08:00 工具 Stage 7 10/10、全量 91/91、fmt/check，project `ui_document_` 98/98、fmt/check，boundary 与 diff 检查全部退出码 0）

## 阶段 8：校验、有限修复和确定性预览

- 开始时间：2026-07-15 14:48:58 +08:00
- 结束时间：2026-07-15 17:40:12 +08:00
- 开发总结：实现 Stage 8 受控修复、可信 run bundle 和独立确定性预览：0-3 轮严格 structured repair 复用正式 validation report 与 Stage 7 冻结策略，按稳定失败类型终止无进展/重复诊断/轮次或 provider 故障；同一 Stage 3 run 内以真实 artifact hash/身份链和事务 marker 归档全流程证据；feature-gated desktop preview bin 复用正式 runtime，等待资源/稳定帧后截图，工具对 strict result、实际 PNG 解码/尺寸/hash 和进程状态双重验证，默认桌面/Android 不包含该入口。
- 验证记录：第 1/5 轮主审核发现 manifest 接受不存在的 caller-supplied artifact link 且无法复用已存在 Stage 3 run，返修为同 run 实际文件稳定读取、hash/length/reparse/身份交叉验证与 `.bundle-partial -> bundle -> COMMITTED`；第 2/5 轮发现 PNG 仅验签名、非法截断图片仍可 Passed/COMMITTED，返修为有界完整解码、真实尺寸/色型/APNG/像素门禁并在复制前后共用验证；第 3/5 轮主 agent 独立复现 repair 10ms 调度型假超时，分离普通 10s 与 timeout 专项 20ms policy 后 worker focused 连续 5 次、全量连续 3 次全绿。最终主 agent 独立运行 repair 6/6、preview 8/8、manifest 9/9、工具全量 115/115、project `ui_document_` 98/98、standalone feature 3/3、两边 fmt/check、boundary 七项及真实 390x844 预览；截图 18588 bytes、SHA-256 `0042bc4f62e75d10e0a331f7c3eedf32f7a0d52e7038be56fcde6700f9259174`，34 帧完成/30 帧稳定，人工检查非空白且文字无重叠，临时产物和进程已清理。

- [x] 对生成文档依次运行 JSON Schema、语义、能力、动作白名单和资源预算校验。（验证：`generation::validate_staging_document` 复用正式 `tooling::validate_json_bytes` 的 syntax/structure/reference/capability/budget report，再应用 Stage 7 冻结 action/binding/i18n/source-map/asset policy；非法 policy/预算测试通过）
- [x] 将 validation report 按 document path 和 node ID 回传给生成器，最多执行可配置次数的结构修复。（验证：`tools/ui-generation/src/repair.rs` 将正式及 generation policy diagnostics 转为有界 `RepairDiagnostic`，保留 document_path/field_path/node_id/fingerprint；`RepairConfiguration` 限制 0..=3 轮严格 structured `{document}` repair）
- [x] 修复只能修改 staging 文档，不得扩展安全白名单、降低预算或删除失败证据。（验证：每轮请求携带只读 `GenerationRepairPolicySnapshot`，输出只能替换 staging `Value` 并重新经过同一正式/冻结策略；`RepairRoundEvidence` 保留 input/output hash、完整 validation report、请求与 provider trace，policy/budget 放宽测试稳定拒绝）
- [x] 连续出现相同错误、达到最大次数或模型不可用时停止，并输出稳定失败类型。（验证：`RepairFailureKind`/稳定 code 区分 no progress、repeated diagnostics、maximum rounds、unavailable、timeout、cancel、malformed、over-budget 与 final policy；focused 6/6 且普通/timeout 测试 policy 分离后连续复跑稳定）
- [x] 校验通过后由独立工具进程调用公开的声明式预览能力，等待字体和图片资源就绪后截图；不得为此把生成器或 provider 注册进正式 `UiFrameworkPlugin`。（验证：project 的 `ui-document-preview` bin 仅由非默认 `ui-document-preview-tool` feature 启用，内部复用正式 `UiFrameworkPlugin`/runtime/reload/`UiScreenshotCommand`，等待资源 loaded 和 30 稳定帧；Cargo metadata required-features 正确，正式插件无 provider/generator 注册）
- [x] 预览 manifest 关联输入图、分析结果、文档、资源、截图、日志和所有修复轮次。（验证：`run_manifest.rs` 在既有同 run 根实际读取并复验 Stage 3/4/6/7 links、参考/analysis/strategy/document/trace 身份链，归档 repair initial/round/final/source map/validation/node summary 与 strict preview result/解码 PNG/log；事务仅最后创建 `COMMITTED`，伪 hash/跨 run/reparse/冲突/非法 PNG 均无 marker）
- [x] 相同 fixture 和 provider 响应应生成相同 canonical JSON 与节点树摘要。（验证：一次成功修复测试重复运行得到相同 canonical document，`node_tree_summary` 按 document path 排序并对摘要文本计算稳定 SHA-256；确定性测试通过）
- [x] 为一次修复成功、重复失败、超预算和资源缺失补充端到端测试。（验证：repair 6 项覆盖成功、unchanged/repeated、max rounds、provider unavailable/timeout/cancel、policy/budget；preview 8 项覆盖缺资源/截图/timeout、截断/错尺寸/APNG/16-bit 和 protected output；manifest 9 项覆盖真实同 run、伪造、跨 run、reparse、事务冲突和读取期变化）
- [x] 在工具 crate 运行格式检查、修复端到端测试和 `cargo check`；在 `project/` 运行 validator/runtime focused tests、格式检查、`cargo check` 和单页面预览截图。（验证：2026-07-15 17:40 +08:00 工具全量 115/115、fmt/check，project `ui_document_` 98/98、standalone feature 3/3、fmt/check，boundary 七项和 diff 检查通过；主 agent 真实 preview exit 0、390x844、34/30 帧、PNG 解码/hash/人工画面检查通过）

## 阶段 9：多状态、响应式和交互补全

- 开始时间：2026-07-15 17:42:38 +08:00
- 结束时间：2026-07-16 13:47:46 +08:00
- 开发总结：建立受信任的同页多参考图 evidence matrix，使共享节点、可见 state 与响应式 override 只能由对应参考证据授权；单图生成继续 fail-closed。新增 page state preview reload、可见状态/手机平板 fixture、正式 `UiNode::Modal` runtime 阻断/owner 清理测试和 14 格 audit runner。audit 对明确缺失 result/log/screenshot evidence 最多重试两次，每次保留独立 attempt 路径和 manifest 记录，不重试语义、进程或取消失败。
- 验证记录：主 agent 独立通过工具 `cargo test --manifest-path tools/ui-generation/Cargo.toml` 127/127、工具 fmt/check、`check-boundary` 七项 true；`project/` 的 `cargo test ui_document_ --lib` 100/100、feature-gated `cargo test --features ui-document-preview-tool standalone_preview --lib` 3/3、fmt/check 均通过。真实 `audit-document --require-distinct-from-initial` 在 390x844 和 1280x800 运行 14 个 screen/device/state capture，manifest `status=passed`、14 个 selected attempt、0 visual expectation failure，PNG/metadata/hash/dimensions 均复核；曾复现 Vulkan swap-chain 瞬态缺失 result evidence，并以受限重试修复后复验通过。

- [x] 多张参考图属于同一页面时，建立共享节点和状态差异映射，不为每张图复制独立页面。（验证：`tools/ui-generation/src/series.rs::validate_page_series` 要求 additional reference 到 primary node 的共享 source-map，且 `document.states` 精确匹配 visible evidence；多参考图/缺少 shared evidence 回归测试通过）
- [x] 从不同尺寸参考图推导响应式 override；只有单一尺寸时使用项目默认断点并记录推导假设。（验证：`ResponsiveDerivation::Observed` 强制 primary+第二个不同 viewport，`ProjectDefault` 仅允许 primary 并写入 `SERIES_RESPONSIVE_PROJECT_DEFAULT_ASSUMPTION` disclosure；对应 3 个 series tests 通过）
- [x] 为 loading、empty、error、selected、disabled、modal 等可见状态生成独立可审核 state。（验证：`fixtures/audit/phone_tablet_multi_state.valid.json` 为每个状态使用 display override，modal 为正式 `UiNode::Modal`；主 agent 14 格 audit 的六个非 initial state 在每种设备均与 initial hash 不同）
- [x] 对参考图未展示的交互只绑定项目允许的默认行为，未知业务动作保持未绑定并报告。（验证：`series.rs` 为 interactive node 输出 `SERIES_ACTION_UNBOUND`，Stage 7 默认 policy 仍拒绝无 evidence 的 actions/bindings；series/generation focused tests 通过）
- [x] 自动补充键盘焦点、触控目标和 accessible label，不改变参考图主要视觉结构。（验证：`build_accessibility_supplements` 输出 label source、稳定 focus order、44px explicit/runtime touch policy；缺 formal label 的交互节点 fail-closed，series tests 覆盖补充结果）
- [x] 验证状态切换、滚动位置、Modal 输入阻断和页面 owner 清理。（验证：`UiDocumentPreviewCommand::SetPageState` 走事务 reload 并保留 `ScrollPosition`；`ui_document_modal_panel_blocks_input_and_owner_cleanup_removes_the_modal_root` 断言 modal pointer block、root despawn 和 owner 清理，project `ui_document_` 100/100）
- [x] 为手机竖屏、平板横屏和同一页面多状态建立 fixture。（验证：`tools/ui-generation/fixtures/audit/phone_tablet_multi_state.valid.json` 定义 compact portrait/expanded landscape 与 initial、loading、empty、error、selected、disabled、modal）
- [x] 使用 UI audit runner 生成每个目标 screen、device、state 的截图和 metadata。（验证：`audit-document` 输出 attempt-level manifest；主 agent 复核 14/14 selected `attempt-01`，390x844/1280x800 PNG、result metadata、hash 和 log 均存在，visual failure 为 0；缺失渲染 evidence 最多两次且可追溯）
- [x] 在工具 crate 运行多状态/响应式生成测试、格式检查和 `cargo check`；在 `project/` 运行响应式/runtime/audit focused tests、格式检查和 `cargo check`。（验证：工具全量 127/127、fmt/check；project `ui_document_` 100/100、standalone preview 3/3、fmt/check；`check-boundary` 七项 true、`git diff --check` 通过）

## 阶段 10：人工决策点和受控草稿晋升

- 开始时间：2026-07-16 13:51:14 +08:00
- 结束时间：2026-07-16 16:42:09 +08:00
- 开发总结：实现绑定 sealed run/document/input hash 的人工决策记录、no-write promotion plan 与重复 plan hash 的显式 `promote`。晋升只原子写入新的 approved 页面包（文档、封闭 registration 审阅声明、授权资源、catalog fragment、许可证），拒绝业务 action/binding/i18n 和现有 owner/目标冲突。项目侧新增只读 approval adapter，以显式生命周期将已批准 source 转为既有 runtime/preview registration，不执行游戏 route。
- 验证记录：工具 promotion focused 6/6、全量 134/134、fmt/check、boundary 七项 true；project approval adapter focused 4/4、`ui_document_` 100/100、feature-gated standalone preview 3/3、fmt/check 通过；主 agent 重跑 promotion 6/6、approval 4/4 和 `git diff --check` 通过。

- [x] 定义必须人工确认的情况，包括素材授权未知、核心布局低置信度、业务动作缺失和框架能力不足。（验证：`PromotionDecisionKind` 与 `derive_questions` 生成 asset/license、core layout、business action、framework capability 和 release approval 问题）
- [x] 将问题限制为少量高影响决策，并附参考图区域、候选方案和各自影响。（验证：`PromotionQuestion` 受 `MAX_DECISIONS=16` 限制，包含 `reference_region`、candidate summary/impact）
- [x] 提供接受、拒绝、替换素材、修改文字、修改约束和保持占位等结构化决定。（验证：`PromotionResolution` 封闭枚举覆盖六种决定，严格 submission/record 校验测试通过）
- [x] 决定结果写入 run manifest，并在重新生成时复用，避免重复询问。（验证：append-only `approval/promotion-decisions.v1.json` 和 marker 绑定 run/manifest/document/input hash；第二次记录、篡改 bundle/submission 均拒绝）
- [x] 设计草稿晋升清单，明确哪些 JSON、资源、i18n key、主题 token 和页面注册会进入正式目录。（验证：`PromotionPlan` 列出 document、registration、resources；封闭模板的 i18n/theme/action/binding 列表强制为空）
- [x] 晋升前检查目标路径冲突、Git LFS、资源许可证、schema version 和现有页面所有权。（验证：promotion tests 覆盖冲突/owner；资源校验绑定 Stage 6 license/spec/Android quality、`.gitattributes` LFS，project facade/schema 与 approved catalog fragment 均复验）
- [x] 实现显式 `promote` 命令：只接受通过全部校验且带人工批准决定的 run，把 `UiDocument` 写入 `project/assets/ui/documents/approved/`，把授权资源写入正式 assets，并生成可审阅的 i18n/theme/page registration 变更。（验证：CLI `promote` 要求精确 `--confirm-plan` hash，仅写 `approved/<document-id>/` 页面包和 `promotion.v1.json` 审阅声明；promotion 6/6）
- [x] 页面主体保持为声明式 JSON；只允许从封闭模板生成确定性的 owner/route/registration 适配，未知业务 action 或 binding 必须阻塞晋升，不允许模型生成任意 Rust 业务实现。（验证：project `approval.rs` 只读解析并拒绝 business fields，route 是 review-only label；approval focused 4/4）
- [x] 晋升命令在写入前生成完整变更计划，要求显式确认，使用临时目录和原子替换，失败时不留下部分正式文件，也不覆盖未声明的现有页面。（验证：no-write `promotion-plan` hash 必须在 promote 重复提交；同卷 staging page directory 单次 rename，冲突/原子失败测试无残留）
- [x] 生成工具默认没有写入 `project/src/`、`project/assets/` 和 approved 文档目录的权限；只有独立 `promote` 子命令在校验、批准和目标所有权检查全部通过后才能写入。（验证：仅 CLI `promote` 调用 `promotion::promote`；inspect/preprocess/generation/repair/preview/audit 未引入 promotion write API）
- [x] 晋升后使用正式游戏构建加载已批准页面，验证路由、资源、action/binding 注册、owner 清理和 audit metadata，确认生成结果会随桌面与 Android 游戏包交付。（验证：approved registration adapter 显式转为 runtime registration，focused lifecycle 测试验证 packaged resource override、active instance、audit recipe、unregister/owner cleanup；business registration 为空，默认 project/Android `--lib` 不依赖工具）
- [x] 为批准、部分批准、拒绝和目标冲突补充测试或 fixture。（验证：promotion 6/6 覆盖批准、拒绝、sealed hash 篡改、owner/target 冲突、资源/LFS/catalog 和冲突后无残留）
- [x] 在工具 crate 运行晋升 dry-run/批准/拒绝/冲突/原子失败测试、格式检查和 `cargo check`；在 `project/` 运行生成页面 focused tests、格式检查、`cargo check` 和 `git diff --check`。（验证：工具全量 134/134、promotion 6/6、fmt/check/boundary；project approval 4/4、`ui_document_` 100/100、standalone preview 3/3、fmt/check；diff check 通过）

## 阶段 11：评测集、成本、可观测性和文档验收

- 开始时间：2026-07-16 16:44:27 +08:00
- 结束时间：2026-07-16 17:51:36 +08:00
- 开发总结：新增只含仓库自有 CC0 合成文本 fixture 的六类离线评测集、Fixture provider 评测 runner、任务调用/耗时/图片/单位/成本硬预算、分析缓存精确 identity/显式失效及结构化报告递归脱敏。补齐工具/正式 project 依赖方向、Android default `--lib` 和真实多状态 audit 的验收记录与当前限制文档。
- 验证记录：主 agent 独立运行 `evaluate-fixtures` 6/6，通过率/首次校验率均为 100%、6 calls、72/48 units、168 microcost；observability 4/4、evaluation 3/3 通过。worker 完成工具全量 142/142、tool fmt/check/boundary、project fmt/check/lib、`ui_document_` 100/100、feature preview 3/3、Android arm64-v8a release `--lib` 和真实 14 格 audit；唯一 Android warning 为既有 `WindowResolution` 未使用 import，未修改。

- [x] 建立来源明确的小型参考图评测集，覆盖登录、列表、HUD、弹窗、复杂美术面板和多尺寸状态。（验证：`fixtures/evaluation/catalog.v1.json` 声明 CC0 repository-authored synthetic text source，六个 case 覆盖全部类别）
- [x] 为每个样例记录期望组件、关键区域、允许差异、不支持能力和人工验收结果。（验证：严格 `EvaluationCase` 包含 components/regions/differences/capabilities 与 reviewer-role acceptance）
- [x] 统计成功率、首次校验通过率、修复次数、耗时、模型调用次数、输入输出量和估算成本。（验证：主 agent `evaluate-fixtures` 输出 6/6、100%、0 repair、181ms、6 calls、72/48 units、168 microcost）
- [x] 按输入 hash、模型、prompt、schema 和参数缓存可复用分析结果，并支持显式失效。（验证：`AnalysisCacheIdentity` 与 `AnalysisCache` 精确绑定并仅在 ignored `summary/ui-generation/.cache/analysis` 落盘；focused test 覆盖变化/失效）
- [x] 日志和报告对凭据、个人信息、账号文字和原始模型敏感内容进行脱敏。（验证：`redact_report_value` 递归遮蔽 secret/account/player/email/prompt/raw response，observability test 通过）
- [x] 设置单任务调用次数、耗时、图片数量和成本上限，超过时停止而不是无限重试。（验证：`TaskBudget` 在 runner/repair 共享，calls/images/elapsed/input/output/cost 产生稳定 hard-stop code；focused test 通过）
- [x] 更新生成流程、输入格式、目录、失败类型、素材授权、安全边界和当前限制文档。（验证：更新 `UI参考图生成与正式包边界.md`、`UI当前限制.md` 与 fixtures README）
- [x] 使用 Fixture provider 在 CI 或本地完成全流程；在线 provider 验收单独记录且不作为普通构建前提。（验证：`evaluate-fixtures` 固定 repository catalog 和 `FixtureProvider`，report `offline_fixture_provider=true`、`online_provider_required=false`）
- [x] 使用 `cargo metadata`/`cargo tree` 和正式桌面、Android `--lib` 构建记录检查依赖方向，证明工具 crate 及其专属依赖未进入正式游戏包。（验证：boundary 七项 true、metadata/tree 排除工具；Android arm64-v8a release `--lib` 临时产物成功）
- [x] 分别运行工具 crate 的测试/格式/检查与 `project` 的正式构建检查，禁止用 `--all-features` 或隐式 workspace 配置把工具带入游戏产物。（验证：独立 manifest 命令、tool 142/142 fmt/check、project fmt/check --lib，未使用 `--all-features`）
- [x] 运行工具 crate 的全流程测试、格式检查和 `cargo check`，运行 `project` 的相关 focused tests、`cargo fmt --all -- --check`、`cargo check`、`git diff --check` 和至少一个真实预览审计。（验证：project UI document 100/100、preview 3/3；真实 audit 14/14、2 devices×7 states、visual failure 0；diff check 通过）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有阶段完成后统一验收。

- 开始时间：2026-07-18 13:57:58 +08:00
- 结束时间：2026-07-18 15:53:00 +08:00
- 验收总结：完成仓库自有参考图与真实 viewport/hash 输入、离线 FixtureProvider 分析和结构化生成、正式 Schema/语义/预算验证、有限修复、Bevy 预览、sealed bundle、人工决策、显式晋升、正常游戏路由和桌面/Android 包交付闭环；最终成功 run 为 `acceptance-03-final-20260718-04`，bundle manifest SHA-256 为 `7c818858a97bdda5ec2dac737f83094e875891a578b345d3146cb7e2d03a301b`，生成输入 SHA-256 为 `af2d4d26da3e6fed646ff802e9ea8bb8f4336faf4b3d92c77f73cd754db0bdc5`，promotion plan SHA-256 为 `4ddfd966b11b504c71f739dec79abdd2908cae5b90c16394bf6a4c57e6b73d24`，批准文档 SHA-256 为 `55e18f9e4d9c320b9a3d70b8c420ef5cc523375a7303477f942f7da2f1639762`。最终复验通过工具测试 144/144、游戏 `ui_document_` 100/100、feature-gated preview 3/3、批准路由 2/2、两种手机尺寸真实 route audit 2/2、工具与游戏 fmt/check、七项 dependency boundary、Android arm64-v8a release `--lib`、Debug APK 组装和 1036 条 APK 资源清单检查；工具拥有的 analysis-only 参考图位于 `tools/ui-generation/fixtures/acceptance/reference.png`、命中 Git LFS 且不属于 Android source set，APK 只包含批准页面 JSON 与正式动态库，不包含生成工具、provider、prompt、工具参考图、模型响应、日志或 staging run。文档已统一说明离线能力和显式晋升已实现，在线 provider/OCR/图片生成及任意用户参考图的真实模型分析仍未实现。

- [x] 输入一张合法参考图和目标 viewport 后，可以生成通过 Schema 与语义验证的 `UiDocument` 草稿。
- [x] 生成文档可以在 Bevy 中加载并产出可追溯预览截图，无需 AI 直接编写 Rust 页面布局。
- [x] 参考元素、生成节点、草稿资源和预览截图之间有稳定 source map。
- [x] 隐藏交互、低置信度布局和素材授权问题不会被静默猜测或自动晋升。
- [x] 非法、超预算或越权模型输出无法进入运行时或正式项目目录。
- [x] Fixture provider 能离线覆盖成功、格式错误、超时、限流、修复失败和取消路径。
- [x] 至少一个多状态或多尺寸样例生成正确的共享结构与响应式变体。
- [x] 每次运行都记录输入、模型、prompt、schema、参数、产物、成本和失败原因，且敏感信息已脱敏。
- [x] 至少一个批准样例已通过显式晋升进入正式游戏目录，可由正常游戏路由加载并随桌面与 Android 包构建；未批准草稿和工具代码不进入包。
- [x] 正式桌面和 Android 包的依赖图、构建目标及资源清单均不包含 AI 生成器、provider、prompt、参考图、模型响应、日志或 staging 产物。
- [x] `cargo fmt`、相关测试、`cargo check` 和生成预览 audit 全部通过。
- [x] 文档明确说明当前生成能力、素材政策、人工决策点和不支持场景。
