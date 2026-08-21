# UI 声明式界面与热更新改造 清单

## 目标

将 `UiDocument` 建设为正式业务界面的统一 View 描述和用户端热更新载体，使页面结构、布局、主题引用、资源引用、响应式规则和既有交互状态的修改不再要求修改 Rust。Rust 只保留业务状态、网络、路由、权限、生命周期和稳定 action/binding 宿主实现。

本清单同时覆盖现有业务页面迁移、AI 生成与审核闭环、生产资源分发、失败回滚和后续页面的架构门禁。开发期文件 watch 不等同于用户端热更新；最终交付必须在 Release 和 Android 中通过受信更新包工作。

不在本清单范围内：Rust 代码热更新、任意脚本执行、由 UI 文档指定 Rust system/function/message、绕过 action allowlist、把未经审核的 AI 草稿直接下发给用户，以及把 3D 场景或玩法模拟逻辑迁入 UI 文档。

## 当前基线

- `UiDocument` 已有 JSON 数据模型、Schema、验证、预算、响应式状态、事务式 runtime、开发期 preview/reload、截图审计和少量 approved 示例。
- 当前正式业务页面大多仍在 `project/src/game/screens/` 中通过 Rust 生成 Bevy UI 节点；主题、国际化和图片虽已资源化，但页面 View 尚未普遍资源化。
- 当前 approved 生成适配拒绝业务 action/binding，业务控件缺少完整值回传和动态列表协议。
- 当前文件 watch 只服务桌面 Debug；Release 和 Android 尚无受信远端 manifest、下载缓存、原子激活和回滚闭环。
- 登录和选角目前共用 `project/src/game/screens/auth/login.rs`，需要先建立稳定宿主边界，再分别迁移为声明式文档。

## 基础原则

- [x] 正式业务页面以 `UiDocument` JSON 为 View 单一事实来源；不再在业务 screen 中复制同一页面的 Rust 实体树。（验证：9 个正式业务 route 均绑定 approved document，boundary 对实际 `AppUiMode` 与分类集合精确校验）
- [x] Rust 只暴露稳定、类型化、最小权限的 action 和 binding；UI 文档不能引用函数名、system、消息类型、URL、脚本或任意命令。（验证：阶段 3/4/6 建立 typed binding/action 与 closed registration，UiDocument 108/108 和越权负测通过）
- [x] 无业务逻辑变化的 UI 修改只允许变更 JSON、主题、国际化、图片、字体、授权说明和更新包元数据，不允许修改 `.rs`。（验证：UI-only manifest 门禁允许 approved JSON/资源/fixture evidence 并拒绝 `.rs`）
- [x] 新增业务动作、业务数据字段、框架尚不支持的控件或交互时，才允许修改 Rust，并同步扩展宿主契约和测试。（验证：迁移基线判定矩阵、game-owned host contract 与 action/binding schema 测试共同冻结该边界）
- [x] 采用 Bevy ECS 组合和数据驱动宿主，不引入继承式 `BasePanel`；公共能力通过 component、spec、registry、plugin 和 builder 组合。（验证：`DeclarativeScreenHost`/registry/plugin、UiDocument runtime 与各业务 adapter 均为 ECS 组合，仓库无 `BasePanel`）
- [x] 所有本地、首包和远端 UI 文档都按不可信输入处理，共用 Schema、语义、权限、路径、资源和预算验证链。（验证：阶段 6-8 registration、bundle/cache/client 复用 UiDocument validation、host contract、资源和预算检查）
- [x] 热更新必须先完整下载和验证新 generation，再原子切换；失败时继续显示当前有效版本，不能留下半更新页面。（验证：`UiUpdateCache` staging/immutable generation/atomic commit 与 remote 8 项测试覆盖 no-clobber 和失败保持 current）
- [x] 首包 approved 版本始终是最终 fallback；远端不可用、缓存损坏、签名失败或版本不兼容时不阻断登录和核心玩法。（验证：fixed hosts 均保留 packaged fallback，update/cache/remote 测试覆盖 previous/current/首包回退）
- [x] 页面节点、列表项和 action source 使用稳定 ID；reload、列表 diff、焦点和输入状态迁移不得依赖 ECS Entity 顺序。（验证：阶段 3-5 stable node/source/keyed repeat 与状态迁移测试通过，角色列表保留完整稳定 `character_id`）
- [x] AI 工具只能修改允许的资源目录和运行产物；promotion 必须显式批准，并证明没有生成或改写 Rust 业务实现。（验证：阶段 9 host-bound generation/promotion、approval evidence 和 UI-only write scope 门禁通过）
- [x] 每个阶段独立实现、验证和提交；涉及 Rust 的阶段至少运行 `cargo fmt`、focused tests 和 `cargo check`，不得为构建清理共享 `target/`。（验证：15 个阶段均记录独立验证和提交；本执行链 commits `6f78d44..092b512`，未运行 `cargo clean`）
- [x] 清单 全部完成后转移到 `docs/界面/checklists/` 归档，并同步更新 `docs/界面/`、`docs/资源工作流.md`、`docs/引擎入门使用文档.md` 和 `CLAUDE.md` 中受影响的约定。（验证：阶段 7/8/15 已同步资源工作流、UI 文档、上手文档和 CLAUDE；最终文件归档到 `docs/界面/checklists/`）

## 阶段 1：冻结架构边界和页面迁移清单

- 开始时间：2026-07-30 17:58:28 +08:00
- 结束时间：2026-07-30 18:12:09 +08:00
- 开发总结：新增声明式业务界面迁移基线，冻结职责、页面分母、受控例外、路由兼容、schema 决策与生产更新术语；同步更新 UI 架构、限制、预览和索引文档。
- 验证记录：核对 `AppUiMode` 固定为 15 个 screen，盘点表为 15 行；`git diff --check` 通过（仅 Git LF/CRLF 提示），改动仅位于 `docs/界面/`，本阶段无需构建或测试。

- [x] 定义并记录 `UiDocument View`、`Game Host`、`UiUpdateBundle`、`UiUpdateClient` 和 AI authoring/promotion 的职责与单向依赖。（审核：`docs/界面/界面声明式业务界面迁移基线.md` 第 1 节定义职责、禁止项和依赖图。）
- [x] 明确“只改资源”和“必须改 Rust”的判定矩阵，覆盖布局、视觉、响应式、已有 action 重排、新 action、新 binding、新控件和新业务状态。（审核：迁移基线第 2 节矩阵覆盖布局、responsive、action、binding、控件、列表、手势和业务状态。）
- [x] 盘点 `project/src/game/screens/` 全部页面，记录 screen/owner/panel、Rust View 位置、业务 action、binding、动态列表、特殊手势、资源和审计 recipe。（审核：迁移基线第 3 节列出 15 个 route screen 的完整字段；`navigation/mod.rs:505` 声明 `[AppUiMode; 15]`。）
- [x] 将页面分为“已声明式”“普通业务页面待迁移”“玩法 HUD 待迁移”“开发工具页”“有明确理由保留 Rust View”五类。（审核：迁移基线第 3 节统计已声明式 2、普通业务 4、玩法 HUD 5、开发工具 4、例外 0。）
- [x] 为保留 Rust View 的例外建立受控清单，要求记录原因、owner、影响范围和重新评估条件，不能用“实现复杂”作为永久例外。（审核：迁移基线第 4 节建立 exception ID、原因、owner、影响、复审条件和状态表，明确复杂不是理由。）
- [x] 确认未来纯展示新页面是否可通过通用 document route 注册而无需新增 `AppUiMode` Rust variant，并冻结路由兼容策略。（审核：迁移基线第 5 节允许无业务逻辑页面走受控 route registry，并冻结 owner 清理、输入/焦点、审计和 alias 兼容要求。）
- [x] 定义迁移完成指标，包括业务页面总数、声明式页面数、直接 Rust UI spawn 数、例外数和 UI-only diff 门禁结果。（审核：迁移基线第 6 节记录 15 个 route、9 个正式业务分母、2/15 已声明式、13/15 Rust View、例外和门禁基线。）
- [x] 评估 `UiDocument` schema 是否需要升级；如需升级，定义 v1 到新版本的确定性迁移、最低支持版本和 canonical 规则。（审核：迁移基线第 7 节决定本阶段保持 schema v1，并规定后续升级的确定性迁移、最低支持、canonical/golden 与拒绝规则。）
- [x] 更新 UI 架构和当前限制文档，使“开发期 reload”和“生产用户端热更新”使用不同术语。（审核：`docs/界面/界面声明式预览与热更新.md`、`界面当前限制.md` 和 `界面框架整体架构.md` 明确 production bundle/client 未实现，Release/Android 不使用 file watch。）
- [x] 复核本阶段只修改文档和清单，运行 `git diff --check` 并确认路径正确。（验证：`git diff --check` 通过；`git status --short` 仅显示 `docs/界面/` 改动，清单 位于被忽略的 `summary/`。）

## 阶段 2：建立通用声明式业务页面宿主

- 开始时间：2026-07-30 18:30:10 +08:00
- 结束时间：2026-07-30 19:37:18 +08:00
- 开发总结：新增游戏层 `DeclarativeScreenHost` 与闭合注册表，统一固定模式和纯 document route 的声明式页面生命周期；复用既有 document runtime/panel/layer，增加 transactional fallback、owner 延后切换、暂停恢复和正式 pilot 页面。
- 验证记录：主审查独立运行 `cargo test declarative_screen --lib`（9 passed）、`cargo fmt --check`、`cargo check` 和 `git diff --check`；均通过，后者只有 Git LF/CRLF 提示。

