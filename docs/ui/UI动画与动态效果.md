# UI 动画与动态效果

通用 UI 动画位于 `project/src/framework/ui/core/animation.rs`。它用于较短的属性过渡、页面和弹窗入场、控件反馈及 loading 循环，不替代 Bevy 的骨骼动画或玩法时间轴。

## 属性和写通道

`UiAnimationSpec` 当前支持这些 target：

| target | 值类型 | 写入组件 | 是否触发布局重排 |
| --- | --- | --- | --- |
| `Alpha` | `Scalar` | 非 Text 节点的 `BackgroundColor.a`，或 Text 节点的 `TextColor.a` | 否 |
| `TransformTranslation` | `Vector` | `UiTransform.translation`，单位为逻辑 px | 否 |
| `TransformScale` | `Vector` | `UiTransform.scale` | 否 |
| `LayoutPosition` | `Vector` | `Node.left/top`，单位为逻辑 px | 是 |
| `LayoutSize` | `Vector` | `Node.width/height`，单位为逻辑 px | 是 |
| `BackgroundColor` | `Color` | `BackgroundColor`，在线性 RGB 中插值 | 否 |
| `TextColor` | `Color` | `TextColor`，在线性 RGB 中插值 | 否 |

纯视觉移动和缩放默认使用 `TransformTranslation` / `TransformScale`。只有兄弟节点必须跟随位置或尺寸变化时，才使用 `LayoutPosition` / `LayoutSize`；后两者会让 Bevy 布局树重新计算，不能用于大量节点的常驻循环。

同一实体可以并行播放互不重叠的 target。同一 target 的新动画替换旧动画，并发送 `Replaced` 事件。`Alpha` 与 `BackgroundColor` / `TextColor` 会写同一颜色组件，因此不能并行；框架返回 `conflicting_target`，不依赖系统或插入顺序决定结果。旧 `UiAnimatedAlpha` 与这三个通用颜色 target 同实体时同样返回 `conflicting_legacy_alpha`；视觉 transform target仍可与旧 alpha 并行。

## 规格和时间语义

`UiAnimationSpec` 明确包含：

- `from` / `to`
- `duration_secs` / `delay_secs`
- `UiAnimationEasing`
- `UiAnimationDirection::Normal/Reverse/Alternate/AlternateReverse`
- `UiAnimationRepeat::Once/Count/Infinite`
- `UiAnimationCompletion::KeepComponent/RemoveComponent/DespawnEntity`

`Count(n)` 表示总播放次数，`Count(0)` 非法。有限动画结束时会按最后一次播放的真实方向选择端点：例如 `Alternate + Count(2)` 最终回到 `from`，`Alternate + Count(3)` 最终到达 `to`，Reverse 不会错误地固定到 `to`。

零时长有限动画在 delay 结束时直接到达真实最终端点；零时长无限循环会返回 `zero_duration_infinite_repeat`。空 ID、值类型不匹配、非有限值、负 duration/delay、非有限 duration/delay、越界 alpha、负布局尺寸及非法颜色 alpha 都会产生稳定 `Rejected` 原因，不会把 NaN 写入 Bevy 组件。

## 命令、打断和完成

业务通过 `UiAnimationCommand` 控制动画：

```rust
animation_commands.write(UiAnimationCommand::start(
    entity,
    UiAnimationSpec::transform_scale(
        UiAnimationId::new("inventory.button.press"),
        Vec2::ONE,
        Vec2::splat(0.94),
        0.1,
    )
    .with_direction(UiAnimationDirection::Alternate)
    .with_repeat(UiAnimationRepeat::Count(2)),
));
```

`start` 使用 spec 声明的 `from`。需要从当前视觉值平滑接续时使用 `continue_from_current`；该模式只接受可无损读取的当前值。布局位置/尺寸或 transform translation 当前不是 px 时返回 `current_value_unavailable`，不会悄悄回退到声明值造成跳变。

取消行为：

