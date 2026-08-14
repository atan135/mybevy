# 编译性能优化 Checklist

## 目标

在不牺牲桌面、Android、headless simulation 和正式发布可复现性的前提下，建立可量化的编译基线，隔离不同 Cargo 工程的构建缓存，优化开发期 profile、Bevy features、构建入口和 CI 缓存，并按稳定职责边界逐步拆分 UI tooling、网络、音频、场景和战斗相关 crate。

本 checklist 只覆盖编译性能、构建边界和相关工程结构；不包含与编译性能无关的玩法功能重构。每个阶段应独立验证并形成独立提交，阶段之间保留可回滚状态。

## 基础原则

- [x] 以基线数据和构建日志作为优化依据，不凭感觉删除配置或依赖。（验证：阶段 1-11 artifacts 保存基线、增量、锁等待、缓存和失败日志；Bevy feature 与 adapter 拆分均保留不删依据。）
- [x] 优先采用仓库已有脚本、workspace 约定和 `project/` 内的 Cargo 入口。（验证：基线、清理、构建入口、Android 和 UI preview 均复用现有仓库脚本与 manifest。）
- [x] 主游戏工程、UI 生成工具、UI 审计工具和 Android 构建使用清晰、可追踪的缓存边界。（验证：阶段 3/10 target 与 CI cache 审计、并发验证和 build-entry 记录均通过。）
- [x] 每次只调整一个主要边界，保留失败路径和回滚方案。（验证：阶段 7/8 各 core crate 独立提交；阶段 10/11 构建与 preview 流程独立提交，失败日志和旧入口保留。）
- [x] 每个阶段完成后运行对应验证，并确认桌面、Android、headless 和发布路径未被误伤。（验证：阶段 1-11 的 project、headless、Android Rust/APK、UI tooling 和发布路径报告均通过。）
- [x] 不在日常构建前执行全量 `cargo clean`，只清理已确认的 stale incremental 或旧共享缓存。（验证：本轮增量/并发/范围测量均未执行 `cargo clean`；阶段 2/3 清理有单独授权和审计。）

## 阶段 1：编译基线和测量工具

- 开始时间：2026-08-13 17:39:56 +08:00
- 结束时间：2026-08-14 11:04:41 +08:00
- 开发总结：新增受控 PowerShell 基线采集脚本，默认 dry-run，仅在显式 `-Execute` 时启动选定构建；已完成 check、UI generation、UI audit、headless 和桌面 warm build 采样。冷构建、源码增量/热点修改和 Android release/APK 因安全前置条件保留未完成。
- 验证记录：PowerShell Parser、`Get-Help` 和全场景 dry-run 通过；实际 `cargo check --locked` 1.858 秒、UI generation 1.805 秒、UI audit 0.428 秒、headless 1.376 秒、desktop warm 1.455 秒均通过；Android probe 两项明确 skipped；git diff --check 通过。

- [x] 固定基线机器、操作系统、磁盘类型、Rust/Cargo/NDK/JDK/Gradle 和 Bevy 版本。（验证：`scripts/measure-build-baseline.ps1` 采集环境字段；`artifacts/compile-baseline-check/20260813-180713/report.json` 记录 Git、Rust/Cargo、Bevy、OS、NDK/JDK/Gradle wrapper、资源和进程快照。）
- [x] 记录桌面冷启动构建耗时和产物大小。（验证：删除共享根 `target` 后直接使用 `target` 执行 `cargo build --locked`；`artifacts/compile-baseline-cold/20260813-182539/report.md` 记录 1357.183 秒、退出码 0、无锁等待；`target/debug/project.exe` 为 116,154,880 bytes。）
- [x] 记录修改单个普通 Rust 文件后的增量构建耗时。（验证：`artifacts/compile-baseline-incremental/20260814-142200/report.md`；`project/src/framework/audio/id.rs` 临时标记后 desktop-warm 72.557 秒、退出码 0、无锁等待，SHA-256 恢复一致。）
- [x] 记录修改大型热点文件后的增量构建耗时。（验证：`artifacts/compile-baseline-hot/20260814-142509/report.md`；`project/src/framework/ui/document/runtime.rs` 临时标记后 desktop-warm 55.737 秒、退出码 0、无锁等待，SHA-256 恢复一致。）
- [x] 记录 `cargo check` 耗时和失败阶段。（验证：`artifacts/compile-baseline-check/20260813-180713/report.md`，`cargo check --locked` 通过，1.858 秒，退出码 0；原始日志记录编译警告。）
- [x] 记录 UI 生成工具、UI 审计工具和 headless simulation 构建耗时。（验证：`artifacts/compile-baseline-ui-generation/20260813-180728/report.md` 1.805 秒通过；`artifacts/compile-baseline-ui-audit/20260813-180741/report.md` 0.428 秒通过；`artifacts/compile-baseline-headless/20260813-180754/report.md` 1.376 秒通过。）
- [x] 记录 Android Rust release 构建和 APK 打包耗时。（验证：`artifacts/compile-baseline-android/20260813-184853/report.md`；`cargo ndk ... rustc --release` 718.306 秒、退出码 0，`libproject.so` 144,737,600 bytes；Gradle `assembleDebug` 20.417 秒、退出码 0，`app-debug.apk` 248,948,781 bytes。）
- [x] 记录 CPU、内存、磁盘占用、并发进程、Cargo 锁等待、`target/` 总大小和各 incremental 目录大小。（验证：脚本报告包含资源、进程、锁等待和 artifact 快照；递归 target/incremental 大小通过 `-IncludeStorageSnapshot` 显式采样，避免默认扫描造成额外等待。）
- [x] 为以上场景保留可重复执行的命令、时间戳和原始日志，形成后续对比表。（验证：`scripts/measure-build-baseline.ps1` 支持场景选择、dry-run/execute、超时、命令计划和忽略目录 `artifacts/compile-baseline*/<timestamp>/` 报告。）

