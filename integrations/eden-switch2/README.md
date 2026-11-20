# Switch 2 HD Rumble series for Eden-based emulators

One `git am` series that adds HD Rumble and DualSense haptics through `renhdrum.dll` to Eden-based Switch 2 emulators.
Built and played on Fritchii v5.8.4; applies to NovaS v0.2.1 with one hand-fixed CMake hunk.

| Patch | Contents |
|---|---|
| `0001` | HD Rumble and DualSense haptics through RenHDrum, settings dialog, `sdl3-hdrumble` package. **By [samaBR85](https://github.com/samaBR85/renhdrum-eden)** (GPL-3.0-or-later; author kept in the commit). |
| `0002` | `EDEN_HDRUMBLE_TRACE=1`: CSV trace of command 214 values (samaBR85, optional). |
| `0003` | RenHDrum header 0.4.0 (adds `renhdrum_set_nx2_mode`; Steam Controller grip functions added in `0018`). |
| `0004` | Real command 213 (`SendVibrationValueInMode`); the Switch 2 Mode reaches RenHDrum. |
| `0005` | DualSense audio is started once and never shut down, so unplugging the pad no longer crashes. SDL3 patches on by default. |
| `0006` | Ignores bogus stick centre offsets (`abs > 0.5`), which made a Pro Controller stick drift. |
| `0007` | The HD Rumble dialog shows only the on/off switch; the tuning values are fixed to the tested defaults unless `hd_rumble_advanced=true` is set by hand in `qt-config.ini`. |
| `0008` | The dialog lists the supported controllers (Switch Pro / Joy-Con, DualSense, Xbox), marks the connected one, and has an on/off switch for DualSense and Xbox. |
| `0009` | A wired DualSense keeps looking for its speaker every 5 s after the quick attempts, restarts audio haptics cleanly when re-enabled, and the log shows the Switch 2 rumble mode the game sends. |
| `0010` | Pro Controller / Joy-Con over Bluetooth default to level 3 (superseded by `0011`; kept in series for patch history). |
| `0011` | Bluetooth HD Rumble exposes two modes: **Detailed (5 ms)** (default, runs the 5 ms envelope at any level) and **Compatible** (level 4 fallback, newest value every 32 ms, 40 ms resend). Removes the level-3 lock, hides the legacy level selector in the advanced dialog, and logs the active mode. |
| `0012` | Switch 2 Mode 2 keeps the default exponential curve as the fallback when RenHDrum does not take the vibration. |
| `0013` | Hands a Switch controller back to the standard rumble after 8 refused packets when SDL does not take HD Rumble packets. |
| `0014` | Each wired DualSense claims its own 4-channel speaker instead of sharing the first one. |
| `0015` | DualSense Bluetooth haptics automatically restart every 10 s after the wireless link drops. |
| `0016` | A wired DualSense falls back to the standard rumble and keeps looking if its speaker disappears while the pad stays connected. |
| `0017` | Destroys a lost DualSense speaker stream under the pad lock so it cannot race with teardown or a DSP rebuild. |
| `0018` | Steam Controller (2026) grips play both HD Rumble bands (experimental; protocol and measurements by iafx, PCM stream found by LiveHaptics). |
| `0019` | Lists the Steam Controller (2026) as experimental in the HD Rumble dialog. |
| `0020` | Exposes connected RenHDrum pads, runtime library version (`renhdrum_version()`), and per-kind toggles (`switch_hd_rumble_enabled`, `steam_hd_rumble_enabled`, `hd_rumble_on_connect`) and reads `hd_rumble_bt_fast` directly. |
| `0021` | Neutral shared Qt HD Rumble dialog (live controller list, Bluetooth mode & per-type toggles, two-phase test rumble + rumble on connect, scrubbed zip problem report, and `Powered by RenHDrum <version>` footer; supersedes the UI of `0007`/`0008`/`0019` without modifying existing patches). |
| `0022` | Keeps short (< 15 ms) vibration pulses when HD Rumble is enabled even if "Enable Accurate Vibration" is off. |
| `0023` | Compacts the shared HD Rumble dialog (status on the enable row, auto-sized pad list without group box, single `Controllers` group with tooltips). |
| `0024` | Makes a newly connected RenHDrum controller Player 1 when Player 1 is not on a connected controller (always on, status bar message), so HD Rumble works without a visit to Settings → Controls. |
| `0025` | **Opt-in, off by default.** A wired DualSense plays the high-band haptics on its body speaker and a held trigger kicks on strong hits, only when `dualsense_bo3.txt` sits next to the exe (line 1 = speaker %, default 60; line 2 = `1` turns the trigger kick on). Needs a `renhdrum.dll` with `renhdrum_dualsense_dsp_set_speaker` and `renhdrum_tri_trigger_*`. Built on top of `0023`; touches only `renhdrum.h` and `renhdrum_pad.cpp`, so it is independent of `0024` (plan 118 step 5). |
| `0026` | Fixes a trigger that could keep vibrating: only `Tick` sends the trigger packets now (`Vibrate` just stores the newest sample), and a mutex keeps packet order equal to state-machine order. Needs `0025`; touches only `renhdrum_pad.cpp`. Pairs with the RenHDrum core of plan 118 step 8a (no C ABI change) (plan 118 step 8b). |
| `0027` | The trigger kick ignores rumble while the player presses another button (face, shoulder, stick, D-pad, or the other trigger): the host calls the new optional `renhdrum_tri_trigger_note_action` just before `tri_update`; missing symbol is skipped. Needs `0025` + `0026` and RenHDrum with the 10a core; touches `renhdrum_pad.cpp` and the bundled `externals/renhdrum/renhdrum.h` (plan 118 step 10a). |
| `0028` | **HD Haptics on by default + new settings page** (plans 119 step 7 and 120 step 4). The DualSense bundle of `0025`-`0027` no longer needs `dualsense_bo3.txt`: it is on whenever HD Rumble and the **HD Haptics** switch are (USB: speaker + trigger kick; Bluetooth: rumble + triggers in one report `0x36`; the speaker joins that report with core 0.5.0, see `0031`), and the switch applies while the game runs. All blocks come from the core (`renhdrum_hd_haptics_*`, `renhdrum_dualsense_dsp_build_bt36_report`). The HD Rumble dialog is drawn from the core's settings table (`integrations/qt/renhdrum_settings_dialog.*`); every value is saved as `key=value` in `qt-config.ini` section `[RenHDrum]` (`Settings::values.renhdrum_values`), the old `hd_rumble_*` / `dualsense_*` / `xbox_*` settings (per-game intensity included) are gone, and the driver reads the core's value store (`renhdrum_store_*`, `renhdrum_xbox_direct_*`) instead of its own `ShapeAmplitude` / Xbox Direct code. Needs `0025`-`0027` and a `renhdrum.dll` from the same plan (settings table + `renhdrum_hd_haptics_*`). Paths are Lichii's (`src/lichii/...`); touches `settings.h`, `frontend_common/config.*`, `hid_core/frontend/emulated_controller.cpp`, `sdl_driver.cpp`, `renhdrum_pad.*`, `externals/renhdrum/renhdrum.h`, and the dialog files. |
| `0029` | Settings page: help text drawn in the theme text colour at reduced alpha instead of `palette(mid)` so it stays readable. Needs `0028`; touches only `renhdrum_settings_dialog.cpp`. |
| `0030` | Steam Controller (2026) uses the core's `0x86` PCM mode manager (`renhdrum_valve_grip_mode_tick` / `_off`) and opt-in 40 ppm drift skip (`renhdrum_valve_grip_dsp_create_ex`), and yields to Steam Input virtual stand-in gamepads (`SDL_GetGamepadSteamHandle`); updates `externals/renhdrum/renhdrum.h` and `renhdrum_pad.cpp`. |
| `0031` | Hardens the DualSense reliability paths (`0014`–`0017`) with HD Haptics and Bluetooth Opus speaker support (`dsp_set_speaker` synced over Bluetooth `0x36` and reset when a lost USB speaker clears, re-verify `joystick && dsp` before Bluetooth restart, and full lock-ordering note). |
| `0032` | Adds the shared Song Rung theme (`src/qt_common/brand/song_rung_theme.*`), `AmbientBackdrop` (`ambient_backdrop.*`, ported from Brontide, GPL-3.0-or-later), embedded IBM Plex fonts (+ `IBM-Plex-OFL.txt`), RenHDrum logos, and thin QSS resource (`song_rung_brand.qrc`). |
| `0033` | Updates `renhdrum_settings_dialog.*` with Song Rung cards, toggle switches, monospace sliders, and the 18 px RenHDrum logo + 70% opacity credit footer (`src/lichii/...`; map to `src/yuzu/...` for Fritchii). |
| `0034` | Settings → Controls: the `HD Rumble` button becomes a Song Rung pill (`qt_common/brand/rumble_pill.*`, port from Brontide: wave icon, light streak running around the border, faster on hover, flash on click; the clock stops when hidden or minimised) and sits below the controller picture instead of on top of the analog test circles (`src/lichii/...`; map to `src/yuzu/...` for Fritchii; apply with `git am -C1` there). |

## Files to ship next to the exe
- `renhdrum.dll` 0.5.0 (or newer).
- Microsoft Visual C++ Redistributable x64 (`vc_redist.x64.exe`): the emulator needs `MSVCP140` / `VCRUNTIME140`.
- `recipes\*.nx2recipe` (Fritchii only): data for pre-building shaders, not in the repo. Without it the game runs but stutters on new scenes.
- Copy them all with `tools\sw2-kit.ps1 -Dest <exe folder> -Repo <core source>` in the RenHDrum workshop.

## Build
Add `-DENABLE_RENHDRUM=ON` (and leave `-DRENHDRUM_DOWNLOAD=OFF`). `RENHDRUM_SDL_PATCHES` is ON by default, so SDL3 is built from the `sdl3-hdrumble` package.
Put **RenHDrum 0.5.0** `renhdrum.dll` next to the executable. The `RENHDRUM_DOWNLOAD` option still fetches 0.2.1, which has no Mode 2 support.
With patch `0022` applied and HD Rumble enabled, short (< 15 ms) vibration pulses are kept automatically even with "Enable Accurate Vibration" off (enable it only if applying earlier patches without `0022`). Without the DLL every pad keeps the standard rumble.
With two wired DualSense controllers the speakers are matched in listing order; plug them in one after the other if the rumble lands on the wrong pad.

## Applied to
| Core | Tag | Result |
|---|---|---|
| Fritchii | v5.8.4 | Built, played (Mario Kart World, wired DualSense and Pro) |
| NovaS | v0.2.1 | Applies (`0001` needs the last hunk of `src/yuzu/CMakeLists.txt` pasted by hand; `0007`-`0009` apply cleanly). Not built yet |
| Lichii | build-160 | Applies with `git am --3way` once the Fritchii blobs are fetched; `0001` needs `PumpEvents()` merged by hand (Lichii added a motion-sensor restart there, so it stays non-const). Not built yet |
| Lichii | build-177 | Applied manually (`src/yuzu/` → `src/lichii/`, PR #1497 DualSense haptics yielded when HD Rumble owns the pad, and `0022` keeps Lichii's `IsVibrationStreamed()` check); `0024` exported from it with paths changed to `src/yuzu/` |
| Orchid | v0.9.5 | Does not apply: `sdl_driver.cpp` (4 hunks), CMake (1) |

The same series in `git am --3way` form fails on a clean clone without the original blobs; use `git am --reject` and paste the CMake hunk.

Credit: patches `0001` and `0002` are samaBR85's work, see [renhdrum-eden](https://github.com/samaBR85/renhdrum-eden).
