# 方圆本地 Codex 生成与审核 Checklist

## 目标

建立开发期本地 Codex 方圆内容创作流程：从结构化任务和受控 catalog context 生成草稿蓝图，经确定性 parse、canonical、audit 和有限修复后，由显式 promotion 操作进入 `project/assets/fangyuan/blueprints/generated/`。

本清单只实现本地开发工具和离线 fixture，不实现游戏内提示词、客户端或服务端在线模型调用、凭据、计费、限流、联网同步，也不保证相同提示词每次得到完全相同的 AI 输出。确定性从已生成并确认的 canonical blueprint 开始。

## 基础原则

- [ ] Codex 负责创作候选；Rust 工具负责 schema、canonical、audit、hash、promotion 和失败退出码。
- [ ] 草稿和运行产物保存在被忽略的 `summary/fangyuan-generation/<run-id>/`，未经 promotion 不进入正式首包资源。
- [ ] promotion 目标固定在 `project/assets/fangyuan/blueprints/generated/`，禁止 Codex 直接覆盖现有 approved 资产。
- [ ] 第一版 primitive 直接写 `surface_material_preset_id`，并可用默认关闭的 `surface_noise_enabled` 开启该 preset 已登记的 noise；不生成对象级 material binding table、noise tile/strength、UV、PBR 任意参数或 Shader。
- [ ] 一个对象优先不超过 4 种 surface preset，按对象和部件分组后由 primitive 复用，不逐 primitive 随机选材。
- [ ] 本清单依赖“方圆图集资源合同与蓝图接入”全部完成，不复制 catalog 或 blueprint 校验逻辑。
- [ ] 每个阶段独立实现、验证和提交，完成后填写阶段元信息。

## 援引文档

以下文档用于说明 checklist 的需求来源和编制依据：

- 文档名称：方圆灵构 AI 生成与图集材质渲染
  - 路径或链接：[AI生成与图集材质渲染](../../docs/方圆灵构/架构/AI生成与图集材质渲染.md)
  - 用途说明：定义 AI 输入、分阶段输出、选材、确定性边界、audit 和离线 fixture。
- 文档名称：方圆灵构生成流程与权限边界
  - 路径或链接：[生成流程与权限边界](../../docs/方圆灵构/规范/生成流程与权限边界.md)
  - 用途说明：限定 AI 能生成的表现内容以及不能控制的玩法强度。
- 文档名称：方圆灵构蓝图格式与生成约束
  - 路径或链接：[蓝图格式与生成约束](../../docs/方圆灵构/规范/蓝图格式与生成约束.md)
  - 用途说明：提供 cube/sphere、坐标、尺寸、role 和 RON 输出合同。
- 文档名称：方圆灵构审核与预算规则
  - 路径或链接：[审核与预算规则](../../docs/方圆灵构/规范/审核与预算规则.md)
  - 用途说明：定义生成结果进入正式资源前的审核和预算门禁。

## 阶段 1：锁定本地生成任务和 run bundle 合同

- 开始时间：2026-08-26 09:42:50 +08:00
- 结束时间：2026-08-26 09:54:29 +08:00
- 开发总结：锁定 provider-neutral 的单对象 v0 task、run bundle、稳定 hash/命名和 promotion 冲突合同；新增严格 JSON Schema 与离线有效 fixture，并将本地 Codex authoring 入口接入方圆文档。未实现 CLI、在线 provider 或网络能力。
- 验证记录：PowerShell `Test-Json -SchemaFile` 验证 `本地生成任务.v1.fixture.json` 通过；`git diff --check` 通过；审阅确认新增合同不包含 provider/model/endpoint/token/绝对路径字段，且未修改 `summary/` 外的代码或既有 `epoch_inheritance.rs` 行尾改动。

