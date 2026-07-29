# Cargo 构建目录空间优化 Checklist

## 目标

在不合并 `project`、`tools/ui-generation` 与 `tools/ui-visual-audit` 代码边界、不取消各自独立 `Cargo.toml` 和 `Cargo.lock` 的前提下，将三个 Cargo 根的构建产物收敛到仓库级共享 `target/`。在版本、feature、target triple、profile 和 rustc 参数一致时复用依赖产物，降低 Bevy、PDB 和增量缓存的重复硬盘占用。

本任务不要求立即改造成 workspace，不要求 UI 工具进入正式游戏包，也不把共享 target 等同于必然复用。`tools/ui-generation` 继续通过现有 `project::framework::ui::document::tooling` facade 使用 UiDocument 协议；`tools/ui-visual-audit` 继续只依赖 `ui-generation` 的 `provider-core` 特性。若共享 target 和 profile 对齐后空间收益仍不足，再单独评估轻量协议 crate。

## 当前基线

以下数据于 `2026-07-29` 在没有运行中的 Cargo、Rust、游戏或 Android 构建进程时复测；后续实施前如构建内容发生变化，重新记录同口径数据。

- 仓库总占用约 `37.64 GiB`，文件数 `20,621`；仓库根 `target/` 当前不存在。
- `project/target` 为 `21.59 GiB`，文件数 `9,198`；其中 `debug/deps` 为 `9.52 GiB`，`debug/incremental` 为 `9.39 GiB`。
- `tools/ui-generation/target` 为 `14.81 GiB`，文件数 `8,928`；其中 `debug/deps` 为 `10.27 GiB`，`debug/incremental` 为 `3.20 GiB`。
- `tools/ui-visual-audit/target` 为 `1.09 GiB`，文件数 `1,416`；其中 `debug/deps` 为 `0.61 GiB`，`debug/incremental` 为 `0.22 GiB`。
- `android/app/build` 与 `android/build` 当前不存在；Android 产物不能从本次目录快照推导，应在移动端验证阶段单独复测。
- 三个 Cargo 根当前分别输出到各自本地 target，仓库根、三个 Cargo 根下均不存在 `.cargo/config.toml`。
- `project` 与 `ui-generation` 的锁文件当前解析到相同的 Bevy `0.18.1`、Serde `1.0.228`、Tokio `1.52.3`；这只是复用前提之一，不能替代 profile 和 feature 验证。
- `project` 使用 `[profile.dev] opt-level = 1` 与依赖包 `opt-level = 3`；两个工具当前没有等价 profile。`ui-generation` 默认 `full` feature 会引入 `project`；`ui-visual-audit` 仅通过 `provider-core` 使用 UI 生成工具。
- `scripts/start-two-clients.ps1`、`scripts/start-robot-sync-two-clients.ps1`、`scripts/run-ui-audit.ps1`、UI 审计 README 和 fixture 当前引用旧 target 路径。正式文档与已归档验收记录也包含 `project/target`，迁移时必须区分活跃路径与历史证据。

## 基础原则

- [ ] 保持三个工具的代码、命令入口、锁文件和正式包边界独立；共享 target 不建立运行时依赖，也不把工具打入游戏包。
- [ ] 将代码所有权边界和本地构建缓存目录分开处理；只有版本、feature、target triple、profile、rustc 参数和编译目标一致的产物才视为可复用。
- [ ] 不修改或归档当前未提交的开发内容；实施前先确认工作区归属和提交边界。
- [ ] 根共享 `target/` 必须被 Git 忽略；不提交 PDB、rlib、APK、增量缓存、日志或本地配置。
- [ ] 每个阶段完成后执行对应验证并独立提交，避免把目录迁移、架构拆包和无关功能开发混在一起。
- [ ] 清理任意共享缓存前确认没有 `cargo`、`rustc`、游戏客户端、UI 工具或 Android 构建进程正在使用相关文件。
- [ ] 只迁移当前脚本、正式使用说明、fixture 和自动化测试；已归档 checklist 中的历史路径、截图和验证记录保持原样。
- [ ] 本 checklist 当前位于被忽略的 `summary/`；整体完成后按仓库约定归档到 `docs/<领域>/checklists/` 并随实现提交。

## 阶段 1：基线复测与复用条件确认

