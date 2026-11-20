# Song Rung Colour Palette

Source: `.ai/research/bo-ao-brontide-ho-so.md` (Section 2). Fusion base style + `QPalette` + thin QSS (`song-rung.qss`).

## Shared UI Palette

| Role | Dark | Light | QPalette / Token |
|---|---|---|---|
| Window background | `#151413` | `#F7F5F2` | `Window` / `@WINDOW@` |
| Surface (cards, inputs, lists) | `#201E1C` | `#FFFFFF` | `Base`, `Button` / `@SURFACE@` |
| Raised surface (alternate rows, tooltips) | `#282522` | `#F0ECE6` | `AlternateBase`, `ToolTipBase` |
| Text | `#F3EFEA` | `#1E1B18` | `WindowText`, `Text`, `ButtonText` / `@TEXT@` |
| Muted / secondary text | `#A39B93` | `#665E57` | `PlaceholderText` / `@MUTED@` |
| Disabled text | `#706962` | `#9B938B` | `Disabled` text roles / `@DISABLED@` |
| Accent (selection, active switch, slider) | `#E68A2E` | `#C46812` | `Highlight`, `Link` / `@ACCENT@` |
| Text on accent | `#151413` | `#FFFFFF` | `HighlightedText` / `@ON_ACCENT@` |
| Soft accent (row selection, hover, pill) | `#42301E` | `#FDEBD6` | `@ACCENT_SOFT@` |
| Danger / error | `#E05252` | `#C93B3B` | `@DANGER@` |
| Divider / border line | `#2E2B28` | `#E6E1DB` | `Mid` / `@LINE@` |
| Active status dot | `#65A30D` | `#4D7C0F` | `@OK@` |

## Per-App Identity Accents (icon and header accent strip only)

- **Brontide-NS2:** `#38BDF8` (primary arc) / `#1E4E68` (inner shade)
- **Brontide-oot:** `#E68A2E` (primary arc) / `#65A30D` (moss inner shade)
