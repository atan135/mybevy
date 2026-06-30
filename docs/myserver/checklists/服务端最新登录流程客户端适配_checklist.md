# 服务端最新登录流程客户端适配 Checklist

## 目标

根据 MyServer 服务端最新完善版本的登录、选角和角色四属性流程，改造 mybevy 客户端主链路。交付内容包括：账号登录获取 access token、角色列表/创建/profile/选择、character-bound game ticket 生命周期管理、game proxy 鉴权和重连衔接、角色四属性查询与 push 消费、服务端错误与账号/角色状态展示、登录/选角 UI 交互优化、联调验收和必要文档更新。

本任务不直接实现完整账号中心、支付、实名、防沉迷、生产密钥管理、完整属性面板、完整称号/职业 UI、跨实例持久离线 push 补偿或服务端协议变更；如发现服务端协议缺口，记录为后续服务端任务。

## 基础原则

- [ ] 以服务端最新账号登录、角色生命周期、选角签票、响应字段、错误码和状态机为准，不保留与新协议冲突的客户端假设。
- [ ] 明确账号身份 `player_id` / `playerId` 与游戏内角色身份 `character_id` / `characterId` 的边界，玩法主体统一使用当前 ticket 绑定角色。
- [ ] `project/src/game/myserver/` 保持低层协议、HTTP、角色、ticket 和 game proxy 连接职责，避免 UI 页面直接依赖 protobuf 细节。
- [ ] 登录页和登录状态展示遵循现有 `framework/ui`、主题 token、i18n 和响应式布局约定。
- [ ] 保留开发期 guest login / 自动联调入口，但不能阻塞正式登录流程落地。
- [ ] 生产客户端默认只依赖 `auth-http` 和 `game-proxy` 玩家入口，不把 `game-server:7000` 或内部周边服务作为正式准入路径。
- [ ] 角色四属性（地、火、水、风）的永久状态以服务端 `affinity` / `mastery` 响应或 push 为准，客户端不本地预测永久变化。
- [ ] 登录链路日志兼顾安全和可定位性：线上默认脱敏，保留 request id、状态转换、错误码、连接信息、过期时间和凭据短指纹等诊断字段。
- [ ] 每个阶段完成后运行对应验证，并按阶段提交。

## 阶段 1：服务端流程和范围确认

- 开始时间：2026-06-28 12:23:39 +08:00
- 结束时间：2026-06-28 12:30:34 +08:00
- 开发总结：已完成 MyServer 最新账号登录、角色生命周期、选角签票、character-bound ticket、四属性和旧环境变量联调入口的范围确认。确认客户端第一版必须从登录直接 ticket 假设迁移到 access token -> 角色列表/创建/profile/选角 -> character-bound ticket -> game proxy AuthReq，并保留开发期 env fallback。
- 验证记录：worker `Banach` 只读核对服务端文档与代码、客户端现状，主审复核结论并确认 `git status --short` 无业务代码改动。

- [x] 梳理 MyServer 最新账号登录流程中的入口接口、请求字段、响应字段、错误码、账号状态和 access token 下发规则。（验证：`docs/协议与客户端/外部客户端接入说明.md:43`、`docs/协议与客户端/协议设计.md:1012`、`apps/auth-http/src/auth/auth.service.ts:68` 确认 login/register/guest-login 返回账号 access token 和账号状态错误码）
- [x] 梳理登录后角色流程：`GET /api/v1/characters`、`POST /api/v1/characters`、`GET /api/v1/characters/{character_id}/profile`、`POST /api/v1/characters/select`、`POST /api/v1/characters/delete`、`POST /api/v1/characters/restore` 和 `/api/v1/game-ticket/issue`。（验证：`docs/协议与客户端/协议设计.md:1019`、`apps/auth-http/src/characters/characters.controller.ts:16`、`apps/auth-http/src/game-ticket/game-ticket.controller.ts:69` 覆盖角色列表/创建/profile/select/delete/restore/issue）
- [x] 明确正式账号登录、游客登录、注册审核中、封禁、维护模式、版本不兼容、无角色、角色已删除、角色不可选和角色数量上限等状态在客户端第一版是否接入。（验证：`apps/auth-http/src/auth/auth.service.ts:102`、`docs/协议与客户端/协议设计.md:247`、`docs/协议与客户端/协议设计.md:1052` 确认第一版必须接入账号/角色阻断和数量上限；版本不兼容暂列服务端待确认）
- [x] 明确 `services` endpoint、game host、game port、transport、character-bound ticket 过期时间、access token 生命周期和 `world_id` 字段来源。（验证：`docs/协议与客户端/外部客户端接入说明.md:36`、`docs/协议与客户端/协议设计.md:1118`、`apps/auth-http/src/config.js:359`、`apps/auth-http/src/game-ticket/game-ticket.controller.ts:131` 确认 endpoint、TTL 和 world_id 来源）
- [x] 明确旧的缺少 `characterId` game ticket 没有兼容开关，客户端不再假设登录/游客登录会直接返回可进游戏 ticket。（验证：`docs/协议与客户端/协议设计.md:269`、`apps/auth-http/src/auth-store.js:65`、`apps/auth-http/src/config.js:102` 确认缺 `characterId` ticket 被拒绝且没有兼容开关）
- [x] 明确角色四属性首版范围：选角 profile 展示、进入游戏后 `GetCharacterElementsReq/Res(1413/1414)` 查询、`CharacterElementsChangePush(1505)` 消费，以及暂缓完整属性面板。（验证：`docs/协议与客户端/外部客户端接入说明.md:55`、`docs/协议与客户端/协议设计.md:295`、`docs/协议与客户端/协议设计.md:360`、`docs/游戏服与接入层/角色体系与四属性设计.md:1059` 确认 P1 四属性可接入和暂缓范围）
- [x] 确认旧的环境变量联调入口是否继续保留，以及正式流程与 `AUTHORITY_*` / `MYSERVER_*` 开发变量的优先级关系。（验证：`project/src/game/myserver/types.rs:35`、`project/src/game/myserver/plugin.rs:529`、`project/src/game/authority/plugin.rs:72`、`scripts/start-robot-sync-two-clients.ps1:153` 确认旧 env 作为开发期入口保留，正式 session 和 services endpoint 优先）
- [x] 形成阶段结论：列出本次必须实现、暂缓实现和需要服务端补充确认的清单。（验证：worker 输出明确必须实现正式/游客登录账号 session、角色列表/创建/profile/选角、ticket issue、AuthReq、错误码、四属性 1413/1414/1505；暂缓完整属性面板/称号职业 UI/跨实例 push；待确认版本不兼容错误码、维护模式范围、生产 protocol 和 access token 续期展示）

