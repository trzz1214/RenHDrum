# Static Recompilation Games HD Rumble Integration Guide

This guide explains how I integrate `renhdrum.dll` (`0.5.0`) into C and C++ static recompilation runtimes using SDL3 (or SDL2) to deliver authentic HD Rumble across all four controllers supported by the core. I have shipped it this way in Longshot, totk-pc-recomp and dkb-pc-recomp. It also documents standalone hardware reliability fixes for the Nintendo Switch Pro Controller.

> [!NOTE]
> **Supported Controllers**: `✅ Nintendo Pro Controller` · `✅ DualSense` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)`.

---

## Integration Checklist

Seven checks I run before shipping any recomp integration. They come from real PR review lessons (`Goldoire/fefw-recomp#95`) and hardware testing across Hookshot, Longshot, `totk-pc-recomp`, and `LASRecomp`:

1. **Load `renhdrum.dll` by full path beside the executable.** Never pass a bare filename that searches CWD or `PATH`. Verify `(renhdrum_version() >> 8) == (RENHDRUM_VERSION >> 8)` (`0x0005xx` for `0.5.0`), and resolve optional functions as "use if present". When the core's `major.minor` changes, copy the new `renhdrum.h` into your tree and rebuild; otherwise the bridge refuses the DLL and the game keeps its normal rumble (safe on purpose).
2. **Apply intensity in one place only.** `volume_percent` in `renhdrum_dualsense_dsp_create` is already the final output gain; never scale samples again before pushing them. To change volume at runtime, recreate the DSP handle while keeping the audio stream open.
3. **Respect the host's rumble and HD Rumble switches everywhere.** Turning rumble off must stop both RenHDrum haptics and plain motor rumble; turning HD Rumble off must release all claimed pads cleanly back to the host's built-in rumble.
4. **Always route Switch Pro Bluetooth through the core's limiter.** **Compatible** (level 4, default) runs on unpatched SDL3 out of the box (newest value, 32 ms minimum spacing, 40 ms resend while rumbling). **Detailed (5 ms)** (`bt_fast`) requires idle-gated SDL3 transport patches. Never push raw unthrottled HID packets over Bluetooth.
5. **Fall back in layers and avoid lock inversion.** Missing DLL, version mismatch, missing optional exports, missing DualSense speaker, or repeated write failures must degrade cleanly per pad. Always drop host locks before joining worker threads.
6. **Call `rh_shutdown()` on every exit path while playing.** `ExitProcess` / `TerminateProcess` skip `atexit`, so a handler is not enough (see section 11, lesson 1). Do not call it from a crash handler.
7. **Never claim a virtual pad.** Skip Steam Input stand-in pads and the host's own virtual pads, or the rumble fires twice (see section 11, lesson 4).

---

## 1. What You Need (`renhdrum.dll` 0.5.0 and Header)

To integrate RenHDrum into a C/C++ recomp runtime:

- **`renhdrum.dll` (`0.5.0`)** placed directly beside the compiled game executable (Windows x64).
- **[`include/renhdrum.h`](../../include/renhdrum.h)** (with `RENHDRUM_VERSION` in branch `0x0005xx`).
- Optional drop-in reference implementation:
  - [`integrations/recomp-c/bridge/renhdrum_recomp_bridge.c`](../../integrations/recomp-c/bridge/renhdrum_recomp_bridge.c)
  - [`integrations/recomp-c/bridge/renhdrum_recomp_bridge.h`](../../integrations/recomp-c/bridge/renhdrum_recomp_bridge.h)
  - [`integrations/recomp-c/reference/recomp-c-reference.patch`](../../integrations/recomp-c/reference/recomp-c-reference.patch) (reference patch against Hookshot `v2.2.0-rc.2`, GPL-3.0) and [`integrations/recomp-c/reference/longshot-reference.patch`](../../integrations/recomp-c/reference/longshot-reference.patch) (Longshot, GPL-3.0-or-later). The bridge is the same in both; only the hook sites differ.

You do **not** need to link against `renhdrum.dll.lib` or `SDL3.lib` at compile time. The reference bridge resolves `renhdrum.dll` with `LoadLibraryExW` and resolves SDL3/SDL2 functions at runtime with `GetModuleHandleA` + `GetProcAddress`.

With the bridge, call `rh_set_switch_gen(1)` for a Switch 1 title or `rh_set_switch_gen(2)` for a Switch 2 title **before** `rh_load()` (it sets `renhdrum_set_nx2_mode` to `0` or `2`). Switch 2 guest commands use audio-range amplitudes and frequencies; Switch 1 commands do not.

---

## 2. Safe DLL Loading & Version Check

