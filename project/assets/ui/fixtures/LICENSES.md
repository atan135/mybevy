# UI Fixture Sources And Licenses

These files are development fixtures. They are not approved product artwork and must not be referenced by shipping screens.

## Generated image fixtures

The following PNGs were created for this repository from deterministic geometric primitives. They contain no copied pixels, logos, fonts, or external reference artwork:

- `visual-foundation/transparent-edge.png`
- `visual-foundation/non-square-2x1.png`
- `visual-foundation/nine-slice-12px.png`
- `visual-foundation/atlas-four-frames.png`

Copyright holder: MyBevy project contributors. Use is limited by the repository's project policy; no separate third-party license is required.

## Figtree font fixtures

Files:

- `fonts/FigtreeFixture-Regular.ttf` (weight 400)
- `fonts/FigtreeFixture-Medium.ttf` (weight 500)
- `fonts/FigtreeFixture-Bold.ttf` (weight 700)

Upstream family: Figtree by Erik Kennedy.

Pinned Google Fonts revision: `7888febb355d9e2c2c4aa357d20734e383f2198f`.

Upstream source:

- `https://raw.githubusercontent.com/google/fonts/7888febb355d9e2c2c4aa357d20734e383f2198f/ofl/figtree/Figtree%5Bwght%5D.ttf`
- `https://raw.githubusercontent.com/google/fonts/7888febb355d9e2c2c4aa357d20734e383f2198f/ofl/figtree/OFL.txt`

The three files are static instances of the upstream `wght` axis. No synthetic emboldening or duplicated font files are used. They are redistributed under SIL Open Font License 1.1; see `Figtree-OFL.txt` in this directory.

Upstream variable font SHA-256: `26ad3db9b31ff7dde67a91ff515d022d2f495cd506590699cf264f0bfe6fb714`.

Instantiation used fontTools 4.62.1 with `--update-name-table --no-recalc-timestamp`; the resulting static fonts have `OS/2.usWeightClass` values 400, 500, and 700 and no `fvar` table. Output hashes are recorded in `manifest.ron`.

These Latin fixtures do not replace `ui/fonts/MyBevyUiCjk-Regular.otf`. The font registry loads them only to exercise explicit 400/500/700 faces in the development UI Gallery; shipping screens must not select the `FigtreeFixture` family.
