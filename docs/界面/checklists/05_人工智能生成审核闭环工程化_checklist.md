# 05. AI 生成审核闭环工程化 清单

## 目标

将参考图输入、AI 生成、声明式 UI 预览、视觉审核、问题定位、受限修复、复跑和人工晋升串成一套可恢复、可追溯、可控制成本的端到端工程流程。该流程建立在现有 UI audit Runner 与修复循环骨架上，补齐真实生成器、真实审核器和安全执行边界。

默认修复目标是 staging 中的 `UiDocument` 和草稿素材。只有审核问题被明确归类为通用组件或框架能力缺口，并经过人工批准后，才允许修改 Rust、主题公共结构或正式资源。流程不自动提交、push 或更新参考基准。

## 已有基础与依赖

- 现有 `run-ui-audit.ps1` 已支持设备矩阵、analysis fixture、FixMode、迭代快照、`cargo fmt`/`cargo check` 和安全策略骨架。
- 依赖 `01_界面高保真视觉基础能力_checklist.md`、`02_界面声明式描述与运行时生成_checklist.md`、`03_人工智能参考图生成界面_checklist.md` 和 `04_界面参考图视觉审核_checklist.md` 的最终公共接口。
- 真实远程 Android 设备执行依赖外部 adminapi、game-server 和 client 调试链路；本仓库负责调用、artifact 和失败处理。
- 本清单是整体工程化与最终验收清单，不重复实现前置清单内部算法。

## 基础原则

- 复核完成时间：2026-07-28 19:44:37 +08:00
- 复核总结：复核既有闭环治理、隔离、修复策略、受保护路径、失败恢复和 default-off 离线合同；阶段 7-10 的独立验证与 Stage 11 离线验收均未调用在线 provider 或 AI 凭据。

- [x] 每次运行拥有不可变输入快照、唯一 run ID、显式状态机和完整 artifact 关联。（验证：`tools/ui-generation/src/run_manifest.rs` 的 `ClosedLoopRunManifest` 与 checkpoint/recovery 契约；既有 closed loop tests 和本轮 fixture run 均通过）
- [x] 自动化只在独立工作目录或专用 Git worktree 中修改文件，不破坏用户现有 dirty worktree。（验证：`scripts/run-ui-audit.ps1` 的 detached worktree/isolation path；Stage 11 report `caller_worktree_unchanged=true`）
- [x] 修复范围按 UI 文档、草稿资源、页面局部代码、通用控件、主题、框架核心逐级升级。（验证：Runner fix policy/allowed roots 与 `closed_loop_fix_plan` scope/approval guards 已由阶段 5-8 测试覆盖）
- [x] 参考图、审核阈值、mask、安全策略、验证脚本和基准图不属于自动修复允许范围。（验证：Runner protected target policy、CI baseline approval gate 与阶段 10 security fixture 均 fail-closed）
- [x] 任一外部调用、修改、验证和审批都有超时、失败出口、重试上限和可恢复记录。（验证：state policy max attempts、iteration budget、cancellation/recovery 和阶段 7 Runner SelfTest 通过）
- [x] 普通开发、构建和 CI 默认不调用付费模型，也不需要 AI 凭据。（验证：default Off/Fixture mode、CI five-mode contract 与 Stage 11 report 均为 offline-only 且 cost `[0,0]`）

## 阶段 1：端到端运行契约和状态机

- 开始时间：2026-07-19 13:49:27 +08:00
- 结束时间：2026-07-19 14:19:13 +08:00
- 开发总结：在 `tools/ui-generation` 建立独立于 sealed Stage 3 bundle 的 `ClosedLoopRunManifest` v1，覆盖闭环 artifact/provenance/budget、13 态策略、持久化 checkpoint、失败/取消、精确 cache 恢复与新 attempt。恢复计划以 checkpoint index/state/attempt 绑定，避免修复循环中重复状态误复用。同步补充总体流程文档、状态图和恢复边界说明；该闭环 Runner 尚未接入 `ui-generation` 在线生成 provider 或自动修复，`ui-visual-audit` 的在线 `analyze-ai` adapter 属于独立显式 opt-in 阶段。
- 验证记录：主审核打回 1 轮，修复重复 `Previewing` 等状态按枚举名恢复会误选首次 checkpoint 的问题；独立复跑 `cargo test --manifest-path tools/ui-generation/Cargo.toml` 为 153 passed，`cargo fmt --manifest-path tools/ui-generation/Cargo.toml --all -- --check`、`cargo check --manifest-path tools/ui-generation/Cargo.toml`、`cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .` 和 `git diff --check` 均通过。

