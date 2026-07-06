# 方圆动态表现和对象接入第九至第十一阶段 Checklist

## 目标

合并推进方圆系统第九至第十一阶段：程序 VFX 播放器、技能规则层和个性层、装备 / NPC / 天道自演接入。目标是在渲染规模化底座之上，建立受控、可审核、可回放、可降级的动态表现和玩法对象表达体系。

本 checklist 重点处理 VFX recipe、deterministic tick / seed、技能规则层和个性层分离、装备语义 socket、低成本 NPC 表达和天道生成物生命周期。本阶段不实现完整战斗结算、职业技能库、传统骨骼模型、Chunk / LOD / AOI 完整体系、发布期 Bake 或纪元继承。

## 功能地图

| 功能域 | 处理方式 |
| --- | --- |
| 程序 VFX | recipe + tick + seed 生成短生命周期 primitive states |
| 技能层次 | template 固定规则层，visual blueprint 控制个性层 |
| 装备 | blueprint 编译 primitive set，并暴露 grip、tip、core、guard、aura socket |
| NPC | 使用低成本 primitive / profile / VFX 状态表达，可预算降级 |
| 天道自演 | manifest、decay、solidify、recycle 生命周期和预算回收 |
| 非目标 | 不做完整战斗数值、传统模型、完整 AOI、Bake 或运营持久化 |

## 基础原则

- [x] VFX recipe、技能视觉蓝图、装备、NPC 和天道生成物都必须可审核、可预算、可降级。
- [x] 同一 start tick、seed 和输入事件必须生成一致视觉状态。
- [x] 动态 primitive state 不能成为玩法 Entity；玩法规则和视觉输出保持分层。
- [x] 规则层永远优先于个性层，降级时必须保留真实范围、方向和危险边界。
- [x] 装备 socket 是语义锚点，不等同于骨骼动画系统。
- [x] 天道生成物默认临时，只有满足固化条件才进入区域或纪元档案。
- [x] 每个阶段完成后运行对应验证，并按阶段提交。

## 阶段 1：动态表现边界和数据契约

- 开始时间：2026-07-04 20:50:41 +08:00
- 结束时间：2026-07-04 21:02:54 +08:00
- 开发总结：完成阶段 9-11 技术路线、现有 primitive/material/audit/authority 接入点和动态对象共享字段契约的只读复核；阶段 1 不改业务代码。
- 验证记录：`git status --short --untracked-files=all` 输出为空；`rg` 在 worker 环境不可用，改用 `Get-ChildItem` / `Select-String` / `Get-Content` 只读复核。

