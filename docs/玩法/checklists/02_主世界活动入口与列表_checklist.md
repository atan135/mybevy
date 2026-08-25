# 主世界活动入口与列表 Checklist

## 目标

在主世界 HUD 增加活动入口，打开活动列表页，展示当前角色可读的有效活动，并建立列表、详情和返回的页面路由边界。

## 基础原则

- [x] 主世界 HUD 只负责活动按钮和打开列表的桥接，不复制活动规则。（审核：HUD action 仅写 `ActivityScreenCommand::OpenList`，活动业务在 `screens/activity/`。）
- [x] 活动页面具体代码位于 `project/src/game/screens/activity/`，页面结构使用 approved `UiDocument`。（审核：list/detail/login_reward/lottery hosts 均使用 approved document，解析回归通过。）
- [x] 活动列表只请求摘要；列表项点击后才请求详情并进入具体玩法。（审核：`opening_list_requests_only_the_activity_summary` 和 activity selection route 测试通过。）

## 援引文档

- 文档名称：活动客户端功能文档
  - 路径或链接：`docs/玩法/活动/活动客户端功能文档.md`
  - 用途说明：定义主世界入口、有效活动过滤、列表到详情和历史记录按钮要求。
- 文档名称：主世界界面与场景内面板清单
  - 路径或链接：`docs/界面/checklists/03_主世界界面与场景内面板_checklist.md`
  - 用途说明：复用主世界 HUD owner、generation、输入阻断和 approved document 约束。

## 阶段 1：主世界 HUD 入口

- 开始时间：2026-08-24 18:50:25 +08:00
- 结束时间：2026-08-24 19:36:41 +08:00
- 开发总结：主世界 HUD 增加受控活动入口；入口仅桥接到活动列表，页面代码留在 `screens/activity/`。
- 验证记录：`cargo test activity --lib` 15 项和 `cargo test main_world --lib` 233 项通过；主世界 HUD action 定向测试通过。

- [x] 在主世界 HUD approved document 增加“活动”按钮和稳定 action/node 标识。（验证：`main_world_hud.v1.json` 定义 `main_world.activity` 和 `main_world.open_activity`，promotion registration 同步 action/binding。）
- [x] 在 `project/src/game/screens/gameplay/` 保留 HUD 桥接动作，将页面实现放入 `screens/activity/`。（验证：HUD handler 仅写 `ActivityScreenCommand::OpenList`，list/detail/host 位于 `project/src/game/screens/activity/`。）
- [x] 活动按钮仅在主世界当前 generation active 且 game 连接可用时打开活动列表。（验证：`main_world_activity_is_available` 校验 active generation、authenticated character-bound connection；HUD action 定向测试通过。）
- [x] 活动面板打开时阻断覆盖区域 gameplay 输入，关闭后恢复当前主世界输入。（验证：活动 documents 使用 `block_lower` scroll；关闭时清理 focus 和 detached routes，approved document blocking-surface 测试通过。）

## 阶段 2：活动列表页

- 开始时间：2026-08-24 18:50:25 +08:00
- 结束时间：2026-08-24 19:36:41 +08:00
- 开发总结：新增 approved 活动列表浮层，使用服务端摘要和受控 state 渲染加载、空态和失败重试。
- 验证记录：活动定向测试验证打开列表只请求 summary、过滤和 retry；approved list document 解析测试通过。

- [x] 在 `project/src/game/screens/activity/` 注册活动列表 host、route 和生命周期清理。（验证：`ActivityScreensPlugin` 注册 `main_world_activity_list` fixed host、owner 与 cleanup route。）
- [x] 打开列表只发送 `ActivityListReq`，展示服务端返回的活动摘要。（验证：`opening_list_requests_only_the_activity_summary` 通过，list binding 直接消费 `ActivityClientState.activities`。）
- [x] 过滤草稿、归档、下线和未知类型；保留运行中及仍可查看/领取的有效活动。（验证：`visible_activity_filters_unreleased_offline_and_unknown_activity_types` 通过。）
- [x] 覆盖加载、空列表、加载失败、重试、时间展示和活动版本替换状态。（验证：list load state/bindings 覆盖 loading/ready/error 与 empty repeat；`retrying_a_failed_list_reuses_the_list_route_without_requesting_detail` 通过。）

## 阶段 3：列表到详情路由

- 开始时间：2026-08-24 18:50:25 +08:00
- 结束时间：2026-08-24 19:36:41 +08:00
- 开发总结：列表项受 document/action/source/参数和 generation 共同校验后才打开详情；详情为列表之上的独立浮层。
- 验证记录：活动路由定向测试覆盖 list-only 打开、stale generation 拒绝和 retry 不请求 detail。

- [x] 列表项点击后才以 `activity_id` 和 `version` 请求详情。（验证：`activity_selection` 校验 owner/action/source/typed params，`OpenDetail` 才写 `LoadActivityDetail`。）
- [x] 列表打开时不得自动请求或显示任意活动详情。（验证：`opening_list_requests_only_the_activity_summary` 通过。）
- [x] 详情返回列表时不重建主世界会话，并保留列表滚动位置和有效缓存。（验证：`CloseDetail` 只关闭 detail route，底层 list host/state 保留，未写主世界路由/entry 命令。）
- [x] 快速重复点击、迟到 action 和旧 generation action 均被拒绝或忽略。（验证：screen state 拒绝同 generation 重开与非法 selection；`stale_main_world_generation_cannot_open_an_activity_detail` 通过。）

## 阶段 4：UI 和交互验证

- 开始时间：2026-08-24 19:20:00 +08:00
- 结束时间：
- 开发总结：自动化输入、关闭、焦点和 cleanup 回归完成；固定窗口真实活动视觉验收等待可用的 authenticated character-bound 会话。
- 验证记录：`cargo test activity --lib` 15 项、`cargo test main_world --lib` 233 项、`cargo fmt --check`、`cargo check --lib`、`git diff --check` 通过；未使用伪造活动数据启动 `run_fast.ps1`。

- [x] 接入触控/鼠标、返回、关闭、焦点和重复点击禁用。（验证：document button actions、blocking scroll、CloseDetail/CloseList focus cleanup 和 repeat guards 受定向测试覆盖。）
- [ ] 使用 `scripts/run_fast.ps1` 的 `2772x1280`、`3.25`、`50%` 配置验收窄屏和横屏布局。（阻塞：当前环境没有已认证的 character-bound 活动会话；不得伪造服务端活动数据，待联调环境补验。）
- [x] 验证 HUD、列表和详情之间无重叠、输入泄漏或残留实体。（验证：主世界 teardown 纳入活动 list/detail owners/routes，`main_world` 233 项回归和 activity blocking-surface document 测试通过；真实渲染视觉验收待补。）

## 最终完成定义

- 开始时间：
- 结束时间：
- 验收总结：

- [x] 主世界 HUD 有活动按钮，点击后只打开活动列表。（验证：`active_main_world_activity_button_opens_only_the_activity_list_bridge` 通过。）
- [x] 列表只展示当前角色可读的有效活动。（验证：`visible_activity_filters_unreleased_offline_and_unknown_activity_types` 通过。）
- [x] 只有点击列表项后才能进入对应活动玩法详情。（验证：list action 的 activity/version 校验、dedicated detail route 和 stale generation 回归通过。）
- [x] 页面生命周期、输入路由和主世界 generation 回归测试通过。（验证：`cargo test activity --lib` 40 passed、`cargo test main_world --lib` 233 passed；固定窗口真实视觉验收仍保留阶段 4 阻塞项。）