## 阶段 2：登录协议类型和响应解析

- 开始时间：2026-06-28 12:32:02 +08:00
- 结束时间：2026-06-28 12:50:36 +08:00
- 开发总结：已将 myserver 协议解析层调整为账号 session + 角色选择 + character-bound ticket 口径，补充角色、生命周期、四属性和 ticket payload 诊断类型，并为旧登录 ticket、缺少 `characterId` ticket、角色响应和四属性 payload 增加解析测试。为保持现有模块可编译，最小调整了 ticket issue 请求体、登录后无 ticket 的连接拦截和 1413/1414/1505 消息枚举/解码。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（14 passed）、`cargo check`、`git diff --check`，均通过。

- [x] 对比客户端当前 `GuestLogin`、`RefreshTicket`、`AuthReq`、`LoginResponse`、`TicketResponse` 与服务端最新账号登录、角色选择和 character-bound ticket 协议的差异。（验证：`project/src/game/myserver/types.rs:297` 将 `LoginResponse.ticket` 改为可选账号 session 口径，`project/src/game/myserver/plugin.rs:541` 让 ticket issue 必带当前 `character_id`，`project/src/game/myserver/plugin.rs:618` 阻止登录仅返回账号 session 时直接连接）
- [x] 更新 `project/src/game/myserver/types.rs` 中账号登录、access token、角色列表、角色 profile、角色生命周期、选角签票、ticket、endpoint、账号/角色状态和错误信息相关结构体。（验证：`project/src/game/myserver/types.rs:297`、`:312`、`:326`、`:331`、`:340`、`:351`、`:525` 定义登录、角色列表/profile/lifecycle/select/ticket/error 响应类型）
- [x] 为角色对象 snake_case 字段补充 serde 映射，包括 `character_id`、`character_id_short`、`display_discriminator`、`same_name_hint`、`world_id`、`appearance_json`、`last_login_at`、`deleted_at`、`position`、`attributes` 和 `lifecycle`。（验证：`project/src/game/myserver/types.rs:366` 的 `CharacterSummary` 覆盖 snake_case 字段，`cargo test myserver --lib` 中 `parses_character_list_and_empty_character_list` 通过）
- [x] 为 character-bound game ticket payload 补充客户端诊断解析字段：`playerId`、`characterId`、`worldId`、`exp`、`ver` 和必要的脱敏短指纹。（验证：`project/src/game/myserver/types.rs:620` 定义 `GameTicketPayload`，`:640` 解析 payload 和短指纹，`:987` 测试 `parses_character_bound_ticket_payload_with_fingerprints` 通过）
- [x] 为角色四属性补充类型映射：`affinity`、`mastery`、`GetCharacterElementsRes.elements`、`CharacterElementsChangePush(1505)` 的 `before` / `change` / `after` / `meta`。（验证：`project/src/game/myserver/types.rs:451`、`:463`、`:495`、`:514` 定义四属性 JSON 类型，`project/src/game/myserver/protocol.rs:66`、`:67`、`:72` 注册 1413/1414/1505，`project/src/game/myserver/plugin.rs:953`、`:1046` 解码 push/response）
- [x] 为服务端新增或调整的登录、角色和 ticket 响应字段补充 serde 映射，避免字段缺失导致整包解析失败。（验证：`project/src/game/myserver/types.rs:366` 使用可选字段和 `extra` 容忍未知字段，`:829` 测试缺省可选字段和未知 `future_field` 通过）
- [x] 保留 guest login 兼容路径，并明确正式登录和 guest login 共享账号 session 字段，但都必须经过角色选择后才能进游戏。（验证：`project/src/game/myserver/types.rs:754` 测试账号/游客登录无 ticket 可解析，`project/src/game/myserver/plugin.rs:612` 仅在有 ticket 时连接，否则提示选择角色）
- [x] 增加最小单元测试或解析测试，覆盖账号登录成功、角色列表、无角色、创建角色、选角签票、缺省可选字段、错误响应、未知附加字段和旧 ticket 缺 `characterId` 拒绝。（验证：`project/src/game/myserver/types.rs:754`、`:779`、`:829`、`:855`、`:876`、`:897`、`:938`、`:974`、`:987` 覆盖解析场景，`cargo test myserver --lib` 14 passed）
- [x] 运行 `cargo fmt` 和针对 myserver 模块的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 3：Session Resource 和 Typed Event

- 开始时间：2026-06-28 12:53:29 +08:00
- 结束时间：2026-06-28 13:14:34 +08:00
- 开发总结：已扩展 MyServer session resource，集中承载账号 token、角色列表、当前角色、endpoint、ticket 和四属性缓存；新增 session 写入/清理方法与 UI 可消费 typed events，并把登录、ticket 和四属性响应接入 session 更新。阶段 4 的正式 HTTP 请求链路和错误码映射仍按计划后续实现。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（19 passed）、`cargo check`、`git diff --check`，均通过。

