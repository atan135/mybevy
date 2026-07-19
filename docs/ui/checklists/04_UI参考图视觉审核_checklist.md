# 04. UI 参考图视觉审核 Checklist

## 目标

在现有 UI 截图与审计 Runner 基础上，建立参考图驱动的视觉审核能力。系统应能把参考图、目标页面截图和 UI metadata 对齐，生成可解释的差异图片、数值指标、语义问题和 AI 审核结论，并按明确规则决定通过、失败或需要人工确认。

确定性布局检查和图像指标是主要证据，AI 用于解释复杂视觉差异和给出修复方向，不能单独覆盖硬性失败。该清单不重复实现已经完成的截图 API、页面 recipe、设备矩阵和修复循环骨架。

## 已有基础与依赖

- 已有 `UiScreenshotCommand`、本地单页面审计、滚动 state、设备矩阵、metadata、manifest 和 `report.md`。
- 已有 AI analysis input/output、severity、gating 和 fixture 解析，但真实外部 AI 分析尚未接入。
- 依赖声明式 UI 协议提供 document/node source map 时，可以把问题定位到精确节点；传统 Rust 页面仍应支持文件级定位。
- 与 `03_AI参考图生成UI_checklist.md` 共享 reference element ID，但审核器必须保持独立证据和独立通过规则。

## 基础原则

- [x] 参考图和实现图必须按明确 viewport、裁切和状态映射比较，不允许任意拉伸图片制造虚假相似。（验证：阶段 2 deterministic viewport、阶段 4 strict crop/scale 1.0 和阶段 11 manifest matrix 合并验收）
- [x] 确定性硬失败优先于 AI 主观评分，包括尺寸错误、关键裁切、文字重叠和关键控件不可达。（验证：阶段 7 semantic hard failure 与阶段 9 merge priority/four-state gate；工具回归测试通过）
- [x] 动态区域、允许差异和阈值必须显式记录，不能由审核器临时忽略问题区域。（验证：阶段 6 mask/coverage/reference binding 和阶段 11 report bundle 显式输出 masks、notes、thresholds）
- [x] 所有得分必须附原始输入、算法版本、参数和可视化证据，保证可以复现。（验证：`ui_comparison_bundle_v1`/`ComparisonResult` hash-bound links、四图、算法/metrics/threshold 与 `logs/comparison/`）
- [x] 基准图更新需要人工批准，不允许自动修复流程顺便修改基准以获得通过。（验证：`baseline.rs` plan/apply/verify 的 approval SHA 绑定、receipt 事务及 runner baseline command boundary SelfTest）
- [x] 在线 AI 不可用时，确定性审核和 Fixture 分析仍可在 CI 中运行。（验证：`.github/workflows/ui-visual-audit.yml` 默认 strict Fixture self-test，不提供在线凭据；`-OnlineAi` 仅显式受控入口）

## 阶段 1：参考基准清单和存储策略

- 开始时间：2026-07-18 16:08:50 +08:00
- 结束时间：2026-07-18 16:30:10 +08:00
- 开发总结：建立独立 `tools/ui-visual-audit/` Rust 工具工程和严格 reference manifest v1，完成可提交/私有基准分区、完整 viewport/图片/来源/授权/版本/允许差异契约、canonical 路径与图片解码门禁、复合唯一键和禁止无记录覆盖的基准更新验证；工具不依赖 `project`，正式基准只进入工具 fixture/LFS 路径。
- 验证记录：worker 首轮 14 项测试后按主审核返修 decoded/original/physical 尺寸绑定、`source_uri` 和 snake_case 错误协议；主 agent 独立运行工具测试 16/16、fmt/check、CLI `--help`、`git diff --check`、尾随空白扫描、Git ignore/LFS 属性、Cargo metadata 和 Android source set 检查均通过。Windows 当前无符号链接创建权限，symlink escape 测试在可创建链接的平台执行；canonical containment 逻辑和普通路径回归已通过。

- [x] 定义 reference manifest，映射 reference ID、screen、device、state、viewport、图片和允许差异配置。（验证：`tools/ui-visual-audit/src/reference_manifest.rs` 的 `ReferenceManifest`/`ReferenceEntry`/`ReferenceKey`/`Viewport`/`ReferenceImage`/`AllowedDifferences` 使用严格 `deny_unknown_fields` 契约，CLI `validate-manifest` 可解析并完整验证）
- [x] 记录参考图原始尺寸、逻辑尺寸、物理尺寸、device scale、方向、色彩空间、hash、来源和授权状态。（验证：`Viewport`、`ImageMetadata`、`ReferenceProvenance` 覆盖全部字段，decoded/original/physical 尺寸和 orientation/device scale 交叉校验测试通过）
- [x] 区分本地临时参考图与可提交基准图：临时输入进入 `summary/`，正式测试基准进入明确的测试目录。（验证：`ReferenceStorage::{TemporaryLocal,CommittedFixture}` 只映射 `summary/ui-visual-audit/` 与 `tools/ui-visual-audit/fixtures/references/` 两个 canonical 允许根，私有路径命中 `.gitignore`）
- [x] 正式 PNG/JPEG 基准图遵守 Git LFS，且不进入 Android 首包 runtime assets。（验证：`.gitattributes` 对工具 references 的 png/jpg/jpeg 均返回 `filter=lfs`；`android/app/build.gradle` 只打包 `project/assets`，Cargo metadata 确认 `project` 不含 `ui-visual-audit`）
- [x] 定义同一页面多个参考尺寸、状态、语言和主题的唯一键及冲突处理。（验证：`CompositeKey` 绑定 screen/device/state/locale/theme 和完整 logical/physical/scale/orientation viewport，重复 ID 与重复 key 使用独立错误码；相关测试通过）
- [x] 对缺图、重复键、hash 不一致、尺寸不匹配和不允许的外部路径返回稳定错误。（验证：snake_case `ErrorCode` 覆盖 missing/not-file/hash/dimensions/viewport/duplicate/unsafe/outside-root/corrupt/too-large，canonical containment 和分类测试通过）
- [x] 定义基准版本和更新原因字段，禁止无记录覆盖。（验证：`BaselineRevision` 和 `validate_baseline_update` 强制连续版本、非空原因、前序 hash 绑定和新 hash 变化，版本溢出与非法 revision 测试通过）
- [x] 为 manifest 解析、路径限制、唯一键和失败分类补充测试。（验证：工具 crate 16/16 测试覆盖严格 JSON、预算、路径、hash、尺寸、来源 URI、授权、唯一键、基准更新和稳定错误 JSON）
- [x] 运行 `git diff --check`；涉及 Rust 时运行 `cargo fmt` 和 `cargo check`。（验证：`cargo fmt --manifest-path tools/ui-visual-audit/Cargo.toml --all -- --check`、`cargo check --manifest-path tools/ui-visual-audit/Cargo.toml` 和 `git diff --check` 均退出码 0）

