# UI 声明式业务界面迁移基线

本文冻结 `UiDocument` 成为正式业务 View 与用户端 UI 内容更新载体前的架构边界、页面盘点和验收口径。它记录 2026-07-30 以来的实现基线和后续阶段约束；已实现的受信远端更新能力与尚未实现的业务宿主迁移明确分开记录。

相关协议和已实现的桌面开发预览见 [UI声明式文档协议.md](UI声明式文档协议.md) 与 [UI声明式预览与热更新.md](UI声明式预览与热更新.md)。资源首包和通用内容缓存约定见 [../assets-workflow.md](../assets-workflow.md)。

## 1. 冻结的职责和依赖

| 层或概念 | 职责 | 可依赖 | 禁止依赖或职责 |
| --- | --- | --- | --- |
| `UiDocument View` | 只描述稳定 node ID、结构、布局、主题/资源引用、响应式变体、允许的表现状态和受限控件声明；所有来源均先经 schema、语义、权限、路径和预算验证。 | `framework/ui` 的公开协议、已注册资源/token、由宿主注入的 typed binding/action ID。 | Rust type/system/message/function、URL、真实文件系统路径、脚本、任意命令、网络访问、路由决策、权限决策和玩法状态机。 |
| Game Host（后续 `DeclarativeScreenHost` 或等价物） | 游戏层显式注册 document ID、route、owner、panel/layer、生命周期、action allowlist、binding schema、audit profile 与 fallback；把 framework 通用 dispatch 映射为游戏命令。 | `framework/ui/document`、`game/navigation`、业务资源/消息。 | 反射或执行 document 指定的 Rust 名称；自行建立第二套 panel/input/focus 管理；依赖 AI 工具 crate。 |
| `UiUpdateBundle`（immutable generation 数据包） | 一个 immutable generation 的 manifest、document、approved registration、资源引用、版本/兼容信息、hash、授权与预算声明；安装前完整验证。签名由外层 release envelope 提供。 | 已批准 document/asset 产物和游戏二进制内的 host contract。 | 业务 Rust、可执行代码、未批准 draft、任意 URL/path、权限扩大字段。 |
| `UiUpdateClient`（远端客户端能力） | 通过固定受信 endpoint 拉取已签名 manifest 和文件；以 ETag、限流、有限重试、临时文件续传和安全点驱动 `UiUpdateCache` staging/activation，保留 current/previous generation。 | `UiUpdateBundle`、平台私有缓存、现有 network HTTP、二进制 trust roots 和 `UiDocumentRuntime` 的受控 reload 入口。 | 绕过 validation/host contract；在主线程阻塞下载；就地覆盖 active generation；让远端包指定 endpoint、信任根、宿主或业务动作。 |
| AI authoring / promotion | `tools/ui-generation/` 在独立 Cargo 根生成不可信 draft、证据和 promotion plan；人工批准的 `promote` 才能写 approved JSON、授权资源、catalog 与 closed registration。 | `project::framework::ui::document::tooling` 单向 facade、受限仓库资源。 | 进入正式游戏依赖图、生成/改写业务 Rust、自动发布给用户、直接获得 action/binding 权限。 |

目标依赖方向固定如下；箭头表示允许的依赖或数据交付：

```text
AI authoring/promotion -> approved UiDocument/resources -> UiUpdateBundle -> UiUpdateClient
                                                           |                     |
                                                           v                     v
                                                    Game Host ------------> UiDocument Runtime
                                                           |
                                                           v
                                                  game route / typed commands
```

`project/src/framework/ui/document/update.rs` 已实现版本化 `UiUpdateBundle` 与 `UiUpdateCache` 的本地事务闭环：调用方只能把完整 import 写入应用私有缓存的 `staging`，通过 manifest/hash/schema/budget、approved registration/host contract、资源 metadata 和授权 metadata 后，immutable generation 才会写入 `active` 提交记录；损坏 active 会回退到上一个有效提交。`remote.rs` 的 `UiUpdateClient` 使用已有 network HTTP event 读取固定 endpoint，验签 Ed25519 release envelope、拒绝未知/撤销 key、客户端不兼容与未授权 downgrade，并在下载完成后仍复用 cache 的完整验证。它拒绝项目目录作为缓存根，且 document 只以 logical asset ID 经 verified `content_cache://<generation>/...` catalog 获取 typed handle。UI framework 只注册不带配置的 plugin；实际游戏发布仍须在 `AssetPlugin` 前提供平台私有 cache root 与二进制 trust roots，首包 approved 页面始终是 fallback。正式 approved adapter 采用版本化、默认拒绝的 host contract：legacy registration 仍只允许展示字段；v2 只有与游戏层预注册的 document/owner/route、binding schema、action ID/source node、资源清单和 audit profile 精确一致时才可引用既有业务能力，见 `project/src/framework/ui/document/approval.rs`。它不接受 Rust 类型、handler/system/message、URL、真实文件路径或执行字符串。

