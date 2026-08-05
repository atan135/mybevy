# 邮箱界面与通知闭环 Checklist

## 目标

在现有 `MailPlugin`、`MailClientState`、HTTPS 命令/事件和领取 reconciliation 基础上，交付主世界内可用的声明式邮箱界面、未读角标、通知刷新和完整错误状态。邮箱使用 PostgreSQL/邮件 HTTP API 结果作为权威事实，聊天 push 只负责使缓存失效。

本清单不重复实现邮件运营后台、服务端发信系统或附件发放事务；发现服务端契约缺口时在 MyServer 单独修复并记录跨仓库依赖。

## 依赖与边界

- 依赖 `project/src/game/myserver/mail.rs` 已有列表、详情、已读、领取和有界 reconciliation。
- 依赖 `04_聊天应用级接线_checklist.md` 提供 `MailNotifyPush` 桥接；chat 不可用时邮箱仍可主动 HTTPS 查询。
- 依赖 `03_主世界HUD与场景内面板_checklist.md` 提供邮件入口和场景内 panel 生命周期。
- 邮件 ownership 使用账号 `player_id`，附件发放目标使用当前 ticket 绑定的 `character_id`。
- Android 真机上的公网邮件 HTTPS、通知、触控、前后台与领取验收集中转移至 `07_公网部署后Android真机验收_checklist.md`。

## 基础原则

- [x] UI 只发送 `MailClientCommand` 并消费 `MailClientState/Event`，不直接持有 HTTP 客户端或拼接 endpoint。（验证：`main_world_mail` host adapter 只写 command/读 state，HTTP path/header 封装于 `game/myserver/mail.rs`）
- [x] push 摘要不是邮件权威事实；列表、详情、已读和领取状态均以 HTTPS 响应为准。（验证：`mail_push_only_invalidates_and_refreshes_authoritative_list` 及两份主题文档明确 HTTPS/PostgreSQL 权威边界）
- [x] 领取操作遵守服务端幂等工作流，结果未知时重查，不通过客户端重复发放或本地猜测成功。（验证：`post_attempted` 限制单 POST，202/网络/5xx 使用 1/2/4 GET 对账；MyServer 重复 claim 无二次库存发放）
- [x] 切账号、角色、环境和 endpoint 时清理旧身份数据并取消旧 HTTP 请求。（验证：identity 包含 ticket 指纹/player/character/endpoint，reset 和迟到 response 定向测试通过）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（验证：阶段 1-8 对应 `eeace8b`、`84a4e77`、`88e2b81`、`e778cd0`、`a27ecc4`、`967de29`、`927e62c`、`a1116b1`；服务端验收为 `11f295e`）

## 阶段 1：邮件现状和公开契约基线

- 开始时间：2026-08-05 19:07:19 +08:00
- 结束时间：2026-08-05 19:37:34 +08:00
- 开发总结：冻结正式邮件公网 descriptor、固定 API path/header/null 语义，引入 Local profile 显式 HTTP 策略，并对照 MyServer 固化摘要、详情、分页、领取状态与公开错误契约。
- 验证记录：`cargo test mail::tests --lib`（9 项）、邮件 HTTP policy 与统一 descriptor 定向测试、`cargo fmt --check`、`cargo check` 和 `git diff --check` 通过；仅有既有 dead-code 警告。

- [x] 确认 `MailPlugin` 已注册到 `MyServerPlugin`，并随 session 身份变化同步 endpoint 和 availability。
- [x] 确认客户端已有列表、详情、标记已读、单封领取、202 结果未知和有界 reconciliation 适配。
- [x] 确认 `forward_chat_inbound` 已存在但尚未由应用级 chat 系统注册调用。
- [x] 固化公网 `services.mail` descriptor、固定 `/api/v1/mails` path、`X-Game-Ticket` 和 `null` 不可用语义。（验证：`mail.rs` 固定生产 host/443、API path 和 header；endpoint/null/禁止 9003 回退测试通过）
- [x] 明确本地 HTTP 联调的显式配置方式；当前只接受 HTTPS 的实现不得与文档宣称的本地 HTTP 路径冲突。（验证：`ClientServiceEndpointPolicy` 仅允许 Local profile 接收 auth 下发 HTTP descriptor，Production 拒绝；policy 定向测试通过）
- [x] 对照服务端冻结邮件摘要、详情、分页、领取状态和稳定错误码字段。（验证：只读核对 `H:\project\MyServer\apps\mail-service`，`public_mail_contract_deserializes_null_pagination_and_claim_states` 及 `docs/myserver/邮件客户端公开契约.md` 固化公开字段）

