# UI 声明式文档协议

本文定义 `UiDocument` 的协议边界、版本兼容、安全模型、目录约定和首版资源预算。它是后续 Rust 数据模型、JSON Schema、验证器和 ECS 构建器的规范输入。阶段 1 只冻结协议决策和最小 fixture，不代表运行时已经能够加载声明式页面。

## 1. 目标与职责

`UiDocument` 是页面结构的版本化中间表示。它负责描述以下白名单信息：

- 页面 metadata、稳定的 document ID 和 node ID。
- 受支持的布局、样式 token、资源引用和控件变体。
- 文本、图片、控件内容以及类型化 binding。
- 响应式条件、页面状态和节点 override。
- 对游戏层已注册 action ID 的引用和受限参数。
- 供校验报告、预览、截图 audit 和 AI 修复定位使用的源路径。

它的主要使用流程是：

```text
AI / 人工编辑
  -> JSON 或受控 RON
  -> 完整解析
  -> 版本判定和必要迁移
  -> 结构、引用、能力、安全和预算校验
  -> 解析 responsive/state 后的 effective document
  -> 事务式 ECS 构建
  -> audit metadata / validation report
```

同一份 document、资源 revision、theme、locale、binding snapshot、页面状态和 viewport profile 必须得到相同的 effective document 和稳定节点树。协议不能读取时间、随机数、任意环境变量或未声明的平台信息改变结果。

## 2. 非目标

`UiDocument` 首版不承担以下职责：

- 不生成、编译或加载任意 Rust 类型、Rust 函数、Bevy system 或反射组件名。
- 不包含脚本、表达式语言、shell、进程、动态库或 shader 源码。
- 不直接发起 HTTP、TCP、KCP、文件下载或其他网络操作。
- 不读取任意文件、绝对路径、父目录、UNC 路径或用户目录。
- 不实现登录、支付、匹配、战斗、背包等业务规则和权限判定。
- 不替代 `UiFrameworkPlugin`、widgets、Panel Manager、i18n、binding、focus 或 audit。
- 不序列化 widgets 的内部 ECS 子树，也不承诺其内部实体结构稳定。
- 不识别参考图，不调用 AI 服务，不把任意 Rust 代码作为页面生成结果。

复杂业务页面可以继续使用 Rust 实现。声明式文档不是强制替代所有人工页面的机制。

## 3. 与 framework 和 game 层的协作

职责边界如下：

| 所属层 | 负责内容 | 禁止内容 |
| --- | --- | --- |
| `framework/ui` | 协议类型、解析、迁移、白名单校验、effective document、事务式构建、稳定 node marker、诊断和 audit metadata | 具体游戏路由、账号/角色规则、业务权限和业务命令实现 |
| `framework/ui/widgets` | 将声明式控件映射到稳定公共 helper 和统一交互事件 | 为文档复制一套控件内部结构或交互系统 |
| `game` | 注册 document ID、owner、panel、route 和 action ID；实现业务命令；决定页面进入/退出和权限 | 允许文档通过字符串指定任意 Rust 类型、system 或消息 |
| Rust 业务页面 | 承载暂未进入白名单的复杂 UI，并可嵌入或打开声明式 panel | 绕过同一 Panel Manager、input、focus 和 audit 规则 |

游戏层必须先向 action registry 注册允许的 action ID、参数 schema、owner 范围和处理器，document 才能引用它。示例中的 `example.continue` 只是注册表键，不是函数名、消息类型名或自动获得执行权的命令。未知 action、参数类型不符、owner 不符或当前权限不允许时均拒绝加载或执行。

页面路由和生命周期仍由 `project/src/game/screens/` 与 `project/src/game/navigation/` 负责。声明式页面根必须进入现有 Panel Manager 和 layer 体系，关闭 owner 时与 Rust 页面使用相同清理语义。

## 4. 可信边界和处理顺序

所有 document bytes 都是不可信输入，包括随 APK 打包的文件、人工编辑的 RON、AI 生成的 JSON、开发期 hot reload 文件和已下载缓存。`approved` 只表示内容经过发布流程批准，不表示可以跳过运行时校验。

运行时必须按固定顺序处理：

