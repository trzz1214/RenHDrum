# Monograph 08: Steam Controller Haptics — SDL3 Transport Analysis & Planned Architecture

> [!IMPORTANT]
> **Status: Steam Controller (2026) grip audio: experimental support, built from iafx's hardware measurements; trackpads, Steam Deck and the 2015 controller are still research.**  
> I do not own physical Steam Controller hardware. All hardware-side findings for the 2026 "Triton" controller below were measured by **iafx** (`RenHDrum#5`, `iafx/Lichii`), building on **LiveHaptics** (MIT) for the grip PCM stream, while SDL3 transport details were verified against upstream SDL 3.4.8 source code. Because I cannot test on real hardware, the 2026 grip backend is marked experimental everywhere and relies on community testers to validate pacing and tactile feel.

---

## 1. Hardware Identification & Transport Constraints

All code references below point to the official [`libsdl-org/SDL`](https://github.com/libsdl-org/SDL/tree/release-3.4.8) repository at tag `release-3.4.8` under `src/joystick/`.

### 1.1 Device Identifiers (VID `0x28DE`)
- **Steam Controller (2015, 1st Generation)**:
  - `0x1102` ( Wired USB )
  - `0x1142` ( Wireless Receiver / Dongle )
  - `0x1105` / `0x1106` ( Bluetooth LE )
  - Source: [`hidapi/steam/controller_constants.h#L40-L49`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_constants.h#L40-L49).
- **Steam Controller (2026, codenamed "Triton" in SDL3)**:
  - `0x1302` ( Wired USB )
  - `0x1303` ( Bluetooth LE )
  - `0x1304` ( Wireless Receiver / "Proteus" Dongle )
  - Source: [`controller_list.h#L632-L634`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/controller_list.h#L632-L634).
- **Steam Deck**:
  - `0x1205`
  - Source: [`hidapi/steam/controller_constants.h#L51`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_constants.h#L51).

### 1.2 Absence of USB Audio Class Endpoints
Unlike the PlayStation 5 DualSense controller—which exposes a 4-channel 48 kHz USB Audio Class device driving stereo voice coil actuators—none of the Valve controller generations expose a USB audio endpoint for haptics. Consequently, DualSense-style USB audio output cannot be used for Steam Controllers, and all haptic commands must travel through HID reports. (On the 2026 "Triton" model, however, the grip motors accept a mu-law PCM audio stream packed inside HID output reports `0x86` / `0x88`, as described in Section 3.)

---

## 2. Verified SDL 3.4.8 Driver Capabilities

### 2.1 Steam Controller 2015 (`SDL_hidapi_steam.c`)
- **Physical Actuators**: Two Linear Resonant Actuators (LRAs) mounted beneath the left and right trackpads.
- **Pulse Feature Report (`0x8F`)**: Accepting `ID_TRIGGER_HAPTIC_PULSE = 0x8F` ([`steam/controller_constants.h#L72`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_constants.h#L72)) formatted via `MsgFireHapticPulse` (`which_pad`, `pulse_duration`, `pulse_interval`, `pulse_count`, `dBgain`, `priority` in [`steam/controller_structs.h#L72-L80`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_structs.h#L72-L80)).
- **Standard Rumble Fails Silently**: In SDL 3.4.8, `HIDAPI_DriverSteam_RumbleJoystick` unconditionally returns `SDL_Unsupported()` ([`SDL_hidapi_steam.c#L1278-L1282`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam.c#L1278-L1282)). When an emulator falls back to standard SDL rumble, the 2015 Steam Controller produces **no vibration at all**.
- **Raw Effect Passthrough Works Out of the Box**: `HIDAPI_DriverSteam_SendJoystickEffect` forwards a 65-byte buffer directly as a HID feature report via `SetFeatureReport` ([`SDL_hidapi_steam.c#L1301-L1310`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam.c#L1301-L1310)), and the driver is enabled by default on Windows ([`SDL_hidapi_steam.c#L36-L41`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam.c#L36-L41)). This allows RenHDrum to dispatch `0x8F` haptic pulse reports through stock SDL3 without custom SDL patches.

### 2.2 Steam Controller 2026 / "Triton" (`SDL_hidapi_steam_triton.c`)
- **Standard Rumble (`0x80`)**: `HIDAPI_DriverSteamTriton_RumbleJoystick` dispatches output report `0x80` carrying only motor `speed` parameters ([`SDL_hidapi_steam_triton.c#L437-L459`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam_triton.c#L437-L459)).
- **Tone & Pulse Structs Defined, Raw Passthrough Blocked (`0x83`)**: SDL 3.4.8 headers define frequency-capable reports including `ID_OUT_REPORT_HAPTIC_LFO_TONE = 0x83` (`MsgHapticLfoTone` with `side`, `gain_db`, `frequency`, `duration_ms`, `lfo_freq`, `lfo_depth`) and pulse report `0x81` ([`steam/controller_structs.h#L176-L203`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_structs.h#L176-L203), [`#L225-L233`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_structs.h#L225-L233)). However, `HIDAPI_DriverSteamTriton_SendJoystickEffect` returns `SDL_Unsupported()` ([`SDL_hidapi_steam_triton.c#L476-L479`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam_triton.c#L476-L479)), preventing applications from sending `0x83` trackpad tone reports through unpatched SDL3. (This block does **not** affect the grip PCM path `0x86` / `0x88`, which opens a second HID handle via `SDL_hid_open_path`.)
- **0-Based Side Codes in `0x83`**: On physical Triton hardware, `0x83` side IDs are 0-based: `0x00` = left trackpad, `0x01` = right trackpad, `0x02` = both trackpads, `0x03` = left grip, `0x04` = right grip (measured by iafx, `RenHDrum#5`). The `0x01 = L, 0x02 = R, 0x03 = Both` comment in SDL 3.4.8 (`steam/controller_structs.h#L106`) sits on an older command struct prior to `haptic_intensity_t`, not on the Triton structs.
- **Report ID Notes (`0x84`–`0x88`)**: In SDL 3.4.8 (`steam/controller_structs.h#L225-L233`), `0x85` is named `LOG_SWEEP`, `0x86` is named `HAPTIC_SCRIPT`, and `0x84` / `0x88` are absent. On physical hardware (with SDL 3.4.14), iafx (`RenHDrum#5`) reported `0x84` as log sweep, `0x85` as script, `0x86` as PCM mode control, and `0x88` as the 64-byte grip PCM stream.

### 2.3 Steam Deck (`SDL_hidapi_steamdeck.c`)
- `HIDAPI_DriverSteamDeck_SendJoystickEffect` returns `SDL_Unsupported()` ([`SDL_hidapi_steamdeck.c#L422-L424`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steamdeck.c#L422-L424)).

### 2.4 Summary Matrix (SDL 3.4.8)

| Device | VID:PID | `SDL_RumbleJoystick` | `SDL_SendJoystickEffect` | Frequency Haptic Report in SDL 3.4.8 Headers |
|---|---|---|---|---|
| **Steam Controller (2015)** | `28DE:1102` / `1142` / `1105` / `1106` | `SDL_Unsupported()` | **Supported** (65-byte feature report) | `0x8F` (`MsgFireHapticPulse`: on/off/count/dBgain) |
| **Steam Controller (2026 "Triton")** | `28DE:1302` / `1303` / `1304` | Supported (`0x80` speed only) | `SDL_Unsupported()` | `0x83` (`MsgHapticLfoTone`: side/gain_db/frequency/duration_ms) |
| **Steam Deck** | `28DE:1205` | Supported (`0xEB` simple rumble) | `SDL_Unsupported()` | Not exposed via `SendJoystickEffect` |

---

## 3. Hardware Measurements on Triton (measured by iafx, RenHDrum#5)

> [!NOTE]
> **Third-Party Hardware Measurements**  
> I do not own the controller; the protocol details and tactile figures below were measured on physical hardware by **iafx** (`RenHDrum#5`, `iafx/Lichii`), building on **LiveHaptics** (MIT) for the PCM stream discovery.

### 3.1 Grip PCM Streaming over HID (`0x86` / `0x88`)
- **Dual-Band Playback on All 4 Actuators**: Unlike the trackpads (`0x83` single-frequency tone per side), the two grip actuators accept a mu-law PCM audio stream (`0x86` / `0x88`), so each grip plays both Low and High Switch HD Rumble bands simultaneously, and all four motors (two grips via PCM + two trackpads via `0x83`) can run at the same time (measured by iafx, `RenHDrum#5`).
- **Mode Control (`0x86`)**: `0x86 [id, op, channel, format]` enables (`op = 2`) or disables (`op = 1`) PCM mode on grip channels (`2` and `5`), with a ~200 ms settle delay after enabling. `format = 8` selects 8 kHz mu-law; `format = 9` selects 4 kHz mu-law.
- **Audio Report (`0x88`, 64 bytes)**: `report[0] = 0x88`, `report[1] = 31` (sample count per side), `report[2..32]` = 31 left-grip mu-law samples, `report[33..63]` = 31 right-grip mu-law samples.
- **No SDL3 Patch Needed for Grip PCM**: Opening a second HID handle with `SDL_hid_open_path` and writing `0x86` / `0x88` from a dedicated steady-clock thread works on stock SDL3 (whereas `0x83` trackpad tones still require either a second HID handle or an SDL3 patch).

### 3.2 Transport Formats & Packet-Driven Pacing
- The controller plays grip PCM samples at the rate packets arrive; sending packets too slowly stretches and lowers the pitch of the vibration, so a dedicated steady-clock sender thread is required (measured by iafx, `RenHDrum#5`). Earlier community estimates of ~10 ms spacing apply only to discrete tone commands, not grip PCM.
- **Measured rates by connection** (measured by iafx, `RenHDrum#5`):
  - **Wired USB (`28DE:1302`)**: mu-law 8 kHz (258 packets/s, `3.875 ms` period; 16-bit PCM exceeds link bandwidth).
  - **Wireless Dongle (`28DE:1304`)**: mu-law 4 kHz (129 packets/s, `7.75 ms` period).
  - **Bluetooth LE on Windows (`28DE:1303`)**: mu-law 4 kHz. Bluetooth outside Windows has not been measured and falls back to standard rumble.

### 3.3 Follow-Up Findings (13/11, measured by iafx, RenHDrum#5)
1. **Accumulating Buffer Drift (~1.7 ms/min)**: Continuous streaming causes latency to climb by ~17 ms every 10 minutes (the controller buffers ~80 ms and does not self-drain). Stopping transmission during silence lets the buffer drain and resets latency to baseline. RenHDrum's host loop stops sending 100 ms after the last non-silent sample.
2. **Unplug vs Power-Off Behind the Dongle**: Unplugging USB or the wireless dongle causes an in-flight HID write to fail within ~33 ms without hanging (the host falls back to standard rumble after 100 consecutive write errors). However, turning the controller off while the dongle stays plugged in produces no write error; when powered back on, the controller has lost PCM mode until re-plugged or the emulator restarts (known limitation).
3. **Per-Band Gain Table (Shape Preserved Across Amplitudes)**: Compared by feel against a DualSense (1 person, 1 controller, 2 passes), Triton's grips are ~5 dB louder flat across 40–500 Hz and ~20 dB louder at 800 Hz, preserving the same response shape across 4 amplitude levels (within 3 dB). RenHDrum applies a log-frequency interpolated gain (`-5 dB` at 40–500 Hz, `-20 dB` at 800 Hz, clamped outside) after the DualSense amplitude curve (sqrt for Switch 1, L1 law for Switch 2).
4. **Trackpad Response & Shell Resonances (for future `0x83` work)**: Trackpad response peaks around ~160 Hz and drops off below 60 Hz, with shell-rattle resonances reported around 160–170 Hz, 260–270 Hz, and 500–540 Hz (measured by iafx, `RenHDrum#5`).

---

## 4. Current Roadblocks & Practical Limitations

1. **No Physical Hardware for Local Testing**: I do not own a 2015 or 2026 Steam Controller, so the experimental 2026 grip support relies on iafx's measurements and community testers for real-hardware validation.
2. **Steam / Steam Input Contention & PCM Mode Reset**: On Windows, `SDL_hid_open_path` opens a shared handle even while Steam is running (observed by iafx, `RenHDrum#5`; Linux is untested), so exiting Steam may not be required on Windows. However, an active Steam client can reset the controller's PCM mode mid-session (observed by iafx; not yet handled).
3. **Single-Frequency Command per Side (Trackpads Only)**: The one-frequency-per-side constraint applies only to the trackpads (`0x83`) and the 2015 controller (`0x8F`). The 2026 grips play both Low and High HD Rumble bands simultaneously via PCM.
4. **Direct HID Write Required for Trackpad Tones (`0x83`)**: Stock SDL 3.4.8 blocks `SendJoystickEffect` on Triton, so sending `0x83` trackpad tones requires either a small SDL3 driver patch or direct `hidapi` handle access (grip PCM already uses a second `SDL_hid_open_path` handle).
5. **Known Limits of the Experimental Grip Implementation**:
   - **Power-off behind the dongle**: powering the controller off while the dongle remains plugged in loses PCM mode until re-plugged or restarted.
   - **Startup-only DSP settings**: volume and Crisp Notes settings for the Steam grip DSP are captured when the controller is opened and do not update live from the HD Rumble dialog while connected.
   - **Bluetooth outside Windows**: `28DE:1303` on non-Windows platforms is unmeasured and falls back to standard rumble.
6. **Target Emulator Constraint**: Ryuhdr runs on SDL2 with its vibration logic frozen as the reference baseline, so Steam Controller support will not be added to Ryuhdr. Experimental 2026 grip support lives in RenHDrum and the SDL3-based Eden Switch 2 patch series (`integrations/eden-switch2/`).

---

## 5. Current Implementation & Remaining Research

RenHDrum now implements the **2026 Steam Controller ("Triton") grip PCM backend** as an experimental profile of `DualSenseHapticsDsp` (`src/valve/grip.rs`), exposed via `renhdrum_valve_grip_dsp_create` and `renhdrum_valve_grip_build_report` (`include/renhdrum.h`):

- **Implemented — Triton Grip PCM (`0x86` / `0x88`, Experimental)**:
  - Reuses the 48 kHz dual-band DualSense sine synthesizer (preserving Switch 1 sqrt and Switch 2 L1 amplitude laws), applies the per-band `-5 dB` / `-20 dB` grip gain table, downsamples to 8 kHz (USB) or 4 kHz (dongle / Windows Bluetooth), encodes mu-law G.711, and packs 64-byte `0x88` reports.
  - The host opens a second HID handle via `SDL_hid_open_path`, enables PCM via `0x86`, sends on a steady monotonic timer, stops 100 ms after the last vibration to drain the controller buffer, and falls back to standard SDL rumble after 100 consecutive write errors.
- **Still Research (Not Yet Implemented)**:
  - **Steam Controller 2015 (`0x8F` via Stock SDL3)**: Synthesize 65-byte `ID_TRIGGER_HAPTIC_PULSE` (`0x8F`) feature reports where pulse period $(\text{on} + \text{off}) \approx 1 / f$ and amplitude maps to `dBgain`, dispatched through `SDL_SendJoystickEffect`.
  - **Steam Controller 2026 Trackpads (`0x83`)**: Optional high-band tone accompaniment avoiding the shell resonance bands noted in Section 3.3.
  - **Steam Deck (`0xEA`)**: Deferred to a later phase.