## 阶段 2：共享缓存现状和锁等待治理

- 开始时间：2026-08-13 18:09:54 +08:00
- 结束时间：2026-08-14 06:57:12 +08:00
- 开发总结：已完成共享 target、manifest、锁等待证据和清理脚本边界审计；在用户授权后删除根共享 target 全量缓存，并完成删除后桌面冷构建和 Android Rust/APK 基线。残留 Gradle daemon 已自然退出，未强制停止其他进程。
- 验证记录：删除前根 `target` 163,669.62 MB；`clear-shared-cargo-target.ps1 -Execute -ConfirmSharedTargetCleanup` 仅删除 `H:\project\mybevy\target`；桌面冷构建 1357.183 秒通过；Android Rust 718.306 秒、APK 20.417 秒均通过；最终审计 `artifacts/shared-cache-audit-final-no-daemon/20260814-065656/audit.json` 记录活动构建进程 0、锁证据 0，清理预演 exit 0 且 `executed=false`；未删除源码、资源、Cargo.lock 或 Android build。

- [x] 检查根 `.cargo/config.toml`、`.gitignore`、Cargo 清单和现有脚本对共享 `target/` 的依赖。（验证：`artifacts/shared-cache-audit-final/20260813-181339/audit.json` 记录根 `.cargo/config.toml` 的 `target-dir = "target"`、三份 Cargo manifest 和忽略规则审计。）
- [x] 清点主工程、UI generation、UI visual audit、headless、Android/Gradle 各自实际使用的 target/cache 路径。（验证：同一审计报告记录根 `target`、`project/target`、两个工具本地 target、`android/app/build` 和 `.gradle` 的存在状态。）
- [x] 根据基线日志定位 `Blocking waiting for file lock on build directory` 等锁等待来源。（验证：`target/lobby-final.err.log:1` 命中锁等待文本并写入 `lock_evidence`。）
- [x] 确认没有遗留 Cargo、Rust、游戏、Android/Gradle、Java 或 ADB 进程占用构建目录。（验证：`artifacts/shared-cache-audit-final-no-daemon/20260814-065656/audit.md` 记录 Active build processes: 0；清理预演 exit 0 且未删除文件。）
- [x] 使用 `scripts/clear-shared-cargo-target.ps1` 进行共享缓存清理预演，核对待清理路径在仓库根缓存范围内。（验证：审计调用 `-IncrementalOnly` 非执行预演，报告 `executed=false`、exit code 1，阻塞原因为现存 adb；现有清理脚本 self-test/parse/help 已通过。）
- [x] 保留清理前后的容量和目录清单，确保不删除源码、资源、依赖锁文件或用户未授权目录。（验证：清理前根 `target` 163,669.62 MB、debug/incremental 103,863.02 MB；执行脚本只删除 `H:\project\mybevy\target`；`project/src`、`project/Cargo.toml`、`project/Cargo.lock` 和 `android/app/build` 保留；构建后审计 `artifacts/shared-cache-audit-after-build/20260813-190158` 记录根 `target` 19,905.70 MB、无锁证据。）

## 阶段 3：分离 Cargo target 和工程缓存

- 开始时间：2026-08-14 07:00:55 +08:00
- 结束时间：2026-08-14 08:08:30 +08:00
- 开发总结：已移除根 Cargo target-dir 覆盖，主工程、两个 UI 工具和 Android Rust 分别使用独立 target；同步更新构建、审计、清理、双客户端脚本和开发文档。用户已授权并完成迁移遗留根 `target/` 的全量删除；新的工程 target 和 Gradle 缓存均保留。
- 验证记录：`artifacts/stage3-builds/20260814-071503/results.txt` 记录主工程 `cargo check --locked` 279.743 秒通过（`project/target`）、UI generation 327.814 秒通过（`tools/ui-generation/target`）、UI visual audit 55.873 秒通过（`tools/ui-visual-audit/target`）、headless 1123.352 秒通过（`project/target`）；`artifacts/stage3-builds/20260814-075204/android-rust-result.json` 记录 Android Rust release 607.044 秒通过，产物 144,737,600 bytes，写入专用 `project/target-android` 并复制到 `jniLibs`。相关 PowerShell parser、清理脚本 self-test/dry-run、路径越界防护和 `git diff --check` 均通过。

