# Generic Recomp C Bridge for RenHDrum

A drop-in C/C++ bridge connecting Nintendo Switch static recompilation projects directly to **RenHDrum** (`renhdrum.dll` `0.4.x`), enabling authentic, low-latency **HD Rumble** for all four supported controllers, complete with a **Controls** menu toggle and per-pad compatibility with native recomp HD Rumble.

For deeper architecture notes, lock-ordering rules, and standalone Pro Controller fixes, see [`docs/integration/recomp-games.md`](../../docs/integration/recomp-games.md).

> [!NOTE]
> **Supported Controllers**: `✅ Nintendo Pro Controller` · `✅ DualSense` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)`.

---

## Directory Structure

```text
integrations/recomp-c/
├── bridge/
│   ├── renhdrum_recomp_bridge.c   # Worker threads, dynamic DLL/SDL loader, 4-controller bridge
│   └── renhdrum_recomp_bridge.h   # Minimal public C interface
├── reference/
│   └── recomp-c-reference.patch   # Complete reference patch against Hookshot v2.2.0-rc.2 (GPL-3.0)
└── README.md
```

---

## 1. What You Need

- **`renhdrum.dll` (`0.4.x`)** placed beside the compiled game executable (Windows x64).
- **`include/renhdrum.h`** (`RENHDRUM_VERSION` branch `0x0004xx`) on your compiler include path.
- **`bridge/renhdrum_recomp_bridge.c`** and **`bridge/renhdrum_recomp_bridge.h`** copied into your host runtime (e.g., `host/rh_bridge.c/.h`).

I designed the bridge to load both `renhdrum.dll` and SDL3/SDL2 functions dynamically (`LoadLibraryExW` + `GetModuleHandleA` + `GetProcAddress`), so no `.lib` import library is required at link time. After loading, the bridge calls `renhdrum_set_nx2_mode(2)` for Switch 2 vibration translation (use `0` for Switch 1 titles).

---

## 2. Safe DLL Loading & Version Check

Never load `"renhdrum.dll"` by bare filename (Windows searches CWD and `PATH` first). In `rh_load()`:

1. Build the absolute path next to the executable using `GetModuleFileNameW`.
2. Load it with `LoadLibraryExW(dll_path, NULL, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32)`.
3. Resolve core symbols (`renhdrum_version`, `renhdrum_engine_*`, `renhdrum_settings_default`, `renhdrum_set_nx2_mode`).
4. Enforce a matching `major.minor` branch:
   ```c
   uint32_t ver = rh.version();
   if ((ver >> 8) != ((uint32_t)RENHDRUM_VERSION >> 8)) {
       FreeLibrary(rh.dll);
       return 0; /* Reject e.g. 0.3.x and fall back cleanly */
   }
   ```
5. Resolve optional controller exports (DualSense DSP, Xbox 4-motor converter, Steam Controller 2026 grips) as "use if present" so missing optional symbols only degrade that specific controller type.

---

## 3. The Two Hook Points (`vibration_send` and Per-Pad Loop)

Register `rh_load()` at startup, call `rh_attach(gc)` / `rh_detach(gc)` on controller hotplug, and register an `atexit` handler that detaches and closes pads via `SDL_CloseGamepad` + `rh_cleanup()`.

Then insert two runtime hooks:

### Hook 1: Guest Vibration Call (`host/nn_hid.c:vibration_send`)
Place `rh_vibration` **before** `rumble_map` / `hos_sdl_rumble_hd`, passing the raw unscaled float parameters (`amp_low`, `freq_low`, `amp_high`, `freq_high`):

```c
if (rh_vibration(dev, v[0], v[1], v[2], v[3])) {
    return; /* Only skips fallback when ALL connected pads are claimed by RenHDrum */
}
```

Also wire `PermitVibration` → `rh_permit((int)ARG(0))` and ensure both `IsOunceVibrationDeviceMounted` HLE stubs return `1` for Switch 2 titles.

### Hook 2: Per-Pad Polling Loop (`host/pad_sdl.c:pads_poll`)
Sync the menu switch with `rh_set_enabled(rx_option(HOS_OPT_HD_RUMBLE, 1) != 0)`, then skip native HD Rumble and motor rumble **only** for slots owned by RenHDrum:

```c
if (rh_pad_claimed(i)) {
    continue; /* Prevents double rumble on claimed slots while leaving other pads untouched */
}
if (hd_poll(i, side, now)) continue;
```

---

## 4. The Four Supported Controllers and How Each Is Detected

1. **Nintendo Switch Pro Controller (`057E:2009`)**:
   - Matched by VID `0x057E`, PID `0x2009`.
   - Uses `RenHdcEngine` with `SDL_SendJoystickEffect` (`0x10` packets), 40 ms wired resend (`RH_RESEND_US`), and Bluetooth **Level 4 (Compatible)** by default (`HOOKSHOT_BT_LEVEL=1..4` override) so it works on unpatched SDL3.
2. **Sony DualSense / DualSense Edge (`054C:0CE6` / `0x0DF2`)**:
   - Matched by VID `0x054C`, PID `0x0CE6` or `0x0DF2`.
   - **USB**: Opens a 4-channel 48 kHz `SDL_AUDIO_F32` playback stream on `"DualSense"` / `"Wireless Controller"` (`channels >= 4`). Calls `SDL_PumpEvents()` before device enumeration so hotplugged pads appear without an SDL window, and retries every 2 seconds if the speaker endpoint arrives late.
   - **Bluetooth**: Opens the HID path and runs a high-resolution timer thread sending 142-byte report `0x32` every `10.667 ms` (`RENHDRUM_DS5_BT_PERIOD_NS`) with a 2-second silence gate.
3. **Microsoft Xbox One S (`XBOXONE` type or VID `0x045E`)**:
   - Matched by `SDL_GetGamepadType(pad) == 3` (`XBOXONE` in SDL3; `2` in SDL2) or VID `0x045E`.
   - Uses `renhdrum_xbox_rumble_*`. All motor ratios and curves come directly from the core (`XBOX_DEFAULT` / `PRO_DEFAULT` in Mode 2) — the bridge never sets custom motor numbers.
   - Worker calls `SDL_RumbleJoystick` (grips, 2000 ms) and `SDL_RumbleJoystickTriggers` (triggers, 200 ms short pulse), calls `renhdrum_xbox_rumble_resend` if SDL refuses the first command, and zeroes all 4 motors on detach.
4. **Steam Controller 2026 (`0x28DE`, PIDs `0x1302` / `0x1303` / `0x1304`, Experimental)**:
   - Matched by VID `0x28DE` and PID `0x1302` (wired, 8 kHz mu-law), `0x1303` (Bluetooth, 4 kHz), or `0x1304` (dongle, 4 kHz).
   - **Steam Input stand-in check**: Never claim a pad with `SDL_GetGamepadSteamHandle(pad) != 0`. If a stand-in pad is present or opens later, stop `rh_sv_worker` and hand rumble back to Steam Input to prevent double rumble.
   - Sends `0x86` mode enable (`channels 2, 5`), streams 64-byte `0x88` reports while active + `100 ms` tail, and falls back after 100 consecutive write failures. Marked **experimental** everywhere.

---

## 5. Toggling HD Rumble & Locking Intensity

In the Hookshot `v2.2.0-rc.2` reference patch (`gpu/vk/vk_menu.cpp`, `gpu/vk/vk_replay.cpp`):

- Add an **HD Rumble** row (`opts.hd_rumble = 1`, `HOS_OPT_HD_RUMBLE`, `preset_exempt = true`) directly above **Rumble strength**.
- Toggling it off calls `rh_set_enabled(0)`, detaching active engines and clearing `rh_pad_claimed` without forgetting connected pads; toggling it back on re-attaches them immediately.
- When `opts.hd_rumble != 0`, lock **Rumble strength** (`row_fixed`) and display `"-"` while preserving the user's saved INI value.
- Display the live 1-line controller state from `rh_status_line()` in the Controls hint/help area.

---

## 6. Coexisting with a Recomp's Native HD Rumble

Hookshot `v2.2.0-rc.2` already ships native HD Rumble for Pro and Joy-Con (`host/hd_rumble.h`, `hd_poll`). To coexist safely:

- Gate `hd_poll()` and `rumble_due()` per slot with `if (rh_pad_claimed(i)) continue;`.
- Return `1` from `rh_vibration()` only when every connected pad is claimed (`rh_all_pads_claimed()`), so unclaimed secondary pads (e.g., Joy-Con) continue receiving native rumble.
- Turning **HD Rumble** off yields all slots immediately back to the recomp's native HD Rumble.

---

## 7. Building `renhdrum.dll` with `crt-static`

`RenHDrum`'s `.cargo/config.toml` sets `rustflags = ["-C", "target-feature=+crt-static"]` for `x86_64-pc-windows-msvc`. Always run `cargo build --release` from inside the `RenHDrum` directory (or pass `RUSTFLAGS="-C target-feature=+crt-static"`), then verify with `dumpbin /dependents target\release\renhdrum.dll` that `VCRUNTIME140.dll` is absent. Bundle `renhdrum.dll`, `SDL2.dll`, and `SDL3.dll` beside the game executable (`nx2/build.py` and `nx2/make_game.py`).

---

## 8. Self-Test Checklist

- Missing `renhdrum.dll` and `0.3.x` DLLs are rejected cleanly with fallback to host rumble.
- Missing optional exports (`renhdrum_valve_grip_*`, `renhdrum_xbox_rumble_*`) degrade only that pad type.
- Controls menu shows `"HD Rumble"` above `"Rumble strength"`, locks `"Rumble strength"` to `"-"` when on, persists `hd_rumble` in INI, and updates `rh_status_line()`.
- All 4 controllers (Pro USB/BT, DualSense USB hotplug/BT, Xbox 4-motor grips+triggers, Steam Controller 2026 + Steam Input stand-in handoff) behave as expected, and the 250 ms silence watchdog stops hung rumbles on pause.
- Process exit calls `SDL_CloseGamepad` so wired Pro Controllers leave `0x30` mode cleanly.

---

## 9. Lessons from PR #95 (`Goldoire/fefw-recomp`)

Six checks distilled from the `Goldoire/fefw-recomp#95` review (`v0.1.7`, `renhdrum.dll` `0.3.0`):