- [x] 定义端到端 run manifest，关联 generation input、reference manifest、UiDocument、assets、preview、comparison、analysis、fix 和 approval。（验证：`run_manifest.rs` 的 `ClosedLoopRunManifest`/`ClosedLoopArtifactLinks` 绑定九类 artifact link，并拒绝不安全或重复路径；闭环生命周期测试通过）
- [x] 定义状态机：Created、Preparing、Generating、Validating、Previewing、Auditing、PlanningFix、ApplyingFix、Verifying、AwaitingApproval、Passed、Failed、Cancelled。（验证：`ClosedLoopRunState` 定义 13 个状态，`ClosedLoopRunState::policy` 固化允许来源与终态）
- [x] 为每个状态定义允许进入条件、持久化字段、超时、可重试性和终态。（验证：`ClosedLoopStatePolicy`、checkpoint 与文档状态表覆盖进入证据、cache key、attempt、时限、可重试性和终态；非法迁移测试通过）
- [x] 统一前置清单的 failure type，避免同一错误在生成器、审核器和 Runner 中使用不同名称。（验证：`TaskFailureKind` 新增 manifest/runner/audit/fix/approval 分类，`from_legacy_failure_type` 显式映射既有 audit `failure_type`，未知值保持未映射）
- [x] 记录工具版本、提交、模型、prompt、schema、算法、viewport、theme、locale 和预算配置。（验证：`ClosedLoopRunProvenance` 和 `ClosedLoopBudgetConfiguration` 要求全部字段非空且预算为正，损坏 manifest fail-closed）
- [x] 支持从最近一个完整状态恢复，不重复已成功且 cache key 未变化的外部调用。（验证：`ClosedLoopCheckpointIdentity` 以 index/state/attempt 区分循环状态，`recovery_plan`/`restart_from` 精确截断；重复 Previewing 回归测试验证最新 checkpoint 和单点 cache 失效）
- [x] 对非法状态跳转、manifest 损坏、版本不兼容和取消竞态补充测试。（验证：`closed_loop_manifest_rejects_illegal_state_transitions`、损坏/协议不兼容、持久化和 Passed 后取消测试均纳入工具 153 项测试）
- [x] 更新总体流程文档和状态图。（验证：`docs/界面/界面参考图生成与正式包边界.md` 新增“闭环运行契约”、Mermaid 状态图、状态策略表与恢复说明）
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

- 开始时间：2026-07-28 18:36:44 +08:00
- 结束时间：2026-07-28 18:45:38 +08:00
- 开发总结：核验既有 Runner 的阶段 7 实现：分类迭代预算、不可变 iteration artifact、改善/停滞/振荡判定、受 hash guard 的回滚与取消恢复均已实现；无需新增业务代码。
- 验证记录：主审核独立运行 `./scripts/run-ui-audit.ps1 -SelfTest` exit 0（128.7 秒，strict comparison captures 3/3）；PowerShell AST parser 与 `git diff --check` 通过，工作区无代码改动。