- [x] 定义 provider-neutral task schema，至少包含 task id、对象用途、提示词、primitive/bounds/体积预算、允许 kind/role、材质数量上限和禁止项。（验证：`docs/方圆灵构/规范/本地生成任务.v1.schema.json` 固定闭合 v1 字段、预算、cube/sphere、现有 role 词表、4 preset 上限和固定禁止项；有效 fixture 已通过 schema 校验。）
- [x] 定义 `summary/fangyuan-generation/<run-id>/` 内容：原始 task、预处理 task、catalog context、draft、canonical、audit report、promotion decision 和日志摘要。（验证：`本地生成任务与运行包合同.md` 的 Run Bundle 固定 9 类相对 artifact 路径、wrapper schema 和不写成功产物的失败语义。）
- [x] 定义 run id、文件名、schema version、时间字段来源和内容 hash；工具不得臆造 AI provider、模型或网络调用记录。（验证：合同固定 `fygen-v1-YYYYMMDDTHHMMSSZ-xxxxxxxxxxxx`、本地 UTC clock、wrapper schema version、UTF-8 SHA-256 artifact manifest，并明确禁止 provider/model/endpoint/token/network 字段。）
- [x] 定义正式目标 `project/assets/fangyuan/blueprints/generated/<stable-id>.ron` 的稳定命名和冲突策略。（验证：合同限定 snake_case `stable_id`、固定单文件目标、默认拒绝覆盖、内容相同幂等及显式 old hash + 原子更新边界。）
- [x] 锁定 v0 流程为单对象、cube/sphere、最多 4 种 preset、每 preset 最多一个 catalog 建议 noise、无任意 override。（验证：合同和 schema 的 `primitive_kinds`、`max_surface_presets <= 4`、单 suggestion 与 `allow_arbitrary_overrides: false` 一致。）
- [x] 明确同提示词的 AI 输出可以变化，但同一 draft 经 canonical、audit 和 promotion 必须得到相同内容与 hash。（验证：合同“确定性与非目标”明确 prompt 可变、从 draft 字节起稳定的 canonical/audit/decision/hash 边界。）
- [x] 明确 production online provider 为独立后续清单，不在 CLI 中预留未受控 URL、token 或脚本入口。（验证：合同明确生产 provider/凭据/网络审计为后续清单，并限定本地 Codex 仅写当前 run draft。）
- [x] 如本阶段只修改文档/fixture schema，执行格式和路径检查；如修改 Rust，运行 `cargo fmt` 和 `cargo check`。（验证：本阶段无 Rust 变更；`Test-Json -SchemaFile` 和 `git diff --check` 通过。）

## 阶段 2：建立独立 authoring CLI 和构建入口

- 开始时间：2026-08-26 09:55:42 +08:00
- 结束时间：2026-08-26 10:26:10 +08:00
- 开发总结：新增仅开发期启用的 `fangyuan_authoring` bin 与 authoring framework 边界，实现严格 task v1 parser、受控 run bundle、`prepare-task`/`inspect-run` 和为后续阶段保留的安全失败 `validate-draft`/`promote`；接入 Cargo、build-entry 和 CI。未接入 AI provider、网络、canonical、audit 或正式 promotion。
- 验证记录：worker 通过 `cargo fmt --all -- --check`、authoring 8/8、Fangyuan 457/457、`cargo check --locked`、authoring locked build-entry 和 `git diff --check`；主 agent 复跑 `cargo test --locked --lib authoring -- --nocapture`（8/8）与 `build-entry.ps1 -Target fangyuan-authoring -Action check -Locked` 通过。真实 CLI `--help=0`、未知命令=2、非法 run 延后命令=1；仅有仓库既有 warnings。

