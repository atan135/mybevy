# 公网部署后 Android 真机验收 清单

## 目标

在 02-06 功能、桌面联调、服务端验证和公网准入完成后，将与目标客户端版本匹配的 MyServer、auth、game proxy、chat WSS、mail HTTPS 和 Caddy 配置部署到公网，再使用 Android 真机执行安装、完整业务路径、系统生命周期、网络切换、视觉、性能和安全验收。

本清单不使用 Android 真机连接本机 Local MyServer，不用本地端口转发、ADB reverse 或临时明文入口替代公网部署。公网版本、目标 endpoint 或测试账号未确认时，不启动真机验收。

## 前置清单与输入

- `02_主世界进场与权威闭环_checklist.md`
- `03_主世界界面与场景内面板_checklist.md`
- `04_聊天应用级接线_checklist.md`
- `05_邮箱界面与通知闭环_checklist.md`
- `06_登录进场主界面与通信出口跨端联调验收_checklist.md`
- 目标 Git commit/branch、Android versionCode/versionName、APK SHA-256 和构建记录。
- 已部署的 auth HTTPS、game KCP/TCP、chat WSS、mail HTTPS 公网 endpoint 与可回滚版本。
- 获授权的测试账号/角色/邮件数据，以及可用 ADB 真机。

## 基础原则

- [ ] 真机只访问已批准的公网 endpoint，不直连 registry、Redis、NATS、PostgreSQL、admin 或内部 listener。
- [ ] 真机验收的 APK、服务端、Caddy 与配置必须关联到明确版本，禁止在测试期间无记录漂移。
- [ ] 不在日志、截图、报告或文件名中保存完整 ticket、token、密码、邮件正文、附件内容或聊天正文。
- [ ] 每个阶段记录设备、Android API、网络类型、版本、时间窗口、公网 endpoint 指纹和结果。
- [ ] 发现功能缺陷时回到 02-06 对应清单修复、重新部署并重跑受影响真机项，不在真机环境做无记录临时绕过。

## 阶段 1：公网部署与版本准入

- 开始时间：2026-08-07 16:05:25 +08:00
- 结束时间：2026-08-07 18:05:12 +08:00
- 开发总结：完成前置清单、生产 release、公网 descriptor、协议、回滚点和候选 APK 准入；处理只读检查中 Redis 凭据进入内部工具输出的安全事件，完成生产 Redis 与 9 个消费者的凭据轮换并使旧凭据失效。轮换恢复中定位并规避 auth worker lease、match registry 启动竞态、game-server stale lease/Unix socket 三类问题，最终固定依赖服务→game-server→proxy 的串行恢复顺序。
- 验证记录：客户端 `main@4368ed1`，累计范围 `0673c6b..4368ed1`（37 commits）；生产 release 保持 `v0.1.0-fef5c9b49d36`。最终 13/13 容器运行，10 个 secret 文件与 9 个 runtime Env 一致，新 Redis 凭据有效且旧凭据无效；forward/final postflight 和最终 `postflight --no-deps` 通过，API 200、mail 无票 401、WSS 101、UDP 4000 与 Caddy validate 正常。303 秒 11/11 采样中容器 ID/RestartCount 不变，观察期目标错误为 0；旧凭据备份与临时文件已删除。范围偏差：一次未带 `--no-deps` 的 postflight 因 `infrastructure.env` 变化连带重建 PostgreSQL，但未删除/替换 volume；随后 PostgreSQL healthy、OOM=false、mail/postflight 及完整稳定窗口均通过，后续 postflight 固定使用 `--no-deps`。候选 Debug APK 已基于当前 HEAD 重建并完成版本、签名、ABI、SHA-256 与包体资源复核。

