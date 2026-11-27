# RenHDrum C ABI Reference

Header location: [`include/renhdrum.h`](../../include/renhdrum.h)  
Library name: `renhdrum.dll` (Windows), `librenhdrum.so` (Linux), `librenhdrum.dylib` (macOS)

> [!WARNING]
> **15 ms rate filter: bypass it only for pads RenHDrum drives**
> In Eden, yuzu, and PokeDinner derivatives, the host rate-limiting block (`src/hid_core/frontend/emulated_controller.cpp`) swallows micro-timed HD Rumble sub-packets before they reach RenHDrum. I do not ask users to turn on `Accurate Vibration`. Instead the host skips that filter conditionally: only for a pad that RenHDrum is currently driving. Every other pad keeps the host's normal filter. The user changes no setting.

> [!NOTE]
> **Supported Controllers**: `✅ DualSense` · `✅ Nintendo Pro Controller` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)` · `🧪 Switch 2 Pro Controller (USB + Bluetooth, experimental)`.

---

## 1. Return Codes and Constants

| Constant | Value | Description |
|---|---|---|
| `RENHDRUM_VERSION` | `0x000601` | Encoded as `0xMMNNPP` (Major 0, Minor 6, Patch 1). |
| `RENHDRUM_OK` | `0` | Call completed successfully. |
| `RENHDRUM_ERR_NULL_PTR` | `-1` | A required pointer argument was null. |
| `RENHDRUM_ERR_INVALID_ARG` | `-2` | An argument value was outside acceptable bounds. |
| `RENHDRUM_ERR_DECODE_FAILED` | `-3` | Packet decoding or parameter unpacking failed. |

---

## 2. Data Structures

### `RenHdcSettings`
Host runtime configuration passed to `renhdrum_engine_apply_settings`.
```c
typedef struct RenHdcSettings {
    bool enabled;                  /* Global engine enable */
    int32_t usb_interval_ms;       /* USB target frame spacing (default 15 ms) */
    int32_t bluetooth_level;       /* Token bucket tier (1=Stable 60ms [NULL config default], 2=Balanced 45ms, 3=Quality 35ms, 4=Compatible 32ms newest-value [Standard preset]) */
    bool usb_auto_slow;            /* Auto-slowdown to 30 ms under high sample rates */
    bool sync_to_input;            /* Input synchronization */
    bool click_pulse;              /* Enable hardware click packet emulation */
    bool sync_bluetooth;           /* Pacing on Bluetooth */
    bool usb_no_queue;             /* Idle-gated non-blocking writes on USB */
    int32_t usb_pause_ms;          /* Inter-packet pause duration (default 2) */
    bool envelope_5ms;             /* 5 ms multi-sample AM/FM envelope on USB */
    bool frequency_in_packet;      /* In-packet frequency changes (FormatThree7bit / Two14bit) */
    bool frequency_both_bands;     /* Dual-band frequency switching */
    bool bt_after_input;           /* Write-After-Input gating on Bluetooth */
    bool crisp_notes;              /* Crisp notes attack transient shaping (+3 dB / -3 dB) on USB */
    bool bt_crisp_notes;           /* Crisp notes attack transient shaping on Bluetooth */
    bool bt_fast;                  /* Idle-gated 5 ms multi-sample framing on Bluetooth */
} RenHdcSettings;
```

### `RenHdSideSample`
Represents one channel's vibration parameters (low and high bands).
```c
typedef struct RenHdSideSample {
    float amp_low;   /* Low-band amplitude [0.0, 1.0] */
    float freq_low;  /* Low-band frequency in Hz (typically 40.0 - 640.0) */
    float amp_high;  /* High-band amplitude [0.0, 1.0] */
    float freq_high; /* High-band frequency in Hz (typically 80.0 - 1250.0) */
} RenHdSideSample;
```

---

## 2b. Settings Resolvers

### `renhdrum_settings_resolve_pure`
```c
int32_t renhdrum_settings_resolve_pure(const RenHdcConfig *config, RenHdcSettings *out_settings);
```
Use this when the host is not Brontide. It reads no IPC shared memory and no flag files; out-of-range values fall back to defaults. A `NULL` config gives the same result as `renhdrum_settings_default`.
`renhdrum_settings_resolve` (Brontide) additionally lets `Brontide-App` and flag files override the config.

## 2c. Settings table

The core lists every HD Rumble setting (key, label, range, default, controller, tier) so a host can draw its page from the table (layout rules: `ui-guidelines.md`). Values live in a host-owned `RenHdcStore`; `renhdrum_store_to_config` turns them into `RenHdcConfig` (unchanged, 48 bytes).

```c
int32_t n, stride;
const RenHdcOption *base = renhdrum_options(&n, &stride);
for (int i = 0; i < n; i++) {
    const RenHdcOption *o = (const RenHdcOption *)((const char *)base + i * stride);
}
```

- `kind` 2 (CHOICE): `choices` is `A|B|C`, the value is the index. `usb_interval_ms` values 0..3 mean 10 / 15 / 20 / 30 ms.
- `requires_row` (Rust field `requires`; C++20 reserves that word): index of a row that must be non-zero for this row to matter (grey it out otherwise); `link` 1 = USB only, 2 = Bluetooth only.
- `renhdrum_store_set` clamps to `[min, max]`; an unknown key returns `RENHDRUM_ERR_INVALID_ARG`. Never rename a key after release.
- `switch_bt_mode` 0 (Detailed) writes level 4 + `hd_rumble_experimental_bt_fast = true`; 1 (Compatible) writes level 4 + false.
- `renhdrum_store_is_recommended`: true when every value equals its default. Keys with no `RenHdcConfig` field (`gain`, `low_boost`, `xbox_*`) are host settings: the host reads them with `renhdrum_store_get`.
- `RenHdcStore` is not thread-safe; the host keeps it on one thread.

## 3. Stateless Encoders

These functions perform pure mathematical transformations and packet encoding without maintaining state or allocating memory.

### `renhdrum_version`
```c
uint32_t renhdrum_version(void);
```
Returns the integer library version `0x000601`.

### `renhdrum_set_nx2_mode`
```c
void renhdrum_set_nx2_mode(int32_t mode);
```
Sets the Switch 2 vibration translation mode:
- `mode == 2`: Switch 2 translation active. Call when receiving guest Switch 2 vibration commands (e.g. `nn::hid::IHidServer` Command 214 `SendVibrationValuesInMode`).
  - **Sony DualSense**: Automatically applies the fixed Mode 2 law (`a <= 0.001 -> 0`, else `min(1, 3.5 * a^0.75)`, no EQ) across all Switch 2 games before DSP synthesis (keeping the 5.3 ms slot peak-hold, 4 ms smoothing, and 0.8 output ceiling).
  - **Nintendo Switch Pro Controller 1**: Automatically applies the Pro curve `PRO_DEFAULT` (`T = 0.004, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950`), silencing micro-vibrations at or below `0.004` instead of lifting them, with an initial +20% (1.2x) onset boost for 25 ms on low frequencies below 150 Hz.
  - **Microsoft Xbox One S**: Maps amplitudes through the same Pro curve (`PRO_DEFAULT`, `T = 0.004`, without the 1.2x boost) and then passes them to the Switch 1 four-motor converter.
- `mode == 0`: Classic Switch 1 mode. Amplitudes pass through 100% untouched and bit-exact. Call when running classic Switch 1 games or standard vibration commands.

### `renhdrum_encode_standard_side`
```c
int32_t renhdrum_encode_standard_side(
    float low_freq, float low_amp,
    float high_freq, float high_amp,
    uint8_t *out_bytes4
);
```
Encodes a single channel's low and high bands into a standard 4-byte dekuNukem Switch rumble word.

### `renhdrum_encode_standard_packet`
```c
int32_t renhdrum_encode_standard_packet(
    float left_lf, float left_la, float left_hf, float left_ha,
    float right_lf, float right_la, float right_hf, float right_ha,
    uint8_t *out_bytes8
);
```
Encodes both left and right channels into the 8 rumble payload bytes of an output report.

### `renhdrum_amfm_pack_three`
```c
int32_t renhdrum_amfm_pack_three(
    int32_t lo0, int32_t hi0,
    int32_t lo1, int32_t hi1,
    int32_t lo2, int32_t hi2,
    uint32_t *out_val
);
```
Packs three 5-bit low-band and high-band amplitude samples into a 32-bit `FormatThree` word.

---

## 4. Stateful Engine (`RenHdcEngine`)

Manages the 15 ms transmission window, token bucket pacer, auto-slowdown detector, and stereo channel latching.

### `renhdrum_engine_create`
```c
RenHdcEngine *renhdrum_engine_create(bool is_bluetooth, bool is_pro_controller);
```
Allocates and initializes an engine instance.

### `renhdrum_engine_destroy`
```c
void renhdrum_engine_destroy(RenHdcEngine *engine);
```
Frees engine state.

### `renhdrum_engine_apply_settings`
```c
int32_t renhdrum_engine_apply_settings(RenHdcEngine *engine, const RenHdcSettings *settings);
```
Applies transport and pacing settings to the running engine.

### `renhdrum_engine_push_sample`
```c
int32_t renhdrum_engine_push_sample(
    RenHdcEngine *engine,
    int64_t now_us,
    float left_al, float left_fl, float left_ah, float left_fh,
    float right_al, float right_fl, float right_ah, float right_fh
);
```
Enqueues a stereo vibration sample with a monotonic timestamp in microseconds.

### `renhdrum_engine_push_half_sample`
```c
int32_t renhdrum_engine_push_half_sample(
    RenHdcEngine *engine,
    int64_t now_us,
    bool is_right,
    float al, float fl, float ah, float fh
);
```
Atomic stereo latch: enqueues a single side's vibration. The engine buffers the half-sample until its counterpart arrives or the window triggers.

### `renhdrum_engine_tick`
```c
int32_t renhdrum_engine_tick(RenHdcEngine *engine, int64_t now_us, uint8_t *out_packet10);
```
Evaluates the transmission timer and pacer at timestamp `now_us`:
- Returns `1`: A 10-byte Subcommand `0x10` packet was written to `out_packet10`.
- Returns `0`: No packet ready; wait for the next tick.
- Returns `< 0`: Error occurred.

---

## 5. DualSense Audio Haptics DSP (`RenHdcDualSenseDsp`)

Synthesizes real-time 4-channel 48 kHz PCM audio for DualSense voice coil motors.

### `renhdrum_dualsense_dsp_create`
```c
RenHdcDualSenseDsp *renhdrum_dualsense_dsp_create(int32_t volume_percent, bool crisp_notes);
```
Creates a DSP synthesizer instance. `volume_percent` defaults to 100.

### `renhdrum_dualsense_dsp_destroy`
```c
void renhdrum_dualsense_dsp_destroy(RenHdcDualSenseDsp *dsp);
```
Frees DSP synthesizer state.

### `renhdrum_dualsense_dsp_set_target`
```c
int32_t renhdrum_dualsense_dsp_set_target(
    RenHdcDualSenseDsp *dsp,
    float left_al, float left_fl, float left_ah, float left_fh,
    float right_al, float right_fl, float right_ah, float right_fh
);
```
Sets the target vibration goals for both channels.

### `renhdrum_dualsense_dsp_set_half_target`
```c
int32_t renhdrum_dualsense_dsp_set_half_target(
    RenHdcDualSenseDsp *dsp,
    bool is_right,
    float al, float fl, float ah, float fh
);
```
Sets the vibration target for a single channel.

### `renhdrum_dualsense_dsp_fill_f32`
```c
int32_t renhdrum_dualsense_dsp_fill_f32(RenHdcDualSenseDsp *dsp, float *out_interleaved_4ch, size_t frame_count);
```
Fills `out_interleaved_4ch` with interleaved 4-channel 48 kHz float samples (Channels 0 & 1 = Audio left/right; Channels 2 & 3 = Haptics left/right). `out_interleaved_4ch` must hold `frame_count * 4` floats.

### `renhdrum_valve_grip_dsp_create` / `renhdrum_valve_grip_build_report` (experimental)
```c
RenHdcDualSenseDsp *renhdrum_valve_grip_dsp_create(int32_t volume_percent, bool crisp_notes);
RenHdcDualSenseDsp *renhdrum_valve_grip_dsp_create_ex(int32_t volume_percent, bool crisp_notes, bool drift_compensation);
int32_t renhdrum_valve_grip_build_report(RenHdcDualSenseDsp *dsp, uint8_t format, uint8_t *out_report64);
int32_t renhdrum_valve_grip_mode_tick(RenHdcDualSenseDsp *dsp, uint8_t format, uint64_t now_ns, bool stalled, uint8_t *out_reports8);
int32_t renhdrum_valve_grip_mode_off(uint8_t format, uint8_t *out_reports8);
```
Steam Controller (2026) grip haptics. Experimental: I built this from iafx's hardware measurements in RenHDrum#5; I do not own the controller.

`renhdrum_valve_grip_dsp_create` returns the same handle type as the DualSense DSP, with a grip profile (per-band gain of -5 dB flat from 40 to 500 Hz, falling to -20 dB at 800 Hz, applied after the same amplitude law) and leaves 40 ppm drift packet-skipping disabled so older callers that treat any `>= 0` return as sendable stay safe. Use `renhdrum_valve_grip_dsp_create_ex(..., true)` to opt into 40 ppm drift compensation. Feed the handle with `renhdrum_dualsense_dsp_set_half_target` and free it with `renhdrum_dualsense_dsp_destroy`.

`renhdrum_valve_grip_build_report` fills `out_report64` (64 bytes, `RENHDRUM_VALVE_GRIP_REPORT_SIZE`) with HID report `0x88`: byte 0 = `0x88`, byte 1 = 31 samples, bytes 2..32 = left grip, bytes 33..63 = right grip, mu-law coded. `format` 8 = mu-law 8 kHz (USB cable, call every `RENHDRUM_VALVE_GRIP_PERIOD_NS_8K`), 9 = mu-law 4 kHz (wireless dongle, Bluetooth on Windows, every `RENHDRUM_VALVE_GRIP_PERIOD_NS_4K`). Returns `1` when the block carries vibration, `0` when silent, `2` (`RENHDRUM_VALVE_GRIP_SKIP`, only when created with `drift_compensation = true`) once every 25,000 continuous packets so the host skips sending `out_report64` without advancing DSP phase (resets after a `>= 100 ms` silence pause), or a negative code on error (`RENHDRUM_ERR_INVALID_ARG` / `RENHDRUM_ERR_NULL_PTR`).

`renhdrum_valve_grip_mode_tick` manages the HID `0x86` PCM mode state in the core without heap allocation. Call it once per report period with the monotonic clock `now_ns` and `stalled` (`true` if the previous HID write failed or stalled). It writes `0` or `2` consecutive 4-byte `0x86` reports (`RENHDRUM_VALVE_GRIP_MODE_REPORTS_SIZE` = `8` bytes; bytes `0..4` channel `2`, bytes `4..8` channel `5`) and returns the report count (`0` or `2`, or `< 0` on error): `"on"` (`op = 2`) when vibration starts, `"on"` refreshed every `3 s` while active or immediately on `stalled = true`, and `"off"` (`op = 1`) after `100 ms` of silence (also resetting the drift counter).

`renhdrum_valve_grip_mode_off` writes the two 4-byte `0x86` `"off"` reports (channels `2` and `5`, accepting `format` `0`, `8`, or `9`) into `out_reports8` and returns `2` (`< 0` on error), for detach and all exit paths.

---

## 5b. Xbox One Rumble (`RenHdcXboxRumble`)

Converts Switch HD Rumble to the four motors of a Microsoft Xbox One controller. See [xbox.md](xbox.md).

### `renhdrum_xbox_rumble_create`
```c
RenHdcXboxRumble *renhdrum_xbox_rumble_create(bool is_bluetooth);
```
Creates a converter. `is_bluetooth` selects the tick period (20 ms) instead of USB (10 ms).

### `renhdrum_xbox_rumble_destroy`
```c
void renhdrum_xbox_rumble_destroy(RenHdcXboxRumble *rumble);
```
Frees the converter.

### `renhdrum_xbox_rumble_push_half_sample`
```c
int32_t renhdrum_xbox_rumble_push_half_sample(
    RenHdcXboxRumble *rumble,
    int64_t now_us,
    bool is_right,
    float low_amp, float low_freq, float high_amp, float high_freq
);
```
Pushes one hand's HD Rumble value. `now_us` is a monotonic microsecond clock.

### `renhdrum_xbox_rumble_tick`
```c
int32_t renhdrum_xbox_rumble_tick(RenHdcXboxRumble *rumble, int64_t now_us, uint16_t *out_motors4);
```
- Returns `1`: `out_motors4` holds left grip, right grip, left trigger, right trigger; send them now.
- Returns `0`: nothing to send.
- Returns `< 0`: error.

### `renhdrum_xbox_rumble_resend`
```c
int32_t renhdrum_xbox_rumble_resend(RenHdcXboxRumble *rumble);
```
Marks the last values as unsent, so the next `tick` returns `1` again. Use it when the send failed.

---

## 5c. Switch 2 Pro Controller HD Rumble 2 (`RenHdcPro2`, experimental)

> **🧪 Switch 2 Pro Controller (USB + Bluetooth, experimental).** Frequency law measured on a real Switch 2 Pro Controller by `101Leafy` (`Frizti/Fritchii#6`), packet layout from `ndeadly` (`switch2_controller_research`) and `BlueRetro` (Apache-2.0), and 15 ms BLE pacing from `NS2Pro-Bridge-Windows` (MIT). I do not own the controller, so feedback from anyone who has one is welcome.