Never call `LoadLibraryA("renhdrum.dll")` or `SDL_LoadObject("renhdrum.dll")` with a bare name: Windows searches the current working directory and `PATH`, allowing an older or unrelated DLL elsewhere on the system to hijack the load.

Resolve the executable's own directory with `GetModuleFileNameW`, append `renhdrum.dll`, and load with restricted search flags:

```c
WCHAR exe_path[MAX_PATH];
DWORD exe_len = GetModuleFileNameW(NULL, exe_path, MAX_PATH);
/* find last '\\' or '/' in exe_path, copy directory + L"renhdrum.dll" into dll_path */

HMODULE dll = LoadLibraryExW(
    dll_path,
    NULL,
    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32
);
```

After loading:

1. **Resolve required core exports** (`renhdrum_version`, `renhdrum_engine_create`, `renhdrum_engine_destroy`, `renhdrum_settings_default`, `renhdrum_engine_apply_settings`, `renhdrum_engine_push_half_sample`, `renhdrum_engine_tick`, `renhdrum_set_nx2_mode`). If any required symbol is missing, call `FreeLibrary` and disable the bridge.
2. **Verify the `major.minor` branch**:
   ```c
   uint32_t ver = rh.version();
   if ((ver >> 8) != ((uint32_t)RENHDRUM_VERSION >> 8)) {
       hos_log("renhdrum: off (incompatible renhdrum.dll version 0x%06x, expected branch 0x%04x)",
               (unsigned)ver, (unsigned)((uint32_t)RENHDRUM_VERSION >> 8));
       FreeLibrary(rh.dll);
       /* zero rh struct, mark tried = 1, fall back to host rumble */
       return 0;
   }
   ```
   A DLL from another `major.minor` branch (for example `0.3.x`, `0x0003xx`) is rejected cleanly so structural or ABI mismatches never cause crashes.
3. **Load optional device exports as "use if present"**:
   - DualSense DSP: `renhdrum_dualsense_dsp_create`, `renhdrum_dualsense_dsp_destroy`, `renhdrum_dualsense_dsp_set_half_target`, `renhdrum_dualsense_dsp_fill_f32`, `renhdrum_dualsense_dsp_build_bt_report`.
   - Xbox One S 4-motor converter: `renhdrum_xbox_rumble_create`, `renhdrum_xbox_rumble_destroy`, `renhdrum_xbox_rumble_push_half_sample`, `renhdrum_xbox_rumble_tick`, `renhdrum_xbox_rumble_resend`.
   - Steam Controller 2026 grips: `renhdrum_valve_grip_dsp_create`, `renhdrum_valve_grip_build_report`.
   - New in `0.5.0`: `renhdrum_options` and the `renhdrum_store_*` functions (settings), DualSense HD Haptics (speaker, adaptive triggers, combined Bluetooth report `0x36`), Xbox Direct mode, Steam `renhdrum_valve_grip_mode_tick` / `renhdrum_valve_grip_mode_off`.
   If an optional symbol is missing, only that controller type stays unclaimed and uses the host's standard rumble.
4. **Support an environment kill switch** (e.g., `HOOKSHOT_RENHDRUM=0`) so users or testers can compare native vs. RenHDrum rumble without deleting the DLL.

---

## 3. The Four Hook Points of the `recomp-c` Bridge

The [`recomp-c` bridge](../../integrations/recomp-c/README.md) is the one piece of C I reuse for every recomp. Wiring a host to it takes four hooks:

| Hook | Where | Calls |
|---|---|---|
| Load | Startup, before the first pad opens | `rh_set_switch_gen(1 or 2)`, then `rh_load()` |
| Open / close a pad | Where the host opens and closes `SDL_Gamepad` | `rh_attach(pad)`, `rh_detach(pad)` |
| Rumble values | Guest vibration entry and the per-pad poll loop (3.1, 3.2) | `rh_vibration(...)`, `rh_permit(on)`, `rh_pad_claimed(i)`, `rh_set_enabled(on)` |
| Exit | Every path that ends the process while playing | `rh_shutdown()` (section 11, lesson 1) |

The host's HD Rumble settings page calls `rh_status_line()` and the `rh_option_*` functions (section 11, lesson 6). All runtime vibration routing goes through the two hook points below.

Sections 3.1 and 3.2 show the minimal pattern (the host asks `rh_pad_claimed(i)` each poll; totk-pc-recomp works this way). My current Longshot port goes one step further: it calls `rh_attach(gc)` once per pad when HD Rumble is on, records a claimed pad as its own vibration backend (`VIB_RH_BRIDGE` in `host/vibration.h`) and skips it in the poll loop, so its existing Motors / HD Rumble choice stays untouched for every other pad. Both work; pick the one that fits the host's own rumble routing.

