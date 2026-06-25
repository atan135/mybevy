# 方圆灵构示例资产

`home_preview.ron` 是第一阶段默认蓝图示例：平面围栏中间放置一只由 `cube` 和 `sphere` 组合的大水牛。

完整格式、字段、坐标、颜色、数量限制和禁止事项见 [docs/世界观/方圆灵构蓝图规则.md](../../../docs/世界观/方圆灵构蓝图规则.md)。

需要让 Codex 重新生成默认预览时，从仓库根目录发起请求，并同时指向规则文档和目标文件：

```text
请根据 docs/世界观/方圆灵构蓝图规则.md 生成 project/assets/fangyuan/home_preview.ron。
```

生成后确认 `home_preview.ron` 仍只包含 `cube` 和 `sphere`，数量不超过 `1000`，并且能看出一圈围栏和围栏中间的大水牛轮廓。
