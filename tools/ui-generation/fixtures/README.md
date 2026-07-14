# UI generation fixtures

This directory contains reviewable, repository-owned text fixtures for the development-only UI
generation tool. `task.valid.json` is a contract example and intentionally references a nonexistent
`reference.example.png`; no third-party or unlicensed binary reference image is committed for Stage
1. Tests that exercise byte reads and SHA-256 verification create private temporary files.

Any future binary fixture must include its source, authorization, and license record in the task
metadata and follow the repository Git LFS rules. Tool fixtures must never be copied into
`project/assets/` merely to make a generation test pass.