- 开始时间：2026-07-29 09:55:36 +08:00
- 结束时间：2026-07-29 10:00:55 +08:00
- 开发总结：已复核三个独立 Cargo 根、构建目录、依赖解析、profile、环境覆盖、脚本路径和 Android 输出边界；共享 target 的主要收益来自对齐后的 Bevy 等公共依赖，`project` 根包本身可能因作为 path dependency 时的优化级别不同保留独立产物。
- 验证记录：worker 只读采集；主 agent 复核工作区无可见 Git 改动且无相关进程。未运行构建或测试。

- [x] 在开发改动稳定且没有 Cargo 构建进程时，重新统计仓库、根 `target/`、三个旧 Cargo target 和 Android 构建目录的文件数与字节数。（验证：仓库 `40,411,497,438` bytes/`20,621` files；根 target 不存在；三个旧 target 分别为 `21.59 GiB`/`14.81 GiB`/`1.09 GiB`；Android build 目录不存在）
- [x] 记录三个 Cargo 根的 `cargo metadata --no-deps` 输出，确认迁移前 target 目录与迁移后预期的仓库根 target 目录。（验证：metadata 分别指向 `project/target`、`tools/ui-generation/target`、`tools/ui-visual-audit/target`；阶段 2 目标为仓库根 `target/`）
- [x] 记录三个锁文件中 Bevy、Serde、Tokio、Image 等公共依赖的解析版本与 feature 差异；区分 UI 审计工具不直接依赖 Bevy 的情况。（验证：project 与 ui-generation 均为 Bevy `0.18.1`、Image `0.25.10`、Serde `1.0.228`、Tokio `1.52.3`；ui-visual-audit 仅解析 Image/Serde）
- [x] 对比三个 Cargo 根的 dev profile、feature、target triple、`CARGO_TARGET_DIR`、`RUSTFLAGS`、本地 `.cargo` 覆盖和 CI 环境变量，列出阻止复用的参数差异。（验证：仅 project 配置 dev `opt-level=1` 和依赖 `opt-level=3`；两个工具无等价 profile；无 CARGO_TARGET_DIR/RUSTFLAGS/config/CI 覆盖；host 为 `x86_64-pc-windows-msvc`）
- [x] 记录主工程、两个工具、Android 命令、双客户端脚本和 UI audit runner 所期望的二进制或输出位置。（验证：双客户端脚本指向 `project/target/debug/project.exe`；audit runner 指向 `tools/ui-visual-audit/target/debug/ui-visual-audit.exe`；Android `cargo ndk -o` 输出 jniLibs）
- [x] 明确共享目录中的桌面 debug、桌面 release、Android arm64、UI 生成工具 debug 和 UI 审计工具 debug 子目录；不混淆不同 target triple 和 profile。（验证：预期桌面 debug/release 为 `target/debug`/`target/release`，Android 为 `target/aarch64-linux-android/release`）
- [x] 保存清理前基线、冷构建与增量构建的测量方法，用于阶段 5 重建后的体积与耗时对比。（验证：统一使用递归文件字节统计；清理后依次记录 `cargo check --locked`、UI generation 测试/boundary check、UI audit 测试的冷构建和未清理复跑耗时）

## 阶段 2：仓库级共享 target 配置

- 开始时间：2026-07-29 10:01:54 +08:00
- 结束时间：2026-07-29 10:10:39 +08:00
- 开发总结：已将三个独立 Cargo 根收敛到仓库根共享 target；未引入 workspace，保留独立 manifest/lock 和旧 target 忽略规则。阶段 2 建立的根缓存为阶段 5 的受控冷重建基准，旧缓存尚未删除。
- 验证记录：主 agent 独立复核四种 metadata 调用、根缓存忽略规则和 diff；worker 并发 `cargo check --locked` 均通过，仅有正常 Cargo 锁等待。

