# 活动历史领取记录 Checklist

## 目标

为活动列表提供“我的领取记录”入口，使用服务端 character-bound 查询展示当前角色自己的历史领取记录，并支持分页、空状态、失败和重试。

## 基础原则

- [x] 历史记录来自服务端，不从本地动作日志、缓存或当前进度推导。（服务端从 PostgreSQL `activity_claim_record` 查询 append-only 事实。）
- [x] 记录查询只允许读取当前 character 有权读取的数据。（请求从已认证 character-bound ticket 推导身份，SQL 强制 `character_id` 条件，游标 HMAC 绑定 character。）
- [x] 服务端未分配协议和 msgType 前，客户端不自行假设 wire contract。（服务端已正式分配 `1445/1446`；客户端 vendor 已同步当前协议拆分。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义历史领取记录页面、服务端查询边界、分页字段和安全要求。
- 文档名称：MyServer 活动功能总纲
  - 路径或链接：`../MyServer/docs/游戏服与接入层/活动系统/活动功能总纲.md`
  - 用途说明：确认记录来源、角色范围和服务端事实存储语义。
- 文档名称：活动模块 02：玩家入口与统一领奖
  - 路径或链接：`../MyServer/docs/游戏服与接入层/活动系统/checklists/活动模块02_玩家入口与统一领奖_checklist.md`
  - 用途说明：确认统一领奖和历史记录查询的服务端边界。

## 阶段 1：历史记录协议契约

- 开始时间：2026-08-24 21:37:55 +08:00
- 结束时间：2026-08-25（服务端阶段）
- 开发总结：已完成 MyServer 服务端历史领取记录协议、character-bound keyset 分页查询、HMAC 游标、奖励摘要脱敏、错误语义、指标、数据库索引和 mock-client 编解码；客户端 vendor proto、状态和页面在后续阶段完成，真实联调仍待执行。服务端协议重构提交 `ae7c6f3adafcd364530ae4b27edb59497506da44` 后，客户端 vendor 已更新并记录来源。
- 验证记录：`cargo test --locked --manifest-path apps/game-server/Cargo.toml activity:: -- --nocapture`（84 passed）；`npm test --prefix tools/mock-client`（20 passed）；`npm run check:proto:server`（baseline、server-only generation、breaking check、routing consistency、6 fixtures 全部通过；明确跳过 mybevy vendor）。

- [x] 与服务端确认 `ActivityClaimHistoryReq/Res` 的字段、分页方式、权限和错误码。（`packages/proto/game.proto`；请求仅 `cursor/limit`，响应 `records/next_cursor/has_more`，角色从 ticket 推导。）
- [x] 由服务端分配正式 msgType `1445/1446`，并同步 MyServer 生成代码、`myserver-protocol`、game-proxy、mock-client 和协议基线。（客户端 `project/vendor/myserver/proto/` 已同步 `game.proto`、`game_item.proto`、`game_activity.proto` 及来源 README，来源提交为 `ae7c6f3adafcd364530ae4b27edb59497506da44`。）
- [x] 明确活动 ID/版本、阶段或动作标识、领取时间、结果状态和奖励摘要字段。（记录不公开 `character_id`、ticket、client request id、订单快照或内部错误。）
- [x] 明确空列表、游标失效、记录不存在、权限拒绝和存储不可用行为。（空列表成功返回；无效/篡改/跨角色游标为 `ACTIVITY_HISTORY_CURSOR_INVALID`；未鉴权连接由统一 gate 返回 `NOT_AUTHENTICATED` 或 `AUTH_CONTEXT_INCOMPLETE`，引擎上下文缺失时为 `ACTIVITY_AUTH_REQUIRED`；存储错误为 `ACTIVITY_STORAGE_UNAVAILABLE`；不存在记录不单独泄露。）

### 服务端改造完成项

