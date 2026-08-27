# 生成式 UI 对照验收 清单

## 目标

以 Rust 代码版 UI Gallery 为基准，建立一份可由 `UiDocument` runtime 加载的声明式对照页，通过稳定 case ID、相同审计状态和确定性截图逐项暴露协议、运行时、生成工具与视觉验收问题，并按证据逐步修复。

本清单中的“生成式 UI”包括生成流程最终交付的声明式 `UiDocument`。当前在线 provider、OCR 和图片生成适配尚未实现，本轮 approved Gallery 是人工整理的确定性对照基线，不作为真实模型生成能力证明。

## 基础原则

- [x] 代码版 Gallery 是行为与视觉覆盖分母，声明式页面不得用减少案例规避差异。
- [x] 每个首批案例使用稳定 `gallery.pair.*` ID，每个截图区使用稳定 `gallery.section.*` ID。
- [x] 生成工具、运行期页面和审计证据保持边界清晰，不把离线 fixture 描述为在线 AI 结果。
- [ ] 每个新增对照批次独立实现、验证和提交，并在完成后更新本清单。

## 阶段 1：盘点基线与对照边界

- 开始时间：未记录
- 结束时间：2026-08-10 17:50:57 +08:00
- 开发总结：确认代码版 Gallery 的 22 个审计状态、声明式 Gallery 的既有覆盖和当前生成工具边界，确定先建立可复验的首批对照页。
- 验证记录：检查 `ui_gallery.rs`、声明式 Gallery 文档、页面路由和 UI generation 文档；确认在线 provider/OCR/图片生成适配未实现。

- [x] 盘点代码版 Gallery 的图片、排版、图标、样式、效果、动画和通用控件专题。
- [x] 确认代码版 Gallery 现有 22 个默认稳定 capture state。
- [x] 确认声明式 Gallery 原页面只覆盖基础控件，且审计 recipe 只有 `initial`。
- [x] 明确首批工作验证声明式 runtime 和对照方法，不宣称完成真实在线生成。

## 阶段 2：建立首批一一对应案例

- 开始时间：未记录
- 结束时间：2026-08-10 17:50:57 +08:00
- 开发总结：重建 approved 声明式 Gallery，加入 12 个稳定首批 pair、完整控件种类和专题节点。
- 验证记录：`cargo test --lib ui_document_gallery --no-fail-fast` 通过，5 个测试通过；文档校验、pair ID、控件种类和覆盖层状态适配均有单元测试覆盖。

- [x] 使用 `gallery.pair.*` ID 标识首批 12 个代码版/声明式对应案例。
- [x] 覆盖图片 fit、九宫格、平铺、atlas、排版、溢出、图标、继承样式、效果和禁用控件状态。
- [x] 保留 Button、TextInput、Checkbox、Toggle、Segmented、Slider、Stepper、Progress、Tab、Select、Badge、Tooltip 和 ImageButton 等运行时控件。
- [x] 为高级图片声明确定性原始尺寸和解码字节预算。
- [x] 为代码版 Gallery 的每个目标视觉或交互案例建立完整 pair ID，不只覆盖首批 12 项。

## 阶段 3：统一审计状态与滚动锚点

- 开始时间：未记录
- 结束时间：2026-08-10 17:50:57 +08:00
- 开发总结：声明式 Gallery 复用代码版 Gallery 的 22 个默认 capture state，并将 document node ID 映射到现有滚动目标和专题锚点。
- 验证记录：审计 dry-run 通过；五状态真实审计 `20260810-173331-902c9d` 通过；覆盖层审计 `20260810-180457-0237ec` 通过并人工确认 Dropdown/Tooltip 可见；确定性重复截图 PNG hash 一致。

- [x] 为声明式页面注册 `OwnerDocument` ready condition。
- [x] 复用代码版 Gallery 的完整 capture contract。
- [x] 将 `gallery.root` 和 `gallery.section.*` 节点映射到稳定滚动目标和专题锚点。
- [x] 通过 `gallery.select` 和 `gallery.tooltip` node ID 确定性打开 Dropdown 与 Tooltip 审计状态。
- [x] 增加代码版与声明式 Gallery capture parity 单元测试。
- [ ] 建立代码版与声明式页面的统一 reference manifest 和自动逐状态截图比较。
- [ ] 在报告中按 pair ID 关联两个页面的节点、截图区域、差异和处置状态。

## 阶段 4：修复首轮运行时问题

- 开始时间：未记录
- 结束时间：2026-08-10 17:50:57 +08:00
- 开发总结：根据真实审计修复高级图片缺少声明尺寸和 atlas ready-state 丢失 frame rect 两类问题。
- 验证记录：首次真实审计 `20260810-172639-a107f1` 稳定报告 `UI_DOCUMENT_ADVANCED_IMAGE_SIZE_MISSING`；修复后五状态审计通过。`cargo test --lib atlas_ --no-fail-fast` 通过，4 个测试通过；atlas 单状态审计 `20260810-174505-117762` 通过，人工确认四个图帧独立显示且重复截图 hash 一致。

- [x] 将高级图片尺寸缺失作为生成文档错误暴露并补齐 Gallery 资源声明。
- [x] 为 atlas 增加显式 fit 状态，资源 ready 后保留并偏移 frame rect。
- [x] 覆盖 atlas contain 和 cover 裁切坐标回归测试。
- [x] 使用实际截图确认红圆、绿方、蓝菱形和黄十字分别显示。

## 阶段 5：补齐交互、动画与深层状态矩阵