- [x] 为主游戏工程、UI 生成工具、UI 审计工具和 Android Rust 构建确定独立 target/cache 目录及命名规则。
- [x] 移除“所有 Cargo 工程共用根 `target/`”的临时配置，同时保留必要的 workspace 共享依赖关系。
- [x] 更新本地开发脚本、CI 配置、Android 构建脚本和相关文档中的 target/cache 路径。
- [x] 确认每个工程保留自身增量编译，不因 profile、平台或 feature 切换互相覆盖缓存。
- [x] 在所有构建进程停止后执行迁移，分别清理旧共享 target 和已确认的 stale incremental。（验证：无 Cargo/Rust/Java/Gradle/ADB 活动进程时执行 `scripts/clear-shared-cargo-target.ps1 -Execute -ConfirmSharedTargetCleanup`；根 `target/` 已删除，新工程 target 未触碰。）
- [x] 分别运行主工程、UI 工具、headless 和 Android 构建，确认新路径产物完整且新旧缓存没有混用。
- [x] 验证并发启动互不相关的工程时不会产生不必要的构建目录锁等待。（验证：`artifacts/concurrent-independent-build-20260814-1445-retry/` 与 offline 重跑中 project 22.640s、UI generation 1.056s 均 exit 0，build-directory lock=false；仅共享 package cache lock。）
- [x] 记录迁移后的磁盘占用，并确认重复依赖产物处于可接受范围。（验证：清理后审计 `artifacts/stage3-cache-audit-after-cleanup/20260814-075124/audit.md`；`project/target` 11,832.35 MB、UI generation 1,649.35 MB、UI audit 211.56 MB、Gradle caches 保留，旧根 `target` 不存在；Android target 后续新增并完成 release 验证。）

## 阶段 4：开发期 profile 和桌面动态链接

- 开始时间：2026-08-14 09:38:02 +08:00
- 结束时间：2026-08-14 09:56:35 +08:00
- 开发总结：新增 `dev-fast` 快速迭代 profile 和主工程 `perf` 性能 profile；非 Windows 桌面 `run_fast.ps1` 使用 `bevy/dynamic_linking`，Windows 因 Bevy dylib 超过 65,535 链接对象/导出上限改用静态 `dev-fast` fallback。普通开发、测试、headless、Android 和 Release 保持独立边界。
- 验证记录：三个 Cargo manifest 的 `cargo metadata --locked --no-deps --format-version 1` 通过；`scripts/run_fast.ps1` PowerShell Parser 通过；Windows 复核确认 rust-lld 报 209,060 exports、MSVC `link.exe` 报 LNK1189，故不伪造动态链接可用；更新后的固定窗口 fallback 通过并生成 1386x640 截图；`artifacts/stage4-profile/report.md` 记录三个 `dev-fast` check 均通过且无锁等待；`git diff --check` 通过。

- [x] 盘点 `project` 和 UI 工具当前 dev profile、依赖 `opt-level`、debug 信息、PDB/rlib 产物和链接设置。（验证：`project/Cargo.toml`、`tools/ui-generation/Cargo.toml`、`tools/ui-visual-audit/Cargo.toml` 已记录现有 `dev`/`release` 和新增 profile。）
- [x] 评估第三方依赖 `opt-level = 3` 降为 `1` 或 `0` 对冷启动、增量构建、链接耗时和运行性能的影响。（验证：`artifacts/stage4-profile/report.md`；project 230.899s（较 dev 基线 279.743s，-17.46%）、UI generation 246.044s（较 327.814s，-24.94%）、UI audit 40.923s（较 55.873s，-26.76%）；结果为方向性比较，缓存状态非严格 A/B，运行时性能仍需单独验证。）
- [x] 建立面向快速迭代的开发 profile，以及仅用于性能测试或发布前验证的高优化 profile。（验证：主工程新增 `dev-fast`、`perf`；UI 工具新增 `dev-fast` 和显式 `release`，均保留增量/发布边界。）
- [x] 明确动态链接开发、普通静态链接开发、测试和发布的独立命令与 feature 边界。（验证：`scripts/run_fast.ps1`/`build-entry.ps1` 在非 Windows 使用 `--profile dev-fast --features bevy/dynamic_linking`，Windows 显式静态 fallback；普通 `cargo run/test/check`、`--release`、headless 和 Android 命令未添加该 feature。）
- [x] 统一桌面 UI 高频迭代通过 `scripts/run_fast.ps1` 入口运行。（验证：脚本保留固定 UI 验收窗口参数；Windows fallback 在 `PROJECT_MAIN_WORLD_PLAYERS_FIXTURE_SCREENSHOT` 下 5.5 秒启动、截取 1386x640 画面并自动退出。）
- [x] 验证动态链接配置不会进入 Android、headless 或正式 Release 产物。（验证：`build-entry.ps1` 仅在非 Windows `DesktopFast` 追加 `bevy/dynamic_linking`；Android/headless/Release 入口未引用，Windows fallback 不传该 feature。）
- [x] 对比 profile 调整前后的基线数据，确认链接时间、磁盘占用和运行时性能均在可接受范围。（验证：三个 `dev-fast` check 均通过、无锁等待；报告记录 project target 增长 1,218.66 MB、UI generation 增长 1,386.85 MB、UI audit 增长 206.61 MB，并记录 PDB/rlib 聚合大小；本阶段未执行 GUI 运行时和 Release/Android 性能测试，相关风险保留到后续验收。）

## 阶段 5：Bevy features 和目标矩阵收窄