- [x] `game-server` 新增 `ActivityClaimHistoryReq/Res` 路由，服务端从认证上下文取得 character，不接受请求体中的角色 ID。
- [x] PostgreSQL 历史 store 使用 `(character_id, created_at DESC, id DESC)` keyset 查询；补充对应数据库索引 migration。
- [x] 游标使用 `ticket_secret` HMAC，并包含版本、character 摘要、时间戳和记录 ID；无效、篡改或跨角色复用统一拒绝。
- [x] 奖励只返回 `granted` 记录中最多 16 项 item ID/数量摘要；`manual_review`、失败、处理中和其他非 granted 状态不返回奖励快照。
- [x] 增加 `history` 低基数活动指标维度及 `ACTIVITY_INVALID_REQUEST`、`ACTIVITY_HISTORY_CURSOR_INVALID`、`ACTIVITY_STORAGE_UNAVAILABLE` 错误语义。
- [x] 增加 Rust history store/engine 安全与分页测试、重复语义幂等/不同事实不折叠测试、mock-client 编解码契约测试；真实 PostgreSQL、带角色 ticket 的 KCP/TCP 会话和跨服务联调尚未执行。

### 服务端改动需求（2026-08-25）

1. 在 `H:\project\MyServer\packages\proto\game.proto` 分配一对新的正式 `ActivityClaimHistoryReq/Res` msgType，并更新 message mapping、游戏服 handler、协议文档和 mock-client。
2. 请求不得携带 `character_id`，从已认证的 character-bound game ticket 推导身份；字段仅包含受限分页 `cursor` 和 `limit`。
3. 响应应返回 `records`、`next_cursor`、`has_more`；每条公开记录至少包含活动 ID/版本、阶段或动作标识、完成时间、公开结果状态和受限奖励摘要。不得暴露 ticket、完整 client request ID、订单快照、内部错误或其他角色数据。
4. 查询必须以 `character_id` 强制隔离；补充 invalid/expired ticket、cursor invalid、空列表、storage unavailable、manual review 和 duplicate 记录的契约/集成/mock-client 测试。
5. 已完成：服务端合入后记录来源 commit，并同步 `project/vendor/myserver/proto/`、`myserver-protocol`、客户端状态/页面/测试。

## 阶段 2：客户端协议与状态适配

- 开始时间：2026-08-25 10:26:28 +08:00
- 结束时间：2026-08-25 11:03:40 +08:00
- 开发总结：已同步 MyServer `13b1d9e0` 的 1445/1446 proto 快照，接入 character/connection scoped history 状态、受限 keyset cursor 分页和请求去重；响应严格校验 7 种服务端公开状态、记录/奖励数量与字段长度，cursor 不进入 Debug 或 pending 元数据。2026-08-26 按 MyServer `ae7c6f3a` 协议拆分提交重新同步 vendor 文件，消息号和字段号保持兼容。
- 验证记录：主审发现并修复了 failure status 与服务端 `activity_claim_record` 枚举不一致的问题；`cargo fmt`、`cargo check --lib`、`cargo test activity --lib`（47 passed）、`cargo test -p myserver-protocol activity --lib`（3 passed）及 `git diff --check` 通过。

- [x] 扩展 `MyServerCommand/Event`、`PendingRequest` 和 `ActivityClientState` 的历史记录查询状态。（审核：`types.rs`、`plugin.rs`、`activity.rs` 接入 1445/1446 和 typed `ActivityClaimHistory` 状态，protocol round-trip 3 项通过。）
- [x] 支持分页 cursor/page size、加载更多、请求去重和连接代际隔离。（审核：history 单请求去重、20/50 page size 限制、replace/append 及 scope change 清空由 47 项 activity 定向测试覆盖。）
- [x] 校验响应 character 作用域、活动版本和记录字段大小，不把敏感身份写入日志。（审核：tracker 绑定 connection/character scope，响应严格校验 7 种服务端 status、50 条记录、16 项奖励和文本/cursor 边界；command Debug 不输出 cursor。）
- [x] 增加协议 round-trip、权限拒绝、空结果和游标错误测试。（审核：`activity_claim_history_proto_round_trip`、authenticated connection、empty/page、cursor error、payload mismatch 与 scope isolation 回归通过。）

## 阶段 3：历史记录页面

