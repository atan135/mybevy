# 构建文档

本目录记录构建性能、Cargo 缓存和构建验收相关清单。统一构建入口、桌面 profile、Android target 和缓存边界以仓库根 [README](../../README.md) 与 [协作和开发约定](../../CLAUDE.md) 为准。

## 当前清单

- [编译性能优化清单](./checklists/编译性能优化_checklist.md)：记录构建 profile、缓存隔离、统一构建入口和回归验证。

## 维护边界

- 这里记录构建工具链和性能约定，不记录具体业务模块的编译错误排查。
- 修改 `scripts/build-entry.ps1`、`scripts/run_fast.ps1`、Cargo profile、target 目录或 Android 构建入口时，应同步检查本目录、根 README 和引擎入门文档。
- 构建产物和 Cargo target 不属于文档提交范围。