1. 在字节预算内读取，不跟随符号链接逃离允许根目录。
2. 按调用方明确指定的格式解析；不根据内容猜测格式。
3. 读取 schema version，并执行本页第 6 节的兼容判定。
4. 如需要迁移，在内存中逐版本迁移为当前模型，保留迁移报告。
5. 校验结构、稳定 ID、引用、能力白名单、资源路径和预算。
6. 校验 action 参数类型、target node、owner 和权限范围。
7. 根据受限 viewport/profile 和页面状态生成 effective document。
8. 预解析资源并再次检查资源类型和来源。
9. 全部成功后才事务式生成 ECS 实体；任一步失败都不能留下可见的半棵树。

解析器必须拒绝未知字段，只有 metadata 中未来明确保留的 inert annotation map 可以作为扩展点。annotation 不能参与布局、样式、binding、action、资源解析或条件判断。

## 5. 交换格式

### 5.1 JSON

JSON 是 AI structured output、工具进程、网络传输、validation report 和自动修复 patch 的唯一交换格式。文件使用 UTF-8，不带 BOM。字段名使用 `snake_case`。禁止注释、尾逗号、`NaN`、`Infinity` 和重复 object key。

后续阶段必须提供与 Rust 类型一致的 JSON Schema 和 canonical JSON 规则。AI 输出必须先通过同一 schema 和运行时验证器，不能因 structured output 已通过服务端约束而跳过客户端校验。

### 5.2 RON

RON 只允许用于开发者人工维护且进入首包批准目录的资源。RON 必须复用与 JSON 相同的 Serde 数据模型、默认值、版本规则、校验器、预算和 action/resource 白名单，不能拥有 JSON 没有的执行能力。

以下入口不接受 RON：

- AI 模型输出和自动修复结果。
- 网络 API、剪贴板导入和跨进程交换。
- 未批准 draft 和任意外部下载文件。

当前不提供与 JSON fixture 重复的 RON 示例，避免模型尚未落地时两份手写表示发生漂移。后续支持 RON loader 时，必须由测试使用同一 `UiDocument` 在 JSON/RON 间 round-trip，并校验生成相同的 canonical JSON。

## 6. Schema version 与兼容策略

### 6.1 版本字段

- 字段名固定为 `schema_version`，值为正整数，不使用 semver 字符串。
- 当前写出版本 `CURRENT_SCHEMA_VERSION = 1`。
- 当前最低支持版本 `MIN_SUPPORTED_SCHEMA_VERSION = 1`。
- serializer 和迁移器只写当前版本，不继续生成旧版本。
- 缺失、零、负数、浮点数或字符串版本均拒绝，code 为 `UI_SCHEMA_VERSION_INVALID`。

兼容结果必须使用机器可读 code，不根据错误文案分支：

| 兼容等级 | 结果 code | 行为 |
| --- | --- | --- |
| 无损读取 | `UI_SCHEMA_LOSSLESS_READ` | 当前 decoder 可在不改变 document 语义的情况下读取；继续完整校验 |
| 需要迁移 | `UI_SCHEMA_MIGRATION_REQUIRED` | 存在完整的逐版本迁移链；只在内存或新文件中迁移，生成迁移报告后继续校验 |
| 拒绝加载 | `UI_SCHEMA_REJECTED` | 不解析为运行时 document，不构建实体；同时提供具体 reason code |

拒绝加载的 reason code：

| 条件 | reason code |
| --- | --- |
| `schema_version > CURRENT_SCHEMA_VERSION` | `UI_SCHEMA_FUTURE_VERSION` |
| `schema_version < MIN_SUPPORTED_SCHEMA_VERSION` | `UI_SCHEMA_VERSION_UNSUPPORTED` |
| 版本格式非法或缺失 | `UI_SCHEMA_VERSION_INVALID` |
| 需要迁移但迁移链不完整 | `UI_SCHEMA_MIGRATION_UNAVAILABLE` |
| 迁移后无法通过当前结构或语义校验 | `UI_SCHEMA_MIGRATION_FAILED` |

未知未来版本必须拒绝，不能用当前结构做 best-effort 解析。低于最低版本的 document 即使字段看起来可读也必须拒绝。

### 6.2 兼容变更

同一 schema version 内只允许不改变 canonical 语义的实现修复和文档澄清。以下变化必须增加 schema version：

- 新增、删除或重命名任何参与运行时行为的字段或 enum variant。
- 修改字段默认值、合并顺序、响应式优先级或 action 语义。
- 改变 ID、路径、颜色、数值或预算的合法范围，并可能影响既有 document。
- 改变节点到 widgets 或 Bevy UI 的可观察映射。

