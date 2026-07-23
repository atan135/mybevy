# 05. AI 生成审核闭环工程化 Checklist

## 目标

将参考图输入、AI 生成、声明式 UI 预览、视觉审核、问题定位、受限修复、复跑和人工晋升串成一套可恢复、可追溯、可控制成本的端到端工程流程。该流程建立在现有 UI audit Runner 与修复循环骨架上，补齐真实生成器、真实审核器和安全执行边界。

默认修复目标是 staging 中的 `UiDocument` 和草稿素材。只有审核问题被明确归类为通用组件或框架能力缺口，并经过人工批准后，才允许修改 Rust、主题公共结构或正式资源。流程不自动提交、push 或更新参考基准。

## 已有基础与依赖

- 现有 `run-ui-audit.ps1` 已支持设备矩阵、analysis fixture、FixMode、迭代快照、`cargo fmt`/`cargo check` 和安全策略骨架。
- 依赖 `01_UI高保真视觉基础能力_checklist.md`、`02_UI声明式描述与运行时生成_checklist.md`、`03_AI参考图生成UI_checklist.md` 和 `04_UI参考图视觉审核_checklist.md` 的最终公共接口。
- 真实远程 Android 设备执行依赖外部 adminapi、game-server 和 client 调试链路；本仓库负责调用、artifact 和失败处理。
- 本清单是整体工程化与最终验收清单，不重复实现前置清单内部算法。

## 基础原则

- 复核开始时间：2026-07-22 18:58:54 +08:00
- 复核结束时间：2026-07-23 10:40:03 +08:00
- 复核总结：复核阶段 1 至 9 的闭环实现并补齐持久化重试上限、Command 修复 worktree 隔离与严格审核迭代预算初始化；完成快速隔离自测及完整离线 Runner 回归，不调用在线 provider、远程设备或 AI 凭据。

- [x] 每次运行拥有不可变输入快照、唯一 run ID、显式状态机和完整 artifact 关联。（验证：`tools/ui-generation/src/run_manifest.rs:1375` 的 `ClosedLoopRunManifest` 与 checkpoint/recovery 契约；主 agent 运行 `cargo test --manifest-path tools/ui-generation/Cargo.toml closed_loop_ -- --nocapture`，30 passed）
- [x] 自动化只在独立工作目录或专用 Git worktree 中修改文件，不破坏用户现有 dirty worktree。（验证：`scripts/run-ui-audit.ps1:3249` 创建 detached worktree，`-SelfTestWorktree` 以 caller dirty fixture 覆盖不复制、仅 worktree 修改、失败/取消回滚；主 agent 复跑通过）
- [x] 修复范围按 UI 文档、草稿资源、页面局部代码、通用控件、主题、框架核心逐级升级。（验证：`scripts/run-ui-audit.ps1:1639` 的策略及 allowed roots 分层限制；`closed_loop_` 测试覆盖计划作用域与升级审批，30 passed）
- [x] 参考图、审核阈值、mask、安全策略、验证脚本和基准图不属于自动修复允许范围。（验证：`scripts/run-ui-audit.ps1:5322` 将 protected reference/rule 归入人工复核，`-SelfTestWorktree` 与完整 self-test 均验证 protected path 拒绝）
- [x] 任一外部调用、修改、验证和审批都有超时、失败出口、重试上限和可恢复记录。（验证：`run_manifest.rs:1137` 定义每状态 `max_attempts`，`restart_from` 与 parse 均 fail-closed；`tools/ui-visual-audit/src/ai.rs:1249` 绑定 iteration/provider 上限；完整 `run-ui-audit.ps1 -SelfTest` exit 0，689.7s）
- [x] 普通开发、构建和 CI 默认不调用付费模型，也不需要 AI 凭据。（验证：`closed_loop_generation.rs:437` 的 Off mode 无副作用/凭据依赖测试通过；`.github/workflows/ui-visual-audit.yml:35` 明确 push/PR 不含 Online AI）

## 阶段 1：端到端运行契约和状态机

- 开始时间：2026-07-19 13:49:27 +08:00
- 结束时间：2026-07-19 14:19:13 +08:00
- 开发总结：在 `tools/ui-generation` 建立独立于 sealed Stage 3 bundle 的 `ClosedLoopRunManifest` v1，覆盖闭环 artifact/provenance/budget、13 态策略、持久化 checkpoint、失败/取消、精确 cache 恢复与新 attempt。恢复计划以 checkpoint index/state/attempt 绑定，避免修复循环中重复状态误复用。同步补充总体流程文档、状态图和恢复边界说明，未接入在线 provider 或自动修复。
- 验证记录：主审核打回 1 轮，修复重复 `Previewing` 等状态按枚举名恢复会误选首次 checkpoint 的问题；独立复跑 `cargo test --manifest-path tools/ui-generation/Cargo.toml` 为 153 passed，`cargo fmt --manifest-path tools/ui-generation/Cargo.toml --all -- --check`、`cargo check --manifest-path tools/ui-generation/Cargo.toml`、`cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .` 和 `git diff --check` 均通过。