## 阶段 2：确定性截图环境和资源就绪条件

- 开始时间：2026-07-18 16:31:55 +08:00
- 结束时间：2026-07-18 19:50:29 +08:00
- 开发总结：扩展现有本地 UI audit recipe、状态机和 runner，建立按页面 recipe 与显式环境 override 合并的确定性捕获契约；支持 runtime profile viewport、locale/theme、state、seed、非负固定虚拟时间、终态动画和显式动态内容策略，按目标 panel/document instance 隔离资源就绪与稳定签名，并以重复 PNG SHA-256、实际截图尺寸和完整 metadata 形成可复现证据。普通非确定性 audit 保持兼容，deterministic 子开关在 audit 总开关关闭时不会影响普通游戏。
- 验证记录：worker 首轮实现后按主审核完成两轮返修，修复 recipe 死配置、全局资源误阻塞、locale/theme 仅记录不门禁、非零固定时间不生效、非法数值静默回退和 audit-off 冻结普通运行；主 agent 独立运行 local audit 测试 58/58、generated acceptance 2/2、`cargo fmt --all -- --check`、`cargo check`、runner SelfTest、`git diff --check` 均通过。真实验收 `project/target/ui-audit/acceptance-04-stage02-repair-01/` 在 phone-small/phone-portrait 各重复两次，实际 PNG hash 与 manifest 全部一致；四份 metadata 均记录 target root `20v1`、document instance `1`、全部 readiness true、`default.ron` 非 fallback、requested/actual time `123.5s` 和 delta `0`，两种尺寸截图已人工查看。

- [x] 在现有 audit recipe 中增加参考审核所需的目标 viewport、locale、theme、UI state 和可选随机种子。（验证：`project/src/framework/ui/audit/local.rs` 的 `UiAuditReferenceRecipe` builder 与 `resolve_determinism_for_screen` 将 recipe/显式 override 合并到最终 plan，并同步 `UiAuditDeterminismContext`；recipe plan 测试和 navigation 页面注册通过）
- [x] 截图前等待目标 panel、字体、图片和声明式文档构建完成，不把资源占位状态当作最终图。（验证：`collect_ui_audit_readiness` 按 owner 选择目标 root、按 document instance 和 ChildOf 层级过滤资源；目标 pending image、无关 pending image和同 document 多实例隔离测试通过）
- [x] 冻结或注入时间、随机数据、倒计时和动画进度，使静态 reference state 可重复捕获。（验证：确定性 runtime 注入合并后的 seed context、`Time<Virtual>` 固定 elapsed、zero delta/paused 和 disabled terminal motion；`123.5s` 测试及真实 metadata requested/actual 一致）
- [x] 对无法冻结的动态内容要求配置 mask 或稳定测试数据，不静默重复截图碰运气。（验证：closed `stable_fixture`/`explicit_mask` 策略要求非空 fixture 或 mask ID，runner 与 Rust config 均拒绝缺失/非法组合，相关配置测试和 SelfTest 通过）
- [x] metadata 记录应用版本、提交、viewport、scale、locale、theme、资源就绪状态和动画状态。（验证：真实四份 metadata 包含 package/version/git commit、请求与实际 viewport/scale、locale/theme source、root/instance readiness、虚拟时间、motion 和重复捕获身份）
- [x] 连续捕获相同 state，验证像素或指标波动在允许范围内；超出时分类为 `nondeterministic_capture`。（验证：同进程按 state 重复 2-8 次并比较完整 PNG SHA-256，差异返回稳定 `nondeterministic_capture`；两个 profile 各两次实际 hash 完全一致）
- [x] 对字体未加载、图片未就绪、页面不稳定和截图尺寸错误提供独立失败类型。（验证：状态机稳定输出 `font_not_ready`、`image_not_ready`、`unstable_ui`、`screenshot_size_mismatch`，另含 `document_not_ready`、locale/theme 和 nondeterministic 分类；failure string 契约测试通过）
- [x] 为就绪门控、超时、固定时间和重复捕获补充测试。（验证：local audit 58/58 覆盖资源/locale/theme 门禁、独立超时分类、123.5 秒冻结、严格 env、重复 hash、实例隔离和 audit-off 兼容）
- [x] 使用至少两个窗口 profile 运行真实重复截图验收。（验证：`acceptance-04-stage02-repair-01` 的 phone-small 为 `da04782...0163`、phone-portrait 为 `e24a66...5e02`，每个 profile 两次捕获严格同 hash且 manifest 2/2 passed）

## 阶段 3：独立图像比较引擎和 CLI 协议

- 开始时间：2026-07-18 19:52:11 +08:00
- 结束时间：2026-07-18 20:18:40 +08:00
- 开发总结：在独立 `tools/ui-visual-audit/` crate 中增加 comparison 库 API 与 `compare` CLI，建立 reference/actual/config/可选 mask/output 的严格输入协议、canonical 允许根、空输出目录和 create-new 事务报告边界，以及稳定 JSON report/error 和 exit `0/2/3/4/5` 协议；当前 `exact_rgba_v1` 只执行有预算的 RGBA8 精确比较和 full_image 区域统计，明确保留后续标准化、对齐、感知指标和差异图阶段。
- 验证记录：主 agent 独立运行工具测试 30/30、`cargo fmt --manifest-path tools/ui-visual-audit/Cargo.toml --all -- --check`、`cargo check`、CLI help、Cargo metadata/tree、Android source set、`git diff --check`、未跟踪文件空白和 fixture JSON 解析均通过。真实 phone-small 两次截图比较 exit 0、720x1600、1,152,000 像素零差异；phone-small 对 phone-portrait exit 3、稳定 `dimensions_mismatch`，两次均生成可直接解析的 canonical Windows artifact 路径与落盘 JSON。