## 2. 资源改动与 Rust 改动判定

满足左列且不改变已批准 document 的宿主契约时，只能修改 JSON、主题/i18n、图片/字体授权资料或更新包元数据。任何右列情况都必须先修改 Rust contract、测试和文档，不能以 document 字段偷渡。

| 变更 | 只改资源 | 必须改 Rust |
| --- | --- | --- |
| 布局、层级、间距、颜色、视觉资源、文案 | 已有 schema、node、token、asset ID 和预算范围内。 | 需要新的 node、布局/效果语义、asset source 或预算规则。 |
| 响应式 | 已有 `responsive` patch 与 profile 条件可表达。 | 新 profile selector、patch 字段或合并优先级。 |
| 已有动作的重排 | 只复用同 document/owner/source node 已注册的 action，参数 shape 不变。 | 新 action ID、允许 source、target、权限、去重或参数 schema。 |
| 绑定的展示位置/格式 | 只复用已注册 binding，类型、scope、format 和只读/回写规则不变。 | 新 binding path、类型、scope、写回能力、loading/error 行为或业务状态。 |
| 新控件 | 用当前 runtime 已支持的控件及既有公共 interaction。 | 新控件种类、事件、输入语义、无障碍、focus 或资源预检。 |
| 动态列表 | 不适用：当前 schema/runtime 没有受限 Repeat/collection 协议。 | 增加稳定 key、item scope、模板预算、keyed diff 和状态迁移。 |
| 手势/输入 | 只使用已有 click、文本输入、scroll、选择/数值控件的已注册语义。 | 拖拽、长按、双指、gameplay 竞争输入或新的 IME/gesture 语义。 |
| 业务状态 | 已有宿主 binding 的纯表现更新。 | 新网络数据、权限、路由、场景/authority/MyServer 命令、持久化或业务状态机。 |

UI-only diff 门禁的判定输入是改动后的 Git 路径，而不是提交说明：只要 diff 含 `.rs`、`Cargo.toml`、`Cargo.lock`、Android 代码或授权以外的脚本，即不是 UI-only；只含已批准 JSON、`project/assets/ui/` 受管资源、主题、i18n、资源 catalog/manifest、授权说明和相应审计 artifact 时才可候选通过。门禁本身尚未实现，当前基线结果为“不可判定/未通过”，不得把人工判断当作自动化证明。

## 3. 页面盘点

盘点范围是 `project/src/game/screens/` 下、由 `AppUiMode` 路由的全部 15 个 screen。所有 owner 常量和 Rust View 的 conventional panel 常量定义在 `project/src/game/ui_ids.rs`；两个声明式页面使用 registration 中的 document panel/layer，而没有对应的 `UiPanelId` 常量。所有 screen 已由 `project/src/game/navigation/mod.rs` 注册到 `UiAuditScreenRegistry`。

审计 recipe 缩写：`A0` 为该 screen 的默认 route capture；`A1` 为 document owner-ready 的 initial capture 和默认 reference recipe；`A2` 为 `ui_gallery` 的多状态、scroll 与 anchor capture。`A0` 并不等于已经完成目标设备人工验收。