- [x] 新增仓库根 `.cargo/config.toml`，以相对该配置文件的路径将 `[build] target-dir` 设置为 `target`；文档中只写“仓库根 `target/`”，不硬编码盘符或绝对路径。（验证：`.cargo/config.toml` 定义 `[build] target-dir = "target"`）
- [x] 分别从 `project/`、`tools/ui-generation/`、`tools/ui-visual-audit/` 和仓库根使用 `--manifest-path` 查询 Cargo metadata，确认 `target_directory` 都解析为同一个仓库根 target 目录。（验证：四种调用均返回 `H:\project\mybevy\target`；三个 `workspace_root` 仍各自独立）
- [x] 在根 `.gitignore` 增加 `/target/`，并确认其覆盖 PDB、rlib、APK、增量缓存和生成日志；保留现有子工程 target 忽略规则，直到迁移和回滚窗口结束。（验证：`git check-ignore` 覆盖 PDB、rlib、incremental、Android `.so` 和日志；三条旧规则仍存在）
- [x] 保留三份 `Cargo.toml`、三份 `Cargo.lock` 和各自命令入口的独立性，不在本阶段引入 workspace。（验证：三个 metadata `workspace_root` 分别为各 Cargo 根，manifest/lock 均存在且无 `[workspace]`）
- [x] 明确 `CARGO_TARGET_DIR` 的优先级和本地/CI 使用约定，避免环境变量绕过仓库配置而重新生成分散 target。（验证：设为 `target/stage2-env-override` 时 metadata 改指该目录；约定本地/CI 默认不设置，仅隔离构建显式设置）
- [x] 验证并发运行不同 Cargo 根的构建只发生正常锁等待，不出现产物覆盖、错误启动或错误清理。（验证：并发 `cargo check --locked` 的 ui-visual-audit 24.91s、project 4m48s 均 exit 0；stderr 仅有 package cache/build directory 正常锁等待）

## 阶段 3：对齐可复用的开发 profile

- 开始时间：2026-07-29 10:11:36 +08:00
- 结束时间：2026-07-29 11:01:30 +08:00
- 开发总结：两个工具现与主工程保持根包 `opt-level=1`、普通依赖 `opt-level=3` 的开发 profile。针对 path dependency 使用精确覆盖：`project` 和 `ui-generation` 均保持 `opt-level=1`，避免被通配依赖规则误提升；UI 审计工具的 `provider-core` feature 与 UI 生成工具默认 `full` 不同，因此该 path crate 本身仍保留独立 fingerprint。
- 验证记录：首次完整 UI 生成工具测试触发共享 target 的 profile 重建；主 agent 在热缓存上独立复跑四项验证全部通过。未降低 debug 信息，编译的 project path dependency 保持 `-C debuginfo=2`。

- [x] 建立 profile fingerprint 对照表，分别覆盖主工程作为根包、`project` 作为 UI 生成工具 path dependency、Bevy 等公共依赖、UI 生成工具根包和 UI 审计工具根包。（验证：project/两个工具根包均为 `opt-level=1`；普通依赖为 `3`；project/ui-generation path dependency 由精确覆盖保持 `1`）
- [x] 在 UI 生成工具和 UI 审计工具 Cargo 根配置经过验证的 dev profile，使公共依赖与主工程使用一致的优化和调试参数；不要假定锁文件一致即代表产物可复用。（验证：两个工具 manifest 均新增 `[profile.dev] opt-level=1` 与 `[profile.dev.package."*"] opt-level=3`；四项锁定验证均通过）
- [x] 检查通配依赖 profile 对 `project` 作为 path dependency 的影响；必要时使用 package-specific override，避免 `project` 根包与依赖包意外采用不同的优化级别。（验证：ui-generation 对 `project` 配置 `opt-level=1`；编译命令确认 project 以 `-C opt-level=1 -C debuginfo=2` 构建）
- [x] 检查 UI 工具特有依赖在 profile 对齐后的编译耗时、调试体验和运行行为；记录预期不能复用的根包或 feature 组合。（验证：首次重建后 ui-generation 206 tests 通过；ui-visual-audit 对 ui-generation 使用 `provider-core`，与默认 `full` feature 不同，预期 path crate 产物不复用）
- [x] 评估 `debug = 1` 或 `debug = "line-tables-only"` 对 Windows PDB 大小和调试能力的影响；没有调试验收证据前不直接降低全仓调试信息。（验证：无 PDB/调试验收证据，保留 Cargo 默认 `debuginfo=2`，未修改 debug profile）
- [x] 保留桌面、release 和 Android target triple 的 Cargo 自然隔离，不用手工目录复制替代 Cargo fingerprint 管理。（验证：仅配置通用根 target-dir 和 dev profile，未新增 target triple/profile 覆盖或目录复制逻辑）
- [x] 分别执行主工程 `cargo check`、UI 生成工具测试与 `check-boundary`、UI 审计工具测试，确认 profile 对齐不改变功能结果。（验证：`cargo check --locked` 通过；ui-generation `206 passed`；boundary check 所有字段 true；ui-visual-audit `76 passed, 1 ignored` 且所有集成测试通过）

