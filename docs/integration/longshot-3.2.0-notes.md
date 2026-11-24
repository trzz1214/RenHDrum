# Notes on the RenHDrum path in Longshot 3.2.0

Longshot added RenHDrum in its own code in 3.2.0, and I am glad it did. While I moved my offline build to 3.2.0 I read that code closely and found a few gaps. Because of them, players on the official build do not get the full HD Rumble on a Switch Pro Controller or the full HD Haptics on a DualSense. This page lists what I found, so the Longshot developers can check it quickly.

- All file and line numbers are from Longshot tag `v3.2.0` (commit `79d192c6`).
- I found these by reading the code. My fork has every fix below. I tested it by hand on a Switch Pro Controller (Bluetooth and USB) and a DualSense (cable). I did not test the Switch 2 Pro Controller or Joy-Con items on hardware.
- Reference patch with all fixes (uses the shared `recomp-c` bridge and keeps Longshot's `pad_speaker.c` working): [`integrations/recomp-c/reference/longshot-reference.patch`](../../integrations/recomp-c/reference/longshot-reference.patch).
- The general lessons are in [recomp-games.md, section 11, lesson 15](recomp-games.md#11-lessons-from-the-longshot-totk-pc-recomp-dkb-pc-recomp-and-mkw-recomp-ports).

## Summary

| # | What a player may notice | Where | Short fix |
|---|---|---|---|
| 1 | Switch Pro Controller over Bluetooth feels coarse and late | `host/rumble_renhdrum.c:715` | Apply the core's settings after creating the engine |
| 2 | Wired Switch Pro Controller stops during a long, steady rumble | `host/pad_sdl.c:715-718` | Resend the last packet every 40 ms while rumbling |
| 3 | No rumble at all on a Switch 2 Pro Controller (USB), paired Joy-Cons or third-party Switch pads | `host/vibration.h:49, 73-75`, `host/pad_sdl.c:717` | HD path only for a real `057E:2009`; fall back to motors when the send fails |
| 4 | After closing the window, DualSense triggers stay stiff or a wired Pro Controller acts strange until replugged | `host/hos_relaunch.c:186-207`, `host/windows_stop.c:88-90` | Call the vibration shutdown inside `hos_close_finish()` |
| 5 | Rumble shape changes when Vibration strength is not 100%; wired DualSense feels softer than it should | `host/rumble_renhdrum.c:172-177, 372` | Gain after the curve; `crisp_notes = true` on a cable |
| 6 | Switch 2 games log an unimplemented import; the game cannot pause vibration | `host/nn_hid.c:1894` | Add `IsOunceVibrationDeviceMounted`, wire `PermitVibration` |

## Details

### 1. Switch Pro Controller over Bluetooth stays on the core's default level

`host/rumble_renhdrum.c` creates the Switch engine with `rh.engine_create(...)` (table entry at line 50, call at line 715) and never calls `renhdrum_settings_resolve_pure` / `renhdrum_engine_apply_settings`. The core then keeps its default Bluetooth level: 60 ms spacing and no resend. The intended level for stock SDL3 is **Compatible**: 32 ms spacing, newest value wins, resend every 40 ms.

Fix: right after creating the engine, resolve the settings and apply them.

### 2. Wired Switch Pro Controller stops during a steady rumble

`host/pad_sdl.c:715-718` sends a packet only when `rh_switch_poll` returns a new one. On USB the core skips a packet that equals the last one. Stock `SDL3.dll` does not resend raw `0x10` packets (as `host/hd_rumble.h:34` already notes), so a steady rumble fades out after a moment.

Fix: while rumbling on USB, resend the last non-silent packet every 40 ms. A patched `SDL3.dll` (see "Optional" below) does not need this.

### 3. No rumble on Switch 2 Pro, paired Joy-Cons and third-party Switch pads

`host/vibration.h:49` maps every `SDL_GamepadType` from 7 to 10 to `VIB_PAD_SWITCH`, and lines 73-75 turn that into `VIB_RH_SWITCH` whenever RenHDrum is on. `host/pad_sdl.c:717` ignores the return value of `hos_sdl_pad_effect`, so a refused send never falls back to `VIB_MOTORS`. The virtual Switch 2 Pro pad made in `host/pad_switch2.c` (lines 310-322) never sets `SendEffect` (declared at line 49), so it cannot take the HD packets.

Fix: use the RenHDrum Switch path only for a real `057E:2009` pad (not virtual, not a Steam Input stand-in), and drop a pad to motor rumble when a send fails.

### 4. Closing the window or restarting skips the RenHDrum shutdown

`hos_close_finish()` (`host/hos_relaunch.c:186-198`) is followed by `ExitProcess(0)` at line 207. The windowless stop route in `host/windows_stop.c:88-90` also goes through `hos_close_finish()` and then `TerminateProcess`. Neither calls `hos_sdl_vibration_shutdown()`, which only runs after `nnMain` returns (`host/hos_main.c:2494-2495`). So the DualSense triggers are not released, the Steam Controller does not get its "off" report, and no `SDL_Gamepad` is closed, which can leave a wired Pro Controller in its special mode.

Fix: call the shutdown inside `hos_close_finish()`. In my fork it stops the pad thread first (at most 250 ms), then calls `rh_shutdown()`, and closes the pads only if that thread has stopped, so it does not race the poll loop.

### 5. Smaller items

- `host/rumble_renhdrum.c:172-177` multiplies Vibration strength into the samples before `engine_push_half_sample`. That moves the core's thresholds and curve breakpoints when strength is not 100%. The DualSense path already applies gain after the curve (lines 332-338); the Switch path should do the same.
- `host/rumble_renhdrum.c:372` creates the wired DualSense DSP with `crisp_notes = false`. `true` is meant for a cable; `false` only for the Bluetooth `0x32` report.
- `host/vibration.h:48` picks Xbox pads by `SDL_GamepadType` 3 only; also checking VID `0x045E` is safer. `host/pad_sdl.c:449-450` holds the Xbox triggers for 1000 ms (200 ms is enough), and `renhdrum_xbox_rumble_resend` is not called.

### 6. Switch 2 HLE

`host/nn_hid.c` has no `IsOunceVibrationDeviceMounted`, so a Switch 2 game logs an unimplemented import. `PermitVibration(bool)` at line 1894 returns without doing anything.

Fix: add both overloads of `IsOunceVibrationDeviceMounted` (report mounted) and pass `PermitVibration` through to the rumble layer.

### Optional: patched SDL3 for 5 ms on the Pro Controller

RenHDrum ships two small SDL3 patches: [`0001-hdrumble.patch`](../../integrations/sdl3/sdl3-patches/0001-hdrumble.patch) and [`0002-hdrumble-idle-write.patch`](../../integrations/sdl3/sdl3-patches/0002-hdrumble-idle-write.patch). They unlock the 5 ms envelope over USB and Bluetooth and write-after-input over Bluetooth. Longshot does not track `SDL3.dll` in git, so a packaged build can use a patched DLL without touching `check_third_party.py`.

---

I am happy to help test any of this, or to split it into small pull requests if that is easier.