- [x] 复核方圆技术路线中阶段 9、10、11 的目标、验收、风险和控制方式。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1780` 定义阶段 9 VFX，`:1807` 定义阶段 10 技能规则/个性层，`:1834` 定义阶段 11 装备/NPC/天道接入；worker 报告覆盖目标、验收、风险和控制方式）
- [x] 复核 VFX、skill、equipment、NPC、tiandao 与现有 primitive、material、audit、authority 的接入点。（验证：`project/src/framework/fangyuan/primitive.rs:178` 的 primitive 字段、`project/src/framework/fangyuan/audit.rs:118` 的 budget profile、`project/src/framework/fangyuan/object.rs:3` 的逻辑对象根、`project/src/game/authority/types.rs:78` 的输入帧和 `docs/世界观/天道自演.md:105` 的生命周期均已抽查）
- [x] 定义动态对象共享字段：id、version、source、budget class、lifecycle、profile refs、seed 和 replay key。（验证：worker 报告给出 `FangyuanDynamicObjectId`、source/budget/lifecycle enum、profile refs、`u64` seed 和 authority replay key 的后续落点建议）
- [x] 明确本 checklist 不做完整战斗结算、职业技能库、传统骨骼模型、Chunk / AOI、Bake 和纪元继承。（验证：checklist 目标段已列非目标；`docs/fangyuan/方圆程序动画与技能表现技术路线.md:160` 限定玩家不能改真实规则，`:193` 明确技能视觉不绑定固定模型或骨骼）
- [x] 验证命令：只读 `rg`、`Get-Content`、`git status --short`。（验证：worker 尝试 `rg` 失败后使用 `Get-ChildItem` / `Select-String` / `Get-Content` 只读复核；主 agent 复跑 `git status --short --untracked-files=all` 输出为空）

## 阶段 2：VFX Recipe 和确定性时间源

- 开始时间：2026-07-04 21:04:43 +08:00
- 结束时间：2026-07-04 21:24:21 +08:00
- 开发总结：新增 framework/fangyuan VFX recipe 数据模型、离散 clock、确定性 seed 合成和纯函数 evaluation，输出可测试 dynamic primitive state；本阶段未接入场景渲染或玩法 Entity。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_vfx_recipe -- --nocapture` 通过（4 passed）；`cargo test fangyuan_vfx_determinism -- --nocapture` 通过（4 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 定义 `FangyuanVfxRecipe` 或等价格式，包含 id、version、duration、seed policy、emitters、curves 和 budget hints。（验证：`project/src/framework/fangyuan/vfx.rs:11` 定义 recipe，字段覆盖 `id`、`version`、`duration_ticks`、`seed_policy`、`emitters`、`curves`、`budget_hints`）
- [x] 支持 spawn、move、scale、fade、color_shift、emissive_pulse、trail、impact_expand 等受控 operator。（验证：`project/src/framework/fangyuan/vfx.rs:188` 的 `FangyuanVfxOperator` 覆盖全部 operator，`:236` 对 operator payload 做校验）
- [x] 建立 VFX runtime clock，使用 authority tick 或等价离散时间驱动。（验证：`project/src/framework/fangyuan/vfx.rs:311` 定义 `FangyuanVfxClock`，`:326` / `:330` 计算 elapsed ticks/seconds）
- [x] 定义 deterministic seed 合成方式，覆盖 recipe id、caster id、event id、start tick 和 emitter index。（验证：`project/src/framework/fangyuan/vfx.rs:466` 的 `compose_fangyuan_vfx_seed` 在 deterministic policy 下混入 recipe id/version、caster id、event id、start tick 和 emitter index）
- [x] 实现 recipe evaluation 的纯函数入口，输入 tick / seed 后输出可测试 primitive state。（验证：`project/src/framework/fangyuan/vfx.rs:378` 的 `evaluate_fangyuan_vfx_recipe` 返回 `FangyuanVfxDynamicPrimitiveState`，`:360` 状态携带位置、缩放、颜色、alpha、emissive、profile、lifecycle 和 seed）
- [x] 为默认 recipe、非法 operator、同 seed 一致、不同 seed 差异、跳帧评估和重复回放补测试。（验证：`project/src/framework/fangyuan/vfx.rs:831`、`:854`、`:892`、`:908`、`:922`、`:937` 覆盖对应测试；两个定向 cargo test 均通过）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_vfx_recipe -- --nocapture`、`cargo test fangyuan_vfx_determinism -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑四条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 3：VFX Primitive Evaluator 和播放输出