- [x] 确认 02-06 无未解决的公网/真机前置阻塞，记录本次验收 commit 列表和累计 diff 范围。（验证：02-06 归档清单未完成项均为 0；范围 `0673c6baa53bfb6a397d90b0df5943f5c233b776..4368ed1a75ed6e2dca2a63f5587812da6a444edb`，37 commits、48 files、`+12205/-429`。）
- [x] 部署与 APK 协议兼容的 auth、game server/proxy、chat、mail 和 Caddy 版本，保留部署记录与回滚点。（验证：生产 current 为 `v0.1.0-fef5c9b49d36`，13 个常规容器运行且核心依赖 healthy，Caddy validate 通过；回滚点 `v0.1.0-d95f6b666e4e` 及更早 release 保留，本阶段无需重新部署。）
- [x] 在非真机客户端先验证公网 HTTPS、KCP/TCP、WSS 和 mail HTTPS 健康，确认 descriptor 不返回内部 endpoint。（验证：API health/TLS 200、mail 无票 401、WSS Upgrade 101、game/chat 内部 readiness 200；公开 descriptor 为 `game.zergzerg.cn:4000/kcp`、`chat.game.zergzerg.cn:443/wss`、`api.game.zergzerg.cn:443/https`，生产未公开 TCP fallback。）
- [x] 构建候选 APK，记录 Git SHA、versionCode/versionName、ABI、签名指纹、文件大小和 SHA-256。（验证：`app-debug.apk` 基于 `4368ed1` 于 2026-08-07 16:08:40 +08:00 构建；`com.mybevy.project`、versionCode 1、versionName 1.0、仅 arm64-v8a、151501057 bytes、APK SHA-256 `3C3D89271923B3B01D48BF9AB4F8C4334571CD464FB5BA9136AE3EA431AFF706`、v2 签名证书 SHA-256 `17E2E5FFBE7C7C9DEB3224051EF384614E35A98A5A2796137C3B0BC66AD977FA`。）
- [x] 检查 APK 包含 approved UI documents、场景 catalog/manifest/layout、字体、图片、音频和 arm64 `libproject.so`。（验证：source/APK assets 均为 255；approved documents 31、scene/layout 10、字体 4、图片 34、音频 19；arm64 `libproject.so` 143593864 bytes，SHA-256 `82EB74E4E7FF401271EB2A9943AABA617904F64D231D059844BBB4B3F0321C04`。）
- [x] 明确灰度开关、旧版兼容、最小协议和回滚操作，再批准真机安装。（验证：当前/最低/legacy implicit 均为协议 v1，06 阶段 8 已记录 chat/mail/UI/Caddy 灰度与回滚；生产 Redis 凭据轮换后旧凭据失效，13/13 容器、两次 postflight、公网门禁及 303 秒稳定观察通过，准许进入真机安装。）

## 阶段 2：设备矩阵、安装和启动

- 开始时间：2026-08-10 10:01:19 +08:00
- 结束时间：
- 开发总结：进行中。候选 Debug APK 已在授权真机上完成覆盖安装和冷启动；登录页实际渲染且 Activity/进程保持前台。启动日志同时发现 `bevy_gilrs` 在 Android 以 ERROR 级别报告平台不支持，以及 theme/i18n/audio manifest 的 fallback 警告，尚未作为候选版本通过项。
- 验证记录：设备为 Xiaomi 2510ERA8BC，Android 16 / API 36，arm64-v8a，物理分辨率 1280x2772、density 520（3.25），横屏后 2772x1280，左侧挖孔安全区 152 px，手势导航（`navigation_mode=2`）。`adb install -r` 成功，包 `com.mybevy.project` 为 versionCode 1 / versionName 1.0 / v2 签名，最后更新时间 2026-08-10 10:01:19 +08:00；冷启动 TotalTime 1432 ms，登录页截图正常，scene catalog 5/5 loaded，未见 panic、FATAL 或 ANR。初始内存约 332101 KB PSS / 483772 KB RSS；截图和诊断保存在被忽略的 `summary/android-debug/2026-08-10-public-device/`。

