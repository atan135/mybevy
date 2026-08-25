# 活动客户端基础能力 Checklist

## 目标

建立活动客户端的协议、MyServer 消息接口、共享状态、请求追踪、错误分类和安全边界，为活动列表、历史记录和具体玩法提供稳定基础。

## 基础原则

- [x] 所有活动请求使用已鉴权的 character-bound game 连接和现有 `PendingRequest` 生命周期。（审核：activity sender 拒绝未鉴权连接，复用 `send_request_with_seq`/pending；`activity_request_requires_an_authenticated_character_bound_connection` 通过。）
- [x] 不在客户端复制服务端资格、奖励、概率、消耗或随机规则。（审核：adapter 只消费 server typed view，claim/draw payload 不含上述服务端字段；登录奖励/抽奖 action 测试通过。）
- [x] 旧连接、旧角色和旧活动版本的响应不能覆盖当前状态。（审核：connection/activity key/scope 校验与 old detail failure 隔离，40 项 activity 回归通过。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义 1435-1444 活动协议、状态 adapter、错误和身份安全边界。
- 文档名称：MyServer 协议设计
  - 路径或链接：`../MyServer/docs/协议与客户端/协议设计.md`
  - 用途说明：核对协议来源、消息编号和编解码约束。
- 文档名称：MyServer 外部客户端接入说明
  - 路径或链接：`../MyServer/docs/协议与客户端/外部客户端接入说明.md`
  - 用途说明：核对 character-bound ticket 和客户端接入边界。

## 阶段 1：协议快照

- 开始时间：2026-08-24 17:52:00 +08:00
- 结束时间：2026-08-24 18:48:22 +08:00
- 开发总结：同步活动协议快照，补齐 1435-1444 packet 类型、MyServer 命令/事件和活动共享状态。
- 验证记录：`cargo test -p myserver-protocol` 通过 4 项；`cargo test activity --lib` 通过 8 项；`cargo test error_response_from_the_wrong_connection_keeps_the_current_pending_request --lib` 通过 1 项。

- [x] 同步并记录活动 `game.proto` 来源 commit 到 `project/vendor/myserver/README.md`。（验证：`project/vendor/myserver/README.md` 记录 `f61fbc98f02f1cfac48a1ba11ebdd73e324ad1ae` 和 proto hash `297a8e226a1b4414545b7a39770d96581f10b076`。）
- [x] 生成并验证 `ActivitySummary`、列表、详情、进度、领取和动作 protobuf 类型。（验证：`project/vendor/myserver/proto/game.proto:933` 至 `:1023` 定义活动 messages，`cargo check --lib` 通过。）
- [x] 在 `project/crates/myserver-protocol/src/lib.rs` 增加 1435-1444 的 `MessageType`。（验证：`MessageType::ActivityListReq` 至 `ActivityActionRes` 映射 1435-1444，协议 crate 测试通过。）
- [x] 补充消息号 round-trip、编解码和未知字段兼容测试。（验证：`activity_message_types_round_trip`、`activity_proto_round_trip_ignores_unknown_fields` 通过。）

## 阶段 2：MyServer 命令与事件

- 开始时间：2026-08-24 17:52:00 +08:00
- 结束时间：2026-08-24 18:48:22 +08:00
- 开发总结：接入五类活动请求和响应，保留现有 seq、pending 与连接校验边界。
- 验证记录：活动定向测试覆盖未认证拒绝、旧连接 ErrorRes 隔离和 stage ID 精确比对。

- [x] 扩展 `MyServerCommand`：活动列表、详情、进度、阶段领取和抽奖动作。（验证：`project/src/game/myserver/types.rs` 定义 `LoadActivity*`、`ClaimActivityStage` 与 `RunActivityAction`。）
- [x] 扩展 `MyServerEvent`：成功、失败、处理中、重复、版本冲突和恢复状态。（验证：活动 response/rejected/protocol event 由 `activity.rs` 转为 `ActivityEvent`，保留服务端 `processing`、`duplicate` 和错误码。）
- [x] 复用 `send_request_with_seq`、`PendingRequest`、seq 和连接代际校验。（验证：`send_activity_request` 复用 seq/pending，拒绝无 authenticated character 的连接；旧连接 `ErrorRes` 定向测试通过。）
- [x] 解码 1436、1438、1440、1442、1444，并校验活动 ID 和版本。（验证：`plugin.rs` 分发五种 activity response，`activity.rs` 对 detail/progress/claim/action 校验活动 ID、版本及写入上下文。）