- [x] 选择并记录独立比较引擎边界，确保比较工具不会增加 Android 游戏运行时体积或启动成本。（验证：`tools/ui-visual-audit/README.md` 记录 development-only exact boundary；Cargo metadata/tree 无 project/Bevy/Android 依赖，Android source set 仍只打包 `project/assets`）
- [x] 定义 CLI 或库输入：reference、actual、配置、可选 mask 和输出目录。（验证：`ComparisonRequest` 和 clap `compare` 子命令完整声明 repository/allowed roots、reference、actual、strict config、optional mask、output directory，CLI help 实际通过）
- [x] 定义机器可读输出，包含算法版本、尺寸、指标、区域结果、失败类型和生成 artifact。（验证：strict `ComparisonReport` v1 输出 `exact_rgba_v1`、绝对输入、dimensions、integer-millionths metrics、full_image region、typed failure 和 comparison_report artifact；stdout 与落盘 JSON 契约测试通过）
- [x] 支持非零退出码区分输入失败、比较失败、阈值失败和内部错误。（验证：公开 `ComparisonExitCode` 固定 0/2/3/4/5；真实 binary 覆盖 0/2/3/4，artifact write failure 单测覆盖 internal 5 且不 panic）
- [x] 所有路径使用绝对解析和允许根校验，禁止覆盖输入图或写入仓库外未知位置。（验证：canonical repository/input/output root containment、parent traversal、symlink/ancestor 后置 canonical、空目录与 reserved artifact 门禁均有测试；报告使用 create_new 临时文件再 rename）
- [x] 为图片解码、输出目录、同名冲突、损坏图片和不支持格式提供明确错误。（验证：bounded PNG/JPEG decode 及 `image_corrupt`、`image_unsupported_format`、`image_format_mismatch`、output file/nonempty、artifact conflict、dimension/mask 和 write failure 分类测试通过）
- [x] 准备 1x1、纯色、局部差异、透明图、尺寸不一致和损坏图的 golden fixtures。（验证：`fixtures/comparison/golden-cases.json` v1 含 7 个规格，覆盖全部要求及 unsupported GIF；测试确定性生成临时 PNG，未提交二进制且无需新增 LFS 文件）
- [x] 为 CLI 参数和 JSON 输出建立契约测试。（验证：真实 binary integration tests 覆盖 help、mask、成功 stdout/落盘一致、invalid args、reserved name 和稳定 failure/exit JSON；golden/config 使用 deny_unknown_fields）
- [x] 运行比较工具测试、`git diff --check`；涉及 Rust 时运行 `cargo fmt` 和 `cargo check`。（验证：工具 30/30 测试、fmt --check、cargo check、git diff --check 全部退出码 0）

## 阶段 4：图像标准化、尺寸检查和受限对齐

- 开始时间：2026-07-18 20:20:34 +08:00
- 结束时间：2026-07-18 20:56:17 +08:00
- 开发总结：保留 `compare/exact_rgba_v1` 语义并新增独立 `normalize-align/normalize_align_v1` 预处理边界，支持 EXIF 1..8、受限 sRGB 输入、straight alpha 隐藏 RGB 清零和 RGBA8 输出；以 strict manifest 声明四类 crop、严格物理尺寸/比例和有硬上限的确定性整数平移，scale 固定 1.0，并输出 normalized/cropped/aligned PNG、完整参数、质量/身份门禁和 original/aligned 双向坐标映射。未知 ICC/cICP/非 sRGB gamma 显式拒绝，不虚构色彩转换。
- 验证记录：worker 首轮按主审核返修相同 expected hash 被误判 swapped，并把 EXIF 证据扩展到 1..8 逐像素与坐标映射；主 agent 独立运行工具测试 42/42、fmt --check、cargo check、clippy `--all-targets -D warnings`、CLI help 和 `git diff --check` 均通过。阶段 2 phone-small 两张真实截图主验收 normalized/aligned 为 720x1600、crop 0、translation `(0,0)`、scale 1.0，后接 exact 比较 1,152,000 像素零差异；phone-small 对 phone-portrait 稳定 exit 3 `aspect_ratio_mismatch`，失败报告保留中间图且无 aligned artifact，两张 aligned PNG 已人工查看。

- [x] 统一参考图与实现图的方向、色彩空间、alpha 处理和像素格式。（验证：`normalization.rs` 应用 EXIF 1..8、接受明确/约定 sRGB 且拒绝未知 profile，输出 `srgb`/`straight_zero_transparent_rgb`/`rgba8`；EXIF 1..8、RGB 和透明 alpha golden 通过）
- [x] 默认要求目标物理尺寸和长宽比一致；不一致时先失败并报告，不直接拉伸到相同尺寸。（验证：crop 后先以整数交叉乘法检查比例再检查物理尺寸，稳定分类 `aspect_ratio_mismatch`/`dimensions_mismatch`，所有报告 scale 固定 1.0且无 resize 路径）
- [x] 支持 manifest 明确声明的系统 UI 裁切、安全区偏移和固定边框裁切。（验证：strict `CropDeclaration` 支持 `none/system_ui/safe_area/fixed_border` 及四向 insets，逐角色记录 before/after dimensions；三类显式裁切 golden 通过）
- [x] 只允许小范围整数或亚像素平移校正，并设置最大偏移，避免对齐算法掩盖真实布局错误。（验证：none/integer_search/declared_integer 仅做确定性整数裁重叠区，per-axis 配置上限且硬上限 16；越界稳定 `maximum_translation_exceeded`，无插值或缩放）
- [x] 输出对齐前后尺寸、裁切、缩放和平移参数，并保留标准化中间图。（验证：NormalizationReport 记录 original/oriented/cropped/aligned sizes、crop、selected/max translation、millionths scale；成功 artifact 含 6 张中间 PNG 和 report，真实 7 个 artifact 全部存在）
- [x] 检测全透明、近空白、明显截图错误和 reference/actual 互换。（验证：独立 `screenshot_too_small`、`image_all_transparent`、保守 dominant-sample `image_near_blank`、expected hash identity 和仅在不同角色 hash 交叉匹配时的 `inputs_swapped`；同 hash 合法回归通过）
- [x] 为 EXIF 方向、alpha、色彩转换、尺寸失败和最大平移补充 golden tests。（验证：normalization integration 8 项覆盖 EXIF 1..8 非方形逐像素、alpha/RGB、crop、比例/尺寸、自动/越界平移、ICC、质量与身份；工具总计 42/42）
- [x] 验证对齐后的坐标仍能映射回 reference element 和 UI node bounds。（验证：每个角色报告 original_to_aligned/aligned_to_original 整数仿射、有效原始/对齐 bounds；平移后的 reference element 与 actual node rect 映射相同并分别 round-trip，EXIF 1..8 mapping 逐项通过）
- [x] 运行比较引擎测试和至少一组真实截图对齐验收。（验证：工具 42/42；主 agent 真实 phone-small normalize exit 0、aligned 两图 SHA-256 相同且 exact 零差异，跨 profile 严格比例失败 exit 3）

## 阶段 5：像素差异、感知指标和差异图