| Screen / 分类 | owner / panel 或 document layer | 当前 Rust View | action / binding / 动态列表 | 特殊手势或输入 | 直接资源 | 审计 recipe |
| --- | --- | --- | --- | --- | --- | --- |
| `login`，普通业务待迁移 | `login` / `login_page` | `auth/login.rs::setup_login_screen` | 登录、游客登录、环境切换、加载角色；`auth.login.subtitle`；无列表。 | 两个文本输入、password、分段环境选择。 | `ui/images/login_stillwater_background.png`，主题/i18n/font。 | A0 |
| `character_select`，普通业务待迁移 | `character_select` / `character_select_page` | `auth/login.rs::setup_character_select_screen` | 加载/创建/选择角色、切换账号；`auth.character.subtitle`；当前 Rust 逐行生成角色列表。 | 角色名文本输入、可变数量角色行。 | 主题/i18n/font。 | A0 |
| `lobby`，普通业务待迁移 | `lobby` / `game_list_page` | `lobby/game_list.rs::setup_game_list_screen` | 启动 Touch Ripple、场景切换、选角、退出账号、确认 modal；无 binding；固定玩法入口集合。 | 按钮、confirm modal。 | 主题/i18n/font。 | A0 |
| `audio_settings`，普通业务待迁移 | `audio_settings` / `audio_settings_page` | `settings/audio.rs::setup_audio_settings` | 各音频 bus 音量、master mute、回大厅；无 document binding；按固定 audio bus 生成控制行。 | Slider、toggle。 | 主题/i18n/font。 | A0 |
| `wanfa_touch_ripple`，玩法 HUD 待迁移 | `wanfa_touch_ripple` / `touch_ripple_hud` | `gameplay/touch_ripple.rs::setup_touch_ripple_overlay` | 仅路由回大厅；无 binding/list。 | 玩法触控/鼠标输入在 feature 层；HUD 本身仅按钮。 | 主题/i18n/font。 | A0 |
| `sample_scene`，玩法 HUD 待迁移 | `sample_scene` / `sample_scene_hud` | `gameplay/sample_scene.rs::setup_sample_scene_hud` | 请求 scene exit 后回大厅；无 binding/list。 | HUD 按钮；场景退出事件回退路由。 | 主题/i18n/font。 | A0 |
| `robot_sync_scene`，玩法 HUD 待迁移 | `robot_sync_scene` / `robot_sync_scene_hud` | `gameplay/robot_sync_scene.rs::setup_robot_sync_scene_hud` | authority leave、scene exit、回大厅；实时 authority/session 状态文本；无列表。 | HUD 按钮；authority/scene 生命周期。 | 主题/i18n/font。 | A0 |
| `fangyuan_home`，玩法 HUD 待迁移 | `fangyuan_home` / `fangyuan_home_hud` | `gameplay/fangyuan_home.rs::setup_fangyuan_home_hud` | blueprint reload/clear/trial audit/budget、debug module、scene exit；Fangyuan 状态/调试文本；固定 debug module 行。 | HUD 按钮；scene lifecycle。 | 方圆首包 palette/layout 由场景层加载；UI 使用主题/i18n/font。 | A0 |
| `fangyuan_player_preview`，玩法 HUD 待迁移 | `fangyuan_player_preview` / `fangyuan_player_preview_hud` | `gameplay/fangyuan_player_preview.rs::setup_fangyuan_player_preview` | 回大厅；无 binding/list。 | HUD 按钮；同一 setup 还生成 3D camera/light。 | 主题/i18n/font；camera/light 非 UI resource。 | A0 |
| `ui_gallery`，开发工具页 | `ui_gallery` / `ui_gallery_page` | `dev/ui_gallery.rs::setup_ui_gallery` | Toast、Loading、Confirm、Floating、Dropdown/Tooltip、binding preview；`UiBindingValues`；固定展示样例。 | Scroll/anchor、TextInput、各通用控件和 audit state。 | Gallery 图片、atlas、图标、主题/i18n/font。 | A2 |
| `ui_document_gallery`，已声明式 | `ui_document_gallery` / document `gallery.declarative` page | `dev/ui_document_gallery.rs` 只注册/关闭 preview，View 是 `approved/gallery/declarative_gallery.v1.json`。 | 已注册 `gallery.set_status` local action 与 document binding；无动态列表。 | 文档中的标准控件。 | approved document assets、主题/i18n/font。 | A1 |
| `ui_generated_acceptance`，已声明式 | `generated_acceptance_approved` / document page | `dev/ui_generated_acceptance.rs` 通过 closed promotion registration 注册/关闭 View。 | approved adapter 当前要求 action/binding/i18n 为空；无列表。 | 标准 document interaction。 | `approved/generated_acceptance_fixture/` document/catalog/assets。 | A1 |
| `ai_login_reference`，开发工具页 | `ai_login_reference` / `ai_login_reference_page` | `dev/ai_login_reference.rs::setup_ai_login_reference` | 本地按钮视觉测试；无 binding/list。 | Down/Up/Cancel 的压下反馈和动画。 | `ui/images/ai_login_reference/` 下背景、sigil、panel surface。 | A0 |
| `audio_gallery`，开发工具页 | `audio_gallery` / `audio_gallery_page` | `dev/audio_gallery.rs::setup_audio_gallery` | 大量 `AudioCommand`（cue/music/instance/bus/spatial）、状态/诊断刷新；固定测试 catalog，不是 UI data list。 | 文本输入、按钮和音频空间辅助交互。 | 音频 catalog/首包音频、主题/i18n/font。 | A0 |
| `audio_monitor`，开发工具页 | `audio_monitor` / `audio_monitor_page` | `dev/audio_monitor.rs::setup_audio_monitor` | 只读 audio debug 快照；无 binding/list。 | 无特殊 gesture。 | 主题/i18n/font。 | A0 |

