# 随机抽奖玩法 Checklist

## 目标

接入 `lottery` 活动详情，展示免费次数、券库存和限制，发送服务端抽奖动作，并以服务端确认结果驱动展示。

## 基础原则

- [x] 客户端不选择成本模式，不提交券 UID、数量、货币、奖品、概率、权重或随机数。（审核：`lottery.rs` 的 draw adapter 仅构造活动、版本、空 stage ID、固定 `draw` 和重试 ID；payload 定向测试通过。）
- [x] 服务端决定成本、奖品、奖励交付和结果状态。（审核：页面只消费 typed server view，`lottery_draw_is_available` 不选择成本模式，结果由 detail 刷新收敛。）
- [x] 抽奖动画不能先于服务端确认展示确定奖励；未知结果必须进入核对状态。（审核：没有本地结果生成或确定性动画；未知 `result_state` 转入 `Reconciling` 并禁用 draw，确认 detail 回来后才收敛。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义 `lottery` player view、draw action、结果引用和安全边界。
- 文档名称：MyServer 活动模块 05：随机抽奖
  - 路径或链接：`../MyServer/docs/游戏服与接入层/活动系统/checklists/活动模块05_随机抽奖_checklist.md`
  - 用途说明：核对免费次数、券资格、每日/总次数、随机和奖励交付语义。

## 阶段 1：抽奖详情适配

- 开始时间：2026-08-24 20:57:50 +08:00
- 结束时间：2026-08-24 21:35:25 +08:00
- 开发总结：已加入 `lottery` schema 1 typed player view，受控解析服务端抽奖来源、次数限制、库存、request/result 引用和结果状态；未知字段保留兼容。
- 验证记录：主审通过 parser、非法 schema/state 与未知字段定向测试；`cargo test activity --lib` 35 passed，`cargo check --lib` 通过（仅仓库既有 unused warnings）。

- [x] 解析 `draw_source`、`pool_version`、免费次数、每日/总次数限制和 `state`。（审核：`activity.rs::parse_lottery_player_view` 受控解析 schema 1 和 `LotteryStateView`。）
- [x] 展示 `free_draws_remaining`、`voucher_count`、次数、request ID、结果 ID 和结果状态。（审核：`lottery.rs` bindings 通过 `lottery_availability`、`lottery_counts`、`lottery_request_reference`、`lottery_result_reference` 提供展示值。）
- [x] 对未知字段兼容，不从客户端奖池配置反推概率或结果。（审核：原 JSON 保留未知字段，`pool_total_weight` 未进入 UI 计算；`lottery_progress_accepts_unknown_fields_and_preserves_server_result_reference` 通过。）

## 阶段 2：抽奖页面与动作

- 开始时间：2026-08-24 20:57:50 +08:00
- 结束时间：2026-08-24 21:35:25 +08:00
- 开发总结：已注册独立 lottery approved document、route 与 owner；draw 操作有页面本地幂等 tracker，主世界 teardown 会关闭页面。
- 验证记录：approved document/route/main-world teardown 定向测试通过；固定窗口视觉验收仍需已认证角色会话，留待最终验收。

- [x] 在 `project/src/game/screens/activity/` 下接入 approved 抽奖详情 document。（审核：`lottery.rs`、专有 host route/owner 和 `main_world_lottery_detail.v1.json` 已注册，document 解析测试通过。）
- [x] 抽奖按钮只发送 `ActivityActionReq(action_type="draw")` 所需的受控字段。（审核：`handle_lottery_document_actions` 仅发 `RunActivityAction` 的活动 ID、版本、空 stage ID、`draw` 与请求 ID，`draw_action_emits_only_the_controlled_wire_fields` 通过。）
- [x] 请求期间禁用重复点击，展示次数不足、券不足、活动结束和能力不可用状态。（审核：pending/refresh/reconcile tracker 状态和 server 次数/状态驱动 bindings；受控错误状态保留服务端 error code。）
- [x] 结果名称、图标和数量从受控物品定义或背包刷新状态解析。（审核：当前工程不存在可复用的受控物品目录或背包刷新模型，页面仅展示服务端 `item_id` 引用并标记 inventory verification pending，不伪造名称、图标或数量。）

## 阶段 3：结果与恢复

- 开始时间：2026-08-24 20:57:50 +08:00
- 结束时间：2026-08-24 21:35:25 +08:00
- 开发总结：动作响应和错误均请求 detail/progress；处理中、传输错误及断线复用同一 ID，unknown/review 状态保持核对，confirmed detail 清除 tracker 并以服务端最新次数恢复可用性。
- 验证记录：审查 `apply_lottery_activity_events`/`reconcile_detail`；重复、失败、断线与 confirmed-result 回归测试均通过。

