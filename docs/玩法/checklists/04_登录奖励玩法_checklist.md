# 登录奖励玩法 Checklist

## 目标

接入 `login_reward` 活动详情，展示服务端周期和阶段状态，支持符合资格的阶段领取及自动领取状态。

## 基础原则

- [x] 阶段资格、周期推进、领取时间和奖励交付由服务端决定。（审核：`login_reward.rs` 只消费服务端 `achieved`、`claimable`、`claimed`、`claim_mode` 和活动状态；领取命令不携带资格、时间或奖励字段。）
- [x] `stage_id` 使用服务端 `stage_no` 转换结果，不使用客户端数组下标。（审核：`login_reward.rs` 用匹配到的 `stage_no.to_string()` 生成领取字段，`stage_id_uses_server_stage_number_instead_of_list_offset` 通过。）
- [x] `ActivityProgressRes.progress_json={}` 不得清空最近一次可信 detail 状态。（审核：`activity.rs::apply_progress_to_detail` 保留可信 detail，`placeholder_progress_preserves_a_trusted_login_reward_detail` 通过。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义 `login_reward` player view、阶段状态和领取行为。
- 文档名称：MyServer 活动模块 04：登录奖励
  - 路径或链接：`../MyServer/docs/游戏服与接入层/活动系统/checklists/活动模块04_登录奖励_checklist.md`
  - 用途说明：核对服务端周期、资格、领取和幂等语义。

## 阶段 1：登录奖励详情适配

- 开始时间：2026-08-24 19:38:13 +08:00
- 结束时间：2026-08-24 20:55:38 +08:00
- 开发总结：已完成 schema 1 受控 player view adapter；解析阶段 ID 严格取服务端 `stage_no`，并保留可信 detail 以兼容当前 progress `{}` 占位响应。
- 验证记录：主审查通过 `activity.rs` parser/fallback 与 26 项 activity 定向测试；`cargo fmt --check`、`cargo check --lib` 均通过（仅仓库既有 unused warnings）。

- [x] 解析 `type`、`schema_version`、周期、连续/累计计数和今日状态。（审核：`activity.rs::parse_login_reward_player_view` 受控解析 schema 1、周期、计数和 `today_status`，`progress_view_accepts_unknown_fields` 通过。）
- [x] 解析阶段列表中的 `stage_no`、`required_count`、`reward_group_key`、`achieved`、`claimable`、`claimed`。（审核：`activity.rs::parse_login_reward_stage` 和 typed `LoginRewardStageView` 覆盖全部字段。）
- [x] 对未知字段保持兼容，对非法 schema 或过大 JSON 显示受控错误。（审核：原始 JSON 保留未知字段；`login_reward_rejects_invalid_schema_and_duplicate_stage_numbers`、`login_reward_rejects_oversized_or_invalid_stage_fields` 通过。）

## 阶段 2：登录奖励页面

- 开始时间：2026-08-24 19:38:13 +08:00
- 结束时间：2026-08-24 20:55:38 +08:00
- 开发总结：已接入专用 approved UiDocument、detached route、owner 与主世界 teardown；绑定只显示服务端传回的受控状态。
- 验证记录：approved 文档解析/阻断滚动测试及活动 route 测试通过；真实已登录角色会话的固定窗口视觉验收留待总体验收阶段。

- [x] 在 `project/src/game/screens/activity/` 下接入 approved 登录奖励详情 document。（审核：`login_reward.rs`、专有 host route/owner 和 `project/assets/ui/documents/approved/activity/main_world_login_reward_detail.v1.json` 已注册，approved document 解析测试通过。）
- [x] `achieved=false` 显示未达成，`claimable=true` 才显示可领取动作，`claimed=true` 禁止再次领取。（审核：`login_reward_stage_status`/`stage_claim_is_available` 使用服务端字段，`stage_binding_covers_claimed_and_retryable_render_states` 通过。）
- [x] `claim_mode=automatic` 不显示手动领取按钮，等待服务端状态刷新。（审核：`stage_claim_is_available` 要求 `manual`，`automatic_or_unachieved_stages_never_offer_manual_claim` 通过。）
- [x] 覆盖未开始、运行中、已结束宽限、已领取、空状态和服务端错误。（审核：binding/approved document 覆盖 `published`、`running`、`ended` grace、阶段已领取、repeat empty/error 状态。）

## 阶段 3：阶段领取流程

- 开始时间：2026-08-24 19:38:13 +08:00
- 结束时间：2026-08-24 20:55:38 +08:00
- 开发总结：领取仅发活动、版本、服务端阶段 ID 和重试 ID；ID 使用随机 nonce 命名空间，处理中和断线重试复用原值，响应后受控刷新 detail/progress。
- 验证记录：审查 `handle_login_reward_document_actions`/`apply_login_reward_activity_events`，并通过 retry、跨 tracker ID 隔离、命令载荷和 mock 回归测试。