1. **Full DLL path beside exe**: Use `GetModuleFileNameW` + `LoadLibraryExW` (`LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`), never a bare filename.
2. **`crt-static` builds**: Avoid `VCRUNTIME140.dll` dependency so players without VC++ Redistributable do not lose haptics silently.
3. **Honor the master rumble-off toggle**: Stop both RenHDrum and motor rumble at the single dispatch gate.
4. **Scale intensity once & reuse streams**: `volume_percent` in `renhdrum_dualsense_dsp_create` is already the final gain; never multiply samples again, and keep the audio stream open if recreating the DSP.
5. **Bluetooth through the core's limiter**: Always pace Pro Bluetooth inside `RenHdcEngine` (default Level 4 Compatible on unpatched SDL3) to prevent input lag.
6. **Layered fallbacks & lock order**: Never hold the host controller mutex while waiting on worker threads, and fall back to standard rumble across every device failure path.

---

## Reference Patch (`Hookshot v2.2.0-rc.2`)

[`reference/recomp-c-reference.patch`](reference/recomp-c-reference.patch) applies cleanly to Hookshot `v2.2.0-rc.2`:

```bash
git checkout v2.2.0-rc.2
git apply integrations/recomp-c/reference/recomp-c-reference.patch
```

## License

- Bridge implementation (`renhdrum_recomp_bridge.c`, `renhdrum_recomp_bridge.h`): **MIT OR Apache-2.0**.
- Hookshot reference patch (`reference/recomp-c-reference.patch`): **GPL-3.0**. Credit to **NotRexed** and the Hookshot contributors.