- [x] 将 access token、refresh/token 生命周期、账号 `player_id`、login name、guest id、角色列表、当前 `character_id`、`world_id`、角色 profile、game ticket、ticket 过期时间和服务 endpoint 写入统一 session resource。（验证：`project/src/game/myserver/types.rs:78` 扩展 `MyServerSession` 字段，`:185`、`:206`、`:226`、`:244`、`:259` 定义登录/角色列表/选角/ticket/profile 写入方法）
- [x] 为登录成功、登录失败、角色列表加载成功/失败、需要创建角色、角色选择成功/失败、账号状态阻断、维护、封禁、版本不兼容和网络失败定义可被 UI 消费的 typed event。（验证：`project/src/game/myserver/types.rs:496` 的 `MyServerEvent` 新增角色列表、建角需求、选角、账号阻断、维护、封禁、版本不兼容、网络失败事件，`project/src/game/myserver/plugin.rs:361`、`:386`、`:427` 已发出网络失败事件）
- [x] 为角色四属性缓存建立最小状态：当前角色 `affinity`、`mastery`、最近已应用 `CharacterPushMeta.sequence/revision` 和快照刷新时间。（验证：`project/src/game/myserver/types.rs:372` 定义 `CharacterElementsCache`，`:288`、`:308` 写入 1414 响应和 1505 push，`project/src/game/myserver/plugin.rs:1071`、`:1091` 接入缓存更新事件）
- [x] 明确 session resource 的 reset、logout、切换账号、切换角色、角色删除/恢复和连接断开时字段清理规则。（验证：`project/src/game/myserver/types.rs:131`、`:139`、`:143`、`:149`、`:161`、`:320`、`:346` 定义 logout/switch/断连/生命周期清理规则）
- [x] 为可选字段、旧 guest login 响应、新账号登录响应、角色列表响应、选角响应和四属性响应补充 session 写入测试。（验证：`project/src/game/myserver/types.rs:754`、`:1175`、`:779`、`:855`、`:1411`、`:1490`、`:1563` 覆盖登录、角色列表、选角、四属性、清理和生命周期写入测试，`cargo test myserver --lib` 19 passed）
- [x] 运行 `cargo fmt` 和针对 myserver session/event 的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 4：登录命令和 HTTP 请求链路

- 开始时间：2026-06-28 13:17:15 +08:00
- 结束时间：2026-06-28 13:57:41 +08:00
- 开发总结：已接入账号登录、注册、游客登录、角色列表/创建/profile/选择、角色删除/恢复、ticket issue 和登出 HTTP 命令链路；新增 pending HTTP 请求表和同类请求拒绝策略，统一 HTTP 响应分发、JSON 错误解析和 typed event 输出。注册审核中响应已按服务端 `pendingReview=true` 单独识别，不再误报 JSON 解析失败或网络失败。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（31 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换。

- [x] 为正式登录、注册、游客登录、角色列表、角色创建、角色 profile、角色选择、角色删除、角色恢复、ticket 补发和登出整理 `MyServerCommand` 命令边界。（验证：`project/src/game/myserver/types.rs:499` 定义完整命令，`:430` 定义对应 pending HTTP operation，`:750` 定义 UI 可识别 operation）
- [x] 更新账号登录和角色 HTTP 请求构造，覆盖请求 body、Bearer access token headers、timeout、request id 和 Content-Type。（验证：`project/src/game/myserver/plugin.rs:573`、`:604`、`:635`、`:665`、`:696`、`:734`、`:768`、`:842`、`:887` 构造各入口请求，`:990` 统一写入 JSON/Bearer/timeout/request id）
- [x] 处理 HTTP 非 2xx、超时、网络失败和 JSON 解析失败，统一输出 typed event，并优先读取 JSON 中的 `error` / `errorCode`。（验证：`project/src/game/myserver/plugin.rs:1016` 分发 HTTP 响应，`:1261`、`:1294` 统一解析失败和非 2xx，`:1347` 优先读取 JSON 错误字段，`:1432` 将注册 `pendingReview=true` 转为 `AccountStatusBlocked`）
- [x] 避免登录、拉角色、创建角色、选角和补发 ticket 中重复发请求，明确同类 pending request 的覆盖、拒绝或取消策略。（验证：`project/src/game/myserver/plugin.rs:917` 统一发送前检查 pending，`:973` 通过 duplicate group 拒绝同类重复请求，`project/src/game/myserver/types.rs:430` 定义登录/角色/ticket/logout 分组来源）
- [x] 增加请求构造和失败路径测试，覆盖正式登录、guest login、角色列表、创建角色、选角签票、ticket issue、超时和非 2xx。（验证：`project/src/game/myserver/plugin.rs:2041`、`:2059`、`:2074`、`:2093`、`:2114` 覆盖请求构造，`:2168`、`:2183` 覆盖非 2xx/JSON 错误，`:2200`、`:2238` 覆盖注册审核中和注册成功，`:2261` 覆盖重复 pending）
- [x] 运行 `cargo fmt` 和针对登录请求链路的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 5：登录态状态机骨架

- 开始时间：2026-06-28 14:01:21 +08:00
- 结束时间：2026-06-28 14:53:42 +08:00
- 开发总结：已为 MyServer session 增加账号、角色选择和 game connection 三组显式状态枚举，并把 HTTP 请求入队、前置失败、响应失败、注册审核阻断、选角成功、profile 查看、ticket issue、连接、鉴权、断开、登出、切账号和切角色接入集中状态转换。修复审核中发现的选角失败污染当前 `character_id`、profile 误标已选择、普通补票误标断线和手动断开状态不明确问题。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（36 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换。