## 阶段 2：邮箱 fixed host 和 approved document

- 开始时间：2026-08-05 19:38:56 +08:00
- 结束时间：2026-08-05 20:22:49 +08:00
- 开发总结：将主世界邮箱骨架扩展为完整 fixed host 契约和 approved document，注册七类闭合动作、typed collection/detail/claim bindings、独立首包 fallback 与安全关闭恢复路径。
- 验证记录：`cargo test main_world_mail --lib`（9 项）、`cargo test ui_document --lib`（108 项）、approved gameplay host/promotion 定向测试、`cargo fmt --check`、`cargo check` 和 JSON/`git diff --check` 通过；仅有既有 dead-code 警告。

- [x] 为邮箱定义 document ID、owner、panel/layer、route label、首包 source 和 audit profiles。（验证：`main_world_mail_declarative_screen_host` 固定 `game.main_world_mail`、mail panel owner/route、Floating 层、approved source 与四档 audit profiles）
- [x] 注册列表刷新、选择邮件、加载更多、标记已读、领取、重试和关闭等 closed action descriptor。（验证：`gameplay_action_descriptors` 注册 7 个唯一 source action；mail ID 使用 64-byte OpaqueId 参数，registry 伪造拒绝测试通过）
- [x] 定义 availability、collection state、列表、选中详情、领取状态和错误信息的 typed binding schema。（验证：`main_world_mail_binding_schema` 定义 owner/item scope、50 条 keyed list、32 个附件及结构化状态；host 过滤 Item scope 测试通过）
- [x] 新增 approved 邮箱 document，不新增普通业务 Rust View 例外。（验证：`main_world_mail.v1.json` 与 promotion 通过 approved host audit 和 108 项 UiDocument 测试）
- [x] 邮箱作为主世界场景内面板打开；关闭只释放 UI instance 和 focus，不退出房间或取消 reconciliation。（验证：`main_world_mail_close_only_closes_its_route_and_preserves_mail_state` 及 fallback recovery 测试通过）
- [x] packaged document 或更新 generation 失败时保留首包 fallback 和安全关闭入口。（验证：独立 `main_world_mail_fallback.v1.json` 仅含闭合 close action；fallback 全失效测试确认只关闭 panel 并保留 reconciliation）

## 阶段 3：列表、未读数和分页

- 开始时间：2026-08-05 20:24:02 +08:00
- 结束时间：2026-08-05 20:59:27 +08:00
- 开发总结：接通权威邮件列表与声明式 Repeat，建立明确加载状态、filter/offset 分页、稳定合并去重、identity/generation 迟到响应隔离及 HUD 权威未读语义。
- 验证记录：`cargo test mail::tests --lib`（14 项）、`cargo test main_world_mail --lib`（13 项）、gameplay host 定向测试、`cargo fmt --check`、`cargo check` 和 `git diff --check` 通过；仅有既有/后续阶段预留 dead-code 警告。

- [x] 打开邮箱时发送权威 `LoadList`，覆盖 loading、ready、empty、error 和 retry 状态。（验证：`MailListLoadState` 覆盖 InitialLoading/Refreshing/LoadingMore/Ready/Empty/Failed，打开与 retry action 定向测试通过）
- [x] 使用稳定完整 `mail_id` 作为 Repeat key 和 action opaque ID，并由 host 对当前权威列表复验。（验证：document Repeat key=`mail_id`；select adapter 校验 closed target/单参数并调用 `contains_authoritative_mail`，伪造与完整 ID 测试通过）
- [x] 展示标题、发送方、发送时间、未读状态、附件标记和过期状态，不显示内部 ownership 字段。（验证：`main_world_mail_list_item` 生成有界公开字段和 RFC3339 expiry 状态；binding 测试确认不含 sender id/type、player/character/endpoint）
- [x] 正确处理 status filter、limit、offset、has_more 和下一页；刷新与加载更多不得互相覆盖或重复条目。（验证：`list_query_supports_status_limit_and_stable_paginated_merge` 覆盖 filter、权威 next_offset、重复请求抑制和原位去重更新）
- [x] 使用 list generation 丢弃迟到页和旧身份响应，排序与去重结果保持稳定。（验证：pending 绑定 generation+identity；迟到 refresh 与切身份取消/丢弃测试通过，刷新保留服务端顺序、分页稳定追加）
- [x] 将权威 unread_count 同步到主世界 HUD；未知、加载中和服务不可用不得显示为确定的零。（验证：`authoritative_unread_count` 仅允许非 stale Ready/Empty/LoadingMore；HUD 测试区分隐藏未知与可见权威 `0`）