- 开始时间：2026-07-04 21:26:46 +08:00
- 结束时间：2026-07-04 21:50:25 +08:00
- 开发总结：扩展 VFX evaluator、dynamic primitive state source/lifetime/profile 访问、标准 Mesh fallback 数据输出和 VFX runtime 多实例生命周期管理；本阶段不接试炼场实体生成，场景集成留给阶段 9。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_vfx_eval -- --nocapture` 通过（4 passed）；`cargo test fangyuan_vfx -- --nocapture` 通过（15 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 实现 projectile、range marker、shield、impact expand、trail segment 和 fade 的状态计算。（验证：`project/src/framework/fangyuan/vfx.rs:721`、`:747`、`:768`、`:789`、`:805` 提供默认 recipe；`:1263` 测试覆盖 projectile/range/shield/impact/trail/fade 状态）
- [x] 输出统一 dynamic primitive state，携带 position、scale、color、alpha、emissive、profile、lifetime 和 source。（验证：`project/src/framework/fangyuan/vfx.rs:362` 定义 state，`:382` 提供 position/profile/lifetime/runtime primitive 访问，`:424` 定义 source）
- [x] 将 dynamic primitive states 接入短生命周期显示路径；如动态实例化尚不稳定，保留标准动态 Mesh fallback。（验证：`project/src/framework/fangyuan/vfx.rs:458` 定义标准 Mesh fallback primitive，`:469` 将 dynamic state 转换为 fallback 渲染数据，`:1336` 测试转换）
- [x] 管理 VFX instance 生命周期，结束后释放渲染资源和统计状态。（验证：`project/src/framework/fangyuan/vfx.rs:493` / `:544` 定义 instance/runtime，`:573` tick 后移除结束实例并刷新 fallback，`:601` / `:610` 清理 clear/reload 状态）
- [x] 为超出 duration、空 recipe、负时间、多 VFX 同播、结束清理、clear scene、reload 和 fallback 补测试。（验证：`project/src/framework/fangyuan/vfx.rs:1328` 覆盖 pre-start 负时间空状态，`:1356` 覆盖多 VFX、结束清理、clear/reload/fallback，`:1438` 覆盖 future start 不提前显示；`cargo test fangyuan_vfx -- --nocapture` 15 passed）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_vfx_eval -- --nocapture`、`cargo test fangyuan_vfx -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑四条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 4：VFX 预算、审核和 authority 回放

- 开始时间：2026-07-04 21:52:27 +08:00
- 结束时间：2026-07-04 22:17:36 +08:00
- 开发总结：新增 VFX recipe audit/budget estimate、预算压力下降级评估、authority replay event 字段和 primitive state hash；authority 侧仅补充 payload 对齐测试，不改协议核心行为。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_vfx_audit -- --nocapture` 通过（2 passed）；`cargo test fangyuan_vfx_replay -- --nocapture` 通过（5 passed）；`cargo test authority -- --nocapture` 通过（8 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 为 recipe 增加审核入口，检查 duration、peak primitive、trail、alpha、emissive、profile 和 role。（验证：`project/src/framework/fangyuan/vfx.rs:832` / `:836` 增加 audit 入口，`:840` budget estimate 覆盖 duration、peak、trail、alpha、emissive、material profile 和 role）
- [x] 超出推荐预算时生成 warning 和降级建议，例如减少 trail、降低 alpha、缩短 residue、降低 emissive。（验证：`project/src/framework/fangyuan/vfx.rs:844` 至 `:915` 生成 warning finding 和 suggestion，`:2028` 测试覆盖预算 warning 与建议）
- [x] Runtime 在预算压力下能按可解释规则跳过装饰层或降低 trail，而不破坏主要可读效果。（验证：`project/src/framework/fangyuan/vfx.rs:502` 定义 `FangyuanVfxBudgetPressure`，`:707` / `:971` 支持 pressure evaluate，`:2110` 测试确认保留 core/impact 且跳过 decoration、限制 trail）
- [x] 将 VFX 触发事件与 authority 输入 / 回放 tick 对齐，明确事件字段和本地预测边界。（验证：`project/src/framework/fangyuan/vfx.rs:375` / `:385` 定义 prediction boundary 与 replay event 字段；`project/src/game/authority/plugin.rs:1505` 测试 `AuthorityFrame` payload 字段对齐）
- [x] 同一事件序列回放两次时，关键帧 primitive state hash 一致。（验证：`project/src/framework/fangyuan/vfx.rs:941` 定义 primitive state hash，`:2162` / `:2187` / `:2242` 测试 tick jump、pause/resume 和重复 replay hash）
- [x] 为 recipe audit、budget estimate、runtime degrade、tick 跳跃、暂停恢复、延迟事件和 seed 冲突补测试。（验证：`project/src/framework/fangyuan/vfx.rs:2028`、`:2110`、`:2162`、`:2187`、`:2208`、`:2242` 覆盖对应场景；seed 冲突测试显式暴露 external seed 边界）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_vfx_audit -- --nocapture`、`cargo test fangyuan_vfx_replay -- --nocapture`、`cargo test authority -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 5：技能模板和视觉蓝图