- [x] 建立账号登录态状态机，覆盖未登录、登录中、账号已登录、登录失败、被阻断、过期和已登出状态。（验证：`project/src/game/myserver/types.rs:78` 定义 `AccountLoginState` 全状态，`:426` 登录成功写 `LoggedIn`，`:377` 失败写入，`:1440` 通过错误分类接入阻断/过期）
- [x] 建立角色选择状态机，覆盖未加载、加载中、无角色、创建中、待选择、profile 加载中、选择中、已选择、角色阻断和选角失败状态。（验证：`project/src/game/myserver/types.rs:95` 定义 `CharacterSelectionState` 全状态，`:449`、`:479`、`:486`、`:521` 分别写入列表/创建/选角/profile 成功状态，`:1459` 接入角色失败/阻断）
- [x] 建立 game connection 状态机，覆盖未连接、连接中、已连接、鉴权中、已鉴权、断开、重连中和重连失败状态。（验证：`project/src/game/myserver/types.rs:115` 定义 `GameConnectionState` 全状态，`:307`、`:313`、`:229`、`:315`、`:341`、`:349` 定义 ticket/连接/断开/鉴权转换）
- [x] 将 `GuestLogin`、正式登录、角色命令、ticket issue、`ConnectGame`、`Disconnect` 的基础状态转换集中处理，避免 UI 和玩法直接改 session 状态。（验证：`project/src/game/myserver/types.rs:357`、`:377` 集中处理 HTTP 开始/失败，`project/src/game/myserver/plugin.rs:992` 请求入队触发状态，`:1782` disconnect 触发 `disconnect_cleanup`，`:1795`、`:1988`、`:1994` 接入鉴权状态）
- [x] 处理重复点击登录、登录中切换账号、选角中切换角色、ticket 补发中断线、断线时再次登录等并发边界。（验证：`project/src/game/myserver/types.rs:145` 新增 `pending_character_id` 防止选角中污染当前角色，`:307`、`:313` 区分普通补票和重连补票，`:1807`、`:1871`、`:1914` 测试切账号/前置失败/重复命令和连接边界）
- [x] 增加状态机测试，覆盖成功登录、无角色创建、选角成功、登录失败、选角失败、登出、重复命令去重、切换账号清理和切换角色清理。（验证：`project/src/game/myserver/types.rs:1656`、`:1717`、`:1807`、`:1871`、`:1914` 覆盖成功登录/无角色/选角/失败/登出/重复命令/切换清理/连接状态）
- [x] 运行 `cargo fmt` 和针对登录态状态机的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 6：Ticket Issue 和 Game Auth 链路

- 开始时间：2026-06-28 14:56:47 +08:00
- 结束时间：2026-06-28 15:52:39 +08:00
- 开发总结：已串联 character-bound ticket 提前补发、ticket issue 响应处理、ConnectGame 和 game proxy AuthReq 鉴权链路；补票与 keepalive ping 开关解耦，补票请求始终携带当前已选 `character_id`。同时补充旧 ticket 拒绝、角色不匹配拒绝、reconnect 使用新 ticket、game auth 失败原因分类和相关测试。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（45 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换。

- [x] 按 character-bound ticket 过期时间触发提前补发，补发请求必须携带当前选中 `character_id`，失败时输出明确事件，不静默丢失连接状态。（验证：`project/src/game/myserver/types.rs:199`/`:209` 解析过期时间并判断刷新阈值，`project/src/game/myserver/plugin.rs:559`/`:585` 在已鉴权连接中触发补票，`:899` 构造 ticket issue，`:908`/`:919`/`:1475` 输出明确失败事件，`:2613` 覆盖 keepalive 关闭仍可补票）
- [x] 将 `/api/v1/game-ticket/issue`、`ConnectGame` 和 game proxy 连接成功后的 `AuthReq` 串成独立链路。（验证：`project/src/game/myserver/plugin.rs:899` 发起 ticket issue，`:1697` 处理 ticket response，`:1792` 进入 ConnectGame，`:1886` 校验最新 ticket，`:1926` 使用 `AuthReq` 期待 `AuthRes`）
- [x] 在 game proxy 连接成功后使用最新 character-bound ticket 发起 `AuthReq`，鉴权失败时区分 ticket 过期、缺少 `characterId`、账号阻断、角色阻断和协议错误。（验证：`project/src/game/myserver/plugin.rs:1886` 连接后校验 ticket 与当前角色，`:2111` 处理 `AuthRes`，`:2124`/`:2131` 发出 `GameAuthRejected`；`project/src/game/myserver/types.rs:1084`/`:1552` 定义并分类失败原因）
- [x] 明确 ticket issue 期间已有 game connection 的保留、重连或断开策略，并避免使用旧角色 ticket 连接新角色会话。（验证：`project/src/game/myserver/plugin.rs:1750` 普通补票只刷新 session ticket，`:1754` 登录后连接才进入连接计划，`:1792`/`:1837` 拒绝角色不匹配 ticket，`:2669` 覆盖 reconnect 使用响应 ticket 并断开旧连接）
- [x] 增加测试，覆盖未登录无法签发、未选角色无法签发、ticket 补发成功、补发失败、旧 ticket 缺 `characterId`、game auth 成功和 game auth 失败。（验证：`project/src/game/myserver/plugin.rs:2510`、`:2543`、`:2613`、`:2721`、`:2758`、`:2810` 覆盖签发前置、补票、旧 ticket、auth 成功和失败分类；`project/src/game/myserver/types.rs:2765`/`:2810` 覆盖刷新 helper 和失败分类）
- [x] 运行 `cargo fmt` 和针对 ticket/auth 链路的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 7：诊断日志和脱敏策略