- [x] 定义端到端 run manifest，关联 generation input、reference manifest、UiDocument、assets、preview、comparison、analysis、fix 和 approval。（验证：`run_manifest.rs` 的 `ClosedLoopRunManifest`/`ClosedLoopArtifactLinks` 绑定九类 artifact link，并拒绝不安全或重复路径；闭环生命周期测试通过）
- [x] 定义状态机：Created、Preparing、Generating、Validating、Previewing、Auditing、PlanningFix、ApplyingFix、Verifying、AwaitingApproval、Passed、Failed、Cancelled。（验证：`ClosedLoopRunState` 定义 13 个状态，`ClosedLoopRunState::policy` 固化允许来源与终态）
- [x] 为每个状态定义允许进入条件、持久化字段、超时、可重试性和终态。（验证：`ClosedLoopStatePolicy`、checkpoint 与文档状态表覆盖进入证据、cache key、attempt、时限、可重试性和终态；非法迁移测试通过）
- [x] 统一前置清单的 failure type，避免同一错误在生成器、审核器和 Runner 中使用不同名称。（验证：`TaskFailureKind` 新增 manifest/runner/audit/fix/approval 分类，`from_legacy_failure_type` 显式映射既有 audit `failure_type`，未知值保持未映射）
- [x] 记录工具版本、提交、模型、prompt、schema、算法、viewport、theme、locale 和预算配置。（验证：`ClosedLoopRunProvenance` 和 `ClosedLoopBudgetConfiguration` 要求全部字段非空且预算为正，损坏 manifest fail-closed）
- [x] 支持从最近一个完整状态恢复，不重复已成功且 cache key 未变化的外部调用。（验证：`ClosedLoopCheckpointIdentity` 以 index/state/attempt 区分循环状态，`recovery_plan`/`restart_from` 精确截断；重复 Previewing 回归测试验证最新 checkpoint 和单点 cache 失效）
- [x] 对非法状态跳转、manifest 损坏、版本不兼容和取消竞态补充测试。（验证：`closed_loop_manifest_rejects_illegal_state_transitions`、损坏/协议不兼容、持久化和 Passed 后取消测试均纳入工具 153 项测试）
- [x] 更新总体流程文档和状态图。（验证：`docs/ui/UI参考图生成与正式包边界.md` 新增“闭环运行契约”、Mermaid 状态图、状态策略表与恢复说明）
- [x] 运行 `git diff --check`；涉及 Rust/PowerShell 时运行相应 parser、测试、`cargo fmt` 和 `cargo check`。（验证：工具 fmt/check、完整测试、边界检查和 `git diff --check` 已由主审核独立复跑通过）

## 阶段 2：隔离工作区、文件快照和并发锁

- 开始时间：2026-07-19 14:20:53 +08:00
- 结束时间：2026-07-19 15:15:07 +08:00
- 开发总结：新增 `workspace` 隔离层，支持 draft staging 与 detached Git worktree、来源提交/dirty 快照、允许根解析、迭代 hash/diff、持久化 lease lock、过期回收和取消保留。主审核补强 lease identity、刷新与跨进程回收 guard，防止活跃或同 run ID 的新锁被过期 handle 误删。
- 验证记录：主审核打回 1 轮修复 TTL 活跃锁被回收、旧 handle 删除同 run 新锁和 stale reclaimer TOCTOU 风险。独立运行 workspace 9/9、工具全量 162 项、fmt/check、boundary、`git diff --check` 和 `run-ui-audit.ps1 -SelfTest`；Runner strict comparison 3/3 passed，耗时 138 秒。

- [x] 为只生成草稿的 run 使用独立 staging 目录，为允许改代码的 run 使用专用 Git worktree 或等价隔离机制。（验证：`workspace.rs` 的 `DraftStaging`/`CodeWorktree` 创建 no-clobber staging 或 detached `git worktree add`）
- [x] 启动前记录源提交、工作树状态和允许修改根，不把用户未提交改动复制为隐式输入。（验证：`SourceWorktreeSnapshot` 记录 HEAD 与 porcelain hash；dirty worktree/worktree 提交测试通过）
- [x] 禁止在用户当前 dirty worktree 上执行 reset、checkout 覆盖、clean 或递归删除。（验证：Git 调用仅限只读 inspection 和新路径 `worktree add`；无 cleanup API，取消保留 workspace）
- [x] 每轮修改前后生成文件 hash、状态快照和统一 diff，并保留新建、修改、删除分类。（验证：`WorkspaceTreeSnapshot`/`WorkspaceFileDiff` 与 created/modified/deleted 定向测试通过）
- [x] 对同一目标页面、正式资源或 worktree 建立并发锁，超时后明确失败而不是并发覆盖。（验证：lease ID、`refresh_locks` 和跨进程 guard 防止重回收；并发、刷新、旧 lease 与 stale reclaimer 9 项测试通过）
- [x] 所有输出路径解析后必须位于本轮 run root、专用 worktree 或明确批准的晋升目标。（验证：allowed modification roots、canonical containment 和 reparse 拒绝测试通过）
- [x] 定义 run 取消、进程崩溃和机器重启后的锁回收与临时目录保留策略。（验证：workspace 不自动删除，取消仅释放自有 lease；过期 lease 在 guard 下回收，文档说明长调用续约）
- [x] 为 dirty worktree、路径穿越、符号链接、并发冲突和中断恢复补充测试。（验证：workspace 模块 9/9 覆盖 dirty、escape/symlink、并发 timeout、stale recovery、lease refresh 与 old lease drop）
- [x] 运行 Runner self-test 和文件安全策略定向测试。（验证：`cargo test --manifest-path tools/ui-generation/Cargo.toml workspace -- --nocapture` 9/9，`run-ui-audit.ps1 -SelfTest` strict comparison 3/3 passed）