- 开始时间：2026-07-04 22:25:34 +08:00
- 结束时间：2026-07-04 22:54:03 +08:00
- 开发总结：新增方圆技能模板和视觉蓝图数据模型，建立规则层/个性层分离、字段权限策略、默认 projectile/circle/cone/shield 模板和 fallback 注册表；本阶段不实现运行时层叠和可读性审核。
- 验证记录：`cargo fmt --check` 通过；普通 `cargo test fangyuan_skill_template -- --nocapture` 触发 Windows/MSVC 增量链接缓存错误，改用 `CARGO_INCREMENTAL=0 cargo test fangyuan_skill_template -- --nocapture` 通过（5 passed）；`CARGO_INCREMENTAL=0 cargo test fangyuan_skill_visual -- --nocapture` 通过（5 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 新增 `FangyuanSkillTemplate` 或等价结构，表达规则层、range shape、direction、danger boundary、cast / impact 时序和强制可见元素。（验证：`project/src/framework/fangyuan/skill.rs:18` 定义 template，`:23` 至 `:34` 覆盖规则层、范围、方向、危险边界、时序、强制可见元素和 authority behavior）
- [x] 新增 `FangyuanSkillVisualBlueprint` 或等价结构，引用 skill template 并定义颜色、profile、trail、decor、impact residue 和 emissive 个性层。（验证：`project/src/framework/fangyuan/skill.rs:295` 定义 visual blueprint，`:297` / `:298` 引用模板，`:299` 至 `:313` 覆盖颜色、profile、VFX recipe、trail、decor、impact residue、emissive）
- [x] 明确哪些字段玩家可改、哪些字段系统锁定、哪些字段只能通过审核建议降级。（验证：`project/src/framework/fangyuan/skill.rs:215` 定义 `FangyuanSkillFieldPolicy`，`:224` / `:236` 返回 template/visual 字段权限，`:832` / `:848` / `:859` 给出默认锁定、玩家可改和审核降级字段）
- [x] 提供少量默认模板，例如 projectile、circle area、cone、shield。（验证：`project/src/framework/fangyuan/skill.rs:657` 返回默认模板列表，`:675`、`:694`、`:708`、`:725` 分别定义 projectile/circle/cone/shield）
- [x] 为模板版本、非法范围、缺失强制可见元素、非法 template 引用、越权覆盖规则字段和缺省 fallback 补测试。（验证：`project/src/framework/fangyuan/skill.rs:951`、`:964`、`:984`、`:1027`、`:1041`、`:999`、`:1071` 覆盖对应测试）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_skill_template -- --nocapture`、`cargo test fangyuan_skill_visual -- --nocapture`、`cargo check`。（验证：主 agent 复跑 `cargo fmt --check` 和 `cargo check` 通过；普通模板测试遇到 MSVC 增量链接缓存错误，`CARGO_INCREMENTAL=0` 复跑模板与视觉测试均通过）

## 阶段 6：技能可读性审核、运行时层叠和降级

- 开始时间：2026-07-04 22:57:10 +08:00
- 结束时间：2026-07-04 23:23:36 +08:00
- 开发总结：新增技能视觉可读性审核、规则层/个性层 runtime presentation 编译和技能降级等级；规则层排序、可见性和生命周期保持独立，降级仅作用于个性层。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_skill_audit -- --nocapture` 通过（3 passed）；`cargo test fangyuan_skill_runtime -- --nocapture` 通过（2 passed）；`cargo test fangyuan_skill_degrade -- --nocapture` 通过（2 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 审核视觉范围是否小于、偏离或误导真实规则范围。（验证：`project/src/framework/fangyuan/skill.rs:423` 定义 `FangyuanSkillVisualRangeHint`，`:717` 的 audit 入口检查 visual range，`:1922` / `:1937` 测试覆盖范围过小和形状误导）
- [x] 审核个性层是否遮挡强制规则层、颜色是否与危险等级冲突、透明和发光是否超预算。（验证：`project/src/framework/fangyuan/skill.rs:741` 至 `:780` 检查 rule alpha、occlusion、color、transparent budget、emissive；`:1951` 测试覆盖）
- [x] 将 skill template 和 visual blueprint 编译为规则层 VFX 与个性层 VFX 两组 runtime primitive states。（验证：`project/src/framework/fangyuan/skill.rs:697` 定义 `FangyuanSkillRuntimePresentation`，`:786` 编译入口输出 `rule_layer_states` 与 `personality_layer_states`）
- [x] 播放时确保规则层排序、可见性和生命周期不被个性层覆盖。（验证：`project/src/framework/fangyuan/skill.rs:703` 的 `playback_states` 规则层先于个性层，`:1680` 排序规则层，`:1969` / `:1999` 测试覆盖播放顺序和生命周期不被覆盖）
- [x] 定义技能表现降级等级，优先移除装饰、残影、透明、发光和长 trail。（验证：`project/src/framework/fangyuan/skill.rs:571` 定义 `FangyuanSkillDegradeLevel`，`:1535` 至 `:1618` 在个性层编译中按等级压缩发光、残留、decor 和 trail）
- [x] 为危险边界不足、装饰越界、过度遮挡、颜色冲突、降级后规则层不变和运行时播放补测试。（验证：`project/src/framework/fangyuan/skill.rs:1922`、`:1937`、`:1951`、`:1969`、`:2025`、`:2059` 覆盖对应场景）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_skill_audit -- --nocapture`、`cargo test fangyuan_skill_runtime -- --nocapture`、`cargo test fangyuan_skill_degrade -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 7：装备 Blueprint 和技能 Socket 联动