### 3.1 Hook 1: Guest Vibration Entry (`host/nn_hid.c:vibration_send`)

Intercept guest vibration values **before** the host's downmix (`rumble_map`) or built-in HD Rumble (`hos_sdl_rumble_hd`). Pass the raw, unscaled floats (`amp_low`, `freq_low`, `amp_high`, `freq_high`):

```c
const int dev = (G32(handle) >> 16) & 1;
if (rh_vibration(dev, v[0], v[1], v[2], v[3])) {
    return; /* Handled by RenHDrum for all connected pads */
}
rumble_map(v, amp[dev]);
```

`rh_vibration` feeds all active RenHDrum engines (`Pro`, `DualSense`, `Xbox`, `Steam`) and returns `1` **only** when `handled && rh_all_pads_claimed()`. If an unclaimed secondary pad is also connected, `rh_vibration` returns `0` so the host still computes fallback values for that pad.

Also wire:
- `PermitVibration(bool)` → `rh_permit((int)ARG(0))` (pushes neutral `(0.0f, 160.0f, 0.0f, 320.0f)` to both sides when `0`).
- Switch 2 HLE `IsOunceVibrationDeviceMounted` → return `1` (both overloads) so Switch 2 games see a mounted HD Rumble device.

### 3.2 Hook 2: Per-Pad Poll Loop (`host/pad_sdl.c:pads_poll`)

At the top of `pads_poll()`, sync the menu option to the bridge (`rh_set_enabled(rx_option(HOS_OPT_HD_RUMBLE, 1) != 0)`). Then, inside the per-pad loop, skip the host's built-in HD Rumble and motor rumble **only** for slots claimed by RenHDrum:

```c
if (rh_pad_claimed(i)) continue;
if (hd_poll(i, side, now)) continue;
if (rumble_due(lo, hi, pads[i].rumble_lo, pads[i].rumble_hi, now, pads[i].rumble_ns)) {
    hos_sdl_pad_rumble(gc, lo, hi, lo || hi ? 1000 : 0);
}
```

### 3.3 1 ms Worker Thread, 250 ms Silence Watchdog & Lock Discipline

Games frequently stop sending vibration commands when paused or loading without sending an explicit zero packet. The worker thread tracks timestamps (`rh_at[2]`, `rh_ds_at[2]`, `rh_xb_at[2]`, `rh_sv_at[2]`) per side and injects `(0.0f, 160.0f, 0.0f, 320.0f)` after `RH_STALE_US` (`250000` µs) of silence.

Follow strict lock discipline:
- Never acquire the host mutex while holding a bridge lock (`rh_lock`, `rh_ds_lock`, `rh_xb_lock`, `rh_sv_lock`).
- Release bridge locks **before** calling `SDL_SendJoystickEffect`, `SDL_RumbleJoystick`, or `SDL_RumbleJoystickTriggers`.
- On detach, clear the engine pointers under the lock, release the lock, then stop devices and `WaitForSingleObject(thread, 2000)` outside the lock. If a worker thread fails to stop within 2000 ms, log and skip `hid_close` / `destroy` rather than risking a use-after-free or shutdown hang.

---

## 4. The Four Supported Controllers and How Each Is Detected

### 4.1 Nintendo Switch Pro Controller (`057E:2009`)
- **Detection**: `vid == 0x057E && pid == 0x2009`.
- **Engine**: `renhdrum_engine_create(bt != 0, true)` with `renhdrum_settings_default(&s)`.
- **Bluetooth**: Default to `s.bluetooth_level = 4` (**Compatible** — 32 ms spacing, newest value, 40 ms resend handled inside the core, safe on unpatched SDL3). Allow optional `HOOKSHOT_BT_LEVEL=1..4` override. Keep `usb_no_queue = bt_after_input = bt_fast = false` when targeting unpatched SDL3.
- **Output**: 10-byte sub-command `0x10` packets via `SDL_SendJoystickEffect`. Over USB, if no new non-silent packet was sent for `40 ms` (`RH_RESEND_US = 40000`), resend the last non-silent packet so the controller's hardware watchdog does not drop sustained rumbles. On detach, send the neutral packet `{0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40}`.

