# Switch 2 HD Rumble series for Eden-based emulators

One `git am` series that adds HD Rumble and DualSense haptics through `renhdrum.dll` to Eden-based Switch 2 emulators.
Built and played on Fritchii v5.8.4; applies to NovaS v0.2.1 with one hand-fixed CMake hunk.

| Patch | Contents |
|---|---|
| `0001` | HD Rumble and DualSense haptics through RenHDrum, settings dialog, `sdl3-hdrumble` package. **By [samaBR85](https://github.com/samaBR85/renhdrum-eden)** (GPL-3.0-or-later; author kept in the commit). |
| `0002` | `EDEN_HDRUMBLE_TRACE=1`: CSV trace of command 214 values (samaBR85, optional). |
| `0003` | RenHDrum header 0.3.1 (adds `renhdrum_set_nx2_mode`). |
| `0004` | Real command 213 (`SendVibrationValueInMode`); the Switch 2 Mode reaches RenHDrum. |
| `0005` | DualSense audio is started once and never shut down, so unplugging the pad no longer crashes. SDL3 patches on by default. |
| `0006` | Ignores bogus stick centre offsets (`abs > 0.5`), which made a Pro Controller stick drift. |

## Build
Add `-DENABLE_RENHDRUM=ON` (and leave `-DRENHDRUM_DOWNLOAD=OFF`). `RENHDRUM_SDL_PATCHES` is ON by default, so SDL3 is built from the `sdl3-hdrumble` package.
Put **RenHDrum 0.3.1** `renhdrum.dll` next to the executable. The `RENHDRUM_DOWNLOAD` option still fetches 0.2.1, which has no Mode 2 support.
Turn on "Enable Accurate Vibration" in Configure Vibration. Without the DLL every pad keeps the standard rumble.

## Applied to
| Core | Tag | Result |
|---|---|---|
| Fritchii | v5.8.4 | Built, played (Mario Kart World, wired DualSense and Pro) |
| NovaS | v0.2.1 | Applies; only the last hunk of `src/yuzu/CMakeLists.txt` needs pasting by hand. Not built yet |
| Lichii | build-150 | Does not apply: `sdl_driver.cpp` (1 hunk), `sdl_driver.h` (1), CMake (1) |
| Orchid | v0.9.5 | Does not apply: `sdl_driver.cpp` (4 hunks), CMake (1) |

The same series in `git am --3way` form fails on a clean clone without the original blobs; use `git am --reject` and paste the CMake hunk.

Credit: patches `0001` and `0002` are samaBR85's work, see [renhdrum-eden](https://github.com/samaBR85/renhdrum-eden).