Packs both HD Rumble bands (`al`/`fl`, `ah`/`fh`) into 64-byte USB output reports (`0x02`) carrying three 5 ms samples per side under `0x70 | tid`, with a configurable amplitude ceiling (`RENHDRUM_PRO2_AMP_CEILING_DEFAULT = 450`, `0` selects the default, clamped to `1023`). When `renhdrum_set_nx2_mode(2)` is active, amplitudes pass through the Switch 2 Mode 2 law (`a <= 0.001 -> 0`, else `min(1, 3.5 * a^0.75)`) before scaling to the ceiling.

Constants:
- `RENHDRUM_PRO2_REPORT_SIZE` (`64`)
- `RENHDRUM_PRO2_PERIOD_US` (`15000`)
- `RENHDRUM_PRO2_VID` (`0x057E`)
- `RENHDRUM_PRO2_PID` (`0x2069`)
- `RENHDRUM_PRO2_AMP_CEILING_DEFAULT` (`450`)

```c
RenHdcPro2 *renhdrum_pro2_create(uint16_t amp_ceiling);
void renhdrum_pro2_destroy(RenHdcPro2 *h);
int32_t renhdrum_pro2_push_half_sample(
    RenHdcPro2 *h, int64_t now_us, bool is_right,
    float al, float fl, float ah, float fh
);
int32_t renhdrum_pro2_build_report(RenHdcPro2 *h, int64_t now_us, uint8_t *out_report64);
int32_t renhdrum_pro2_silent_report(RenHdcPro2 *h, uint8_t *out_report64);
```