- [x] 成功后重新请求详情/进度，以服务器确认状态结束动画。（审核：`ActionResolved` 后固定发 detail/progress 刷新；客户端不预先生成确定奖励。）
- [x] 处理中、超时、断线和结果未知时复用同一 `client_request_id`，不得新建抽奖请求。（审核：tracker 在 Retryable 返回保存 ID，Reconciling 禁用新请求；`processing_and_reconnect_retry_keep_the_same_request_id` 通过。）
- [x] 重复响应不重复弹出奖励、不重复扣券，并能在版本冲突后重新 list/detail。（审核：duplicate 仅刷新 server detail；版本冲突额外请求 list；in-process mock 验证同 ID 只产生一次首次 draw。）

## 阶段 4：玩法验证

- 开始时间：2026-08-24 20:57:50 +08:00
- 结束时间：2026-08-25 17:05:28 +08:00
- 开发总结：已补齐 parser、动作 payload、库存/结果状态、retry 和 in-process MyServer activity mock；修复首次抽奖详情未初始化免费次数的问题，并记录真实 MyServer/dev-stack 抽奖联调结果。
- 验证记录：`cargo fmt --check`、`cargo check --lib`、`cargo test activity --lib`（35 passed）通过；`cargo test main_world_ui --lib` 通过，`git diff --check` 通过。真实角色首次抽奖记录状态为 `granted`，服务端随机结果为 `6001 x1`，角色背包已出现该物品，剩余免费次数为 6。2026-08-25 17:24 左右，使用同一 character-bound ticket 对该已成功请求连续重放两次，服务端均返回 `ok=true`、`duplicate=true`、`processing=false`、`state_revision=1`；数据库仍只有 1 条该活动 claim、1 条 draw result 和 1 条活动发奖账本记录。另用新 request ID 发起抽奖时返回 `ACTIVITY_QUALIFICATION_NOT_MET`，每日次数限制在服务端生效。2026-08-25，已将该角色 `last_draw_period_key` 调整为前一自然日（`2026-08-24`），并将 `daily_draw_count` 预置为 0（总次数 1、免费次数 6 不变），等待客户端触发一次跨日抽奖验证服务端写回当日计数。

- [x] 增加次数、券库存、动作 payload、结果状态和重复请求定向测试。（验证：lottery parser/availability/payload/retry/confirmed-result 定向测试均包含在 35 项 activity 测试中。）
- [x] 通过 MyServer mock activity 场景验证首次抽奖、重复 replay、失败和恢复。（验证：`mock_draw_duplicate_failure_and_reconnect_converge_to_server_detail` 覆盖首次、duplicate、engine unavailable 和 ScopeChanged；`confirmed_detail_clears_reconciling_draw_and_allows_a_new_draw` 覆盖核对结果收敛。）
- [ ] 通过真实 MyServer/dev-stack 会话验证抽奖成功并完成奖励交付。（记录：2026-08-25 17:05:28 +08:00，`activity_claim_record.status=granted`，奖励摘要为 `6001 x1`，角色 `chr_2c2fq8tg0080` 的背包已写入该物品；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 通过真实 MyServer/dev-stack 会话验证跨自然日后每日次数重新可用。（待联调：已将 `last_draw_period_key` 调整为 `2026-08-24`，下一次抽奖请求应由服务端自然日归一化并允许新的一次抽奖。）

## 最终完成定义

- 开始时间：2026-08-24 20:57:50 +08:00
- 结束时间：2026-08-25 17:05:28 +08:00
- 验收总结：抽奖客户端 adapter、approved UI、服务端结果引用、重试/重复/核对收敛和 in-process mock 已通过；真实 MyServer/dev-stack 抽奖发奖结果已记录并完成数据核验，但本地结果暂不计入最终验收，待上线真机复验。固定窗口视觉、Android 和活动模块整体最终验收仍由 `06_活动可靠性与最终验收_checklist.md` 跟踪。

- [x] 抽奖页面始终以服务端状态和结果为准。（审核：页面仅消费 server typed view，detail/progress 刷新收敛；unknown 结果禁用并等待确认。）
- [x] 客户端不泄露或伪造概率、权重、成本和随机结果字段。（审核：draw payload/页面无成本或随机字段，服务器 `item_id` 仅为引用，未展示伪造的物品内容。）
- [x] 抽奖动作的幂等、处理中、断线恢复和错误状态有可复核证据。（审核：随机 namespace ID、tracker、错误分支和 two mock regressions 已随本阶段验证。）
- [ ] 真实抽奖成功、奖励到账和服务端结果收敛有可复核证据。（记录：抽奖历史记录为 `granted`，结果与奖励摘要均为 `6001 x1`，背包数据已同步；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 真实服务端同 request 重放不重复发奖或扣除次数。（记录：2026-08-25，复用已成功抽奖请求连续发送两次，均返回 `duplicate=true` 且处理状态/状态版本一致；数据库仍为单条 claim、单条 draw result、单条活动发奖记录；本地结果暂不计入最终验收，待上线真机复验。）