- 开始时间：2026-07-04 23:28:38 +08:00
- 结束时间：2026-07-05 00:24:03 +08:00
- 开发总结：新增方圆装备 blueprint、语义 socket runtime set、装备审核和默认练习装备；技能视觉蓝图可声明装备 socket binding，运行时编译会应用 socket 位置并记录缺失 fallback。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_equipment -- --nocapture` 通过（6 passed）；`cargo test fangyuan_skill -- --nocapture` 通过（20 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 新增 `FangyuanEquipmentBlueprint` 或等价结构，编译为 runtime primitive set。（验证：`project/src/framework/fangyuan/equipment.rs:37` 定义 blueprint，`:111` 编译 `FangyuanEquipmentRuntime`，`:888` 测试默认装备编译 primitive set）
- [x] 定义 grip、tip、core、guard、aura 等语义 socket，记录位置、语义、可引用规则和 fallback。（验证：`project/src/framework/fangyuan/equipment.rs:350` 定义 socket 记录，`:398` 定义 grip/tip/core/guard/aura，`:191` runtime set 解析引用规则和 fallback）
- [x] 审核装备 primitive 数、bounds、材质、透明、发光和 socket 合法性。（验证：`project/src/framework/fangyuan/equipment.rs:126` 装备 audit 接入预算，`:752` 审核 socket，`:959` 测试 primitive bounds/material/transparent/emissive）
- [x] 技能 VFX recipe / skill visual blueprint 支持引用装备 socket 作为发射点、轨迹控制点或装饰锚点。（验证：`project/src/framework/fangyuan/skill.rs:331` visual blueprint 声明 socket bindings，`:391` 定义 binding target，`:1752` / `:1797` 应用到 emitter origin 或 Move from/to）
- [x] socket 缺失时使用明确 fallback，不让技能播放失败成静默错误。（验证：`project/src/framework/fangyuan/equipment.rs:328` resolution 携带 fallback diagnostic，`project/src/framework/fangyuan/skill.rs:812` presentation 记录 runtime binding，`:2375` 测试 missing socket fallback）
- [x] 为默认装备、缺失 socket、重复 socket、非法 socket 位置、projectile from tip、shield from core 和 missing socket fallback 补测试。（验证：`project/src/framework/fangyuan/equipment.rs:888`、`:907`、`:922`、`:938` 覆盖装备用例；`project/src/framework/fangyuan/skill.rs:2303`、`:2340`、`:2375` 覆盖技能 socket 用例）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_equipment -- --nocapture`、`cargo test fangyuan_skill -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑四条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 8：方圆 NPC 和天道自演 FSM

- 开始时间：2026-07-05 00:27:04 +08:00
- 结束时间：2026-07-05 01:15:43 +08:00
- 开发总结：新增方圆 NPC profile/blueprint、状态表现和降级编译；新增天道生成物 manifest/decay/solidify/recycle 生命周期 FSM、TTL 和预算回收结果。本阶段保持纯数据和纯函数，不接场景实体、UI、AOI/LOD 或持久化。
- 验证记录：`cargo fmt --check` 通过；普通 `cargo test fangyuan_npc -- --nocapture` 触发 Windows/MSVC 增量链接匿名 LLVM 符号错误，改用 `CARGO_INCREMENTAL=0 cargo test fangyuan_npc -- --nocapture` 通过（6 passed）；`CARGO_INCREMENTAL=0 cargo test fangyuan_tiandao -- --nocapture` 通过（8 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 新增方圆 NPC blueprint 或 profile，支持低成本 body、marker、role color、aura 和 simple state。（验证：`project/src/framework/fangyuan/npc.rs:28` 定义 blueprint，`:286` 定义 profile，`:314` 编译 body/core/marker/aura/nameplate，`:803` 测试默认低成本 primitive set）
- [x] NPC 表现支持 idle、moving、casting、damaged 等少量状态的材质或 VFX 变化。（验证：`project/src/framework/fangyuan/npc.rs:171` 定义四种 simple state，`:197` 起按状态修改 scale、alpha、emissive 和 lifecycle，`:824` / `:854` 测试状态表现）
- [x] NPC 可按预算降级为 marker / silhouette / nameplate 级别，不保留高成本装饰。（验证：`project/src/framework/fangyuan/npc.rs:249` 定义 Full/Silhouette/Marker/Nameplate，`:649` 执行降级过滤，`:876` 测试移除 aura/decor/material/emissive）
- [x] 新增天道生成物数据结构，包含 cause、region、budget cost、lifecycle state、ttl 和 solidify score。（验证：`project/src/framework/fangyuan/tiandao.rs:8` 定义 manifestation 字段，`:337` 测试 cause/region/budget/ttl/score）
- [x] 实现 manifest、decay、solidify、recycle 四类生命周期状态和转换规则。（验证：`project/src/framework/fangyuan/tiandao.rs:85` 定义四态，`:195` FSM 入口，`:280` manifest tick/decay/solidify 规则）
- [x] 为 NPC blueprint、状态切换、降级、天道生命周期转换、ttl、固化条件和回收预算补测试。（验证：`project/src/framework/fangyuan/npc.rs:803`、`:824`、`:854`、`:876`、`:912` 覆盖 NPC；`project/src/framework/fangyuan/tiandao.rs:337`、`:372`、`:402`、`:427`、`:443` 覆盖天道 FSM、TTL、固化和预算释放）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_npc -- --nocapture`、`cargo test fangyuan_tiandao -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑格式和 check 通过；普通 NPC 测试遇到 MSVC 增量链接错误，关闭增量后 NPC 6 passed、Tiandao 8 passed；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 9：统一预算、试炼场和场景集成

- 开始时间：2026-07-05 01:19:26 +08:00
- 结束时间：2026-07-05 03:04:08 +08:00
- 开发总结：新增统一对象预算/审核模块，接入 VFX、技能、装备、NPC 和天道生成物；Home 试炼入口现在会生成实际可见 trial mesh 实体，HUD 显示 trial 对象和预算摘要，Reload/Clear/Exit/重复进入会清理 trial runtime、实体和专用材质。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan_object_budget -- --nocapture` 通过（5 passed）；`cargo test fangyuan_vfx -- --nocapture` 通过（22 passed）；`cargo test fangyuan_home -- --nocapture` 通过（51 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] VFX、技能、装备、NPC 和天道生成物接入统一 audit report 和 budget profile。（验证：`project/src/framework/fangyuan/object_budget.rs:126` 定义统一 profile，`:338` audit 汇总 VFX/skill/equipment/NPC/tiandao entry 与嵌套 report）
- [x] 定义 object class 优先级，热点时优先降级 NPC 装饰、天道临时残留、装备 aura 和技能个性层。（验证：`project/src/framework/fangyuan/object_budget.rs:56` 定义 degrade target 顺序，`:1062` 测试确认热点建议顺序）
- [x] 准备试炼入口或调试场景，能触发 projectile、range、shield、impact，展示装备、NPC 和天道生成物。（验证：`project/src/framework/fangyuan/object_budget.rs:647` 启动默认 showcase，`:709` 输出 VFX/装备/NPC/天道 visual primitives；`project/src/game/scenes/fangyuan_home.rs:2262` 生成 trial mesh 实体）
- [x] HUD 显示 active VFX、template id、visual id、equipment、npc、tiandao counts、budget cost 和 finding 摘要。（验证：`project/src/game/screens/gameplay/fangyuan_home.rs:244` HUD 文本包含 trial/vfx/tpl/vis/eq/npc/td/cost/find 字段）
- [x] Reload、Clear、场景退出和重复进入不残留对象、VFX 或预算状态。（验证：`project/src/game/scenes/fangyuan_home.rs:2545` 清理 trial runtime/entity/material，`:4513` stage9 测试覆盖 reload、clear、exit、mode switch 和 re-enter）
- [x] 为跨类型总预算、单类型超预算、降级建议、试炼场路由和场景清理补测试。（验证：`project/src/framework/fangyuan/object_budget.rs:1062`、`:1098`、`:1120`、`:1165` 覆盖预算/降级/试炼 runtime；`project/src/game/scenes/fangyuan_home.rs:5751` / `:5796` 断言 trial 实体和材质清理）
- [x] 验证命令：`cargo fmt --check`、`cargo test fangyuan_object_budget -- --nocapture`、`cargo test fangyuan_vfx -- --nocapture`、`cargo test fangyuan_home -- --nocapture`、`cargo check`。（验证：主 agent 在 `project/` 复跑五条命令全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 阶段 10：回归测试和手动验收