纯新增可选 metadata annotation 若不参与任何运行时行为，可以不增加版本，但必须由 JSON Schema 明确限制 key、value 类型和总字节数。

### 6.3 废弃字段和迁移

- 字段进入废弃期时，decoder 在其仍受支持的版本内读取它，并报告 warning code `UI_SCHEMA_DEPRECATED_FIELD` 和字段路径。
- 废弃字段不能静默改变含义，也不能被未知新字段覆盖。
- 删除字段时必须增加 schema version，并提供确定性的 `N -> N + 1` 迁移或把旧版本移出最低支持范围。
- 迁移是纯数据转换，不读取网络、文件、环境变量、时间或随机数，不执行 action。
- 多版本迁移只允许顺序组合相邻迁移，不能直接猜测跨版本转换。
- 迁移输出必须再次经过当前版本的全部校验。源文件默认只读，工具只有在显式 `--output` 指向允许的 authoring 目录时才能写新文件。
- 迁移报告至少包含 source version、target version、应用步骤、warning、被改字段路径和最终 canonical document hash。

## 7. 目录和资源来源边界

### 7.1 仓库目录

| 内容 | 目录 | 运行时可直接加载 |
| --- | --- | --- |
| 人工 authoring source | `project/ui-documents/source/` | 否，必须经过批准流程 |
| AI 或工具生成 draft | `artifacts/ui-documents/drafts/` | 否，始终按不可信草稿处理 |
| 批准的首包 document | `project/assets/ui/documents/approved/` | 是，但仍需完整校验 |
| 测试和协议 fixture | `project/assets/ui/documents/fixtures/` | 仅 dev/test build |
| effective document、报告和截图 metadata | `artifacts/ui-documents/runtime/` | 否，只是派生产物，不能反向当 source |

draft 不得通过重命名或复制自动升级为 approved。批准步骤至少要固定 schema version、canonical hash、资源清单、budget profile 和 action allowlist 审核结果。`artifacts/` 产物不进入 APK 首包。

首版仓库用 `project/assets/ui/documents/fixtures/minimal_page.v1.json` 保存协议 fixture。它用于后续 parser、schema、迁移和构建测试，不是已注册的正式业务页面。

### 7.2 Document 加载来源

生产 loader 只允许显式 source enum，不接受裸操作系统路径：

- `packaged`: 相对 `project/assets/` 的 `ui/documents/approved/...`。
- `content_cache`: 已通过内容 manifest、大小和 hash 校验的 `ui/documents/approved/...`；该 source 接入前保持禁用。
- `fixture`: 相对 `project/assets/` 的 `ui/documents/fixtures/...`，只在 dev/test build 启用。

生产构建默认不 watch 本地目录。开发期 watch 也只能 watch 由启动配置选择的 authoring 根，不能由 document 自己指定路径。

### 7.3 Asset table 路径

document 节点不能直接写图片、字体、图标、图集或材质路径。节点只能引用 document asset table 中的稳定 asset ID。asset entry 再选择受限 source 和规范化相对路径。

首包 asset path 相对 `project/assets/`，必须满足：

- UTF-8、正斜杠、小写路径；首版允许根为 `ui/`。
- 不得以 `/`、`\\`、盘符或 URI scheme 开头。
- 不得包含空 segment、`.`、`..`、NUL、反斜杠或 percent-encoded path separator。
- 规范化后仍位于允许根目录，且 asset kind 与扩展名、注册 manifest 一致。
- 不接受 `file://`、`http://`、`https://`、任意 hostname、任意 `content_cache://` 字符串或符号链接逃逸。

后续下载资源只能由宿主内容系统把受验证的 logical asset ID 解析到注册 source；document 无权提供 URL、缓存根或本地真实路径。

## 8. 安全模型

协议采用 deny-by-default 能力模型：

- node type、layout 字段、style role、token、asset kind、widget variant、binding type、condition 和 action kind 均为 closed enum 或注册表 ID。
- 未知字段、未知 enum、未知 token、未知 resource、未知 action 和类型不匹配全部产生稳定诊断。
- document 不能声明 Rust 类型名、组件路径、函数、system、消息类型、shader 路径、脚本、shell、命令行、网络地址或裸操作系统路径。资源只允许使用第 7.3 节 asset table 中经过规范化的 logical packaged path。
- action registry 由 game 层建立。每项注册固定 action ID、参数 schema、允许 owner、允许页面/节点范围和权限检查；framework 只分发通过校验的类型化请求。
- binding 只能读取 loader 注入给当前 document owner 的只读或显式可写 scope。不能通过 path 穿越到其他页面、ECS query、全局资源或账号安全字段。
- responsive 条件只允许框架提供的 width class、height class、orientation、safe-area class、input mode 和受限 platform enum。不能读取环境变量、设备标识、文件或任意 feature flag 字符串。
- validation report 对外只包含稳定 code、severity、document path、node ID、字段路径和安全的修复提示，不回显本机绝对路径、secret 或 Rust debug dump。
- 解析、迁移、校验和 effective merge 必须有工作量上限；畸形输入不能 panic、无限递归或无限累计错误。