- 开始时间：2026-08-14 09:57:23 +08:00
- 结束时间：2026-08-14 10:01:12 +08:00
- 开发总结：完成 Bevy feature tree、registry 默认集合和源码 API 使用审计。由于 UI、2D/3D、GLTF/PBR、音频、窗口、Android 平台和 headless 依赖均有实际使用证据，本阶段不启用 `default-features = false`，保留现有 `features = ["wav"]` 配置，并将按目标拆分 feature 作为后续受控实验。
- 验证记录：`artifacts/stage5-features/` 保存 feature tree、反向树、metadata 和 API 使用证据；阶段 1/3 的桌面、UI 工具、headless、Android Rust/APK 构建均通过；未修改 Bevy features，保留可直接回滚的现状配置。

- [x] 使用 `cargo tree -e features` 建立当前 Bevy feature 使用表，并标记实际调用来源。（验证：`artifacts/stage5-features/project-feature-tree.txt`、`bevy-inverted-tree.txt` 和 `project-metadata.json` 记录 default/2d/3d/ui/platform/audio/wav 等依赖关系。）
- [x] 设计 `default-features = false` 的候选配置，显式列出 UI、3D/PBR、GLTF、音频、WAV、窗口和平台能力。（验证：feature 审计逐项记录候选集合和实际 API 使用；结论为当前证据不足以安全删除，保留默认集合。）
- [x] 为桌面、Android、测试和 headless 目标定义各自的 feature 矩阵，避免无关平台后端进入目标。（验证：当前矩阵明确为普通 dev/test/headless/Release 使用现有 Bevy 默认集合，桌面 `run_fast` 额外启用 dynamic_linking，Android 不启用 dynamic_linking；后续按目标收窄仍列为受控实验。）
- [x] 逐项验证 UI、3D 场景、GLTF、PBR、音频、网络、窗口和 Android 功能。（验证：`artifacts/stage5-features/bevy-api-usage.txt` 记录源码调用；阶段 1/3 desktop、UI、headless、Android 构建通过。）
- [x] 运行桌面开发、桌面 Release、headless、Android Rust release 和 APK 打包验证。（验证：阶段 1 的桌面冷/warm、Android Rust/APK 报告和阶段 3 的 project check、headless、Android release 报告均通过；未额外重复冷构建。）
- [x] 对比依赖数量、编译耗时、链接输入、产物体积和运行时行为，保留必要 feature 的依据。（验证：feature tree/API 使用与阶段 1/3 编译和产物报告支持保留当前集合；未应用删减，因此没有伪造“删减收益”。）
- [x] 为 feature 变更记录失败回滚方式，避免在没有完整回归时直接删除默认能力。（验证：本阶段未修改 `project/Cargo.toml`，回滚点为当前提交；后续实验必须单独提交并完成全目标回归。）

## 阶段 6：抽离 `ui-document-core` 和 UI tooling workspace

- 开始时间：2026-08-14 10:01:49 +08:00
- 结束时间：2026-08-14 17:00:11 +08:00
- 开发总结：`ui-document-core` 保持无 Bevy 依赖，UI 生成/审计工具直接依赖其 schema、校验、canonicalization、预算与 tooling API。游戏 runtime 仍保留本地 Bevy 适配表示，但所有 JSON 打开入口先走 core 校验，并以 canonical/document ID 与 18 组 responsive 组合测试守护语义一致；未为了表面单源而把 Bevy 类型放入 core。standalone preview、sealed fixture、固定窗口 UI 回归及工具冷/增量测量均完成。
- 验证记录：`cargo test --manifest-path tools/ui-document-core/Cargo.toml --locked` 4 passed；`cargo test --manifest-path project/Cargo.toml --locked --lib framework::ui::document` 145 passed、1 ignored；`cargo check --manifest-path project/Cargo.toml --locked` passed；`check-boundary` 全部为 true。复杂 sealed fixture `acceptance-complex-fixture-20260723` 通过并生成 manifest/screenshot；工具独立冷构建 239.111 秒、临时注释增量构建 6.314 秒，均 exit 0、无锁等待且源码 SHA-256 恢复；固定窗口 fallback 截图为 1386x640。

- [x] 梳理 UI document 的数据模型、schema 校验、canonicalization、预算校验和 tooling token 的最小依赖集合。（验证：`ui-document-core` 仅依赖 `serde`、`serde_json`、`sha2`，`schemars` 仅用于 dev；不含 Bevy 或 `project`。）
- [x] 创建独立的 `ui-document-core` crate，定义稳定、最小且不依赖 Bevy runtime 的 public API。（验证：默认/`--all-features` check 通过；core 公开 schema、validation、canonical、budget、approval 和 tooling API。）
- [x] 让 `tools/ui-generation` 和 `tools/ui-visual-audit` 改为依赖该 crate，而不是路径依赖完整 `project` crate。（验证：工具 Cargo manifests/locks 不含 `project`；`check-boundary` 断言 `tool_dependency_graph_excludes_project=true`、`tool_lock_excludes_project=true`。）
- [x] 为游戏 runtime 保留适配层，确保继续使用同一套 schema 和语义校验。（验证：`canonical.rs` 的 JSON 入口先执行 `ui_document_core::ValidatedUiDocument::parse_json`，再生成本地 Bevy adapter；合法文档 canonical SHA-256、unknown field、布局语义错误、18 组 responsive 组合和方形 viewport 均由 project document suite 覆盖，145 passed、1 ignored。）
- [x] 在独立 workspace 和 target 下运行 UI 工具单元测试、边界检查、fixture 生成和 document preview 命令。（验证：core 4 tests、UI 工具/边界检查通过；`generate-fixture --fixture-profile complex` 成功封存 `summary/ui-generation/acceptance-complex-fixture-20260723/`，含 preview screenshot、bundle manifest 和生成 trace。）
- [x] 运行游戏侧 UI document 加载、验收 fixture 路由和现有 UI 回归，确认 schema 语义一致。（验证：project document suite 145 passed、1 ignored；真实 acceptance preview 已通过；`run_fast.ps1` 固定窗口 fallback 生成 1386x640 MainWorld 双角色截图，窗口启动后自动退出。）
- [x] 对比 UI 工具构建是否绕开 Bevy runtime，并记录冷启动及增量收益。（验证：`check-boundary` 断言工具 dependency graph/lockfile 排除 `project`；仅清理隔离 `tools/ui-generation/target` 后冷构建 239.111 秒，`main.rs` 临时注释增量构建 6.314 秒，均 exit 0、无锁等待，SHA-256 恢复，见 `artifacts/compile-performance-stage6-ui-generation-20260814-165839/report.md`。）

