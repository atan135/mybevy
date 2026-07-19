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

## Full-image metrics and visual diffs

`analyze-diff` consumes the same-size aligned PNG pair produced above. It is a separate `ui_diff_metrics_v1` boundary and does not change `exact_rgba_v1` or `normalize_align_v1`. Inputs must be lowercase PNG with an 8-bit RGBA IHDR, supported sRGB semantics, and normalized straight alpha whose fully transparent pixels contain zero RGB. Implicit RGB conversion, hidden transparent RGB, ICC/CICP data, non-sRGB gamma, dimension mismatch, unsafe paths, and decode/budget failures are rejected explicitly.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- analyze-diff `
  --repository-root . `
  --allowed-input-root project/target/ui-visual-audit `
  --allowed-input-root tools/ui-visual-audit/fixtures `
  --allowed-output-root project/target/ui-visual-audit `
  --reference <aligned-reference.png> `
  --actual <aligned-actual.png> `
  --config tools/ui-visual-audit/fixtures/comparison/ui-diff-metrics-v1.config.json `
  --output-directory project/target/ui-visual-audit/<new-run-directory>
```

Raw evidence includes per-channel absolute sums, means in channel-unit millionths, maxima, exact changed pixels, configured over-threshold pixels, and a separate alpha section. Tolerated evidence never replaces raw evidence. The default small-noise rule ignores only pixels where every RGBA channel differs by at most 3. The anti-alias rule additionally ignores RGB differences up to 12 only when reference and actual are both fixed-threshold edges at the exact same coordinate and alpha differs by at most 3. It performs no neighbor search, blur, resize, or translation, so a 1-pixel layout or font-position change remains visible.

The structural metric is SSIM from Wang et al. (2004), adapted only by fixing the UI audit execution contract: alpha is composited over white with integer arithmetic, luma is `(77R + 150G + 29B + 128) / 256`, windows are non-overlapping 8x8 blocks including deterministic partial edge blocks, population variance is used, and `K1=0.01`, `K2=0.03`, `L=255`. All sufficient statistics and the SSIM rational formula use `i128`; each window and the final average use signed half-up rounding to millionths. Its declared range is `[-1_000_000, 1_000_000]`, where `1_000_000` is identical structure. This local luminance metric is useful for UI geometry but is not treated as a final gate or substituted for color/alpha evidence.

Explainable categories are reported separately:

- geometry is the XOR of same-coordinate 3x3 Sobel edge membership;
- color is a tolerated changed pixel where edge membership agrees;
- large-area content is a 4-connected tolerated-difference component meeting both the configured absolute and image-ratio minimum.

Every category records counts, ratios, and dominant bounds where applicable. Processing is single-threaded and row-major with fixed integer color, edge, rounding, and connected-component rules. `side-by-side.png` is `2W x H` with each half unchanged; `overlay.png`, `heatmap.png`, and the tolerance-aware `binary-diff.png` are `W x H`. The five-file bundle, including `diff-metrics-report.json`, is staged through create-new temporary files and finalized with same-directory hard links, which fail atomically if a destination appears; rollback removes only paths recorded as created by the current transaction.

Before allocating metric and artifact work buffers, the tool computes the same conservative peak estimate written to the report: `pixel_count * 64 + 4 MiB`, using checked integer arithmetic. Runs above the public 512 MiB `DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES` limit fail as `image_too_large`; the estimate and budget cannot drift because validation returns the exact value later serialized. Reports mark this as `estimated_not_os_measured`. Elapsed milliseconds cover input validation, decoding, analysis, and PNG encoding, but explicitly exclude artifact persistence and stdout serialization.

`fixtures/comparison/ui-diff-metrics-v1.golden-cases.json` is a binary-free textual fixture catalog for solid bias, 1-pixel shift, font anti-alias edges, missing control, large background change, and alpha change. Tests deterministically generate each PNG and pin the SHA-256 of the complete serialized metric object, so every raw, tolerated, SSIM, and category value participates in golden review.

## Region, mask, and local weighted rules

`audit-regions` is the independent `ui_region_audit_v1` stage after normalization and full-image metric analysis. It consumes the exact aligned PNG artifacts and successful `normalization-report.json` from `normalize_align_v1`, plus the unchanged `ui_diff_metrics_v1` metric configuration. The region configuration and report are strict JSON; the committed example is `fixtures/comparison/ui-region-audit-v1.config.json`.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- audit-regions `
  --repository-root . `
  --allowed-input-root project/target/ui-visual-audit `
  --allowed-input-root tools/ui-visual-audit/fixtures `
  --allowed-output-root project/target/ui-visual-audit `
  --reference <aligned-reference.png> `
  --actual <aligned-actual.png> `
  --diff-config tools/ui-visual-audit/fixtures/comparison/ui-diff-metrics-v1.config.json `
  --region-config <bound-region-config.json> `
  --normalization-report <normalization-report.json> `
  --output-directory project/target/ui-visual-audit/<new-region-run>
```