首版不会把 document 签名当成绕过校验的凭据。若后续加入签名，签名只证明来源和完整性，仍需执行当前版本的全部安全与预算校验。

## 9. `mobile_baseline_v1` 资源预算

每份 v1 document 必须选择已注册 budget profile。最小 fixture 使用 `mobile_baseline_v1`。以下是该 profile 的硬上限，超限使用 `UI_BUDGET_EXCEEDED` 拒绝，不允许截断后继续构建：

| 项目 | 上限 |
| --- | ---: |
| 原始 document bytes | 256 KiB |
| document nodes | 512 |
| tree depth，root 计为 1 | 24 |
| 单节点直接 children | 128 |
| asset entries | 128 |
| token/style entries | 256 |
| action references | 64 |
| responsive variants | 32 |
| 全部 responsive overrides | 256 |
| 单个普通字符串 | 4 KiB UTF-8 |
| 单个 literal text | 16 KiB UTF-8 |
| action 参数 canonical JSON | 4 KiB |
| metadata annotations | 8 KiB canonical JSON |
| 单张图片声明宽或高 | 4096 px |
| 单张图片声明 decoded bytes | 16 MiB |
| 全 document 图片声明 decoded bytes | 64 MiB |
| 单次 validation diagnostics | 100 |

预算计数基于完整迁移后的 document，并在 responsive/state merge 前后各检查一次。document node 数不把 widget helper 的框架内部子实体算入协议节点数；runtime 仍应另外记录实际 ECS entity 数和视觉 primitive 预算。

图片预算使用 asset metadata 和解码后的尺寸计算，不信任 document 自报值。字体、图标、材质和效果必须来自 framework 注册表；document 不能内嵌二进制、base64、data URI 或 shader 参数 blob。阴影层、渐变 stop、图集 frame、平铺、材质和动画的细分上限由对应能力版本补充，但不得高于 `UI安全区与视觉预算.md` 的移动端策略。

budget profile 的收紧若会拒绝已批准 document，必须发布新 profile ID 或提升 schema/policy revision，不得在同一 ID 下静默改变结果。发布流程需要明确把 approved document 固定到 profile revision。

## 10. 最小 v1 fixture

规范 fixture 位于：

```text
project/assets/ui/documents/fixtures/minimal_page.v1.json
```

它覆盖以下最小路径：

- `page.root`: 页面根 Container。
- `page.title`: literal Text。
- `page.hero`: 通过 asset table 引用首包 Image。
- `page.continue`: 引用注册 action ID 的 Button。
- `compact_portrait`: Compact + Portrait 条件下修改根 padding/gap 和图片高度的 responsive override。

fixture 中的字段是 v1 核心模型的协议种子。后续 Rust 类型、JSON Schema 和 runtime 必须与它保持一致；如果实现阶段确认字段表达无法满足确定性或安全要求，应通过显式 schema version 和迁移调整，不能静默改写 fixture 含义。

最小示例不是“校验通过即可执行”。宿主还必须注册 document ID、owner/panel、`example.continue` action 和 fixture source，资源预解析成功后才能构建。

## 11. 阶段 1 评审记录

评审日期：2026-07-13

评审结论：协议边界和 v1 决策可以进入 Rust 数据模型设计，运行时实现尚未开始。

已核对项目：

- 职责、非目标、framework/game/Rust 页面边界已明确。
- JSON 与 RON 入口不重叠，且共用同一验证链。
- 当前/最低版本、未来版本、废弃字段、迁移和三种兼容结果有稳定 code。
- source、draft、approved、fixture、runtime artifact 和 asset path 已隔离。
- 示例覆盖页面根、文本、图片、按钮和一个响应式变体。
- 任意代码、脚本、shell、网络和绝对路径能力均未进入协议。
- `mobile_baseline_v1` 给出可执行的硬预算，超限行为为拒绝加载。

