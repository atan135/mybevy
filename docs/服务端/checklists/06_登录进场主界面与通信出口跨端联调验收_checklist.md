# 登录进场主界面与通信出口联调验收 清单

## 目标

在认证、主世界进场、主世界 HUD、聊天应用接线和邮箱 UI 五份功能清单完成后，对 mybevy 客户端、MyServer 服务端、Caddy 公网入口和桌面执行端到端联调与最终验收，证明完整用户路径、异常恢复、安全边界和灰度回滚能够闭环。

本清单不承载前置功能的核心实现；发现缺陷时回到对应功能清单修复并重新执行受影响验收项。

Android 真机不再属于本清单执行范围。待 02-06 功能与公网准入完成、目标版本部署到公网后，再启动 `07_公网部署后Android真机验收_checklist.md`。

## 前置清单

- `01_注册与登录入口适配_checklist.md`
- `02_主世界进场与权威闭环_checklist.md`
- `03_主世界界面与场景内面板_checklist.md`
- `04_聊天应用级接线_checklist.md`
- `05_邮箱界面与通知闭环_checklist.md`

## 基础原则

- [x] 联调以 character-bound ticket 和账号/角色身份分离为不可回退约束。（验证：阶段 1-8 的登录、game/chat/mail、跨账号 ownership 和公网越权矩阵均以当前 `character_id` 为玩法主体，无账号 `player_id` fallback。）
- [x] 生产客户端只访问 auth/Caddy 公网入口和 game proxy 公网端口，不直连内部服务或 registry。（验证：生产只公开 Caddy 80/443 TCP、SSH 和 game proxy 4000/UDP；客户端 descriptor 均为公网 host，内部 listener 与 registry 外部不可达。）
- [x] 真实服务、集成测试、部署检查和外部端口操作执行前确认依赖、范围和影响。（验证：阶段记录包含本地/生产范围、临时账号、Compose override、迁移、镜像、Caddy 和恢复判据，最终临时 override 为 0。）
- [x] 验收失败必须记录复现条件、关联 ID、稳定错误码和归属清单，不用临时绕过代替修复。（验证：KCP DNS、Room Ready、Unix socket 和 mail 持久化缺陷均回到对应仓库修复并回归；领取降级保留 `MAIL_CLAIM_ROUTE_UNAVAILABLE` 等稳定错误码。）
- [x] 每个阶段完成后保存验证记录，并按阶段独立提交文档或必要修复。（验证：阶段 1-8 均含开始/结束、总结和验证记录；关键修复提交包括 mybevy `cc6e8b3`、`b98ff8c` 与 MyServer `02c646d`。）

## 阶段 1：版本、配置和契约准入

- 开始时间：2026-08-06 09:29:28 +08:00
- 结束时间：2026-08-06 09:53:25 +08:00
- 开发总结：补齐生产 chat descriptor 的 Caddy host/port 约束，并将 MyServer 已实现的 room policy 错误码登记到协议静态目录；客户端与 MyServer 协议检查全部通过。
- 验证记录：`MYSERVER_CLIENT_ROOT=H:\project\mybevy npm run check:proto` 6/6 通过；`cargo fmt --check`、两个 MyServer chat 定向测试和两个仓库的 `git diff --check` 通过。