- [x] 新增独立 `fangyuan_authoring` bin 或等价明确入口，不把 authoring 子命令挤进只负责 artifact 的 `fangyuan_bake` 语义。（验证：`project/src/bin/fangyuan_authoring.rs` 仅包装 `framework::fangyuan::authoring`；现有 `fangyuan_bake` 未变。）
- [x] 在 `project/Cargo.toml` 显式注册 bin，并在 `scripts/build-entry.ps1` 增加可复现的 check/run target；Android 正式包不依赖该工具入口。（验证：Cargo bin 使用 `fangyuan-authoring-tool` feature，authoring module 仅在非 Android target 导出；build-entry 新增 `fangyuan-authoring` target，主 agent locked check 通过。）
- [x] 将 authoring bin 的 locked check 接入 `.github/workflows/build-matrix.yml` 或现有 project 工具矩阵，避免本地可用但 CI 不编译。（验证：Project bins and tests job 新增 `Check Fangyuan authoring bin`，调用 locked build-entry target 并保留 transcript。）
- [x] 提供 `prepare-task`、`validate-draft`、`promote` 和 `inspect-run` 等最小子命令，参数解析、help 和退出码稳定。（验证：`authoring.rs` 定义四命令、严格 option parser/usage/exit code；真实 CLI help=0、unknown=2、延后命令失败=1。）
- [x] 所有路径先解析为明确根目录下的受控目标；拒绝 absolute、drive、URL、目录逃逸、输入输出同文件和覆盖非本 run 的文件。（验证：`canonical_repository_root`、`resolve_repository_input_file`、`open_run` 和 artifact containment 检查受 canonical root 限制；单测拒绝 `../`、URL 及 authoring run 输入重叠。）
- [x] CLI 输出同时提供人类摘要和机器可读 report 文件，不要求调用方解析易变日志文本。（验证：CLI 输出 `format_fangyuan_authoring_cli_report` 摘要；run 内 `logs/summary.json` 和 manifest 按稳定 schema 写入，prepare/inspect 单测覆盖。）
- [x] 失败时保留输入、错误 report 和可重试状态，不生成看似成功的 canonical 或 promotion decision。（验证：无效 task 单测保留 `task.source.json` 与 failure log；延后 validate/promote 只记录失败，断言 canonical/decision 不存在。）
- [x] 为 help、缺参数、非法路径、未知命令、成功退出码和失败退出码增加测试。（验证：authoring 8/8 覆盖 parser help/missing/unknown、路径 boundary、prepare success、invalid task 与 deferred error；真实 CLI 覆盖 0/2/1 退出码。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan`、authoring bin check 和 `cargo check`。（验证：worker 已通过 fmt、Fangyuan 457/457、cargo check locked、build-entry locked；主 agent 重跑 authoring 8/8 与 locked build-entry 均通过。）

## 阶段 3：生成受控 AI catalog context

- 开始时间：2026-08-26 10:27:11 +08:00
- 结束时间：2026-08-26 11:01:07 +08:00
- 开发总结：`prepare-task` 现通过已验证 resolver 导出稳定、脱敏的 catalog context 与候选集；未重新解析 catalog JSON，未导出路径、像素或 noise tile 参数。
- 验证记录：authoring 11/11、Fangyuan 460/460、fmt、locked check、authoring build-entry 与 `git diff --check` 均通过。

- [x] `prepare-task` 调用已完成的 catalog validator/resolver，不通过手写字符串再次解析三份 JSON。（验证：`authoring.rs` 调用 `FangyuanSurfacePresetRegistry::load_from_paths`，`resolver.rs` 在联合校验后导出只读 metadata。）
- [x] 导出紧凑 context，保留 preset ID、名称、描述、material family、preferred roles、primitive kinds、PBR 摘要、projection/render model 和 optional noise 建议。（验证：catalog context schema 与 metadata view 覆盖这些受控字段。）
- [x] context 明确 surface color/normal 同坐标、noise 独立索引、AO 固定 `1.0` 和 preset 参数为语义估算。（验证：context 固定 atlas 语义说明、AO=1.0 与受限 suggestion 摘要。）
- [x] 根据 task 的用途、role、材质关键词和禁止项生成候选集；无法可靠缩减时允许输出完整 256 项，但不得丢失稳定 ID。（验证：关键词、role/purpose、forbidden 过滤和 256 fallback 测试通过。）
- [x] 低置信度选择允许返回最多 3 个候选及理由，最终 draft 仍必须选择一个已登记 preset 或安全默认值。（验证：role/purpose 策略固定 low confidence 前三并带理由；最终 draft 校验留待阶段5。）
- [x] context 不包含本机绝对路径、账号信息、token、任意 URL、Shader 源码或视觉 noise 像素数据。（验证：context 稳定性测试断言不含 atlas 文件名或 noise ID，类型未含上述字段。）
- [x] 同一 task、catalog version 和 hash 生成 byte-stable canonical context，catalog 变化触发新 hash。（验证：canonical JSON、preset ID 排序和 identity sensitivity 测试通过。）
- [x] 为关键词匹配、role 优先、特殊 surface 排除、完整 fallback 和稳定输出增加测试。（验证：新增 authoring context focused tests，authoring 11/11。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan` 和 authoring CLI fixture 测试。（验证：fmt、Fangyuan 460/460 和 authoring 11/11 通过。）