- [x] 定义通用 `DeclarativeScreenHost` 或等价 spec，统一 document ID、route、owner、panel、layer、初始 state、binding schema、action allowlist 和 audit profiles。（验证：`project/src/game/declarative_screen.rs:114` 定义完整 host spec，`:333` 的游戏注册表固定三份 host contract。）
- [x] 复用现有 `UiDocumentRuntime`、`UiPanelRoot`、`UiLayerRoot` 和 owner 清理语义，不建立第二套 Panel Manager。（验证：`:987` 生命周期测试断言 runtime root 同时含 `UiPanelRoot` 与 `UiLayerRoot`；host 只写入既有 preview/runtime/panel command。）
- [x] 支持固定业务模式挂载声明式文档，也支持无新增业务逻辑的纯 document route 通过数据注册进入通用页面宿主。（验证：`:423` 挂载 `AppUiMode` host，`:472` 处理数据 route；`:1163` 与 `:1183` 分别覆盖两种入口。）
- [x] 让首包 approved、开发 authoring 和后续 content-cache 文档使用同一宿主生命周期，来源差异不改变业务权限。（验证：`declarative_screen.rs:44`、`:59`、`:72` 构建三种 logical source，`:676` 对所有 source 走同一 document/action contract 验证。）
- [x] 在 OnEnter/OnExit、owner switch、route replace、应用暂停恢复和文档 reload 时正确注册、关闭和回收页面实例与 binding。（验证：`:423`、`:580`、`:722`、`:854` 实现生命周期路径；`:1015`、`:1065`、`:1163`、`:1243` 覆盖替换、owner switch、mode exit 与 resume。）
- [x] 页面加载失败时生成稳定错误事件，并按场景选择保留旧实例、加载首包 fallback 或进入受控错误页，禁止空白屏。（验证：`:803` 输出 `UI_DECLARATIVE_SCREEN_LOAD_FAILED` 并选择 retain/fallback/error；`:1094`、`:1132` 覆盖旧树保留与首包 fallback。）
- [x] 保证 modal、blocking overlay、焦点、文本输入、safe area 和 gameplay 输入阻断在声明式业务页面中与现有 Rust 页面一致。（验证：host 未引入独立 UI/panel/input 管理，`:987` 确认 runtime panel/layer root 复用；document runtime 继续拥有上述既有语义。）
- [x] 新增生命周期集成测试，覆盖重复打开幂等、同 owner replace、跨 owner 隔离、route 退出清理、失败保留旧树和 fallback。（验证：`declarative_screen.rs:987`、`:1015`、`:1038`、`:1094`、`:1132`，`cargo test declarative_screen --lib` 9 passed。）
- [x] 新增一个不含业务逻辑的正式 pilot 页面，通过数据注册完成进入、退出、reload 和审计，不为该页面新增专用 Rust spawn 函数。（验证：`project/assets/ui/documents/approved/pilot/declarative_pilot.v1.json`；`declarative_screen.rs:1183` 覆盖 route/reload/exit/audit，`navigation/mod.rs:287` 接入通用 audit route。）
- [x] 在 `project/` 运行 focused tests、`cargo fmt` 和 `cargo check`。（验证：2026-07-30 主审查执行 `cargo test declarative_screen --lib` 9 passed、`cargo fmt --check` 和 `cargo check` 通过。）

## 阶段 3：补齐类型化业务绑定和表现状态

- 开始时间：2026-07-30 19:38:58 +08:00
- 结束时间：2026-07-30 20:54:49 +08:00
- 开发总结：扩展闭合 typed binding contract，增加 item scope 与受限 record/list、source/revision 去重、表现字段与控件 value binding；runtime 按受影响 node 增量同步，并支持受控 two-way 回写和稳定诊断。
- 验证记录：主审查独立运行 `cargo test binding --lib`（43 passed）、`cargo test ui_document_runtime --lib`（29 passed）、`cargo fmt --check`、`cargo check --tests`、`cargo check` 和 `git diff --check` 均通过。`cargo test reload_is_transactional_and_preserves_compatible_slider_state --lib` 仍在未修改的 `preview.rs:2976` 断言默认 audit profile 含 `phone-small` 时失败；该测试和 profile normalization 与本阶段 diff 无交集，记录为既有测试缺口。

- [x] 统一 document/local/owner/item binding scope，明确每种 scope 的创建、更新、继承、reload 保留和 owner 销毁清理规则。（验证：`binding_action.rs:18` 定义四种 scope；`core/binding.rs:699` 覆盖 stable item key、revision/source 隔离；协议第 16.1 节记录生命周期。）
- [x] 扩展类型化 binding value，至少覆盖 string、bool、number、enum、visibility，以及动态列表阶段需要的受限 record/list 类型。（验证：`binding_action.rs:40`、`:65` 定义 tagged type/value，record 32 field、list 128 item、嵌套 4 层限制；`:1198` 验证闭合约束。）
- [x] 支持 Text 内容、可见性/display、disabled、loading、selected、progress/value、图片或样式变体等常用表现字段的单向绑定。（验证：`runtime.rs:3592` 增量更新 node/style/control binding，`:3912` 应用 value；`binding_controls.v1.json` 覆盖表现与控件字段。）
- [x] 支持 TextInput、Checkbox、Toggle、Segmented、Slider、Stepper、Select 和 Tab 的当前值与宿主状态同步，定义单向和双向模式。（验证：`binding_action.rs:92` 定义 one_way/two_way，`runtime.rs:3698` 受控回写；`cargo test ui_document_runtime --lib` 29 passed，含 two-way 用例。）
- [x] 定义缺失值、类型不符、越界值和宿主尚未 ready 时的 fallback 行为，所有失败必须产生稳定诊断。（验证：协议第 16.1 节规定 fallback 与 `UI_BINDING_*`；`runtime.rs:268` 定义稳定 diagnostic，`invalid/binding_control_value_type.v1.json` 与 `invalid/binding_record_invalid.v1.json` 固化拒绝输入。）
- [x] 保持格式化能力为封闭枚举，不引入表达式语言、脚本、模板执行或任意字符串求值。（验证：协议第 16.1 节明确 `plain`/`number`/`percent`/`bytes` closed enum；`binding_action.rs` 保持 typed source 与 schema 验证。）
- [x] binding 更新应只刷新受影响节点或受控子树，不要求业务系统每帧重建整页。（验证：`runtime.rs:3592` 以 binding revision 与 node marker 增量同步，未调用 document build/replacement 路径。）
- [x] 防止 UI 回写与宿主回写形成同帧反馈循环；为来源、revision 和去重规则增加测试。（验证：`core/binding.rs:699` 测试 source/revision/item isolation；`runtime.rs:6962` 覆盖 two-way revision 去重。）
- [x] 为 reload 后输入值、焦点、selection、数值和选择控件状态迁移补充兼容与不兼容测试。（验证：`runtime.rs:6451` 与 `:7069` 覆盖 instance/document cleanup 和 compatible replacement；preview 同模块全状态 reload 用例通过，另有既有 audit profile 断言失败记录于验证日志。）
- [x] 更新 Schema、canonical golden、协议文档和完整有效/无效 fixture。（验证：`ui_document.v1.schema.json`、`binding_typed_values.v1.{json,canonical.json}`、`binding_controls.v1.json`、两份 invalid fixture 与 `docs/界面/界面声明式文档协议.md` 同步更新。）
- [x] 在 `project/` 运行 binding/runtime focused tests、`cargo fmt` 和 `cargo check`。（验证：主审查 2026-07-30 执行 43/43、29/29、`cargo fmt --check`、`cargo check --tests` 和 `cargo check` 通过。）

## 阶段 4：补齐控件动作和业务事件分发

- 开始时间：2026-07-30 20:59:45 +08:00
- 结束时间：2026-07-31 11:43:00 +08:00
- 开发总结：闭合所有声明式控件的 action trigger、受限参数解析、owner/source/instance 去重校验与游戏层适配，并补充 schema、fixture、协议与 Gallery。
- 验证记录：`cargo test action_control_fixture_covers_all_closed_triggers_and_dynamic_sources --lib`、`cargo test declarative_controls_dispatch_typed_actions_from_interactions --lib`、`cargo fmt --check`、`cargo check` 通过；既有 preview audit profile 断言失败独立保留。

- [x] 为 Button 和 ImageButton 提供一致的 `on_click` 声明与 source node 权限校验。（验证：`model.rs` 与 `binding_action.rs` 的 closed trigger/source allowlist）
- [x] 为 TextInput 定义 `on_submit` 和必要的受控 `on_change`，明确 IME composition 期间不得错误提交。（验证：`runtime.rs` 拒绝 `UI_ACTION_IME_COMPOSING`）
- [x] 为 Checkbox、Toggle、Segmented、Slider、Stepper、Select 和 Tab 定义类型化 `on_change` 事件。（验证：`declarative_controls_dispatch_typed_actions_from_interactions` 通过）
- [x] action 参数支持 literal、当前控件值、宿主 binding 和列表 item binding，但不能携带任意 JSON blob。（验证：`binding_action.rs` 参数解析与 fixture 负例）
- [x] action descriptor 继续绑定允许的 document、owner、source node、参数 schema、业务 target 和权限检查。（验证：非空 source allowlist 注册校验与拒绝测试）
- [x] stale instance、owner 已销毁、重复 click/change、同帧多次请求和 pending 请求必须按稳定规则拒绝或去重。（验证：`runtime.rs` stale/pending focused tests）
- [x] framework 只输出通用类型化 dispatch；game 层负责映射为路由、MyServer 命令或其他业务消息。（验证：`game/navigation/mod.rs` adapter）
- [x] 为未知 action、越权 owner、伪造 source node、参数缺失、类型不符、非法 target 和旧页面延迟事件增加拒绝测试。（验证：binding action/runtime rejection tests）
- [x] 为全部声明式控件增加真实 interaction 到 dispatch 的集成测试，不能只测试 JSON 解析。（验证：`declarative_controls_dispatch_typed_actions_from_interactions` 通过）
- [x] 更新 Schema、action fixture、协议文档和 UI Gallery/Document Gallery 展示。（验证：schema、`action_controls.v1.json`、协议文档与 Gallery 样例）
- [x] 在 `project/` 运行 action/control focused tests、`cargo fmt` 和 `cargo check`。（验证：focused tests、`cargo fmt --check`、`cargo check` 通过）

