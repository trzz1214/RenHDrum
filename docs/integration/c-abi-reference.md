# RenHDrum C ABI Reference

Header location: [`include/renhdrum.h`](../../include/renhdrum.h)  
Library name: `renhdrum.dll` (Windows), `librenhdrum.so` (Linux), `librenhdrum.dylib` (macOS)

> [!WARNING]
> **CRITICAL REQUIREMENT: Accurate Vibration / 15 ms Throttle Bypass**
> In Eden, yuzu, and PokeDinner derivatives, host configurations **must** enable `Accurate Vibration` (`enable_accurate_vibrations = true`) or bypass the 15 ms rate-limiting block in `src/hid_core/frontend/emulated_controller.cpp`. If accurate vibration is disabled, the host rate limiter swallows and drops micro-timed HD Rumble sub-packets before they reach RenHDrum!

> [!NOTE]
> **Supported Controllers**: `✅ DualSense` · `✅ Nintendo Pro Controller` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)`.

---

## 1. Return Codes and Constants

| Constant | Value | Description |
|---|---|---|
| `RENHDRUM_VERSION` | `0x000500` | Encoded as `0xMMNNPP` (Major 0, Minor 5, Patch 0). |
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
    int32_t bluetooth_level;       /* Token bucket tier (1=Stable 60ms, 2=Balanced 45ms, 3=Quality 35ms, 4=Compatible 32ms newest-value, default) */
    bool usb_auto_slow;            /* Auto-slowdown to 30 ms under high sample rates */
    bool sync_to_input;            /* Input synchronization */
    bool click_pulse;              /* Enable hardware click packet emulation */
    bool sync_bluetooth;           /* Pacing on Bluetooth */
    bool usb_no_queue;             /* Idle-gated non-blocking writes on USB */
    int32_t usb_pause_ms;          /* Inter-packet pause duration (default 0) */
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
Returns the integer library version `0x000500`.

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
int32_t renhdrum_dualsense_dsp_fill_f32(RenHdcDualSenseDsp *dsp, float *out_buffer, int32_t total_floats);
```
Fills `out_buffer` with interleaved 4-channel 48 kHz float samples (Channels 0 & 1 = Audio left/right; Channels 2 & 3 = Haptics left/right). `total_floats` must be a multiple of 4.

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

## 6. Real-Time Memory Guarantees

The following functions are strictly guaranteed to perform **zero heap allocations**:
- `renhdrum_engine_push_sample`
- `renhdrum_engine_push_half_sample`
- `renhdrum_engine_tick`
- `renhdrum_dualsense_dsp_set_target`
- `renhdrum_dualsense_dsp_set_half_target`
- `renhdrum_dualsense_dsp_fill_f32`
- All stateless encoders (`renhdrum_encode_*`, `renhdrum_amfm_*`, `renhdrum_crisp_*`)