## 阶段 4：固化 Codex 提示词和草稿输出协议

- 开始时间：2026-08-26 11:01:55 +08:00
- 结束时间：2026-08-26 11:26:28 +08:00
- 开发总结：生成受控 Codex task template、草稿协议 receipt 和合法/非法手写 RON fixture；模板只允许当前 run draft 写入，未调用在线模型。
- 验证记录：authoring 14/14、Fangyuan 463/463、fmt、locked check 和 authoring build-entry 通过；`git diff --check` 通过。

- [x] 建立本地 Codex 任务模板，明确依次读取设计文档、预处理 task 和 catalog context，不要求模型扫描不相关仓库文件。（验证：`input/codex-task.md` 固定读取顺序和当前 run 输入。）
- [x] 提示词要求先输出对象计划和部件表，再生成 primitive；部件包含 role、比例、预算和 preset 选择理由。（验证：模板固定 plan/part table 输出协议。）
- [x] 草稿最终只写合法 RON blueprint，primitive 只使用 cube/sphere、position、size、color、role、现有 alpha/emissive/material profile、可选 surface preset 和受控 noise bool。（验证：valid Codex draft fixture 可由 blueprint parser/validate 接受。）
- [x] 禁止草稿包含任意路径、URL、UV、tile index、noise index/strength/target、roughness、metallic、脚本、Shader、rotation 或自由 mesh。（验证：invalid fixture 覆盖禁项且严格 blueprint schema 拒绝。）
- [x] 生成器按部件复用材质，一个对象优先不超过 4 种 preset；water/sky/lava 只有提示词和 role 明确时才可选择。（验证：模板固定复用、4 preset 与特殊表面双重条件。）
- [x] Codex 把草稿写入当前 run bundle，不修改 promotion 目录、catalog、文档、Rust 源码或其他 run。（验证：模板只授权当前 `draft/blueprint.ron`。）
- [x] 记录 prompt template version、task hash、catalog hash 和 draft hash，但不虚构模型确定性或在线 provider metadata。（验证：manifest protocol receipt 和 inspect-run 草稿 SHA-256。）
- [x] 使用手写合格/不合格 draft 验证输出协议能被下一阶段工具可靠消费。（验证：authoring fixtures 和 14 项定向测试通过。）

## 阶段 5：实现 draft canonical、审核和有限修复

- 开始时间：2026-08-26 11:34:56 +08:00
- 结束时间：2026-08-26 12:28:30 +08:00
- 开发总结：实现严格 draft parse、input/catalog hash 绑定、稳定 canonical RON、复用 surface/blueprint audit、有限确定性修复与 blocked/ready decision；失败会清理 stale canonical。
- 验证记录：主 agent authoring 23/23；worker Fangyuan 472/472、fmt、locked check/build-entry 与 diff check 全部通过。

