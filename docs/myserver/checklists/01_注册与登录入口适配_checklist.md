# 注册与登录入口适配 Checklist

## 目标

在现有 MyServer 账号登录、游客登录、角色选择和 character-bound ticket 链路上补齐密码账号注册入口。注册页继续使用 fixed host + approved `UiDocument`，支持注册审核、输入校验、错误反馈和环境切换，不在 UI document 中承载密码等敏感业务状态。

本清单只负责注册与登录入口，不重复实现角色列表、选角签票、game proxy 鉴权、主世界进场、聊天或邮件业务。

## 依赖与边界

- 依赖 `project/src/game/myserver/` 已有 `MyServerCommand::Register`、注册响应解析和账号会话状态。
- 依赖 `project/src/game/screens/auth/host.rs` 的登录 fixed host 和 `project/assets/ui/documents/approved/auth/login.v1.json`。
- 后续主世界进场依赖本清单输出的有效账号会话、选中角色和 character-bound ticket。
- 服务端协议以相邻 `H:/project/MyServer` 仓库的 auth-http 实现和公开客户端文档为准。

## 基础原则

- [x] 账号 `player_id` 只用于登录、安全和审计，玩法主体继续使用当前 ticket 绑定的 `character_id`。（验证：现有 character-bound ticket 链路未变，注册只建立账号 session。）
- [x] 密码、确认密码和其他敏感输入只保存在受控 ECS 输入组件中，不进入普通 binding、action 参数、日志或诊断快照。（验证：auth/MyServer 审计测试、sensitive document 声明和 Debug 脱敏断言通过。）
- [x] 服务端是账号格式、密码规则、重复账号、审核状态和安全阻断的最终权威；本地校验只用于提前反馈。（验证：静态复核 MyServer auth-http DTO/service 与客户端错误映射一致。）
- [x] UI 继续遵守 fixed host、closed action/binding allowlist、approved document 和首包 fallback 边界。（验证：ui-generation check-boundary 全部 true，auth document audit 测试通过。）
- [x] 每个阶段完成后运行对应验证，并按阶段独立提交。（验证：阶段 1-5 已分别提交 3ae5443、cd5b5f7、ca8df91、02af253、786f1c5。）

## 阶段 1：注册协议和现状基线

- 开始时间：2026-08-04 10:31:32 +08:00
- 结束时间：2026-08-04 10:53:22 +08:00
- 开发总结：客户端注册契约已固化为脱敏请求校验、独立审核状态和直接登录复用链路；审核中不会形成可用账号或角色会话。
- 验证记录：worker 运行 cargo fmt、cargo test myserver --lib（102 passed）和 cargo check；主审核复跑 cargo test myserver --lib（103 passed，只有既有 dead-code warnings）。

- [x] 确认客户端底层已支持 `MyServerCommand::Register`，并能区分直接建立登录会话和 `pendingReview=true` 响应。
- [x] 确认当前登录 fixed host 只开放账号登录、游客登录和环境切换，尚未开放注册 action。
- [x] 固化首版注册请求字段、账号规范化、密码 `6..128`、确认密码、本地校验和服务端错误码映射。（审核：`project/src/game/myserver/types.rs` 定义 `validate_registration_request`、UTF-16 长度校验和四个稳定错误码映射；registration 定向单测通过。）
- [x] 明确注册审核分支：审核中不创建可用登录会话、不进入角色选择，并提供返回登录的稳定入口。（审核：`registration_pending_review` 清理 token、角色、ticket 和连接计划，`DismissRegistrationReview` 仅复位本地审核状态；端到端 pendingReview 测试通过。）
- [x] 明确注册成功策略：服务端返回账号 session 时进入角色加载流程；没有 session 时停留在登录页并显示权威状态。（审核：`project/src/game/myserver/plugin.rs` 的 `RegisterResponse::Login` 复用 `handle_login_success`，`PendingReview` 独立发出审核事件；`cargo test myserver --lib` 103 passed。）
- [x] 将密码找回、验证码、第三方登录、用户协议和账号注销标记为本清单非目标。（审核：本 checklist 的目标与依赖边界仅覆盖密码账号注册入口，未向客户端协议或认证页加入上述业务。）

