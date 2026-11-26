# Longshot v3.2.0-rh2 Changes for RenHDrum 0.6.0

This page lists the changes in my `Longshot + RenHDrum` `v3.2.0-rh2` build (fork branch `renhdrum-137`, commit `8d0b4863`) on top of upstream Longshot `v3.2.0` (`79d192c6`). Everything is broken down by feature so the Longshot developers can take whatever fits and skip the rest. I am not opening a pull request for these, and I am happy to answer any questions.

---

## 1. RenHDrum 0.6.0 ABI check (`RENHDRUM_ABI_COMPATIBLE`), shared `recomp-c` bridge, and HD Rumble menu

- **What it does:** Updates `renhdrum.h` to `0.6.0` (`0x000600`), switches the DLL version check in `rh_load()` from exact `major.minor` matching to `RENHDRUM_ABI_COMPATIBLE(ver)`, and replaces `host/rumble_renhdrum.{c,h}` with the shared `recomp-c` bridge plus a dynamic **HD Rumble** menu page driven by `renhdrum_options()` (exposed through `MkwVkHost.rh_option`, while keeping `pad_speaker.c` on the same wired DualSense audio stream).
- **Why:** Longshot `v3.2.0`'s 0.5 bridge rejects `renhdrum.dll` `0.6.0` because its check expects branch `0x0005xx`; `RENHDRUM_ABI_COMPATIBLE` accepts `0.6.0` and future backwards-compatible `0.x` releases without a rebuild, and the shared bridge keeps all five supported controllers and INI settings in sync.
- **Main files:** `third_party/renhdrum/include/renhdrum.h`, `host/renhdrum.h`, `host/renhdrum_recomp_bridge.{c,h}`, `host/rh_host.h`, `host/rx_backend.c`, `gpu/vk/vk_api.h`, `gpu/vk/vk_menu.{h,cpp}`, `gpu/vk/menu/hd_rumble.{h,cpp}`, `gpu/vk/menu/controller.cpp`, `tests/test_renhdrum.py`, `tests/test_hd_rumble_menu.py`.
- **How to turn off:** Toggle **HD Rumble** off in the Controls menu, or set `HOS_HD_RUMBLE=0`, `HOS_RENHDRUM=0`, or `HOS_RUMBLE_BACKEND=motors`.
- **Tested on:** Switch Pro Controller (USB & Bluetooth) and DualSense (USB & Bluetooth) on Windows 11, plus unit tests (`test_renhdrum`, `test_hd_rumble_menu`, `test_vibration_menu`, `test_vibration_pick`).
- **Recommendation & dependencies:** **Recommended** for the 1-line `RENHDRUM_ABI_COMPATIBLE` version-check update (required if you ship or support `renhdrum.dll` `0.6.0`; keeping `0.5.0` works without it). **Optional** for replacing `host/rumble_renhdrum.c` with the full `recomp-c` bridge and dynamic HD Rumble menu page.

## 2. Core rumble reliability fixes on top of Longshot `v3.2.0`

- **What it does:** Applies resolved Switch Pro settings right after creating the engine (fixing coarse Bluetooth pacing), resends wired Switch Pro packets every 40 ms on stock SDL3 (or activates 5 ms no-queue / idle-write when patched `SDL3.dll` is present), restricts the Switch HD path to real `057E:2009` controllers with automatic motor fallback when sends fail (fixing paired Joy-Cons and third-party Switch pads), calls `hos_sdl_vibration_shutdown()` inside `hos_close_finish()` before `ExitProcess` / `TerminateProcess`, applies gain after the curve with `crisp_notes = true` on wired DualSense, and implements `IsOunceVibrationDeviceMounted` + `PermitVibration` in `nn_hid.c`.
- **Why:** Fixes the six concrete gaps listed in [`longshot-3.2.0-notes.md`](longshot-3.2.0-notes.md) where players lost Switch Pro Bluetooth detail, steady wired rumble, third-party / Joy-Con motor fallback, or clean trigger and controller release on window close.
- **Main files:** `host/renhdrum_recomp_bridge.c`, `host/pad_sdl.c`, `host/vibration.h`, `host/hos_relaunch.c`, `host/nn_hid.c`, `host/pad_speaker.c`, `tests/test_windows_stop.py`, `tests/test_vibration_pick.py`.
- **How to turn off:** These are reliability fixes; you can still force standard motor rumble via `HOS_RUMBLE_BACKEND=motors` or disable HD Rumble in the Controls menu.
- **Tested on:** Switch Pro Controller (USB & Bluetooth) and DualSense (USB & Bluetooth) on Windows 11, plus `test_windows_stop.py` and `test_vibration_pick.py`; paired Joy-Cons and third-party Switch pads were verified by code review and unit tests rather than hardware.
- **Recommendation & dependencies:** **Recommended**. Works with both `renhdrum.dll` `0.5.0` and `0.6.0` (patched `SDL3.dll` `0001` / `0002` is optional for 5 ms over Bluetooth and USB on Switch Pro 1).

