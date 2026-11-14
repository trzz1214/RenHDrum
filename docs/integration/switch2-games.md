# Switch 2 Games HD Rumble Integration

This guide describes how to integrate RenHDrum into an Eden-family (or PokeDinner / yuzu derived) emulator to enable authentic HD Rumble in Switch 2 titles across DualSense, Switch Pro Controller 1, Xbox One, and experimental Steam Controller (2026) gamepads.

Background and physical analysis: [research/11: Switch 2 HD Rumble](../research/11-switch2-hd-rumble.md). Before shipping, go through the [integration checklist](recomp-games.md#integration-checklist).

> [!NOTE]
> **Supported Controllers**: `✅ DualSense` · `✅ Nintendo Pro Controller` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)`.

> [!WARNING]
> **CRITICAL: Enable Accurate Vibration / 15 ms Throttle Bypass**
> For authentic HD Rumble synthesis, host emulators (Eden, yuzu, PokeDinner derivatives) **must** enable `Accurate Vibration` (`enable_accurate_vibrations = true`) in controller settings, or bypass the 15 ms rate-limiting block in `src/hid_core/frontend/emulated_controller.cpp`. If accurate vibration is disabled, the host frontend rate limiter will swallow and drop micro-timed HD Rumble packets before they ever reach RenHDrum!

> [!IMPORTANT]
> **Licensing & Clean Boundary**: RenHDrum is licensed under MIT OR Apache-2.0. The emulator integration relies exclusively on the public C ABI defined in `include/renhdrum.h`. Do not copy GPL emulator code into this repository; use function signatures and architectural descriptions only.

---

## 1. IPC Layer: Enabling Command 214

In baseline Switch 2 emulator branches (such as PokeDinner 2.5.0), the IPC method for command 214 (`SendVibrationValuesInMode`) is typically an empty stub that returns success and drops incoming vibration packets.

To receive vibration from Switch 2 titles (such as Donkey Kong Bananza and Mario Kart World), replace the stub in `IHidServer` with an active dispatcher. Credit for the dispatching pattern belongs to **neo Luigi** (Eglantine PR #155, commits `2b413a46a2` and `465aa33077`).

### 1.1 Command 214 Dispatching Pattern

In `src/core/hle/service/hid/hid_server.cpp`:

1. Define `SendNx2VibrationValues` to accept:
   - `u64 mode`: The 64-bit mode flag (`Mode == 2` in MKW, `0` in classic titles).
   - `AppletResourceUserId aruid`: The process resource identifier.
   - `vibration_handles`: Buffer array of `Core::HID::VibrationDeviceHandle`.
   - `vibration_values`: Buffer array of `Core::HID::VibrationValue` (16 bytes each: `amp_low`, `freq_low`, `amp_high`, `freq_high`).
2. Iterate through matched index pairs `min(handles.size(), values.size())`:
   - Call `GetResourceManager()->SendVibrationValue(aruid.pid, handles[i], values[i])`.
   - Treat any errors as non-fatal/best-effort (log as debug rather than returning error codes that games might treat as fatal crashes).
3. Return `R_SUCCEED()`.

### 1.2 Status of Commands 213 and 220

- **Command 213 (`SendVibrationValueInMode`)**: A genuine dispatcher is implemented in reference testbeds (`Brontide-NS2 1.1.0` and `Brontide-oot 1.0.1`), unpacking the single handle and 16-byte `VibrationValue` from the raw `0x20` payload and routing it to `ResourceManager::SendVibrationValue`. Across 25 in-game trace sessions on retail titles (*Donkey Kong Bananza*, *Mario Kart World*, *Ocarina of Time* Switch 2 Edition), zero calls were observed (100% of game traffic flows through command 214).
- **Command 220 (`IsOunceVibrationDeviceMounted`)**: Returns a 1-byte boolean `0`. Retail titles do not invoke command 220 during gameplay.

### 1.3 Switch 2 Rumble Mode Notification (`renhdrum_set_nx2_mode`)

When processing IPC command 214 (`SendNx2VibrationValues`), the host runtime notifies RenHDrum that Switch 2 rumble mode is active:

```cpp
// Switch 2 guest IPC command 214 active
renhdrum_set_nx2_mode(2);
```

When classic Switch 1 vibration commands are processed, notify RenHDrum to keep mode 0:

```cpp
// Switch 1 classic vibration mode (untouched)
renhdrum_set_nx2_mode(0);
```

#### How RenHDrum Handles Modes:
- **`mode == 2` (Switch 2 Active)**:
  - **Sony DualSense**: Automatically passes vibration amplitudes through the fixed Mode 2 law (`a <= 0.001 -> 0`, else `min(1, 3.5 * a^0.75)`, no EQ) before DSP synthesis, matching physical voice-coil dynamics.
  - **Nintendo Switch Pro Controller 1**: Automatically maps amplitudes through the Pro curve `PRO_DEFAULT` (`T = 0.004, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950`), silencing micro-vibrations at or below `0.004` instead of lifting them, and applies a 1.2x onset boost for the first 25 ms on low frequencies below 150 Hz.
  - **Microsoft Xbox One S**: Maps amplitudes through the same Pro curve (`PRO_DEFAULT`, `T = 0.004`, without the 1.2x boost), then passes them to the Switch 1 four-motor converter.
- **`mode == 0` (Classic Switch 1)**:
  - Input amplitudes pass through 100% bit-exact and untouched.

---

## 2. Host Driver Layer: Routing via `_half_` Entry Points

Once `ResourceManager` forwards the 16-byte `VibrationValue` to the input backend (`sdl_driver.cpp`), the sample must be directed to RenHDrum. Because each handle represents one side of a controller, use RenHDrum's individual half-sample entry points.

### 2.1 Identifying Channel and Parameters

Each vibration call supplies:
- `is_right`: `false` for left actuator, `true` for right actuator.
- `low_amp`: Float `[0.0f, 1.0f]`.
- `low_freq`: Float in Hz (e.g. 40–400 Hz).
- `high_amp`: Float `[0.0f, 1.0f]`.
- `high_freq`: Float in Hz (e.g. 400–1252 Hz).
- `now_us`: Monotonic timestamp in microseconds (via `std::chrono` or host clock).

### 2.2 Calling the Appropriate Controller Engine

Depending on the detected controller device attached to the player slot, route to the corresponding RenHDrum engine:

#### A. PlayStation 5 DualSense (Audio Haptics DSP)
Target the voice coil DSP instance:

```cpp
// Target continuous audio DSP instance
renhdrum_dualsense_dsp_set_half_target(
    ds5_dsp,
    is_right,
    low_amp,
    low_freq,
    high_amp,
    high_freq
);
```

On wired USB, audio callbacks invoke `renhdrum_dualsense_dsp_fill_f32`. Over Bluetooth, a periodic 10.67 ms timer calls `renhdrum_dualsense_dsp_build_bt_report` (HID report `0x32`). See [dualsense.md](dualsense.md) for full audio device setup.

#### B. Nintendo Switch Pro Controller 1 (LRA HD Rumble)
Target the packet encoder instance:

```cpp
renhdrum_engine_push_half_sample(
    engine,
    now_us,
    is_right,
    low_amp,
    low_freq,
    high_amp,
    high_freq
);
```

The engine generates sub-command `0x10` packets scheduled at 15 ms intervals for USB and paced via token-bucket windows for Bluetooth. See [sdl3-host.md](sdl3-host.md).

#### C. Xbox One S / Xbox Series (Four-Motor ERM Translator)
Target the Xbox rumble translator:

```cpp
renhdrum_xbox_rumble_push_half_sample(
    xbox_rumble,
    now_us,
    is_right,
    low_amp,
    low_freq,
    high_amp,
    high_freq
);
```

The translator ticks on a 10 ms (USB) or 20 ms (Bluetooth) cadence, outputting 4-motor values (left/right grips and left/right triggers) over `rawinput` or Windows.Gaming.Input (WGI). See [xbox.md](xbox.md).

---

## 3. Preventing Hanging Rumble Defects

Do **not** fall back to monolithic duration calls such as `SDL_RumbleGamepad(..., 2000)`. Switch 2 titles routinely signal the end of a rumble sequence simply by ceasing to send new IPC commands. A fixed 2-second duration causes gamepads to buzz continuously during pause menus and loading screens (research/11, section 4).

Instead:
1. Always let RenHDrum's internal silence watchdog decay the signal.
2. In the emulator host driver, if HD Rumble is toggled off in settings, route through standard SDL rumble with short hold durations (e.g. 50–100 ms) rather than 2000 ms.
3. Apply settings immediately when the user changes preferences by re-invoking `renhdrum_engine_apply_settings` under a driver mutex.

---

## 4. Reference Testbeds

Two reference implementations combining command 214 dispatching with RenHDrum 0.3.0 and Profile-Guided Optimization (PGO) are available:
1. **Brontide-NS2 1.1.0** (based on PokeDinner 2.5.0 by Franseven): Tuned for *Donkey Kong Bananza* and *Mario Kart World*.
2. **Brontide-oot 1.0.1** (based on Fritchii v5.5 by Frizti, Lichii by zortos293, and Dynamic FPS by Dark) - tuned for Ocarina of Time (Switch 2 Edition) with comprehensive AMD/NVIDIA ahead-of-time shader preparation (120,461 pipelines), 10 GiB disk cache allocation, DLSS presets, lower memory footprint, and RenHDrum 0.3.0 HD Rumble.