- [x] 复用并扩展现有 MaxFixIterations，分别限制生成修复、文档修复、素材修复和代码修复次数。（验证：`scripts/run-ui-audit.ps1:60` 定义四类 `Max*FixIterations`，`New/Test/Use-UiAuditFixIterationBudget` 在 `:2183` 记录分类额度，SelfTest 断言 code 上限不计入 asset）
- [x] 每轮保留 before/after 文档、资源 hash、截图、比较结果、analysis、fix plan、diff 和验证日志。（验证：`Copy-UiAuditIterationSnapshot` 与 iteration artifact links 保存 capture SHA-256、analysis/report、fix plan、workspace snapshot/diff 和检查日志；SelfTest 通过）
- [x] 定义改善判定：hard failure 减少、关键区域指标改善且未引入新阻塞问题。（验证：`Test-UiAuditFixImprovement` 位于 `scripts/run-ui-audit.ps1:2357`，比较 hard failure、关键区域、blocking issue 和新阻塞问题；SelfTest 通过）
- [x] 连续两轮问题签名相同、指标无改善、问题迁移到其他设备或预算耗尽时提前停止。（验证：Runner loop 使用 `Test-UiAuditFixOscillation`、停滞和 device migration 判定，分类预算耗尽返回 `iteration_budget_exhausted`；SelfTest 覆盖）
- [x] 修复后出现编译失败、schema 失败、截图失败或严重回归时回滚本轮文件快照。（验证：`Restore-UiAuditFixWorkspaceSnapshot` 位于 `scripts/run-ui-audit.ps1:2498`，Command 在命令、检查、取消和 regression 失败后恢复快照；SelfTest 通过）
- [x] 回滚不得覆盖 run 启动前不属于自动化的用户改动。（验证：恢复前要求当前 hash 与本轮 after hash 一致；SelfTest 断言用户文件保持 `user content before run`）
- [x] 支持用户取消后安全终止当前外部调用并保留最后完整 iteration。（验证：`FixCancellationFile` 参数与活动进程终止路径记录 `last_complete_iteration`；SelfTest 取消场景通过）
- [x] 为改善、退化、振荡、最大次数、验证失败和取消补充状态机测试。（验证：`scripts/run-ui-audit.ps1:7854` 之后的 SelfTest 覆盖 pass、max iterations、check failure、degraded、stagnation、oscillation、分类预算、rollback 和 cancellation）
- [x] 运行 FixMode Fixture/Mock 的完整正向与失败演练。（验证：主审核 `./scripts/run-ui-audit.ps1 -SelfTest` exit 0，strict comparison 3/3 passed）

## 阶段 8：分层验证和复跑矩阵

- 开始时间：2026-07-28 18:45:38 +08:00
- 结束时间：2026-07-28 19:08:30 +08:00
- 开发总结：核验并补齐分层验证实现；修正 catalog 对全不透明 RGBA PNG 的 alpha metadata 误拒绝，保持实际透明像素 fail-closed，新增回归测试并更新当前 catalog 资产计数。
- 验证记录：主审核独立运行 `cargo test --manifest-path tools/ui-generation/Cargo.toml --lib asset_strategy` 为 17/17；worker 运行工具全量 206/206、promotion 6/6、fmt/check/boundary、PowerShell AST、Runner SelfTest（124.2 秒，strict captures 3/3）和 `git diff --check` 均通过。