- `KeepCurrent`：保留当前组件值并移除轨道。
- `SnapToStart`：写入声明的 `from` 后移除轨道。
- `SnapToEnd`：按真实最终播放方向写入端点后移除轨道。

`Seek` 把轨道定位到指定的 `0..=1` 周期进度；`pause: true` 用于审计和可重复预览。非有限进度返回 `non_finite_seek_progress`。`UiAnimationEvent` 会报告 `Completed`、`Cancelled`、`Replaced` 或带稳定错误的 `Rejected`。

动画状态直接挂在目标实体上。目标实体或其页面根被 despawn 时，轨道随实体删除，不保留全局 registry，也不会向已经不存在的实体发送完成事件。Panel owner 切换仍由 `CloseAllForOwner` 立即清理覆盖层，不等待动画。`DespawnEntity` completion 只用于明确允许延迟销毁的局部视觉节点。

## 动态效果策略

全局 `UiMotionPolicy` 是 Resource：

- `Full`：按声明的 delay、duration 和 repeat 播放。
- `Reduced`：有限动画取消 delay 并以 4 倍速度完成；无限循环直接静止在第一周期的真实终点并完成清理。
- `Disabled`：忽略 delay、暂停和 seek，当帧到达有限动画的真实最终端点并完成清理。

策略同时作用于旧 `UiAnimatedAlpha`。运行时切换 policy 会更新 `UiAnimationDebugSnapshot.policy`；已经 seek+pause 或以 `KeepComponent` 完成的轨道不会因快照更新重复写视觉值。业务的可访问性设置只需写这个 Resource，不要逐实体扫描 marker。

## 主题热更新

主题刷新系统先应用新的颜色、布局和文字值，动画系统随后取消所有仍在运行的通用轨道和旧 alpha，且取消帧不再写旧动画值。因此受主题 marker 管理的字段由新主题接管，不会在动画结束或取消后恢复旧主题值。已经完成且保留的轨道不再写组件，也不会覆盖热更新。

从页面代码直接插入 `UiAnimations` 只适合内置、已验证的固定组合；动态业务优先写 `UiAnimationCommand`，以获得 target component 检查、打断事件和稳定拒绝原因。tick 仍会防御直接插入后出现 legacy alpha 的通道冲突。

## 覆盖层和 Gallery

当前组合：

- Toast 保留兼容 alpha 入场和生命周期末尾退场；Loading、Confirm 保留 alpha 入场，关闭和 owner cleanup 仍立即 despawn。半透明遮罩以主题目标 alpha 为终点。
- Loading panel 叠加 `TransformScale` 无限 pulse；Reduced/Disabled 下会静止并清理。
- Confirm panel 叠加有限 `TransformScale` 入场，不改变 modal 焦点、Interaction 或 owner。
- UI Gallery 展示控件回弹、页面入场、弹窗入退能力、loading、显式布局尺寸、颜色和 alpha；Gallery 的退场样例不改变当前 Confirm 立即关闭语义。

UI audit 的 `animations` state 对齐固定 Gallery anchor。审计应用任意 Gallery capture state 时，会把全部 `GalleryAnimationSample` 一次性 seek 到 `0.625` 并 pause，避免更早的 image/effects 截图被布局循环改变。30 帧稳定等待期间不会重复写 player、快照或目标值。metadata 输出全局 `motion_policy` 和按 Name/Entity 稳定排序的 `animation_snapshots`，每条轨道包含 target、状态、raw/eased progress、pause 和布局重排标记。

## 当前边界

- 当前没有关键帧序列、轨道组完成 barrier、弹簧/物理曲线或旋转 target。
- `UiTransform.translation` 只对 px 提供 ContinueFromCurrent；Percent/Auto 必须由业务先转换或改用声明起点。
- `Alpha` 不是子树继承 opacity；需要整组淡入时，应分别动画实际颜色节点或使用受控的组合 helper。
- 页面切换和 owner 清理优先保证输入、焦点和资源释放，当前不等待整页退场动画。