Every config binds the original reference SHA-256 and positive baseline revision. The hash must equal the reference identity recorded by the supplied normalization report. Every ignore declaration repeats that binding, has a non-empty review reason, and fails when either hash or revision no longer matches the active config. PNG mask shapes additionally bind their own encoded-file SHA-256. This makes a baseline change invalidate stale exclusions instead of silently carrying them forward.

Shapes use explicit `aligned`, `reference_original`, or `actual_original` coordinates. Rectangles use half-open integer bounds. Polygons are rasterized deterministically at integer pixel centers with no anti-alias expansion. PNG masks must already be aligned, same-size, canonical RGBA8/sRGB PNGs and select pixels by nonzero alpha. A declaration chooses either `reject_out_of_bounds` or `clip_to_aligned`; empty selections always fail. Reference-element bounds may originate only in reference-original or aligned space, while declarative-node bounds may originate only in actual-original or aligned space. Original coordinates are transformed by the exact forward maps from the normalization report, and major differences are mapped back through both inverse transforms.

`declared_regions_only` audits only the union of explicit include regions. `full_image` is accepted only when the declared include union covers every aligned pixel; a gap fails as `audit_scope_incomplete`, and the tool never invents an implicit region result. Exclusions are unioned once for coverage, so overlap does not inflate the ignored ratio. The exact ignored fraction is capped below 100% by `maximum_ignored_ratio_millionths`, and no configuration can exclude every pixel. `ignored-regions.png` paints every excluded pixel opaque magenta. `audit-coverage.png` uses red for critical, amber for normal, cyan for decorative, magenta for ignored, and a dimmed source image for uncovered pixels.

Each region independently reports raw RGBA/alpha/tolerance metrics, fixed-point SSIM, geometry/color/large-area categories, threshold violations, local status, overlap counts, and up to five strongest aligned/reference-original/actual-original difference locations. Ratios use only evaluated region pixels. Sobel membership is calculated on the full aligned inputs before region sampling so a region boundary does not invent an edge. SSIM keeps the global 8x8 grid, includes only evaluated pixels in each window, skips empty windows, and gives each non-empty window equal weight. Large-area connectivity cannot cross an excluded pixel. Critical, normal, and decorative profiles have strictly decreasing weights; key text and key buttons must be critical. Critical maximum tolerances cannot be looser than normal, and normal cannot be looser than decorative. Weight totals sum independent region outcomes and never average overlapping pixels or conceal a failed region.

The report intentionally says `region_local_rules_only_no_global_pass_failed_needs_review_or_invalid_gate`. A region may have local status `failed` while the command exits `0`; stage 9 owns global status and threshold exit behavior. Input, mapping, stale binding, mask, ratio, memory, and artifact errors still use the shared stable snake-case failure protocol. Region processing preserves the 512 MiB deterministic memory gate with additional selection-mask headroom, limits declaration and polygon counts, and persists its two PNGs plus JSON with the same create-new, no-clobber transaction discipline as full-image metrics.