- 开始时间：2026-08-10 18:20:00 +08:00
- 结束时间：2026-08-10 21:26:00 +08:00
- 开发总结：声明式 Gallery 已按代码版页面顺序补齐 20 个顶层专题、23 个逻辑标题、完整目标 case ID、资源、控件状态矩阵、24 项 Stress、效果和动画 adapter；323 处可见文本复用代码版 i18n key。审计驱动器在状态稳定后重新校准滚动锚点，消除了 Dropdown 状态首次截图与重复截图之间的 28px 漂移。
- 补充对齐：2026-08-11 按同屏反馈统一字体语义与颜色、大厅按钮位置、图片适配边框和网格、高保真图片尺寸、Regular/Medium/Bold 与问号图标、Clip/Ellipsis、图标及图片按钮、动画底色和组件完整状态样式；随后继续同步图片卡片 6px padding / 3px gap、Bounded tiling 与 Atlas Grid、section/caption muted 角色，以及 TextInput 框外消息、错误边框和 6px 字段列间距；声明式页面继续由 `scripts/generate-ui-gallery-parity.ps1` 确定性生成。
- 验证记录：`cargo test --lib ui_document --no-fail-fast` 通过，118 个测试通过；本轮 `ui_document_gallery` 8 个测试和 TextInput 定点测试通过；`cargo check`、`cargo fmt --all -- --check`、`git diff --check` 与审计脚本 SelfTest 通过。固定验收尺寸完整双页审计 `20260811-102031-c4a2b5` 通过，两页各 22 状态且重复截图 hash 稳定；六个反馈专题复验 `20260811-103800-aa2400` 通过；图片适配定点复验 `20260811-105704-48485c` 通过；Grid/TextInput 最终复验 `20260811-124322-ed8058` 通过并人工确认对应布局一致。

- [x] 以代码版 Gallery 的实际页面顺序建立完整章节合同，不再使用合并章节或额外说明性内容。
- [x] 为图片 fit、四类视觉 fixture、高保真综合区、九宫格、三轴平铺和四个 atlas frame 建立逐项 pair ID。
- [x] 对齐 Typography、overflow、alignment、missing glyph、按钮、图标、作用域样式、效果、选择控件、数值控件、输入、binding、overlay、正式图片和 atlas source 的全部文案及案例。
- [x] 对齐 Badge、Progress、Tab、Dropdown、Tooltip、Checkbox、Toggle 和 Segmented 的完整状态矩阵。
- [x] 将代码版诊断专用 Stress Sample 从 96 项收敛到双方都满足 document 节点预算的 24 项，并保持 Item/Ready/Waiting/Done/Inspect 内容一致。
- [x] 为声明式页面补齐代码版 Gallery 中可观测的动画播放、暂停和 seek 语义，移除静态 Progress 占位。
- [x] 对齐 Hovered、Pressed、Focused、Disabled、Selected、Open 和校验失败等控件状态矩阵。
- [x] 对齐 Dropdown、Tooltip、Modal 等覆盖层的打开、输入阻断、定位和 owner cleanup 行为。
- [ ] 覆盖长文案、超长单词、本地化、窄屏、横屏和安全区边界。
- [x] 增加代码版/声明式章节、pair ID、可见文案、资源和状态矩阵的自动内容合同测试。
- [ ] 为每项差异分类为生成输入、生成规划、协议能力、runtime 实现、资源或审计问题。

## 阶段 6：接入真实生成链路

- 开始时间：
- 结束时间：
- 开发总结：
- 验证记录：

- [ ] 接入受控在线视觉分析和结构化生成 provider，保持凭据、预算、隐私和日志边界。
- [ ] 使用代码版 Gallery 截图作为真实参考输入，生成新的未批准草稿 run。
- [ ] 将模型输出与人工确定性 Gallery 分开标记、存储和报告。
- [ ] 对生成失败、Schema 失败、资源缺失和有限 repair 失败保留可复验 artifact。
- [ ] 经人工批准和显式 promotion 后，才将生成结果纳入 approved 对照候选。

## 阶段 7：文档与最终验收

- 开始时间：2026-08-10 17:50:57 +08:00
- 结束时间：
- 开发总结：
- 验证记录：`cargo check`、`cargo fmt --all -- --check` 和 `git diff --check` 通过；仅保留仓库既有 dead-code 警告和 Git 行尾提示。

- [x] 在 UI 调试文档记录代码版/声明式 Gallery 的共用 capture contract 和运行命令。
- [x] 在 UI 当前限制中记录跨页面自动 diff、动画协议、深层状态矩阵和在线 provider 边界。
- [x] 运行 `cargo check`。
- [x] 运行 `cargo fmt --all -- --check`。
- [x] 运行 `git diff --check`。
- [x] 审阅完整 diff，确认没有引入无关改动或构建产物。
- [ ] 所有阶段完成后将清单归档到 `docs/界面/checklists/` 并纳入提交。

## 最终完成定义

以下项目作为整体完成标准，不要求每个开发阶段都执行，由所有相关阶段完成后统一验收。

- 开始时间：2026-08-10 17:50:57 +08:00
- 结束时间：
- 验收总结：

- [x] 代码版 Gallery 的全部目标案例都有稳定的一一对应 pair ID。
- [x] 两个页面在当前 `phone-landscape` 目标设备和相同 22 状态下均能确定性截图。
- [ ] 自动报告能关联每个 pair 的截图、metadata、差异分类、严重度和修复状态。
- [x] 本轮发现的协议、runtime 和审计缺陷均有最小回归测试，生成质量问题均有可复验输入与 artifact。
- [ ] 真实模型结果与人工 fixture 明确区分，任何生成结果都经过校验、审计和显式批准。
- [ ] Android 真机完成 density、safe-area、字体、触控、覆盖层和高级图片验收。