- 开始时间：2026-07-05 03:06:37 +08:00
- 结束时间：2026-07-05 03:15:16 +08:00
- 开发总结：完成第 10 阶段回归验证和受控手机比例窗口启动验收；本阶段未改业务代码。受限于当前环境，视觉类验收记录为窗口可启动加自动/代码证据复核，不等同人工肉眼确认。
- 验证记录：`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（301 passed）；`cargo test authority -- --nocapture` 通过（8 passed，含 VFX replay event payload 对齐测试）；`cargo check` 通过；`cargo run -- --window-profile phone-portrait` 受控启动成功并主动结束，无 panic、无残留进程；保留既有 `selection.rs:32` dead_code warning。

- [x] 运行 `cargo fmt --check`。（验证：worker 在 `project/` 执行通过）
- [x] 运行 `cargo test fangyuan -- --nocapture`。（验证：worker 在 `project/` 执行通过，301 passed、0 failed）
- [x] 运行 `cargo test authority -- --nocapture` 或等价 authority 回放相关测试。（验证：worker 在 `project/` 执行通过，8 passed、0 failed，覆盖 `authority_vfx_replay_event_payload_aligns_with_frame_tick_fields`）
- [x] 运行 `cargo check`。（验证：worker 在 `project/` 执行通过，仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）
- [x] 手动运行手机比例窗口，确认 VFX 预览、技能规则层、装备 socket、NPC 和天道生成物显示合理。（验证：`cargo run -- --window-profile phone-portrait` 受控启动成功并主动结束，无 panic/残留；视觉合理性以 `cargo test fangyuan -- --nocapture` 和 trial visual primitive 覆盖作为自动/代码证据，非人工肉眼确认）
- [x] 手动验证同一技能模板不同皮肤仍能识别真实范围和危险边界。（验证：`cargo test fangyuan -- --nocapture` 通过；相关测试覆盖视觉范围过小、形状误导、规则层/个性层顺序和降级后规则层不变，属于自动/代码证据）
- [x] 手动验证同一 seed / replay 下 projectile、impact、颜色曲线和生命周期一致。（验证：`cargo test fangyuan -- --nocapture` 和 `cargo test authority -- --nocapture` 通过；相关 VFX 测试覆盖同 seed replay、tick jump、hash 稳定、projectile/impact/颜色曲线/生命周期，属于自动/代码证据）

## 阶段 11：文档同步和归档准备

- 开始时间：2026-07-05 03:16:18 +08:00
- 结束时间：2026-07-05 03:31:38 +08:00
- 开发总结：完成方圆技术路线、程序动画与技能表现路线、天道自演蓝图规则层边界的文档同步，并创建 `docs/fangyuan/checklists/` 归档副本；归档副本会随最终 checklist 状态同步提交。
- 验证记录：`git diff --check` 通过；`cargo fmt --check` 通过；`cargo test fangyuan -- --nocapture` 通过（301 passed）；`cargo test authority -- --nocapture` 通过（8 passed）；`cargo check` 通过（保留既有 `selection.rs:32` dead_code warning）。

- [x] 更新方圆技术路线，记录 VFX recipe、确定性、技能规则层/个性层、装备 socket、NPC 抽象表达和天道 FSM。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1781` 起更新阶段 9-11，`docs/fangyuan/方圆程序动画与技能表现技术路线.md:9` 起记录当前落地边界）
- [x] 更新世界观蓝图规则，说明玩家定制不可越过规则层、可读性限制、对象审核和预算要求。（验证：`docs/世界观/天道自演.md:196` 新增蓝图规则层边界、可读性限制、对象审核和预算要求）
- [x] 确认文档仍明确完整战斗结算、职业技能库、传统骨骼模型、Chunk / AOI、Bake 和纪元继承不是本 checklist 能力。（验证：`docs/fangyuan/方圆对象资源构建与渲染技术路线.md:1876`、`docs/fangyuan/方圆程序动画与技能表现技术路线.md:651`、`docs/世界观/天道自演.md:238` 均保留非目标边界）
- [x] checklist 全部完成后，按仓库约定从 `summary/` 归档到 `docs/fangyuan/checklists/`。（验证：已创建并同步 `docs/fangyuan/checklists/方圆动态表现和对象接入第九至第十一阶段_checklist.md`）
- [x] 验证命令：`git diff --check`、`cargo fmt --check`、`cargo test fangyuan -- --nocapture`、`cargo test authority -- --nocapture`、`cargo check`。（验证：worker 在 `project/` / 仓库根目录执行全部通过；仅有既有 `src/framework/ui/widgets/controls/selection.rs:32` 未使用 warning）

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-07-05 03:31:38 +08:00
- 结束时间：2026-07-05 03:31:38 +08:00
- 验收总结：第九至第十一阶段全部完成，方圆动态表现和对象接入已形成 VFX recipe、确定性回放、技能规则层/个性层、装备 socket、NPC 低成本表达、天道 FSM、统一对象预算和 Home 试炼入口的可测试闭环；文档已同步并归档。人工视觉验收受限于当前环境，仅完成受控窗口启动和自动/代码证据复核。