## Runtime semantic tree audit

`audit-semantics` consumes one runtime capture metadata JSON and applies the independent `ui_semantic_audit_v1` contract. The runtime emits `semantic_tree.schema_version = 3` only while local audit mode is active. Coordinates are logical pixels, rectangles are half-open, and every float is normalized to the nearest 1/64 pixel with half-away-from-zero rounding. Each node includes its stable hierarchy, optional Bevy `Name`, and `ComputedNode::stack_index()`; each panel and Toast root includes its real capture Entity, optional `Name`, and kind-specific likely source files. `capture_entity` remains diagnostic-only and never participates in `stable_id`. The target page subtree is combined only with visible Floating/Modal/BlockingOverlay subtrees owned by the same UI owner and visible global Toast subtrees; unrelated owners are excluded.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- audit-semantics `
  --repository-root . `
  --allowed-input-root project/target/ui-audit `
  --allowed-input-root tools/ui-visual-audit/fixtures/semantic `
  --allowed-output-root project/target/ui-visual-audit `
  --metadata <capture-metadata.json> `
  --config tools/ui-visual-audit/fixtures/semantic/ui-semantic-audit-v1.config.json `
  --output-directory project/target/ui-visual-audit/<new-semantic-run>
```

Node bounds are intersected with the viewport and every clipping/scroll ancestor before `fully_clipped` is decided. Invisible, fully clipped, and pure layout nodes are excluded from ordinary rules, while visible zero-size semantic nodes are checked before the clip skip. Text overlap compares only effective visible text measurements in the same panel. The rules cover critical text clipping, safe-area overflow, unreachable scroll content, touch target size, visible/accessibility label evidence, disabled/loading interaction consistency, and overlay z/focus/input behavior.

Stable IDs never contain Bevy Entity values. Declarative IDs use document owner, panel, document ID, and node ID; traditional IDs use stable names or a parent path plus same-role sibling ordinal. `capture_entity` remains a run-local diagnostic only. Findings retain document/node/source path for declarative UI and panel/likely-file hints for traditional UI.

Overlay metadata distinguishes the single active focus scope from inactive underlying scopes. Blocking overlays take focus precedence; otherwise modal and focusable floating panels follow the production z/order rule. Only the approved `dropdown` and `tooltip` panels may use `transient_above_modal`, which must be strictly above modal and below blocking/toast layers. A loading overlay with no focusable controls passes only when focus suppression is explicit; Toast must not trap focus, route blocking input, or block lower picking.

`semantic-audit-report.json` uses report schema v3 and records the exact runtime metadata SHA-256 together with semantic status and findings, separately from full-image and region metrics. A semantic hard failure returns exit code `4`; the report fixes `visual_similarity_consumed`, `local_visual_scores_consumed`, and `can_visual_score_offset_hard_failure` to `false`. This is not the stage-9 aggregate pass/review/invalid gate. Inputs are capped at 8 MiB and 50,000 nodes. Text overlap uses visible clipped rectangles with a stable sweep/active set. The public `MAX_SEMANTIC_OVERLAP_CANDIDATES` CPU-work limit is checked after deterministic O(n log n) X/Y projection planning and again during scanning; exceeding it returns `semantic_overlap_candidate_limit_exceeded`. All rule families separately share `MAX_SEMANTIC_FINDINGS`, whose overflow returns `semantic_findings_limit_exceeded` without a truncated report. The checked 64 MiB estimate includes the maximum in-memory and serialized finding payload plus overlap candidate workspace; report persistence uses create-new temporary files plus no-clobber hard-link finalization.

## AI visual analysis adapter