- **Host send loop (USB via SDL)**: send the 64-byte block via `SDL_SendGamepadEffect` / `SDL_SendJoystickEffect` on patched SDL (patch `0003`, `SDL_hidapi_switch2.c`). Poll `renhdrum_pro2_build_report` roughly every 5 ms using a monotonic microsecond clock `now_us`; the core returns `1` only once every 15 ms (`RENHDRUM_PRO2_PERIOD_US`) when a report is due, emits three silent packets when vibration stops (or after 2 s with no pushes), and then returns `0` until vibration resumes.
- **Bluetooth**: send bytes 1..32 of the report after a `0x00` byte (33 bytes) to the vibration characteristic (handle `0x0012`, write without response), every 15 ms, latest wins, but always deliver the silent reports. SDL has no Switch 2 Bluetooth yet (`libsdl-org/SDL#15963`), so the host must own the BLE link.
- **Virtual-joystick `SendEffect` bridge (Brontide-NS2 / Longshot pattern)**: when the host owns the transport itself (BLE GATT in Brontide-NS2, or WinUSB in Longshot), expose the controller as an SDL virtual joystick (`SDL_AttachVirtualJoystick`) with `SDL_VirtualJoystickDesc::SendEffect` implemented. Inside `SendEffect`, accept only `size == 64 && data[0] == 0x02` (return `SDL_Unsupported()` / `false` otherwise), copy the payload under a tiny dedicated lock with **no** WinRT, `WriteFile`, or SDL calls inside the callback, set a short raw-mode timeout (e.g. 100 ms), and count 3 stop writes when all samples have zero amplitude so silent reports are never dropped. A dedicated writer thread takes the buffered bytes after releasing that lock and sends either the 33-byte `0x00` packet (BLE) or the 64-byte `0x02` report (USB). On the RenHDrum side, probe once on attach with `renhdrum_pro2_silent_report` via `SDL_SendJoystickEffect` (falling back to standard rumble if rejected) and always call `SDL_SendJoystickEffect` outside any driver lock.
- **`renhdrum_pro2_silent_report`**: fills `out_report64` with a zero-amplitude `0x02` report and increments `tid` (used for probing `SDL_SendGamepadEffect` / `SDL_SendJoystickEffect` on open and sending 3 stop packets on shutdown or disable).

