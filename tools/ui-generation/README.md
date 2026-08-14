# UI Generation Tool

`tools/ui-generation` 是桌面/CI 使用的离线 UI 生成工具，不属于 `project` workspace，不依赖 Bevy runtime。它使用自己的 `tools/ui-generation/target` 增量缓存，并通过 `tools/ui-document-core` 共享 schema、校验和 tooling facade。

从仓库根目录执行常用检查：

```powershell
cargo test --locked --manifest-path tools/ui-generation/Cargo.toml
cargo run --locked --manifest-path tools/ui-generation/Cargo.toml -- check-boundary --repository-root .
cargo run --locked --manifest-path tools/ui-generation/Cargo.toml -- preview-document --document <document.json> --output-directory <new-output-dir> --repository-root . --width 390 --height 844
```

`project/target`、`tools/ui-generation/target` 和 Android 的 `project/target-android` 不共享 Cargo target。CI 使用 `.github/workflows/build-matrix.yml` 的独立 cache key；锁文件或 manifest 变化会失效缓存，源码变化保留可复用的增量产物。工具失败时优先查看命令输出和 CI 上传的 transcript，不要通过 `cargo clean` 恢复。