阶段 2 已继续确认 ID 字符格式、字段默认值、完整 JSON Schema、canonical JSON 字段顺序和合法/非法 fixture，详见第 12 节；这些落地项没有改变阶段 1 冻结的可信边界。

## 12. v1 核心模型冻结项

阶段 2 已将阶段 1 fixture 对应的核心 Rust 模型落在 `framework/ui/document/`。当前节点、布局、样式、token、state 和 responsive 类型是后续阶段扩展的 v1 种子，不代表阶段 3 至阶段 8 的完整能力已经实现。

### 12.1 稳定 ID

所有 ID 使用 ASCII 小写 snake_case segment，segment 之间使用 `.` 分隔，总长度为 1 至 128 bytes。每个 segment 必须以 `a-z` 开头，后续字符只允许 `a-z`、`0-9` 和 `_`。

- `UiDocumentId`、`UiNodeId` 和 `UiActionId` 必须至少包含两个 segment，例如 `example.minimal_page`、`page.root` 和 `example.continue`。
- `UiAssetId` 和 `UiStyleId` 允许单 segment，例如 `hero_image` 和 `title`。
- ID 区分大小写之外不做 Unicode normalization，因为非 ASCII 输入直接拒绝。
- 单个 document 的 node ID 必须唯一；验证后的 node index 保存 canonical document path，并由同一对 `document_id + node_id` 生成 ECS marker 和 audit metadata。

### 12.2 默认值和 canonical JSON

普通输入允许省略明确声明为 optional/default 的字段。解析后 canonical JSON 必须写出全部默认字段，包括空 table/list、`null` optional、默认 enum 和零值布局种子，不能根据平台或运行时环境省略。

Canonical JSON 使用 UTF-8、两空格缩进、LF 结尾；所有 object key 递归按 Unicode code point 升序排列，array 保持协议顺序。整数保持十进制整数表示，不写指数、`NaN`、`Infinity` 或负零。阶段 3 引入的浮点数必须有限，并将负零归一化；canonical 输出完成后再次解析必须得到相同 Rust 值和相同 canonical bytes。

Rust 类型通过测试期 `schemars` 派生 Draft 2020-12 schema，并与 `project/assets/ui/documents/schema/ui_document.v1.schema.json` 做完整 golden 比对。`schemars` 仅为 dev-dependency，不进入桌面或 Android 的生产依赖图。canonical golden 位于 `project/assets/ui/documents/fixtures/minimal_page.v1.canonical.json`。

## 13. v1 布局和值类型冻结项

阶段 3 将布局种子扩展为受限的正式协议。长度只允许 `auto`、`px`、`percent`、`vw` 和 `vh`；所有数值必须有限，尺寸、间距和边框不得为负，百分比及 viewport 百分比范围为 0 至 100。`min_width`、`max_width`、`min_height` 和 `max_height` 仅在单位相同时做静态矛盾检查，跨单位约束交给 Bevy 在确定 viewport 后求值。canonical JSON 将 `-0.0` 归一化为 `0.0`。

布局 `display` 只允许 `flex`、`grid` 和 `none`。Flex 支持方向、换行、grow/shrink/basis、轴对齐和独立 row/column gap；Grid 支持显式行列、固定 repeat、自动轨道、自动流和子项 start/span。单轴最多 32 个轨道定义，单项 repeat 最大 16，展开后最多 64 条轨道，span 最大 32。Grid 不提供任意 CSS 字符串、命名 line、auto-fill 或 auto-fit。

`overflow` 每轴只允许 `visible`、`clip` 和 `scroll`。局部 `z_index` 映射为独立 Bevy `ZIndex` 组件而不是 `Node` 字段，绝对值不得超过 1000；它不提供跨 UI 树的 `GlobalZIndex` 能力。

Absolute 的唯一包含块是直接父节点的 border box，与 Bevy 0.18.1 `PositionType::Absolute` 一致。每个轴必须满足以下二选一规则：显式尺寸加恰好一个锚点，或省略尺寸并同时给出两侧锚点。显式尺寸加双锚点视为过度约束，少于可求解输入视为约束不足。该规则禁止节点只携带无参考系的截图坐标。

所有布局错误在构建 ECS 前返回稳定 code 和完整字段 path。阶段 3 code 覆盖非有限值、负尺寸、百分比越界、约束矛盾、Grid 上限、Absolute 约束和 z-index 越界；阶段 9 将其纳入统一 severity、node ID 和修复提示报告。