- 开始时间：2026-07-18 20:58:06 +08:00
- 结束时间：2026-07-18 21:37:55 +08:00
- 开发总结：新增独立 `analyze-diff/ui_diff_metrics_v1` 分析边界，在保留 raw RGBA 和 alpha 证据的同时提供受限容差、固定点 SSIM、Sobel 几何边缘、颜色和 4 连通大面积内容分类；确定性生成 side-by-side、overlay、heatmap、binary diff 与 JSON 报告，并以 create-new 临时文件、同目录 no-clobber hard link 和事务回滚保护 artifact。输入严格限定为已对齐的 RGBA8/sRGB PNG，拒绝透明像素隐藏 RGB；分析前执行与报告共用的 512 MiB 保守内存预算估算。
- 验证记录：worker 首轮实现后按主审核完成 1 轮返修，补齐内存预算硬门禁、artifact 并发冲突/回滚不覆盖未知文件和 `aligned_alpha_invalid`；主 agent 独立运行工具测试 55/55、fmt --check、cargo check、clippy `--all-targets -D warnings`、CLI help、`git diff --check` 均通过。真实 phone-small 主验收 `project/target/ui-visual-audit/stage05-main-phone-small/` 分析 720x1600、1,152,000 像素，raw/tolerated/geometry/color/large-area 差异均为 0、SSIM 1,000,000/1,000,000（18,000 窗口）；耗时 3,774 ms，保守峰值估算 77,922,304 bytes/512 MiB，四张 PNG 共 146,173 bytes，artifact 尺寸和内容已人工查看且无临时残留。

- [x] 实现逐像素绝对差、平均误差、最大误差、超阈值像素比例和 alpha 差异。（验证：`RawPixelMetrics` 固定输出 RGBA 各通道 sum/mean/max/over-threshold、整体 changed/ratio/mean/max，并以独立 `AlphaMetrics` 保留 alpha 统计；CLI/序列化契约测试通过）
- [x] 增加至少一种适合 UI 的结构或感知指标，并记录选择理由、版本和数值范围。（验证：固定点 `wang2004_ui_luma_fixed_window_v1` SSIM 使用 8x8 局部亮度窗口，报告记录 UI 几何敏感的选择理由、Wang 2004 常量和 [-1,000,000, 1,000,000] 范围；identity 与 constant shift 固定值测试通过）
- [x] 分离几何边缘差异、颜色差异和大面积内容差异，避免只给一个无法解释的总分。（验证：报告分别输出 same-coordinate Sobel edge XOR、边缘成员一致时的容差后颜色差异，以及满足像素/比例下限的 4 连通差异组件，包含计数、比例和 bounds）
- [x] 生成同尺寸 side-by-side、半透明 overlay、热力图和二值差异图。（验证：成功事务固定生成 `side-by-side.png` 1440x1600、其余三图 720x1600 和 `diff-metrics-report.json`；真实图逐张人工查看，零差异 heatmap/binary 为全黑）
- [x] 对抗锯齿和字体渲染差异设置小范围容差，但不得扩大到掩盖字号或位置错误。（验证：小通道容差只接受全部 RGBA <=3；抗锯齿只接受同坐标双边缘、RGB <=12 且 alpha <=3，不做邻域搜索、模糊、缩放、平移或重新对齐；1px shift golden 仍保留差异）
- [x] 保持指标计算确定性，固定线程、舍入和颜色转换中会影响结果的参数。（验证：报告固定 single-thread row-major、BT.601 整数亮度、白底整数 alpha 合成、Sobel/SSIM 参数、i128 有理数和 signed half-up millionths；重复 artifact 像素与文本指标测试通过）
- [x] 用纯色偏差、1px 位移、字体边缘、缺失控件和大面积背景变化校准指标。（验证：`ui-diff-metrics-v1.golden-cases.json` 覆盖 solid color bias、1px shift、font antialias edge、missing control、large background change，另含 alpha change）
- [x] 为每个指标建立 golden value 或允许误差测试。（验证：文本 fixture 固定完整序列化 metrics 对象 SHA-256，覆盖 raw、alpha、tolerated、SSIM 和三类 explainable category；`metrics_golden` 4/4 通过）
- [x] 运行比较引擎完整测试并记录性能与内存使用。（验证：工具完整测试 55/55；真实 1,152,000 像素用时 3,774 ms、估算峰值 77,922,304 bytes，边界测试确认 8,323,072 像素恰好满足 512 MiB、再多 1 像素稳定失败 `image_too_large`）

## 阶段 6：区域、遮罩和加权审核规则

- 开始时间：2026-07-18 21:39:21 +08:00
- 结束时间：2026-07-18 22:16:32 +08:00
- 开发总结：新增独立 `audit-regions/ui_region_audit_v1` 区域审核边界，将矩形、多边形和 RGBA PNG mask 确定性栅格化到 aligned 坐标，统一接入 reference element、声明式 node 和手工区域；支持显式 include/exclude union、critical/normal/decorative 局部阈值和权重、reference hash/revision 与 mask 文件 hash 绑定、逐区域完整指标/局部状态/主要差异位置，以及 ignored/coverage 两张可视化证据。区域层复用阶段 5 masked metrics，不改变既有全图 golden；局部失败明确不越权成为阶段 9 全局门禁。
- 验证记录：worker 首轮实现后按主审核完成 1 轮返修，修复 `full_image` 在声明区域未覆盖整图时夸大有效审核覆盖，并固定区域附加内存预算临界测试；主 agent 独立运行工具测试 67/67、fmt --check、cargo check、clippy `--all-targets -D warnings`、CLI help 和 `git diff --check` 均通过。真实 phone-small 主验收 `project/target/ui-visual-audit/stage06-main-real-hud-repair-01/` 为 720x1600、1,152,000 像素，动态顶部带忽略 34,560 像素（3%）、include union 840,320 像素，4 个区域全部局部通过、passed weight 250/failed 0；3 个 artifact 均存在，ignored/coverage PNG 已人工查看。