## 阶段 3：真实生成器接入和草稿装载

- 开始时间：2026-07-19 15:16:12 +08:00
- 结束时间：2026-07-19 16:20:52 +08:00
- 开发总结：新增 `closed-loop-generate` 与 Runner `GenerationMode`，以 Rust 工具封装 Off/Fixture/Plan/Provider，持久化生成证据并通过 standalone 声明式预览产生临时审计映射。Provider 无适配器时 fail-closed，默认 Off 无副作用；修复 Windows preview 子进程树收尾与 Fixture 成功 manifest 落盘。
- 验证记录：主审核打回 1 轮修复 Fixture preview 完成后命令不退出和 manifest 未落盘；工具 169 项、Runner SelfTest、AST、fmt/check/boundary 通过。真实 Fixture smoke 60.5 秒 exit 0，protocol v2 manifest 为 `auditing`，生成、验证、source map、资源和 preview links 完整。

- [x] 为 Runner 增加 GenerationMode：Off、Fixture、Plan 和 Provider，默认 Off。（验证：`closed-loop-generate` 与 `run-ui-audit.ps1` 参数；Off 无副作用测试通过）
- [x] Provider 模式调用 `AI参考图生成UI` 的稳定接口，不在 PowerShell 中复制 prompt 或解析模型细节。（验证：PowerShell 仅调用 Rust CLI；未批准 adapter/缺凭据 fail-closed 测试通过）
- [x] 将生成结果、provider metadata、validation report、source map 和草稿素材写入 run manifest。（验证：protocol v2 artifact links 和 Fixture smoke manifest 完整）
- [x] 只有生成与语义验证通过后才进入 Bevy 预览，失败时保留完整草稿和诊断。（验证：preview timeout 写 terminal Failed manifest；资源缺失测试通过）
- [x] 通过声明式运行时加载草稿页面，并自动注册本轮临时 screen、device 和 state 审计映射。（验证：standalone runtime registration 输出 generated draft/device/states）
- [x] 处理 provider 超时、凭据缺失、缓存命中、用户取消和 schema 不兼容。（验证：Provider/preview failure taxonomy、现有 cancellation/cache/schema contracts 与定向测试通过）
- [x] 对 Fixture 成功、非法输出、资源缺失、超预算和 Provider 不可用补充端到端测试。（验证：closed-loop generation 定向测试及工具 169 项通过）
- [x] 确保普通 `run-ui-audit.ps1` 未启用 GenerationMode 时行为不变。（验证：Off CLI 与 Runner SelfTest 通过）
- [x] 运行 Runner self-test、`git diff --check`、相关测试、`cargo fmt` 和 `cargo check`。（验证：SelfTest 135.8 秒、fmt/check/boundary/diff 和工具 169 项通过）

## 阶段 4：真实视觉审核接入和问题归属

- 开始时间：2026-07-19 16:21:44 +08:00
- 结束时间：2026-07-19 18:35:18 +08:00
- 开发总结：在 `run-ui-audit.ps1` 增加独立于严格 comparison bundle 的闭环问题报告：从语义、区域、gate 和 AI 审核 artifact 归一化出可追溯 issue，并以 source map 绑定声明式文档。报告区分 hard/visual/AI 优先级，按根因跨 device/state 归并；reference、baseline、mask、threshold 等受保护路径强制人工复核，未知 node 和缺失证据 fail-closed。
- 验证记录：主审核独立运行 `./scripts/run-ui-audit.ps1 -SelfTest` 通过（154 秒，含三组严格 reference capture、真实 visual failure、semantic finding、Fixture AI issue 和既有修复失败路径）；`cargo test --manifest-path tools/ui-visual-audit/Cargo.toml --test cli_contract --test regions_contract --test gate_cli_contract` 为 20/20 passed；PowerShell parser 与 `git diff --check` 通过。