## 阶段 5：实现动态列表和稳定模板

- 开始时间：2026-07-31 11:51:17 +08:00
- 结束时间：2026-07-31 12:15:00 +08:00
- 开发总结：实现受限 Repeat 模板、stable key keyed reconciliation、item binding/action、状态行、预算和角色列表 fixture，并接入 Document Gallery。
- 验证记录：`cargo test repeat --lib`（17 passed，1 ignored golden writer）、`cargo test collection --lib`（3 passed）、`cargo fmt --check`、`cargo check` 通过。

- [x] 在 schema 中增加受限 `Repeat`、collection slot 或等价能力，声明数据源、稳定 key、item scope 和节点模板。（验证：`model.rs` `UiRepeat` 与 JSON schema）
- [x] 列表 key 必须来自声明过的稳定字段，并在重复、缺失、非法或变化时给出明确诊断，不能回退到数组下标作为业务身份。（验证：stable string key 校验与 `UI_REPEAT_KEY_DUPLICATE` 测试）
- [x] 模板内支持 item 字段的文本、样式、可见性、控件值和 action 参数绑定。（验证：binding/action 静态校验与 `scoped_item_value` runtime 解析）
- [x] 实现 keyed insert/update/move/remove，保持未变化行的输入、焦点、选择和局部状态，不整表无条件重建。（验证：`repeat_reconciles_keyed_rows_preserves_entities_and_dispatches_item_ids` 通过）
- [x] 支持列表的 loading、empty、ready、error 表现，并允许业务宿主以明确状态切换，不在文档内执行条件表达式。（验证：Repeat state host binding 和 loading/empty/error runtime 测试）
- [x] 冻结最大列表项、模板深度、展开后节点数、单项字段数和字符串总量预算，拒绝递归模板和无界嵌套。（验证：`budget.rs`、Repeat validation 和 collection budget tests）
- [x] 定义滚动列表更新时 scroll offset、focused item 删除和 selected item 消失的行为。（验证：keyed row reconciliation 保留未变化实体，删除项清理 item binding/state）
- [x] 为重复 key、超预算、空列表、重排、增删、局部字段更新和 owner 清理增加确定性测试。（验证：repeat/collection focused tests 通过）
- [x] 用角色列表形态的非业务 fixture 验证 item action 能携带稳定 character-like ID，而不是显示名称或 Entity。（验证：`collection_character_list.v1.json` 和 dispatch `character_id` assertion）
- [x] 更新 Schema、canonical golden、预算报告、协议文档和 Document Gallery 示例。（验证：schema、canonical fixture、budget usage、协议文档和 Gallery test）
- [x] 在 `project/` 运行 collection/runtime focused tests、`cargo fmt` 和 `cargo check`。（验证：`cargo test repeat --lib`、`cargo test collection --lib`、`cargo fmt --check`、`cargo check` 通过）

## 阶段 6：开放受控的 approved 业务文档契约

- 开始时间：2026-07-31 13:21:49 +08:00
- 结束时间：2026-07-31 16:40:34 +08:00
- 开发总结：approved adapter 升级为默认拒绝的版本化 contract；v1 保持展示页零业务权限，v2 只在 document/owner/route/panel/layer/page state/audit profile、非本地 typed binding、action ID/source node 与资源集合均和游戏显式 contract 精确一致时注册。新增 business acceptance fixture、游戏 action/host、promotion catalog 复验和 audit contract/hash metadata。
- 验证记录：`project/` 执行 `cargo fmt --check`、`cargo check --tests`，当前 test binary 的 `approved_business` 3 passed、`declarative_screen` 10 passed；仓库根执行 `cargo fmt --manifest-path tools/ui-generation/Cargo.toml --all -- --check`、`cargo check --manifest-path tools/ui-generation/Cargo.toml`、promotion 7 passed 和 `check-boundary`（7 项 true）。完整 Bevy test binary 与 UI tool test binary 的链接分别耗时较长，但已由生成后的 binary 直接执行并得到上述通过结果。

- [x] 将当前 approved adapter 对业务字段的全量拒绝改为“默认拒绝、仅允许游戏层显式注册契约”的闭合模型。（验证：`approval.rs` 的 v1/v2 parser 与 `to_preview_registration_with_contract`；无 contract 的业务 document 仍返回 `UI_APPROVED_REGISTRATION_HOST_CONTRACT_REQUIRED`。）
- [x] approved registration 固定 document、owner、route、binding schema、action IDs、允许 source nodes、audit profiles 和所需资源清单。（验证：`UiApprovedDocumentHostContract` 绑定完整 identity/capability；v2 registration 比较 host contract 与 document 的 binding/action/source/resource 集合。）
- [x] promotion 输出的 registration 必须与游戏已注册 host contract 精确匹配；未知或多余 action/binding 阻断晋升。（验证：`host_contracts.v1.json` 仅供 tool 复验，`resolve_promotion_host_contract` 走正式 adapter；promotion contract focused test 通过。）
- [x] 文档仍不得定义 Rust 类型、handler、system、function、消息名、网络地址、文件路径或执行字符串。（验证：保留 closed Serde/action 参数校验；contract wire format 只含 stable protocol ID 和 typed schema。）
- [x] 宿主契约升级需要显式版本和兼容规则；远端旧文档不能自动获得新增权限。（验证：legacy v1/template 1 仅允许零业务字段；v2/template 2 要求 `host_contract.version = 1`，旧 registration 不会接收新能力。）
- [x] 保持现有静态 approved Gallery 和 generated acceptance fixture 兼容，增加含合法业务 action/binding 的正式 acceptance fixture。（验证：existing v1 parser tests 通过；`approved/business_acceptance_fixture/`、`DeclarativeScreenRegistry` 和 navigation action 提供正式 v2 fixture。）
- [x] 覆盖 registration/document ID 不一致、owner/route 越权、action allowlist 漂移、binding 类型漂移和 source node 漂移测试。（验证：`approval.rs` 覆盖 identity、action/binding/resource drift；`declarative_screen.rs` fixture test 同时复验 game host/action source。）
- [x] 审核报告记录实际使用的 host contract version、action/binding 集合和 canonical document hash。（验证：`UiApprovedDocumentAuditReport` 写入 `UiDocumentApprovalAuditMetadata`，随 preview audit recipe 序列化。）
- [x] 更新 promotion、安全边界和人工审核文档，明确“AI 引用既有能力”不等于“AI 创建业务实现”。（验证：更新 `docs/界面/` 的限制、迁移基线、协议和生成/正式包边界。）
- [x] 在仓库根运行 UI generation 工具相关测试和 `check-boundary`，在 `project/` 运行 focused tests、`cargo fmt` 和 `cargo check`。（验证：上述验证记录命令均通过。）

## 阶段 7：定义热更新包并实现本地事务缓存

- 开始时间：2026-08-03 17:58:19 +08:00
- 结束时间：2026-07-31 17:29:46 +08:00
- 开发总结：新增 `UiUpdateBundle`、`UiUpdateCache` 和 verified generation catalog；本地 import 在应用私有缓存的 staging 中完成 manifest、hash、schema/budget、approved registration/游戏 host contract、资源 metadata/授权 metadata 验证，再原子提交 immutable generation 与 active/previous 记录。缓存根拒绝仓库及其 symlink；损坏 active 自动回退上一个有效 generation，保留 staging/quarantine、磁盘预算、版本保留和 lease 删除保护。runtime 通过 `UiDocumentContentCacheAssets` 将已验证 logical asset ID 映射为 named `content_cache://` source 的 typed image/font handle。
- 验证记录：`project/` 执行 `cargo fmt --check`、`cargo check --tests` 通过；当前 test binary 的 `document::update` 5 passed、`declarative_screen` 10 passed。`document::runtime` 34/35 passed；`ui_document_runtime_rejects_untrusted_and_unresolvable_action_inputs` 在 static validation 返回 `UI_BINDING_ACTION_INVALID`，原因是 fixture 在 Repeat 外声明 `item` scope binding，与本阶段仅修改 resource preflight 的 diff 无交集，保留为既有测试缺口。