- [x] VFX recipe 可以生成 projectile、range、shield、impact 和 trail 等短生命周期 primitive states。（验证：`cargo test fangyuan_vfx -- --nocapture` 22 passed；`cargo test fangyuan -- --nocapture` 301 passed）
- [x] 同一 tick、seed 和触发事件回放结果一致，有自动测试覆盖。（验证：VFX determinism/replay 测试和 `cargo test authority -- --nocapture` 8 passed）
- [x] 技能模板能表达规则层，视觉蓝图只能表达受控个性层。（验证：`project/src/framework/fangyuan/skill.rs` 覆盖 template、visual blueprint、field policy 和 runtime presentation；`cargo test fangyuan -- --nocapture` 通过）
- [x] 审核能发现视觉范围误导、规则层遮挡、颜色冲突和个性层超预算。（验证：`cargo test fangyuan_skill_audit -- --nocapture` 曾通过，最终 `cargo test fangyuan -- --nocapture` 301 passed）
- [x] 装备 blueprint 能编译为方圆 primitive set，并暴露稳定语义 socket。（验证：`cargo test fangyuan_equipment -- --nocapture` 曾通过，最终 `cargo test fangyuan -- --nocapture` 301 passed）
- [x] 技能 VFX 能引用装备 socket，缺失 socket 有明确 fallback。（验证：`project/src/framework/fangyuan/skill.rs` socket binding 测试纳入 `cargo test fangyuan -- --nocapture`）
- [x] 方圆 NPC 能低成本显示、状态切换并按预算降级。（验证：NPC 测试纳入 `cargo test fangyuan -- --nocapture`）
- [x] 天道自演生成物具备 manifest、decay、solidify、recycle 生命周期和预算回收。（验证：tiandao 测试纳入 `cargo test fangyuan -- --nocapture`）
- [x] 试炼场或调试场景可触发、重播和清理动态表现与对象。（验证：`project/src/game/scenes/fangyuan_home.rs:4513` stage9 测试覆盖 reload、clear、exit、mode switch 和 re-enter；`cargo test fangyuan_home -- --nocapture` 51 passed）
- [x] `cargo fmt --check` 通过。（验证：阶段 10 和阶段 11 均执行通过）
- [x] `cargo test fangyuan -- --nocapture` 通过。（验证：阶段 10 和阶段 11 均执行通过，301 passed）
- [x] `cargo test authority -- --nocapture` 或等价回放测试通过。（验证：阶段 10 和阶段 11 均执行通过，8 passed）
- [x] `cargo check` 通过。（验证：阶段 10 和阶段 11 均执行通过，仅有既有 `selection.rs:32` dead_code warning）