## 3. Context-aware DualSense adaptive triggers (Tier 2) and `HD Triggers` toggle

- **What it does:** Adds an OoT game-state adaptive trigger layer on R2 (L2 stays on Tier 1 rumble-reactive HD Haptics triggers): slingshot, bow, and fire/ice/light arrows apply smooth `TENSION` while aiming and send `OFF` on shot; Hookshot / Longshot apply a `TWO_STAGE` notch with deep-squeeze firing (clearing ZR past 95% on real analog triggers), cancel-on-release after a 250 ms hold, and `IMPACT_PULSE` on shot; Din's Fire uses `ENGINE_ROUGH` gated by game rumble hysteresis (`0.03` on / `0.015` off for 100 ms); Megaton Hammer uses `HEAVY_LOAD` while swinging, `IMPACT_PULSE` on impact (`>= 0.06` rumble), then quake `ENGINE_ROUGH`; pause menus and Ocarina audio cleanly clear both triggers. In the bridge, impact pulses beat Tier 1 for 150 ms while active Tier 1 rumble beats sustained presets; also adds a host-side **DualSense: HD Triggers** toggle (`hd_triggers`, on by default, saved in `renhdrum.ini`).
- **Why:** Gives DualSense players item-specific trigger resistance and recoil while letting them switch back to pure Tier 1 HD Haptics triggers in one toggle without turning off HD Haptics.
- **Main files:** `host/oot_trigger.{h,c}`, `host/oot_trigprobe.{h,c}` (shared Link pointer-chain reader `hos_oot_link_read`, plus optional `HOS_TRIG_PROBE=1` probe logger), `host/oot_state_tap.c`, `host/oot_menu.c`, `host/nn_hid.c`, `host/nn_audio.c`, `host/pad_sdl.c` (`hos_pad_r2_depth`), `host/renhdrum_recomp_bridge.{c,h}` (`rh_trigger_preset`, `rh_game_rumble_amp`, `hd_triggers`), `gpu/vk/menu/hd_rumble.cpp`, `tests/test_oot_trigger.py`, `tests/test_oot_trigprobe.py`.
- **How to turn off:** Toggle **DualSense: HD Triggers** off in the HD Rumble menu (immediately sends `RENHDRUM_TRIG_OFF` to L2/R2 and falls back to Tier 1), or set `HOS_TRIG_PRESETS=0`. Sub-features can also be disabled or tuned with `HOS_TRIG_DEEP_FIRE=0`, `HOS_TRIG_HOOK_CANCEL=0`, `HOS_TRIG_SLING`, `HOS_TRIG_BOW`, `HOS_TRIG_HOOK`, `HOS_TRIG_DIN`, `HOS_TRIG_HAMMER`, `HOS_TRIG_QUAKE`, and related `HOS_TRIG_*` variables.
- **Tested on:** DualSense over USB on Windows 11 across slingshot, bow / elemental arrows, Hookshot / Longshot (aim, deep fire, cancel), Din's Fire, Megaton Hammer, Ocarina, pause menu, and live menu toggling across restarts, plus `test_oot_trigger.py`; not yet tested over DualSense Bluetooth.
- **Recommendation & dependencies:** **Optional** (game-specific layer). Requires `renhdrum.dll` `0.6.0` for `renhdrum_trigger_preset_*`; on an older DLL without those exports, the bridge skips presets silently and stays on Tier 1. See [`ui-guidelines.md`](ui-guidelines.md#3-basic-settings-shown-on-the-card-of-the-controller-in-use) for the host toggle pattern.

## 4. DualSense PS button and split touchpad halves (Map / Bag)

- **What it does:** Keeps the PS / Home button opening Longshot's overlay (`opts.pad_menu = 1`) and maps clicking the left half of the DualSense touchpad (`x < 0.5`) to holding `Minus` (Map / Key Items / Threads of Time) and the right half (`x >= 0.5`) to holding `Plus` (Pause / Items menu), injected into `buttons` before `hold_toggle_apply` just like physical buttons.
- **Why:** Using the two touchpad halves feels natural on a DualSense and behaves identically to physical `Minus` / `Plus`, including switching directly between the Map screen and the Items menu.
- **Main files:** `host/oot_pad_light.h` (`oot_touch_zone`), `host/hos_sdl.{c,h}` (`hos_sdl_pad_finger_x`), `host/pad_sdl.c` (`hos_pad_touch_zone_held`), `host/nn_hid.c`, `tests/test_oot_pad_light.py`.
- **How to turn off:** Set `HOS_TOUCH_ZONES=0` to restore upstream behavior (touchpad click opens the F1 menu).
- **Tested on:** DualSense over USB and Bluetooth on Windows 11 (verified PS button, Map-to-Bag switching, and `test_oot_pad_light.py`).
- **Recommendation & dependencies:** **Optional**. No `renhdrum.dll` dependency; uses standard SDL3 (`SDL_GetGamepadTouchpadFinger`).

## 5. DualSense light bar (hit flash & low-heart breath) and low-battery mic LED

- **What it does:** Flashes the DualSense light bar red for 150 ms when Link takes damage (capped at 2 flashes/s), breathes orange `(255, 64, 0)` on a 1 s cycle at 1 heart or less, and returns to SDL's default player-1 blue `(0, 0, 64)` otherwise; also breathes the microphone LED (mode 2) when on battery at 20% or below (off above 25% or when wired / charging) while leaving the 5 white player LEDs to SDL's default.
- **Why:** Provides glanceable damage and low-battery feedback without distracting indicator LEDs or interfering with rumble/trigger traffic (updates are rate-limited to `>= 80 ms` under `tri_lock`, sent only on change, and restored on detach).
- **Main files:** `host/oot_pad_light.h` (`oot_bar_color`, `oot_mic_mode`), `host/oot_leds.c`, `host/oot_trigger.c`, `host/renhdrum_recomp_bridge.{c,h}` (`rh_leds`), `host/rh_host.h`, `tests/test_oot_pad_light.py`.
- **How to turn off:** Set `HOS_PAD_LEDS=0` (leaves both the light bar and the microphone LED completely untouched).
- **Tested on:** DualSense over USB and Bluetooth on Windows 11 (verified hit flash, low-heart breath, and 0 dropped Bluetooth `0x36` packets alongside LED updates), plus `tests/test_oot_pad_light.py`.
- **Recommendation & dependencies:** **Optional**. No `renhdrum.dll` dependency; uses SDL3 `SDL_SetGamepadLED`, `SDL_GetGamepadPowerInfo`, and `SDL_SendJoystickEffect` (if `SDL_SetGamepadLED` is missing, logs once and skips the light bar).

## 6. Windows volume pin for DualSense USB audio haptics

- **What it does:** While DualSense USB audio-stream haptics are active, pins the matched DualSense Windows audio endpoint to 100% (unmuted) via `IAudioEndpointVolume`, checks it every ~2 s, and restores the user's previous volume and mute state on detach, audio stream loss, or `rh_shutdown()`.
- **Why:** On Windows, the DualSense's 4-channel audio device scales both the controller speaker and the two haptic voice coils; if Windows lowers that endpoint's volume (for example to 30%), USB haptics become noticeably weaker than Bluetooth `0x36` haptics. (Note: unplugging the cable mid-game removes the Windows endpoint before restore, leaving Windows at 100% for the next plug-in.)
- **Main files:** `host/hos_sdl.{c,h}` (`pad_endpoint`, `hos_sdl_pad_volume`), `host/renhdrum_recomp_bridge.c` (`rh_ds_vol_pin`, `rh_ds_vol_release`).
- **How to turn off:** Set `HOS_PAD_VOLUME_PIN=0`.
- **Tested on:** DualSense over USB on Windows 11 (verified volume lowered to 30% -> pinned to 100% in-game -> restored to 30% on exit, with no trigger stutter).
- **Recommendation & dependencies:** **Recommended** for any Windows host driving DualSense USB audio haptics. Windows-only (COM `IAudioEndpointVolume`); no `0.6.0` DLL dependency.

## 7. Switch 2 Pro Controller USB HD Rumble 2 (Experimental)

- **What it does:** Adds `v_send_effect` on Longshot's host-managed virtual Switch 2 Pro joystick (`057E:2069`) to forward 64-byte `0x02` raw HD Rumble 2 reports from `renhdrum_pro2_*` over USB, removes the `057E:2069` exclusion in `vib_choose`, and fixes the `SDL_IsJoystickVirtual(SDL_GetJoystickID(joy))` signature in the bridge so `057E:2069` is admitted while other SDL virtual joysticks remain ignored.
- **Why:** Enables experimental Switch 2 Pro USB HD Rumble 2 in Longshot without needing SDL3 patch `0003` or `libusb`, with an attach-time silent probe, 3 silent stop reports on detach, and automatic fallback to standard motor rumble after `RH_PRO2_FAILS_MAX` (100) consecutive write failures or if `rumble_stuck` triggers.
- **Main files:** `host/pad_switch2.c`, `host/pad_sdl.c`, `host/renhdrum_recomp_bridge.c`.
- **How to turn off:** Set `HOS_SWITCH2=0` (disables Longshot's Switch 2 USB pad driver) or turn off HD Rumble in the Controls menu.
- **Tested on:** Compile-checked, unit-tested (`test_renhdrum`, `test_vibration_pick`, `test_vibration_menu`), smoke-tested with default and `HOS_SWITCH2=0`, and regression-tested with DualSense; **not yet tested on real Switch 2 Pro hardware** (labeled `experimental` in logs and `rh_status_line()`). Note: `host/hos_sdl.c` (`hos_sdl_pad_virtual`) still passes `SDL_Joystick *` instead of `SDL_JoystickID` and was intentionally left unchanged so virtual Joy-Con 2 behavior stays untouched.
- **Recommendation & dependencies:** **Optional (Experimental)**. Requires `renhdrum.dll` `0.6.0` (`renhdrum_pro2_*` exports); close Steam completely when testing on Windows.

## 8. Fork-only items (not intended for upstream)

- **Signal probe (`HOS_TRIG_PROBE=1`):** `host/oot_trigprobe.c` logs Link verb / item / flag / trigger-layer diagnostics (`trigprobe:` and `trigger: send ...`) when `HOS_TRIG_PROBE=1` is set. **Fork-only** debugging tool; safe to omit.
- **Offline launcher packaging:** The `v3.2.0-rh2` standalone bundle replaces the launcher's online release check with an offline stub pointing at the bundled toolchain so the package never contacts upstream servers. Not part of the Longshot repository patch (**Fork-only**).

---

## Getting the code and references

- **Reference patch (applies cleanly on upstream `v3.2.0`, commit `79d192c6`; GPL-3.0-or-later):** [`integrations/recomp-c/reference/longshot-reference.patch`](../../integrations/recomp-c/reference/longshot-reference.patch)
- **Full source archive:** The `Longshot-RenHDrum-v3.2.0-rh2-source` archive on the [website download section](https://ryuhdr.threedola.workers.dev/#download)
- **UI guidelines for the `HD Triggers` host toggle:** [`docs/integration/ui-guidelines.md`](ui-guidelines.md#3-basic-settings-shown-on-the-card-of-the-controller-in-use)
- **Earlier line-by-line notes on `v3.2.0`:** [`docs/integration/longshot-3.2.0-notes.md`](longshot-3.2.0-notes.md)