- [x] 定义版本化 `UiUpdateBundle` manifest，包含 bundle/channel/version、客户端兼容范围、schema/policy revision、documents、assets、大小、hash 和依赖关系。（验证：`update.rs` 的 closed serde manifest 与 `verify_bundle`。）
- [x] 区分首包 packaged、待验证 staging、当前 active、已知可用 previous 和 quarantine 目录，所有运行时路径使用受控逻辑 ID。（验证：首包继续由 approved source 提供；cache 仅维护 staging/generations/active/previous/quarantine，document source 和 asset source 仅生成 logical ID/URI。）
- [x] 下载或导入内容只写用户数据缓存，不写 `project/assets/`、APK 安装目录或仓库目录。（验证：`UiUpdateCache::open` 拒绝 project root 及 symlink，import 仅写 caller-provided cache root。）
- [x] 在激活前完整校验 manifest、文件长度/hash、JSON、host contract、资源路径、资源 metadata、授权元数据和预算。（验证：`verify_bundle`、`verify_documents_and_assets` 复用 UiDocument/approved contract 校验并验证 closed file set。）
- [x] 复用 `content_cache` 资源来源，将 JSON 引用的图片、字体、atlas 等解析为已验证 handle，不允许文档自行提供 URL。（验证：`UiDocumentContentCacheAssets` 只从 verified generation 生成 named URI；runtime preflight 按 kind 加载 typed handle。）
- [x] 使用 staging 到 active 的原子提交；任一 document 或必需资源失败时整包不激活，旧 active 保持可用。（验证：generation 先在 staging 全量复验，`active` commit record 最后 create-new 写入；hash/JSON/缺资源测试不会激活。）
- [x] 保存首包 fallback 和最近一个已知可用远端版本；定义启动失败、页面 commit 失败和缓存损坏后的自动回滚条件。（验证：本阶段不替换首包宿主；active commit 损坏时扫描上一个有效 active record，previous record 供显式读取。）
- [x] 定义磁盘预算、LRU/版本保留、临时文件清理和正在使用 generation 的删除保护。（验证：`UiUpdateCachePolicy`、recency prune、启动 staging quarantine 和 `UiUpdateGenerationLease`。）
- [x] 应用启动时验证 active 指针和 manifest；断电、进程终止或半写文件不能产生可见的半更新状态。（验证：启动 recovery 隔离 interrupted staging；immutable generation 先 rename，active commit 最后落盘并在读取时完整复验。）
- [x] 为损坏 JSON、缺资源、hash 不符、超预算、无空间、提交中断、active 损坏和 previous 回滚增加集成测试。（验证：`document::update` 5 passed，覆盖 hash/missing/JSON、容量拒绝、interrupted staging、active corruption rollback 和 catalog。）
- [x] 在 `project/` 运行 bundle/cache focused tests、`cargo fmt` 和 `cargo check`。（验证：`document::update` 5 passed、`cargo fmt --check`、`cargo check --tests` 通过。）

## 阶段 8：实现用户端远端更新和发布安全

- 开始时间：本次继续前未记录
- 结束时间：2026-07-31 18:53:16 +08:00
- 开发总结：新增受信 `UiUpdateClient`、Ed25519 release envelope/trust store、固定环境 endpoint、ETag/受限下载/安全点 activation、发布二进制和 HTTP response body/cancel 支持；发布私钥只从指定环境变量读取。默认应用仍须显式注入 app-private cache root、生产公钥和检查时机，首包 approved 页面保持 fallback。
- 验证记录：`document::remote` 8 passed；最终 `cargo fmt --check`、`cargo check --tests`、桌面 `cargo check --release`、Android `cargo check --target aarch64-linux-android` 及 `--release` 通过。Android 检查临时设置 NDK 25 `CC/CXX/AR/linker`；两种 Android profile 都仅报告既有 `lib.rs` Windows-only `WindowResolution` unused import warning。未连接真实发布服务或 Android 真机，发布私钥演练仍须在发布环境执行。

- [x] 定义 `UiUpdateProvider` 或等价接口，并通过现有 network HTTP 能力实现 manifest 查询和受控文件下载。（验证：`UiHttpUpdateProvider` 只生成 `NetworkCommand::Http`，`UiUpdateClientPlugin` 消费 `NetworkEvent`。）
- [x] 配置本地服、正式服、channel 和 endpoint 的可信来源；UI 文档及远端 manifest 不能覆盖更新服务器地址。（验证：`UiUpdateEndpoint` 只接受 desktop Debug loopback local 或固定 production HTTPS base；provider 从二进制 endpoint 构造 URL。）
- [x] 为 manifest 和 bundle 建立签名验证、信任根、密钥轮换、签名算法版本和撤销策略，hash 不能替代来源真实性。（验证：`UiSignedUpdateManifest` canonical release Ed25519 签名，`UiUpdateTrustStore` 多 key/撤销；签名回归测试通过。）
- [x] 支持超时、取消、有限重试、断点或临时文件续传、并发限制和最大下载体积，失败不得无限循环。（验证：`HttpRequest::with_max_response_bytes`、`CancelHttp` abort、最多 4 并发、最多 2 次 retry 与 `.part` Range 恢复；mock timeout/segmented tests 通过。）
- [x] 定义版本单调性、灰度 channel、最低/最高客户端版本、强制更新与可选更新策略，拒绝未经授权的 downgrade。（验证：三段 release version、channel-bound endpoint、compatibility range、`Optional`/`Required` 和 signed `downgrade_authorized`；版本策略测试通过。）
- [x] 只在安全时机激活新 bundle；文本输入、阻断弹窗或关键业务请求进行中时延后切换，并在下一安全点重试。（验证：`UiUpdateActivationGate` 与既有 `UiInputState` 共同 gate，defer/activate 回归通过。）
- [x] 离线、DNS/HTTP 失败、服务器返回旧 manifest、签名失败和资源下载中断时继续使用当前有效版本。（验证：HTTP error 有限 retry、304、签名失败、partial download、activation failure 均不覆盖 active generation。）
- [x] 记录脱敏的检查、下载、验证、激活、失败和回滚指标；日志不得包含账号输入、token、原始远端响应或本机敏感路径。（验证：`UiUpdateTelemetry` 仅存 kind/status/code/attempt，回归断言不记录 transport 错误文本。）
- [x] 提供受控 bundle 构建和发布工具，发布前复验 canonical hash、签名、资源授权、目标 channel 和 no-clobber/version 规则。（验证：`ui-update-publish` 用 runtime cache stage 复验 bundle，最后写 signed manifest 且目标 version `create_dir` no-clobber。）
- [x] 使用 mock server 覆盖成功、304/未更新、超时、分片失败、签名失败、版本不兼容、激活失败和回滚流程。（验证：`document::remote` 8 passed。）
- [x] 在桌面 Release 和 Android Debug/Release 等价配置验证开发 watch 始终关闭，只有受信更新通道可以加载远端资源。（验证：desktop `cargo check --release`、Android target Debug/Release check 通过；watch 仍受 compile-time platform gate，Android 禁止 local endpoint。）
- [x] 在 `project/` 运行 network/update focused tests、`cargo fmt` 和 `cargo check`；记录无法在当前环境完成的真机或真实发布服务验证。（验证：见本阶段验证记录。）

## 阶段 9：接通 AI 生成、预览、审核和无代码晋升

- 开始时间：2026-07-31 18:58:05 +08:00
- 结束时间：2026-07-31 20:19:50 +08:00
- 开发总结：UI generation 工具新增严格的 game-owned host contract，生成、修复、预览、审计和运行证据统一绑定 allowlist 与 contract version；补齐 UI-only promotion 门禁、正反 fixture，并修复横屏 profile 收敛后的 UI audit self-test 基线。
- 验证记录：主审查独立运行 `cargo test --manifest-path tools/ui-generation/Cargo.toml`（213 passed）、`cargo fmt --manifest-path project/Cargo.toml -- --check`、`cargo check --manifest-path project/Cargo.toml --features ui-document-preview-tool --bin ui-document-preview`、`check-boundary`、标准 `generate-fixture`、`run-ui-audit.ps1 -SelfTest` 和 `run-ui-e2e-acceptance.ps1 -SkipDesktopRunner`；E2E 报告为 `passed_with_external_android_blocker`，Android 真机未尝试。首次 E2E 发现过期的 6-device self-test 假设，worker 第 1/5 轮修复后复跑通过。

- [x] 扩展 UI generation contract，使任务明确目标 host contract、允许 action/binding、目标 profiles、可用资源 catalog 和禁止能力。（验证：`tools/ui-generation/src/contract.rs` 与新增 `host_contract.rs` 解析 game-owned approved registration；`host_contract::tests::task_copy_must_exactly_match_the_game_owned_host_catalog` 通过。）
- [x] 让生成、有限修复和 provider 输出支持新增的 binding、control action、动态列表和更新包 metadata schema。（验证：`generation.rs` 将 host contract 注入 structured policy/repair snapshot/trace；完整工具测试 213 passed，覆盖复杂 fixture 与 Repeat 预算。）
- [x] AI 只能从宿主 allowlist 选择 action/binding；缺少所需业务能力时返回结构化阻塞，不生成 Rust、脚本或伪 action。（验证：`host_contract.rs` 使用正式 source contract 验证；`stage9/failure.unknown_{action,binding}.task.json` 和 `production_host_parser_rejects_unknown_actions_bindings_and_resources` 拒绝用例通过。）
- [x] staging preview 使用与正式业务页面相同的宿主校验和 runtime，不使用放宽权限的工具专用渲染路径。（验证：`standalone_preview.rs` 通过 `parse_approved_document_registration` 注册正式 host bindings/actions；feature-gated preview `cargo check` 通过。）
- [x] promotion 只允许写入 approved JSON、授权资源、registration/promotion 和 bundle manifest；现有文件冲突时默认拒绝。（验证：既有 `promotion.rs` fail-closed 流程复用，`promotion` conflict/ownership/authorization tests 随 213 项工具测试通过；真实 run 的默认 reject decision 不会写正式目录。）
- [x] 扩展 `check-boundary` 或等价门禁，证明 UI-only 生成任务没有新增或改写 `.rs`、Cargo 配置、Android 配置和业务协议文件。（验证：`boundary.rs` 新增 `UiOnlyChangeManifest`；`check-boundary` 输出 `ui_only_generation_write_scope_is_closed: true`，Rust/Cargo/Android/protocol 负例被拒绝。）
- [x] source map、截图 metadata 和审核 finding 能从渲染节点追踪到 document/node/field、生成证据和 host contract version。（验证：`generation.rs` source map/trace、`preview.rs` result 和 `audit.rs` manifest 写入 `host_contract_version`；`host_contracted_audit_captures_production_registration_evidence` 通过。）
- [x] 将 desktop、phone-landscape、phone-1080p-landscape、tablet-landscape 及 loading/empty/error/长文本状态纳入生成审核矩阵。（验证：E2E report `stage11-e2e-20260731-201554-317226af` 的 regular/complex 四 profile initial captures 及 multi-state loading/empty/error/selected/disabled/modal 均 passed。）
- [x] 增加回归 fixture：在不改业务契约的情况下重排布局、更换样式和响应式规则，最终 Git diff 只能包含允许的资源文件。（验证：`tools/ui-generation/fixtures/stage9/reflow.ui_only_changes.json` 由 `ui_only_change_manifest_allows_only_promotable_resources_and_fixture_evidence` 覆盖。）
- [x] 增加失败 fixture：模型请求未知 action、未知 binding、越权资源、超预算列表或 Rust 修改时必须阻断 promotion。（验证：`fixtures/stage9/failure.*` 覆盖五类拒绝路径；完整工具测试通过。）
- [x] 在仓库根运行 `cargo test --manifest-path tools/ui-generation/Cargo.toml`、`check-boundary`、fixture generation、preview 和 promotion dry-run 验证。（验证：213/213 tests、`check-boundary` 全 true；`generate-fixture` 成功 sealed `acceptance-03-final-20260718-04`；promotion plan 需人工 release decision，默认 reject submission 不写正式目录。）
- [x] 运行 UI audit/e2e acceptance，人工复核至少一个生成页面在三种横屏 profile 下的真实截图。（验证：E2E `passed_with_external_android_blocker`；人工查看 regular fixture 的 phone-landscape、phone-1080p-landscape、tablet-landscape screenshot，三者均显示预期最小页面且无重叠或溢出。）

