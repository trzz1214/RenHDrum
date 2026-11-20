# RenHDrum settings page for Qt Widgets

`renhdrum_settings_dialog.h/.cpp`: one `QWidget` (`RenHDrumQt::SettingsWidget`) that draws the HD Rumble settings the way `docs/integration/ui-guidelines.md` describes. Only Qt Widgets + `renhdrum.h`; no emulator code. Rows come from `renhdrum_options`, so a newer core can add a row without rebuilding the host.

Put it in a dialog or a tab. Colours are `palette(...)` only.

## What the host gives (`RenHDrumQt::Host`)
- `api`: the core functions (`options`, `store_create/destroy/get/set/reset/is_recommended`). Link renhdrum, or fill them from the DLL symbols.
- `active_pad`: polled about once a second; returns the device bit (1 Switch, 2 DualSense, 4 Xbox, 8 Steam, 0 none), Bluetooth or not, and the name.
- `load(key, &value)` / `save(key, value)`: your storage (an ini section `[RenHDrum]`, one `key=value` per row; skip keys you do not know).
- Optional: `test_rumble`, `xbox_extra_row` (a host-only switch for trigger rumble), `report_problem`, `credit_suffix`, `master_changed`.

## At run time
Your driver keeps its own `RenHdcStore`, fills it with the same saved values (`renhdrum_store_set`) and reads `renhdrum_store_to_config`, `renhdrum_store_shape_amplitude`, `renhdrum_xbox_direct_*`. The page never talks to the driver; the saved values are the link.

Used first in Brontide-NS2 (`src/lichii/configuration/renhdrum_settings_dialog.*`, patch `0028`), then Brontide-oot and Brontide.

## Look and brand
- `song_rung_theme.*`: the shared "Wave" theme (cards, toggle switches, monospace sliders, light and dark).
- `ambient_backdrop.*`: a backdrop that takes its colour from the selected game.
- `rumble_pill.*`: the **HD Rumble** button for the Controls page, a pill with a glowing outline; it sits below the controller picture so it does not cover the stick test circles.
- `brand/`: the RenHDrum logo (light and dark), icons and fonts. The panel ends with the RenHDrum logo and **Powered by RenHDrum**; see `docs/integration/ui-guidelines.md`, section 6.
- The HD Haptics row (`dualsense_hd_haptics`) and every other row come from the core table, so the page needs no change when a row is added.