### 4.2 Sony DualSense / DualSense Edge (`054C:0CE6` / `0x0DF2`)
- **Detection**: `vid == 0x054C && (pid == 0x0CE6 || pid == 0x0DF2)`.
- **Wired USB**: Create `renhdrum_dualsense_dsp_create(100, true)`. Initialize SDL audio (`hos_sdl_init(0x10)`), **call `SDL_PumpEvents()` first** (Windows recomp builds with Vulkan windows often never pump SDL events, so SDL3 audio hotplug never registers a DualSense plugged in after startup without pumping), then enumerate playback devices matching `"DualSense"` or `"Wireless Controller"` with `channels >= 4`. Open a 4-channel `48000 Hz` `SDL_AUDIO_F32` stream (`0x8120`). Retry every 2 seconds if the audio endpoint appears late, and clear SDL's HID rumble (`SDL_RumbleGamepad(pad, 0, 0, 0)`) once opened.
- **Bluetooth**: Create `renhdrum_dualsense_dsp_create(100, false)` (`crisp_notes = false` for cleaner Bluetooth notes). Open the controller's HID path via `SDL_GetGamepadPath` / `SDL_GetJoystickPath` (falling back to `SDL_hid_enumerate(vid, pid)`), stop SDL's motor rumble, and spawn a dedicated high-resolution timer thread sending `142`-byte report `0x32` every `RENHDRUM_DS5_BT_PERIOD_NS` (`10666666` ns) via `renhdrum_dualsense_dsp_build_bt_report(dsp, RENHDRUM_DS5_BT_BUFFER_LENGTH_DEFAULT, report)`. Gate transmission 2 seconds after the last active vibration so the Bluetooth radio stays idle when silent.
  With HD Haptics on (the default) the bridge sends the combined report `0x36` instead, built by `renhdrum_dualsense_dsp_build_bt36_report`: rumble, triggers and the speaker in one report (see [dualsense.md](dualsense.md), section 6).

### 4.3 Microsoft Xbox One S / Xbox Series (`XBOXONE` or VID `0x045E`)
- **Detection**: `SDL_GetGamepadType(pad) == 3` (`SDL_GAMEPAD_TYPE_XBOXONE` in SDL3; `2` in SDL2) **or** `vid == 0x045E` (third-party XInput pads may report another SDL type).
- **Engine**: `renhdrum_xbox_rumble_create(xb_bt != 0)`. If SDL cannot determine the connection type, treat it as Bluetooth (`20 ms` cadence instead of `10 ms` USB).
- **Rule**: Do **not** invent custom motor ratios or curves in the bridge. The core owns the `XBOX_DEFAULT` / `PRO_DEFAULT` mapping and returns exact 16-bit motor amplitudes.
- **Output**: `renhdrum_xbox_rumble_tick` fills `uint16_t motors[4]` (`left grip, right grip, left trigger, right trigger`). Outside the lock, call `SDL_RumbleJoystick(joy, motors[0], motors[1], 2000)` and `SDL_RumbleJoystickTriggers(joy, motors[2], motors[3], 200)`. Keep trigger duration short (`200 ms`) so a dropped packet never leaves triggers stuck on. Note that SDL3's `SDL_RumbleJoystick` returns `bool` (`(r & 0xFF) != 0`), whereas SDL2 returns `0` on success. If the send fails (common on `rawinput` before the first button press), call `renhdrum_xbox_rumble_resend(rh_xb)` so the next tick retries. Zero all four motors on detach.
- **Note on triggers**: Trigger motors require SDL `rawinput` or `Windows.Gaming.Input`; XInput exposes grips only. See [xbox.md](xbox.md).

### 4.4 Steam Controller 2026 (`0x28DE`, PIDs `0x1302` / `0x1303` / `0x1304`, Experimental)
- **Detection**: `vid == 0x28DE` and `pid == 0x1302` (wired, 8 kHz mu-law `RENHDRUM_VALVE_GRIP_FORMAT_MULAW_8K`, `3.875 ms` period), `0x1303` (Bluetooth, 4 kHz `RENHDRUM_VALVE_GRIP_FORMAT_MULAW_4K`, `7.75 ms`), or `0x1304` (dongle, 4 kHz).
- **Steam Input stand-in avoidance**: Always check `SDL_GetGamepadSteamHandle(pad) != 0`. Steam Input creates a virtual stand-in gamepad alongside the physical HID device; **never** claim the stand-in pad. If a stand-in pad is already tracked, skip claiming the real Steam Controller; if a stand-in opens after grips already started, stop the grip worker (`rh_sv_stop`) and clear its claim bit so rumble does not double-fire.
- **Output**: Open the HID path (`SDL_hid_open_path`), create `renhdrum_valve_grip_dsp_create_ex(100, true, true)` (enabling 40 ppm drift packet-skipping; fall back to `renhdrum_valve_grip_dsp_create` when the export is missing), clear SDL motor rumble, and run a dedicated high-resolution thread:
  1. Call `renhdrum_valve_grip_build_report(dsp, format, report)` every format period (`1` = vibrating, `0` = silent, `2` / `RENHDRUM_VALVE_GRIP_SKIP` = skip writing `0x88` this tick for 40 ppm drift compensation).
  2. Call `renhdrum_valve_grip_mode_tick(dsp, format, now_ns, stalled, mode_reports)` (passing `stalled = true` if the previous write failed or timed out, then clearing `stalled`). If it returns `2`, write both 4-byte `0x86` reports (`mode_reports` and `mode_reports + 4`); the core sends `"on"` on first vibration, refreshes `"on"` every `3 s` or immediately on `stalled`, and emits `"off"` after `100 ms` of silence.
  3. Write the `64`-byte `0x88` report (`SDL_hid_write`) while active plus the `100 ms` tail (`RH_STEAM_TAIL_NS`), skipping the write when `build_report` returns `RENHDRUM_VALVE_GRIP_SKIP` (`2`). On `100` consecutive write failures (`RH_STEAM_FAILS_MAX`), stop and release the claim bit (`rh_pads_set_claimed(pad, 0)`) so the pad falls back to host rumble.
  4. On stop, detach, or any exit path, call `renhdrum_valve_grip_mode_off(format, mode_reports)` and write both `0x86` off reports before closing the HID handle.
