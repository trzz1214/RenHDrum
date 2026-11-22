# Generic Recomp C Bridge for RenHDrum

A drop-in C/C++ bridge connecting Nintendo Switch static recompilation projects directly to **RenHDrum** (`renhdrum.dll` `0.5.0`), enabling authentic, low-latency **HD Rumble** and **DualSense HD Haptics** across all four supported controllers, complete with a core-driven **HD Rumble** settings store (`renhdrum.ini`) and per-pad compatibility with native recomp rumble.

For deeper architecture notes, lock-ordering rules, standalone Pro Controller fixes, and [real-world lessons from the Longshot and totk-pc-recomp ports](../../docs/integration/recomp-games.md#11-lessons-from-the-longshot-and-totk-pc-recomp-ports), see [`docs/integration/recomp-games.md`](../../docs/integration/recomp-games.md).

> [!NOTE]
> **Supported Controllers**: `✅ Sony DualSense / Edge (HD Haptics)` · `✅ Nintendo Pro Controller` · `✅ Xbox One S (RenHDrum & Direct)` · `🧪 Steam Controller 2026 (experimental)`.

---

## Directory Structure

```text
integrations/recomp-c/
├── bridge/
│   ├── renhdrum_recomp_bridge.c   # Worker threads, dynamic DLL/SDL loader, 4-controller bridge, INI store
│   └── renhdrum_recomp_bridge.h   # Public C/C++ interface
├── reference/
│   ├── recomp-c-reference.patch   # Legacy reference patch against Hookshot v2.2.0-rc.2 (GPL-3.0; Hookshot is no longer supported, use longshot-reference.patch for Ocarina of Time)
│   ├── longshot-reference.patch   # Reference patch against Longshot (GPL-3.0-or-later)
│   ├── totk-reference.patch       # Reference patch against totk-pc-recomp v0.4.1 (MIT)
│   └── dkb-reference.patch        # Reference patch against dkb-pc-recomp v0.5.1 (GPL-3.0-or-later)
└── README.md
```

---

## 1. What You Need

- **`renhdrum.dll` (`0.5.0`)** placed beside the compiled game executable (Windows x64, built with `+crt-static`).
- **`include/renhdrum.h`** (branch matching `RENHDRUM_VERSION >> 8`, `0x0005xx` for `0.5.0`) on your compiler include path (and alongside the bridge header).
- **`bridge/renhdrum_recomp_bridge.c`** and **`bridge/renhdrum_recomp_bridge.h`** copied into your host runtime (e.g., `host/rh_bridge.c/.h` or `runtime/renhdrum_recomp_bridge.c/.h`).

The bridge resolves both `renhdrum.dll` and SDL3/SDL2 exports dynamically (`LoadLibraryExW` + `GetModuleHandleA` + `GetProcAddress`), so no `.lib` import library is required at link time.

### Switch Generation (`rh_set_switch_gen`)

Call `rh_set_switch_gen(gen)` **before** `rh_load()`:
- `rh_set_switch_gen(1)` for **Switch 1** titles (`renhdrum_set_nx2_mode(0)`).
- `rh_set_switch_gen(2)` for **Switch 2** titles (`renhdrum_set_nx2_mode(2)`, default).

---

## 2. Safe DLL Loading & Version Check

Never load `"renhdrum.dll"` by bare filename (Windows searches CWD and `PATH` first). In `rh_load()`:

1. Build the absolute path next to the executable using `GetModuleFileNameW`.
2. Load it with `LoadLibraryExW(dll_path, NULL, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32)`.
3. Resolve the 8 required core exports (`renhdrum_version`, `renhdrum_engine_*`, `renhdrum_settings_default`, `renhdrum_set_nx2_mode`).
4. Enforce an exact `major.minor` branch match against `RENHDRUM_VERSION`:
   ```c
   uint32_t ver = rh.version();
   if ((ver >> 8) != ((uint32_t)RENHDRUM_VERSION >> 8)) {
       /* Unload DLL, claim no controllers, report mismatch via rh_status_line() */
       FreeLibrary(rh.dll);
       return 0;
   }
   ```
   When the DLL is missing, rejects the version check, or lacks a required export, `rh_load()` claims no pads and `rh_status_line()` reports the exact reason while the game falls back to its built-in rumble.
5. Resolve optional exports (`renhdrum_options`, `renhdrum_store_*`, DualSense HD Haptics, Xbox Direct, Steam `mode_tick`/`mode_off`) as "use if present".

---

## 3. Public C Interface & Safe Exit (`rh_shutdown`)

### Legacy Lifecycle & Vibration Functions (Unchanged Signatures)
- `int rh_load(void)` — Loads `renhdrum.dll`, checks version branch, initializes `RenHdcStore` and `renhdrum.ini`.
- `void rh_attach(void *pad)` / `int rh_detach(void *pad)` — Hotplug hooks on controller open/close.
- `int rh_vibration(int side, float al, float fl, float ah, float fh)` — Feeds raw unscaled guest amplitudes and frequencies.
- `void rh_permit(int on)` — Pushes neutral `(0, 160, 0, 320)` to both sides when `on == 0`.
- `int rh_is_active(void *pad)` / `int rh_pad_claimed(int i)` — Per-pad ownership queries.
- `void rh_set_enabled(int on)` — Master enable/disable without forgetting tracked pads.
- `const char *rh_status_line(void)` — Live 1-line status (or DLL load/version error) for Controls UI.
- `int rh_ds_render_frames(float *out, int frames)` — Renders 4-channel 48 kHz frames from the first active DualSense DSP.
- `void rh_cleanup(void)` — Delegates to `rh_shutdown()`.

### New `0.5.0` Functions
- `void rh_set_switch_gen(int gen)` — `1` for Switch 1 (`nx2_mode = 0`), `2` for Switch 2 (`nx2_mode = 2`, default). Call before `rh_load()`.
- `void rh_shutdown(void)` — Idempotent, thread-safe shutdown safe to call multiple times and directly from process exit paths **before** `ExitProcess` / `TerminateProcess` (do **not** rely on `atexit`, which `ExitProcess`/`TerminateProcess` bypasses). Stops all worker threads, zeroes Pro/Xbox/DualSense motors, releases DualSense speaker and adaptive triggers, sends `0x86` off (`renhdrum_valve_grip_mode_off`) for Steam Controller 2026, and closes HID handles.
- `int rh_option_count(void)` — Returns the number of rows exposed by `renhdrum_options()`.
- `const RenHdcOption *rh_option_at(int i)` — Returns metadata (`key`, `label`, `help`, `choices`, `kind`, range, `devices`, `tier`, `link`, `requires_row`) for row `i`.
- `int rh_option_get(const char *key, int *out)` — Reads the current integer value from the bridge's `RenHdcStore`.
- `int rh_option_set(const char *key, int value)` — Updates the store, applies changes live (`renhdrum_store_to_config` + `renhdrum_engine_apply_settings`, DualSense DSP/HD Haptics, Xbox/Steam), and saves `renhdrum.ini`.
- `void rh_option_reset(void)` — Resets all core options to recommended defaults, applies live, and saves `renhdrum.ini`.

### Settings File (`renhdrum.ini`)
The bridge maintains a single `RenHdcStore` and reads/writes `renhdrum.ini` next to the game executable (`key=value` format under `[RenHDrum]`, using the keys from `renhdrum_options()`). When `renhdrum.ini` does not exist, all options start at the core's recommended defaults. Hosts should render their HD Rumble settings UI dynamically from `rh_option_*` (following `docs/integration/ui-guidelines.md`) rather than hardcoding separate rumble keys. If the menu cannot link the bridge directly (Longshot's menu is a separate DLL), expose the same calls through the host's function table, as described in lesson 6 of the integration guide.