- [x] 仅修改 `UiDocument` 时运行 schema、语义、资源和声明式运行时测试，不无条件触发全量 Rust 编译。（验证：`scripts/run-ui-audit.ps1:3034` 按 modification scope 选择 document validation，验证计划在 `:3104` 生成；SelfTest 覆盖）
- [x] 修改 Rust 时在 `project/` 运行 `cargo fmt`、相关 focused tests 和 `cargo check`。（验证：Runner 在 Rust scope 调用 format/focused/check；本轮工具 `cargo fmt --check`、focused asset strategy 17/17 与 `cargo check` 通过）
- [x] 修改 PowerShell Runner 时运行 parser 检查、self-test 和 `git diff --check`。（验证：Runner scope contract 与 PowerShell AST、`run-ui-audit.ps1 -SelfTest`、`git diff --check` 通过）
- [x] 修改资源时校验格式、尺寸、透明通道、许可证、Git LFS 和 Android 加载兼容性。（验证：`asset_strategy.rs:541` 以实际像素 alpha 校验 opaque，新增 opaque RGBA/transparent RGBA 回归；catalog validation 同时覆盖规格、许可、LFS 和 Android 质量检查）
- [x] 修复后先复跑原失败 capture，再复跑该 screen 的全部关联 device/state，最后运行受影响共享组件页面。（验证：`New-UiAuditFixRerunPlan` 在 `scripts/run-ui-audit.ps1:2044` 定义原 capture、关联矩阵和共享页面阶段，SelfTest 覆盖）
- [x] theme、widget 或 framework 变更必须复跑 UI Gallery 和所有注册使用者的基础矩阵。（验证：Runner 在 `scripts/run-ui-audit.ps1:2087` 扩展 shared UI rerun；SelfTest 验证矩阵扩展）
- [x] 明确区分工具失败、环境失败、产品失败和审核失败，环境失败不得自动解释为视觉退化。（验证：`scripts/run-ui-audit.ps1:2995` 写入 failure class，SelfTest 覆盖 product/environment/audit 路径）
- [x] 将每项验证命令、耗时、退出码和日志路径写入 iteration manifest。（验证：`scripts/run-ui-audit.ps1:3152` 持久化 command evidence，SelfTest 断言 iteration log/artifact links）
- [x] 为验证选择、失败分类和矩阵扩展补充测试。（验证：Runner SelfTest 及工具全量 206/206 通过；本轮定向回归 17/17 通过）

## 阶段 9：缓存、队列、预算和可观测性

- 开始时间：2026-07-28 19:09:35 +08:00
- 结束时间：2026-07-28 19:17:37 +08:00
- 开发总结：核验既有闭环运行治理实现；五类 cache identity、受限队列/共享 provider governor、单 run 与日预算、可关联脱敏遥测及 marker-root artifact retention/cleanup 均已存在，无需新增代码。在线 provider 仍 fail-closed，document/screenshot/comparison 的缓存协议尚未由闭环 CLI 自动物化。
- 验证记录：主审核 `cargo test --manifest-path tools/ui-generation/Cargo.toml operations -- --nocapture` 为 7/7；`operations-stress-fixture` 通过（4 tasks、峰值 provider 2、取消 1、第二次 provider 调用被日预算阻止）；worker 工具全量 206/206、fmt/check/boundary 与 `git diff --check` 通过。

- [x] 为预处理、视觉分析、UiDocument 生成、截图和比较分别定义 cache key 与失效条件。（验证：`tools/ui-generation/src/operations.rs:31` 的 `CacheStage`、`CacheDimensions` 和 `StageCacheKey` 覆盖五阶段；operations tests 7/7 通过）
- [x] 缓存不得跨 schema、prompt、模型、主题、字体、viewport、算法或输入 hash 误复用。（验证：cache dimensions 包含上述 identity，`every_cache_stage_requires_all_reuse_dimensions_and_invalidates_each_one` 通过，压力 Fixture `cross_dimension_reuse_blocked=true`）
- [x] 建立有界任务队列和 provider 并发限制，避免多个 run 同时耗尽显存、API 配额或磁盘。（验证：`ProviderRuntimeGovernor` 位于 `operations.rs:806`；压力 Fixture `peak_running_tasks=2` 且 `provider_concurrency_limit=2`）
- [x] 设置单 run 和每日模型调用、图片数量、token、耗时、迭代和估算费用上限。（验证：`provider_budget.rs` 与 shared governor 持久化 run/daily budget；压力 Fixture 第二次 FixtureProvider 调用被日预算阻止）
- [x] 记录各阶段耗时、缓存命中、重试、调用量、artifact 大小、节点数和最终状态。（验证：`RunTelemetry`/`RunTelemetryReport` 位于 `operations.rs:992`，operations telemetry test 通过）
- [x] 日志使用 run ID、iteration 和 task ID 关联，并对凭据、账号文字和个人信息脱敏。（验证：`RedactedLogEvent` 位于 `operations.rs:1039`，遥测 test 与压力 Fixture `log_redacted=true` 通过）
- [x] 定义 artifact 保留期限、失败 run 保留策略和受控清理命令，禁止无校验递归删除未知目录。（验证：`ArtifactCleaner` 位于 `operations.rs:1212`，仅接受 canonical `ui-generation-artifacts` marker root；cleanup test 覆盖 unknown root、failure TTL 和 link escape）
- [x] 对缓存污染、预算耗尽、队列取消、磁盘不足和日志脱敏补充测试。（验证：operations 7/7 覆盖完整 cache dimensions、日预算、队列取消、磁盘拒绝、脱敏和 cleanup）
- [x] 输出一次多任务压力演练记录。（验证：主审核 `operations-stress-fixture` 输出 `submitted_tasks=4`、`cancelled_tasks=1`、`final_status=passed`）