- Always label Steam Controller 2026 support as **experimental** in UI text and docs.

---

## 5. Toggling HD Rumble & Locking Intensity

This section describes the first integration, Hookshot. A host with a settings page of its own should also read section 11, lesson 6.

In the reference Hookshot integration (`gpu/vk/vk_menu.cpp`, `gpu/vk/vk_replay.cpp`):

1. **Menu option**: Add `opts.hd_rumble` (default `1`), persisted to INI (`hd_rumble`) and marked `preset_exempt = true` so graphics presets never toggle rumble. Forward it to the host via `HOS_OPT_HD_RUMBLE` (`rx_option(HOS_OPT_HD_RUMBLE, 1)`).
2. **Runtime enable/disable (`rh_set_enabled`)**:
   - Calling `rh_set_enabled(0)` detaches every active RenHDrum engine (`rh_detach_engines(NULL, 0)`) and clears `rh_slot_claimed[]` while **keeping** `rh_conn_pads[]` tracked. `rh_pad_claimed(i)` and `rh_vibration` immediately return `0`.
   - Calling `rh_set_enabled(1)` re-attaches every tracked pad in `rh_conn_pads[]`.
3. **Locking "Rumble strength"**:
   - While `opts.hd_rumble != 0`, lock the **Rumble strength** row (`row_fixed(&opts.rumble)` returns `true` so arrow keys, clicks, and `set_row` cannot alter it) and display `"-"` in the value cell.
   - Keep the underlying `opts.rumble` value untouched in memory and INI so turning HD Rumble off immediately restores the user's previous Rumble strength percentage.
4. **Live status line (`rh_status_line`)**:
   Expose a 1-line status callback to the menu (`MkwVkHost::hd_rumble_status`) returning the active hardware state:
   - `"HD Rumble is off: using normal rumble"`
   - `"Connected: Pro Controller (USB), HD Rumble"` / `"(Bluetooth)"`
   - `"Connected: DualSense (USB), HD Rumble"` / `"(Bluetooth)"`
   - `"Connected: Xbox One S, HD Rumble (4 motors)"`
   - `"Steam Controller: HD Rumble (experimental)"`
   - `"No controller HD Rumble supports: normal rumble"`

---

## 6. Coexisting with a Recomp's Native HD Rumble

Newer recomp runtimes such as Hookshot (`v2.2.0-rc.2`) already implement native HD Rumble for Switch Pro and Joy-Con (`host/hd_rumble.h`, `hd_poll` in `host/pad_sdl.c`). To coexist cleanly without double rumble or breaking secondary pads:

1. **Per-pad claim gating**: `pads_poll()` checks `if (rh_pad_claimed(i)) continue;` before both `hd_poll()` and `rumble_due()`. Claimed slots are driven solely by RenHDrum; unclaimed slots (such as Joy-Con or unsupported pads) continue using the host's native `hd_poll` or motor rumble.
2. **All-pads check in `vibration_send`**: `rh_vibration()` updates every claimed engine and returns `1` only if `handled && rh_all_pads_claimed()`. If Player 1 has a DualSense (claimed) and Player 2 has a Joy-Con (unclaimed), `rh_vibration()` returns `0` so `nn_hid.c` still passes the sample to the host for Player 2, while `rh_pad_claimed(0)` skips Player 1 in `pads_poll()`.
3. **Clean handoff when disabled**: Toggling **HD Rumble** off sets `rh_set_enabled(0)`, releasing all claimed pads immediately so Hookshot's built-in Pro/Joy-Con HD Rumble (and motor rumble on other pads) resumes without restarting the game.

---

## 7. Building `renhdrum.dll` with `crt-static`

Shipping a dynamically linked MSVC C runtime (`VCRUNTIME140.dll`) causes `LoadLibraryExW` to fail silently on Windows machines that do not have the Visual C++ Redistributable installed.