### Where to Call `rh_shutdown()` & `SDL_CloseGamepad`
Many static recompilations terminate via `ExitProcess` / `TerminateProcess` (for example on window close or `svcExitProcess`), skipping `atexit` handlers. Call `rh_shutdown()` on every exit path while the game is playing, before `ExitProcess` / `TerminateProcess`, so DualSense speakers and triggers release, Steam grips exit PCM mode and HID handles close. Do not call it from a crash handler. Also call `SDL_CloseGamepad` on every open pad, but only where you can do it on the thread that owns the pads: totk does it right after `rh_shutdown()` in its shutdown function; Longshot polls pads on another thread, so it calls only `rh_shutdown()` (in `hos_close_finish()`). A wired Switch Pro Controller that is never closed stays in `0x30` mode and shows stuck buttons after quitting; see [`docs/integration/recomp-games.md`](../../docs/integration/recomp-games.md#10-pro-controller-reliability-fixes-usable-even-without-hd-rumble).

---

## 4. The Two Runtime Hook Points (`vibration_send` and Per-Pad Loop)

### Hook 1: Guest Vibration Call
Pass raw unscaled guest floats (`amp_low`, `freq_low`, `amp_high`, `freq_high`) before the host's downmix or native HD Rumble:

```c
if (rh_vibration(dev, v[0], v[1], v[2], v[3])) {
    return; /* Skips host fallback only when all connected pads are claimed */
}
```

Wire `PermitVibration` -> `rh_permit(on)`. For Switch 2 titles, ensure `IsOunceVibrationDeviceMounted` returns `1`.

### Hook 2: Per-Pad Polling Loop
Sync the host's master HD Rumble switch with `rh_set_enabled(...)` and skip host rumble only for claimed slots:

```c
if (rh_pad_claimed(i)) {
    continue; /* Prevents double rumble on claimed slots */
}
```

---

## 5. The Four Supported Controllers

1. **Sony DualSense / DualSense Edge (`054C:0CE6` / `0x0DF2`)**:
   - **USB (Wired)**: Opens an independent 4-channel `48000 Hz` `SDL_AUDIO_F32` stream per wired DualSense (`SDL_PumpEvents()` before enumeration, 2 s hotplug retry, safe stream teardown under slot lock with automatic fallback & rediscovery if the audio endpoint disappears). When **HD Haptics** is enabled (`dualsense_hd_haptics`), activates the speaker channel (`renhdrum_dualsense_dsp_set_speaker` + `renhdrum_hd_haptics_speaker_block`) and drives vibration-reactive adaptive triggers (`renhdrum_tri_trigger_*` + `renhdrum_hd_haptics_trigger_block`) from the single worker thread so trigger packets match state-machine order.
   - **Bluetooth**: Sends combined report `0x36` (`renhdrum_dualsense_dsp_build_bt36_report`: rumble + reactive triggers + Opus speaker audio embedded in `0.5.0`) when HD Haptics is enabled, or `0x32` (`renhdrum_dualsense_dsp_build_bt_report`) when disabled. Automatically falls back to standard rumble on write failure and restarts the Bluetooth HID worker every 10 s if the link recovers.
2. **Nintendo Switch Pro Controller (`057E:2009`)**:
   - Uses `RenHdcEngine` with `SDL_SendJoystickEffect` (`0x10` packets) and 40 ms wired resend (`RH_RESEND_US`).
   - Settings are resolved from `RenHdcStore` via `renhdrum_store_to_config` + `renhdrum_settings_resolve_pure`, keeping unpatched SDL3 transport flags off (`usb_no_queue = bt_after_input = bt_fast = false`, Bluetooth Compatible Level 4).
3. **Microsoft Xbox One S (`XBOXONE` or VID `0x045E`)**:
   - Supports both **RenHDrum** mode (`renhdrum_xbox_rumble_*`, 4 motors scaled by `xbox_strength`) and **Direct** mode (`renhdrum_xbox_direct_*`, driven by `xbox_mode = 1` in `RenHdcStore`). Respects `SDL_PROP_JOYSTICK_CAP_TRIGGER_RUMBLE_BOOLEAN` on SDL3 and zeroes all 4 motors on detach/shutdown.
4. **Steam Controller 2026 (`0x28DE`, PIDs `0x1302` / `0x1303` / `0x1304`, Experimental)**:
   - Created with `renhdrum_valve_grip_dsp_create_ex(..., true)` (40 ppm drift compensation; skips `0x88` writes on `RENHDRUM_VALVE_GRIP_SKIP`).
   - Manages `0x86` PCM mode keep-alives via `renhdrum_valve_grip_mode_tick` (passes `stalled = true` on failed or slow HID writes) and sends `renhdrum_valve_grip_mode_off` on detach and `rh_shutdown()`.
   - Never claims Steam Input virtual stand-in pads (`SDL_GetGamepadSteamHandle(pad) != 0`), releasing grips back to Steam Input if a stand-in appears.

---

## 6. Building `renhdrum.dll` with `crt-static`

`RenHDrum`'s `.cargo/config.toml` sets `rustflags = ["-C", "target-feature=+crt-static"]` for `x86_64-pc-windows-msvc`. Run `cargo build --release` from the `RenHDrum` directory and verify with `dumpbin /dependents target\release\renhdrum.dll` that `VCRUNTIME140.dll` is absent. Bundle `renhdrum.dll` beside the game executable alongside `SDL3.dll` (and `SDL2.dll` if used).

---

## License

- Bridge implementation (`renhdrum_recomp_bridge.c`, `renhdrum_recomp_bridge.h`): **MIT OR Apache-2.0**.
- Hookshot reference patch (`reference/recomp-c-reference.patch`): **GPL-3.0**. Credit to **NotRexed** and the Hookshot contributors (legacy reference; Hookshot is no longer supported, use Longshot for Ocarina of Time).
- Longshot reference patch (`reference/longshot-reference.patch`, applies cleanly on upstream `08994a9e`, `main` of 20 Nov 2026; it includes the HD Rumble menu page): **GPL-3.0-or-later**. Credit to **MagicBeanDev** and the Longshot contributors.
- totk-pc-recomp reference patch (`reference/totk-reference.patch`, applies cleanly on tag `v0.4.1`, `6151ac4`; a Switch 1 game, so it calls `rh_set_switch_gen(1)`): **MIT**, as totk-pc-recomp. Credit to the totk-pc-recomp contributors.
- dkb-pc-recomp reference patch (`reference/dkb-reference.patch`, applies cleanly on tag `v0.5.1`, `44ba6b1`; a Switch 2 game, so it calls `rh_set_switch_gen(2)`; only player 1's pad goes through the bridge, player 2 keeps the game's two-motor rumble; it also fixes two `.nx` reading bugs of the installer): **GPL-3.0-or-later**, as dkb-pc-recomp. Credit to Lucas-blip-png and the dkb-pc-recomp contributors.
