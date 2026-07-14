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