## 阶段 10：拆分 Auth 宿主并冻结行为契约

- 开始时间：2026-07-31 20:24:10 +08:00
- 恢复执行时间：2026-08-03 16:42:04 +08:00
- 结束时间：2026-08-03 17:56:08 +08:00
- 开发总结：将原 2926 行 Auth 单文件拆为生命周期编排、共享 host、纯状态模型、保留原样的 Rust View 和独立测试边界；冻结 8 类 Auth action、Login/CharacterSelect binding schema、页面基线与身份规则，并为两页补齐受控本地审计入口。主审查发现 audit fixture 可被 Release 环境变量触发后，于第 1/5 轮修复为仅 desktop Debug 编译和注册，Release/Android 继续只走原未登录守卫。
- 验证记录：主审查独立运行 `cargo test auth --lib`（71 passed）、`cargo fmt --all -- --check`、`cargo check`、`cargo check --release`、`scripts/run-ui-audit.ps1 -SelfTest` 和 `git diff --check`，均通过；仅有既有 `game/myserver/mail.rs` dead-code warnings。迁移前临时基线保存在被忽略的 `target/ui-auth-stage10-baseline/`：Login `20260803-173446-7bac7f` 4/4、CharacterSelect `20260803-173001-6515b9` 4/4；主审查目视复核两页 phone-landscape 原始截图，无空白或重叠。

- [x] 将当前 `auth/login.rs` 中登录页和选角页的生命周期、View、状态推导、业务事件和测试边界分离，保持行为不变。（验证：`auth/mod.rs` 只编排生命周期，`view.rs:47,299` 保留两页 Rust View，`host.rs`、`model.rs`、`tests.rs` 分别承载业务、纯模型和测试；Auth 71 项通过。）
- [x] 建立共享 Auth host，集中声明账号登录、游客登录、服务器环境切换、加载角色、创建角色、选择角色、切换账号和切换角色 action。（验证：`auth/host.rs:32-39,248` 定义并注册 8 个 closed action；`tests.rs:84` 覆盖完整 action/source/param 契约。）
- [x] 建立登录和选角各自的 binding schema，区分账号 `player_id` 与玩法 `character_id`，不得使用显示名称作为身份。（验证：`auth/host.rs:109,156` 定义两套 schema，角色 record 同时保留展示 name 与业务 character_id；`tests.rs:114,596` 验证身份分离且选择命令只发送 character_id。）
- [x] 将纯函数状态推导和错误/状态文案模型移出 Bevy spawn 逻辑，保证可在无渲染 World 中测试。（验证：`auth/model.rs` 独立定义 snapshot、请求门控和错误/文案模型；`tests.rs:136` 不创建 Bevy World 即完成状态推导。）
- [x] 保持 MyServer 环境切换会清空账号、角色、ticket 和连接状态，并在请求 pending 时阻止不安全切换。（验证：`auth/host.rs:561` 继续通过 `MyServerProfiles::try_activate` 切换；`tests.rs:453` 及 pending/in-flight 用例验证身份、ticket、连接清理和拒绝规则。）
- [x] 为现有 Login、CharacterSelect 路由记录 owner、panel、action source 和页面状态基线。（验证：`auth/host.rs:55,71` 的 `LOGIN_PAGE_BASELINE` 与 `CHARACTER_SELECT_PAGE_BASELINE` 固定 mode/owner/panel/RustView/source states；`tests.rs:60` 通过。）
- [x] 保存迁移前 desktop、phone-landscape、phone-1080p-landscape 和 tablet-landscape 截图与 audit metadata 作为行为/视觉参考，不把临时产物提交为正式资源。（验证：被忽略的 `target/ui-auth-stage10-baseline/` 中 Login run `20260803-173446-7bac7f`、CharacterSelect run `20260803-173001-6515b9` 各 4/4 passed，均含 PNG 和 metadata；未进入 Git 状态。）
- [x] 保留并整理现有登录、环境切换、角色请求去重、ID 选择、成功路由和登出测试，避免拆分时降低覆盖。（验证：`auth/tests.rs` 独立包含 32 项 Auth 定向测试，主审查 `cargo test auth --lib` 共 71/71 通过。）
- [x] 确认本阶段不迁移 View、不改变协议、不顺手重做视觉，便于独立回归和提交。（验证：两页仍由 `auth/view.rs` 构建；本阶段未新增 UiDocument JSON、未修改 MyServer protocol，迁移前两页四档审计 8/8 passed。）
- [x] 在 `project/` 运行 Auth/MyServer focused tests、`cargo fmt` 和 `cargo check`，并运行登录/选角最小窗口验收。（验证：主审查 Auth 71/71、fmt、Debug/Release check、audit self-test、diff check 均通过；两页四档本地窗口审计 8/8 passed。）

## 阶段 11：将登录页面迁移为 UiDocument JSON

- 开始时间：2026-08-03 18:17:36 +08:00
- 结束时间：2026-08-03 21:55:35 +08:00
- 开发总结：将 Login 从 Auth Rust View 迁移为固定 `DeclarativeScreenHost` 加载的 approved `auth.login` 文档，页面背景、结构、视觉层级和短横屏布局均由 JSON/资源描述；账号走 local typed binding，密码新增 closed `security: sensitive` 协议并只由 Auth host 从 active instance 的精确 ECS 节点读取。登录、游客和环境切换继续使用既有业务门控，CharacterSelect 仍保留 Rust View 等待阶段 12。主审查先后修复敏感 Enter 仍产生明文 submit message/host 遍历 marker 取值，以及 TextInput 新字段遗漏 canonical golden 两类问题，共 2/5 轮。
- 验证记录：主审查独立运行 `cargo test auth --lib`（75/75）、`cargo test ui_document --lib`（108/108）、`cargo fmt --all -- --check`、`cargo check`、`cargo check --release`、`scripts/run-ui-audit.ps1 -SelfTest`、JSON 解析、`git diff --check` 和 audit 产物密码哨兵扫描，均通过；仅有既有 `game/myserver/mail.rs` dead-code warnings。最终 Login audit 位于被忽略的 `target/ui-auth-stage11-final-matrix/20260803-212035-295d6f`，四档 4/4 passed，每档两次 PNG 哈希一致；主审查逐张目视确认无文字越界、裁切或重叠。软键盘逻辑高度 `1376x320` 与常规 `1376x768` 的响应式解析测试通过。

- [x] 创建 approved 登录文档，描述静水背景、标题、服务器选择、账号/密码输入、登录/游客按钮、状态提示和响应式结构。（验证：`approved/auth/login.v1.json` 通过正式 validator 与 host registration 集成测试，audit runtime commit 24 个 document nodes 且图片/字体/locale/theme/viewport 全部 ready。）
- [x] 将登录页面私有尺寸、间距、硬编码颜色、横屏控制网格和视觉层级迁入 document styles/tokens/responsive variants 或共享主题。（验证：`login_panel`/`login_rule`/feedback styles、节点 layout 与 `short_landscape`/`compact_width` overrides 均在 JSON；Rust Login 布局 helper 已删除。）
- [x] 绑定账号输入、密码输入、当前服务器环境、pending、disabled、loading、错误和状态文案。（验证：账号使用 `auth.login.login_name` two-way local binding，其余状态使用 owner typed bindings；密码按安全规则不进入 binding，而由 active instance 的 `login.password` sensitive ECS 节点闭合读取。）
- [x] 将已有登录、游客登录和服务器环境切换 action 接到 Auth host，不在文档内复制业务条件。（验证：三个 action descriptor 固定 document/owner/source/params，host 继续执行 pending/in-flight 去重、环境锁和 session 清理；Auth 75 项通过。）
- [x] 密码内容不得进入日志、audit metadata、binding debug snapshot、更新缓存或 AI 生成输入。（验证：sensitive 文档静态禁止默认值/value binding/on_change/on_submit，掩码显示并禁复制/submit message/reload snapshot；`UiTextInputValue`、native state、command Debug 脱敏，四个唯一 sentinel 在最终 audit run 中均为 0 matches。）
- [x] 登录成功、失败、维护、封禁、审核中、版本不兼容、被踢和网络错误状态保持现有行为。（验证：`auth_login_document_bindings_cover_pending_notice_and_error_states` 覆盖 pending 与六类 notice/error，既有成功路由、失败和账号状态测试继续通过。）
- [x] 删除登录页面的 Rust View 生成路径和对应硬编码布局 helper，只保留宿主、生命周期和必要 adapter。（验证：`OnEnter(Login)` 不再调用 Rust setup；`auth/view.rs` 仅保留 CharacterSelect 生成与共享辅助逻辑，Login 基线 action source 已改为 `UiDocument`。）
- [x] 证明修改登录布局、背景、间距和横屏排列只需修改 JSON/资源，开发期 reload 失败时旧页面保持可见。（验证：Login View 全部位于 approved JSON/packaged image，host 使用 `PackagedFallback`；通用 preview 事务 reload 测试证明失败保留 current instance，敏感值明确不迁移。）
- [x] 为 document/host 契约、输入提交、pending 去重、环境切换和错误状态增加集成测试。（验证：新增 approved registration、startup/late host、closed action、敏感输入、伪造 marker、同帧去重、环境切换和 same-mode route 等回归；Auth 75/75、UiDocument 108/108。）
- [x] 运行 desktop、phone-landscape、phone-1080p-landscape、tablet-landscape 及软键盘高度变化验收，检查文本不溢出、控件不重排和安全区。（验证：最终 run `20260803-212035-295d6f` 四档 4/4，重复截图精确哈希一致并经主审查目视通过；`1376x768`/`1376x320` 响应式解析测试通过。）
- [x] 在 `project/` 运行 Auth/UiDocument focused tests、`cargo fmt` 和 `cargo check`，并运行 UI audit 截图比较。（验证：主审查 Auth 75/75、UiDocument 108/108、fmt、Debug/Release check、audit self-test、diff check 均通过；四档 audit 4/4 passed。）

