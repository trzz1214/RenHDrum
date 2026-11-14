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

## Files to ship next to the exe
- `renhdrum.dll` 0.4.0 (or newer).
- Microsoft Visual C++ Redistributable x64 (`vc_redist.x64.exe`): the emulator needs `MSVCP140` / `VCRUNTIME140`.
- `recipes\*.nx2recipe` (Fritchii only): data for pre-building shaders, not in the repo. Without it the game runs but stutters on new scenes.
- Copy them all with `tools\sw2-kit.ps1 -Dest <exe folder> -Repo <core source>` in the RenHDrum workshop.

## Build
Add `-DENABLE_RENHDRUM=ON` (and leave `-DRENHDRUM_DOWNLOAD=OFF`). `RENHDRUM_SDL_PATCHES` is ON by default, so SDL3 is built from the `sdl3-hdrumble` package.
Put **RenHDrum 0.4.0** `renhdrum.dll` next to the executable. The `RENHDRUM_DOWNLOAD` option still fetches 0.2.1, which has no Mode 2 support.
Turn on "Enable Accurate Vibration" in Configure Vibration. Without the DLL every pad keeps the standard rumble.
With two wired DualSense controllers the speakers are matched in listing order; plug them in one after the other if the rumble lands on the wrong pad.

## Applied to
| Core | Tag | Result |
|---|---|---|
| Fritchii | v5.8.4 | Built, played (Mario Kart World, wired DualSense and Pro) |
| NovaS | v0.2.1 | Applies (`0001` needs the last hunk of `src/yuzu/CMakeLists.txt` pasted by hand; `0007`-`0009` apply cleanly). Not built yet |
| Lichii | build-160 | Applies with `git am --3way` once the Fritchii blobs are fetched; `0001` needs `PumpEvents()` merged by hand (Lichii added a motion-sensor restart there, so it stays non-const). Not built yet |
| Orchid | v0.9.5 | Does not apply: `sdl_driver.cpp` (4 hunks), CMake (1) |

The same series in `git am --3way` form fails on a clean clone without the original blobs; use `git am --reject` and paste the CMake hunk.

Credit: patches `0001` and `0002` are samaBR85's work, see [renhdrum-eden](https://github.com/samaBR85/renhdrum-eden).
