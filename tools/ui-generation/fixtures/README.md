# UI generation fixtures

This directory contains reviewable, repository-owned text fixtures for the development-only UI
generation tool. `task.valid.json` is a contract example and intentionally references a nonexistent
`reference.example.png`; no third-party or unlicensed binary reference image is committed for Stage
1. Tests that exercise byte reads and SHA-256 verification create private temporary files.

Any future binary fixture must include its source, authorization, and license record in the task
metadata and follow the repository Git LFS rules. Tool fixtures must never be copied into
`project/assets/` merely to make a generation test pass.

`providers/` contains non-sensitive, text-only provider response fixtures owned by this repository.
They exercise valid structured output, malformed output, a deliberately over-budget result for a
later validator, and an interrupted request. The fixtures contain neither credentials nor copied
model responses; their payloads were authored specifically for automated tests.

`preprocess.options.json` is a text-only Stage 3 options example. Every rectangle is expressed in
the full EXIF-normalized image's top-left pixel-edge coordinate system; no region is inferred from
image content. The example dimensions correspond to the documentation-only task placeholder and
do not cause the nonexistent `reference.example.png` to be read. Pixel/EXIF test images are
generated programmatically in private temporary directories.

`analysis/` contains repository-authored, non-sensitive Stage 4 structured-output fixtures. The
valid corpus covers a regular page, long scrolling list, gameplay HUD, and modal. It deliberately
uses placeholder hashes and contains no copied product art or third-party model output. The invalid
corpus isolates an unknown field, a 1176-character recognition candidate that exceeds the text
budget, and a parent cycle with no graph root. These files stay in the tool crate and are never
copied to `project/assets/`.

`generation/` contains repository-authored Stage 7 structured provider envelopes for a minimal
page, a nested page, a formally invalid `UiDocument`, and a valid draft with explicitly unsupported
behavior. They contain no prompt transcript, model response transcript, user image, or production
asset.

`preview/` contains repository-authored bare `UiDocument` inputs for the feature-gated Stage 8
standalone preview process. They contain no business action, binding, packaged asset, or generated
binary material.

`evaluation/catalog.v1.json` is the Stage 11 offline evaluation corpus. It has six
repository-authored synthetic text cases: login, list, HUD, modal, complex art panel, and a
phone/tablet multi-state document. Each case explicitly records expected components, key regions,
allowed differences, unsupported capabilities, device/state coverage, and a reviewer-role-only
human acceptance result. It references the existing structured fixtures by a path constrained to
this directory; it contains no binary reference image, personal data, account copy, prompt, or raw
provider response. `evaluate-fixtures` invokes the non-network `FixtureProvider` and revalidates
each formal artifact before emitting aggregate counts only.