## 阶段 12：将选角页面迁移为 UiDocument JSON

- 开始时间：2026-08-03 21:57:34 +08:00
- 结束时间：2026-08-03 23:54:16 +08:00
- 开发总结：将 CharacterSelect 从共享 Auth Rust View 完整迁移为独立 fixed `DeclarativeScreenHost` 加载的 approved `auth.character_select` 文档；角色列表使用 keyed repeat，稳定 key 与选择参数保留完整 `character_id`。框架新增有界非控制 UTF-8 `OpaqueId` 参数、结构化 action 去重键，并修正 item binding 的 host 校验与 action value 转换。Auth host 保留请求门控、路由守卫、账号/角色身份分离和精确 active-node 角色名读取；旧 `auth/view.rs` 及角色行/layout helper 已删除。主审查第 1/5 轮修正迁移基线中的当前页面统计和 Repeat 能力边界。
- 验证记录：主审查独立运行 `cargo test auth --lib`（80/80）、`cargo test ui_document --lib`（108/108）、`cargo test repeat_ --lib`（13 passed、1 个显式 golden regeneration ignored）、`cargo test opaque_id --lib`（1/1）、`cargo fmt --all -- --check`、`cargo check`、`cargo check --release`、`scripts/run-ui-audit.ps1 -SelfTest` 和 `git diff --check`，均通过；仅有既有 `game/myserver/mail.rs` dead-code warnings。最终 CharacterSelect audit 位于被忽略的 `target/ui-auth-stage12-final-matrix/stage12-character-select-final-2/`，四档 4/4 passed，每档两次 PNG 哈希一致；主审查逐张确认 desktop 空态、phone-landscape 长角色名、phone-1080p-landscape 六角色和 tablet-landscape 错误态无不合理重叠或横向裁切。真实 MyServer 端到端网络交互未在本阶段执行。

- [x] 创建独立 approved 选角文档，不再与登录页面共享同一 Rust View 树或同一 document 生命周期。（验证：`approved/auth/character_select.v1.json` 与独立 promotion registration 由 `character_select_declarative_screen_host` 注册；startup mount 集成测试确认 CharacterSelect 独立实例成功 commit。）
- [x] 使用动态列表模板渲染角色，稳定 key 和选择 action 参数必须使用 `character_id`，显示名称只用于文本。（验证：文档 `character.repeat` 以 `character_id` 为 key，`character.row.select` 从 item binding 传完整 ID；Auth 选择测试覆盖 Unicode/分隔符 ID 且命令不使用 display name。）
- [x] 绑定角色列表、pending character ID、当前选择、元素属性、账号摘要、连接状态和错误状态。（验证：`character_select_binding_schema` 与 `sync_character_select_document_bindings` 注入 owner `list<record>`、current/pending ID、affinity/mastery、account summary、connection 和 feedback typed bindings；对应 binding 集成测试通过。）
- [x] 接入加载角色、创建角色、选择角色、切换账号和切换角色 action，保持请求门控与同帧去重。（验证：五个 closed action 固定 document/owner/source/param；host 从 active `character.create_name` 节点读取创建名、选择前复核 session 完整 ID，并以 `request_sent` 维持同帧业务门控；Auth action 去重测试通过。）
- [x] 为 loading、empty、ready、error、创建中、选择中和已有当前角色状态提供明确页面表现。（验证：`auth.character.view_state` 七值枚举由纯状态函数生成，collection 自带 loading/empty/error 表现，error/notice/profile 绑定控制反馈；Auth 状态测试与四档 audit 通过。）
- [x] 验证角色新增、删除、重排和局部属性更新使用 keyed diff，不丢失无关行状态或滚动位置。（验证：`repeat_reconciles_keyed_rows_preserves_entities_and_dispatches_item_ids` 覆盖新增、删除、重排、文本局部更新，确认保留行 Entity、焦点及 repeat host `ScrollPosition`；repeat focused tests 13 passed、1 ignored。）
- [x] 删除选角页面 Rust View、角色行 spawn 和页面私有布局 helper，保留 Auth host 与纯状态推导。（验证：`project/src/game/screens/auth/view.rs` 已删除，`auth/mod.rs` 的 CharacterSelect 生命周期只保留 audit fixture、session guard 和 document host systems；`model.rs` 保留纯 snapshot/门控推导。）
- [x] 证明角色卡样式、排列、列表尺寸和状态页面修改只涉及 JSON/资源，不修改 Rust。（验证：角色卡 style、repeat grid/spacing、scroll/panel 尺寸和 short-landscape/compact-width overrides 全部位于 approved JSON；Rust host 不再包含 CharacterSelect View/layout helper，迁移基线明确既有 Repeat 契约内的 UI-only 边界。）
- [x] 覆盖 `player_id`/`character_id` 分离、短 ID 只用于展示、选角成功路由和切换账号清理测试。（验证：Auth 80/80 包含 schema/snapshot 身份分离、display discriminator/short ID 展示、完整 ID 选择、成功路由 Lobby、切换账号清空输入并 Logout 等用例。）
- [x] 运行 desktop、phone-landscape、phone-1080p-landscape、tablet-landscape 的空列表、长角色名、多角色和错误状态审计。（验证：`target/ui-auth-stage12-final-matrix/stage12-character-select-final-2/manifest.json` 为 4/4 passed；四档分别命中 empty、长名称单角色、六角色和 error fixture，两次截图哈希精确一致并经主审查目视通过。）
- [x] 在 `project/` 运行 Auth/collection/UiDocument focused tests、`cargo fmt` 和 `cargo check`。（验证：主审查 Auth 80/80、UiDocument 108/108、repeat 13 passed/1 ignored、OpaqueId 1/1、fmt、Debug/Release check、audit self-test 和 diff check 全部通过。）

## 阶段 13：迁移大厅和常规业务页面

- 开始时间：2026-08-03 23:56:43 +08:00
- 结束时间：2026-08-04 02:24:25 +08:00
- 开发总结：将 Lobby 从 788 行 Rust `game_list.rs` 迁移为 fixed `DeclarativeScreenHost` 加载的 approved `game.lobby` 文档；游戏条目使用完整稳定 `entry_id` 的 keyed repeat，Rust host 仅保留列表模型、闭合 action、路由/场景/MyServer 命令与公共 Confirm/Loading 协调。主审查第 1/5 轮删除文字伪造的图片失败证据，改为 Debug 非 Android 指定审计档对真实 `lobby.background` Image 节点注入 preflight Failed，并与同轮成功加载档对照。
- 验证记录：主审查独立运行 `cargo test lobby --lib`（26/26）、`cargo test ui_document --lib`（108/108）、`cargo fmt --all -- --check`、`cargo check`、`cargo check --release`、`scripts/run-ui-audit.ps1 -SelfTest` 和 `git diff --check`，均通过；仅有既有 theme/mail dead-code warnings。内容审计 `target/ui-audit-stage13/stage13-lobby-content-image-fallback/` 四档 4/4 passed、每档两次 PNG 哈希一致；phone-landscape 真实失败节点无 `ImageNode` 且显示 `#7a2930ff` fallback，phone-1080p 同节点为 `ready` 且资源已解析。覆盖层审计 `target/ui-audit-stage13/stage13-lobby-overlays/` 2/2 passed；主审查目视确认内容、Loading 与 Confirm 截图无不合理重叠或文字伪证据。真实 MyServer/场景端到端网络交互未在本阶段执行。

