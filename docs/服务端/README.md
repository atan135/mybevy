# 服务端接入文档

本目录记录 MyServer 在客户端侧的登录、选角、character-bound ticket、聊天、邮件和跨端联调契约。服务端实现以 `H:/project/MyServer` 当前代码和配置为事实来源；本目录只说明客户端可见边界和验收方式。

## 当前文档

- [服务端最新登录流程客户端验收](./服务端最新登录流程客户端验收.md)：注册、登录、选角、进场、环境切换和公网服务验收。
- [聊天应用级接线](./聊天应用级接线.md)：聊天连接、ticket、重连和邮件通知桥接。
- [邮件客户端公开契约](./邮件客户端公开契约.md)：邮件 HTTPS endpoint、分页、领取状态和错误语义。

## 归档清单

- `清单/` 保存已完成或正在收尾的客户端接入阶段清单。活动中的新任务仍放在仓库根 `summary/`，完成后再复制到本目录归档。

## 维护边界

- 账号 `player_id` 和玩法主体 `character_id` 必须按服务端公开契约区分。
- 生产客户端只消费 auth 下发的 service descriptor，不读取 registry 或直连内部服务。
- 修改 `project/src/game/myserver/`、协议快照、登录环境策略或 chat/mail endpoint 时，应同步检查本目录、根 README 和 `CLAUDE.md`。