## 阶段 2：认证页面状态和宿主契约

- 开始时间：2026-08-04 10:53:22 +08:00
- 结束时间：2026-08-04 11:16:21 +08:00
- 开发总结：注册入口已接入既有 auth fixed host，密码及确认密码仅保留在受控 ECS 输入中，注册 action、binding 和冲突操作均受 closed contract 约束。
- 验证记录：worker 运行 cargo fmt、cargo test game::screens::auth --lib（44 passed）、cargo check 和注册回归测试；主审核复跑 cargo test game::screens::auth --lib（44 passed，只有既有 dead-code warnings）。

- [x] 为登录页增加登录/注册局部模式状态，不通过新增顶层 Rust View 绕过声明式页面。（审核：`AuthEntryMode::{Login, Register}` 位于 `project/src/game/screens/auth/host.rs`；approved document 未在本阶段修改。）
- [x] 注册 `auth.register` 等稳定 action ID，限定 document、owner、source node 和参数 schema。（审核：host 注册 `auth.show_registration`、`auth.show_login`、`auth.register`、`auth.dismiss_registration_review` 的 closed action descriptor；allowlist 测试通过。）
- [x] 为注册模式、请求中、注册失败、等待审核和注册成功定义 owner/local binding；密码值不得成为 binding。（审核：`auth.register.*` binding schema 仅含模式与反馈，`auth_registration_bindings_expose_mode_and_feedback_without_sensitive_values` 通过。）
- [x] 将账号、密码、确认密码的读取和清理集中在 auth host adapter，拒绝伪造 document/owner/source node 的注册 action。（审核：`document_registration_credentials` 只读取 active fixed instance，伪造 action 与确认密码测试通过。）
- [x] 提交期间禁用注册、登录、游客登录和环境切换中的冲突操作，确保同一帧和跨帧重复点击只产生一个请求。（审核：`RegistrationState::Registering` 纳入 request gate，重复及冲突 action 测试通过。）
- [x] 环境切换、退出登录和切换账号时清理注册模式、敏感输入、错误提示和待处理请求状态。（审核：auth host 的全量输入清理路径及环境切换测试通过；cargo test game::screens::auth --lib 44 passed。）

## 阶段 3：登录 approved document 注册界面

- 开始时间：2026-08-04 11:17:30 +08:00
- 结束时间：2026-08-04 12:09:36 +08:00
- 开发总结：approved 登录文档已提供注册表单与反馈状态，并改用 display enum 隐藏模式/反馈容器，消除实窗发现的隐藏节点占位裁切。
- 验证记录：worker 运行 cargo fmt、cargo test game::screens::auth --lib（44 passed）、cargo check；四档实窗截图位于 `%TEMP%/mybevy-auth-registration-stage3-20260804/`，主审核抽查 desktop、phone-landscape、tablet-landscape 截图无溢出或重叠。