## 阶段 4：脚本、测试和文档路径迁移

- 开始时间：2026-07-29 11:02:57 +08:00
- 结束时间：2026-07-29 11:42:16 +08:00
- 开发总结：活动脚本、UI audit 缓存、安全断言、README/Bevy 指南和 UI 审计示例均已切换到根共享 `target/`；独立 Cargo manifest/lock 边界保持不变。历史 checklist 和兼容忽略项未改动。
- 验证记录：主 agent 复核 8 个允许文件的 diff 并独立执行两个 `-SkipBuild -DryRun`、PowerShell 7 `run-ui-audit.ps1 -SelfTest`、UI generation `check-boundary`、路径检索和 `git diff --check`，均通过。Windows PowerShell 5.1 仍因 `run-ui-audit.ps1` 第 1186 行的既有语法兼容问题无法解析；同一自测在已安装的 PowerShell 7.6 通过。

- [x] 修改 `scripts/start-two-clients.ps1` 和 `scripts/start-robot-sync-two-clients.ps1`，从仓库级 `target/debug` 查找主游戏二进制。（验证：两个脚本均从 `$repoRoot/target/debug/project.exe` 解析；独立 dry-run 生成的 launcher 同时包含 `Set-Location project` 和该根二进制绝对路径）
- [x] 修改 `scripts/run-ui-audit.ps1`，从仓库级 `target/debug` 查找 `ui-visual-audit` 缓存二进制，并保持 Cargo manifest 调用指向独立 UI 审计工具。（验证：`Invoke-UiAuditVisualTool` 缓存路径为 `target/debug/ui-visual-audit.exe`；PowerShell 7 self-test 多次报告该根缓存命中并通过）
- [x] 更新 UI audit 修复安全策略和 self-test：根 `target/` 必须被拒绝，旧 `project/target/` 在迁移窗口内仍被拒绝，且测试覆盖两条路径。（验证：self-test 对 `target/debug/build-output.rs` 与 `project/target/debug/build-output.rs` 分别断言拒绝；PowerShell 7 self-test exit 0）
- [x] 扫描活跃脚本、README、`docs/bevy-getting-started.md`、UI 工具说明和 UI 审计 fixture 中的 `project/target`、`tools/ui-generation/target`、`tools/ui-visual-audit/target` 硬编码，迁移实际运行路径和示例路径。（验证：受限检索在活动范围仅保留 UI audit 的安全拒绝策略及其测试，README、指南、fixture 和 UI audit 示例均为根 `target/`）
- [x] 不篡改已归档 checklist、历史截图、历史报告和验证记录；为保留的历史路径写明其为当时的验收证据，而非当前运行说明。（验证：`git diff --name-only` 不含 `docs/**/checklists/`；全仓旧路径命中均为历史证据、`.gitignore` 兼容项或当前安全策略）
- [x] 更新 `CLAUDE.md`、`README.md`、`docs/bevy-getting-started.md` 及相关 UI 工具文档，明确独立 Cargo 根和共享仓库级缓存的关系、二进制位置和清理方式。（验证：三份正式说明和 `tools/ui-visual-audit/README.md` 说明根 `.cargo/config.toml`、根 `target/`、独立 Cargo 边界及 `CARGO_TARGET_DIR` 约定；Bevy release 输出更新为 `target/release/project.exe`）
- [x] 验证双客户端脚本、UI audit dry-run/self-test、UI 生成工具 `check-boundary` 和 UI 审计工具调用均不依赖旧 target 路径。（验证：两个 dry-run 和 PowerShell 7 self-test 均通过，UI audit 缓存命中 `target/debug/ui-visual-audit.exe`；`cargo run --locked --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .` 所有边界字段 true）

## 阶段 5：旧目录清理与完整重建验证

- 开始时间：2026-07-29 11:43:36 +08:00
- 结束时间：2026-07-29 12:47:12 +08:00
- 开发总结：已受控删除三个旧缓存和预热根缓存并完成完整冷重建。根共享 `target/` 最终为 `30.53 GiB`，较阶段 1 三个旧目录合计 `37.49 GiB` 减少约 `6.96 GiB`；不同 feature/profile、主工程根包与 path dependency、Android target triple 仍会保留必要的独立构件。
- 验证记录：worker 严格串行完成冷/热序列、桌面启动和 Android arm64 构建；主 agent 复核旧目录不存在、三份 metadata 都指向根 target、Git 工作区无可见改动，并热复跑 `cargo check --locked`（0.80s）通过。