- 开始时间：2026-06-28 15:56:05 +08:00
- 结束时间：2026-06-28 16:25:38 +08:00
- 开发总结：已新增 MyServer 诊断快照和稳定 12 位短指纹脱敏能力，并为 HTTP 请求/响应、登录 session、ticket issue、game connect、AuthReq/AuthRes、redirect、kick 和断连相关路径补充结构化 trace/debug/info/warn 日志。详细状态转换默认 trace，本地可通过 `MYSERVER_DIAGNOSTIC_TRACE` 提升到 debug；新增日志只输出脱敏短指纹，不输出 access token、ticket 或密码明文。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（48 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换。

- [x] 为关键状态转换增加日志或 debug trace，记录 request id、seq、connection id、endpoint、登录状态、连接状态、HTTP status、服务端错误码、ticket 剩余时间和脱敏短指纹。（验证：`project/src/game/myserver/plugin.rs:83`/`:134` 定义 HTTP/game 诊断 trace，`:1224`/`:1289`/`:1354` 记录 HTTP request/response，`:2197`/`:2216` 记录 AuthReq 真实 seq）
- [x] 禁止记录 access token、ticket、密码等敏感值明文，并为脱敏短指纹定义稳定算法和长度。（验证：`project/src/game/myserver/types.rs:24` 定义 12 位长度，`:1540`/`:1604` 定义脱敏短指纹，`project/src/game/myserver/plugin.rs:1905`/`:1999`/`:2227` 日志使用 `*_fp` 字段而非明文凭据）
- [x] 区分本地调试和线上定位的日志策略，明确 debug trace 开关、日志级别、采样规则和敏感字段脱敏规则。（验证：`project/src/game/myserver/plugin.rs:68` 通过 `MYSERVER_DIAGNOSTIC_TRACE` 将默认 trace 提升为 debug，`:83`/`:134` 中 debug/trace 分支均只输出状态和短指纹）
- [x] 确认登录、ticket issue、game auth、redirect、kick 和 reconnect 能通过 request id、connection id、状态转换和脱敏短指纹串联定位。（验证：`project/src/game/myserver/plugin.rs:1905` 登录、`:1999` ticket issue、`:2124` connect/reconnect、`:2216` AuthReq、`:2438` AuthRes、`:2584` redirect、`:2622` kick 均写入关联字段）
- [x] 增加日志脱敏测试或最小断言，覆盖凭据明文不会进入格式化日志字段。（验证：`project/src/game/myserver/types.rs:2867` 覆盖 access token/ticket/password 指纹长度和稳定性，`:2886` 覆盖 diagnostic snapshot Debug 文本不包含明文 token/ticket；`project/src/game/myserver/plugin.rs:2988` 覆盖 AuthReq packet seq 与 pending request 串联）
- [x] 运行 `cargo fmt` 和针对诊断/脱敏逻辑的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 8：错误码和失败展示模型

- 开始时间：2026-06-28 16:28:01 +08:00
- 结束时间：2026-06-28 17:11:54 +08:00
- 开发总结：已新增统一 `MyServerDisplayError` 展示错误模型、稳定错误类型、错误来源、可本地化 message key、retryable/blocking 元数据，并把 HTTP、客户端前置失败、game auth、room join、角色四属性和协议解析失败统一输出为 `DisplayError`。主审打回并修复了角色封禁被误判为账号封禁、ticket issue 前置失败漏发稳定错误、2xx `ok=false` 丢失服务端错误码的问题。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（58 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换。

- [x] 梳理并接入登录、角色生命周期、选角签票、ticket、game auth、room join 和角色四属性相关错误码到统一错误展示模型。（验证：`project/src/game/myserver/types.rs:1129` 定义稳定错误类型，`:1159` 定义 `MyServerDisplayError`；`project/src/game/myserver/plugin.rs:1814`/`:1874` 统一 HTTP 显示错误，`:3934`/`:4011` 覆盖 game auth、room join 和四属性失败事件）
- [x] 将账号阻断、IP 阻断、动态黑名单不可用、维护、封禁、审核中、版本不兼容、角色不可选、角色数量上限、ticket 过期、缺少 `characterId` 和未知错误映射为稳定客户端错误类型。（验证：`project/src/game/myserver/types.rs:1385` 分类服务端错误码，`:3382` 覆盖 IP/玩家阻断/黑名单/缺角色/限流/生命周期/ticket/未知码，`:3502` 确认 `CHARACTER_BANNED` 不误判为账号封禁）
- [x] 为 UI 准备可本地化的错误文案 key，不在 UI 中直接拼接底层协议错误。（验证：`project/src/game/myserver/types.rs:1326` 为每个 `MyServerErrorKind` 输出 `myserver.error.*` message key，`:1357`/`:1370` 提供 retryable/blocking 元数据）
- [x] 对 HTTP 非 2xx、JSON 解析失败、protobuf decode 失败、连接超时和 transport 失败提供稳定错误事件。（验证：`project/src/game/myserver/plugin.rs:1599`/`:1661` 处理 HTTP 非 2xx、2xx `ok=false` 和 JSON 解析失败，`:590`/`:707`/`:745` 处理 transport/连接失败，`:2617`/`:2809`/`:2839`/`:2964`/`:3004` 处理 protobuf decode 失败）
- [x] 增加错误映射测试，覆盖 `IP_BLOCKED`、`PLAYER_BLOCKED`、`BLOCKLIST_UNAVAILABLE`、`MISSING_CHARACTER_ID`、`PREAUTH_MESSAGE_NOT_ALLOWED`、`MSG_RATE_EXCEEDED`、角色生命周期错误、已知错误码、未知错误码、网络错误和协议解析错误。（验证：`project/src/game/myserver/types.rs:3382` 覆盖稳定错误码映射，`project/src/game/myserver/plugin.rs:3389` 覆盖 ticket 前置失败，`:3805`/`:3836`/`:3874`/`:3902`/`:3934`/`:4011` 覆盖 HTTP、ok=false、JSON、transport、game domain 和协议解析失败）
- [x] 运行 `cargo fmt` 和针对错误模型的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 9：Redirect、Kick 和连接恢复