`analyze-ai` is the explicit `ui_ai_visual_analysis_v1` boundary after deterministic normalization, metrics, region, and semantic analysis. Its strict bundle contains one to six captures. Every capture supplies exactly four PNG/JPEG artifacts (`reference`, `actual`, `overlay`, and `heatmap`), one `diff-metrics-report.json` schema v2, one `region-audit-report.json`, one `semantic-audit-report.json` schema v3, runtime UI metadata with semantic tree schema v3, allowed-difference notes, likely source files, and an explicit privacy policy. Diff artifact hashes/dimensions/byte lengths bind overlay and heatmap, region hashes bind reference and actual, and semantic metadata SHA-256 binds the finding report to the exact capture. Any swap or forgery fails before provider invocation. The adapter has fixed limits for input JSON, structured context, encoded image bytes, fully decoded pixels/memory, issues, evidence, strings, privacy rectangles, output tokens, and suggested files. Files are read once through max-plus-one bounded streams; image headers are preflighted and checked decoded pixel/byte budgets are reserved before the same snapshots receive a limits-constrained full decode.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- analyze-ai `
  --repository-root . `
  --allowed-input-root project/target/ui-visual-audit `
  --allowed-input-root tools/ui-visual-audit/fixtures/ai `
  --allowed-output-root project/target/ui-visual-audit `
  --bundle <ui-ai-analysis-bundle.json> `
  --config tools/ui-visual-audit/fixtures/ai/fixture.config.json `
  --output-directory project/target/ui-visual-audit/<new-ai-run>
```

The provider output is strict structured JSON: each issue carries `capture_id`, problem type, severity, problem, image evidence, declared region or screenshot bounds, optional reference element and semantic node ID, likely cause, and suggested files. The adapter rejects unknown fields, nonexistent capture/image/region/node references, out-of-bounds rectangles, and files not present in capture source evidence. Provider output has no pass or hard-failure downgrade field. Every deterministic semantic finding is copied unchanged into `deterministic_hard_failures`, while AI may add explanation or higher-severity issues. Stage 9 still owns the aggregate state and score gate.

`fixture` and `mock` are offline. `online` is disabled unless its config explicitly sets `enabled: true`, uses an absolute HTTPS endpoint without user info, query, or fragment, and resolves the named environment variable through the shared credential boundary. Its dedicated HTTP agent has redirects fixed to zero, treats 3xx as a classified invalid response, and sends a bounded `max_completion_tokens`. The implementation targets an OpenAI-compatible chat-completions request with strict JSON schema output; endpoint, account, key, and model are never hardcoded. Generation and audit model IDs are separate fields, the request operation remains `visual_analysis`, and `self_review_is_sole_conclusion` is always false.

Provider calls reuse the lightweight `ui-generation/provider-core` request, credential, timeout, cancellation, retry, rate-limit, and budget protocol. The audit dependency graph does not include `project` or Bevy. Online requests generate in-memory PNG copies with opaque masks from clipped semantic text rectangles plus explicit physical privacy rectangles; source artifacts are never modified, and visible non-clipped text without valid measured bounds fails closed before upload. Context and report distinguish source and provider-redacted SHA-256 and record mask/image/string/response redaction counts without image bytes. Credentials, prompts, image bytes, raw provider responses, and structured request context are never report/log fields. Sensitive metadata values are collected in a deterministic deduplicated set with fixed count, single-value, and total-byte limits while structural IDs and paths remain intact. Provider prose uses one multi-pattern matcher per response: ASCII text is matched case-insensitively, non-ASCII text remains exact, and email, phone, bearer, JSON token/password, and credential-pattern redaction runs afterward.

The repository validates fixture and mock behavior in normal tests. The online sample is intentionally ignored and was not run without credentials. To opt in explicitly:

```powershell
$env:UI_VISUAL_AUDIT_ONLINE_SAMPLE_ENDPOINT="https://<provider>/v1/chat/completions"
$env:UI_VISUAL_AUDIT_API_KEY="<secret>"
$env:UI_VISUAL_AUDIT_AUDIT_MODEL="<vision-model>"
cargo test --manifest-path tools/ui-visual-audit/Cargo.toml ai::tests::explicit_online_openai_compatible_sample -- --ignored --exact
```