- [x] 记录至少一台授权真机的型号、Android API、ABI、分辨率、density、刘海/挖孔和导航模式。（验证：Xiaomi 2510ERA8BC，Android 16/API 36，arm64-v8a，1280x2772、density 520；横屏左侧挖孔安全区 152 px，手势导航。）
- [ ] 通过 ADB 安装候选 APK，确认包名、版本、签名、首次安装和覆盖安装行为正确。
- [ ] 冷启动到登录页，确认 Activity/进程前台、无 panic/FATAL/ANR，首包 catalog 与 approved documents 加载成功。
- [ ] 验证横屏、安全区、状态栏、刘海/挖孔、手势导航与三键导航不遮挡内容。
- [ ] 采集启动日志、首帧截图、启动耗时、内存和应用版本证据。

## 阶段 3：公网登录、角色与主世界

- 开始时间：2026-08-10 14:55:06 +08:00
- 结束时间：
- 开发总结：进行中，当前被 Android 真机 Lobby 的 Main World `Enter` 物理点击失效阻塞。已从全新应用数据完成正式服游客登录、角色创建、character-bound ticket 获取和公网 KCP 鉴权；不使用 ADB 注入绕过真机触控失败，主世界进场及后续验收保持未完成。
- 验证记录：本轮设备为 Xiaomi 2510ERA8BC，Android 16 / API 36；候选 APK SHA-256 为 `5F5DF4ECAE44BBAF7D0572796A626B9C76517EED4D121386E52F5ADDBABD9B8B`。覆盖安装并清除应用数据后冷启动成功，正式 Production 游客登录进入空角色页；创建本轮临时非敏感角色后，客户端仅记录脱敏 `access_token_fp` / `ticket_fp`，通过 `game.zergzerg.cn:4000/KCP` 完成 character-bound ticket 鉴权并在 Lobby 显示 `Game session connected / ready`。用户在真机上直接点击 Main World 卡片的 `Enter` 无响应，且确认同一入口在 Windows `cargo run` 下正常；点击后 60 秒内进程与 Activity 保持前台，但日志中没有 `StartRoom`、`main-world-public`、authority snapshot、Scene Ready、Room Ready 或 `main world entry active`。因此不宣称主世界已进入，也不继续视觉、HUD 输入或返回 Lobby 复验。新截图与日志保存在被忽略的 `summary/android-debug/2026-08-10-public-device-stage3-retest/`，未保存完整 token、ticket、密码、邮件或聊天正文。

- [ ] 通过正式 HTTPS 完成注册/登录、审核状态、角色创建/选择和 character-bound ticket 获取。
- [ ] 通过公网 game KCP 进入 `main-world-public / movement_demo`，确认 scene 1 快照、Scene/Room ready 和 Active。
- [ ] 确认主世界平地、球体、相机、光照、Loading 和 HUD 可见且无重叠、黑屏或白屏。
- [ ] 验证主世界触控不被 HUD 空白区域阻断，按钮区域不泄漏 gameplay 输入。
- [ ] 返回 Lobby 再进入主世界，确认房间、SceneOwned、HUD、Loading 和输入无重复或残留。

## 阶段 4：HUD、面板与系统输入

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 反复打开关闭设置和邮箱，确认仍在同一主世界房间/scene session，gameplay 输入正确阻断与恢复。
- [ ] 验证触控按下/拖动/释放、滑动、slider、toggle、输入框、软键盘和焦点可见性。
- [ ] 验证 Android 系统返回键按 CloseTop 关闭 Dropdown/Modal/Floating/面板，不越级退出主世界或应用。
- [ ] 验证长邮件标题、错误文案、未读角标和设置控件在真机分辨率/density 下无溢出、遮挡或布局跳动。
- [ ] 验证家园与返回 Lobby 快速重复点击只产生一个受控意图。

