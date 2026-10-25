# RenHDrum C ABI Reference

Header location: [`include/renhdrum.h`](../../include/renhdrum.h)  
Library name: `renhdrum.dll` (Windows), `librenhdrum.so` (Linux), `librenhdrum.dylib` (macOS)

---

## 1. Return Codes and Constants

| Constant | Value | Description |
|---|---|---|
| `RENHDRUM_VERSION` | `0x000100` | Encoded as `0xMMNNPP` (Major 0, Minor 1, Patch 0). |
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
    int32_t bluetooth_level;       /* Token bucket tier (1=Stable 60ms, 2=Balanced 45ms, 3=Quality 35ms) */
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

## 3. Stateless Encoders

These functions perform pure mathematical transformations and packet encoding without maintaining state or allocating memory.

### `renhdrum_version`
```c
uint32_t renhdrum_version(void);
```
Returns the integer library version `0x000100`.

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