- 开始时间：2026-06-28 17:15:55 +08:00
- 结束时间：2026-06-28 18:16:39 +08:00
- 开发总结：已将 `ServerRedirectPush` 接入 fresh ticket 重连恢复链路，按服务端 endpoint 切换目标并在重新鉴权成功后发送 `RoomReconnectReq`，避免普通 `Authenticated` 触发重复 join。已将 `SessionKickPush` 分类为并发登录、封禁、维护、服务端下线和未知原因，写入登录态、阻断重连并输出 UI 可消费事件。主审打回并修复了 reconnect plan 在真实 redirect 链路中被过早清理、失败后 stale plan 残留、kick 后重新登录仍阻断连接的问题。
- 验证记录：主审运行 `cargo fmt --check`、`cargo test myserver --lib`（67 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换。

- [x] 将 `ServerRedirectPush` 接入连接恢复流程，按服务端下发 endpoint 切换目标并重新鉴权。（验证：`project/src/game/myserver/plugin.rs:3183` 处理 redirect endpoint，`:3337` 发出重连开始事件，`:2336`/`:2589` 在 fresh ticket reconnect 中保留 reconnect plan，`:4193` 测试完整 redirect -> ticket -> connect -> AuthRes ok 链路）
- [x] 将 `SessionKickPush` 接入登录态和 UI 提示，区分并发登录、封禁、维护、服务端主动下线和未知原因。（验证：`project/src/game/myserver/plugin.rs:3359` 处理 kick push，`:3418` 按分类写入登录态并发出 `AccountStatusBlocked`/`AccountBanned`/`MaintenanceBlocked`，`project/src/game/myserver/types.rs:1468` 定义分类，`project/src/game/myserver/plugin.rs:4579` 覆盖并发登录/封禁/维护）
- [x] 确认 redirect、kick 和 ticket issue 与 authority endpoint 不产生重复 join 或 stale session。（验证：`project/src/game/myserver/plugin.rs:2963` reconnect auth 发 `ReauthenticatedForReconnect` 而非普通 `Authenticated`，`:4327`/`:4381` 覆盖 ticket 失败和 auth 拒绝后清理 stale reconnect plan，`:4517`/`:4648` 覆盖 kick 阻断重连和重新登录解除阻断）
- [x] 在 `RoomReconnectReq` 中带回当前角色最近已应用的 `last_character_push_sequence`，并明确该字段只用于当前 ticket 绑定角色的角色状态 push 补偿。（验证：`project/src/game/myserver/plugin.rs:2692` 使用 `session.character_elements.last_push_sequence.unwrap_or_default()` 构造 `RoomReconnectReq`，`:4438` 测试携带 sequence 42）
- [x] 明确 reconnect 成功后大厅、房间、authority 会话、角色四属性、称号和职业快照的恢复边界；断线重连后至少重新拉取四属性快照。（验证：`project/src/game/myserver/plugin.rs:3088` 处理 `RoomReconnectRes` 恢复 `room_id`，`:3097` 发送 `GetCharacterElementsReq` 刷新四属性，代码注释明确称号/职业仍走 profile/HTTP 刷新边界）
- [x] 增加最小恢复流程测试或模拟事件测试，覆盖 redirect 后重连、kick 后阻断重连、角色 push sequence 补偿和未知错误兜底。（验证：`project/src/game/myserver/plugin.rs:4193`、`:4327`、`:4381`、`:4438`、`:4517`、`:4579`、`:4648` 覆盖 redirect 成功/失败、kick 阻断、sequence 补偿、未知 reason 和重新登录解除阻断；`project/src/game/myserver/types.rs:2673` 覆盖 reset 清理语义）
- [x] 运行 `cargo fmt` 和针对恢复流程的可用测试或 `cargo check`。（验证：主审运行 `cargo fmt --check`、`cargo test myserver --lib`、`cargo check`、`git diff --check` 全部通过）

## 阶段 10：登录与选角页面结构和输入控件

- 开始时间：2026-06-28 18:20:10 +08:00
- 结束时间：2026-06-28 19:37:18 +08:00
- 开发总结：已将登录页升级为正式账号登录、游客登录、角色列表、建角、选角和开发期 Lobby 入口的页面结构。登录页按钮 now 发送真实 `MyServerCommand`，登录成功后自动拉角色，建角后自动选角并进入 Lobby；切换账号会发 Logout 并清空账号/密码/角色名输入。主审打回并修复了开发期入口重复路由、初始禁用按钮延迟生效和 UI 事件测试覆盖不足的问题。
- 验证记录：worker `Popper` 完成实现并经过 3 轮主审修复；主审运行 `cargo fmt --check`、`cargo test auth_ --lib`（14 passed）、`cargo test myserver --lib`（67 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换 warning。