- [x] 将 reference compare、语义审核和真实 AI analyzer 接入现有 analysis/gating 阶段。（验证：`New-UiAuditClosedLoopAuditReport` 读取 semantic/region/gate/AI reports，`Complete-UiAuditReferenceComparison` 写入 `closed-loop-audit.json`、manifest 和 artifact link；严格 self-test 覆盖真实 comparison 产物）
- [x] 每个 issue 必须关联 screen、device、state、region、evidence 和可选 document/node/source path。（验证：`New-UiAuditClosedLoopIssue` 强制 capture、region、artifact SHA-256 和描述；`Resolve-UiAuditClosedLoopDocument` 绑定 source map，缺失证据自测拒绝）
- [x] 按问题归属分类为 document_layout、document_style、draft_asset、business_content、common_widget、theme、framework、reference_or_rule。（验证：`ConvertTo-UiAuditClosedLoopAttribution` 实现八类归属，self-test 覆盖八类路径、AI typography/color/imagery 和生成草稿 JSON）
- [x] `reference_or_rule` 问题只能进入人工复核，禁止自动修改 reference、mask 或阈值。（验证：受保护路径优先覆盖不可信建议；issue 标记 `requires_manual_review`、`automatic_fix_allowed = false` 和四类 `protected_targets`，self-test 通过）
- [x] 同一根因在多个设备和 state 出现时归并为一个问题组，同时保留所有证据。（验证：`Group-UiAuditClosedLoopIssues` 使用稳定根因 hash 分组，跨 phone/tablet fixture 保留两条 capture 和 evidence）
- [x] 硬性语义失败、确定性视觉失败和 AI 建议分别记录，保持各自优先级。（验证：报告输出 `hard_issues`、`visual_issues`、`ai_issues` 和 `priority_order`；真实 comparison/semantic/Fixture AI self-test 断言三类各一条）
- [x] 为错误归属、跨设备归并、未知节点和证据缺失补充测试。（验证：Runner self-test 覆盖八类归属、受保护路径、跨设备归并、未知 node、缺失及畸形 SHA evidence 拒绝）
- [x] 验证现有只使用 analysis fixture 的 Runner 模式继续可用。（验证：独立 `run-ui-audit.ps1 -SelfTest` 通过，原有 Mock FixMode 成功、最大迭代、验证失败与 allowlist 拒绝路径均通过）
- [x] 运行 Runner self-test、比较 fixture 和至少一个真实 reference audit。（验证：主审核独立 self-test exit 0，三组 strict reference capture 全部通过；ui-visual-audit CLI/region/gate 合约测试 20/20 passed）

## 阶段 5：受限修复计划生成

- 开始时间：2026-07-19 18:36:33 +08:00
- 结束时间：2026-07-19 19:10:58 +08:00
- 开发总结：在 `tools/ui-generation` 新增 Stage 4 audit 到受限 fix plan 的严格协议和 `closed-loop-plan` CLI。计划只输出同源 JSON/Markdown，不应用 patch；默认只建议 draft/asset 目标，按实际 issue capture 生成复跑矩阵。共享组件、theme 和 framework 仅在真实多页面证据、显式 protocol limitation 和人工审批下进入 `awaiting_approval`。主审核补强伪造 capture 拒绝与 symlink/reparse 输出路径拒绝。
- 验证记录：主审核独立运行 `cargo test --manifest-path tools/ui-generation/Cargo.toml closed_loop_fix_plan -- --nocapture` 为 9/9 passed、`cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .`、`cargo fmt --manifest-path tools/ui-generation/Cargo.toml -- --check`、`cargo check --manifest-path tools/ui-generation/Cargo.toml` 和 `git diff --check` 通过；worker 完整工具测试为 178/178 passed，CLI fixture smoke 生成 no-clobber 的 JSON/Markdown。

- [x] 根据 issue group 生成结构化 fix plan，列出目标文件、document path、node ID、修改类型、预期效果和验证矩阵。（验证：`closed_loop_fix_plan.rs` 的 `ClosedLoopFixPlan`/`FixPlanAction` 输出 typed JSON 与同源 Markdown，action 包含 target、node、field path、effect 和可信 capture matrix）
- [x] 优先生成对 `UiDocument`、页面 scoped token 和草稿素材的修改，不直接修改生成器 prompt 或框架核心。（验证：默认 policy 仅允许 `draft/`、`assets/`；document/layout/style 和 draft asset 分别映射受限 modification kind，prompt 与 Runner 路径拒绝）
- [x] 只有协议无法表达且问题在多个页面复现时，才建议 common widget、theme 或 framework 变更。（验证：共享范围 action 必须由关联 issue 的可信 capture 推导两个 screen，并要求 `--protocol-limitation <group>`；伪造第二 screen 回归 fail-closed）
- [x] 业务文案、路由、数据绑定和动作缺失不得由视觉修复器自行猜测。（验证：`BusinessContent` 统一产生 `BusinessContentRequiresHumanReview` rejection，无 action）
- [x] fix plan 必须通过允许根、禁止路径、最大文件数、最大 diff、资源大小和依赖变更策略检查。（验证：policy 限制 roots/files/diff/assets，reference/baseline/mask/threshold/credential/prompt/Runner/Git/Cargo 目标拒绝；unsafe/budget/dependency 定向测试通过）
- [x] 检测互相冲突的修复、重复无效修复和可能降低其他 device/state 的修改。（验证：按 target/node/modification 检测 `ConflictingRepair`/`DuplicateIneffectiveRepair`，每 action 标记 regression guard 并保留关联 capture matrix）
- [x] 对高风险修复设置 `requires_approval`，没有批准时保持 AwaitingApproval。（验证：common_widget/theme/framework action 强制 high risk 和 `requires_approval`，计划 status 为 `awaiting_approval`；无 protocol limitation 时拒绝）
- [x] 为各问题归属、升级条件、安全拒绝和无可用修复补充 fixture 测试。（验证：fixture 覆盖八种归属结果、manual/protected、未确认/多页升级、缺失 byte length、预算/依赖/重复、伪造 capture 与 symlink output；focused tests 9/9 通过）
- [x] 输出可供人阅读和机器执行的同源 fix plan。（验证：`closed-loop-plan` CLI 以 create-new 写 `fix-plan.json` 和由同一 `ClosedLoopFixPlan` 渲染的 `fix-plan.md`，写入重复输出被拒绝）