## 阶段 4：详情和标记已读

- 开始时间：2026-08-05 21:00:43 +08:00
- 结束时间：2026-08-05 22:01:51 +08:00
- 开发总结：接通 generation/identity 隔离的权威详情、显式 MarkRead 幂等同步、正文/附件双层预算、详情返回列表与 CloseTop 层级，并对正文和附件诊断进行脱敏。
- 验证记录：`cargo test mail::tests --lib`（18 项）、`cargo test main_world_mail --lib`（16 项）、`cargo test close_top --lib`（2 项）、approved host audit、`cargo fmt --check`、`cargo check`、JSON 和 `git diff --check` 通过；仅有既有/后续阶段预留 dead-code 警告。

- [x] 选择邮件后加载权威详情，覆盖 loading、not found、过期、无权限、错误和重试状态。（验证：`MailDetailLoadState` 与 detail generation+identity 覆盖 Loading/Ready/403/404/410/Failed；迟到选择和旧身份测试通过）
- [x] 展示正文和附件列表时限制文本、列表和响应大小，防止极端内容破坏布局或内存预算。（验证：HTTP 256 KiB、正文 32 KiB/8192 字符、附件 32 项及 UI 4096-byte binding 门禁；超限拒绝测试通过）
- [x] 按产品规则在打开详情后显式或自动发送 MarkRead，并处理 `already_read` 幂等响应。（验证：approved document 使用显式 Mark read action；host 复验 selected/list mail ID，pending 去重及 `already_read` 测试通过）
- [x] 已读成功后同步详情、列表行和未读数，不在 HTTP 失败时提前永久修改本地状态。（验证：`apply_read_to_cache` 仅在成功 response 后更新并按原 unread 状态递减；网络失败保留列表/详情/未读测试通过）
- [x] 关闭详情返回邮箱列表，关闭邮箱返回主世界 HUD，Escape/BrowserBack 遵守 CloseTop 层级。（验证：closed `back_to_list` action 与 `main_world_mail_close_top_returns_to_list_before_closing_floating_route` 两步测试通过）
- [x] 日志、审计和错误报告不得记录邮件正文或附件内容。（验证：`MailDetail` 自定义 Debug 只含 mail ID/status/附件计数；正文、附件 ID 和服务端原始错误哨兵测试确认不进入 Debug/event/error）

## 阶段 5：附件领取和 reconciliation UI

- 开始时间：2026-08-05 22:03:13 +08:00
- 结束时间：2026-08-05 22:35:02 +08:00
- 开发总结：完成附件领取的权威入口复验、单次 POST 与结果未知对账状态机，覆盖服务端终态/可重试/人工复核语义；对账在面板关闭后继续并可恢复 UI，同时以 ticket 指纹和玩法身份隔离迟到结果。
- 验证记录：`cargo test mail::tests --lib`（24 项）、`cargo test main_world_mail --lib`（18 项）、`cargo test claim --lib`（11 项）、`cargo test reconciliation --lib`（7 项）、`cargo fmt --check` 和 `cargo check` 通过；仅有既有/后续阶段预留 dead-code 警告。