- [x] 支持矩形、多边形或 mask 图片定义动态忽略区域，并记录坐标空间。（验证：strict `RegionShape::{Rectangle,Polygon,MaskImage}` 和 `CoordinateSpace::{Aligned,ReferenceOriginal,ActualOriginal}` 逐像素解析；fixture、三角形 pixel-center 栅格化和 RGBA mask integration 测试通过）
- [x] 支持 critical、normal、decorative 等区域级别和不同阈值，关键文字与按钮权重最高。（验证：三档 profile 独立配置 raw/alpha/tolerated/SSIM/geometry/large-area 阈值且强制 weight 严格递减；`key_text`/`key_button` 必须为 critical，非法顺序稳定拒绝）
- [x] 支持只审核区域和排除区域，但拒绝覆盖整张图或超过配置比例的无理由 mask。（验证：`declared_regions_only` 使用 include union，ignore union 以未舍入交叉乘法执行比例门禁并强制非空 reason；整图排除、超比例、排除后空区域均失败，`full_image` 还要求声明区域逐像素覆盖整图，否则 `audit_scope_incomplete`）
- [x] 将 reference element bounds、声明式 node bounds 和手工区域统一映射到比较坐标。（验证：`BoundsSource` 限定 reference element 使用 reference original/aligned、declarative node 使用 actual original/aligned，manual shape 走同一 resolver；非 identity 双映射单测确认分别使用阶段 4 对应仿射）
- [x] 每个区域独立输出像素指标、感知指标、通过状态和主要差异位置。（验证：每个 `RegionAuditResult` 输出 raw/alpha/tolerated/SSIM/三类 category、实际 threshold、local status/violations、重叠和最多 5 个 aligned/reference-original/actual-original 定位；非零差异 fixture 断言通过）
- [x] mask 与 reference hash 或版本绑定，参考图变化后旧 mask 必须重新确认。（验证：全局与每个 ignore 重复绑定 original reference SHA-256/positive revision，并交叉绑定成功 normalization report；PNG mask 另绑定文件 SHA-256，stale reference/revision、错误 hash/尺寸使用稳定错误拒绝）
- [x] 报告清楚显示被忽略区域，防止审核结果看似通过但证据不完整。（验证：coverage 报告记录 include/ignore/effective/uncovered 像素和比例，`ignored-regions.png` 将全部排除像素绘为不透明洋红，`audit-coverage.png` 分色 critical/normal/decorative/ignored/uncovered；真实图已检查）
- [x] 为边界裁切、重叠区域、空区域、超范围 mask 和权重合并补充测试。（验证：区域单元 5 项和 integration 7 项覆盖 reject/clip、polygon、region/ignore overlap、empty、mask size/hash/binding、ratio、weight、full-image gap、事务 no-clobber 与双坐标映射）
- [x] 运行区域审核 fixture 和真实 HUD 动态区域验收。（验证：工具完整 67/67；fixture 包含局部失败且 CLI exit 0 的阶段边界，真实 720x1600 HUD 四区域局部通过、3% 动态带显式排除；区域峰值固定为 `72*p + 4 MiB`，7,398,286 像素通过、再多 1 像素稳定 `image_too_large`）

## 阶段 7：UI 树和语义布局审核

- 开始时间：2026-07-18 22:18:10 +08:00
- 结束时间：2026-07-19 02:27:46 +08:00
- 开发总结：完成 runtime semantic tree schema v3 和独立 `ui_semantic_audit_v1` 工具链，覆盖稳定节点身份、逻辑坐标/裁切、文字/触控/滚动/状态/弹层硬失败、声明式与传统源码定位，以及与视觉分数严格分离的报告。主审核共完成 3 轮返修：为 finding 和 overlap candidate 增加固定上限及稳定错误码，补齐 panel/Toast 真实 Entity、Name、源码提示，统一可见 Toast ordinal，并修复完全裁切文字/placeholder 被误认为可见 label 的漏报。
- 验证记录：主 agent 独立运行工具全量 91/91、fmt check、`cargo check --all-targets`、clippy `--all-targets -D warnings`，项目 fmt check、`cargo check --tests`、semantic 8/8、Toast 1/1 和 `git diff --check` 均通过。最终 Compact `stage07-repair03-runtime-compact-v3` 与 Expanded `stage07-main-runtime-expanded-v3-final` 各重复 capture 2 次，semantic tree 与 PNG 分别严格一致，截图 SHA-256 为 `530e5f1c...bdff` / `1f164e27...1239`；两份 strict 报告均为 24 nodes、1 panel、14 evaluated、0 hard failure、约 33.7/67.1 MiB，且视觉相似度和局部分数均未参与。

- [x] 扩展 audit metadata，稳定输出可见节点 bounds、clip bounds、文字测量、层级、node ID 和交互状态。（验证：`project/src/framework/ui/audit/semantic.rs` 输出 schema v3、半开逻辑矩形、1/64 归一化、parent/depth/stack_index、稳定 ID、TextLayoutInfo、Interaction 与 panel 信息；Compact/Expanded 重复 semantic tree 严格一致）
- [x] 检测文字重叠、关键文字裁切、节点越出安全区、不可达滚动内容和明显零尺寸节点。（验证：`tools/ui-visual-audit/src/semantic.rs` 实现 5 类规则；20k 单轴分离、260 节点双轴退化、47 全重叠、零尺寸先于 fully-clipped 等测试通过，candidate/finding 超限分别返回稳定错误码）
- [x] 检测按钮、输入框和图标按钮的最小触控目标、可见 label、disabled/loading 状态一致性。（验证：semantic golden 覆盖 touch/label/disabled/loading；runtime World 测试证明普通 Text 与 TextInput placeholder 完全裁切不算 label、部分可见才算，typed value 不替代 label）
- [x] 检测 Modal、Loading、Floating、Toast 的层级、焦点限制和下层输入阻断。（验证：14 项 semantic integration 覆盖 modal/dropdown/tooltip/loading/toast 的 z/focus/Pickable/InputState；Toast 全子树 pass-through 测试 1/1，通过可见 Toast ordinal World 回归）
- [x] 对声明式页面将语义问题定位到 document ID、node ID 和 source path；传统页面定位到 panel、实体名和 likely files。（验证：SemanticLocation 保留 declarative document/node/source；node/panel 传统 finding 保留真实 capture Entity、可选 Name、panel ID 与精确 likely files，modal/tool 和 runtime schema 断言通过）
- [x] 将语义硬失败与视觉分数分开，禁止高视觉相似度抵消不可点击或裁切问题。（验证：Compact/Expanded strict 报告均为 `visual_similarity_consumed=false`、`local_visual_scores_consumed=false`、`can_visual_score_offset_hard_failure=false`，hard failure 独立决定 semantic status/exit 4）
- [x] 为重叠、裁切、越界、触控尺寸、弹层和滚动不可达准备自动 fixture。（验证：`tools/ui-visual-audit/fixtures/semantic/` 与 14 项 golden integration 覆盖全部 12 种 finding code、strict schema、事务 no-clobber、引用完整性和两类容量上限）
- [x] 在 Compact 与 Expanded profile 下运行语义审核测试。（验证：最终 phone-small 360x800/720x1600 与 tablet-landscape 1280x800/2560x1600 各 2 次 deterministic capture；两份报告均 passed、24 nodes、14 evaluated、0 hard failure）
- [x] 在 `project/` 运行 `cargo fmt`、语义审核测试和 `cargo check`。（验证：主 agent 运行 `cargo fmt --all -- --check`、`cargo check --tests`、`cargo test --lib semantic::tests` 8/8 和 Toast 定向 1/1 全部通过）