## 阶段 6：草稿修复、代码升级和晋升审批

- 开始时间：2026-07-19 19:12:23 +08:00
- 结束时间：2026-07-19 19:12:23 +08:00
- 开发总结：新增计划绑定的结构化草稿 patch、版本化资源、审批绑定 Rust diff、完整 preview 和 fail-closed apply CLI；不自动晋升、commit 或 push。
- 验证记录：主审核 `closed_loop_apply` 5/5、`cargo check` 通过；worker 工具全测 183/183、fmt/boundary/diff/CLI smoke 通过。

- [x] 实现 `UiDocument` 的结构化 patch，按 node ID 和字段路径修改，禁止用不受控文本替换 JSON。（验证：解析 JSON 后验证 node/path/字段并 canonicalize）
- [x] 草稿素材修改必须生成新文件或新版本，保留旧 hash、来源和回滚映射。（验证：hash-version 文件与 rollback/provenance record，覆盖拒绝）
- [x] 对经批准的 Rust 修改使用统一 diff/patch，并在应用后重新检查实际改动是否超出 fix plan。（验证：plan/preview/approval digest 绑定，单文件 unified diff 与 post-write snapshot 重检）
- [x] 禁止修改 reference、baseline、mask、阈值、安全策略、Runner 检查命令、Git 配置和凭据文件。（验证：protected path、reparse/symlink 和计划外目标 fail-closed）
- [x] 正式晋升前展示文档、资源、i18n、主题和页面注册的完整 diff，并要求显式批准。（验证：no-write preview 分类输出；apply 要求未过期 explicit approval）
- [x] 晋升检查目标冲突、schema version、资源许可证、Git LFS 和已有页面 owner。（验证：复用既有 promotion 审核；apply preflight 复核目标冲突和计划范围）
- [x] 本流程不自动执行 git commit 或 push；后续提交使用仓库既有 Git 流程。（验证：apply 模块与 CLI 无 Git 写操作）
- [x] 对 patch 冲突、越界修改、资源覆盖、批准过期和部分写入失败补充测试。（验证：`closed_loop_apply` 5/5）
- [x] 运行安全策略定向测试和 `git diff --check`。（验证：主审核测试与检查通过）

## 阶段 7：迭代控制、改善判定和回滚

- 开始时间：2026-07-22 14:29:22 +08:00
- 结束时间：2026-07-22 16:12:34 +08:00
- 开发总结：扩展 Runner 修复循环的分类预算、迭代 artifact 快照、改善判定和提前停止策略；Command 修复在失败、退化、取消时按允许根的 hash 备份条件回滚，不覆盖并发或既有用户改动。
- 验证记录：主审核独立运行 `./scripts/run-ui-audit.ps1 -SelfTest`（exit 0，627.1 秒，严格比较 3/3）、PowerShell parser 与 `git diff --check` 通过；SelfTest 覆盖退化、两轮停滞、振荡、分类预算、命令失败回滚和运行中取消。

- [x] 复用并扩展现有 MaxFixIterations，分别限制生成修复、文档修复、素材修复和代码修复次数。（验证：`run-ui-audit.ps1` 增加四个 `Max*FixIterations` 参数、分类预算记录和 `iteration_budget_exhausted`；SelfTest 断言 code 分类上限不计入 asset）
- [x] 每轮保留 before/after 文档、资源 hash、截图、比较结果、analysis、fix plan、diff 和验证日志。（验证：`Copy-UiAuditIterationSnapshot`、workspace snapshot/diff 和 iteration artifact links；SelfTest 断言 capture SHA-256、analysis/report、fix plan 与 check logs 可追溯）
- [x] 定义改善判定：hard failure 减少、关键区域指标改善且未引入新阻塞问题。（验证：`Test-UiAuditFixImprovement` 同时计算 hard failure、region metric、blocking count、logical root cause 与新阻塞问题）
- [x] 连续两轮问题签名相同、指标无改善、问题迁移到其他设备或预算耗尽时提前停止。（验证：fix loop 记录 `stagnant_rounds`，检测 regression/device migration/ABA oscillation/category budget；SelfTest 覆盖退化、停滞、振荡和预算耗尽）
- [x] 修复后出现编译失败、schema 失败、截图失败或严重回归时回滚本轮文件快照。（验证：Command 修复在 command/check/rerun regression 失败时调用 `Restore-UiAuditFixWorkspaceSnapshot`；SelfTest 断言 command failure rollback 为 `restored`）
- [x] 回滚不得覆盖 run 启动前不属于自动化的用户改动。（验证：restore 仅恢复当前 hash 仍等于本轮 after snapshot 的路径；SelfTest 断言预存 user 文件内容保持不变）
- [x] 支持用户取消后安全终止当前外部调用并保留最后完整 iteration。（验证：`FixCancellationFile` 驱动 `Invoke-UiAuditProcess` 取消；SelfTest 启动外部 Command 后写取消文件，断言 terminated、rollback restored 与 `last_complete_iteration = 0`）
- [x] 为改善、退化、振荡、最大次数、验证失败和取消补充状态机测试。（验证：Runner SelfTest 覆盖 Pass、MaxIterations、CheckFailed、Degraded、stagnation、Oscillation、分类预算、Command rollback 和 active cancellation）
- [x] 运行 FixMode Fixture/Mock 的完整正向与失败演练。（验证：`./scripts/run-ui-audit.ps1 -SelfTest` exit 0，627.1 秒；strict comparison captures 3/3 passed）