## 阶段 10：CI、安全和权限门禁

- 开始时间：2026-07-28 19:17:37 +08:00
- 结束时间：2026-07-28 19:24:33 +08:00
- 开发总结：核验 CI 与安全门禁；五种运行模式、权限/凭据/外部分支拒绝、供应链、基准审批和白名单脱敏报告均已实现。手动和定时在线模式目前均为受保护的 `contract_only`，不读取凭据、上传参考图或调用 provider/远程设备。
- 验证记录：主审核 `ci_security` 3/3、`check-ci-security-contract` 和 `ci-security-fixture` 通过；合同输出 5 modes、offline 20m、online contract 15m、cache 512 MiB、artifact 32 MiB，并拒绝 6 种高风险场景。

- [x] 定义本地开发、PR Fixture、PR 确定性审核、手动在线生成和定时在线审核五种运行模式。（验证：`ui-ci-security-policy.v1.json` 与 `check-ci-security-contract` 输出五个 `validated_modes`）
- [x] 普通 PR 不读取在线 AI 凭据；外部贡献和不受信分支不得访问 secrets 或远程设备。（验证：`ui-visual-audit.yml` 最小 `contents: read`/`persist-credentials: false`，CI fixture 拒绝 ordinary PR credential 与 external branch device）
- [x] 在线任务使用最小权限凭据、受控网络目标和明确超时，不把用户参考图发送给未批准 provider。（验证：`ui-online-audit-contract.yml` 在 `ui-audit-online` environment 以 `contract_only` 运行、`permissions: {}`；fixture 拒绝缺 credential contract 和未批准 provider domain）
- [x] 对生成资源、模型输出、第三方依赖和 shader 执行许可证及供应链检查。（验证：`scripts/test-ui-supply-chain.ps1` SelfTest 与 repository check 通过）
- [x] 在 CI 中校验 reference/baseline 变更需要审批标签或等价人工门禁。（验证：`ui-reference-baseline-approved` 作为精确 approval label，`ci_security` baseline test 通过）
- [x] 禁止自动化提交、push、创建发布或修改分支保护；这些操作不属于 UI 闭环权限。（验证：CI contract 拒绝 `automatic_commit_push_release_or_branch_protection`，fixture 通过）
- [x] 失败报告必须可下载但不包含原始凭据、未脱敏账号数据或无授权参考图。（验证：`write-ui-ci-failure-report.ps1` 白名单脱敏 SelfTest 通过，fixture 拒绝包含 secret/account/reference 的报告）
- [x] 为无 secret、无权限、外部分支、基准变更和 provider 域名拒绝补充演练。（验证：`ci-security-fixture` 输出 six rejected scenarios，包括 missing credential、external branch、baseline label 与 provider domain）
- [x] 记录 CI 超时、缓存和 artifact 配额。（验证：主审核 contract 输出 offline 20m、online 15m、cache 536870912 bytes、artifact 33554432 bytes）

## 阶段 11：桌面与 Android 端到端验收