- [x] `validate-draft` 严格解析 RON、验证顶层 schema 和全部 primitive，不直接运行或渲染未校验文本；noise opt-in 只能指向当前 preset 已登记的 suggestion。（验证：strict RON parser、blueprint validation 与 noise repair/reject tests。）
- [x] 复用 Fangyuan blueprint、surface resolver 和 audit，检查 primitive 数量、bounds、体积、role、材质数量、未知 preset 和特殊表面。（验证：authoring 23/23 覆盖 preset/bounds/material/special surface。）
- [x] canonical 明确字段顺序、浮点格式、primitive 顺序策略和换行，重复运行不产生 diff。（验证：canonical writer 固定 LF/字段/浮点并有 byte-stable test。）
- [x] 有限修复只允许确定性动作，例如填默认 preset、移除非法 optional noise、降低不允许的透明/发光建议或拒绝超预算内容。（验证：repair action 仅覆盖这些动作，其他 error 阻断。）
- [x] 有限修复不得替 AI 重构大段几何、猜测新材质、改变玩法强度或绕过 error；需要重新创作时返回可操作问题列表。（验证：不可修复 finding 产生 blocked decision 和 audit 问题列表。）
- [x] audit report 记录原始 finding、修复动作、剩余 warning/error、requested/resolved preset 和最终 hash。（验证：audit report/repair action/requested-resolved/hash 结构及 ready test。）
- [x] 只有零 error 的 canonical 才进入 `ready_for_promotion`，warning 是否允许 promotion 使用明确 policy。（验证：zero-error gate 写 ready，失败写 blocked 并删除 stale canonical；v0 warning 允许 policy 固定于 decision。）
- [x] 覆盖合法 draft、未知 ID、路径注入、reserved noise、超 4 材质、超 bounds、非法 special surface 和重复 canonical 测试。（验证：authoring 23/23。）
- [x] 运行 `cargo fmt`、`cargo test --lib fangyuan`、authoring CLI 集成测试和 `cargo check`。（验证：fmt、Fangyuan 472/472、authoring 23/23、locked check/build-entry 通过。）

## 阶段 6：实现显式 promotion 和防覆盖边界

- 开始时间：2026-08-26 12:29:17 +08:00
- 结束时间：2026-08-26 13:00:45 +08:00
- 开发总结：实现 hash-gated promotion、固定 generated target、显式覆盖预条件、原子写入、promotion report 和幂等语义。
- 验证记录：authoring 27/27、Fangyuan 476/476、fmt、locked check/build-entry 和 diff check 均通过。

- [x] `promote` 只接受当前 run 中 hash 匹配、audit 无 error 且 decision 明确允许的 canonical 文件。（验证：promote 重验 task/catalog/canonical/audit/decision hash 和 Ready 状态。）
- [x] promotion target 必须位于 `project/assets/fangyuan/blueprints/generated/`，并再次经过 Fangyuan 首包路径校验。（验证：固定 generated target/containment/常规文件校验和非法 target test。）
- [x] 默认拒绝覆盖已存在目标；需要更新时要求显式 expected old hash，防止覆盖并发或人工修改。（验证：冲突和显式 hash update tests。）
- [x] 使用临时文件和原子替换或等价安全写入，失败不会留下截断的正式 RON。（验证：temp+sync+atomic rename，Windows `MoveFileEx` replacement。）
- [x] promotion report 记录 source run、task/catalog/canonical hash、target、旧/新 hash、结果和失败原因。（验证：promotion/report schema 和 manifest 收录。）
- [x] promotion 不复制 `summary/` 中的提示词、日志或私有草稿到正式 assets，只写最终 approved blueprint 和必要元数据。（验证：success test 断言固定 target 仅等于 canonical RON。）
- [x] 重复执行相同 promotion 必须幂等；不同内容同 stable id 必须拒绝或走显式更新路径。（验证：idempotent/conflict/explicit update tests。）
- [x] 覆盖非法 target、stale hash、audit failure、已存在目标、写入失败、幂等和成功 promotion 测试。（验证：authoring 27/27。）
- [x] 运行 `cargo fmt`、authoring CLI 集成测试、`cargo check` 和 `git diff --check`。（验证：fmt、authoring 27/27、Fangyuan 476/476、locked check/build-entry/diff check 通过。）