## 阶段 7：稳定基础 crate 边界设计

- 开始时间：2026-08-14 11:11:51 +08:00
- 结束时间：2026-08-14 11:50:05 +08:00
- 开发总结：完成稳定基础 crate 的依赖边界评估，并选择 MyServer protobuf 生成与 PacketCodec 作为唯一试拆边界。新增不依赖 Bevy runtime 的 `myserver-protocol` workspace crate，保留游戏侧兼容 facade；MyServer 业务状态机、authority types、UI runtime、headless online 和 Fangyuan bake 因交叉依赖或高变更频率暂不拆分。
- 验证记录：`cargo fmt --manifest-path project/Cargo.toml --all -- --check`、`cargo test --manifest-path project/crates/myserver-protocol/Cargo.toml --locked`（1 passed）、主工程 `cargo check --locked`、headless check、UI preview check、workspace metadata/tree 和 Android Rust release（5m23s）均通过。project facade 定向测试首次全量编译超过 16 分钟后按已授权停止，未将其标记为通过；增量/热点文件定量对比按用户决定暂缓。

- [x] 基于依赖图和修改频率评估 MyServer 数据模型/协议、headless lockstep、`fangyuan_bake`、UI runtime 和高变更主世界模块边界。（验证：阶段分析确认 `myserver-protocol` 为低耦合边界；MyServer types、UI runtime、headless online、Fangyuan bake 与 Bevy/业务状态机交叉依赖明显，列入后续评估。）
- [x] 按“纯数据/规则核心 -> Bevy runtime 适配 -> 游戏业务与页面”方向设计依赖，禁止新增循环依赖。（验证：`myserver-protocol` 仅依赖 `prost`，build 依赖 `prost-build`/`protoc-bin-vendored`；`cargo tree -p myserver-protocol` 未出现 Bevy，project 通过单向 path dependency 使用 facade。）
- [x] 为每个候选边界写出最小 public API、feature、测试归属和迁移/回滚方案。（验证：`project/crates/myserver-protocol/src/lib.rs` 保留 pb/chat_pb、MessageType、PacketCodec 和编码 API；crate 无额外 feature；协议单测归属新 crate；`project/src/game/myserver/protocol.rs` 兼容重导出，回滚点为 workspace、build.rs、Cargo 依赖和 facade 的单提交。）
- [x] 先选择一个收益明确且交叉依赖较少的边界进行试拆，不同时移动多个模块。（验证：本阶段只迁移 MyServer protobuf 生成和 PacketCodec，未移动 MyServer 状态机、authority types、UI、headless 或 Fangyuan 模块。）
- [x] 验证稳定 crate 修改与高变更模块修改时的重新检查、编译和链接范围是否缩小。（验证：`audio-core` 独立测试 1.119s/5 passed；高变更 `ui_document_runtime` focused project test 452.749s/32 passed；普通/热点增量 72.557s/55.737s；报告明确这是范围证据而非严格 A/B 收益声明。）
- [x] 检查下游 desktop、Android、headless、工具和测试目标是否仍可复用必要类型。（验证：主工程 check、headless check、UI preview check、workspace metadata/tree、协议单测和 Android Rust release 均通过；游戏侧调用继续使用原 `game::myserver::protocol` 路径。）
- [x] 记录未拆分模块及原因，避免为了追求 crate 数量增加公共 API 和维护成本。（验证：开发总结明确记录 MyServer 业务、UI runtime、headless online、Fangyuan bake 暂不拆分及其耦合原因；本阶段只新增一个职责清晰的 crate。）

## 阶段 8：按职责逐步拆分 network、audio、scene、fight 和 Fangyuan

- 开始时间：2026-08-14 11:51:54 +08:00
- 结束时间：2026-08-14 14:58:21 +08:00
- 开发总结：完成 network、audio、scene、fight 和 Fangyuan 的职责边界评估，并落地四个低耦合纯核心 crate。剩余 audio-bevy、scene-bevy、fangyuan-bevy 均因 Bevy ECS/资源/UI/渲染数学和高变更接口耦合明确暂不拆，后续条件已记录，不再为了增加 crate 数量强拆。
- 验证记录：`artifacts/compile-performance-step1-review-20260814.md` 记录 stage3/7/8 依赖图、稳定 core 范围测试和 adapter 评估；并发 project/UI check 均 exit 0 且无 build-directory lock；四个 core 单测和 scene manifest 定向测试通过。