The `RenHDrum` repository includes `.cargo/config.toml`:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

When building from source:

1. **Run `cargo build --release` from inside the `RenHDrum` repository directory** (Cargo only reads `.cargo/config.toml` from the working directory or its ancestors; if invoked from outside, pass `RUSTFLAGS="-C target-feature=+crt-static"` explicitly).
2. **Verify dependencies** with Visual Studio's `dumpbin`:
   ```powershell
   dumpbin /dependents target\release\renhdrum.dll
   ```
   Confirm only OS DLLs (`KERNEL32.dll`, `ntdll.dll`, `api-ms-win-core-synch-*`, etc.) appear and `VCRUNTIME140.dll` is **not** listed.
3. **Package `renhdrum.dll`** next to the compiled executable (`BESIDE` in `nx2/make_game.py` and `nx2/build.py`) and compare its SHA-256 against `SHA256SUMS.txt`.

---

## 8. Self-Test Checklist

Before considering an integration complete, verify every case below (using mock DLL tests for automated CI plus hardware checks where available):

- [ ] **Missing DLL**: Removing `renhdrum.dll` falls back to host rumble with zero crashes.
- [ ] **Incompatible DLL version**: A DLL from another branch (for example `0.3.x`, `0x000302`) or a DLL missing `renhdrum_version` is rejected and unloaded (`FreeLibrary`).
- [ ] **Missing optional exports**: A `0.5.0` DLL lacking `renhdrum_valve_grip_*` or `renhdrum_xbox_rumble_*` leaves only that controller unclaimed while Pro/DualSense still work.
- [ ] **Controls menu & INI**: `"HD Rumble"` appears directly above `"Rumble strength"`, defaults to on, locks `"Rumble strength"` to `"-"`, preserves saved `rumble_strength` in INI, and updates `rh_status_line()`.
- [ ] **Runtime toggle**: Turning HD Rumble off mid-session detaches engines and restores host rumble; turning it back on re-attaches connected pads.
- [ ] **Multi-pad isolation**: With one claimed pad and one unclaimed pad, `rh_vibration` returns `0` and `rh_pad_claimed` skips only the claimed slot.
- [ ] **Switch Pro Controller (USB & BT)**: Verified `0x10` packets, 40 ms wired resend on sustained rumble, Bluetooth Level 4 pacing, and clean `SDL_CloseGamepad` on exit so wired Pro leaves `0x30` mode.
- [ ] **DualSense (USB & BT)**: Verified 4-ch 48 kHz audio stream opens even when plugged in after launch (`SDL_PumpEvents`), hotplug unplug/replug does not crash, and Bluetooth report `0x32` sends cleanly.
- [ ] **Xbox One S**: Verified 4-motor tick (`grips` + `triggers`), `xb_resend` retry on failure, and all 4 motors zeroed on detach.
- [ ] **Steam Controller 2026 (experimental)**: Verified `0x86` mode on/off + 64-byte `0x88` reports, and verified Steam Input stand-in pad prevents double rumble in both attach orders.
- [ ] **250 ms silence watchdog**: Pausing or entering a static menu stops rumble cleanly within ~250 ms.
- [ ] **Exit**: Closing the game by every route (window close, in-game quit, guest exit) leaves no stuck button, no stuck rumble, no DualSense speaker or trigger left on, and a wired Pro back in its normal mode.
- [ ] **Virtual pads**: With Steam Input on, the rumble does not double up.
- [ ] **`crt-static` check**: `dumpbin /dependents renhdrum.dll` shows no `VCRUNTIME140.dll`.

---

## 9. Lessons from PR #95 (`Goldoire/fefw-recomp`)

Real-world review of PR #95 on *Fire Emblem: Fortune's Weave* recomp (`Goldoire/fefw-recomp`, `v0.1.7`, `renhdrum.dll` `0.3.0`) surfaced six concrete pitfalls that shaped this guide:

1. **Loading by bare DLL name**: Searching CWD and `PATH` allowed stray DLL copies to win. **Fix**: resolve the exe directory with `GetModuleFileNameW` and call `LoadLibraryExW` with `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`.
2. **Dynamic CRT (`VCRUNTIME140.dll`) dependency**: Players without the VC++ Redistributable silently lost haptics. **Fix**: build `renhdrum.dll` with `+crt-static` and verify with `dumpbin /dependents`.
3. **Rumble-off toggle bypassed by haptics**: Disabling rumble in settings left RenHDrum active. **Fix**: enforce the master toggle at the single entry point and detach/yield all engines when disabled.
4. **Double intensity scaling & audio stream reopening**: `volume_percent` in `renhdrum_dualsense_dsp_create` is already the final gain; scaling samples again squared the attenuation (40% became ~16%), and recreating the SDL audio stream on slider drag caused pops. **Fix**: scale in one place only, and recreate only the DSP handle while keeping the stream open.
5. **Unthrottled Switch Pro Bluetooth**: Bypassing the core's limiter caused input lag (#26). **Fix**: always route Bluetooth through `RenHdcEngine` (`bluetooth_level = 4` Compatible by default on unpatched SDL3).
6. **Deadlock risk & missing layered fallbacks**: Joining a worker thread while holding the host controller mutex deadlocks if the worker waits on that mutex to send a packet. **Fix**: establish strict lock ordering, drop host locks before `WaitForSingleObject` / `SDL_WaitThread`, and fall back to standard rumble across every failure layer.

---

## 10. Pro Controller Reliability Fixes (Usable Even Without HD Rumble)

Testing across static recompilation projects uncovered two Windows driver quirks specific to the Nintendo Switch Pro Controller. These fixes apply even when HD Rumble is turned off:

### 10.1 Right Stick Upward Drift Fix (`|offset| > 0.5f -> 0.0f`)
When switching between standard HID reports and full-report sub-commands, stale calibration offsets (such as `offset_y: 0.939940`) can stick in configuration memory and cause continuous upward camera drift. Clamp any stick center offset exceeding `0.50f` to `0.0f`:

```c
if (fabsf(offset_x) > 0.5f) offset_x = 0.0f;
if (fabsf(offset_y) > 0.5f) offset_y = 0.0f;
```

### 10.2 Stuck Left Stick / Errant Inputs on Exit (`SDL_CloseGamepad` / `ClearUSB`)
Many recomp runtimes exit abruptly via `ExitProcess` without closing controllers. Because a wired Switch Pro Controller runs in proprietary mode `0x30`, leaving it open traps it in `0x30` until physically unplugged, which Windows DirectInput misreads as a stuck upper-left stick (`Y = 9472` instead of `32767`) and spurious button presses.

1. **Always call `SDL_CloseGamepad` at exit**, on the thread that owns the pad and on every exit path (an `atexit` handler alone is not enough: `ExitProcess` skips it). Call `rh_shutdown()` first. See section 11, lesson 1.
2. If you ship a custom SDL3 build, ensure `HIDAPI_DriverSwitch_CloseJoystick` issues `ClearUSB`:
   ```c
   WriteProprietary(ctx, k_eSwitchProprietaryCommandIDs_ClearUSB, NULL, 0, false);
   ```

---

## 11. Lessons from the Longshot, totk-pc-recomp and dkb-pc-recomp ports

I ported the `0.5.0` bridge to Longshot, to totk-pc-recomp (a Switch 1 game) and to dkb-pc-recomp (a Switch 2 game). These are the things that cost me time, in the order I would check them.

1. **Exit.** `ExitProcess` and `TerminateProcess` skip `atexit`, so call `rh_shutdown()` on every exit path while the game is playing. Without it a wired Pro Controller is left in its special mode and the player sees stuck buttons after quitting (the "stuck key after exit" bug). `rh_shutdown()` releases the DualSense speaker and triggers, sends the Steam grip "off" report and closes the HID handles; it is safe to call more than once. Do not call it from a crash handler (it can wait on a lock the crashed thread holds).
   - Closing the pads as well is only safe on the thread that owns them. In totk, `input_shutdown()` calls `rh_shutdown()` and then closes the gamepad, ahead of each of the five `plat_exit(0)` calls.
   - In Longshot the pad poll loop runs on another thread, so closing pads from the exit path would race with it. There I call only `rh_shutdown()`, in `hos_close_finish()` (`host/hos_relaunch.c`).