- [x] 只有存在可领取附件且当前 MailAvailability Ready 时启用领取按钮。（验证：host 在提交前复验 Ready availability、权威选中详情、非空附件和未过期状态；claim action/binding 定向测试通过）
- [x] 领取提交期间禁用重复操作，并显示 processing、claimed、already claimed、expired 和失败状态。（验证：`MailClaimWorkflowState` 覆盖提交、处理中、成功、已领取、过期、容量和失败终态；`post_attempted` 阻止同一 workflow 重复 POST）
- [x] HTTP 202、网络中断或服务端 processing/reconciliation_pending 时显示结果确认中，不宣称领取失败或成功。（验证：未知结果统一进入 `ReconciliationPending`，202 收敛和网络异常测试确认不重复发送领取 POST）
- [x] 面板关闭后继续由全局 MailClientState 完成有界 `1/2/4` 秒重查，重新打开时恢复当前 workflow 状态。（验证：全局 state 保留 workflow/timer；关闭详情、fallback 关闭及重新绑定测试覆盖三次有界 GET 和状态恢复）
- [x] 覆盖 retryable_failure、permanent_failure、manual_review、领取入口暂停和玩家可重试提示。（验证：服务端状态映射保留 retryability；三次对账耗尽进入 `ManualReview`/unknown，并使用稳定错误码 `MAIL_CLAIM_RECONCILIATION_EXHAUSTED`）
- [x] 同一邮件重复领取、跨角色切换和迟到 reconciliation 结果不得造成重复发放或污染新角色 UI。（验证：identity 含 ticket 非明文指纹；ticket/player/character/endpoint 变化取消并清空旧 workflow，generation/identity 过滤迟到 claim/detail 响应）

## 阶段 6：通知刷新和身份生命周期

- 开始时间：2026-08-05 22:35:42 +08:00
- 结束时间：2026-08-05 22:54:57 +08:00
- 开发总结：完成 chat 邮件通知的应用级桥接与 generation 过滤，将跨帧重复 push 合并为单在途请求和最多一次后续权威刷新；补齐 ticket 轮换、账号/角色/环境切换的取消清理，并保持邮件 unavailable 与 chat/game 链路隔离。
- 验证记录：`cargo test mail::tests --lib`（27 项）、`cargo test game::myserver::types::tests --lib`（39 项）、`cargo test game::myserver::chat::tests --lib`（21 项）、`cargo test main_world_mail --lib`（18 项）、`cargo test logout --lib`（5 项）、`cargo fmt --check`、`cargo check` 和 `git diff --check` 通过；仅有既有/后续阶段预留 dead-code 警告。

- [x] 注册 `MailNotifyPush -> MailClientCommand::MailNotifyPush` 桥接，并对重复 push 保持幂等。（验证：`MailPlugin` 注册 `forward_chat_mail_notifications`，完整 drain 当前 chat generation 事件并只写一次命令；stale generation/同帧重复 push 测试通过）
- [x] 邮箱关闭时收到 push 更新 stale/unread 状态；邮箱打开时合并为最新一次权威刷新，避免请求风暴。（验证：`list_refresh_queued` 保证最多一个 List 在途和一个 queued follow-up；跨帧 push 测试确认两轮响应后以最新 HTTPS 结果收敛且无额外请求）
- [x] chat 不可用、push 丢失或客户端离线后，重新登录和打开邮箱仍能查询全部权威邮件。（验证：邮件查询不依赖 `ChatClientStatus`；主世界 open mail 继续主动发送 `LoadList`，host/mail 定向测试通过）
- [x] ticket 更新后新请求使用最新 ticket；旧 pending HTTP 响应按 identity/generation 丢弃。（验证：ticket 指纹变化取消旧 request，新 GET 只携带 rotated `X-Game-Ticket`，旧 response 不写缓存测试通过）
- [x] Logout、切环境、切账号和切角色取消旧请求并清空邮件列表、详情、未读和错误。（验证：四类 session 清理均清 mail descriptor 并触发 identity reset；pending cancel、缓存/error/queued 清空测试和 logout 测试通过）
- [x] `services.mail=null`、descriptor 非法和 HTTPS 不可达只禁用邮箱，不阻塞主世界或 chat/game 链路。（验证：null/非法 descriptor 不发邮件 HTTP；网络错误只令 mail Failed，`ChatClientState::Ready` 与主世界链路保持不变）

## 阶段 7：错误、限流和可观测性

- 开始时间：2026-08-05 22:56:07 +08:00
- 结束时间：2026-08-05 23:40:00 +08:00
- 开发总结：建立邮件 HTTP/transport 的稳定公开错误状态和可操作 UI 文案；按读取与领取能力域解析有界 `Retry-After` 并禁用重复操作，claim 429 冷却后仅允许玩家显式幂等重试；新增脱敏请求诊断与无 mail ID 标签的低基数计数。
- 验证记录：`cargo test mail::tests --lib`（36 项）、`cargo test main_world_mail --lib`（19 项）、`cargo fmt --check`、`cargo check` 和 `git diff --check` 通过；主审对照 MyServer 限流契约退回并修正 claim/read 分域及 429 冷却恢复，仅有既有/后续阶段预留 dead-code 警告。

