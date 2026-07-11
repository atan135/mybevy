# UI Font Assets

## Product CJK face

`MyBevyUiCjk-Regular.otf` is the product UI Regular face. It was introduced as a subset derived from Noto Sans CJK SC Regular and is redistributed under SIL Open Font License 1.1; see `NotoSansCJKsc-LICENSE.txt`.

- Runtime path: `ui/fonts/MyBevyUiCjk-Regular.otf`
- Size: 9,207,028 bytes
- SHA-256: `7674e60e1b3f1898d1063c7a9e57172e50d3efdb182b17b99ad92dabc73244bb`
- Declared framework coverage: Basic Latin, Latin-1/Extended A-B, general punctuation, currency symbols, CJK punctuation, CJK Unified Ideographs `U+4E00..U+9FFF`, and common full-width forms.
- Explicit exclusions: CJK extensions, emoji, Japanese kana, Korean Hangul, and language-specific Traditional Chinese extensions.

The original upstream revision and subset command were not recorded when this existing asset entered the repository. The checked-in hash is therefore the reproducible authority for the current face; do not claim upstream-reproducible provenance until that gap is repaired.

## Development fixture family

Figtree Regular/Medium/Bold lives under `../fixtures/fonts/`. Those three static faces are pinned, hashed, and licensed in `../fixtures/manifest.ron` and `../fixtures/LICENSES.md`. They are loaded by the registry for Gallery verification only and are not product fonts.