---

## 5d. Device table and claim status (v0.6.0)

One VID/PID table for every host, so hosts do not each keep their own list.

```c
#define RENHDRUM_ABI_COMPATIBLE(v) /* same major.minor-high bits, DLL >= header */
int32_t renhdrum_device_classify(uint16_t vid, uint16_t pid, RenHdcDeviceInfo *out);
const char *renhdrum_claim_status_text(int32_t code);
```

- **Version check**: at load time test `RENHDRUM_ABI_COMPATIBLE(renhdrum_version())`; if false, report `RENHDRUM_CLAIM_ABI_MISMATCH` and do not call further.
- **`renhdrum_device_classify`**: set `out->struct_size = sizeof(RenHdcDeviceInfo)` first. Returns `RENHDRUM_OK` for a known family, `RENHDRUM_CLAIM_UNSUPPORTED_DEVICE` (`1`) when the family is `RENHDRUM_FAMILY_NONE` (`out` is still filled), `RENHDRUM_ERR_NULL_PTR` for `out == NULL`, `RENHDRUM_ERR_INVALID_ARG` when `struct_size < 16` (nothing written).
- **`RENHDRUM_DEVICE_NO_HD_WRITE`**: never send HD Rumble packets to this pad. `057E:2073` (GameCube controller) powers off when it receives them. `2008` is SDL's virtual combined Joy-Con pair (`RENHDRUM_DEVICE_COMBINED_JOYCON`). Microsoft `045E` is matched by VID alone (any PID), as the recomp bridge does, so only call the function for a device the host already opened as a gamepad. Steam `28DE:1302/1303/1304` carry `RENHDRUM_DEVICE_EXPERIMENTAL` and `steam_grip_format` 8 / 9 / 9.
- **`renhdrum_claim_status_text`**: static NUL-terminated English text with the fix for codes `0..8` (`RENHDRUM_CLAIM_*`); any other code gives `"Unknown status."`. Never NULL, no allocation. The host fills codes 2 to 6 itself (the core opens no device).

