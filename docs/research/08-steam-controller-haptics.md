# Monograph 08: Steam Controller Haptics — SDL3 Transport Analysis & Planned Architecture

> [!IMPORTANT]
> **Status: Research Only — Not Yet Implemented (Beta Planned, Needs Community Testers)**  
> The findings and design in this monograph represent source-code verification against upstream SDL3 and public hardware documentation. **No runtime code has been written yet**, and the author does not currently own physical Steam Controller hardware. A future experimental beta in an SDL3-based emulator will rely on community testers with real hardware to validate pacing and tactile feel.

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
Unlike the PlayStation 5 DualSense controller—which exposes a 4-channel 48 kHz USB Audio Class device driving stereo voice coil actuators—none of the Valve controller generations expose a USB audio endpoint for haptics. Consequently, the DualSense audio DSP pipeline in RenHDrum cannot be used for Steam Controllers. All haptic commands must travel through HID reports.

---

## 2. Verified SDL 3.4.8 Driver Capabilities

### 2.1 Steam Controller 2015 (`SDL_hidapi_steam.c`)
- **Physical Actuators**: Two Linear Resonant Actuators (LRAs) mounted beneath the left and right trackpads.
- **Pulse Feature Report (`0x8F`)**: Accepting `ID_TRIGGER_HAPTIC_PULSE = 0x8F` ([`steam/controller_constants.h#L72`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_constants.h#L72)) formatted via `MsgFireHapticPulse` (`which_pad`, `pulse_duration`, `pulse_interval`, `pulse_count`, `dBgain`, `priority` in [`steam/controller_structs.h#L72-L80`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_structs.h#L72-L80)).
- **Standard Rumble Fails Silently**: In SDL 3.4.8, `HIDAPI_DriverSteam_RumbleJoystick` unconditionally returns `SDL_Unsupported()` ([`SDL_hidapi_steam.c#L1278-L1282`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam.c#L1278-L1282)). When an emulator falls back to standard SDL rumble, the 2015 Steam Controller produces **no vibration at all**.
- **Raw Effect Passthrough Works Out of the Box**: `HIDAPI_DriverSteam_SendJoystickEffect` forwards a 65-byte buffer directly as a HID feature report via `SetFeatureReport` ([`SDL_hidapi_steam.c#L1301-L1310`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam.c#L1301-L1310)), and the driver is enabled by default on Windows ([`SDL_hidapi_steam.c#L36-L41`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam.c#L36-L41)). This allows RenHDrum to dispatch `0x8F` haptic pulse reports through stock SDL3 without custom SDL patches.

### 2.2 Steam Controller 2026 / "Triton" (`SDL_hidapi_steam_triton.c`)
- **Standard Rumble (`0x80`)**: `HIDAPI_DriverSteamTriton_RumbleJoystick` dispatches output report `0x80` carrying only motor `speed` parameters ([`SDL_hidapi_steam_triton.c#L437-L459`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam_triton.c#L437-L459)).
- **Tone & Pulse Structs Defined, Raw Passthrough Blocked**: SDL 3.4.8 headers define frequency-capable reports including `ID_OUT_REPORT_HAPTIC_LFO_TONE = 0x83` (`MsgHapticLfoTone` with `side`, `gain_db`, `frequency`, `duration_ms`, `lfo_freq`, `lfo_depth`) and pulse report `0x81` ([`steam/controller_structs.h#L176-L203`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_structs.h#L176-L203), [`#L225-L233`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/steam/controller_structs.h#L225-L233)). However, `HIDAPI_DriverSteamTriton_SendJoystickEffect` returns `SDL_Unsupported()` ([`SDL_hidapi_steam_triton.c#L476-L479`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steam_triton.c#L476-L479)), preventing applications from sending `0x83` tone reports through unpatched SDL3.

### 2.3 Steam Deck (`SDL_hidapi_steamdeck.c`)
- `HIDAPI_DriverSteamDeck_SendJoystickEffect` returns `SDL_Unsupported()` ([`SDL_hidapi_steamdeck.c#L422-L424`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_steamdeck.c#L422-L424)).

### 2.4 Summary Matrix (SDL 3.4.8)

| Device | VID:PID | `SDL_RumbleJoystick` | `SDL_SendJoystickEffect` | Frequency Haptic Report in SDL 3.4.8 Headers |
|---|---|---|---|---|
| **Steam Controller (2015)** | `28DE:1102` / `1142` / `1105` / `1106` | `SDL_Unsupported()` | **Supported** (65-byte feature report) | `0x8F` (`MsgFireHapticPulse`: on/off/count/dBgain) |
| **Steam Controller (2026 "Triton")** | `28DE:1302` / `1303` / `1304` | Supported (`0x80` speed only) | `SDL_Unsupported()` | `0x83` (`MsgHapticLfoTone`: side/gain_db/frequency/duration_ms) |
| **Steam Deck** | `28DE:1205` | Supported (`0xEB` simple rumble) | `SDL_Unsupported()` | Not exposed via `SendJoystickEffect` |

---

## 3. Unverified Community Reports

> [!WARNING]
> **Unverified / Community Report Only**  
> The items below originate from third-party community projects (such as PadForge and standalone haptic "singer" utilities) and **have not been verified in SDL 3.4.8 or on physical hardware**:
> - **8 kHz PCM Streaming over HID (`0x86` / `0x88` on Triton)**: Community notes suggest Triton can accept raw 8 kHz PCM frames over HID output reports `0x86` or `0x88`. In SDL 3.4.8 (`steam/controller_structs.h:225-233`), `0x86` is defined as `HAPTIC_SCRIPT` and `0x88` is absent. This capability remains unconfirmed.
> - **100–1300 Hz Usable Frequency Range**: The 100–1300 Hz band cited in community tools is a software filter clamp (`ApplyToneFilter` in PadForge), not an official mechanical specification from Valve.
> - **~10 ms Inter-Command Spacing & Latency Figures**: Community "singer" utilities typically space haptic commands by ~10 ms to avoid command drops; exact safe pacing intervals under continuous emulator load remain unverified.

---

## 4. Current Roadblocks & Practical Limitations

1. **No Physical Hardware for Local Testing**: Development relies on community testers who own a 2015 or 2026 Steam Controller.
2. **Steam / Steam Input Exclusive Device Contention**: When the Steam client or Steam Input is active, it claims exclusive control over the controller's HID interfaces. Players will need to exit Steam or disable Steam Input for the controller while running the emulator.
3. **Single-Frequency Command per Side**: Nintendo Switch HD Rumble provides two simultaneous frequency bands (Low + High) per left/right channel, whereas a Steam Controller haptic command drives one frequency per side at a time. The two bands must be merged or arbitrated per frame.
4. **Direct HID Write Required for Triton (`0x83`)**: Because stock SDL 3.4.8 blocks `SendJoystickEffect` on Triton, sending `0x83` tone commands requires either a small SDL3 driver patch or direct `hidapi` handle access.
5. **Target Emulator Constraint**: Ryuhdr runs on SDL2 with its vibration logic frozen as the reference baseline. Consequently, experimental Steam Controller support will not be added to Ryuhdr; it is planned for an SDL3-based emulator (target emulator not yet finalized).

---

## 5. Planned Architecture (Not Yet Implemented)

RenHDrum plans to introduce a **third output path ("Valve Haptics Backend")** alongside the existing Nintendo Switch Subcommand `0x10` encoder and the DualSense 48 kHz audio synthesizer:

- **Dominant-Band Selection & Pacing**: On each frame, for each controller side (left and right), the backend selects the dominant band (higher perceptual amplitude between Low and High), converts its frequency and amplitude into a Valve haptic command, and dispatches updates at roughly ~10 ms intervals (subject to tuning from community hardware feedback).
- **Phase 1 — Steam Controller 2015 (`0x8F` via Stock SDL3)**: Synthesize 65-byte `ID_TRIGGER_HAPTIC_PULSE` (`0x8F`) feature reports where pulse period $(\text{on} + \text{off}) \approx 1 / f$ and amplitude maps to `dBgain`, dispatched directly through `SDL_SendJoystickEffect` without patching SDL3.
- **Phase 2 — Steam Controller 2026 "Triton" (`0x83` via Direct HID / Patched SDL3)**: Dispatch `ID_OUT_REPORT_HAPTIC_LFO_TONE` (`0x83`, `MsgHapticLfoTone`) with direct Hz frequency and dB gain once raw report forwarding is enabled. Steam Deck investigation is deferred to a later phase.