- [x] 将 400/401/403/404/409/429/503、超时、响应过大和 JSON 错误映射为稳定 UI 状态。（验证：`public_http_error`、transport 分类与 `main_world_mail_error_message` 覆盖稳定 code/status；HTTP/JSON/timeout/body-limit 及 19 项 host 测试通过）
- [x] 尊重服务端 `Retry-After` 或稳定重试策略，限流期间禁用重复刷新/领取而不忙循环。（验证：仅接受 delta-seconds 并 clamp 1..300 秒，缺失/非法回退 2 秒；读取/领取分域 cooldown，reconciliation 延后不耗轮次；claim 429 冷却内单 POST、结束后只允许玩家显式重试）
- [x] 记录 operation、HTTP status、错误码、request generation 和脱敏 endpoint；不记录 ticket、正文和附件。（验证：`MailRequestDiagnostic` 与结构化日志仅含低基数字段和 endpoint fingerprint；诊断 Debug 泄漏哨兵测试通过）
- [x] 记录列表刷新、未读更新、领取 reconciliation 开始/完成/耗尽指标，避免高基数 mail ID 标签。（验证：`MailDiagnostics` 饱和计数由生命周期事件更新；成功收敛与三次耗尽测试核对计数，资源/日志均无 mail ID 维度）
- [x] 邮件服务灰度关闭或领取入口暂停时保留列表/详情可用能力，按服务端功能边界降级。（验证：claim pause 仅令 workflow Unavailable；claim 429 不阻塞 GET、list 429 不阻塞 claim 的交叉域测试通过，超时/断线/5xx 仍只进入 reconciliation）

## 阶段 8：自动化、UI 审计和服务端联调

- 开始时间：2026-08-05 23:41:00 +08:00
- 结束时间：2026-08-06 00:55:16 +08:00
- 开发总结：补齐邮箱极端内容的 Debug-only 审计 fixture，修正详情模式列表隐藏和全屏遮挡；完成四档窗口视觉审计，并在 MyServer 增加真实双账号/双角色 ownership、库存归属与重复领取验收。
- 验证记录：`cargo test mail::tests --lib`（37 项）、`cargo test main_world_mail --lib`（21 项）、审计 fixture 定向测试、`cargo fmt --check`、`cargo check`、UI boundary 和 `git diff --check` 通过；四份本地审计 capture 全部 passed；MyServer `npm --workspace mail-service test`（141 项）、core（9 项）、reliability（12 项）、outbox（9 项）、pubsub（5 项）均通过，双账号新夹具单项通过并提交为 MyServer `11f295e`。旧的非 canonical claim/store/auth fixture 存在已知契约漂移，不计入通过范围。

- [x] 增加 host allowlist、Repeat key、列表分页、迟到响应、未读同步和非法 mail ID 测试。（验证：`cargo test main_world_mail --lib` 21 项与 `cargo test mail::tests --lib` 37 项通过，覆盖 closed host registry、完整 opaque ID、分页合并、generation 丢弃和权威未读数）
- [x] 增加详情/已读幂等、202 reconciliation、面板关闭、切角色和 push 去重测试。（验证：mail/UI 定向测试覆盖详情预算、MarkRead、单 POST 对账、CloseTop、identity reset 和 push coalescing）
- [x] 运行 `cargo fmt`、`cargo check` 和 mail/UI 相关定向测试。（验证：`cargo fmt --check`、`cargo check`、37 项 mail、21 项 main_world_mail 及 1 项 fixture 测试通过；仅有已知 dead-code 警告）
- [x] 运行 UI boundary 检查和四种窗口档位视觉审计，覆盖长标题、长正文、多附件和错误状态。（验证：`stage18_main_world_mail` 注入长标题、超 2K 正文、32 附件、分页和 503；phone-landscape、phone-1080p-landscape、tablet-landscape 与 1600x720 四份 capture 全部 passed，boundary 全 true）
- [x] 在 MyServer 执行 mail public contract、ownership、领取幂等、限流和故障恢复测试前确认依赖与范围。（验证：确认 PostgreSQL/Redis/NATS/auth/mail/game 真实依赖和 canonical 范围；mail-service 141、core 9、reliability 12、outbox 9、pubsub 5 项通过，服务与端口已清理）
- [x] 使用两个账号/角色验证邮件 ownership 隔离和附件只发到 ticket 绑定角色。（验证：MyServer `mail-two-account-ownership-acceptance` 使用两套真实注册/登录/建角/ticket，证明跨账号 detail/read/claim 均 404，A 库存增加、B 不变且重复 claim 无二次发放）