- [x] 将登录页从单一 Guest Login 入口升级为可展示正式登录、游客登录、角色选择/创建和开发期入口的页面结构。（验证：`project/src/game/screens/auth/login.rs:214` 构建账号登录区，`:376` 构建角色区，`:577` 保留开发期 Lobby 入口，`project/src/game/screens/auth/mod.rs:17` 注册登录页事件处理系统）
- [x] 增加账号输入、密码输入、登录按钮、游客登录按钮、切换/清理账号入口、角色列表、创建角色入口、角色名输入、选角按钮和必要的 loading 占位。（验证：`project/src/game/screens/auth/login.rs:242`/`:252`/`:271`/`:282` 定义账号/密码/登录/游客入口，`:362` 切换账号入口，`:429` loading 文案占位，`:471` 角色名输入，`:481` 建角按钮，`:496` 角色行选角按钮，`:922` 清空输入）
- [x] 角色列表展示 `name` 时，在需要精确区分的场景同时展示 `character_id_short` 或 `display_discriminator`，不把角色名当唯一键。（验证：`project/src/game/screens/auth/login.rs:531` 展示角色名，`:538` 展示区分明细，`:989` 优先使用 `display_discriminator` / `character_id_short` / id 后缀，`:1069` 测试同名角色保留不同 `character_id` 和 detail）
- [x] 登录、拉角色、创建角色和选角按钮在请求中禁用，并避免重复点击产生多次请求。（验证：`project/src/game/screens/auth/login.rs:615`/`:639` 初始 disabled 直接复用通用 disabled button，`:673` 事件处理按 pending gate 发命令，`:813` 同步 disabled/loading 状态，`:1173` 测试同帧角色请求去重，`:1210` 测试 pending 角色状态阻断请求）
- [x] 保持 UI 风格与现有 `framework/ui/style/theme.rs`、通用控件和页面层级一致，不引入孤立样式体系。（验证：`project/src/game/screens/auth/login.rs:214` 继续使用 `UiTheme`/`UiMetrics`/`screen_label_key`/`text_input`/action button 构建页面，`project/src/framework/ui/widgets/mod.rs:11` 仅补充导出 `UiTextInputValue` 以复用现有输入控件）
- [x] 增加最小 UI 状态测试或手动验收记录，覆盖账号输入、角色创建、角色选择、按钮禁用和 guest login 入口仍可访问。（验证：`project/src/game/screens/auth/login.rs:1087` 账号输入触发 Login，`:1110` guest 入口触发 GuestLogin，`:1128` 角色名触发 CreateCharacter，`:1150` 选角使用 `character_id`，`:1173`/`:1210` 覆盖重复点击和 pending gate，`:1227` 覆盖切换账号清空输入）
- [x] 运行 `cargo fmt` 和 `cargo check`。（验证：主审在 `project/` 运行 `cargo fmt --check`、`cargo check` 通过，并补充运行 `cargo test auth_ --lib`、`cargo test myserver --lib`、`git diff --check` 通过）

## 阶段 11：登录/选角状态绑定和响应式验收

- 开始时间：2026-06-28 19:40:36 +08:00
- 结束时间：2026-06-28 20:21:27 +08:00
- 开发总结：已将登录页动态区改为稳定展示模型，绑定账号态、角色态、game connection、最近 `MyServerDisplayError`、页面 notice 和当前角色四属性快照。页面现在能展示登录/角色/ticket/game auth/网络/维护/封禁/审核/版本不兼容/kick 等状态，选角成功或 Authenticated 后清空输入、错误和焦点并跳 Lobby。主审打回并修复了 `AccountStatusBlocked` 被统一误标为审核中的问题，改为按稳定 code 区分审核中、异地登录/kick 和通用账号阻断。
- 验证记录：worker `Hypatia` 完成实现并经过 1 轮主审修复；主审运行 `cargo fmt --check`、`cargo test auth_ --lib`（16 passed）、`cargo test myserver --lib`（67 passed）、`cargo check`、`git diff --check`，均通过；`git diff --check` 仅提示工作区 LF/CRLF 转换 warning。worker 短时启动 `phone-small`、`phone-portrait`、`phone-1080p`、`tablet-portrait`、`tablet-landscape` 至 30s 超时，未 panic，主审复核无残留 `project`/`cargo`/`rustc` 进程。

- [x] 为登录中、角色加载中、无角色、创建角色中、选角中、签票中、成功跳转、登录失败、选角失败、网络异常、维护、封禁、审核中、版本不兼容和被踢下线设计明确 UI 状态。（验证：`project/src/game/screens/auth/login.rs:407`/`:1344` 展示账号和角色状态，`:1359` 展示 ticket/game connection 状态，`:674` 展示错误/notice 面板，`:1010` 消费失败/维护/封禁/版本/kick 事件，`:1489` 按 code 区分审核、异地登录和通用账号阻断）
- [x] 将登录态、角色态、连接态、四属性快照状态和错误展示模型绑定到登录/选角页面，不让 UI 直接依赖底层协议细节。（验证：`project/src/game/screens/auth/login.rs:145` 从 `MyServerSession` 和 `LoginUiState` 构建稳定 snapshot，`:185` 从 `MyServerDisplayError` 提取展示快照，`:1372` 只读取 session 的当前角色四属性 cache，`project/src/game/myserver/mod.rs:7` 仅 re-export 稳定类型给 UI）
- [x] 选角成功并跳转 Lobby 或服务端指定入口时，页面状态清理干净，不遗留 loading、错误浮层或旧输入焦点。（验证：`project/src/game/screens/auth/login.rs:293` OnExit 清理 `LoginUiState` 和焦点，`:1056` 在 `CharacterSelected`/`Authenticated` 时清空输入、焦点和 runtime state 后写入 Lobby 路由）
- [x] 如展示角色 profile，明确四属性 `affinity` / `mastery` 是服务端长期状态；创建角色阶段不提供初始偏向模板，默认均衡值由服务端决定。（验证：`project/src/game/screens/auth/login.rs:759` 仅在 session 有当前角色四属性快照时展示 profile，`:790` 标明 affinity/mastery 来自长期服务端状态，`:569` 创建角色仍只有角色名输入，`:1719` 测试只展示当前角色服务端 cache）
- [x] 在 phone-small、phone-portrait、phone-1080p、tablet-portrait、tablet-landscape 下检查布局不溢出、不遮挡、按钮可点击。（验证：worker 短时启动五个窗口 profile 均运行到 30s 无 panic；`project/src/game/screens/auth/login.rs:230` 页面纵向滚动，`:354`/`:509`/`:616` 按钮和行布局使用 `flex_wrap`，主审确认无残留运行进程）
- [x] 检查输入焦点、触控命中区域、键盘/鼠标操作、文本对比度和基础可访问性。（验证：`project/src/game/screens/auth/login.rs:330`/`:340`/`:569` 继续复用 framework `text_input`，`:852`/`:876` 继续复用 action button/disabled button，`:293`/`:1058` 清理 `UiFocusState`；文本颜色使用主题 Primary/Muted/Error 角色，未引入孤立样式）
- [x] 运行 `cargo fmt`、`cargo check`，并记录可用窗口 profile 的手动 UI 验收结果。（验证：主审运行 `cargo fmt --check`、`cargo check` 通过；补充运行 `cargo test auth_ --lib`、`cargo test myserver --lib`、`git diff --check` 通过；worker 记录五个窗口 profile 短时启动无 panic）