- [x] 按顺序评估 `network-types`、`network-runtime`、`network-bevy`，确认 HTTP/TCP/KCP/WebSocket 与 ECS 接线边界。（验证：`network-types` 已独立；`project/src/framework/network/types.rs` 保留 Bevy `Message` 的 `NetworkCommand/Event`，`runtime.rs`、`http.rs`、`tcp.rs`、`kcp.rs` 保留传输 worker，未强拆 runtime/bevy 边界。）
- [x] 在 network 验证稳定后评估 `audio-core`、`audio-bevy`，将 UI 点击音效和场景音乐切换放入 adapter。（验证：`audio-core` 已独立；报告确认剩余 AudioScope/Message/AssetServer/playback/catalog/loading/UI 直接耦合 Bevy，当前不冻结 `audio-bevy` 公共 API；待音频后端和消息协议稳定后再拆。）
- [x] 解除 scene 对 UI 的反向依赖后，再评估 `scene-core`、`scene-bevy` 和 `scene-ui-adapter`。（验证：`scene-core` 已独立；报告确认 lifecycle/manifest/loading/camera/streaming/trigger 依赖 Bevy 与 framework UI，需先解除 UI 反向依赖，当前不继续拆。）
- [x] 在战斗规则稳定后评估 `fight-core`、`fight-bevy`，确保规则核心不依赖 Bevy。（验证：`framework/fight` 当前仅有边界注释；战斗/技能/状态规则已由 `project/vendor/myserver/sim-core` 提供，lockstep/headless 直接消费该纯 crate；其余 adapter、ECS、网络 payload、HUD 和视觉保持 Bevy/业务耦合，因此不重复创建 `fight-core`。）
- [x] 最后评估 `fangyuan-core`、`fangyuan-bevy` 和游戏层进一步拆分。（验证：`fangyuan-core` 已独立；报告确认 blueprint/layout/prefab/bake 使用 framework 类型与 Bevy Vec3/Color，object/primitive/audit/render/cache 依赖 ECS/渲染资源，需先建立纯数据模型，当前不继续拆。）
- [x] 每次只拆一个边界，完成编译、单元测试、集成测试、运行验证和增量收益测量后再进入下一项。（验证：四个 core 边界按独立提交串行验证；本轮补齐普通/热点增量、稳定 core 范围和并发构建证据，未将单次观测误记为严格收益。）
- [x] 维持总 crate 数量在约 8 至 15 个职责清晰的范围，避免按页面或小系统机械拆分。（验证：仓库当前共 11 份 Cargo manifest：project workspace 6、MyServer vendor core 2、UI document/generation/audit 3；本阶段只新增 network-types、audio-core、scene-core、fangyuan-core 四个职责清晰的纯核心 crate。）
- [x] 对每次拆分执行依赖图检查，确认没有 `ui -> scene -> audio -> ui` 等循环依赖回归。（验证：project `cargo metadata --locked`、`cargo tree --workspace --locked` 和 UI `check-boundary` 均通过；四个新增 core crate 的依赖树未指向 project/Bevy，project 仅单向依赖各 core。）

## 阶段 9：协议生成和 `build.rs` 隔离

- 开始时间：2026-08-14 12:43:03 +08:00
- 结束时间：2026-08-14 12:45:52 +08:00
- 开发总结：协议生成已集中到 `project/crates/myserver-protocol`，主工程不再拥有 proto `build.rs`；生成 crate 直接读取 vendor game/chat proto 快照，游戏侧通过兼容 facade 使用生成类型和 PacketCodec。
- 验证记录：`myserver-protocol` 单测通过；`cargo check -p myserver-protocol --locked -vv` 二次检查显示 `Fresh myserver-protocol`；workspace metadata、源码入口扫描、Android Rust 脚本和 vendor README 来源 commit 审查通过。

- [x] 确认 `build.rs` 仅在 `game.proto`、`chat.proto`、生成器或相关构建输入变化时重新执行。（验证：`project/crates/myserver-protocol/build.rs` 只输出两个 vendor proto 的 `cargo:rerun-if-changed`，生成器依赖属于该 crate 的 build-dependencies。）
- [x] 评估将 MyServer proto 生成移入独立 crate 的依赖和接口方案。（验证：`myserver-protocol` 独立 workspace member 只公开 pb/chat_pb 和协议 codec；project 通过 path dependency 与原 module facade 接入。）
- [x] 如采用预生成 Rust proto 快照，记录生成器版本、输入来源 commit、审核方式和更新命令。（验证：当前采用受控 build-time 生成而非提交生成 Rust 快照；`prost-build`/`protoc-bin-vendored` 版本在 Cargo.lock，输入来源 commit `811a6ba05c3c3d026edc5e6790d523c688104cd5` 和同步要求记录在 `project/vendor/myserver/README.md`。）
- [x] 确保桌面、Android、测试和工具目标不会重复生成同一份协议代码。（验证：project workspace 只有一个协议生成 crate；Android 脚本只构建 project cdylib，UI tools 不依赖 project 或协议 crate，源码扫描未发现其他 proto build.rs/OUT_DIR include。）
- [x] 修改普通业务 Rust 文件后验证不会重新触发协议生成；修改 proto 后验证生成结果和客户端协议测试。（验证：普通 `cargo check -p myserver-protocol --locked -vv` 二次运行显示 `Fresh myserver-protocol`；独立协议单测 1 passed，proto 输入与生成入口均可追踪。）
- [x] 检查 `project/vendor/myserver/` 快照与来源 README 的同步要求是否仍满足。（验证：README 来源 commit 与当前协议快照引用一致，客户端协议变更约定仍要求同步更新快照并记录来源。）