- [x] 更新 `project/assets/ui/documents/approved/auth/login.v1.json`，增加登录/注册模式切换、确认密码输入和提交入口。（审核：approved document、promotion manifest 与 auth host contract 同步，auth 测试 44 passed。）
- [x] 为注册模式补齐默认、输入错误、请求中、等待审核、账号冲突、网络失败和服务不可用视觉状态。（审核：`login.v1.json` 使用 `auth.register.state`、error/review/success visibility bindings，auth document/profile 测试通过。）
- [x] 保持本地服/正式服分段选择在登录和注册模式下均可见，并在请求期间使用明确禁用状态。（审核：environment segment 位于模式容器外并复用既有 request lock binding；auth 测试 44 passed。）
- [x] 确认敏感文本输入声明、焦点顺序、IME、键盘提交和返回键行为符合 UI 输入框架约定。（审核：两项密码输入声明为 sensitive，无 value binding、on_change 或 on_submit；显式提交按钮符合敏感输入安全约束，profile 文档解析测试通过。）
- [x] 在 desktop、phone-landscape、phone-1080p-landscape、tablet-landscape 下确认最长错误文案、按钮和输入框不溢出或重叠。（审核：四档实际响应窗口已完成 PrintWindow 截图，`desktop-login-final.png`、`phone-landscape-login.png`、`phone-1080p-landscape-login.png`、`tablet-landscape-login.png` 均无裁切；实窗发现的 hidden 占位问题已改为 `display:flex/none` 并复测。）
- [x] 保持 packaged approved document 加载失败时的首包 fallback，不让 UI 更新失败阻断登录。（审核：未修改 existing approved-document fallback host path，auth startup registration 与 document audit 测试通过。）

## 阶段 4：注册命令、响应和错误闭环

- 开始时间：2026-08-04 12:10:45 +08:00
- 结束时间：2026-08-04 12:20:45 +08:00
- 开发总结：注册响应已完成直接登录、审核、稳定错误反馈、输入清理和迟到响应隔离闭环。
- 验证记录：worker 运行 cargo fmt、auth 44 passed、MyServer 95 passed、cargo check；主审核复跑迟到注册响应测试 1 passed。

- [x] 将合法注册提交转换为唯一一次 `MyServerCommand::Register`，并保持密码不进入 Debug 输出。（审核：auth request gate 与脱敏 command Debug 的定向测试通过。）
- [x] 消费注册直接登录成功事件，复用现有账号 session、角色列表加载和角色选择路由。（审核：RegisterResponse::Login 复用 handle_login_success，auth/MyServer 定向测试通过。）
- [x] 消费等待审核响应，显示稳定文案并阻止进入角色选择、Lobby 和 game proxy。（审核：PendingReview 清除会话并显示注册审核 notice，端到端 pendingReview 测试通过。）
- [x] 映射 `INVALID_LOGIN_NAME`、`INVALID_PASSWORD`、`LOGIN_NAME_EXISTS`、`PASSWORD_REGISTER_UNAVAILABLE`、账号阻断、IP 阻断、维护、限流和网络超时。（审核：RegistrationServerError 优先映射字段错误，其余错误使用稳定 MyServerDisplayError key，auth 错误反馈测试覆盖。）
- [x] 对可重试错误保留可安全复用的账号输入；对成功、环境切换和退出登录清除密码及确认密码。（审核：验证失败保留受控输入；LogoutSucceeded、成功与环境切换均清理 active document 输入，auth 测试通过。）
- [x] 对迟到响应使用请求身份或 session generation 隔离，防止切环境后旧响应覆盖新环境页面。（审核：pending_http request identity 在 reset 后清除，主审核复跑 late_register_response_after_session_reset_is_ignored_by_request_identity 通过。）

## 阶段 5：自动化测试和 UI 审计

- 开始时间：2026-08-04 12:21:48 +08:00
- 结束时间：2026-08-04 12:37:57 +08:00
- 开发总结：补齐注册成功、失败、超时和实际输入清理回归测试，并完成敏感字段与 UI contract 审计。
- 验证记录：auth 45 passed，MyServer 97 passed，cargo fmt、cargo check 与 ui-generation check-boundary 全部通过；阶段 3 四档实窗截图复用为 UI 验收记录。

