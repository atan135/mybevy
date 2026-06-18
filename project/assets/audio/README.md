# Audio Placeholder Assets

This directory contains a small development-only placeholder set copied from:

```text
E:\audiokinectic\SampleProject\Originals
```

The files are intentionally limited to a few WAV assets for audio framework testing. They are not final game content.

## Layout

- `ui/`: short placeholder UI feedback sounds.
- `common/`: common gameplay sounds such as footsteps.
- `ambience/`: loop-style environment beds.
- `music/`: loop-style background music samples.
- `battle/`: weapon, impact, and firing samples.
- `voice/`: short English voice samples.
- `spatial/`: samples useful for spatial audio tests.

## Notes

- Source Wwise project files such as `.ssm`, `.ssp`, `.mid`, `.amb`, and `.model` were not copied.
- File names were normalized to lowercase snake case for stable asset paths.
- Treat these files as placeholder resources for local development and internal test builds only.
- Before any public release, replace these placeholders with owned, commissioned, or clearly redistributable audio and record the license/source next to the asset or in `project/assets/licenses/`.