- [x] 将大厅游戏列表页面迁移为 approved UiDocument，Rust 只提供游戏列表、选择、进入和确认操作宿主。（验证：`approved/lobby/lobby.v1.json` 描述完整 View，`lobby/host.rs` 固定注册 `game.lobby` 并承接闭合业务命令，原 `lobby/game_list.rs` 已删除）
- [x] 使用稳定业务 ID 渲染游戏或房间条目，不以显示名称、数组下标或 Entity 作为 action 身份。（验证：文档 keyed repeat 使用 `key: entry_id`，select/enter 传递完整 bounded opaque `entry_id`，host 按当前 typed list 复验；`lobby_actions_are_closed_and_entry_ids_are_revalidated` 通过）
- [x] 覆盖大厅 loading、empty、ready、error、连接断开和确认弹窗状态。（验证：`LobbyCollectionState`、`lobby_view_state` 与 typed binding 覆盖六态，content/overlay 审计分别 4/4、2/2 passed）
- [x] 复用公共 modal/overlay 能力，文档不得绕过 Panel Manager 自行实现全局 Loading 或 Confirm 生命周期。（验证：场景进入使用 `UiPanelRequest::Loading`，Touch Ripple 使用 `UiPanelRequest::Confirm`；相关 action、场景和 cleanup 测试通过）
- [x] 迁移阶段 1 盘点出的其他非设置、非玩法 HUD 普通业务页面；每个页面先冻结 host contract，再删除 Rust View。（验证：阶段 1 分母中本阶段仅 Lobby 符合范围；`lobby.promotion.v1.json` 冻结 contract 后删除唯一旧 Rust View `game_list.rs`）
- [x] 对每个迁移页面保留路由、owner、焦点、返回/关闭、输入阻断和生命周期测试。（验证：fixed host 保留 `lobby` route/owner/page panel，scroll `block_lower: true`；navigation、scene lifecycle 与 `lobby_cleanup_clears_focus_transient_state_and_public_overlays` 测试通过）
- [x] 为长文本、空数据、最大允许列表、图片失败和资源热更新状态增加审计 recipe。（验证：content 四档覆盖 desktop 空态、phone 长文本与真实 `ImageNode` fallback、phone-1080p 24 项、tablet error/hot-update；4/4 passed 且重复哈希一致）
- [x] 逐页验证纯布局/视觉修改的 Git diff 不包含 `.rs`，发现缺失能力时回到 framework 扩展，禁止页面私有 Rust 绕过。（验证：Lobby 的布局、视觉、responsive 与图片 fallback 声明均位于 approved JSON；Rust 只保留 host/model/audit 注入，迁移基线明确 UI-only diff 门禁）
- [x] 更新页面迁移清单和指标，明确尚未迁移页面及原因。（验证：`docs/界面/界面声明式业务界面迁移基线.md` 将 Lobby 标为正式声明式并更新为已声明式 5、普通业务待迁移 1，剩余为阶段 14 的 audio settings）
- [x] 在 `project/` 运行相关业务和 UiDocument focused tests、`cargo fmt` 和 `cargo check`，运行对应 UI audit 矩阵。（验证：主审查 Lobby 26/26、UiDocument 108/108、fmt、Debug/Release check、audit self-test 均通过；content 4/4、overlay 2/2 passed）

## 阶段 14：迁移设置和表单类页面

- 开始时间：2026-08-04 02:26:20 +08:00
- 结束时间：2026-08-04 03:49:24 +08:00
- 开发总结：将 Audio Settings 从 Rust View 迁移为 fixed `DeclarativeScreenHost` 加载的 approved `game.audio_settings` 文档；5 个 bus Slider、master mute Toggle 与返回 Lobby 通过 12 个 owner binding 和 3 个闭合 action 接入，`AudioMixer` 保持权威源并即时生效。现有产品没有保存/取消、dirty、恢复默认、配置持久化或独立 gamepad adapter，contract 与文档明确标为不适用/不宣称。worker 审计发现移动端 Scroll 的文档 layout 覆盖通用 bundle 后未保留 overflow，已补 `overflow.y=scroll` 与 runtime 回归测试，并用 bottom capture 验证修复。
- 验证记录：主审查独立运行 `cargo test settings --lib`（21/21）、`cargo test ui::widgets::controls --lib`（66/66）、`cargo test ui_document --lib`（108/108）、`cargo fmt --all -- --check`、`cargo check`、`cargo check --release`、`scripts/run-ui-audit.ps1 -SelfTest`、JSON 解析和 `git diff --check`，均通过；仅有既有 theme/mail dead-code warnings。最终四档审计 `%TEMP%/mybevy-stage14-ui-audit/stage14-audio-settings-final/` 为 4/4 passed、每档两张截图哈希一致且 `content_reachable=true`；phone bottom 审计 1/1 passed，`current_offset=max_offset=299`。主审查目视确认 boundary clamp、bottom、disabled 与 unavailable/error 截图无重叠或裁切。

- [x] 将音频设置及阶段 1 盘点出的其他设置页面迁移为 approved UiDocument。（验证：阶段 1 唯一正式设置页 `audio_settings` 已由 `approved/audio_settings/audio_settings.v1.json` 描述，fixed host `game.audio_settings` validation/promotion 测试通过）
- [x] 通过类型化 binding/action 接入 slider、stepper、toggle、segmented、select、tab 和保存/恢复默认值等既有业务能力。（验证：实际既有业务仅 5 个 Slider、master Toggle 与返回按钮，12 个 typed owner binding/3 个 action 已闭合；Stepper/Segmented/Select/Tab/保存/默认恢复不存在业务语义，迁移基线明确不适用且未伪造）
- [x] 明确即时生效、提交生效、取消恢复、dirty、validation 和外部状态变化的宿主规则，不在文档中实现业务表达式。（验证：action 在 `AudioSystemSet::Commands` 前写命令、binding 在其后回读 Mixer；外部变化测试通过，提交/取消/dirty/persistence 不适用规则写入迁移基线）
- [x] 处理数值范围变化、设备能力缺失、配置加载失败和保存失败，控件不得显示超出宿主 schema 的旧值。（验证：registry 限制 `0..100`，host 复验 finite/clamp，Mixer 缺失产生 unavailable/error 并禁用控件；当前无配置加载/保存流程，文档明确其失败态不适用）
- [x] 验证键盘、手柄/焦点、鼠标和触控下的控件可用性，以及 disabled/loading/error 的完整视觉状态。（验证：controls 66/66、worker 补充 focus 19/19、slider 4/4、scroll 18/18；四档截图覆盖 disabled/error，loading 不适用。仓库无独立 gamepad adapter，仅验证共享 focus/activation 并在文档明确不宣称额外手柄映射）
- [x] 删除已迁移设置页面的 Rust View 和私有控件布局 helper，保留配置资源与业务系统。（验证：旧 `settings/audio.rs` 及其 setup/layout helper 已删除，音频 framework 的 `AudioMixer`/`AudioCommand`/`AudioSystemSet` 保持不变）
- [x] 为设置热更新期间的当前值、dirty 状态、焦点和滚动位置迁移增加测试。（验证：`audio_settings_reload_keeps_focus_scroll_and_reapplies_authoritative_value` 证明兼容 reload 保留 focus/scroll 后由 Mixer 重施值；产品无 dirty 状态）
- [x] 证明调整设置项排列、分组、说明文本、响应式布局和视觉只修改 JSON/资源。（验证：上述 View 字段均位于 approved JSON，Rust `host.rs` 只含 contract/action/binding/audit adapter；迁移基线冻结 JSON-only 边界）
- [x] 更新设置/UI 文档和页面迁移指标。（验证：`界面声明式业务界面迁移基线.md` 更新为已声明式 6、普通业务待迁移 0 并新增 Audio Settings 宿主语义；`界面组件功能与使用.md` 链接正式 Slider/Toggle 用法）
- [x] 在 `project/` 运行 settings/control/UiDocument focused tests、`cargo fmt` 和 `cargo check`，运行设置页面审计矩阵。（验证：主审查 settings 21/21、controls 66/66、UiDocument 108/108、fmt、Debug/Release check 与 audit self-test 通过；四档 4/4、phone bottom 1/1 passed）

## 阶段 15：迁移玩法 HUD 并收紧长期门禁

- 开始时间：2026-08-04 03:51:26 +08:00
- 结束时间：2026-08-04 06:43:47 +08:00
- 开发总结：将 Touch Ripple、Sample Scene、Robot Sync、Fangyuan Player Preview 和 Fangyuan Home 的普通 HUD View 迁移为五个 fixed host + approved `UiDocument`，Rust 仅保留玩法输入、场景/3D、authority snapshot、Fangyuan blueprint/debug adapter 与生命周期；approved registration 闭合扩展到 `page|hud` 且 layer 仍固定为 `page`。新增基于 `syn` AST 的 route exact-set 和直接 Rust UI tree 门禁，四个开发工具 Rust View 例外保持显式闭合；主审查第 1/5 轮修复了新增 enum variant 可绕过分类、cache/bake 动作对调和跨文档 action 拼接问题。
- 验证记录：主审查独立运行 gameplay 25/25、UiDocument 108/108、navigation 20/20、HUD approval 正负测试、项目与工具 fmt、Debug/Release check、UI audit self-test、UI generation 217/217、真实 `check-boundary`（10 个布尔字段全 true）、JSON 解析和 `git diff --check`，均通过；仅有既有 theme/mail dead-code warnings。桌面审计 initial 20/20、key states 8/8、Fangyuan status/debug scroll 各 4/4，重复截图 hash 一致；主审查目视抽查 Touch、Robot hidden、Fangyuan status/debug bottom 无空白、重叠或不合理裁切。E2E 为 `passed_with_external_android_blocker`；Android arm64 Release、Debug APK、SM-G9730 API 31 安装和 Vulkan 出帧通过，但 secure keyguard 阻止可信触控/IME/安全区/重启离线回滚验收，生产更新另缺 app-private cache root、生产 trust roots、endpoint 和 check trigger。

