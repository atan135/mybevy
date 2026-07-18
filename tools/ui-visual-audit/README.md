# UI Visual Audit Tool

This development-only crate owns reference manifests and, in later stages, deterministic visual comparison. It is intentionally outside `project/` and has no dependency edge into the game or Android package.

## Reference storage

- Committable baselines: `tools/ui-visual-audit/fixtures/references/`
- Private/local references: `summary/ui-visual-audit/`
- Runtime assets: never use `project/assets/` for audit baselines

Manifest image paths are forward-slash relative paths beneath the root selected by `image.storage`. Absolute paths, parent traversal, and canonical path escapes are rejected. Committed PNG/JPEG files are covered by Git LFS rules; local references and run artifacts are ignored.

## Manifest contract

`schema_version: 1` manifests contain one or more references. Every entry binds:

- a stable `reference_id`;
- a unique key of `screen`, `device`, `state`, `locale`, `theme`, and the complete viewport;
- logical/physical/original sizes, device scale, orientation, and color space;
- an image path and SHA-256;
- source, authorization, and license evidence;
- a positive baseline version, non-empty update reason, and previous hash for revisions;
- an explicit allowed-difference profile and initial numeric tolerances.

The validator fully decodes PNG/JPEG inputs under fixed byte, dimension, and allocation limits. Stage-1 references have not been cropped or normalized, so the decoded dimensions must equal both `metadata.original_size` and `viewport.physical_size`. It rejects missing/non-file inputs, unsupported or corrupt encodings, hash and decoded-size mismatches, duplicate IDs/keys, unsafe paths, restricted committed inputs, and unrecorded revision transitions with stable snake-case machine error codes.

Validate a manifest from the repository root:

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- validate-manifest --repository-root . --manifest <manifest.json>
```

Replacing an existing baseline requires consumers to call `validate_baseline_update`: the key and reference ID must stay fixed, the version must increment by one, `previous_sha256` must match the prior baseline, the image hash must change, and `update_reason` must be non-empty. A later stage will add the human-approval update command; this stage only defines and enforces the contract.