- [x] 确认 mybevy 与 MyServer 使用兼容 protobuf、HTTP 字段、错误码和应用协议版本。（验证：MyServer `npm run check:proto` 6/6 通过，含 `project/src/game/myserver/protocol.rs` 与 `project/build.rs` optional mybevy 扫描；static error catalog 与实现 literal 均为 156。）
- [x] 运行 MyServer `npm run check:proto`，确认可选 mybevy 协议扫描通过。（验证：以 `MYSERVER_CLIENT_ROOT=H:\project\mybevy` 重跑，routing diagnostics=0、fixtures 6 passed、client protocol version policy passed。）
- [x] 核对 Local/Production auth base URL、game KCP/TCP、chat WSS 和 mail HTTPS 配置来源。（验证：`MyServerConfig` 的 Local/Production 环境与 `MYSERVER_*`/`MYSERVER_REMOTE_*` 配置生效；生产 chat 固定为 auth descriptor `wss://chat.game.zergzerg.cn:443/`，mail 固定 Caddy `api.game.zergzerg.cn:443`，均无内部回退。）
- [x] 核对主世界固定映射：`grassland_01 / scene_id=1 / spawn_id=1001 / movement_demo / main-world-public`。（验证：客户端 scenes.csv/scene.ron 与 MyServer SceneTable.csv、SceneSpawnPoint.csv、room policy/factory 静态映射一致。）
- [x] 核对客户端 approved UI documents、首包场景 manifest、资源路径和 Android 包内资源完整性。（验证：main world HUD/mail/settings approved document 与 host registration 存在；`world.main` manifest/layout 位于首包 assets；Android sourceSets 打包整个 `project/assets`、ABI 为 arm64-v8a；APK 构建留阶段 7。）
- [x] 确认测试日志和报告目录不会保存 access token、ticket、密码、邮件正文、附件或聊天正文。（验证：客户端 diagnostic snapshot 仅保留 fingerprint、generation、稳定状态和关联 ID；mail/chat/auth 定向脱敏审计未发现敏感正文或凭据；真实服务日志留后续阶段复核。）

## 阶段 2：本地完整主路径联调

- 开始时间：2026-08-06 10:31:57 +08:00
- 结束时间：2026-08-06 14:53:58 +08:00
- 开发总结：完成本地 auth/game/chat/mail 全链路、character-bound ticket、主世界进退、邮件权威查询/领取幂等和场景内设置/邮箱 UI 验收；修复生产 chat descriptor、Local mail descriptor、mail grant 临时密钥、Settings allowlist 与邮箱浮层层级/输入问题。
- 验证记录：真实 Local mybevy 主世界自动生命周期日志、服务端 HTTP/mail/game 日志和人工桌面截图均通过；`npm run check:proto`、auth descriptor 测试、主世界与 UI document 定向 Rust 测试、`cargo fmt --check`、`git diff --check` 通过。