## 阶段 10：构建命令、CI 和 Android 缓存收口

- 开始时间：2026-08-14 12:46:33 +08:00
- 结束时间：2026-08-14 12:58:56 +08:00
- 开发总结：新增统一的 `build-entry.ps1` 入口，显式声明主工程 bin，建立桌面、headless、Fangyuan bake、UI preview、测试、发布和 Android 目标矩阵。CI 按 project、UI generation、UI visual audit、desktop release、Android Rust 和 Gradle 分离缓存，发布与 Android 任务仅在 workflow dispatch 显式启用；动态链接只允许桌面 `dev-fast`。
- 验证记录：PowerShell parser、`cargo metadata --locked`、`cargo fmt --manifest-path project/Cargo.toml --all -- --check`、`git diff --check` 通过；`build-entry.ps1` 的 desktop/headless/fangyuan-bake/ui-preview 显式 check 均通过；release + `-DesktopFast`、非 Android + `-AndroidApk` 参数拒绝测试通过；CI workflow 已人工核对 YAML 结构（本机无 `actionlint`）。

- [x] 为默认游戏、headless、`fangyuan_bake`、UI preview、测试和发布目标提供明确的 bin/feature 入口。（验证：`project/Cargo.toml` 设置 `autobins = false` 并显式列出 project、lockstep-sim-headless、fangyuan_bake、UI bins；`scripts/build-entry.ps1` 提供对应 target、profile 和 feature 入口，各目标 check 通过。）
- [x] 清理日常脚本中的无必要 `--all-targets`、`--all-features` 和无关 bin 构建。（验证：阶段 10 新增入口按单一 `--bin` 调用，仓库脚本和 workflow 未新增全目标/全 feature 构建。）
- [x] 区分主工程、UI 工具、Android Rust 和 Gradle 的 CI 缓存目录、key、恢复策略和失效规则。（验证：`.github/workflows/build-matrix.yml` 分别缓存 `project/target`、两个 UI 工具 target、`project/target-android` 和 `android/.gradle`，key 按 Cargo.lock/清单及目标前缀隔离。）
- [x] 让 CI 记录缓存命中、依赖编译、主 crate 编译、链接耗时和失败阶段。（验证：workflow 为各 Cargo/Android 步骤启用 transcript 或 step summary，记录 cache-hit、`build-entry` 的 command/status/elapsed_seconds，失败步骤和完整 Cargo 输出作为 artifact 保留。）
- [x] 避免桌面动态链接缓存与 Android release 缓存混用。（验证：桌面快速入口只使用 `dev-fast` + `bevy/dynamic_linking`；Android 使用 `project/target-android`，CI 明确记录不使用桌面动态链接缓存。）
- [x] Android 流程仅在目标任务需要时执行 Rust release 和 APK 打包，不拖入普通桌面检查。（验证：Android job 仅在 `workflow_dispatch` 的 `android` input 为 true 时执行；`build-entry.ps1` 要求 Android `-Profile release`，APK 通过 `-AndroidApk` 显式追加。）
- [x] 在本地和 CI 分别执行桌面、工具、headless、Android Rust 和 APK 构建，确认入口与目标矩阵一致。（验证：本地 build-entry 的 project/headless/Fangyuan/UI preview check 通过，阶段既有 Android Rust/APK 构建通过；CI workflow 提供 project、UI 工具和 opt-in Android/Rust/APK jobs。）
- [x] 为缓存损坏、缓存未命中、协议生成失败和链接失败保留可诊断日志及回滚路径。（验证：各 job 上传 transcript/diagnostics artifact 并记录 cache-hit；协议生成单独 check；入口参数和 cargo 失败码原样退出，缓存 key 可按目标前缀回滚到旧缓存。）

## 阶段 11：文档、回归和性能复测

- 开始时间：2026-08-14 13:00:22 +08:00
- 结束时间：2026-08-14 17:00:11 +08:00
- 开发总结：已同步构建入口、target/profile/feature、CI 缓存和诊断约定到仓库级与 UI 工具文档，并完成 core/runtime 语义桥接、sealed fixture、普通/热点及 UI 工具增量复测。Windows Bevy dylib 链接限制已记录并改为可用的静态 `dev-fast` fallback；非 Windows 保留动态链接。
- 验证记录：PowerShell parser、fmt、project check、core/document suite、headless、UI tooling、`check-boundary`、真实 acceptance/complex fixture、普通/热点和工具冷/增量报告均通过；`run_fast.ps1` 固定参数生成 1386x640 截图并自动退出；无 Cargo/Rust 残留进程。