## 阶段 8：实际 AI 视觉分析适配器

- 开始时间：2026-07-19 02:30:12 +08:00
- 结束时间：2026-07-19 05:13:58 +08:00
- 开发总结：新增 `ui_ai_visual_analysis_v1` 严格协议和 `analyze-ai` CLI，复用轻量 `ui-generation/provider-core` 支持 Fixture、Mock 与显式 HTTPS 在线 provider；完整绑定四类图片、diff/region/semantic/runtime metadata 证据，加入有界图片解码、在线像素遮罩、结构化输出校验、失败分类、确定性 hard failure 保留和 runner Provider 模式。
- 验证记录：主代理运行 `ui-visual-audit` fmt/check/clippy `-D warnings` 与全量测试通过（118 passed，1 个显式在线样例因未配置 endpoint/key/model ignored），AI CLI provenance 测试 4/4 通过，mock 分类并发重复 15/15 通过；`ui-generation` 全量 146/146、provider-core 25/25、boundary、项目 fmt/check、runner self-test、Off dry-run、无 project/Bevy 依赖检查和 `git diff --check` 均通过。在线配置样例已记录但未在缺少外部凭据时发起真实调用。

- [x] 在现有 analysis input/output 协议上实现真实 AI provider adapter，保留 Fixture 和 Off 模式。（验证：`tools/ui-visual-audit/src/ai.rs` 实现 Fixture/Mock/显式 Online adapter 与 `analyze-ai`；`scripts/run-ui-audit.ps1` 保留 Auto/Fixture/Off 并新增 Provider，runner self-test 通过）
- [x] AI 输入包含 reference、actual、overlay、heatmap、区域指标、UI metadata 和允许差异说明。（验证：`AiCaptureBundle`、`load_captures`、`build_provider_context` 绑定四图、diff/region/semantic、metadata、allowed differences；AI CLI provenance 4/4 通过）
- [x] 使用严格 structured output 返回 problem type、severity、evidence、region、reference element、node ID、likely cause 和 suggested files。（验证：`AiProviderIssue`、`provider_output_json_schema` 均启用严格字段约束；`provider_output_schema_rejects_unknown_fields_and_pass_claims` 通过）
- [x] 校验 AI 引用的截图、区域和节点确实存在，拒绝无法对应证据的结果。（验证：`validate_provider_output` 校验 capture/image/region/node/bounds/file；伪造引用、artifact、metadata hash 的单元与 CLI 测试通过）
- [x] AI 不得把确定性 hard failure 降级为通过；只能补充解释或提升严重度。（验证：报告复制 semantic findings 并固定 `deterministic_hard_failures_preserved = true`；schema 无 pass/降级字段，runner self-test 验证 hard failure 独立阻断及伪造 capture 拒绝）
- [x] 生成模型与审核模型应支持独立配置；默认不以生成模型的自评作为唯一审核结论。（验证：三类 `AiProviderConfig` 独立记录 audit/generation model，报告固定 `self_review_is_sole_conclusion = false`；独立模型配置测试通过）
- [x] 实现超时、限流、鉴权、响应非法、图片过大和 provider 不支持的失败分类。（验证：`map_provider_failure` 提供稳定错误码；mock 全分类测试通过且并发重复 15/15，图片 header/解码/响应上限测试通过）
- [x] 对日志、图片和模型响应执行凭据与敏感文字脱敏。（验证：在线图片使用语义文字与显式 privacy rect 不透明遮罩，metadata 有界收集、Aho-Corasick echo 脱敏及 token pattern 脱敏；像素副本、缺失文字边界 fail-closed、凭据/文本输出测试通过）
- [x] 为 fixture、mock 和至少一个显式启用的在线分析样例记录验证结果。（验证：fixture 与 mock 纳入 118 项全量测试；`explicit_online_openai_compatible_sample` 和 `online-openai-compatible.config.example.json` 提供显式启用样例，本地因未配置 endpoint/key/model 保持 1 ignored 且未发起网络请求）

## 阶段 9：评分、阈值和通过门禁

- 开始时间：2026-07-19 05:16:07 +08:00
- 结束时间：2026-07-19 06:08:52 +08:00
- 开发总结：新增独立 `ui_visual_gate_v1` 聚合门禁与 `evaluate-gate` CLI，以 path + SHA-256 绑定 diff/region/semantic/可选 AI 报告，按 reference profile 或保守默认六项阈值逐区域决策，输出 `passed`、`failed`、`needs_review`、`invalid` 四态、稳定 failure type、确定性优先级和完整评分分解；Stage 6 local threshold 仅保留为 upstream diagnostics，不覆盖 Stage 9 profile 决策。
- 验证记录：主代理运行工具 fmt/check/clippy `-D warnings` 与全量测试通过（134 passed，1 个显式在线样例 ignored），Gate CLI/真实 Stage 8 AI 聚合测试 8/8、Gate 核心/校准测试 8/8 通过；`evaluate-gate --help`、项目 fmt/check、当前 PowerShell runner self-test、scope 检查和 `git diff --check` 均通过。16 个仓库维护者合成标注案例与正式 profile 双向绑定，六项指标及四态精确校验为 0 误报、0 漏报、0 错分；该结果明确不代表生产样本或用户研究。