## 阶段 3：共享状态与安全边界

- 开始时间：2026-08-24 17:52:00 +08:00
- 结束时间：2026-08-24 18:48:22 +08:00
- 开发总结：建立按 character/连接 scope 隔离的活动状态、progress adapter、request tracker 和脱敏错误模型。
- 验证记录：活动测试覆盖 scope 切换拒绝旧响应、重试键仅存 fingerprint、`{}` progress fallback、未知 JSON 字段和 stage ID 校验。

- [x] 新增 `project/src/game/myserver/activity.rs` 的 typed state、adapter 和 request tracker。（验证：`ActivityClientState`、`ActivityRequestTracker`、`ActivityPlugin` 已注册到 `MyServerPlugin`。）
- [x] 对 `progress_json` 实施大小限制、`type`/`schema_version` 校验和未知字段兼容。（验证：`MAX_ACTIVITY_PROGRESS_JSON_BYTES=64KiB` 和 `parse_progress_json`；未知字段定向测试通过。）
- [x] 按 character、活动 ID、版本和连接代际隔离活动状态。（验证：state scope 使用 character/connection，details 以 `ActivityKey(activity_id, version)` 索引；旧 character scope 响应测试通过。）
- [x] 增加活动错误分类、本地化 key 和脱敏诊断，不记录 ticket、密码、完整 request ID 或奖励内容。（验证：`ActivityErrorKind::message_key` 和 retry policy；request tracker/pending 仅保留 request ID fingerprint，脱敏测试通过。）

## 阶段 4：基础能力验证

- 开始时间：2026-08-24 18:37:27 +08:00
- 结束时间：2026-08-24 18:48:22 +08:00
- 开发总结：完成协议、状态、连接隔离和错误分类定向验证；历史记录协议保持显式服务端依赖。
- 验证记录：`cargo fmt --check`、`cargo check --lib`、`cargo check --tests`、`git diff --check` 均通过；活动测试 8 项、旧连接 ErrorRes 测试 1 项、协议测试 4 项均通过。

- [x] 运行协议、状态、连接代际和错误分类定向测试。（验证：协议 4 项、`cargo test activity --lib` 8 项和旧连接 ErrorRes 1 项均通过。）
- [x] 运行 `cargo fmt --check` 和 `cargo check`。（验证：`cargo fmt --check`、`cargo check --lib`、`cargo check --tests` 均通过；仅有仓库既有 unused/dead-code warnings。）
- [x] 记录历史领取记录协议的服务端依赖；未分配 msgType 前不得伪造客户端协议实现。（验证：vendor README 明确历史记录 msgType 未分配，活动 proto 仅同步 1435-1444。）

## 最终完成定义

- 开始时间：2026-08-24 17:52:00 +08:00
- 结束时间：2026-08-24 18:48:22 +08:00
- 验收总结：01 基础能力完成，可供 02、04、05 使用；03 仍等待服务端发布历史领取记录协议和 msgType。

- [x] 活动公共协议和运行时接口可被 02-05 子清单使用。（验证：`MyServerCommand/Event`、`ActivityClientState` 和 1435-1444 codec 已公开给 game crate。）
- [x] 共享状态在切换角色、登出、切换环境、断线和连接代际变化时正确清理或隔离。（验证：`sync_activity_scope` 变更 character/connection/auth scope 时清空缓存和 tracker；旧 scope/connection 测试通过。）
- [x] 安全边界、错误行为和测试证据可复核。（验证：认证前拒绝、旧连接 ErrorRes、JSON/未知字段、request ID fingerprint 和错误分类均有定向测试。）