Do not run the online sample on captures containing production account data or unreviewed personal text. The repository fixture and mock never make network requests.

## Four-state visual gate

`evaluate-gate` is the strict `ui_visual_gate_v1` aggregation boundary. A gate bundle binds every upstream report by path and lowercase SHA-256, then cross-checks the diff schema v2, region schema v1, semantic schema v3, and optional AI schema v1 protocol versions. It also checks aligned image hashes, dimensions, reference binding, region result/weight consistency, semantic finding counts and separation flags, AI source image hashes, and exact preservation of deterministic semantic hard failures. A malformed or forged evidence chain produces the terminal state `invalid`; it is never treated as an ordinary visual failure.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- evaluate-gate `
  --repository-root . `
  --allowed-input-root project/target/ui-visual-audit `
  --allowed-input-root tools/ui-visual-audit/fixtures `
  --allowed-output-root project/target/ui-visual-audit `
  --bundle <ui-visual-gate-bundle.json> `
  --config tools/ui-visual-audit/fixtures/gate/conservative.config.json `
  --output-directory project/target/ui-visual-audit/<new-gate-run>
```

The terminal states and process exit codes are `passed`/`0`, `needs_review`/`3`, `failed`/`4`, and `invalid`/`2`. Stable primary failure priority is invalid evidence, dimension mismatch, semantic hard failure, critical-region failure, severe AI issue, normal-region failure, medium AI issue, then decorative-region review. Severe and medium AI issues block; minor issues are retained but never block automatically. Critical and normal region failures block independently, while a decorative-only failure requests review. AI is optional and cannot downgrade deterministic evidence.

Each capture uses the Stage 8 identity `capture_id == screen.device.state`, names a reference profile, and binds its baseline; duplicate identities are rejected. `reference_profiles` may provide a complete six-metric threshold set for that exact binding; otherwise the strict `conservative_default` applies and the report records that fallback. Critical thresholds cannot be looser than normal, and normal cannot be looser than decorative. Maximum thresholds are inclusive, as is minimum SSIM. Every region retains raw, alpha, tolerated, SSIM, geometry, and large-area measurements; the applied profile threshold; profile violations; and separate upstream Stage 6 status/violations. Only the selected Stage 9 profile violations decide the gate. Upstream local results remain diagnostic and cannot silently make a deliberately relaxed reference profile fail. The diagnostic per-region quality floor is reported only to aid inspection. There is no global numeric score and no weighted average can hide a failed region.

`fixtures/gate/human-labeled-cases.json` contains sixteen repository-maintainer labels covering all six profile metrics plus deterministic synthetic boundary and precedence cases. It records zero false positives, zero false negatives, and zero four-state misclassifications for this fixture and defines the state ranking used by those counts. The test loads `fixtures/gate/conservative.config.json`, requires a unique bidirectional profile/calibration fixture link, derives each case threshold from the formal profile and named metric, pins the recorded threshold, and asserts the exact terminal state case by case. Its scope is deliberately narrow: it is an engineering regression set derived from committed fixtures, not a user study, production sample, or claim that thresholds are universally calibrated.

Gate config, bundle, and reports use strict unknown-field rejection. Capture/profile/count/string sizes, individual report bytes, and total evidence bytes are bounded. Output uses a new isolated directory and create-new temporary file plus no-clobber finalization; rerunning into an existing destination fails without changing its report.

## Comparison bundle and review report

`build-report` validates the strict `ui_comparison_bundle_v1` machine contract and creates `comparison-result.json` plus `report.md` in a new output directory. Each capture identity remains `screen.device.state` and must include reference, actual, overlay, and heatmap path + SHA-256 links; the six fixed-point metrics; region bounds, levels, thresholds, and status; masks and review reasons; allowed-difference notes; every algorithm version; whether AI actually ran; the four-state gate result; and located issues. Its baseline guard also binds a validated active reference manifest; the declared reference ID, screen/device/state, observed revision/hash, and captured reference artifact hash must all agree with that manifest. Issues keep an explicit region or `null`, severity, evidence image/rect, node ID, source path, likely files, likely cause, and suggested change scope. Unknown information stays `null`/`unknown`; the renderer does not invent source evidence.

