# 活动客户端接入总览 Checklist

## 目标

将 MyServer 活动能力接入 MyBevy 客户端，交付主世界活动入口、活动列表、活动玩法详情、历史领取记录、登录奖励和随机抽奖，并完成协议、生命周期、可靠性、UI 和跨端验收。

## 基础原则

- [x] 活动规则、资格、时间、次数、奖励和随机结果以服务端为最终事实来源。（审核：登录奖励和抽奖页面仅消费 typed server view；写请求不携带资格、成本、奖励、概率或随机结果。）
- [x] 页面代码放在 `project/src/game/screens/activity/`，网络和业务状态放在 `project/src/game/myserver/activity.rs`。（审核：list/detail/login_reward/lottery adapters 位于 screens/activity，协议与 state 位于 myserver/activity。）
- [x] 每份子清单聚焦一个可独立实现、验证和提交的范围；未完成或未验证项保持 `[ ]`。（审核：01、02、04、05 和 06 本地修复已拆分提交；03/06 外部依赖项保持未勾选。）
- [x] 活动状态按 character、活动 ID、版本和连接代际隔离，不记录 ticket、密码或完整请求内容。（审核：request tracker 使用 connection/activity key，页面 tracker 在 scope loss 清除明文 ID，60 项 activity 定向测试通过。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义活动客户端的协议、页面流程、代码边界、状态消费和完成标准。
- 文档名称：主世界界面与场景内面板清单
  - 路径或链接：`docs/界面/checklists/03_主世界界面与场景内面板_checklist.md`
  - 用途说明：提供主世界 HUD 的 approved document、owner、生命周期和输入回归基线。

## 子清单与依赖

| 子清单 | 范围 | 主要依赖 |
| --- | --- | --- |
| [01 活动客户端基础能力](./01_活动客户端基础能力_checklist.md) | 协议快照、MyServer 运行时、typed state、错误和安全边界 | 服务端活动协议 |
| [02 主世界活动入口与列表](./02_主世界活动入口与列表_checklist.md) | 主世界 HUD 活动按钮、活动列表页和列表到详情路由 | 01 |
| [03 活动历史领取记录](./03_活动历史领取记录_checklist.md) | 历史记录协议、分页查询和记录页 | 01、02、服务端历史查询契约 |
| [04 登录奖励玩法](./04_登录奖励玩法_checklist.md) | 登录奖励详情、阶段状态和领取 | 01、02 |
| [05 随机抽奖玩法](./05_随机抽奖玩法_checklist.md) | 抽奖详情、次数/券库存、动作和结果 | 01、02 |
| [06 活动可靠性与最终验收](./06_活动可靠性与最终验收_checklist.md) | 重试、重连、幂等、联调、视觉和跨端验收 | 02、03、04、05 |

## 阶段 1：子清单范围确认

- 开始时间：2026-08-24 17:41:30 +08:00
- 结束时间：2026-08-24 17:52:00 +08:00
- 开发总结：完成 01-06 子清单边界和依赖核对。01 为协议/运行时基础，02 依赖 01 和主世界 approved HUD，04/05 依赖 01/02，03 依赖服务端历史记录 wire contract，06 依赖全部功能和真实联调环境。
- 验证记录：worker 只读核对 `docs/玩法/checklists/01-06`、`docs/玩法/活动/活动客户端功能文档.md`、`project/vendor/myserver`、`project/crates/myserver-protocol`、`project/src/game/myserver` 及 `../MyServer`。MyServer HEAD `f61fbc98f02f1cfac48a1ba11ebdd73e324ad1ae` 仅有活动 1435-1444；全服务端未找到 `ActivityClaimHistoryReq/Res` 或外部 msgType。vendor 快照为 `811a6ba05c3c3d026edc5e6790d523c688104cd5` 且无活动定义，需在 01 同步。

- [x] 确认 01-06 子清单的边界、依赖和服务端阻塞项。（验证：阶段 1 worker 只读核对并形成依赖矩阵；01→02→04/05→06，03 独立依赖服务端历史查询契约。）
- [x] 确认历史领取记录协议由服务端分配消息号后再进入客户端实现。（验证：`../MyServer` HEAD `f61fbc98f02f1cfac48a1ba11ebdd73e324ad1ae` 的 proto、协议文档和活动代码均未定义外部 `ActivityClaimHistoryReq/Res` 或 msgType。）

## 阶段 2：子清单执行跟踪

- 开始时间：2026-08-24 17:52:00 +08:00
- 结束时间：2026-08-24 22:01:27 +08:00
- 开发总结：已完成并提交 01 基础协议/运行时、02 HUD/列表、04 登录奖励、05 随机抽奖和 06 本地可靠性；服务端历史查询契约随后提供并已完成 03 协议/状态和记录页面本地实现，06 实服/视觉/Android/归档项等待外部环境。
- 验证记录：已提交 `a86088c`、`e7c6048`、`e6eef43`、`38ff2cd`、`4b20ea6`、`e21f90c`；历史页面待提交变更独立验证 `cargo fmt --check`、`cargo check --lib`、`cargo test activity --lib`（60 passed）、`cargo test main_world --lib`（233 passed）和 `git diff --check` 通过，只有仓库既有 unused warnings。

- [x] 01-06 子清单均已创建并链接。（验证：`docs/玩法/checklists/01_` 至 `06_` 子清单已归档且在总览依赖表链接。）
- [x] 子清单完成状态、验证记录和阻塞原因与实际代码同步。（验证：01/02/04/05/06 已记录提交和本地测试；03 已完成 1445/1446 协议、状态和页面本地实现；06 保留实服/视觉/Android 未验证项。）

## 最终完成定义

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 01-06 子清单全部完成，或每个未完成项都有明确接受的风险和后续项。（未完成：03 需真实联调与跨端验收；06 需实服会话、固定窗口、Android 和归档，尚未由用户接受风险。）
- [x] 主世界 HUD 活动按钮只打开活动列表；列表项点击后才进入具体活动玩法。（验证：02 阶段 route/action 定向测试通过，HUD action 只写 `OpenList`，详情键只接受列表项动作。）
- [x] 活动列表能打开当前角色自己的历史领取记录，并处理分页、空状态、失败和重试。（验证：03 阶段 3 fixed host、分页 cursor state、empty/error/retry 和 60 条 repeat 安全上限定向测试通过；实服验证保留在 03 阶段 4。）
- [x] 登录奖励、随机抽奖、幂等重放、断线恢复、版本冲突和服务端不可用状态均有可复核证据。（验证：60 项 activity 定向测试、两类 in-process mock、错误分类和 scope 隔离回归测试通过；实服证据仍在 06 未勾选项。）
- [ ] 协议快照、客户端代码、approved UI documents、测试、联调记录和文档同步完成。（未完成：实服联调、固定窗口/Android 证据和最终归档尚缺。）