- [x] 确认没有活动 Cargo、Rust、游戏客户端、UI 工具或 Android 构建进程，并记录清理命令的确切目标。（验证：清理前进程检查为空；逐一核对并操作 `H:\project\mybevy\project\target`、`H:\project\mybevy\tools\ui-generation\target`、`H:\project\mybevy\tools\ui-visual-audit\target` 与冷测量预热根 `H:\project\mybevy\target`）
- [x] 在已保存基线且明确确认后，删除三个旧 target：`project/target`、`tools/ui-generation/target`、`tools/ui-visual-audit/target`；不删除源码、锁文件、资源、summary 产物或 Android 工程配置。（验证：使用同一 PowerShell 的精确绝对路径 `.NET DirectoryInfo.Delete(true)` 后，三目录均不存在；Git 工作区无可见改动）
- [x] 若阶段 2 或阶段 3 已产生预热的根 target，为取得可比较的冷构建数据，在同一次受控操作中清理根 `target/` 后再重建；不要把无关的日常缓存清理伪装成基准测试。（验证：预热根 target 清理后于 `2026-07-29 11:45:54 +08:00` 开始严格串行冷序列；未清理复跑另计为增量数据）
- [x] 依次运行主工程 `cargo check`、UI 生成工具测试和 `check-boundary`、UI 审计工具测试，确认产物只进入根 `target/`。（验证：冷序列 `cargo check` 257.463s、ui-generation 206 passed/约1155.1s、boundary 19.352s、ui-visual-audit 76 passed/1 ignored/131.873s；主复核三份 metadata 皆为 `H:\project\mybevy\target`）
- [x] 运行一次主游戏开发构建，确认 `target/debug/project.exe` 可启动，并能从 `project/assets` 加载资源。（验证：`cargo build --manifest-path project/Cargo.toml` 1019.250s 通过且生成根 `target/debug/project.exe`；以 `project/` 工作目录和 `WGPU_BACKEND=dx12` 启动 20 秒，无 asset 加载错误后优雅退出）
- [x] 在需要验证移动端时运行 Android arm64 Rust 构建，确认中间产物进入共享 target 的 target-triple 子目录，且 `cargo ndk -o` 仍正确复制 `libproject.so` 到 `jniLibs`。（验证：`cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --locked --release --lib --crate-type cdylib` 689.734s 通过；`target/aarch64-linux-android/release` 存在，`jniLibs/arm64-v8a/libproject.so` 为 141,738,048 B）
- [x] 重新统计根 target 总量、`debug/deps`、`debug/incremental`、release、Android 子目录和文件数，并与阶段 1 基线比较。（验证：根 target `32,781,702,441 B`/`18,171` 文件（30.53 GiB）；deps 17.50 GiB、incremental 5.95 GiB、release 0.44 GiB、aarch64 Android 1.54 GiB；较旧三 target 的 37.49 GiB 减少约 6.96 GiB/1,371 文件）
- [x] 记录三个 Cargo 根的冷构建与增量构建耗时，说明实际复用的产物和仍然重复的原因。（验证：热复跑依次为主 check 1.665s、ui-generation test 5.281s、boundary 1.011s、ui-visual-audit test 2.748s；重复原因记录为不同 Cargo.lock/profile/feature、ui-generation `full` 与审计 `provider-core` 及 Android triple）

## 阶段 6：清理策略与失败回滚

- 开始时间：2026-07-29 12:48:03 +08:00
- 结束时间：2026-07-29 13:00:30 +08:00
- 开发总结：新增只指向仓库根共享缓存的受控清理脚本；默认仅预演，实际删除需要双重显式开关，支持优先清理 `target/debug/incremental`。正式说明覆盖共享 `cargo clean` 的影响、进程阻断、阈值/周期、锁等待诊断及无源码回滚步骤。
- 验证记录：主 agent 独立运行 PowerShell 7 与 Windows PowerShell 自测、全量和 incremental 预演、未确认执行拒绝、`cargo fmt --check` 和 `cargo check --locked`；根 target 保留，三个 legacy target 均不存在。未为验证执行真实删除分支，避免清除 30+ GiB 预热缓存。