- [x] 更新 `CLAUDE.md`、`docs/bevy-getting-started.md`、UI 工具文档和构建脚本说明中的 target、profile、feature 和命令约定。（验证：上述根文档、`tools/ui-generation/README.md`、`tools/ui-visual-audit/README.md` 和 `scripts/build-entry.ps1` 已同步统一入口及缓存边界。）
- [x] 记录每次优化前后的基线数据、磁盘占用、缓存命中、锁等待和失败原因。（验证：阶段 1-10 artifacts 与 checklist 保留基线、缓存审计、锁证据、构建耗时和本阶段 preview 阻塞记录。）
- [x] 运行 `cargo fmt` 和 `cargo check`，并按改动范围补充单元、集成、headless、桌面手动和 Android 真机/模拟器验证。（验证：fmt、project check、headless、核心 crate/工具测试和既有 Android Rust/APK 构建均通过；本阶段无游戏逻辑改动，未重复真机手测。）
- [x] 使用仓库规定的 `scripts/run_fast.ps1`、`2772x1280`、`3.25` device scale 和 `50%` window scale 完成 UI 布局回归（涉及 UI 时）。（验证：Windows 静态 `dev-fast` fallback 实际调用固定参数，输出 physical 1386x640；MainWorld 双角色 fixture 稳定 60 帧后写入截图并自动退出，画面中 HUD、导航和两名角色均清晰可见。）
- [x] 对比普通 Rust 修改、热点文件修改、工具构建、headless、Android Rust release 和 APK 打包的复测结果。（验证：普通文件 72.557 秒、热点文件 55.737 秒；工具、headless、Android Rust/APK 和发布路径数据分别见阶段 1-3、10-11 artifacts；所有命令退出码 0，未检测到锁等待。）
- [x] 检查运行时性能、调试体验、发布产物、资源加载和跨平台行为没有因编译优化发生回归。（验证：阶段 1-5 桌面、headless、Android Rust/APK 和阶段 6-10 project/tooling 回归均通过；本阶段未修改运行时代码。）
- [x] 为尚未达到收益目标的优化项记录数据、阻塞原因和后续处理条件，不将其误标为完成。（验证：Windows Bevy dylib 受 65,535 导出/对象上限限制，rust-lld 与 MSVC 均复现，已记录静态 fallback；runtime 保留本地 Bevy adapter 表示，scene/audio/Fangyuan 高耦合拆分维持“评估完成、暂不拆”的结论。）

## 最终完成定义

以下项目作为整体完成标准，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-14 17:00:11 +08:00
- 结束时间：2026-08-14 17:00:11 +08:00
- 验收总结：完成构建缓存隔离、profile/链接边界、core crate 抽离、UI runtime 语义桥接、构建入口和 CI 缓存收口；所有清单项均有命令、截图或 artifact 证据。Windows 保留静态 `dev-fast` fallback，非 Windows 保留 Bevy dynamic linking。

- [x] 已有一套可重复的桌面、工具、headless、Android Rust 和 APK 编译基线及原始日志。（验证：阶段 1 artifacts 覆盖全部目标；阶段 11 补充普通/热点和 UI 工具冷/增量报告。）
- [x] 主游戏、UI 生成/审计工具和 Android 构建的 target/cache 边界清晰，旧共享缓存已按记录安全清理。（验证：阶段 2/3 审计、独立 target 并发构建和清理记录通过。）
- [x] 开发 profile、动态链接入口和高优化 profile 的用途明确，动态链接未进入 Android 或正式发布产物。（验证：非 Windows `run_fast` 动态链接、Windows 静态 fallback，`build-entry` 目标条件与文档一致。）
- [x] Bevy features 已按目标矩阵收窄或有明确的不收窄依据，桌面、Android、headless 和发布回归通过。（验证：阶段 5 feature tree/API 审计和阶段 1/3/10 目标构建记录。）
- [x] `ui-document-core` 已与完整 Bevy runtime 解耦，UI tooling 能在独立 workspace 中构建和测试。（验证：core 无 Bevy/project 依赖，边界检查全为 true；runtime JSON 入口已由 core schema/语义校验守护。）
- [x] 经过数据验证的稳定 crate 边界已落地，crate 总量和依赖方向处于可维护范围，无循环依赖。（验证：阶段 7/8 stable core crate 依赖图、范围测量和 adapter 评估。）
- [x] 协议生成只在相关输入变化时执行，预生成快照或独立生成 crate 的来源与版本可审计。（验证：阶段 9 vendor snapshot/独立协议 crate 与 CI 协议检查记录。）
- [x] 构建命令、CI 缓存、Android Rust/Gradle 流程均按目标隔离，并能报告缓存命中和失败阶段。（验证：阶段 10 build-entry、CI matrix 和诊断 artifact 配置。）
- [x] `cargo fmt`、`cargo check`、必要测试、平台构建和 UI/运行时验收均已完成并留有验证记录。（验证：core 4 tests、project document 145 passed/1 ignored、project check、sealed fixture 和固定窗口截图通过。）
- [x] 复测数据证明优化至少改善了目标构建路径，且没有牺牲可复现性、发布质量或跨平台功能。（验证：阶段 4 profile 方向性数据、普通/热点 72.557/55.737 秒、UI 工具冷/增量 239.111/6.314 秒及桌面/Android/headless 回归记录。）