```powershell
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- build-report `
  --repository-root . `
  --allowed-input-root summary/ui-audit `
  --allowed-input-root project/target/ui-visual-audit `
  --allowed-output-root summary/ui-audit `
  --bundle <comparison-input.json> `
  --output-directory summary/ui-audit/<run-id>/comparison
```

The root manifest first binds the comparison input path and SHA-256 together with the same run ID, analysis path/hash, and fix-iteration path/hash links. The generated result records the final root-manifest path/hash and the comparison-input path/hash, so validators can prove `root -> comparison input -> result -> root` without an impossible circular file hash. Every artifact is resolved below an allowed input root and hashed from a bounded read before Markdown is rendered. Missing artifacts, swapped hashes, incomplete root links, and stale baseline bindings fail with stable machine codes; no missing path is rendered as an ordinary successful link. Output is create-new/no-clobber and deterministic for the same structured evidence.

The repository runner records `artifact_links` for `analysis-input.json`, `analysis.json`, and completed fix iterations. It intentionally leaves `comparison = null` until the comparison engine has supplied the full four-image evidence bundle. Stage 10 does not imply Stage 11's automatic reference-manifest matrix expansion, CI task, or remote-device execution.

## Explicit baseline update workflow

Baseline replacement is never part of generation, AI analysis, report rendering, or the automatic fix loop. It uses three explicit commands:

```powershell
# 1. Read-only planning. The output directory must be new.
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- plan-baseline-update `
  --repository-root . `
  --manifest <reference-manifest.json> `
  --reference-id <reference-id> `
  --new-image <candidate.png> `
  --reason "reviewed visual refresh reason" `
  --metrics-before <before-metrics.json> `
  --metrics-after <after-metrics.json> `
  --allowed-output-root summary/ui-visual-audit `
  --output-directory summary/ui-visual-audit/<plan-id>

# 2. Apply only after a separately authored approval JSON binds the exact plan SHA-256.
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- apply-baseline-update `
  --repository-root . `
  --plan <baseline-update-plan.json> `
  --approval <human-approval.json> `
  --allowed-output-root summary/ui-visual-audit `
  --output-directory summary/ui-visual-audit/<apply-id>

# 3. Prove every related device/state was rerun before acceptance is complete.
cargo run --manifest-path tools/ui-visual-audit/Cargo.toml -- verify-baseline-rerun `
  --repository-root . `
  --receipt <baseline-update-receipt.json> `
  --comparison-result <comparison-result.json> `
  --allowed-output-root summary/ui-visual-audit `
  --output-directory summary/ui-visual-audit/<verification-id>
```

The immutable plan records the update reason, old/new image identity and dimensions, before/after metric report identities, prior/new revision binding, and every manifest entry sharing screen + locale + theme as a required device/state rerun. It always says `human_approval_required = true` and `automatic_fix_may_apply = false`. The separate approval JSON must use schema 1 and provide `plan_sha256`, `approved = true`, non-empty approver, timestamp, and rationale. The tool never writes or self-approves that record.

Apply fails on a stale manifest, changed candidate, changed metric evidence, wrong old binding, path conflict, or unapproved plan. It archives hash-bound old/new image evidence and stages the receipt before replacing any baseline file; receipt publication and post-update manifest validation run inside the manifest/image rollback transaction. Its receipt remains `applied_rerun_required`, `rerun_verification_required = true`, and `acceptance_complete = false`. Only `verify-baseline-rerun` can produce `acceptance_complete = true`, after a bound comparison result contains every required capture with its expected reference ID/revision/hash and an individually `passed` gate state. The runner's fix-command boundary rejects baseline plan/apply/verify entry points before execution and snapshots the committed reference root as forbidden state, including when Git ignores a changed path.