- 开始时间：2026-07-28 19:24:33 +08:00
- 结束时间：
- 开发总结：离线桌面端到端验收通过；真实 Android 的本机覆盖已完成，远程审核链仍 external_blocked。2026-07-29 已通过当前 Android SDK ADB 连接 API 36 真机，构建并替换安装 Debug APK、冷启动到登录页、确认前台窗口/进程、采集无损设备截图；人工实测已覆盖登录软键盘、横屏大厅和 Touch Ripple 的按下硬边圆、拖动水波纹拖尾与松开清理，以及 UI 示例的透明边缘、九宫格、图集和材质降级。设备仍拒绝自动化 `adb shell input` 的 `INJECT_EVENTS`，且没有经验证、获授权的 Remote Http 截图与 system metadata 合同。
- 验证记录：离线 `summary/ui-generation/stage11-e2e-20260728d-report/acceptance-report.json` 为 `passed_with_external_android_blocker`，12 个命令均 exit 0，耗时 731603ms，worktree unchanged；真机证据位于 `summary/ui-generation/stage11-android-adb-20260729/`：设备 `5aad1915`（API 36、1280x2772、520 dpi）、APK SHA-256 `46F35D77FC1C64AE303FFED2F6A18105221A8F331E6AD2873DFB71F9F283F768`、登录页截图 SHA-256 `ED43DCEC01A995D7B2888FA83EA5E6256F290792EA97D74404FD3A72D26A6F8E`、软键盘截图 `account-ime-manual.png`（`mInputShown=true`、`mImeWindowVis=3`）、横屏大厅截图 `lobby-landscape.png`（`ROTATION_90`、2772x1280）、横屏系统 inset（左侧 cutout 152 px、顶部 152 px、底部导航 52 px）、UI 示例视觉基础 `ui-gallery-visual-foundation.png`（SHA-256 `74C6B79B346EF703537371D0B1CDFB1D8CAF9AA4D7D130EE287E809D9BE650E6`，透明边缘、九宫格和图集均可见）、UI 示例材质降级 `ui-gallery-material-fallback.png`（SHA-256 `68797AF50F50880254C0BAFF6BAE109236D033AFCFAF9C0B08062FFB97585D52`，材质降级卡片及周边效果均可见）、触控按下态 `touch-ripple-pressed.png`（SHA-256 `D52D4737FDAD3CCDCFAA1DA4D4F34D371CF87B168DDE166B30466933D62CE2DC`）、拖动态 `touch-ripple-current.png`（SHA-256 `8F311F5DC013C90052F0FF64DA9ED14C2F6A05CC7FCB4054436065CAB7038719`）与释放清理态 `touch-ripple-released.png`（SHA-256 `7724D62A912DE6C1CCE8F52D67D155F185D1BB4C75926C0F2789C9B0B1504DC9`）；`MainActivity` 保持前台且无 fatal/panic。

