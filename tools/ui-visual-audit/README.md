# UI Visual Audit Tool

This development-only crate owns reference manifests and deterministic visual comparison. It is intentionally outside `project/`, has its own Cargo target directory, and has no dependency edge into the game or Android package. Android packages only `project/assets/`, so this executable, its dependencies, fixtures, and reports add no game runtime size or startup work.

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

## Comparison boundary and CLI contract

`compare` is an offline, exact-RGBA contract boundary. Version `exact_rgba_v1` decodes bounded PNG/JPEG inputs to RGBA8, requires equal physical dimensions, and counts exact pixel changes. It intentionally does not claim orientation/color normalization, alignment, perceptual metrics, or visual diff rendering; those remain later pipeline stages.

Every input is resolved to an absolute canonical file beneath at least one repeated `--allowed-input-root`. The single `--allowed-output-root` must also resolve beneath the canonical repository root. The output directory may be new or empty, must stay beneath that root after canonicalization, and is rejected when it contains any file. Reserved artifact-name collisions are reported separately. Reports use create-new temporary files followed by rename and never overwrite an input or an existing artifact.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- compare `
  --repository-root . `
  --allowed-input-root tools/ui-visual-audit/fixtures `
  --allowed-input-root summary `
  --allowed-output-root summary/ui-visual-audit `
  --reference <reference.png> `
  --actual <actual.png> `
  --config tools/ui-visual-audit/fixtures/comparison/exact-v1.config.json `
  --output-directory summary/ui-visual-audit/<new-run-directory>
```

Append `--mask <mask.png>` when a mask is required.

The v1 config is strict JSON:

```json
{
  "schema_version": 1,
  "algorithm_version": "exact_rgba_v1",
  "max_changed_pixel_ratio": 0.0
}
```

A mask includes pixels whose alpha is nonzero and whose RGB contains at least one nonzero channel. An all-excluded mask is a comparison failure. Ratios are serialized as integer millionths to keep the stage-3 machine contract deterministic; later metric versions may add richer values without changing `exact_rgba_v1`.

Successful, comparison-failed, and threshold-failed runs write and print a report with `schema_version`, `algorithm_version`, absolute input paths, decoded dimensions, exact metrics when available, a `full_image` region result, typed failure details, and generated artifacts. Input and internal errors print the same stable snake-case failure object to stderr without creating a report.

Exit codes are public protocol:

- `0`: comparison passed;
- `2`: CLI, path, config, decode, format, or output precondition failure;
- `3`: comparison cannot execute, such as a dimension or mask mismatch;
- `4`: comparison executed and exceeded its threshold;
- `5`: internal serialization or artifact-write failure.

`fixtures/comparison/golden-cases.json` is the versioned, reproducible golden source for 1x1, solid, local-difference, transparent, dimension-mismatch, corrupt, and unsupported-format cases. Tests materialize its RGBA specifications as PNG files in temporary repositories, so no generated binary is committed. Any future committed PNG/JPEG reference baseline remains subject to the repository Git LFS rules.

## Normalization, explicit crops, and bounded alignment

`normalize-align` is a separate, versioned preprocessing boundary. It does not change `exact_rgba_v1`: consumers pass its aligned PNG artifacts to a later comparison stage explicitly. Version `normalize_align_v1` applies EXIF orientations 1 through 8, converts supported unprofiled/declared-sRGB PNG/JPEG pixels to RGBA8, uses straight alpha, and zeros hidden RGB when alpha is zero. It rejects PNG iCCP/cICP, non-sRGB standalone gamma, and JPEG ICC inputs because this version has no real ICC transform and does not claim one.

The normalization manifest is strict JSON. Reference and actual crops are role-specific and must be declared as `none`, `system_ui`, `safe_area`, or `fixed_border`; arbitrary resize and stretch are forbidden. Cropped physical dimensions and aspect ratios must match. Alignment is either disabled, a deterministic integer search, or an explicit integer translation. Per-axis limits are mandatory, hard-capped at 16 pixels, and scale always remains `1.0`. See `fixtures/comparison/normalize-align-v1.manifest.json` for the complete default contract.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- normalize-align `
  --repository-root . `
  --allowed-input-root tools/ui-visual-audit/fixtures `
  --allowed-input-root summary `
  --allowed-output-root summary/ui-visual-audit `
  --reference <reference.png> `
  --actual <actual.png> `
  --normalization-manifest tools/ui-visual-audit/fixtures/comparison/normalize-align-v1.manifest.json `
  --output-directory summary/ui-visual-audit/<new-run-directory>
```

The run retains full normalized, explicitly cropped, and aligned reference/actual PNGs plus `normalization-report.json`. The report records original, oriented, cropped, and aligned sizes; EXIF operation; source/output color and alpha semantics; pixel format; crop kind and insets; bounded translation; fixed millionth scale; quality checks; and exact forward/inverse integer transforms between original image bounds and aligned coordinates. Fully transparent, conservatively near-blank, too-small screenshots, explicit SHA-256 identity mismatches, and hash-proven reference/actual swaps have separate machine codes. Normalization comparison failures keep the completed intermediate PNGs and return exit code `3`; malformed inputs and unsupported profiles return `2`; artifact failures return `5`.