## 阶段 9：文档同步

- 开始时间：2026-08-06 00:57:00 +08:00
- 结束时间：2026-08-06 01:08:00 +08:00
- 开发总结：将邮件公开 endpoint、身份、单次领取/对账、分域限流、push 权威边界和联调步骤同步到 MyServer 主题文档；补齐邮箱 approved document、Floating panel、typed Repeat/fallback 和四档审计说明，并修正上手文档中 Android 网络验收的旧边界。
- 验证记录：主审对照实现复核 endpoint、fallback 路径、host/layer、审计参数和 checklist 07 路径；UI boundary 全 true，引用路径、关键词及 `git diff --check` 通过。第 1/5 轮修正已消除上手文档“Android 不承担网络验收”与 checklist 07 的冲突；`CLAUDE.md` 无冲突，无需修改。

- [x] 更新 `docs/myserver/` 的邮件 endpoint、身份、通知、领取和本地/正式联调步骤。（验证：`邮件客户端公开契约.md` 记录正式 443 endpoint、Local HTTP policy、X-Game-Ticket、player/character 边界、对账与双账号步骤）
- [x] 更新 `docs/ui/` 的邮箱 document、主世界面板、动态列表和响应式验收说明。（验证：`UI声明式业务界面迁移基线.md` 新增 `game.main_world_mail` host、approved/promotion/fallback、typed Repeat 和 stage18 四档审计）
- [x] 记录 chat push 只是失效通知、HTTPS 列表是权威事实的架构边界。（验证：MyServer 主题文档明确 push 不得直接改写列表/未读/领取，HTTPS API 与 PostgreSQL 为权威）
- [x] 检查 `CLAUDE.md` 和上手文档是否需要补充邮件模块、环境变量或验收命令。（验证：`docs/bevy-getting-started.md` 修正 Windows 日常联调与 checklist 07 Android 公网 HTTPS/TLS、push、触控、前后台、弱网对账边界；`CLAUDE.md` 现有模块/环境/审计入口已足够）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-05 19:07:19 +08:00
- 结束时间：2026-08-06 01:08:00 +08:00
- 验收总结：邮箱宣言式 UI、分页/详情/已读/领取、权威未读与 push 失效刷新、单 POST 对账、身份隔离和错误/限流观测均完成。客户端测试、四档桌面视觉审计及 MyServer canonical/双账号验收通过；Android 公网真机项按边界保留给 checklist 07，不在本清单中冒充完成。

- [x] 用户可在主世界打开邮箱、查看分页列表和详情、标记已读并领取单封附件。（验证：closed actions、typed bindings、host adapter 及 21 项 main_world_mail 测试通过）
- [x] 未读角标、push 刷新、主动刷新和离线后重查都以 HTTPS 权威结果收敛。（验证：权威 unread 可见性、push coalescing、open/retry LoadList 和 chat 不可用时主动查询测试通过）
- [x] 重复领取、202 结果未知、限流、领取暂停和 manual review 均有准确状态且不会重复发放。（验证：37 项 mail 测试覆盖 202/网络/5xx、read/claim 429、pause 和耗尽；MyServer 库存/grant 幂等验收通过）
- [x] 关闭邮箱、切场景、切角色、切环境和断线不会泄漏旧邮件数据或取消必要 reconciliation。（验证：面板关闭保留全局 workflow，teardown/session identity reset 清缓存并丢弃迟到响应，断线未知结果继续有界 GET）
- [x] 客户端 UI/逻辑测试、MyServer 权限/幂等验证和桌面验收全部通过；Android 真机结果由 07 清单单独收口。（验证：client mail 37/UI 21、boundary、四档 capture 与 MyServer canonical/双账号验收通过：Android 公网仍明确交由 07）