2. **The header travels with the DLL.** The bridge compares `RENHDRUM_VERSION >> 8` of the header you copied with the DLL's `renhdrum_version() >> 8`. When the DLL's `major.minor` moves, copy the new `renhdrum.h` and rebuild, or the bridge refuses the DLL and the player gets normal rumble. That is deliberate and safe. The `0.5.0` functions are loaded as "use if present": if one is missing, only that feature drops back to the normal path.
3. **When upstream has already added RenHDrum.** Longshot vendored RenHDrum on its own in `third_party/renhdrum/` (pinned tag plus a `tree-sha256`). I replaced its private RenHDrum path with the shared bridge and kept its own Motors and HD Rumble paths for pads RenHDrum does not claim. Bump `third_party/` only the way the project's `PROVENANCE.md` says, and put the new DLL beside the game without breaking the integrity check.
4. **Virtual pads.** Do not claim a Steam Input stand-in (`SDL_GetGamepadSteamHandle(pad) != 0`, or `0x28DE:0x11FF`), nor a pad the host creates itself (Longshot's `pad_switch2.c`). Claiming both the real and the virtual pad rumbles twice.
5. **Switch generation.** Call `rh_set_switch_gen(1)` for a Switch 1 game (totk) and `rh_set_switch_gen(2)` for a Switch 2 game, before `rh_load()`.
6. **Settings.** Keep the HD Rumble on/off switch in the game's own settings file. Everything per controller lives in `renhdrum.ini` beside the executable, read and written through `rh_option_*`. Draw the page from the core's option list and keep no rumble keys of your own (see [ui-guidelines.md](ui-guidelines.md)).
   - A host whose menu has a static model (a fixed table of rows) declares the HD Rumble rows by the keys `renhdrum_options` returns.
   - Longshot's menu lives in a separate DLL that talks to the host only through a function table (`MkwVkHost`). I added one function to it, `rh_option(op, key, value, text, cap)`, with operations get, set, reset, status line and "name of option number i", and raised the table version to 36. The menu checks the table version before calling it.
   - The page declares its rows statically by key. It reloads every value from the host each time it opens, and a change goes through `rh_option_set` and into `renhdrum.ini`.
   - If the host's menu already saves every keyed row into the game's own settings file, the duplicate is harmless as long as loading that file never pushes values back into the core. Push to the core only when the player changes a row.
7. **DualSense and Steam threads.** Send adaptive-trigger packets from one thread only. Give each wired DualSense its own audio stream; when its audio device disappears, destroy the stream under its lock and look for the device again. If a Bluetooth write fails, restart the Bluetooth worker. For the Steam Controller, keep the `0x86` mode report alive and send "off" on unplug and on exit.
8. **Upstream platform checks.** totk's `check_platform.py` flags Win32 calls outside `runtime/platform/`. The bridge is Windows-only. If you want to send it upstream as a PR, move its Win32 calls into the project's platform layer first.
9. **Packaging.** Build `renhdrum.dll` with `+crt-static` and check `dumpbin /dependents` (no `VCRUNTIME140.dll`; section 7). Never ship game dumps or keys.
10. **Start from the newest upstream.** Take the latest upstream at the time you work. If upstream changes a lot between your survey and your build (Longshot reworked its menu within one day), survey again instead of applying an old patch blindly. `git apply --3way` needs the reference tag's blobs in the target repository.
11. **Local co-op: one pad per player.** The bridge sends every vibration to every pad it holds; it does not know players. In dkb-pc-recomp (Donkey Kong Bananza, a second gamepad plays Pauline) I attach only player 1's pad and leave player 2 on the game's own two-motor path. Then check every path that renews player 2's rumble: one of them also refreshed player 1's motors and rumbled the claimed pad twice.

12. **Lessons from the Eden-based Switch 2 and Switch 1 builds (Brontide-NS2, Brontide-oot).** These are C++/Qt emulators, not recomps, but the same mistakes show up:
    - **One writer per trigger.** Trigger commands from two threads undo each other. Send them from one thread.
    - **Take the settings from the core.** Build the HD Rumble page from `renhdrum_options` and keep the values in a `renhdrum_store_*` store; do not keep your own copy of each key. A new core row then needs no host rebuild (see [ui-guidelines.md](ui-guidelines.md), `integrations/qt/`).
    - **Call `renhdrum_dualsense_dsp_set_speaker` on Bluetooth too.** The `0x36` report carries the speaker; the core turns it on by itself if you forget, but the host should set it so the switch is honoured in one place.
    - **Each wired DualSense gets its own speaker.** Do not share the first pad's audio stream between two pads.
    - **Controls page.** Put the **HD Rumble** button below the controller picture, not on top of the stick test circles.

### Which controller is wired in where

| Controller | Brontide-NS2 / oot / Brontide | Longshot | totk-pc-recomp | dkb-pc-recomp |
|---|:---:|:---:|:---:|:---:|
| Switch Pro Controller | yes | yes | yes | yes |
| DualSense, wired (motors + speaker + triggers) | yes | yes | yes | yes |
| DualSense, Bluetooth (rumble + triggers + speaker) | yes | yes | yes | yes |
| Xbox One S / Series | yes | yes | yes | yes |
| Steam Controller (2026) | yes | yes | yes | yes |

Every host here calls the same core functions, so a new core release reaches all of them by replacing `renhdrum.dll` (and the header for the recomp bridge).

---

## Prebuilt Builds & Reference Kit

- Ready-to-use C bridge and reference patches (Hookshot `v2.2.0-rc.2` and Longshot): [`integrations/recomp-c/`](../../integrations/recomp-c/).
- End-user test builds and downloads: see the [website](https://ryuhdr.threedola.workers.dev/).