分类总数固定为：已声明式 2、普通业务待迁移 4、玩法 HUD 待迁移 5、开发工具页 4、已批准 Rust View 例外 0，共 15。前两份声明式页面均是开发/验收页面，正式业务页面当前为 0；不能把它们计入业务迁移完成数。

## 4. 受控 Rust View 例外

当前没有已批准的“永久保留 Rust View”例外。复杂不是例外理由：Fangyuan/authority/scene 的业务、3D camera/light、网络和特殊输入必须继续由 Rust owner 持有，但其 HUD 的纯 View 仍可在 host contract 完成后迁移为 document。

后续若确有不能迁移的 View，必须先在本表登记并经 UI/framework 与页面 owner 复审。空白或“实现复杂”不构成登记依据。

| exception ID | 原因 | owner | 影响范围 | 复审条件 | 状态 |
| --- | --- | --- | --- | --- | --- |
| 无 | 无已批准例外。 | - | - | 任一页面提出 host/schema/控件缺口时先评估是否应补框架能力。 | 不适用 |

## 5. 路由兼容策略

当前 `AppUiMode` 是 15 个固定 Rust enum variant，负责启动别名、owner 切换、panel/binding 清理和 audit route。现有 `UiApprovedDocumentRegistration.route` 只是 review label，approved adapter 不会从它路由页面。

后续把无业务逻辑的纯展示页接入数据注册时，可以使用通用 document route，**无需新增 `AppUiMode`**，前提是：

1. route registry 以稳定 route ID 映射固定 document ID、owner、panel/layer、audit profile 和 fallback，不从 JSON/manifest 采纳任意 route。
2. 该 route 没有独有 business state、权限、action、binding 或 scene lifecycle；出现任一项时先扩展 Game Host contract，必要时保留/新增显式 Rust route adapter。
3. router 仍必须执行 owner switch、`CloseAllForOwner`、binding 清理、input/focus 和 audit registry 语义；通用 route 只是减少空 View 的 enum/spawn boilerplate，不是绕过这些生命周期。
4. 保持现有 `AppUiMode` alias 兼容；迁移已有 mode 时先让其 adapter 调用相同 host，再在完整 audit/回滚验证后决定是否移除 variant。

## 6. 迁移完成指标

以下指标在每页迁移和最终验收时记录。分母采用本页 15 个 routable screen，正式业务分母采用 9（4 个普通业务 + 5 个玩法 HUD）。

| 指标 | 阶段 1 基线 | 完成定义 |
| --- | ---: | --- |
| 全部 routable screen | 15 | 清单与 `AppUiMode`/route registry 无遗漏。 |
| 正式业务 View 已声明式 | 0 / 9 | 9 / 9，或每个剩余项登记为受控、未过期的例外。 |
| 所有声明式 screen | 2 / 15 | 业务和展示页面都由 source registration/host 生命周期打开、关闭和审计。 |
| 直接 Rust View screen | 13 / 15 | 业务目标为 0；只剩受控例外和明确标记的开发工具页。统计按 route setup 是否直接构建 `Node`/widget 实体，而非 `commands.spawn` 的代码行数。 |
| 受控 Rust View 例外 | 0 | 每项有 owner、原因、影响和复审条件；例外总数不能因“复杂”增加。 |
| UI-only diff 门禁 | 未实现 | 对每个候选 UI-only 变更产出 pass/fail 与违规路径；业务 View 文案/布局/资源改动应通过且无 `.rs` 改动。 |
| 宿主契约覆盖 | 当前业务 action/binding 为 0 | 每份业务 document 的 document/owner/source node/action param/binding type/audit profile 都由游戏层闭合注册并有拒绝测试。 |
| 更新可靠性 | 不适用，尚无生产 client | Release 和 Android 均证明：完整验证后原子切换、失败保持旧 generation、缓存不可用回退首包 approved。 |