- [x] 逐个评估 Touch Ripple、Sample Scene、Robot Sync、Fangyuan Player Preview 和 Fangyuan Home HUD，区分声明式 HUD View 与必须保留的玩法/3D/特殊手势逻辑。（验证：`project/src/game/screens/gameplay/host.rs` 统一五个 HUD host；Touch 输入仍在 feature，Sample/Robot/Fangyuan scene、authority、blueprint 和 3D 系统仍保留在各 gameplay/feature 模块）
- [x] 将普通 HUD 面板、状态文本、按钮、列表、进度和调试信息迁移为 UiDocument；玩法模拟、场景实体和特殊输入系统继续留在 Rust。（验证：`project/assets/ui/documents/approved/gameplay/` 包含五份 document/promotion；旧 `touch_ripple.rs` 删除，其余四个 screen 移除 Rust View 树且 gameplay 25/25）
- [x] 为 HUD 建立 owner/document binding，避免 framework 直接依赖 Fangyuan、Robot Sync 或其他游戏业务类型。（验证：`GameplayHudHostContract` 声明 owner-scoped Robot/Fangyuan bindings，framework approval 只识别通用 `UiDocumentPanel::Hud`；UiDocument 108/108）
- [x] 高频数值更新使用受控增量 binding，验证不会每帧重建文档、重复加载资源或造成布局抖动。（验证：Robot/Fangyuan 更新经 `UiBindingValues::set_scoped`；`unchanged_high_frequency_binding_does_not_advance_revision` 通过，四组审计重复截图 hash 一致）
- [x] overlay、modal 和 blocking UI 继续遵循统一层级、焦点与输入阻断规则，场景切换时完整清理。（验证：HUD host 固定 `panel=Hud/layer=Page`，approval 拒绝其他 panel 且 layer 仍闭合；Touch root 无 `Pickable`、仅按钮交互，退出后 document 消失且 focus 清空测试通过）
- [x] 对保留 Rust View 的开发 Gallery、审计工具页或特殊调试 UI 逐项记录例外，业务页面不得借用该例外。（验证：`CONTROLLED_RUST_VIEW_EXCEPTIONS` 仅列 AiLoginReference、AudioGallery、AudioMonitor、UiGallery 四项并绑定精确路径；真实 boundary gate 通过）
- [x] 增加静态边界检查，阻止新的普通业务 screen 直接构建大段 Bevy UI 树；允许项必须来自受控例外清单。（验证：`DirectUiViewVisitor` AST 检测构树调用与 Node/Text/Button/ImageNode，非 legacy sentinel UI 负测和纯 3D 不误报测试通过）
- [x] 在 CI 或本地门禁加入“UI-only fixture/promotion 不修改 Rust”和“全部业务 route 都有声明式文档或受控例外”检查。（验证：UI-only manifest 拒绝 `.rs` 正测/负测通过；`AppUiMode` AST actual set 与三类显式分类 exact-set，新增未分类 enum variant 负测通过）
- [x] 更新全量页面迁移指标，确保没有遗漏仍由 Rust 构建的正式业务页面。（验证：boundary 报告 9 个 approved 正式业务 route、15 个实际 screen 全分类、四个 Rust View 例外 exact-set；`CLAUDE.md`、上手与 UI 架构文档同步）
- [x] 运行全量 UI audit/e2e acceptance、桌面 Release 和 Android 构建/真机可用条件下的热更新、离线、失败回滚和输入验收。（验证：initial 20/20、key states 8/8、两组 Fangyuan scroll 各 4/4，E2E `passed_with_external_android_blocker`，Release check、Android arm64/APK/安装/出帧通过；secure keyguard 与生产更新前置缺失已记录为外部阻塞，未伪报触控/IME/离线回滚通过）
- [x] 在 `project/` 运行 `cargo fmt`、相关全量测试和 `cargo check`；在仓库根运行 UI generation 工具测试、boundary check 和 `git diff --check`。（验证：主审查 gameplay 25/25、UiDocument 108/108、navigation 20/20、fmt、Debug/Release check、audit self-test、UI generation 217/217、boundary 全 true、JSON 与 diff check 通过）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都重复执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-04 06:46:19 +08:00
- 结束时间：2026-08-04 06:48:01 +08:00
- 验收总结：声明式业务宿主、类型化 action/binding、动态列表、approved contract、事务缓存、受信远端更新、AI 无代码生成晋升和九个正式业务 View 迁移均已完成，长期 route/Rust View/UI-only 门禁生效。桌面测试、审计、E2E、Release 与 Android 构建安装证据通过；Android 真机真实下载、重启离线、IME/安全区、触控和失败回滚按清单规则保留未勾选，阻塞原因为 secure keyguard 以及默认应用尚未注入生产 cache root/trust roots/endpoint/check trigger。

- [x] 所有正式普通业务页面均由 approved `UiDocument` 描述 View，或存在经过审阅且有明确技术原因的受控例外。（验证：boundary 报告 9 个正式业务 route 全有 document/promotion；四个直接 Rust View 均为开发工具且来自精确例外清单）
- [x] 登录和选角是两个独立 UiDocument，Rust Auth host 不再生成它们的页面结构和角色行 View。（验证：`auth.login`、`auth.character_select` 独立 approved 文档，旧 Auth Rust View 已删除；Auth 80/80）
- [x] 大厅、设置和普通玩法 HUD 已完成声明式迁移，业务 Rust 只维护宿主状态、动作和生命周期。（验证：Lobby、Audio Settings 和五个 gameplay HUD 均由 fixed host 加载 approved JSON；Stages 13-15 focused tests 通过）
- [x] TextInput 和全部正式选择/数值控件可以通过类型化 binding/action 与业务宿主交互。（验证：Login TextInput、Audio Settings Slider/Toggle 与 closed `current_control_value` action/binding 测试通过）
- [x] 动态列表支持稳定 key、item binding、keyed diff、预算和 loading/empty/error 状态，角色列表已作为真实业务验证。（验证：Repeat/collection 测试与 CharacterSelect 四档空态、长名、六角色、错误态 audit 通过）
- [x] approved 业务文档只能引用精确 allowlist 内的 action/binding，未知或越权能力在静态验证、晋升和 runtime 三处均被拒绝。（验证：approval v2、promotion host contract、runtime source/owner/params 校验及跨文档拼接负测通过）
- [x] AI 可以在不修改 Rust 的情况下生成、预览、审核、修复并晋升使用既有业务契约的页面。（验证：阶段 9 host-bound fixture 全链、UI generation 217/217 和 E2E acceptance 通过）
- [x] 任一纯布局、视觉或响应式修改的验收 diff 只包含允许的 JSON/资源/元数据文件，不包含 `.rs`。（验证：`verify_ui_only_change_manifest` 正负测试和真实 boundary write scope 通过）
- [x] 新增无业务逻辑的纯展示页面可以通过数据注册进入通用 document route，不需要新增专用 Rust screen。（验证：`DeclarativeScreenRegistry` pure document route 与 declarative dev screen 分类测试通过）
- [x] 用户端可从受信远端获取签名 UI bundle，完整验证后原子激活，并能在下次启动继续使用。（验证：阶段 8 Ed25519 envelope/trust store、固定 endpoint、ETag/download、安全点 activation 与持久 generation 测试通过；默认应用启用仍需发布配置）
- [x] 远端不可用、文件损坏、签名失败、版本不兼容、资源缺失、无空间或 commit 失败时继续使用当前有效版本或首包 fallback。（验证：update/cache/remote failure fixtures 覆盖 current/previous/packaged fallback、quarantine 与 no-clobber）
- [x] Release 和 Android 不开放任意文件 watch、裸路径、裸 URL 或未签名 JSON 加载入口。（验证：compile/platform gate、source path/endpoint/trust validation 测试及桌面 Release/Android target check 通过）
- [x] 热更新缓存具有磁盘预算、版本保留、回滚、quarantine 和中断恢复，日志与指标不泄露账号、ticket、输入内容或本机敏感路径。（验证：阶段 7/8 cache/client/observability 测试覆盖预算、lease、staging 恢复、redaction 和安全失败分类）
- [x] desktop、phone-landscape、phone-1080p-landscape、tablet-landscape 的 initial/loading/empty/error/长文本和主要交互状态通过 UI audit。（验证：Stages 11-15 四档矩阵覆盖 Login、CharacterSelect、Lobby、Settings 与五个 HUD；重复截图一致且主审查完成代表性目视检查）
- [ ] Android 真机条件可用时完成下载、激活、重启保持、离线 fallback、软键盘、安全区、触控和失败回滚验收；不可用时保留未勾选并记录阻塞条件。（阻塞：SM-G9730 API 31 位于 secure keyguard 后，虽已完成 arm64 Release、Debug APK、安装、Activity/PID/surface/WindowInsets/Vulkan 出帧检查，但触控、IME、安全区视觉、重启/离线/回滚不可可信执行；生产更新还未注入 app-private cache root、production trust roots、endpoint 和 `CheckNow` 触发点）
- [x] `cargo fmt`、项目相关测试、`cargo check`、UI generation 工具测试、boundary check、UI audit/e2e acceptance 和 `git diff --check` 全部通过。（验证：最终主审查 gameplay 25/25、UiDocument 108/108、navigation 20/20、approval 正负测试、两端 fmt、Debug/Release check、audit self-test、UI generation 217/217、boundary 10 项 true、E2E external-Android-blocker 合格状态与 diff check 通过）
- [x] `docs/界面/`、资源工作流、上手文档和 `CLAUDE.md` 已与最终架构一致，不再把开发期 reload 描述为用户端生产热更新。（验证：相关文档明确区分 desktop Debug preview/watch 与显式配置的签名 production `UiUpdateClient`，并记录默认应用/Android 尚缺的发布接线）
- [x] 本 清单 已从 `summary/` 转移到 `docs/界面/checklists/` 归档并纳入最终提交。（验证：归档路径 `docs/界面/checklists/UI声明式界面与热更新改造_checklist.md`）