- 开始时间：2026-08-25 11:04:35 +08:00
- 结束时间：2026-08-25 12:21:24 +08:00
- 开发总结：新增固定 approved document 的历史领取记录页，列表入口只打开 history route，关闭后保留活动列表状态。页面消费当前 character scoped 的服务端记录，覆盖 loading、empty、error、refresh/retry、load-more 和重复动作抑制。为避免多页累计越过 declarative repeat 预算，客户端缓存与 UI binding 统一限制为 60 条；达到限制时清除内部 cursor、停止分页并保留禁用的加载更多控件。
- 验证记录：主审确认 60 条限制低于 `UI_REPEAT_MAX_ITEMS=64`，当前 8 节点行模板展开为 480，低于 `UI_REPEAT_MAX_EXPANDED_NODES=512`；`cargo fmt --check`、`cargo check --lib`、`cargo test activity --lib`（60 passed）、`cargo test main_world --lib`（233 passed）和 `git diff --check` 通过，只有仓库既有 unused warning。

- [x] 在 `project/src/game/screens/activity/claim_history.rs` 或对应 approved document 中实现记录页。（审核：固定 host、owner、route、binding/action schema 以及 approved document/promotion 已接入；promotion registration 定向测试通过。）
- [x] 从活动列表点击“我的领取记录”后打开记录页，关闭后返回列表并保留列表状态。（审核：list action 只发 `OpenClaimHistory`，`closing_claim_history_returns_to_the_existing_list_without_reloading_it` 通过。）
- [x] 展示活动、版本、阶段/动作、领取时间、结果状态和服务端奖励摘要。（审核：`claim_history_item` 只绑定七个公开字段；`history_item_exposes_only_public_record_summary_fields` 与非 granted 奖励隐藏测试通过。）
- [x] 覆盖加载、空状态、分页、加载更多、失败、重试和重复点击禁用。（审核：first/load-more/empty/retry 以及 duplicate command 定向测试通过；多页缓存截断、满容量不发命令、60 项 activity 测试和 233 项 main-world 测试通过。）

## 阶段 4：记录页联调与验收

- 开始时间：2026-08-25 14:46:34 +08:00
- 结束时间：2026-08-25 17:05:28 +08:00
- 开发总结：首次通过活动按钮加载列表时被 `UI_TEXT_I18N_KEY_MISSING` 阻塞，已由 `e80c806` 补齐中英文 key；第二次加载被 `UI_DECLARATIVE_SCREEN_BINDING_CONTRACT_MISMATCH` 阻塞，已修正 host binding scope 并增加五个活动页面的 runtime contract 回归。随后完成真实角色历史记录页面联调，抽奖奖励摘要可正常显示。
- 验证记录：`cargo fmt --check`、`cargo check --lib`、`cargo test activity_hosts_match_the_runtime_binding_contract --lib`（1 passed）、`cargo test activity --lib`（62 passed）和 `git diff --check` 通过。真实角色历史记录查询显示抽奖记录 `granted`、奖励 `6001 x1`，与服务端 `activity_claim_record` 及角色背包数据一致。2026-08-25，用户确认阶段 4 的记录页联调结果；本地结果仅作记录，暂不计入最终验收，待上线真机复验。

- [ ] 验证当前角色的历史领取记录页可展示已成功发放的抽奖奖励摘要。（记录：角色 `chr_2c2fq8tg0080` 的历史记录显示 `granted` 与 `6001 x1`，服务端记录和背包均已确认；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 验证不同角色之间记录不串线，切换角色/登出后清空旧记录。（记录：2026-08-25 用户确认阶段 4 联调结果；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 验证超时、断线和重连不会本地新增或重复一条历史记录。（记录：2026-08-25 用户确认阶段 4 联调结果；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 验证目标窗口和 Android 安全区下记录列表、分页按钮和错误文案可读。（记录：2026-08-25 用户确认阶段 4 联调结果；本地结果暂不计入最终验收，待上线真机复验。）

## 最终完成定义

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 活动列表的“我的领取记录”按钮可打开当前角色服务端记录页。（记录：真实联调页面已打开当前角色记录，并显示抽奖 `granted` 奖励摘要；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 记录页具备分页、空状态、失败和重试能力。
- [ ] 历史记录协议、权限边界、客户端状态和 UI 测试均有可复核证据。

> 服务端阶段完成，最终完成定义暂不勾选：客户端 vendor 协议同步、状态适配、历史页面和真实联调由客户端后续开发继续完成。