## 阶段 8：分层验证和复跑矩阵

- 开始时间：2026-07-22 16:14:16 +08:00
- 结束时间：2026-07-22 17:25:38 +08:00
- 开发总结：Runner 按 UiDocument、资源、Rust 和 PowerShell 改动选择最小充分验证集，持久化命令级证据与失败类别；修复复跑固定为原失败 capture、关联 screen/device/state、共享组件矩阵三阶段，并对共享 UI 变更扩展至 UI Gallery 和全部注册页面。
- 验证记录：主审核独立运行 `pwsh -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-ui-audit.ps1 -SelfTest`（exit 0，674 秒，strict comparison 3/3）、PowerShell parser 与 `git diff --check` 均通过；资源 LFS 属性断言、验证计划、失败分类与复跑顺序均由 Runner SelfTest 覆盖。

- [x] 仅修改 `UiDocument` 时运行 schema、语义、资源和声明式运行时测试，不无条件触发全量 Rust 编译。（验证：`New-UiAuditValidationPlan` 为 document-only 仅选择 `document_schema_semantic` 与 `document_declarative_runtime`；SelfTest 断言不含 `cargo-check`）
- [x] 修改 Rust 时在 `project/` 运行 `cargo fmt`、相关 focused tests 和 `cargo check`。（验证：Rust scope 选择 `cargo-fmt`、`cargo-focused-tests`、`cargo-check`；Runner SelfTest 断言三项顺序）
- [x] 修改 PowerShell Runner 时运行 parser 检查、self-test 和 `git diff --check`。（验证：Runner scope 选择 `runner_parser`、`runner_self_test`、`runner_diff_check`，优先调用 `pwsh`；独立 parser/self-test/diff 检查通过）
- [x] 修改资源时校验格式、尺寸、透明通道、许可证、Git LFS 和 Android 加载兼容性。（验证：resource scope 选择 format/dimension/alpha、license/Android、Git LFS commands，并对每个二进制资源实际执行 `git check-attr filter=lfs`；SelfTest 覆盖 fixture）
- [x] 修复后先复跑原失败 capture，再复跑该 screen 的全部关联 device/state，最后运行受影响共享组件页面。（验证：`rerun_order` 固化 original capture、affected matrix、shared matrix；Mock rerun manifest 断言 phase 顺序）
- [x] theme、widget 或 framework 变更必须复跑 UI Gallery 和所有注册使用者的基础矩阵。（验证：`Resolve-UiAuditFixRerunMatrix` 检测 shared UI 路径并追加 `KnownScreens`，SelfTest 断言包含 `ui_gallery` 和全部注册页面）
- [x] 明确区分工具失败、环境失败、产品失败和审核失败，环境失败不得自动解释为视觉退化。（验证：`Get-UiAuditFailureClass` 分类并写入 command/task/analysis 记录；SelfTest 覆盖 Runner、device offline、Rust validation 和 AI audit issue）
- [x] 将每项验证命令、耗时、退出码和日志路径写入 iteration manifest。（验证：`Invoke-UiAuditFixChecks` 写入 command、coverage、duration_ms、exit_code、timeout、stdout/stderr 和 failure_class；SelfTest 断言证据齐全）
- [x] 为验证选择、失败分类和矩阵扩展补充测试。（验证：Runner SelfTest 覆盖 document/resource/Rust/Runner 选择、LFS、命令证据、失败分类、shared rerun 和实际 Mock rerun）

## 阶段 9：缓存、队列、预算和可观测性

- 开始时间：2026-07-22 17:32:02 +08:00
- 结束时间：2026-07-22 18:49:38 +08:00
- 开发总结：在 `tools/ui-generation` 增加运行治理协议：五类 cache identity、共享 provider governor、有界队列、单 run/每日原子预算、遥测脱敏、marker 保护的 artifact 保留清理和离线压力演练。ProviderRunner 通过显式注入共享 governor 接入真实执行路径，默认未注入时保持既有离线兼容行为。
- 验证记录：主审核独立运行 `cargo test --manifest-path tools/ui-generation/Cargo.toml`（200/200）、`cargo fmt --manifest-path tools/ui-generation/Cargo.toml --all -- --check`、`cargo check --manifest-path tools/ui-generation/Cargo.toml --no-default-features --features provider-core`、boundary check、`git diff --check` 与 `operations-stress-fixture` 均通过；仅有既有 Windows linker LNK4075 警告。