## 7. Schema 决策

本阶段只改文档，因此不升级 schema。当前 `CURRENT_SCHEMA_VERSION = 1`、`MIN_SUPPORTED_SCHEMA_VERSION = 1`，canonical JSON、JSON Schema golden 和 `mobile_baseline_v1` 继续是唯一当前写出格式。`UiDocument` v1 当前只有版本范围校验和稳定拒绝；仓库尚无 v0 -> v1 或相邻版本的执行迁移链，因此不能把 parser 的直接反序列化称为 migration。

未来的 host lifecycle、route registry、allowlist 和 `UiUpdateBundle` manifest 不改变 document 本身语义时不升级 document schema。任何新增运行时 document 字段、控件事件/参数、binding value/scope、Repeat/collection、响应式 selector/合并规则或默认值变化都必须创建下一个 schema version，并在实现同一变更时提供：

- 确定性的 `vN -> vN+1` 纯数据迁移，不读取网络、文件、环境、时间或随机数；
- 新版本 canonical JSON 与 golden、最低支持版本和迁移报告；
- 未知未来版本、无迁移链和迁移后验证失败的稳定拒绝路径；
- 把首包 approved 重新 canonicalize/验证，不静默原地覆盖源文件。

在目标 version 和真实字段确定前，不预写臆测的 v1 -> v2 字段重命名规则，避免文档成为与实现不同步的伪协议。

## 8. Reload 与生产更新术语

| 术语 | 当前状态 | 适用环境 | 不代表什么 |
| --- | --- | --- | --- |
| 开发期 preview/reload | 已实现：安全 source root、显式 reload、事务 replace、状态迁移报告。 | 桌面 Debug；watch 还需 `MYBEVY_UI_DOCUMENT_WATCH=1`。 | 不是网络分发、签名、缓存 generation 或生产热更新。 |
| 开发期 file watch | 已实现但默认关闭；Release/Android 强制关闭。 | desktop Debug。 | 不是用户端热更新，也不能通过运行时命令绕过平台门。 |
| 生产用户端 UI 热更新 | 已实现为显式配置的 `UiUpdateClient`：固定 endpoint、签名、下载约束、cache generation 与安全点激活。 | 可由桌面 Release、Android 配置启用。 | 不能把直接 HTTPS asset loading 或本地文件 watch 称为完成端到端发布。 |
| approved 首包 fallback | 已有 approved document 与 closed registration 样例；remote/cache 失败或没有 active generation 时仍由宿主保留首包。 | 桌面/Android 包内。 | 不代表默认游戏已绑定真实 cache root、公钥或远端检查时机。 |

当前默认应用尚未创建平台私有 cache root、注册实际 `content_cache` Bevy asset source、提供生产公钥或向游戏 route 安装 active generation，因此 Release/Android 仍只加载首包 approved 文档；现有 Rust View 仍照常由游戏路由创建。`UiUpdateCache` 已覆盖缓存损坏到 previous generation 的本地恢复；远端不可用、签名失败与版本发布策略也会保留 current generation 或回退首包，不会阻断登录/核心玩法。

## 9. 阶段 1 审计方法

- 页面和 owner/panel：比对 `project/src/game/navigation/mod.rs` 的 `AppUiMode`、`project/src/game/ui_ids.rs` 和本页 15 行。
- Rust View/action/binding：检查 `project/src/game/screens/` 相应 setup/handler 与 `UiBindingValues` 使用处。
- 声明式基线：检查 `project/src/game/screens/dev/ui_document_gallery.rs`、`ui_generated_acceptance.rs`、`project/assets/ui/documents/approved/` 与 `approval.rs` 的 closed-field rejection。
- 生产更新边界：检查 `project/src/framework/ui/document/update.rs`、`remote.rs`、`project/src/framework/network/` 和 Android 工程；远端 provider、签名、下载与 cache activation 已有，默认应用/Android 仍须显式提供私有 cache root、生产 trust roots、检查触发点和真机发布验收。
- 本阶段只允许本文及现有 UI 架构/限制/索引文档变更；提交前由主流程运行 `git diff --check` 并确认没有 Rust、资源、Cargo、Android 或 `summary/` 改动。