## 阶段 7：建立“青玉机关塔”固定验收 fixture

- 开始时间：2026-08-26 13:01:32 +08:00
- 结束时间：2026-08-26 13:32:19 +08:00
- 开发总结：按受控本地 Codex 草稿协议建立青玉机关塔固定任务和 run bundle；修正首次越界尺寸后，两次 validate 得到相同 canonical hash，并显式 promotion 为正式 generated fixture。新增非法 rotation 草稿拒绝测试和现有 blueprint loader 静态合同测试；一次成功仅证明本地流程与合同，不承诺模型可重复生成相同几何。
- 验证记录：正式 fixture SHA-256 为 `1bfa3dd0b3d2435ce2fef72bccc7fb197a44bba11a06af2cbd69c737cf1413bc`；authoring 28 项、Fangyuan 478 项、`cargo check --locked`、fmt、authoring locked build-entry 与 `git diff --check` 通过。run bundle 位于被忽略的 `summary/fangyuan-generation/fygen-v1-20260826T050943Z-f44140e33a55`；实现/fixture 已提交 `3fc652b`。

- [x] 创建固定任务：青玉机关塔，50 到 150 个 primitive，包含石质底座、金属框架和青玉核心。（验证：`project/src/framework/fangyuan/fixtures/authoring/qingyu_mechanism_tower.task.json`，最终 72 primitives。）
- [x] 要求同时使用 cube、sphere、非均匀 scale、3 种 surface preset，并仅让石质材质通过 `surface_noise_enabled: true` 使用一个已登记 roughness noise 建议。（验证：generated RON 与 `generated_qingyu_mechanism_tower_fixture_loads_under_the_static_contract` 断言 72、cube/sphere、3 preset、非均匀 size 及石质 noise 约束。）
- [x] 固定任务禁止透明、发光、water、sky、lava、triplanar 必需效果和任意 rotation，避免特殊 Shader 阻塞 v0。（验证：task `additional_forbidden_features` 固定这些收紧项；fixture loader contract 验证输出不含特殊表面和禁止视觉字段。）
- [x] 使用本地 Codex 生成至少一个 draft，保留 run bundle；不要把一次成功输出解释为模型可重复性证明。（验证：当前 agent 按 `input/codex-task.md` 协议创作一次 draft；ignored run bundle 已保留，阶段总结明确不承诺重复性。）
- [x] 通过 validate、canonical、audit 和 promotion 得到正式 fixture，并记录最终 primitive/material 数量和 hash。（验证：两次 validate 后 promotion 成功，fixture=72 primitive/3 preset，SHA-256=`1bfa3dd0...cf1413bc`。）
- [x] 再次对同一 canonical 执行 validate/audit，确认结果和 hash 不变；重新生成的新 draft允许不同。（验证：重复 validate 的 canonical SHA-256 两次均为 `1bfa3dd0b3d2435ce2fef72bccc7fb197a44bba11a06af2cbd69c737cf1413bc`。）
- [x] 构造一个故意非法的 fixture，确认工具拒绝路径/UV/未知 preset 或超预算内容且不产生正式资产。（验证：`qingyu_mechanism_tower_invalid_draft.ron` 含 rotation，authoring test 断言 reject 且 target 不存在。）
- [x] 检查正式 fixture 只包含受控字段和稳定 ID，能被现有 blueprint loader 加载。（验证：`blueprint.rs` 生成 fixture loader test 通过，正式 RON 仅有受控 blueprint 字段。）

## 阶段 8：工具链验收、说明和下游交接

- 开始时间：2026-08-26 13:33:27 +08:00
- 结束时间：2026-08-26 13:59:00 +08:00
- 开发总结：完成真实离线 run、catalog stale promotion 拒绝、正式 asset 边界与下游交接文档同步。
- 验证记录：authoring 30/30、Fangyuan 480/480、全量 2182 passed/1 ignored，desktop/headless/bake/authoring locked checks 全通过。

