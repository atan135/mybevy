# 方圆灵构示例资产

`home_preview.ron` 是第一阶段默认蓝图示例：一条由 `cube` 和 `sphere` 组合的金黄色龙环绕家园，脚底下有灰白色云，周围护栏预留一个门。

完整格式、字段、坐标、颜色和生成约束见 [蓝图格式与生成约束](../../../docs/方圆灵构/规范/蓝图格式与生成约束.md)；数量限制、禁止事项和审核规则见 [审核与预算规则](../../../docs/方圆灵构/规范/审核与预算规则.md)。

需要让 Codex 重新生成默认预览时，从仓库根目录发起请求，并同时指向规则文档和目标文件：

```text
请根据 docs/方圆灵构/规范/蓝图格式与生成约束.md 和 docs/方圆灵构/规范/审核与预算规则.md 生成 project/assets/fangyuan/home_preview.ron。
```

生成后确认 `home_preview.ron` 仍只包含 `cube` 和 `sphere`，数量不超过 `1000`，并且能看出金黄色龙、灰白色云、护栏和入口门的轮廓。

## 基础表面图集

`atlases/base/surface_atlas.json` 是基础表面图集的共享语义目录，按稳定 `tile_id` 绑定 `surface_color.png` 与 `surface_normal.png` 的同一格坐标。AI 生成方圆 primitive 时应读取该目录选择条目，不应直接猜测 UV 或只依赖线性 index。

当前图集为 `1024×1024`、`16×16` 网格，每格 `64×64`，其中包含 `60×60` 有效内容和四周各 `2px` padding。噪点图集若不与这 256 个表面语义槽严格对齐，应使用独立目录文件。

`atlases/noise/noise_atlas.json` 是独立的程序化噪点目录，引用 `procedural_noise.png`。其源图实际由 `12×6` 和 `10×5` 两个网格拼合而成，因此归一化后的 `16×16` 图集中只有 122 个有效格；AI 和运行时只能选择目录中明确登记的 `tile_id`，其余 134 格为黑色保留位。

`atlases/base/surface_material_presets.json` 为 256 个表面条目提供 primitive 级 PBR 常量、法线强度、参考重复尺寸和可选噪点建议，因此当前不需要 ORM 图集。该文件目前是 AI 创作 companion catalog；现有 Rust `FangyuanMaterialProfile` 仍只支持 color、alpha 和 emissive，运行时接入需要后续扩展。

### v1 资源合同边界

三个 catalog 均为 schema v1，固定 ID 为 `fangyuan.base.surface`、`fangyuan.base.surface_material_presets` 和 `fangyuan.noise.procedural`。surface 与 noise 条目 ID 使用小写 ASCII 的 `surface_xNN_yNN` / `noise_xNN_yNN` 形式，preset 使用 `material:surface/xNN/yNN`；稳定 ID 最大 64 字节，所属目录内唯一，数组按 `index = y * 16 + x` 升序保存。

三份图集统一为 `1024×1024`、`16×16`、`64×64` cell、`60×60` 内容和 `2px` padding。noise 仅有 122 个登记格，其他 134 格为 reserved。`surface_atlas.json` 的实际条目字段是 `tiles[].tile_id`，preset 才使用 `presets[].surface_tile_id` 跨目录引用 surface tile。

`surface_noise_enabled` 是 blueprint/runtime 的受控布尔开关，缺省为 `false`。当它为 `true` 但 preset 没有 noise suggestion 时只产生 warning，运行时仍禁用 noise。preset 的 `companions` 路径仅供首包 catalog 使用：允许从当前 catalog 相对解析并规范化 `../noise/noise_atlas.json`，但最终路径必须留在 `fangyuan/atlases/`；这不改变普通资源路径的 traversal 禁止规则，也不允许 AI 或 blueprint 携带路径。