- [x] 发送 `ActivityClaimReq` 时只提交活动 ID、版本、阶段 ID 和客户端请求 ID。（审核：`handle_login_reward_document_actions` 仅构造 `ClaimActivityStage` 四字段，`claim_action_emits_only_the_server_stage_mapping_and_retry_id` 通过。）
- [x] 成功、重复、处理中、资格不足和版本冲突按服务端结果刷新详情/进度。（审核：`apply_login_reward_activity_events` 根据 `ActionResolved`/typed 错误刷新 detail/progress，版本冲突额外刷新列表。）
- [x] 领取按钮在请求期间禁用，超时和断线重试复用同一 `client_request_id`。（审核：页面 tracker 保持 entry 原 ID；随机 nonce + 序号避免重启碰撞；`retry_processing_and_disconnect_keep_the_same_request_id` 与 `fresh_trackers_use_distinct_bounded_request_ids` 通过。）

## 阶段 4：玩法验证

- 开始时间：2026-08-24 20:44:39 +08:00
- 结束时间：2026-08-25 17:46:10 +08:00
- 开发总结：补充进程内 MyServer activity mock，覆盖首次成功、同 request ID duplicate、连接代际恢复和服务端 detail 已领取收敛；真实 MyServer/dev-stack 登录奖励联调结果已记录，待上线真机阶段统一验收。
- 验证记录：`cargo test activity --lib` 主审通过，26 passed；mock 验证首次/重复响应各触发一组 detail/progress 刷新，重连复用 ID，已领取 detail 收敛后禁领。真实角色完成累计 3 天阶段领取后，服务端 `activity_claim_record` 状态为 `granted`，奖励 `1001 x1` 与角色绑定 `4001 x1` 已发放。2026-08-25 17:46:10，累计 7 天阶段已实际领取成功，服务端新增 `granted` claim，奖励 `6001 x1` 已写入发奖账本和当前角色背包；`player_activity_state.state_revision=5`，累计/连续计数均为 7，阶段 3 和阶段 7 均已领取。本次仅作本地联调记录，暂不计入最终验收，待上线真机复验。

- [x] 增加阶段映射、状态渲染、领取命令和 detail fallback 定向测试。（验证：`cargo test activity --lib` 通过，25 passed，覆盖上述映射、渲染、命令、fallback 和 retry 场景。）
- [x] 通过 MyServer mock activity 场景验证首次领取、重复领取和恢复。（验证：`mock_claim_success_duplicate_and_reconnect_reconcile_to_server_detail` 注入首次/duplicate `ActionResolved` 与 `ScopeChanged`，验证刷新、同 ID 重试和 claimed detail 收敛禁领。）
- [ ] 通过真实 MyServer/dev-stack 会话验证累计 3 天阶段领取，并确认领取后状态为已领取。（记录：2026-08-25 16:51:46 +08:00，服务端记录为 `granted`，奖励已到账，后续状态包含该阶段的已领取标记；本地结果暂不计入最终验收，待上线真机复验。）
- [ ] 通过真实 MyServer/dev-stack 会话验证累计 7 天阶段可领取并在领取后收敛为已领取。（记录：2026-08-25 17:46:10 已测试成功，claim 状态为 `granted`，奖励 `6001 x1` 已到账，阶段 7 已领取标记已写入；按当前验收策略暂不勾选，待上线真机复验。）

## 最终完成定义

- 开始时间：2026-08-24 19:38:13 +08:00
- 结束时间：2026-08-25 17:46:10 +08:00
- 验收总结：客户端登录奖励 adapter、approved UI、领取重试/刷新、进程内 MyServer mock 已通过；真实 MyServer/dev-stack 的累计 3 天、7 天奖励领取结果已记录并完成数据核验，但本地结果暂不计入最终验收，待上线真机复验。固定窗口视觉、Android 和活动模块整体最终验收仍由 `06_活动可靠性与最终验收_checklist.md` 跟踪。

- [x] 登录奖励详情正确展示服务端阶段状态。（审核：typed server player view 到 owner bindings 的映射、状态分支和 approved document 均经 activity 定向测试验证。）
- [x] 阶段领取不重复扣除或重复展示奖励，并能在重连后收敛到服务端状态。（审核：in-process mock 验证 duplicate 不形成第二次服务端首次领取，重连复用 ID，claimed detail 收敛后禁领。）
- [x] UI、协议 adapter、错误状态和测试证据同步完成。（审核：schema/错误、专有 UI host、route cleanup、命令/重试及 26 项定向测试均在本阶段提交前通过。）
- [ ] 真实 MyServer 登录奖励领取完成并收敛为已领取状态。（记录：本地 dev-stack 已验证累计 3 天和 7 天阶段均为 `granted` 并完成发奖，但本项暂不作为最终验收，待上线真机复验。）