## 5e. Test vibration (v0.6.0)

A "Test" button needs no state in the core: the script is a pure function of time.

```c
#define RENHDRUM_DEMO_QUICK_PING 0  /* 150 ms tap left, then right */
#define RENHDRUM_DEMO_TECH_SHOW 1   /* ~2.4 s: tap, low-band sweep, high-band notes */
#define RENHDRUM_DEMO_CHECKUP 2     /* ~4 s: tap, sweep, 4 notes, 4 strength steps, stop */
int32_t renhdrum_demo_sample(int32_t preset, uint32_t t_ms, RenHdSideSample *out_left, RenHdSideSample *out_right);
```

- Call it every tick with `t_ms` = milliseconds since the host pressed "Test", and send both sides to the pad's usual sink (engine `push_sample`, DualSense / Steam `set_target`, Xbox, Pro 2).
- Returns `1` while running, `0` once finished (outputs are zero; stop calling), `RENHDRUM_ERR_NULL_PTR` for a NULL output, `RENHDRUM_ERR_INVALID_ARG` for an unknown preset.
- `QUICK_PING` (ends at 450 ms): 0 to 150 ms left side 0.6 at 160 / 320 Hz, 300 to 450 ms right side.
- `TECH_SHOW` (ends at 2400 ms): 0 to 150 ms tap 0.7; 300 to 1100 ms low band 0.5 sweeping 40 to 160 Hz on a log scale; 1200 to 2400 ms four 300 ms high-band notes at 0.5 on 160, 240, 320 and 480 Hz. Both sides are identical.
- `CHECKUP` (ends at 4000 ms, both sides identical):