- [x] 在确认依赖后启动本地 auth-http、game proxy、game server、Redis、NATS、PostgreSQL、chat 和 mail 所需服务。（验证：`dev-stack.ps1 -NoRedis -NoAdminApi -NoAdminWeb -NoMetricsCollector -WithChat -WithMail` 启动/复用本地栈；NATS/auth/proxy/chat/game/mail 的 managed status 为 running，Redis/PostgreSQL 为既有本地实例，日志位于 `H:\project\MyServer\logs\dev-stack\`。）
- [x] 完成密码注册直接登录分支，并完成账号登录、角色创建/选择和 character-bound ticket 获取。（验证：唯一临时本地身份 register/login/character-create 均为 HTTP 201，后续 proxy game auth、入房和 character-bound ticket 流程通过；凭据未写入记录。）
- [x] 从 Lobby 加入 `main-world-public / movement_demo`，确认权威场景映射、初始快照、Scene ready 和 Room ready。（验证：真实 Local mybevy Debug client 完成 proxy auth、Join、权威 snapshot、`SceneEvent::Ready`、RoomReady ack 与 MainWorld Active；脱敏日志位于临时 `mybevy-stage2-*` 目录，测试 client 已停止。）
- [x] 在主世界反复打开关闭设置和邮箱，确认房间、scene session 和 gameplay 输入状态不被破坏。（验证：人工桌面复验 Settings 浮层可用、Mail 显式 Local descriptor 进入空列表；修复后 Mail 根层 ZIndex=80 覆盖 HUD、无脱离 unread `0`、全屏浮层阻断下层 gameplay 输入，截图确认无重叠。）
- [x] 接收一封测试邮件通知、刷新权威列表、查看详情、标记已读并完成附件领取。（验证：`MAIL_NOTIFY_PUSH` type 20301、list/detail/read HTTP 200、first claim HTTP 200 后主动详情收敛为 claimed、repeat claim `already_claimed=true`；mail/game 脱敏日志确认 registry 投递与 inventory grant。）
- [x] 从主世界退出回 Lobby，再次进入主世界，确认账号/角色会话保留且旧场景/HUD 无残留。（验证：真实 Debug client 自动路径记录 LeaveRoom、SceneExited、Lobby 完成；人工桌面复验确认回 Lobby 后再次入场无旧 HUD/scene 残留。）

## 阶段 3：注册审核和身份隔离联调

- 开始时间：2026-08-06 14:55:54 +08:00
- 结束时间：2026-08-06 15:05:00 +08:00
- 开发总结：补齐 ACCOUNT_LOCKED 客户端阻断映射、auth 账号锁定与 IP block guard 回归；以 character-bound ticket、跨账号邮件 ownership 和 generation 测试验证身份隔离。
- 验证记录：auth-http `npm test` 99/99、client protocol version check、MyBevy 错误映射/会话切换定向测试、game-server 同账号多角色隔离测试与两仓库 diff 检查通过。

- [x] 验证 `pendingReview=true` 注册不进入选角、Lobby 或 game proxy。（验证：auth-store register 测试断言 pendingReview 时 session=null/status=pending_review；客户端 pending notice binding 定向测试通过。）
- [x] 验证重复账号、错误密码、账号锁定、封禁、维护、IP 阻断和版本不兼容的页面状态。（验证：auth-http 99/99 覆盖重复/凭据/ACCOUNT_LOCKED/IP_BLOCKED；客户端将 ACCOUNT_LOCKED 映射为阻断页面；维护/version guard 与页面 binding 定向验证通过，未切换真实运维开关。）
- [x] 使用同账号不同角色验证主世界玩法主体、移动状态和附件领取均使用当前 `character_id`。（验证：character select/ticket 定向测试和 game-server 同账号 character_id 的资产、buff、room member/index 隔离用例通过。）
- [x] 使用不同账号验证邮件 ownership、聊天账号身份和房间角色身份互不越权。（验证：mail two-account ownership acceptance 对 detail/read/claim 返回 MAIL_NOT_FOUND；chat/game-proxy ticket owner 定向测试通过。）
- [x] 切角色、切账号、Logout 和切环境后，旧 endpoint、ticket、邮件数据、chat connection 和 room session 全部失效。（验证：六条 auth/MyServer 切换定向回归与 mail/chat runtime cleanup 测试通过。）
- [x] 验证迟到 HTTP、WSS 和 game push 不覆盖新 identity/generation。（验证：late register、old mail response、old chat runtime event 和 session reset generation 定向测试通过。）

## 阶段 4：场景与房间异常恢复

- 开始时间：2026-08-06 15:15:40 +08:00
- 结束时间：2026-08-06 15:51:17 +08:00
- 开发总结：补齐 RoomReady 拒绝/超时的确定失败状态，并以 client/server fault fixture 和隔离 live proxy 演练验证场景、房间与断线恢复。
- 验证记录：main_world_entry 32/32、scene lifecycle 7/7、Lobby/kick/domain error 定向测试、cargo check/fmt/diff check 通过；隔离 client 在 proxy 中断后由 RoomReconnect 与 recovery snapshot 回到 Active。

- [x] 快速重复点击进入游戏、家园和返回，不产生重复 room member、scene root、HUD 或 Loading。（验证：Lobby/MainWorldEntry 的去重与 home/return intent 定向回归通过。）
- [x] 验证房间满员、policy mismatch、未知 scene ID、join timeout 和 ready timeout 均回到确定位置。（验证：room error 映射、未知权威 scene、RoomReady response/request timeout 定向回归通过；ReadyTimedOut/JoinRejected 均退出 in-flight。）
- [x] 验证 required asset、manifest、相机和出生点失败时清理 pending scene 并恢复可操作页面。（验证：四类 SceneFailure 均断言 Failed/SceneLoadFailed、无 in-flight session。）
- [x] 主世界 active 时中断 game proxy，验证冻结输入、补票、重新鉴权、RoomReconnect 和快照恢复。（验证：隔离 Local client Active 后停止/恢复 managed proxy，日志依次记录 recovery requested、RoomReconnect accepted、recovery snapshot accepted、recovered Active；proxy 端口恢复健康。）
- [x] 房间成员状态过期时验证重新加入固定公共房间，不复用无效 session。（验证：membership expiry/reconnect fixed public room 定向回归通过。）
- [x] 验证 session kick、封禁、维护和不可恢复 auth 失败会清理主世界并回登录。（验证：kick 分级、fatal account event、auth/domain failure 定向回归通过；真实控制面写入未执行。）

## 阶段 5：聊天与邮件故障矩阵

- 开始时间：2026-08-06 16:20:26 +08:00
- 结束时间：2026-08-06 16:47:20 +08:00
- 开发总结：完成 chat/mail descriptor、故障、reconciliation 与幂等矩阵；以隔离 Local client 对 chat/mail 服务中断验证核心游戏链路独立性。
- 验证记录：client chat 21/21、mail 37/37、plugin 54/54、host 46/46、chat-server 95/95、mail-service 141/141通过；Local chat 下线/恢复和 mail worker 暂停/恢复后服务健康。

- [x] chat descriptor 为 null、WSS DNS/TLS 失败、鉴权拒绝和限流时，登录、主世界和邮箱 HTTPS 仍可使用。（验证：chat runtime fixture 覆盖 Unavailable/TLS/auth/rate states；Local chat 下线 8 秒时隔离 client 主世界持续 Active，服务恢复后监听正常。）
- [x] mail descriptor 为 null、HTTPS 超时、429 和 503 时，主世界和 chat/game 链路不受阻断。（验证：mail client/server fixture 覆盖 descriptor null、timeout、429/503；Local mail worker 暂停后 health 明确失败、恢复 HTTP 200，game/chat 保持可达。）
- [x] chat 重连期间 ticket 刷新只发生一次，旧 runtime 事件不污染新连接。（验证：chat runtime 21 项定向测试断言 RefreshTicket 去重和旧 generation event 丢弃；live client 未建立 chat socket，已记录限制。）
- [x] MailNotifyPush 重复、乱序和丢失后，邮箱主动查询仍收敛到 PostgreSQL 权威列表。（验证：mail fixture 断言同 generation push 合并为一次权威 HTTPS refresh、stale generation ignored。）
- [x] 附件领取在 202、断网、服务端重启、入口暂停和 manual review 下保持幂等并准确展示状态。（验证：mail-service 141 项覆盖 202/restart/pause/manual review；client 断网后进入 reconciliation 且不重复 POST。）
- [x] 关闭邮箱、切场景或应用进入后台期间的 reconciliation 最终可恢复或给出明确人工处理状态。（验证：mail UI close/fallback 保留 reconciliation、后台取消 pre-auth queue、manual review 1/2/4 秒有界重查定向测试通过。）

## 阶段 6：公网入口与安全验收

- 开始时间：2026-08-07 10:34:47 +08:00
- 结束时间：2026-08-07 11:09:44 +08:00
- 开发总结：完成 MyServer `d95f6b666e4e` 正式 release `v0.1.0-d95f6b666e4e` 的数据库迁移、镜像摘要锁、readiness、Caddy 切换和公网安全矩阵；验收中发现 Bevy KCP 仅接受数值 IP，已在共享网络框架补充异步 DNS 解析与回归测试，并使用正式 descriptor 完成真实 `4000/UDP` 鉴权和连续 ping。临时游客角色均已软删除并 logout，完整凭据未落盘或输出。
- 验证记录：`npm run check:proto` 6/6、KCP 定向测试 4/4、`cargo fmt --check`、`cargo check` 和调试客户端构建通过；正式 auth 返回 game `game.zergzerg.cn:4000/kcp`、chat `chat.game.zergzerg.cn:443/wss`、mail `api.game.zergzerg.cn:443/https`，严格 TLS 下 HTTPS/WSS 通过且 WSS SAN/有效期正确。公网矩阵验证 valid、无票、URL/Bearer 误传、篡改、过期、撤销、version、ownership 与跨账号角色越权；服务器只有 `80/443/TCP`、运维 SSH 和 `4000/UDP` listener，内部服务无宿主映射，Caddy internal 404、admin API 无凭据 401。客户端及 Caddy/auth/game-proxy/game-server/chat/mail 自阶段开始后的日志对完整 ticket、凭据值、密码值和正文/附件字段均为 0 命中；API health 200，容器稳定且无验收期新增重启。

- [x] 正式 auth 响应返回公网 `services.chat` WSS 和 `services.mail` HTTPS descriptor，不返回内部 host/port。（验证：两组隔离账号和严格 TLS 临时账号均只返回 `game.zergzerg.cn:4000/kcp`、`chat.game.zergzerg.cn:443/wss`、`api.game.zergzerg.cn:443/https`，`announce` 为 null。）
- [x] 公网客户端只访问 Caddy `443/TCP`、必要的 `80/TCP` 跳转和游戏 `4000/UDP` 或批准的 TCP fallback。（验证：宿主映射仅 Caddy 80/443 与 game-proxy 4000/UDP；修复 KCP DNS 后 Bevy 客户端通过正式 descriptor 完成连接、character 鉴权并连续收到 3 次 ping。）
- [x] 验证客户端不能访问 Redis、NATS、PostgreSQL、registry、admin token 或 chat/mail 内部 listener。（验证：Redis/NATS/PostgreSQL 和所有应用内部端口均无宿主映射或 listener，外部协议探测无响应；`/api/v1/internal/*` 返回 404，admin API 无凭据返回 401。）
- [x] 验证 chat ticket 不出现在 URL/query，mail 只通过 `X-Game-Ticket` 使用当前 character-bound ticket。（验证：WSS 使用 credential-free 根 URL 和首个二进制 `ChatAuthReq`；mail 有效 header 返回 200，query-only 和 Bearer-only 均返回 401，`player_id` override 返回 400。）
- [x] 验证无 ticket、过期、撤销、ownership/version 不匹配和越权账号/角色请求全部被拒绝。（验证：mail 无票 401，chat 空票 `INVALID_TICKET_FORMAT`；正确签名过期票、篡改票、撤销票和 logout 后旧 version 票在 mail/chat 均拒绝；跨账号签发和撤销分别返回 `CHARACTER_OWNER_MISMATCH` 与 `TICKET_OWNER_MISMATCH`。）
- [x] 检查客户端、Caddy 和服务端日志，确认不记录完整 ticket、密码、邮件正文、附件或聊天正文。（验证：客户端日志只含 `ticket_fp`；Caddy、auth、game-proxy、game-server、chat、mail 阶段时间窗日志对完整 ticket 形态、凭据值、密码值和正文/附件字段均为 0 命中。）

## 阶段 7：桌面产品与 Android 包体准入验收

- 开始时间：2026-08-07 11:29:09 +08:00
- 结束时间：2026-08-07 13:42:11 +08:00
- 开发总结：完成桌面产品、Android 包体准入和正式服主世界闭环验收；修复 Bevy KCP 公网域名解析，以及生产 game-server/game-proxy/match-service 的共享 Unix socket 路径、空卷权限和滚动发布残留 socket 清理。最终 MyServer release `v0.1.0-fef5c9b49d36` 已上线并稳定，桌面客户端完成登录、选角、KCP 鉴权、公共房间、场景 Ready、主世界可视化、5 秒性能采样、退出清理和截图。10 个阶段临时游客角色均已删除，并在严格 TLS 下确认角色列表为空后 logout。
- 验证记录：客户端 `cargo fmt -- --check`、`cargo check`、`cargo test --lib`（1810 passed、0 failed、1 ignored）、UI boundary 和最终 Debug build 通过；标准三档、自定义 1600x720、邮件面板及真实主世界截图通过。Android arm64 Rust 动态库和 Debug/Release APK 构建通过，离线核验 ABI、Activity、签名状态、31 份 approved documents、254 项首包资源和场景 manifest 完整。真实正式服自动路径记录 `main world entry active`，5.0096 秒内 59.89 application updates/s、49 个 ECS 实体、9 个场景 session 实体，进程峰值工作集 1,095,639,040 bytes；随后 LeaveRoom 响应成功、`SceneEvent::Exited` 和 Lobby exit completed。场景反复进退无残留、相机随 session 清理、KCP 出站超时有界三项定向测试通过，验收窗口无 SendFailed 或未响应 gameplay 请求。MyServer 部署测试 13/13 通过，最终 release 的 init exit=0、两枚 socket 为 UID/GID 10001、proxy 持续发现 1 个 `proxy-local` endpoint，13 个常规容器稳定且近 10 分钟无 socket/权限错误。已记录运维风险：game-server 未在 60 秒 stop grace 内退出，本次由 worker lease TTL 与 readiness 自动恢复，后续应单独修正优雅停服。

- [x] 运行 `cargo fmt`、`cargo check` 和五份前置清单要求的客户端测试。
- [x] 运行 UI boundary 检查，确认新增正式页面均为 approved document + fixed host。
- [x] 使用 phone-landscape、phone-1080p-landscape、tablet-landscape 和自定义窗口尺寸完成视觉验收。
- [x] 构建 Android Rust 动态库和 Debug/Release 候选 APK，离线检查 ABI、启动 Activity、权限、approved documents、场景 manifest 和首包资源完整性；不在本阶段安装真机。
- [x] 验证主世界平地、球体、相机和光照在桌面可见，Loading/HUD/面板无重叠。（验证：正式服 `1600x720 / device scale 2` 截图显示平地、受定向光照的球体和正确相机视角；HUD 顶栏与场景、Settings/Mail/Home/Lobby 控件无重叠，截图位于 `summary/debug/phase7-desktop-final/run-20260807-1334/`。）
- [x] 记录主世界实体数、内存峰值、帧率、网络队列和切场景清理结果，确认无持续增长。（验证：5.0096 秒采样为 59.89 application updates/s、49 ECS/9 scene-owned entities，峰值工作集 1,095,639,040 bytes；真实自动退出收到 LeaveRoom 200 并完成 SceneExited/Lobby，gameplay 请求均有响应且无 SendFailed；反复 enter/exit 无残留、camera cleanup、KCP stalled outbound deadline 定向测试各 1/1 通过。）

## 阶段 8：灰度、暂停和回滚演练

- 开始时间：2026-08-07 13:44:19 +08:00
- 结束时间：2026-08-07 14:10:14 +08:00
- 开发总结：完成生产 chat descriptor 置空、mail 新领取暂停及逐项恢复演练；演练中发现生产 mail-service 未启用 PostgreSQL，先补齐 `DB_ENABLED=true` 并验证跨重建持久化，同时在 MyServer 增加生产 fail-closed 校验和回归测试；完成 UI 远端更新失败、协议回滚和 Caddy 静态/运行配置验收。
- 验证记录：chat=null 时登录仍可取 mail/game descriptor、完成建角/选角和 ticket 签发，恢复后 descriptor 回到 `chat.game.zergzerg.cn:443/wss`；mail intake=false 时健康页为 `false/true`、列表 200、首次领取 503 `MAIL_CLAIM_INTAKE_DISABLED`，恢复后测试邮件跨容器重建保留，离线 route 进入可解释的 `MAIL_CLAIM_ROUTE_UNAVAILABLE`，角色上线后 recovery worker 自动收敛为 `claimed`；UI remote/update 定向测试 13/13、mail config 30/30、auth-http 完整测试、协议策略检查及 game-server/game-proxy 版本策略测试通过；最终 13 个生产容器稳定，Caddy validate 通过，公开健康页 200，game proxy `proxy-local endpoint_count=1` 且近 10 分钟目标错误为 0。
- 灰度记录：chat 开关使用一次性 Compose override 将 `AUTH_PUBLIC_CHAT_HOST` 置空，只重建 `auth-http`；观察登录 `services.chat`、auth 健康及 mail/game 主链路；回滚为使用正式 Compose 单文件重建 `auth-http`，恢复判据为公开 WSS descriptor、健康状态和核心 descriptor 全部恢复。
- 灰度记录：mail 开关使用一次性 Compose override 设置 `MAIL_CLAIM_NEW_REQUESTS_ENABLED=false`、保持 recovery=true，只重建 `mail-service`；观察健康页、列表、领取错误码、持久工作流和 game grant；回滚为使用正式 Compose 单文件重建 `mail-service`，恢复判据为健康页 `true/true`、邮件跨重建存在、workflow claimed 且错误码清空。
- 灰度记录：UI 更新不改生产资源，使用确定性 mock 覆盖超时/304、坏签名、激活失败、损坏 active generation；恢复判据为保留已验证 generation、回滚 previous generation 或保持首包可用。Caddy 未做在线内容改写，回滚边界由静态路由测试、容器内 `caddy validate` 和公网健康检查共同验证。

- [x] 将 chat descriptor 置 null，确认客户端显示受控不可用且核心主链路继续运行。（验证：生产登录响应 `services.chat=null`；客户端 absent descriptor 测试确认不回退内部地址；mail/game descriptor、角色列表、建角、选角与 ticket 签发正常；恢复后 WSS descriptor 正常。）
- [x] 将 mail descriptor 置 null 或暂停领取入口，确认列表/领取按服务端能力边界降级。（验证：生产 intake=false/recovery=true；邮件列表 200 且测试邮件可见，首次领取返回 503 `MAIL_CLAIM_INTAKE_DISABLED`；恢复后持久工作流自动领取完成。）
- [x] 模拟 UI 远端更新不可用、签名失败和缓存损坏，确认保留 current generation 或首包 fallback。（验证：remote tests 8/8 覆盖有限重试、坏签名保留 active generation、激活失败保留 previous generation；update tests 5/5 覆盖损坏 active 回滚和无有效 generation 时不激活。）
- [x] 回滚客户端版本时确认服务端 minimum protocol、descriptor 和错误码仍产生可解释结果。（验证：当前/最低/legacy implicit 均为 v1；Node 静态策略检查通过，game-server/game-proxy 各 4/4 验证 legacy/current 接受、supported older 观测桶及 `CLIENT_PROTOCOL_VERSION_TOO_OLD/NEW`。）
- [x] 回滚 chat/mail/Caddy 配置时确认内部发信、邮件恢复和 game proxy 不受破坏。（验证：内部发信 201；mail 恢复后 PostgreSQL workflow claimed；chat/mail 正式配置恢复；Caddy validate 与 10 项边缘配置测试通过；game proxy 持续发现 1 个 `proxy-local` endpoint。）
- [x] 记录每项灰度开关、观察指标、回滚步骤和恢复判据。（验证：本阶段三条灰度记录覆盖 chat、mail、UI/Caddy；生产临时 override 数量为 0，测试角色软删除、活跃角色数 0、logout 200。）

## 阶段 9：文档、归档和最终报告

- 开始时间：2026-08-07 14:37:36 +08:00
- 结束时间：2026-08-07 14:41:03 +08:00
- 开发总结：完成注册/登录、固定公共主世界、chat WSS、mail HTTPS、生产降级回滚和 UI scene-local 面板文档收口；修正生产 game endpoint、UI route 数量及已过时的 TCP fallback 口径，并将本清单归档到 MyServer 文档域。
- 验证记录：UI generation `check-boundary` 全部为 true；主世界 HUD/设置/邮箱的 7 个 approved/promotion/fallback 文件均存在；五份前置清单未完成项均为 0；`git diff --check` 通过。本阶段仅修改 Markdown，无 Rust 行为改动，不重复运行已在阶段 7 完成的 1810 项客户端测试和 Android 构建。

- [x] 更新 `docs/服务端/服务端最新登录流程客户端验收.md` 为当前注册、进场、chat 和 mail 全链路。（验证：新增公网 WSS/HTTPS、凭据、generation、reconciliation、灰度和回滚口径，生产 game descriptor 修正为 `game.zergzerg.cn:4000/kcp`。）
- [x] 更新 `docs/场景/` 的固定公共房间、主世界映射、ready、退出和重连说明。（验证：补充幂等 StartRoom、三重 ready、recovery snapshot、成员过期重加及公网 KCP/Android 验收边界。）
- [x] 更新 `docs/界面/` 的主世界 HUD、场景内设置/邮箱和 approved document 页面盘点。（验证：记录三个 document/owner、Floating 清理和 mail reconciliation 生命周期，并统一为 10 个正式业务 route、16 个 screen。）
- [x] 检查 `docs/引擎入门使用文档.md`、`CLAUDE.md` 和常用命令是否需要同步。（验证：两份文档已同步生产 descriptor、持久 mail 和 10/16 UI 门禁；Rust、Android 和工具常用命令未变化。）
- [x] 汇总客户端、服务端、公网、桌面和 Android 包体准入记录以及剩余风险；真机记录由 07 清单单独归档。（验证：阶段 6-8 汇总公网、桌面、包体、灰度与回滚证据；Android 真机继续由 07 清单负责。）
- [x] 五份前置 清单 和本 清单 完成后，按仓库约定移动到对应 `docs/<领域>/checklists/` 归档并纳入 Git 提交。（验证：01-05 归档清单均为 0 个未完成项；06 已移动到 `docs/服务端/checklists/`。）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-07 14:40:14 +08:00
- 结束时间：2026-08-07 14:41:03 +08:00
- 验收总结：06 清单定义的桌面跨端联调、公网入口、安全、包体准入、降级和回滚目标全部完成。生产 release 仍为 `v0.1.0-fef5c9b49d36`，13 个容器稳定且临时 override 为 0。剩余风险：Android 真机闭环由 07 清单继续；客户端默认仍以首包 approved UI fallback 为可用基线，真实远端 UI trust root/cache endpoint 尚未启用；生产已外置设置 `DB_ENABLED=true`，fail-closed 修复 `02c646d` 将随下一次镜像 release 固化；game-server 曾超过 60 秒 stop grace，需后续优化优雅停服但 lease/readiness 已恢复。

- [x] 用户可以完成注册或登录、选角、进入 Lobby、加入固定公共主世界、打开设置和邮箱并退出回 Lobby。（验证：阶段 2 产品路径和阶段 7 正式服桌面闭环均通过。）
- [x] 主世界使用 `grassland_01 / movement_demo / main-world-public` 权威链路，ready、移动快照、断线重连和退出闭环正确。（验证：阶段 2、4、7 覆盖三重 ready、recovery snapshot、LeaveRoom/SceneExited 与无残留。）
- [x] chat WSS 和 mail HTTPS 使用 auth 下发公网 descriptor，内部 endpoint 对客户端不可达。（验证：生产下发 WSS/HTTPS/KCP 公网 descriptor，阶段 6 外部端口与越权矩阵通过。）
- [x] 邮件通知、权威查询、已读、领取及结果未知重查形成幂等闭环。（验证：阶段 2、3、5、8 覆盖 push 后 GET、read、单次 claim、1/2/4 秒对账、持久恢复和跨账号隔离。）
- [x] 服务不可用、超时、断网、切环境、切角色、session kick 和资源失败均有确定恢复结果。（验证：阶段 3-5 和 8 的 identity generation、场景失败、网络中断、descriptor null、限流及回滚矩阵通过。）
- [x] 桌面、客户端、服务端、公网安全、Android 包体准入、灰度和回滚验收全部完成并有可追溯记录；Android 真机验收不作为本清单完成门槛。（验证：阶段 6-8 保存测试数、release、健康状态、截图/性能、APK 离线准入及恢复判据；真机范围明确转交 07。）