- [x] 选取至少一个常规页面和一个复杂美术页面，从参考图完整运行生成、预览、审核、修复和通过流程。（验证：stage11 report 的 `generated_profiles` 为 regular/complex，两个 sealed fixture run、document audit 和 reference integrity 均通过）
- [x] 桌面矩阵至少覆盖 `phone-landscape`、`phone-1080p-landscape` 和 `tablet-landscape`；工具默认矩阵另含 `desktop`。（验证：stage11 report 的 desktop Runner 使用三个横屏设备且 `status=passed`、`failed=0`，standalone audit 默认设备包含四项）
- [x] 验证多个 state、长列表滚动、Modal、Loading、字体加载和图片资源就绪。（验证：stage11 report 的 multi-state audit 记录 28 captures，覆盖 initial/loading/empty/error/selected/disabled/modal 及 UI Gallery 滚动状态）
- [ ] 通过真实远程链路在 Android 设备执行至少一次截图与 metadata 审核；外部链路不可用时保留未完成并记录阻塞。（进展：真实设备截图、设备尺寸/density、前台窗口和进程均已由 ADB 验证；阻塞：Runner `-RequireRealAndroid` 仍要求经授权的 Remote Http/adminapi 截图与 system metadata 合同，当前未提供 base URL、token 或 client command endpoint。延期决定：2026-07-29 14:41:06 +08:00 用户确认本轮暂不建设或接入远端链路，等待后续前后端联调提供 `AdminApiBaseUrl`、token 及目标 device/client/session 后再执行）
- [x] Android 验收覆盖安全区、软键盘、触控、横竖屏、高 DPI、九宫格和材质降级。（验证：API 36、1280x2772、520 dpi；登录页 IME `mInputShown=true`；大厅 `ROTATION_90`；横屏 inset 左 152 px/顶 152 px/底 52 px；UI 示例 `ui-gallery-visual-foundation.png` 显示九宫格，`ui-gallery-material-fallback.png` 显示材质降级；Touch Ripple 按下/拖动/松开三态截图均已采集）
- [x] 记录端到端耗时、模型成本、迭代次数、峰值内存、截图稳定性和视觉审核结果。（验证：stage11 report 记录 731603ms、fixture cost `[0,0]`、repair/Runner iteration evidence、RepeatCaptures=2 和 visual audit 结果）
- [x] 演练 provider 超时、无网络、设备离线、编译失败、视觉退化和人工拒绝晋升。（验证：stage11 report 的 failure rehearsals 记录离线 provider/preview timeout、external device blocker、Runner check/regression fixture 与 promotion rejection test）
- [x] 确认失败 run 可以恢复或回滚，且用户原工作树没有被修改。（验证：stage11 report `caller_worktree_unchanged=true`；worktree isolation self-test 与 terminal failure manifest 测试均 exit 0）
- [x] 生成最终端到端验收报告并清理不需要的临时产物。（验证：当前 acceptance JSON/Markdown、命令日志和保留 evidence 已写入 ignored run root，temporary input 在 finally 清理）

## 阶段 12：文档、运维手册和整体交付

- 开始时间：2026-07-28 19:44:37 +08:00
- 结束时间：2026-07-28 19:44:37 +08:00
- 开发总结：核验归档阶段 12 与当前文档；本地 Fixture/预览入口、生成到晋升边界、provider/凭据/预算/缓存/artifact/脱敏、失败定位和兼容升级策略均持续记录，无需新增文档修改。
- 验证记录：主审核确认 `docs/界面/界面参考图生成与正式包边界.md` 的完整流程与 Android 外部边界、`docs/引擎入门使用文档.md:1060` 的 preview-document 入口及 `:1067` 的离线 E2E 入口；本轮 stage11 offline E2E exit 0，阶段 8-10 工具/Runner/CI 安全验证通过。

- [x] 更新 `docs/界面/`，描述生成、声明式协议、视觉审核、修复、晋升和基准更新完整流程。（验证：`docs/界面/界面参考图生成与正式包边界.md` 覆盖 input、生成、audit、fix、promotion 与 reference/baseline policy）
- [x] 更新 `docs/引擎入门使用文档.md`，只加入新成员真正需要的本地 Fixture 和预览入口。（验证：`docs/引擎入门使用文档.md:1060` 提供 `preview-document`，`:1067` 提供明确标记为离线的 Stage 11 E2E 入口）
- [x] 记录 provider 配置、凭据来源、预算、缓存、artifact、日志脱敏和故障排查。（验证：`界面参考图生成与正式包边界.md` 的权限、缓存、artifact retention 与 failure 定位章节）
- [x] 记录哪些操作自动执行、哪些必须人工批准、哪些明确禁止。（验证：同文档 automation/promotion/protected targets 表明确划分三类权限）
- [x] 为常见失败类型提供定位顺序，不要求使用者阅读整个 Runner 源码。（验证：同文档 failure troubleshooting table 和 `docs/界面/界面调试与验收.md` failure taxonomy）
- [x] 记录 Schema、prompt、算法、reference 和 baseline 的升级兼容策略。（验证：同文档 protocol/cache/baseline upgrade policy 要求 immutable run 与 approval binding）
- [x] 运行全部 Fixture/self-test、至少一个双设备 reference audit 和一个 FixMode 端到端演练。（验证：本轮 stage11 offline E2E、阶段 7 Runner SelfTest strict 3/3、阶段 8-10 fixture/security tests 均通过）
- [x] 在 `project/` 运行 `cargo fmt`、相关测试和 `cargo check`，并运行 `git diff --check`。（验证：归档阶段 12 的 project fmt、1557 tests、check 与 diff check 通过；本轮未改 project，工具 crate/Runner/CI 验证均重新通过）
- [x] 清点所有文档、fixture、脚本和正式资源路径，确认符合仓库约定。（验证：当前 docs、`tools/ui-generation/fixtures/`、`scripts/run-ui-e2e-acceptance.ps1` 和 approved acceptance document 路径均可解析）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有阶段完成后统一验收。

