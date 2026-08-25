# 活动可靠性与最终验收 Checklist

## 目标

对活动列表、历史记录、登录奖励和随机抽奖执行跨模块可靠性、生命周期、UI、联调和跨端最终验收，确认活动功能可交付。

## 基础原则

- [x] 验收以服务端响应和可复核证据为准，不以客户端临时状态代替成功。（审核：动作成功/duplicate/unknown 都请求 detail/progress 刷新；本轮同时保留进程内 mock 与真实 dev-stack 的 action/replay、数据库和历史记录证据。）
- [x] 所有重试、重连、处理中和结果未知路径保持幂等，不重复发奖或扣除资源。（审核：login reward/lottery trackers 保留原请求 ID；mock 与 reconciling 回归测试覆盖重复和恢复。）
- [ ] UI 验收使用仓库规定的固定窗口、设备比例和安全区环境。（阻塞：缺少已认证角色会话及真实活动数据，无法从 `run_fast.ps1` 验收列表、两类详情、历史、加载和错误状态。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义活动生命周期、错误行为、协议安全和整体完成标准。
- 文档名称：公网部署后 Android 真机验收清单
  - 路径或链接：`summary/07_公网部署后Android真机验收_checklist.md`
  - 用途说明：复用登录、主世界、前后台、网络和 Android 真机验收环境。
- 文档名称：界面自动化审计与主世界界面清单
  - 路径或链接：`docs/界面/` 下对应文档和清单
  - 用途说明：核对 approved document、输入路由、视觉和响应式验收。

## 阶段 1：错误与重试闭环

- 开始时间：2026-08-24 21:38:40 +08:00
- 结束时间：2026-08-24 21:58:46 +08:00
- 开发总结：补齐限流/连接不可用分类、受限错误码清洗、人工核对锁定、失败请求活动键隔离和页面级 retry ID scope 清理。
- 验证记录：`cargo test activity --lib` 主审通过，40 passed；覆盖限流、未知错误脱敏、manual review、old detail failure、scope loss 和 duplicate/retry 路径。

- [x] 覆盖能力不可用、资格不足、已领取、版本冲突、限流、处理中、可重试失败和人工核对状态。（审核：`ActivityErrorKind`/两类页面错误分支覆盖上述分类，manual review/reconciling 与 rate-limit 受控行为有定向测试。）
- [x] 超时、断线和重连复用同一 `client_request_id`，不重复弹奖励或扣券。（审核：login reward/lottery retry tracker 与 in-process duplicate mock 测试通过，结果始终等候 server detail 收敛。）
- [x] 未知 `ACTIVITY_*` 错误按脱敏失败处理，并提供可重试或返回入口。（审核：`sanitize_activity_error_code` 只保留受限格式/长度的 code，其余归并 `ACTIVITY_UNKNOWN`；页面显示受控失败/核对状态。）

## 阶段 2：生命周期与隔离

- 开始时间：2026-08-24 21:38:40 +08:00
- 结束时间：2026-08-24 21:58:46 +08:00
- 开发总结：scope 改变清空活动 state/pending；页面 retry tracker 在无 character 时清除明文 ID，失败事件携带可选 activity key，旧 detail 失败不污染当前详情。
- 验证记录：40 项 activity 定向测试及 233 项 main-world 测试主审通过；approved activity routes 已纳入主世界 teardown owner/route 顺序。

- [x] 切换角色、登出、切换环境、ticket 刷新和 game 连接代际变化时清空或隔离活动状态。（审核：`ActivityClientState::sync_scope` 清理 state/tracker；scope change 和两类 page-local tracker 清理定向测试通过。）
- [x] 旧响应、旧 push、迟到 UI action 不覆盖新角色、新版本或新主世界 generation。（审核：request connection ID/活动 key 校验、old detail failure 与 stale main-world generation 定向测试通过。）
- [x] 活动页面关闭、返回主世界和返回 Lobby 后无残留 document、实体、pending request 或输入阻断。（审核：main-world teardown 明确关闭列表/通用详情/login reward/lottery routes 与 owners；`cargo test main_world --lib` 233 passed。）

## 阶段 3：端到端联调

- 开始时间：2026-08-25 09:10:40 +08:00
- 结束时间：2026-08-25 17:54:23 +08:00
- 开发总结：完成 MyServer 联调准备核查，并记录真实抽奖、奖励发放、历史记录查看、同 request 重放、完整连续流程和角色隔离结果；累计 7 天登录奖励也已完成本地真实领取。上述结果暂不作为最终验收，待上线真机阶段统一复验。跨自然日抽奖、双实例和故障注入仍未完成。
- 验证记录：初始核查的 MyServer HEAD 为 `f61fbc98f02f1cfac48a1ba11ebdd73e324ad1ae`；历史协议服务端来源为 `13b1d9e019553400ea94cbf5d69f5ff4bbdb9677`，已同步 client vendor。真实角色历史记录当前有 3 条已发放记录：累计 3 天登录奖励、一次抽奖、累计 7 天登录奖励；7 天 claim 为 `granted`，奖励 `6001 x1` 已写入发奖账本和角色背包，登录奖励状态版本为 5，阶段 3/7 均已领取且累计/连续均为 7。用户确认完整连续流程通过，并确认多角色记录不串线、切换角色和登出后旧记录清空。此前同一抽奖请求重放两次均返回 `duplicate=true`，数据库未产生第二条 claim/draw/grant；新请求受每日次数限制拒绝。以上均保留为本地联调记录，不勾选为最终验收项。抽奖状态仍预置为 `last_draw_period_key=2026-08-24`、`daily_draw_count=0`，等待跨日动作。`scripts/dev-stack.ps1 -RestartGame` 已用于抽奖初始化修复后的服务端重启。