- [x] 为预处理、视觉分析、UiDocument 生成、截图和比较分别定义 cache key 与失效条件。（验证：`operations.rs` 的 `CacheStage`/`StageCacheKey` 覆盖五类阶段并输出稳定 digest，cache reuse 定向测试通过）
- [x] 缓存不得跨 schema、prompt、模型、主题、字体、viewport、算法或输入 hash 误复用。（验证：`CacheDimensions` 强制九类维度且 invalid identity fail-closed；逐维扰动测试及 `AnalysisCacheIdentity` theme miss 测试通过）
- [x] 建立有界任务队列和 provider 并发限制，避免多个 run 同时耗尽显存、API 配额或磁盘。（验证：`ProviderRuntimeGovernor` 显式共享 `BoundedTaskQueue` 并接入真实 `ProviderRunner`；18 项 runner 定向测试覆盖并发拒绝、取消/失败释放和零计数 provider 清理）
- [x] 设置单 run 和每日模型调用、图片数量、token、耗时、迭代和估算费用上限。（验证：`TaskBudget` 与 `DailyBudget` 使用可回滚的 attempt reservation，真实 ProviderRunner 测试断言 queue/daily/local 前置拒绝不残留记账，snapshot 恢复仍生效）
- [x] 记录各阶段耗时、缓存命中、重试、调用量、artifact 大小、节点数和最终状态。（验证：`RunTelemetry`/`StageTelemetry` 汇总上述字段并绑定 run correlation，operations 定向测试通过）
- [x] 日志使用 run ID、iteration 和 task ID 关联，并对凭据、账号文字和个人信息脱敏。（验证：`RunCorrelation`/`RedactedLogEvent` 复用 structured redact，operations 与 observability 测试覆盖 credential/account/PII/model content）
- [x] 定义 artifact 保留期限、失败 run 保留策略和受控清理命令，禁止无校验递归删除未知目录。（验证：`ArtifactCleaner` 要求新建 marker root、root digest plan、safe run ID 和 reparse 检查；CLI 提供 initialize/cleanup dry-run，清理定向测试通过）
- [x] 对缓存污染、预算耗尽、队列取消、磁盘不足和日志脱敏补充测试。（验证：operations 7 项测试与 ProviderRunner 18 项真实执行路径测试覆盖 cache、atomic budget、queue cancellation、disk reserve、redaction、timeout 和 snapshot restore）
- [x] 输出一次多任务压力演练记录。（验证：`operations-stress-fixture` exit 0：4 tasks、provider limit/peak 2、cancelled 1、cache isolation、daily budget、disk reserve、redaction 和真实 FixtureProvider daily interception 全部通过）

## 阶段 10：CI、安全和权限门禁

- 开始时间：2026-07-23 10:42:05 +08:00
- 结束时间：2026-07-23 10:55:00 +08:00
- 开发总结：新增离线 CI security contract、受保护 online contract workflow、baseline 审批、供应链/许可证与脱敏 artifact 门禁；online 仍为 contract_only，未接入 provider 或远程设备。
- 验证记录：主 agent 复跑三个 PowerShell SelfTest、`check-ci-security-contract`、`ci-security-fixture` 与 `git diff --check` 全部通过；完整 Runner self-test 未重跑，Runner 核心未修改。

- [x] 定义五种运行模式。（验证：`ci-security-fixture` 输出 local/PR fixture/PR deterministic/manual online/scheduled online 五模式）
- [x] PR 与不受信分支不读 secrets 或远程设备。（验证：workflow `persist-credentials: false`，fixture 覆盖拒绝路径）
- [x] 在线任务限制权限、域名和超时。（验证：online workflow 为 protected `contract_only`，fixture 拒绝未批准 provider）
- [x] 检查许可证和供应链。（验证：`test-ui-supply-chain.ps1 -SelfTest` 通过）
- [x] baseline/reference 变更需要审批标签。（验证：`test-ui-reference-baseline-approval.ps1 -SelfTest` 通过）
- [x] 禁止自动提交、push、发布和分支保护修改。（验证：CI contract fixture 拒绝该 capability）
- [x] 下载 artifact 已脱敏。（验证：`write-ui-ci-failure-report.ps1 -SelfTest` 通过）
- [x] 覆盖无 secret、无权限、外部分支、基准和 provider 拒绝。（验证：`ci-security-fixture` 输出六类 rejected scenarios）
- [x] 记录 CI 超时、缓存和 artifact 配额。（验证：contract 输出 offline/online timeout 与 cache/artifact byte limits）

## 阶段 11：桌面与 Android 端到端验收

- 开始时间：2026-07-23 11:34:05 +08:00
- 结束时间：
- 开发总结：离线桌面验收通过，真实 Android 仍 external_blocked，待 validated remote screenshot/system metadata contract 与设备授权后继续。
- 验证记录：`summary/ui-generation/stage11-e2e-final-offline-20260723c-report/acceptance-report.json` 为 `passed_with_external_android_blocker`；工具全量测试、fmt/check/boundary/parser/diff 通过。