- [x] 定义尺寸、语义 hard failure、关键区域、普通区域、装饰区域和 AI issue 的合并顺序。（验证：`gate.rs` 的 `merge_order`/`failure_priority` 固定 invalid、dimension、semantic、critical、AI severe、normal、AI medium、decorative review 顺序；真实 AI 优先级测试通过）
- [x] 允许每个 reference profile 配置阈值，提供保守默认值，不使用一个全局分数覆盖所有 UI 类型。（验证：`GateConfig` 为每个 profile 提供三类区域各六项阈值并校验严格度顺序，未匹配时使用 `conservative_default`；profile 放宽/收紧集成测试通过）
- [x] 定义 `passed`、`failed`、`needs_review` 和 `invalid` 四类终态及稳定 failure type。（验证：`GateState`、`GateFailureType` 和 `GateExitCode` 固定四态、八类优先 failure type 及 0/4/3/2 退出码；CLI 四态测试通过）
- [x] 明确严重和中等问题的阻断规则，轻微问题进入报告但不自动阻断。（验证：`evaluate_ai`/`ai_gate_rule` 保留全部 AI issue，severe/medium blocking、minor report-only；真实 Stage 8 Fixture AI severe/medium/minor Gate 测试通过）
- [x] 防止多个低分区域被平均后隐藏，关键区域任一失败应保持可见。（验证：每个 `GateRegionResult` 独立保存 profile violations、state 与 blocking，`averaging_used_for_gate = false`；critical 不可平均隐藏测试通过）
- [x] 用人工标注评测集校准阈值，记录误报、漏报和调整理由。（验证：`fixtures/gate/human-labeled-cases.json` 包含 16 个维护者合成标签、六项指标、四态定义和调整理由；测试从正式 config 派生阈值并精确得到 0/0/0，文档明确非生产校准）
- [x] 输出评分分解，不只输出“相似度 80%”之类单一结果。（验证：`VisualGateReport` 分解 dimensions、semantic、各 level/region 六项指标、upstream diagnostics、profile violations、AI issues 和 reasons，固定 `global_numeric_score_emitted = false`）
- [x] 为边界值、hard failure 优先级和 `needs_review` 分支补充测试。（验证：Gate 核心 8/8 与 CLI/AI 8/8 覆盖阈值等于/上下 1、四态、invalid hash、dimension/semantic/critical 优先、decorative review、determinism/no-clobber）
- [x] 运行 runner self-test 和门禁 fixture。（验证：`& .\scripts\run-ui-audit.ps1 -SelfTest` 通过；Gate CLI 8/8 和工具全量 134 passed，1 online ignored）

## 阶段 10：报告、基准更新和问题定位

- 开始时间：2026-07-19 09:23:53 +08:00
- 结束时间：2026-07-19 10:06:20 +08:00
- 开发总结：新增严格 `ui_comparison_bundle_v1` 报告构建、问题定位和显式 baseline plan/apply/verify 命令；报告以当前 reference manifest、reference artifact 和 binding 三方哈希约束 capture，基准更新要求独立人工批准、事务性 receipt 和完整关联矩阵重跑。Runner 记录 analysis/fix artifact links，并在自动修复命令执行前阻断 baseline 入口。
- 验证记录：worker 完成两轮主审核返修，补齐 active manifest/reference artifact 绑定、receipt 发布失败回滚和 comparison result 来源重新验证；主 agent 独立运行 `cargo test --manifest-path tools/ui-visual-audit/Cargo.toml --test report_baseline_cli` 8/8、fmt --check、cargo check --all-targets、clippy `-D warnings`、`run-ui-audit.ps1 -SelfTest` 与 `git diff --check` 均通过。完整工具测试为 75 passed、1 online sample ignored；所有本地 fixture 成功/失败演示在临时目录退出时清理。

- [x] 扩展 `report.md`，按 screen、device、state 展示 reference、actual、overlay、heatmap、指标和语义问题。（验证：`tools/ui-visual-audit/src/report.rs` 的 `render_markdown` 逐 capture 渲染四图链接、六项指标、region 表和问题表；`report_baseline_cli` 成功报告契约测试通过）
- [x] 生成机器可读 comparison JSON，并从根 manifest、analysis 和 fix iteration 双向关联。（验证：`ComparisonResult` 与 `validate_comparison_result_provenance` 重新验证 root `comparison.input` hash、analysis/fix backlink 和 capture projection；伪造 root/input 负向测试通过）
- [x] 每个问题显示区域、严重度、证据、node ID、source path、likely cause 和建议修改范围。（验证：`LocatedIssue` strict schema 和 `render_markdown` 问题表覆盖所有字段，未定位 region/越界 evidence fixture 稳定拒绝）
- [x] 报告明确显示 mask、允许差异、算法版本、阈值和 AI 是否实际运行。（验证：`ComparisonCapture` 的 `masks`/`allowed_differences`/`algorithms`/`thresholds`/`ai` 受 strict schema 校验并由 `render_markdown` 显示；CLI contract 测试通过）
- [x] 建立显式基准更新命令，要求更新原因、旧新图片、指标变化和人工批准记录。（验证：`baseline.rs` 的 `plan_baseline_update`、`apply_baseline_update`、`verify_baseline_rerun` 和 CLI 三个子命令绑定 reason、图片、metrics、plan SHA 与 approval record）
- [x] 禁止自动修复循环直接执行基准更新；检测到基准变更时默认阻断。（验证：`scripts/run-ui-audit.ps1` 的 `Test-UiAuditFixCommandBoundary` 在执行前拒绝 plan/apply/verify，SelfTest 验证没有命令副作用；`validate_baseline_guard` 拒绝未经 receipt 的 binding 变化）
- [x] 基准更新后重新运行全部关联 device/state，不只审核更新的单张图。（验证：`related_capture_requirements` 按 screen/locale/theme 生成完整 requirement；`verify_baseline_rerun` 核对 active manifest、reference ID、artifact hash 和 passed gate，缺失/alias identity fixture 均拒绝）
- [x] 为链接完整性、artifact 缺失、基准冲突和未批准更新补充测试。（验证：`tools/ui-visual-audit/tests/report_baseline_cli.rs` 覆盖 missing/swapped link、active manifest/reference artifact 伪造、unapproved/stale conflict、receipt 事务回滚及 rerun provenance）
- [x] 运行本地成功与失败报告演示并清理临时产物。（验证：`local_success_and_failure_report_demo_cleans_all_temporary_artifacts` 生成 `passed`/`failed` 报告后确认临时根已删除；工具测试通过）

## 阶段 11：Runner、CI、远程设备和文档验收

- 开始时间：2026-07-19 10:08:31 +08:00
- 结束时间：2026-07-19 11:07:21 +08:00
- 开发总结：将 strict reference matrix 接入现有 Runner，按 reference manifest 自动生成 `screen/device/state` task、执行 compare/normalize/diff/regions/semantic/Fixture-or-explicit-Provider AI/gate/build-report 全链，并写入 comparison evidence、失败精确复跑、缓存/LFS/超时/缺 artifact 日志与矩阵预算。新增 Windows GitHub Actions workflow，默认运行 LFS checkout、Rust stable、工具 contract 和离线两设备多状态 Fixture self-test；在线 AI 保持显式受控入口。真实 Android 真机验证经当前范围确认暂不执行，现有 Http metadata 合同缺口继续由 Mock fail-closed 回归记录。
- 验证记录：主 agent 独立运行 `run-ui-audit.ps1 -SelfTest`，strict report 为 3/3 capture（`ui_gallery.phone-small.initial`、`ui_gallery.phone-small.bottom`、`ui_gallery.tablet-portrait.initial`），耗时 92,193 ms、artifact 2,009,481 bytes，完整 comparison 链通过；工具完整测试 75 passed、1 online sample ignored，项目 `cargo fmt --all -- --check`/`cargo check`、两个 PowerShell AST parse、CI workflow 静态审阅和 `git diff --check` 均通过。`-Remote -RemoteBackend Mock -RequireRealAndroid` 已验证稳定写入 `external_unavailable` 并非真机通过。