- [ ] 复现 `main_world -> activity_list -> detail -> progress -> action -> replay -> detail` 流程。（记录：用户已确认完整连续流程通过；本地结果暂不作为最终验收，待上线真机复验。）
- [ ] 验证历史记录入口、分页和记录与当前角色绑定。（记录：用户已确认多角色记录不串线，切换角色/登出后旧记录清空；当前角色数据库历史记录与 3 天登录、抽奖、7 天登录三条事实一致；本地结果暂不作为最终验收。）
- [ ] 验证首次 action `duplicate=false`，同 request replay `duplicate=true`。（记录：首次真实抽奖记录已为 `granted`；2026-08-25 使用同一 character-bound ticket 重放该 request 两次，服务端均返回 `ok=true`、`duplicate=true`、`processing=false`、`state_revision=1`，数据库仍只有原 claim/draw/grant。全新 request 在每日次数已用尽时返回 `ACTIVITY_QUALIFICATION_NOT_MET`，未产生新记录；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 验证双 game-server 实例、服务端 disabled、版本冲突和存储不可用场景。（待联调：需要服务端测试环境和实例拓扑。）
- [ ] 验证真实抽奖 action 成功、奖励发放并在历史记录页展示奖励摘要。（记录：2026-08-25 17:05:28 +08:00，真实角色收到 `6001 x1`，记录状态为 `granted`，历史记录页面显示同一条记录；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 验证跨自然日后抽奖每日次数归零并允许新的一次抽奖。（待联调：服务端状态已预置 `last_draw_period_key=2026-08-24`、`daily_draw_count=0`，等待客户端发起下一次抽奖并确认写回今日日期。）
- [ ] 验证累计 7 天登录奖励可领取并在服务端收敛为已领取。（记录：2026-08-25 17:46:10，服务端 claim 为 `granted`，`6001 x1` 已到账，阶段 7 已领取标记和累计/连续 7 天状态已持久化；暂不作为最终验收，待上线真机复验。）

## 阶段 4：UI、性能与跨端验收

- 开始时间：2026-08-25 14:46:34 +08:00
- 结束时间：
- 开发总结：
- 验证记录：首次固定窗口 UI 操作发现 activity list 因 `UI_TEXT_I18N_KEY_MISSING` 不能加载；`e80c806` 已补齐 list/history 的 `en_us`/`zh_cn` 文案，并以实际 RON catalog 验证 approved document。第二次操作发现 `UI_DECLARATIVE_SCREEN_BINDING_CONTRACT_MISMATCH`；host 现仅暴露 Document/Owner scope，Item binding 留给 repeat runtime，登录奖励 item binding 已改为 Item scope，五个活动页面真实 `validate_host_source` 回归通过。仍需用户重新运行 `run_fast.ps1` 取得真实窗口结果。

- [x] 运行 `cargo fmt --check`、`cargo check` 和活动定向测试。（验证：`cargo fmt --check`、`cargo check --lib`、`cargo test activity --lib` 60 passed、`cargo test main_world --lib` 233 passed。）
- [ ] 从仓库根目录运行 `scripts/run_fast.ps1`，按 `2772x1280`、`3.25`、`50%` 验收列表、详情、记录、加载和错误状态。（待联调：当前没有已认证活动会话；历史记录协议和页面已完成，但尚无真实页面状态可验收。）
- [ ] 验证桌面触控/鼠标、Android 安全区、系统返回键、前后台和断网恢复。（待联调：需要目标设备/Android 环境和真实服务端会话。）
- [ ] 记录稳定页面的截图、日志和性能观察，不提交 ticket、密码、完整 request ID 或生产凭据。（待联调：没有可记录的实服稳定页面；本地测试未输出敏感请求 ID。）

## 阶段 5：归档与文档同步

- 开始时间：2026-08-25 12:22:47 +08:00
- 结束时间：
- 开发总结：
- 验证记录：

- [x] 活动功能文档、协议快照、客户端代码、approved UI documents 和测试证据同步完成。（审核：`docs/玩法/活动/活动客户端功能文档.md` 已记录 1445/1446、`6f696fb`、20/50/60 分页边界、approved document 契约、60 项 activity 与 233 项 main-world 本地测试，并明确实服/跨端未执行。）
- [x] 更新 `docs/玩法/README.md` 的活动清单/归档入口。（审核：README 的活动条目已说明历史记录范围、本地与实服/跨端验收边界，以及全部子项完成后归档到 `docs/玩法/checklists/`；未链接 Git 忽略的 `summary/`。）
- [ ] 所有子清单完成后，将活动模块清单归档到 `docs/玩法/checklists/`，不保留未说明状态的临时项。

## 最终完成定义

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 活动入口、列表、历史记录、登录奖励和随机抽奖完整流程通过。
- [ ] 重复请求、处理中、断线重连、版本冲突、服务端 disabled 和人工核对路径均有结果证据。
- [ ] 桌面和 Android 目标环境的 UI、输入、生命周期和网络行为通过或记录明确接受的风险。