- [x] 选取至少一个常规页面和一个复杂美术页面，从参考图完整运行生成、预览、审核、修复和通过流程。（验证：最终报告 regular/complex sealed bundle 均通过）
- [x] 桌面矩阵至少覆盖 `phone-small`、`phone-portrait`、`tablet-portrait` 和 `tablet-landscape`。（验证：四 profile audit 与成功 desktop manifest 4/4 tasks）
- [x] 验证多个 state、长列表滚动、Modal、Loading、字体加载和图片资源就绪。（验证：28 multi-state captures、desktop metadata 与 ui_gallery scroll states）
- [ ] 通过真实远程链路在 Android 设备执行至少一次截图与 metadata 审核；外部链路不可用时保留未完成并记录阻塞。
- [ ] Android 验收覆盖安全区、软键盘、触控、横竖屏、高 DPI、九宫格和材质降级。
- [x] 记录端到端耗时、模型成本、迭代次数、峰值内存、截图稳定性和视觉审核结果。（验证：acceptance report 记录 165256ms、离线 fixture、重复 SHA 与 audit 结果）
- [x] 演练 provider 超时、无网络、设备离线、编译失败、视觉退化和人工拒绝晋升。（验证：report commands 包含 provider/preview timeout、取消与人工拒绝；Android device 作为 external blocker 记录）
- [x] 确认失败 run 可以恢复或回滚，且用户原工作树没有被修改。（验证：首次 desktop fixture 参数失败后 b run 成功，report 声明 caller_worktree_unchanged）
- [x] 生成最终端到端验收报告并清理不需要的临时产物。（验证：acceptance-report.json/md 已生成，Temp input finally cleanup；保留 ignored 日志证据）

## 阶段 12：文档、运维手册和整体交付

- 开始时间：2026-07-23 14:06:25 +08:00
- 结束时间：2026-07-23 14:52:05 +08:00
- 开发总结：补充 UI 参考图生成闭环的本地运行、交付验证、权限边界、失败定位、版本升级和 artifact 保留手册，并在 Bevy 入门文档增加最小 Fixture/独立预览入口。修复 Windows checkout 下 golden JSON 行尾比较，以及无效 Atlas/NineSlice 组合在返回错误前请求资源加载的问题。
- 验证记录：`tools/ui-generation` 205 tests、fmt/check、boundary、CI security contract/fixture 均通过；三个 PowerShell 安全 self-test、正式供应链与当前变更 reference/baseline 审批路径检查通过。最终离线验收 `summary/ui-generation/stage11-e2e-20260723-143428-acbaf11c-report/acceptance-report.json` 为 `passed_with_external_android_blocker`（2 generation runs、2 document audits、8 reference comparisons、Runner/worktree self-test 均 exit 0、caller worktree unchanged）。`project` fmt、1557 tests 和 cargo check 通过；git diff --check 通过。

- [x] 更新 `docs/ui/`，描述生成、声明式协议、视觉审核、修复、晋升和基准更新完整流程。
- [x] 更新 `docs/bevy-getting-started.md`，只加入新成员真正需要的本地 Fixture 和预览入口。
- [x] 记录 provider 配置、凭据来源、预算、缓存、artifact、日志脱敏和故障排查。
- [x] 记录哪些操作自动执行、哪些必须人工批准、哪些明确禁止。
- [x] 为常见失败类型提供定位顺序，不要求使用者阅读整个 Runner 源码。
- [x] 记录 Schema、prompt、算法、reference 和 baseline 的升级兼容策略。
- [x] 运行全部 Fixture/self-test、至少一个双设备 reference audit 和一个 FixMode 端到端演练。
- [x] 在 `project/` 运行 `cargo fmt`、相关测试和 `cargo check`，并运行 `git diff --check`。
- [x] 清点所有文档、fixture、脚本和正式资源路径，确认符合仓库约定。

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有阶段完成后统一验收。

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 合法参考图可以通过一次 run 生成可运行 UI 草稿、预览截图、视觉审核报告和可定位问题。
- [ ] 自动修复优先修改声明式草稿，并能在有限迭代内通过或以明确原因终止。
- [ ] 框架级或代码级修改必须经过升级条件和人工批准，实际 diff 不得超出计划范围。
- [ ] 自动化无法修改参考图、baseline、mask、阈值、安全策略或验证脚本来规避失败。
- [ ] 每轮输入、模型、文档、素材、截图、指标、analysis、diff、验证和审批完整可追溯。
- [ ] provider、网络、设备、编译或审核失败不会破坏用户工作树，且可以恢复或回滚。
- [ ] 普通本地开发和 PR Fixture 模式不需要在线模型凭据或付费调用。
- [ ] CI、成本、并发、缓存、日志脱敏、资源授权和 artifact 保留策略均已验证。
- [ ] 桌面多 profile 端到端流程通过；真实 Android 有验收记录或明确外部阻塞项。
- [ ] Runner self-test、比较测试、生成 Fixture、`cargo fmt`、相关测试、`cargo check` 和 `git diff --check` 全部通过。
- [ ] 文档足以让新开发者运行 Fixture 流程、理解审批边界并定位失败。