## 阶段 5：聊天 WSS 与邮件 HTTPS

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 确认 auth descriptor 下发正式 `wss` chat 和 `https` mail endpoint，真机不发起内部 host/port 连接。
- [ ] 完成 ChatAuth，验证鉴权前门禁、业务 Ready、push 和断线重连，ticket 不出现在 URL/query/日志。
- [ ] 打开邮箱并通过公网 HTTPS 完成列表、详情、已读、领取和结果未知 reconciliation。
- [ ] 验证 MailNotifyPush 只触发权威列表刷新，重复/乱序/丢失通知不导致重复发放或错误未读数。
- [ ] 验证关闭邮箱、切场景或通知到达时，全局 MailClientState 不泄漏旧身份且必要 reconciliation 可完成。

## 阶段 6：前后台、旋转与网络切换

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 在登录、Lobby、主世界和邮箱面板分别执行前台 -> 后台 -> 前台，无 crash/ANR，UI、Scene 和身份状态可恢复。
- [ ] 后台期间暂停/关闭 chat，恢复后按最新 ticket/endpoint 重建，不发送过期积压请求。
- [ ] 在 Wi-Fi、蜂窝网络和无网络之间切换，验证 game KCP/TCP、chat WSS 和 mail HTTPS 的冻结、超时、重连与错误状态。
- [ ] 主世界断网后冻结输入，网络恢复后重鉴权、RoomReconnect、recovery snapshot/ready 恢复 Active。
- [ ] 进程被系统回收后重启，不伪造旧房间、邮件领取或聊天已恢复，并进入可说明的重新登录/恢复路径。

## 阶段 7：视觉、性能和稳定性

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 采集登录、Lobby、主世界、HUD、设置和邮箱关键截图，复核字体、图标、透明边缘、九宫格、材质降级、安全区和文字溢出。
- [ ] 记录稳定主世界、打开面板、场景切换和前后台过程的 FPS/frame time、ECS/SceneOwned 实体数、RSS/PSS/Private Dirty 和 GPU 相关可用指标。
- [ ] 连续进出主世界、家园、设置和邮箱，确认实体、内存、连接、pending request 和 document instance 无持续增长。
- [ ] 执行至少 30 分钟前台稳定性和多次前后台/网络切换演练，无 crash、ANR、白屏或无法恢复连接。
- [ ] 对比桌面预算与真机结果，记录设备特有降级、驱动差异和剩余风险。

## 阶段 8：负向安全、回滚与报告归档

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 在获授权的测试环境验证无 ticket、过期/撤销 ticket、ownership/version 不匹配和内部 endpoint 均被拒绝。
- [ ] 验证 chat 公网入口拒绝错误协议、明文 `ws`、内部 `9001/9011` 直连和携带无效 ticket 的 WSS 鉴权；不以本地 dev-stack 的显式 `ws` 结果替代。
- [ ] 检查 `dumpsys package`、网络日志和抓包元数据，确认无未批准明文流量、内部 host/port 或敏感凭证泄漏。
- [ ] 演练 chat/mail descriptor 置 null、功能暂停和服务端版本回滚，确认客户端受控降级且核心主链路可恢复。
- [ ] 汇总设备、APK、公网部署、功能、视觉、性能、网络、安全、回滚和剩余风险证据。
- [ ] 为每个失败项记录复现条件、归属清单、修复 commit、重新部署版本和重验结果。
- [ ] 验收通过后将本清单与证据按仓库约定归档到 `docs/<领域>/checklists/`。

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由全部真机阶段完成后统一验收。

- 开始时间：
- 结束时间：
- 验收总结：

- [ ] 目标 APK 与公网服务版本可追溯、协议兼容且可回滚。
- [ ] 真机可通过公网完成登录/选角、主世界、HUD/面板、chat WSS、mail HTTPS 和退出 Lobby 完整路径。
- [ ] 安全区、触控、软键盘、系统返回键、前后台、网络切换和进程重启均有确定结果。
- [ ] 主世界视觉、页面排版、帧率、内存、实体和连接生命周期在目标设备上通过或有已接受的明确风险。
- [ ] 真机只访问批准公网入口，身份、邮件、聊天和日志安全边界通过验收。
- [ ] 失败、修复、重新部署、重验、灰度与回滚证据完整归档。