- [x] 增加 auth host 测试，覆盖 action allowlist、source node、重复提交、确认密码不一致和敏感输入清理。（审核：auth 45 项覆盖上述 action/input 场景，实际 fixed-host 输入清理回归测试通过。）
- [x] 增加注册直接登录、等待审核、重复账号、服务不可用、超时和切环境迟到响应测试。（审核：MyServer 97 项覆盖直接登录、pending review、409/503、超时和 reset 后旧 request ID 响应忽略。）
- [x] 验证普通 binding/action/debug 输出中不存在密码和确认密码。（审核：binding/action schema 审计和 MyServerCommand Debug 脱敏断言通过。）
- [x] 运行 `cargo fmt`、`cargo check` 和 auth/MyServer 相关定向测试。（验证：fmt、check、auth 45 passed、MyServer 97 passed。）
- [x] 运行 `cargo run --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .`。（验证：命令退出 0，全部 JSON 门禁项为 true。）
- [x] 使用仓库约定的四种窗口档位完成登录/注册各状态截图或人工验收，并记录结果。（验证：阶段 3 实窗 desktop、phone-landscape、phone-1080p-landscape、tablet-landscape 截图无重叠或裁切。）

## 阶段 6：前后端契约与文档同步

- 开始时间：2026-08-04 12:38:44 +08:00
- 结束时间：2026-08-04 12:53:06 +08:00
- 开发总结：完成 MyServer auth-http 静态契约复核、客户端登录流程与 UI 敏感输入文档同步。
- 验证记录：MyServer npm run check:proto 六项通过；npm run test:auth-http 3 passed（健康检查、游客登录、bearer 拒绝），未覆盖完整密码注册 DB 链路，完整栈联调未启动。

- [x] 对照 MyServer auth-http 注册实现和公开客户端说明复核请求字段、审核响应和错误码。（验证：RegisterDto/service/controller、register store test 与外部客户端说明静态核对通过。）
- [x] 在服务端仓库运行注册/auth 定向测试前确认依赖和执行范围，并记录真实执行结果。（验证：经确认运行 npm run test:auth-http，3 passed；该脚本未覆盖完整密码注册 DB 链路，已记录限制。）
- [x] 运行 MyServer 协议一致性检查，确认客户端注册适配未与服务端公开契约漂移。（验证：MYSERVER_CLIENT_ROOT=H:\project\mybevy 下 npm run check:proto 六项通过。）
- [x] 更新 `docs/myserver/服务端最新登录流程客户端验收.md` 的注册入口、审核分支和环境切换步骤。（验证：文档增加注册请求、审核/错误和请求身份边界。）
- [x] 检查 `docs/ui/`、`docs/bevy-getting-started.md` 和 `CLAUDE.md` 是否需要同步页面或启动说明。（验证：三处均同步注册、敏感输入与环境切换说明。）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-04 10:31:32 +08:00
- 结束时间：2026-08-04 12:53:06 +08:00
- 验收总结：注册入口、审核/错误闭环、敏感输入隔离、声明式 UI 门禁和四档实窗验收已完成；服务端 auth 定向测试及协议检查通过，完整 MyServer 栈注册联调未启动。

- [x] 用户可以在本地服和正式服登录页切换登录/注册模式，并完成密码账号注册。（验证：approved document、auth host 测试和四档实窗验收通过。）
- [x] 直接登录成功和等待审核两种服务端响应均进入正确页面状态，不出现 Lobby 假成功。（验证：MyServer 直接登录/pending review 回归测试通过。）
- [x] 重复账号、字段错误、安全阻断、维护、限流、断网和超时均有明确且可恢复的反馈。（验证：auth/MyServer 错误映射和超时测试通过。）
- [x] 密码和确认密码不进入普通 binding、action、日志、错误报告或持久化缓存。（验证：sensitive input、schema 审计和 Debug 脱敏测试通过。）
- [x] 登录、游客登录、注册、选角和环境切换的既有行为未回归。（验证：auth 45 passed、MyServer 97 passed、服务端 auth-http 3 passed。）
- [x] 声明式 UI 门禁、Rust 检查、目标分辨率和必要的前后端契约验证全部通过。（验证：check-boundary 全 true、cargo check、四档实窗、check:proto 均通过；完整 MyServer 栈联调未启动。）