- [x] 将比较引擎接入现有 `run-ui-audit.ps1`，保留无 reference 的普通审计模式。（验证：`Complete-UiAuditReferenceComparison` 串联 compare 至 build-report；Runner 强制 `-ReferenceManifest` 配合 `-StrictReference`，普通模式不读取 reference，SelfTest 原有普通/remote/fix 回归通过）
- [x] 支持按 reference manifest 自动展开 screen、device、state，并只复跑失败映射的相关矩阵。（验证：`Get-UiAuditReferenceEntries`/`Get-UiAuditReferenceTaskSeeds` 依据 manifest 生成矩阵，`Get-FailedTaskSeedsFromManifest` 消费 `comparison.failed_captures` 的精确 state；严格 self-test 验证 3 条 mapping）
- [x] CI 默认运行确定性比较和 Fixture AI；真实在线 AI 作为显式启用或定时任务。（验证：`.github/workflows/ui-visual-audit.yml` 在 PR/push 默认执行 Windows LFS checkout、Rust stable、工具检查及 strict Fixture self-test；`run-ui-audit-ci.ps1 -OnlineAi` 才要求显式 provider config）
- [x] 对比较工具缓存、基准图 LFS 拉取失败、artifact 缺失和超时提供明确日志。（验证：`Invoke-UiAuditVisualTool` 写入 `logs/comparison/`、报告 cache hit/cold 与超时；`Get-UiAuditReferenceEntries` 对 LFS pointer/missing 图提示 `git lfs pull`，每步 artifact 缺失稳定失败）
- [x] 真实远程 Android 截图与 metadata 验证暂不执行，不作为本清单完成门槛。（已知限制：当前没有可用 Android 设备及返回 status bar/safe area/font/touch 的 Http metadata 合同；`Complete-UiAuditAndroidValidation` 继续以 `external_unavailable`/`pending_remote_metadata_validation` fail-closed，Mock 不作为真机证据）
- [x] 记录比较耗时、峰值内存、artifact 大小和矩阵总耗时，设置合理预算。（验证：`manifest.comparison.performance` 记录 matrix elapsed、estimated peak memory、artifact bytes 与 1800 s/768 MiB/1 GiB 默认预算；严格 self-test 写入 92,193 ms、2,009,481 bytes）
- [x] 更新 `docs/ui/UI调试与验收.md`、自动化审计方案和新成员运行说明。（验证：三个文档说明 strict 命令、CI/在线 AI 边界、artifact 目录、预算与 Android fail-closed 状态）
- [x] 运行 runner self-test、至少一个双设备 reference audit、`git diff --check`。（验证：主 agent `run-ui-audit.ps1 -SelfTest` 通过，严格 comparison result 断言 3 capture、2 devices、2 states；`git diff --check` 退出码 0）
- [x] 在 `project/` 运行 `cargo fmt`、相关测试和 `cargo check`。（验证：主 agent 运行 `cargo fmt --all -- --check` 与 `cargo check` 退出码 0；工具完整 `cargo test` 通过）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有阶段完成后统一验收。

- 开始时间：2026-07-19 11:09:23 +08:00
- 结束时间：2026-07-19 11:09:23 +08:00
- 验收总结：阶段 10 和 11 分别提交为 `5dc95d1`、`d9af769`。严格 manifest matrix、comparison evidence、baseline 人审闭环、离线 CI 和两设备多状态 Fixture audit 均已验收；真实 Android 真机验证经当前范围确认暂不执行，不作为本清单完成门槛。Runner 仍以 fail-closed 记录无设备/无 Http metadata 合同的限制，未将 Mock 作为设备证据。

- [x] 每个受审核的 screen、device、state 都能唯一映射到参考图、实现截图、metadata 和比较配置。（验证：`Get-UiAuditReferenceTaskSeeds`/`Get-UiAuditCapturedReferencePairs` 使用唯一 capture ID，三 capture strict self-test 生成 hash-bound bundle）
- [x] 尺寸不匹配、关键裁切、文字重叠、控件不可达和弹层错误会稳定阻断，不受 AI 主观评分覆盖。（验证：normalization、semantic 和 gate contracts 的完整工具测试 75 passed；hard failure priority 不消费 AI pass）
- [x] 审核报告包含 side-by-side、overlay、heatmap、区域指标、语义问题和可定位修复建议。（验证：`report.rs::render_markdown`/`comparison-result.json` 和 Runner strict self-test 的 report artifact 通过）
- [x] 动态 mask 和允许差异显式可见、受比例限制并与参考版本绑定。（验证：`ui_region_audit_v1` binding/coverage contract 和 strict report 的 `masks`、`allowed_differences` 字段）
- [x] 相同输入和算法版本产生一致的确定性指标与通过结果。（验证：确定性 screenshot/normalize/diff/gate contracts 与三 capture strict Fixture self-test 3/3 passed）
- [x] 在线 AI 不可用时仍可完成确定性审核；在线启用时结果经过 Schema 和证据校验。（验证：CI offline Fixture workflow、`run-ui-audit-ci.ps1 -OnlineAi` 显式 config 门槛，以及 AI provider strict output tests）
- [x] 基准更新必须有人工批准记录，自动修复流程无法静默改变基准。（验证：`BaselineApproval`/`BaselineUpdateReceipt`、receipt rollback test 和 `baseline_update_forbidden` SelfTest）
- [x] 本地至少两个设备 profile 和一个多状态页面完成 reference audit。（验证：主 agent Runner self-test 的 `phone-small/initial`、`phone-small/bottom`、`tablet-portrait/initial` 3/3 passed，92,193 ms、2,009,481 bytes artifacts）
- [x] 真实 Android 审核有验证记录，或明确保留外部设备链路阻塞项。（验证：`Complete-UiAuditAndroidValidation` 报告 `external_unavailable`/`pending_remote_metadata_validation`；Remote Mock + `-RequireRealAndroid` fail-closed，不冒充真机成功）
- [x] runner self-test、比较引擎测试、`cargo fmt`、相关测试和 `cargo check` 全部通过。（验证：Runner self-test、工具 `cargo test` 75 passed/1 ignored、项目 fmt/check、工具 fmt/check/clippy 和 `git diff --check` 均退出码 0）