- 开始时间：2026-07-28 19:24:33 +08:00
- 结束时间：2026-07-29 14:12:03 +08:00
- 验收总结：离线 UI 生成、审核、有限修复、晋升控制、可追溯性、运维与 CI 安全合同均通过验收；桌面多 profile 与 Android 本机启动、IME、横屏、安全区、九宫格、材质降级和触控三态均有记录。真实 Remote Http/adminapi 截图与 metadata 合同仍依赖外部服务授权，作为明确 external_blocked 后续项保留在阶段 11，不以本机截图替代。

- [x] 合法参考图可以通过一次 run 生成可运行 UI 草稿、预览截图、视觉审核报告和可定位问题。（验证：Stage 11 regular/complex sealed fixture run、preview、document audit 和 acceptance report 均通过）
- [x] 自动修复优先修改声明式草稿，并能在有限迭代内通过或以明确原因终止。（验证：Stage 11 report 记录 FixMode/repair iteration，terminal failure manifest 与 promotion rejection 演练通过）
- [x] 框架级或代码级修改必须经过升级条件和人工批准，实际 diff 不得超出计划范围。（验证：Stage 4-6 promotion plan、protected target 和 closed approval adapter 已验收，`git diff --check` 通过）
- [x] 自动化无法修改参考图、baseline、mask、阈值、安全策略或验证脚本来规避失败。（验证：Stage 10 CI contract/fixture 拒绝 protected target、baseline label 和高权限工作流变更）
- [x] 每轮输入、模型、文档、素材、截图、指标、analysis、diff、验证和审批完整可追溯。（验证：Stage 9 operations report、run manifest/checkpoint、telemetry/redacted artifact 与 Stage 11 acceptance report）
- [x] provider、网络、设备、编译或审核失败不会破坏用户工作树，且可以恢复或回滚。（验证：Stage 11 `caller_worktree_unchanged=true`、worktree isolation self-test 和 terminal failure manifest 测试通过）
- [x] 普通本地开发和 PR Fixture 模式不需要在线模型凭据或付费调用。（验证：Stage 10 offline CI/Fixture contract 和 Stage 11 fixture cost `[0,0]`）
- [x] CI、成本、并发、缓存、日志脱敏、资源授权和 artifact 保留策略均已验证。（验证：Stage 8 asset/preflight、Stage 9 operations stress、Stage 10 security fixture 和 policy self-test 全部通过）
- [x] 桌面多 profile 端到端流程通过；真实 Android 有验收记录或明确外部阻塞项。（验证：Stage 11 four-profile desktop audit 通过；Android API 36 本机启动、IME、横屏、安全区、九宫格、材质降级和 Touch Ripple 三态已记录，Remote Http/adminapi 合同缺失已明确保留）
- [x] Runner self-test、比较测试、生成 Fixture、`cargo fmt`、相关测试、`cargo check` 和 `git diff --check` 全部通过。（验证：Stage 7 Runner SelfTest strict 3/3、Stage 8 `cargo test` 206/206、Stage 9/10 test/check/fixture 均通过，本次 `git diff --check` 通过）
- [x] 文档足以让新开发者运行 Fixture 流程、理解审批边界并定位失败。（验证：Stage 12 核验 `docs/界面/界面参考图生成与正式包边界.md`、`docs/界面/界面调试与验收.md` 和 `docs/引擎入门使用文档.md:1060`/`:1067`）