| Window (ms) | Part | Signal |
|---|---|---|
| `0..149` | 1. Tap | Both bands at amplitude `0.7` (`160` / `320` Hz) |
| `300..1099` | 2. Sweep | Low band `0.5` sweeping `40 -> 160` Hz on a log scale |
| `1200..2399` | 3. Four notes | High band `0.5` at `160`, `240`, `320`, `480` Hz (300 ms each) |
| `2500..3699` | 4. Strength steps | Low band `160` Hz at `0.25`, `0.5`, `0.75`, `1.0` (300 ms each) |
| `3700..3999` | 5. Stop | Silence (`0.0`) so the player feels the controller stop cleanly |

- No state, no heap allocation, safe to call from the rumble thread.

### Send-rate meter (`renhdrum_test_meter_*`)

```c
typedef struct RenHdTestMeter RenHdTestMeter;
RenHdTestMeter *renhdrum_test_meter_create(void);
void renhdrum_test_meter_destroy(RenHdTestMeter *meter);
void renhdrum_test_meter_record(RenHdTestMeter *meter, uint64_t t_us, bool ok);
int32_t renhdrum_test_meter_format(const RenHdTestMeter *meter, char *out, size_t cap);
```

- Create one meter per connected pad when starting a checkup test, and destroy it once formatting is complete and no send thread can still call `record`.
- **Call `renhdrum_test_meter_record` at the actual packet write site** (where the host sends a HID, SDL, or audio packet to the controller), not where the game pushes a rumble sample. Internally mutex-protected with poison recovery, so it is safe to call from any send thread (`NULL` is a no-op).
- `renhdrum_test_meter_format` writes a NUL-terminated ASCII summary `sent <ok>/<total>, <avg> ms avg, stuck <yes|no>` (`n/a` when fewer than 2 writes were recorded; `stuck yes` if any gap between consecutive `record` calls exceeds 100 ms or 3+ consecutive writes failed). Returns the byte count written excluding NUL, `RENHDRUM_ERR_NULL_PTR`, or `RENHDRUM_ERR_INVALID_ARG` if `cap` is too small.