- [x] 提供只清理根共享 target 的仓库级脚本或明确命令，并显式说明共享配置下任一 Cargo 根的 `cargo clean` 都会影响所有复用该目录的构建缓存。（验证：`scripts/clear-shared-cargo-target.ps1` 固定解析根 `target/`，只允许其本身或 `debug/incremental`；README、Bevy 指南和 CLAUDE.md 明确 `cargo clean` 影响全部共享缓存）
- [x] 明确运行中的游戏、两个 UI 工具、测试和 Android 构建期间禁止执行共享缓存清理；清理入口先检测相关进程并要求人工确认。（验证：脚本在预演和实际删除前检测 cargo/rustc/link、游戏、UI 工具、测试子进程、Gradle/ADB/关联 Java；删除需 `-Execute -ConfirmSharedTargetCleanup`，PowerShell 7/5.1 `-SelfTest` 通过）
- [x] 约定磁盘阈值或人工周期，优先清理 stale incremental，而不是在每次构建前自动清空缓存。（验证：README 和 Bevy 指南约定根 target 超过 35 GiB、磁盘紧张或发布/大型分支后人工检查，`-IncrementalOnly` 只清理 `target/debug/incremental`）
- [x] 记录共享 target 出现锁等待时的诊断方法，区分正常 Cargo 排队、遗留构建进程和真实死锁。（验证：README、Bevy 指南和 CLAUDE.md 说明 CPU/磁盘/日志进展、遗留进程排查与保留信息后重启构建的条件）
- [x] 定义回滚步骤：移除根 `.cargo/config.toml`、恢复 `.gitignore` 和脚本/文档路径、删除根共享缓存后分别重建三个 Cargo 根。（验证：README 和 Bevy 指南包含按此顺序的回滚步骤及三个 Cargo 根重建命令范围）
- [x] 验证回滚不要求修改源码、锁文件、UI 工具与正式包的依赖方向，且旧 target 路径不会被误当作仍在使用的共享缓存。（验证：三份正式说明明确不改 Rust 源码/Cargo.lock/依赖方向；脚本 self-test 拒绝 project、ui-generation、ui-visual-audit 三个 legacy target 作为清理范围）

## 阶段 7：UI 工具轻量依赖边界评估

- 开始时间：2026-07-29 13:01:42 +08:00
- 结束时间：2026-07-29 13:04:12 +08:00
- 开发总结：阶段 5 已使总缓存较三份旧缓存减少约 6.96 GiB，达到本任务的目录空间目标；因此保留现有 tooling facade，不提取 `ui-document-core`，也不为此改造 project feature。依赖图仍证明全功能生成工具会编译 project/Bevy 等运行时依赖，但审计工具的 provider-core 边界保持轻量。
- 验证记录：使用锁定 Cargo 依赖图检查 UI 生成工具到 Bevy 的反向路径、project 直接依赖和 UI 审计工具图；未修改源码、锁文件或 UI 工具依赖方向，未产生拆包提交。