## 14. v1 样式、视觉与资源冻结项

### 14.1 样式层级与 token

样式解析顺序固定为 token 值、组件 style、节点 inline override，后层覆盖前层的同名属性。组件 style 可以通过 `extends` 继承另一个组件 style，继承链从父到子合并；节点 `style.component` 应用组件结果后，再应用 `style.inline`。背景、边框、圆角、文字、透明度、阴影和材质字段按完整属性覆盖，不做依赖 object key 顺序的隐式合并。

token 只允许 `color`、有限 `number` 和指向另一 token 的 `reference`。解析器在构建实体前检查所有 token 和组件 style，包括当前节点未引用的声明。未知 token、token 类型不匹配、token 循环、未知组件 style 和 style 继承循环分别返回稳定字段错误；不根据错误文案分支。

颜色输入使用非线性 sRGB 色彩空间。允许 `#rrggbb`、`#rrggbbaa`，或显式包含 `red`、`green`、`blue`、`alpha` 四个 0 至 1 数值通道的 `srgb` object。六位 hex 的 alpha 固定为 `ff`；数值通道按 `round(channel * 255)` 量化。canonical JSON 一律写小写八位 `#rrggbbaa`，因此 alpha 始终显式且不同输入形式得到相同字节表示。

视觉属性首版覆盖 solid background、linear gradient、uniform border、四角 radius、文字颜色/字体资源/字号/行高/字距/字重、opacity、box shadow，以及封闭的 `frosted_panel_v1` 材质参数。linear gradient 至少 2 个、最多 6 个 stop，position 必须在 0 至 1 内按升序排列；单个 effective style 最多 3 层 box shadow。材质参数是 tagged closed enum，不接受 shader path、shader source、参数 blob 或任意材质名称。

### 14.2 Asset table 与图片呈现

asset table 的 kind 为 `image`、`font`、`icon`、`atlas` 或 `material`。节点和 style 只持有 asset ID；解析后仍会检查使用位置要求的 kind。`packaged` path 相对 `project/assets/`，首版只允许小写 ASCII `ui/` 根、正斜杠和安全 segment，并按 kind 限制图片/图标/图集及字体扩展名。绝对路径、盘符、URI、data URI、反斜杠、空 segment、`.`、`..`、NUL 和 percent-encoded separator 全部拒绝。`content_cache` 只接受稳定 logical ID，不接受真实缓存路径或 URL。

material asset 只能使用 `built_in_material` source。当前唯一 allowlist ID 为 `frosted_panel_v1`，它映射到 framework 已注册的 `UiMaterialId::FrostedPanelV1`；document 不能提供或覆盖 framework 内部 shader 路径。单 document 最多声明 4 个 material asset。

图片 presentation 是 closed enum：

- `fit` 支持 `contain`、`cover`、`stretch`；`cover` 的 focus 使用左上为 `(0, 0)`、右下为 `(1, 1)` 的归一化源图坐标。
- `nine_slice` 描述四边 inset、center/sides 的 stretch 或 tile、corner scale 和 slice 上限，并适配现有 `widgets::image::UiNineSlice`。
- `tiled` 描述 x、y 或 both 轴、stretch value 和 repeat 上限，并适配现有 `widgets::image::UiImageTiling`。
- `atlas_frame` 通过 atlas asset 的稳定 frame ID 选择 rect、original size 和 pivot，再应用 contain/cover/stretch。

atlas 最多 256 个 frame；frame 必须位于声明的 atlas 尺寸内，original size 不得小于裁切 rect，pivot 必须位于 0 至 1。九宫格最多生成 4096 slice，平铺最多 65536 repeat；具体布局计算继续复用现有 widgets 能力，document 层不复制 Bevy 渲染系统。

图片、图标和图集可声明宽、高和 decoded bytes。单边上限 4096 px，单资源 decoded bytes 上限 16 MiB，全 document 合计上限 64 MiB。声明只用于静态预检；运行时仍必须用实际解码 metadata 复核，不信任 document 自报尺寸。

阶段 4 golden fixture 为 `project/assets/ui/documents/fixtures/style_resources.v1.json`，覆盖 token alias、组件继承、inline override、两种颜色输入、字体和材质引用、Contain/Cover focus、九宫格、平铺和图集 frame。对应 canonical fixture 与 Rust 派生 JSON Schema 由 `ui_document_` 测试防止漂移。