- [x] 从空 run 目录完整执行 prepare-task、Codex 草稿、validate-draft、audit 和 promote，记录每一步命令、输入、输出和退出码。（验证：`fygen-v1-20260826T053759Z-6efcd5f3aef6` 完成链路并 promotion `stage1_contract_fixture.ron`。）
- [x] 验证网络断开、未登录 MyServer 和无在线模型凭据时，本地 Codex/fixture 流程仍可完成。（验证：CLI 无网络/provider/MyServer 参数，测试拒绝相关控制项，真实 run 未使用外部服务。）
- [x] 验证草稿不会被游戏首包自动发现，只有 promoted blueprint 能通过受控路径加载。（验证：run bundle 被忽略；generated RON 通过 loader contract。）
- [x] 验证 catalog hash 变化会使旧 run 进入 stale 状态，不能用旧审核结果 promotion。（验证：promotion stale context test 返回 `promotion_inputs_invalid` 且不写 target。）
- [x] 从仓库根运行 project test、desktop/headless/fangyuan-bake/authoring check，并检查无构建产物进入 Git。（验证：2182 passed/1 ignored；四个 locked target 通过；Git 仅正式 RON/源码/文档。）
- [x] 更新方圆生成文档、资源说明和 authoring CLI 使用说明，明确本地 Codex 与生产在线 AI 的区别。（验证：四份方圆/资源文档同步离线命令、stale 和运行包边界。）
- [x] 检查 `AGENTS.md` 是否需要增加 authoring bin、生成目录和命令；如新增统一入口则同步更新。（验证：`AGENTS.md` 增加 authoring bin、generated 目录和 build-entry 命令。）
- [x] 确认下游 Shader checklist 只读取 promoted canonical blueprint 和 resolver，不读取 prompt、draft 或 AI 日志。（验证：生成/资源文档明确 Shader 仅消费 promoted canonical 与 resolver。）

## 最终完成定义

以下项目作为本清单整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-26 09:42:50 +08:00
- 结束时间：2026-08-26 13:59:00 +08:00
- 验收总结：本地离线 authoring CLI、catalog context、Codex 草稿协议、canonical/audit/repair、hash-gated promotion、青玉机关塔和端到端验收均完成；不含在线 provider。

- [x] 本地 Codex 能在受控 task/context 下生成只含 cube/sphere、稳定 surface preset ID 和受控 noise bool 的草稿蓝图。（验证：task/template/fixture 与青玉 run。）
- [x] 草稿、canonical、audit 和 promotion 产物按 run id 隔离，草稿不会直接进入正式 assets。（验证：fixed run bundle 和 promotion boundary。）
- [x] Rust authoring 工具独立完成 parse、canonical、audit、有限修复、hash 和 promotion，不信任 AI 自报结果。（验证：authoring 30/30。）
- [x] 一个对象默认不超过 4 种 surface preset，材质按部件复用，不逐 primitive 随机分配。（验证：task budget 和青玉 3 preset fixture。）
- [x] 非法路径、URL、UV、Shader、未知 ID、reserved noise、越界几何和超预算草稿被拒绝或确定性降级。（验证：authoring rejection/repair tests。）
- [x] promotion 仅写入 `project/assets/fangyuan/blueprints/generated/`，具备 hash 门禁、防覆盖、幂等和失败恢复。（验证：promotion tests 与真实 run。）
- [x] “青玉机关塔”正式 fixture 已通过生成、审核和 promotion，能由游戏 blueprint loader 加载。（验证：72 primitive fixture、双次 hash 与 loader test。）
- [x] 同一 canonical 的解析、审核和 hash 稳定；文档不承诺相同提示词每次产生相同 AI 几何。（验证：byte-stable test 和文档边界。）
- [x] 整个流程不依赖游戏内在线 AI、MyServer、provider token 或网络连接。（验证：CLI/test/document boundary。）
- [x] 相关测试、构建入口和文档通过验收，正式资源中不包含运行日志、私有草稿或本机路径。（验证：2182 passed/1 ignored、locked targets 和 Git scope。）