- [x] 根据阶段 5 的数据判断共享 target 和 profile 对齐是否已经达到空间目标；达到目标则不为拆包而拆包。（验证：根 target 为 30.53 GiB，较旧三 target 合计 37.49 GiB 减少约 6.96 GiB；本阶段决定不拆包）
- [x] 使用 Cargo 依赖图确认 `ui-generation` 的默认 `full` feature 是否仍因 `project` path dependency 编译完整 Bevy 渲染、音频、网络和游戏模块，并单独确认 `ui-visual-audit` 的 `provider-core` 边界没有意外拉入这些模块。（验证：`cargo tree --manifest-path tools/ui-generation/Cargo.toml -e normal -i bevy` 为 `bevy <- project <- ui-generation`；project 直接依赖含 Bevy、reqwest、tokio、tokio_kcp、authority/sim core；UI audit 对 `project` 与 `bevy` 的反向查询均无匹配）
- [x] 比较保留现有 `project::framework::ui::document::tooling` facade、提取轻量 `ui-document-core` crate 与为 `project` 增加 tooling-only feature 三种方案的体积收益、API 稳定性和维护成本。（验证：保留 facade 无迁移成本但 full 工具冷构建会含 project/Bevy；core crate 理论上可消除该工具的运行时图但需稳定协议 crate 和兼容迁移；tooling-only feature 需把 project 的无条件 Bevy/运行时依赖拆成可选 feature，维护风险更高；空间目标已达成，均不实施）
- [x] 若提取轻量 crate，限定其只承载稳定数据协议、Serde 模型、纯校验和 canonical hash 等无 Bevy 能力；迁移方案必须保持 tooling facade 的兼容边界。（验证：本阶段未提取；评估结论已限定未来 `ui-document-core` 只能承载协议/纯逻辑，且必须保留现有 tooling facade）
- [x] 保持 `project` 和 `ui-generation` 单向依赖轻量 crate，禁止轻量 crate 反向依赖游戏运行时、Bevy UI runtime 或生成工具；UI 审计工具继续经 `provider-core` 使用必要能力。（验证：未改变当前方向；`ui-visual-audit` Cargo.toml 仍以 `default-features = false, features = ["provider-core"]` 依赖 ui-generation，依赖图不含 project/Bevy）
- [x] 单独制定并提交拆包设计，不把该架构改造混入共享 target 的基础迁移提交。（验证：空间目标已达成，不需要拆包设计或提交；本轮四个基础迁移提交均未包含轻量 crate/依赖方向改造）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-29 09:55:36 +08:00
- 结束时间：2026-07-29 13:05:03 +08:00
- 验收总结：三个独立 Cargo 根保持独立清单、锁文件和发布边界，并通过根 Cargo 配置共享缓存。冷重建后的共享 target 为 30.53 GiB，较旧三 target 合计减少约 6.96 GiB；桌面与 Android 验证、工具测试和清理策略验证均通过。阶段 7 结论为现有空间目标已达成，不进行轻量 crate 拆分。

- [x] `project`、`tools/ui-generation` 与 `tools/ui-visual-audit` 继续保持独立代码边界、Cargo manifest、锁文件和发布边界。（验证：三个 `cargo metadata --no-deps` 的 workspace_root 仍各自独立，三个 manifest/lock 文件均保留，未引入 workspace）
- [x] 三个 Cargo 根及从仓库根发起的 `--manifest-path` 调用均使用仓库根共享 `target/`，且根 target 被 Git 忽略。（验证：最终 metadata 三个 manifest 均返回 `H:\project\mybevy\target`；根 `.gitignore` 包含 `/target/`）
- [x] 主工程、两个工具和 Android 构建没有因共享缓存出现产物覆盖、错误启动或错误复制。（验证：阶段 2 并发 check 仅发生正常锁等待；阶段 5 桌面启动无 asset 错误，Android arm64 构建通过并复制 141,738,048 B `libproject.so`）
- [x] 双客户端脚本、UI audit 缓存路径与安全规则、活跃工具文档、fixture 和当前正式文档均与根 target 路径一致；历史验收记录未被篡改。（验证：阶段 4 两个 launcher dry-run、PowerShell 7 UI audit self-test 与 boundary check 通过；历史 checklist 未纳入该阶段 diff）
- [x] 主工程 `cargo check`、UI 生成工具测试、`check-boundary`、UI 审计工具测试和必要的桌面启动验证通过。（验证：阶段 5 冷序列为 project check 257.463s、ui-generation 206 passed、boundary 19.352s、ui-audit 76 passed/1 ignored、桌面运行 20 秒；阶段 6 热 `cargo check --locked` 1.74s 通过）
- [x] 重建后的硬盘占用与阶段 1 基线相比有明确、可复现的下降，并记录冷构建、增量构建和未复用产物的代价。（验证：30.53 GiB/18,171 files 对比旧三目录 37.49 GiB/19,542 files，减少约 6.96 GiB/1,371 files；冷/热时长和 feature/profile/triple 差异均记录于阶段 5）
- [x] 共享 target 的清理、并发锁等待、失败诊断和回滚流程有明确说明并经过验证。（验证：阶段 6 脚本双 PowerShell self-test、dry-run 与未确认删除拒绝通过；README、Bevy 指南和 CLAUDE.md 已记录维护流程）
- [x] 未提交 target、PDB、rlib、APK、日志、本地配置、密钥或其他构建产物；完成的 checklist 已归档到对应 `docs/<领域>/checklists/` 并随实现提交。（验证：根 target 已被 Git 忽略；本 checklist 将归档到 `docs/ui/checklists/` 并在归档提交前复核暂存范围）