## 阶段 12：业务回归、联调文档和归档准备

- 开始时间：2026-06-28 20:26:01 +08:00
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 将账号登录和选角成功后的 session 信息接入大厅、房间和 authority MyServer endpoint 使用路径。
- [ ] 确认房间、匹配、输入、移动、战斗、背包和 transfer 的客户端路径使用当前 `character_id` 作为玩法主体，不新增 `player_id` fallback 或双字段兼容。
- [ ] 确认 Touch Ripple、Robot Sync 和现有 `AUTHORITY_DEV_MODE=myserver` 联调入口仍可使用。
- [ ] 使用 MyServer 本地完整栈执行正式登录或 guest login、拉取/创建角色、选择角色、签发 character-bound ticket 到 game proxy 鉴权闭环。
- [ ] 验证登录后加入 `ui_touch_room`、`robot_sync_room` 或当前可用测试房间的最小流程。
- [ ] 验证 ticket 提前补发、断开重连、redirect、kick 或服务端维护模式的可观察行为。
- [ ] 验证进入游戏后可查询当前角色四属性，并能消费或兜底刷新 `CharacterElementsChangePush(1505)`。
- [ ] 检查日志中不输出 access token、ticket、密码或其他敏感凭据明文，同时保留 request id、状态转换、HTTP status、错误码、endpoint、ticket 过期/剩余时间和脱敏短指纹，保证本地调试和线上问题可串联定位。
- [ ] 更新 `docs/bevy-getting-started.md` 中 MyServer 登录、选角、character-bound ticket、环境变量、本地联调和常见失败路径说明。
- [ ] 如 UI 页面行为或限制变化，更新 `docs/ui/` 下相关文档。
- [ ] 如 MyServer 客户端模块边界、命令或事件变化，更新 `CLAUDE.md` 或相关架构说明。
- [ ] 记录服务端最新登录/选角流程对应的客户端验收步骤，包括本地栈启动、登录、拉角色、创建角色、选角、签票、进房、断线、重连、kick 和维护模式。
- [ ] 记录角色四属性客户端验收步骤，包括 profile 展示、`GetCharacterElementsReq/Res`、push 去重/保序、断线后快照重拉和不本地预测永久变化。
- [ ] 记录登录链路日志与诊断字段说明，包括 request id 串联方式、凭据脱敏短指纹规则、线上日志级别和敏感信息禁止项。
- [ ] 整理暂缓项和后续任务，例如游客升级、登录态持久化、安全存储、正式域名/证书配置、完整属性面板、完整称号/职业 UI 和跨实例持久离线 push 补偿。
- [ ] 确认本 checklist 完成后按仓库约定归档到 `docs/<领域>/checklists/`。

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 客户端能按 MyServer 最新登录/选角流程完成至少一种正式可用路径，并正确建立账号登录态和当前角色态。
- [ ] 客户端能拉取或创建角色、选择角色、获取和补发 character-bound game ticket，并用最新 ticket 完成 game proxy 鉴权。
- [ ] 客户端不会使用缺少 `characterId` 的旧 ticket 进入游戏；玩法主体使用当前 `character_id`，账号级链路才使用 `player_id`。
- [ ] 客户端能查询当前角色四属性，并能按服务端响应或 `CharacterElementsChangePush(1505)` 更新长期 `affinity` / `mastery` 缓存。
- [ ] 登录失败、角色流程失败、账号阻断、角色阻断、维护、封禁、版本不兼容、kick、redirect 和网络异常都有可理解的 UI 展示或明确日志。
- [ ] 登录、ticket issue、game auth、redirect、kick 和 reconnect 能通过 request id、connection id、状态转换和脱敏短指纹在本地与线上日志中串联定位。
- [ ] 登录/选角页在主要手机和平板窗口 profile 下无文本溢出、控件遮挡或重复点击导致的异常请求。
- [ ] Touch Ripple、Robot Sync 和现有 MyServer 开发期联调入口没有被新登录流程破坏。
- [ ] `cargo fmt` 和 `cargo check` 通过。
- [ ] 相关文档已更新，未完成的服务端依赖或后续功能已记录。