---

## 6. Real-Time Memory Guarantees

The following functions are strictly guaranteed to perform **zero heap allocations**:
- `renhdrum_engine_push_sample`
- `renhdrum_engine_push_half_sample`
- `renhdrum_engine_tick`
- `renhdrum_dualsense_dsp_set_target`
- `renhdrum_dualsense_dsp_set_half_target`
- `renhdrum_dualsense_dsp_fill_f32`
- `renhdrum_pro2_push_half_sample`
- `renhdrum_pro2_build_report`
- `renhdrum_pro2_silent_report`
- `renhdrum_demo_sample`
- All stateless encoders (`renhdrum_encode_*`, `renhdrum_amfm_*`, `renhdrum_crisp_*`)

---

## Upgrading from 0.5 to 0.6

1. **Replace `include/renhdrum.h` and `renhdrum.dll` together.** Check compatibility at load time with `RENHDRUM_ABI_COMPATIBLE(renhdrum_version())`.
2. **All 0.5 functions keep identical signatures and behavior.** Existing hosts continue to work with zero breaking changes.
3. **All new 0.6 functions are optional.** Load them by name if you use them:
   - Adaptive trigger presets: `renhdrum_trigger_preset_create` / `destroy` / `set` / `tick`
   - Switch 2 Pro HD Rumble 2 (experimental): `renhdrum_pro2_create` / `destroy` / `push_half_sample` / `build_report` / `silent_report`
   - Shared device table & claim status: `renhdrum_device_classify`, `renhdrum_claim_status_text`
   - Test vibration & send-rate meter: `renhdrum_demo_sample` (`RENHDRUM_DEMO_CHECKUP`) and `renhdrum_test_meter_create` / `destroy` / `record` / `format`
4. **Standard checkup report line format** (for copy-paste bug/controller reports):
   `<pad name> | <VID:PID> | <USB|BT> | RenHDrum <x.y.z> | <host> <ver> | <meter text> | <answer>`
