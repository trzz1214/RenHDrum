#ifndef RENHDRUM_H
#define RENHDRUM_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RENHDRUM_VERSION 0x000600

#define RENHDRUM_OK 0
#define RENHDRUM_ERR_NULL_PTR -1
#define RENHDRUM_ERR_INVALID_ARG -2
#define RENHDRUM_ERR_DECODE_FAILED -3

typedef struct RenHdcSettings {
    bool enabled;
    int32_t usb_interval_ms;
    int32_t bluetooth_level; /* 1-4, 4 = Compatible (default) */
    bool usb_auto_slow;
    bool sync_to_input;
    bool click_pulse;
    bool sync_bluetooth;
    bool usb_no_queue;
    int32_t usb_pause_ms;
    bool envelope_5ms;
    bool frequency_in_packet;
    bool frequency_both_bands;
    bool bt_after_input;
    bool crisp_notes;
    bool bt_crisp_notes;
    bool bt_fast;
} RenHdcSettings;

typedef struct RenHdcConfig {
    bool enable_hd_rumble;
    int32_t hd_rumble_usb_interval_ms;
    int32_t hd_rumble_bluetooth_level;
    bool hd_rumble_usb_auto_slow;
    bool hd_rumble_experimental_usb_no_queue;
    bool hd_rumble_experimental_envelope_5ms;
    bool hd_rumble_experimental_frequency_in_packet;
    bool hd_rumble_experimental_frequency_both_bands;
    bool hd_rumble_experimental_bt_after_input;
    bool hd_rumble_experimental_bt_fast;
    bool hd_rumble_experimental_crisp_notes;
    bool hd_rumble_experimental_bt_crisp_notes;
    bool hd_rumble_experimental_sync_to_input;
    bool hd_rumble_experimental_click_pulse;
    bool hd_rumble_experimental_sync_bluetooth;
    bool hd_rumble_sync_auto_slow;
    float strong_rumble;
    float weak_rumble;
    bool enable_rumble;
    bool enable_adaptive_triggers;
    bool enable_dualsense_haptics;
    int32_t dualsense_haptics_volume;
    bool dualsense_crisp_notes;
    bool dualsense_bt_crisp_notes;
    bool dualsense_hd_haptics_off; /* false = HD Haptics on (default): DualSense USB speaker + trigger kick */
} RenHdcConfig;

typedef struct RenHdSideSample {
    float amp_low;
    float freq_low;
    float amp_high;
    float freq_high;
} RenHdSideSample;

typedef struct RenHdClickSide {
    int32_t kind; /* 0 = DualResonance3Pulse, 1 = SingleWaveResonance */
    int32_t res_high_code;
    int32_t res_low_code;
    int32_t pulse1_code;
    int32_t pulse2_code;
    bool res_high_off;
    bool res_low_off;
    bool pulse1_off;
    bool pulse2_off;
    int32_t amp7;
    int32_t freq7;
    bool freq_is_high;
} RenHdClickSide;

uint32_t renhdrum_version(void);

int32_t renhdrum_settings_default(RenHdcSettings *out_settings);
int32_t renhdrum_settings_resolve(const RenHdcConfig *config, RenHdcSettings *out_settings);
/* Use this when the host is not Brontide: reads no IPC and no flag files, only `config`. */
/* NULL config gives the same result as renhdrum_settings_default. */
int32_t renhdrum_settings_resolve_pure(const RenHdcConfig *config, RenHdcSettings *out_settings);

int32_t renhdrum_preset_apply(int32_t kind, RenHdcConfig *config);
int32_t renhdrum_preset_detect(const RenHdcConfig *config, int32_t *out_kind);

/* Settings table + value store (see docs/integration/c-abi-reference.md "Settings table"). */
typedef struct RenHdcOption {
    const char *key;      /* never changes after release */
    const char *label;    /* EN */
    const char *help;     /* EN, may be empty */
    const char *choices;  /* CHOICE kind: "Detailed (5 ms)|Compatible"; else empty */
    int32_t kind;         /* 0 BOOL, 1 PERCENT/INT, 2 CHOICE (value = index) */
    int32_t min, max, step, def;
    uint32_t devices;     /* bit: 1 Switch, 2 DualSense, 4 Xbox, 8 Steam */
    int32_t tier;         /* 0 Basic, 1 Advanced */
    int32_t link;         /* 0 any, 1 USB only, 2 Bluetooth only */
    int32_t requires_row; /* (not "requires": C++20 keyword) row index that must be non-zero for this row to matter, -1 = none */
} RenHdcOption;
const RenHdcOption *renhdrum_options(int32_t *out_count, int32_t *out_stride);

typedef struct RenHdcStore RenHdcStore; /* values; host-owned, not thread-safe */
RenHdcStore *renhdrum_store_create(void); /* recommended defaults */
void renhdrum_store_destroy(RenHdcStore *s);
int32_t renhdrum_store_get(const RenHdcStore *s, const char *key, int32_t *out);
int32_t renhdrum_store_set(RenHdcStore *s, const char *key, int32_t value); /* clamped to [min,max]; unknown key = RENHDRUM_ERR_INVALID_ARG */
int32_t renhdrum_store_reset(RenHdcStore *s);
int32_t renhdrum_store_is_recommended(const RenHdcStore *s, bool *out);
int32_t renhdrum_store_to_config(const RenHdcStore *s, RenHdcConfig *out);
/* Intensity (`gain`) + weak-rumble lift (`low_boost`) of the store applied to one amplitude 0..1. */
float renhdrum_store_shape_amplitude(const RenHdcStore *s, float amplitude);

/* Xbox "Direct" mode (store key xbox_mode = 1). Returns a mask: bit 0 = send grips out_motors4[0..2]
   (SDL_RumbleJoystick, 2000 ms), bit 1 = send triggers out_motors4[2..4] (SDL_RumbleJoystickTriggers,
   2000 ms), 0 = nothing to send, <0 = error. Pass raw game amplitudes; gain / low_boost / xbox_* come
   from the store. `triggers`: the pad has trigger motors the host drives. */
typedef struct RenHdcXboxDirect RenHdcXboxDirect;
RenHdcXboxDirect *renhdrum_xbox_direct_create(void);
void renhdrum_xbox_direct_destroy(RenHdcXboxDirect *d);
int32_t renhdrum_xbox_direct_push(
    RenHdcXboxDirect *d,
    const RenHdcStore *s,
    int64_t now_us,
    bool is_right,
    float low_amp,
    float high_amp,
    bool triggers,
    uint16_t *out_motors4
);
/* Host timer: when a burst ends, returns the settled motor values (same mask). */
int32_t renhdrum_xbox_direct_tick(RenHdcXboxDirect *d, int64_t now_us, bool triggers, uint16_t *out_motors4);

int32_t renhdrum_encode_standard_side(
    float low_freq,
    float low_amp,
    float high_freq,
    float high_amp,
    uint8_t *out_bytes4
);

int32_t renhdrum_encode_standard_packet(
    float left_lf,
    float left_la,
    float left_hf,
    float left_ha,
    float right_lf,
    float right_la,
    float right_hf,
    float right_ha,
    uint8_t *out_bytes8
);

int32_t renhdrum_amfm_game_amplitude(int32_t code, float *out_amp);
int32_t renhdrum_amfm_code(float game_amp);
int32_t renhdrum_amfm_freq7(int32_t high, float freq);
int32_t renhdrum_amfm_amp7(float game_amplitude);
int32_t renhdrum_amfm_preset_code(int32_t high, int32_t freq7);

int32_t renhdrum_amfm_pack_three(
    int32_t lo0,
    int32_t hi0,
    int32_t lo1,
    int32_t hi1,
    int32_t lo2,
    int32_t hi2,
    uint32_t *out_val
);

int32_t renhdrum_amfm_pack_three_7bit(
    int32_t high,
    int32_t is_fm,
    int32_t value7,
    int32_t lo1,
    int32_t hi1,
    int32_t lo2,
    int32_t hi2,
    uint32_t *out_val
);

int32_t renhdrum_amfm_pack_two_14bit(
    int32_t high,
    int32_t freq7,
    int32_t amp7,
    int32_t other0,
    int32_t lo1,
    int32_t hi1,
    uint32_t *out_val
);

int32_t renhdrum_amfm_pack_two_14bit_level(
    int32_t high,
    int32_t freq7,
    float game_amplitude,
    int32_t other0,
    int32_t lo1,
    int32_t hi1,
    uint32_t *out_val
);

int32_t renhdrum_amfm_pack_two_14bit_reach(
    int32_t high,
    int32_t freq7,
    float game_amplitude,
    int32_t reach_code,
    float other_amplitude,
    int32_t lo1,
    int32_t hi1,
    uint32_t *out_val
);

int32_t renhdrum_amfm_is_three_sample_side(const uint8_t *in_bytes4);

int32_t renhdrum_click_amp4(float amplitude);
int32_t renhdrum_click_amp7(float amplitude);
int32_t renhdrum_click_build_packet(float amp_l, float amp_r, uint8_t *out_packet10);

int32_t renhdrum_click_encode_side(const RenHdClickSide *side, uint8_t *out_bytes4);
int32_t renhdrum_click_decode_side(const uint8_t *in_bytes4, RenHdClickSide *out_side);
int32_t renhdrum_click_is_dual_wave_side(const uint8_t *in_bytes4);
int32_t renhdrum_click_is_click_packet(const uint8_t *in_bytes10);

float renhdrum_crisp_slot_gain(int32_t slots_since_onset);
int32_t renhdrum_crisp_apply(
    const RenHdSideSample *s,
    float gain,
    RenHdSideSample *out_sample,
    int32_t *out_capped
);
int32_t renhdrum_crisp_shape_slot(
    int32_t *slots_since_onset,
    const RenHdSideSample *s,
    RenHdSideSample *out_sample,
    int32_t *out_onset,
    int32_t *out_capped
);

/* Opaque pointer types for stateful engines */
typedef struct RenHdcEngine RenHdcEngine;
typedef struct RenHdcDualSenseDsp RenHdcDualSenseDsp;
typedef struct RenHdcXboxRumble RenHdcXboxRumble;

/* Thread safety: never call a handle's destroy in parallel with any other call on the same handle. */
RenHdcEngine *renhdrum_engine_create(bool is_bluetooth, bool is_pro_controller);
void renhdrum_engine_destroy(RenHdcEngine *engine);
int32_t renhdrum_engine_apply_settings(RenHdcEngine *engine, const RenHdcSettings *settings);
int32_t renhdrum_engine_push_sample(
    RenHdcEngine *engine,
    int64_t now_us,
    float left_al,
    float left_fl,
    float left_ah,
    float left_fh,
    float right_al,
    float right_fl,
    float right_ah,
    float right_fh
);
int32_t renhdrum_engine_push_half_sample(
    RenHdcEngine *engine,
    int64_t now_us,
    bool is_right,
    float al,
    float fl,
    float ah,
    float fh
);
int32_t renhdrum_engine_tick(RenHdcEngine *engine, int64_t now_us, uint8_t *out_packet10);

// volume_percent (clamped 10..100) is the final output gain; the core already applies it.
// The host must NOT scale samples again before pushing them. The core exports no volume
// setter yet: to change it, destroy and re-create the DSP object (no need to reopen the
// audio stream). If the Brontide-App config is present in shared memory, it overrides both arguments.
RenHdcDualSenseDsp *renhdrum_dualsense_dsp_create(int32_t volume_percent, bool crisp_notes);
void renhdrum_dualsense_dsp_destroy(RenHdcDualSenseDsp *dsp);
int32_t renhdrum_dualsense_dsp_set_target(
    RenHdcDualSenseDsp *dsp,
    float left_al,
    float left_fl,
    float left_ah,
    float left_fh,
    float right_al,
    float right_fl,
    float right_ah,
    float right_fh
);
int32_t renhdrum_dualsense_dsp_set_half_target(
    RenHdcDualSenseDsp *dsp,
    bool is_right,
    float al,
    float fl,
    float ah,
    float fh
);
/* Speaker channel (channel 1, USB) level 0..100 percent; 0 = off (default). */
#define RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT 60
int32_t renhdrum_dualsense_dsp_set_speaker(RenHdcDualSenseDsp *dsp, int32_t percent);
/* out_interleaved_4ch must hold frame_count * 4 floats. */
int32_t renhdrum_dualsense_dsp_fill_f32(
    RenHdcDualSenseDsp *dsp,
    float *out_interleaved_4ch,
    size_t frame_count
);
/* DualSense over Bluetooth: one HID output report 0x32 every RENHDRUM_DS5_BT_PERIOD_NS.
   Report layout from SAxense by egormanga (https://apps.sdore.me/SAxense). */
#define RENHDRUM_DS5_BT_REPORT_SIZE 142
#define RENHDRUM_DS5_BT_PERIOD_NS 10666666ULL
#define RENHDRUM_DS5_BT_BUFFER_LENGTH_DEFAULT 0xFF
/* out_report142 must be a buffer of at least 142 bytes (RENHDRUM_DS5_BT_REPORT_SIZE). */
int32_t renhdrum_dualsense_dsp_build_bt_report(
    RenHdcDualSenseDsp *dsp,
    uint8_t buffer_length,
    uint8_t *out_report142
);
/* DualSense over Bluetooth, combined report 0x36 (rumble + adaptive triggers + speaker), one per
   RENHDRUM_DS5_BT_PERIOD_NS. The speaker is one Opus frame (200 bytes) per report, on while HD Haptics is on
   (the core turns it on by itself if the host never called renhdrum_dualsense_dsp_set_speaker; hosts should
   call it on Bluetooth too). Layout after Enhanced-PS5-DualSense-Haptics-Bridge (MIT, ragnarok179).
   state47_or_null: 47-byte block (byte 0 bits 0x04 R2 / 0x08 L2, R2 effect [10..21], L2 [21..32]);
   NULL = triggers untouched. seq is reserved/unused. out_report398 needs 398 bytes.
   Returns 1 = send out_report398, 0 = nothing to send (rumble + triggers idle over 2 s), < 0 on error. */
#define RENHDRUM_DS5_BT36_REPORT_SIZE 398
int32_t renhdrum_dualsense_dsp_build_bt36_report(
    RenHdcDualSenseDsp *dsp,
    const uint8_t *state47_or_null,
    uint8_t seq,
    uint8_t *out_report398
);
/* Experimental: Steam Controller (2026) grips, report 0x88, built from iafx's measurements (RenHDrum#5).
   Create with renhdrum_valve_grip_dsp_create[_ex], then use renhdrum_dualsense_dsp_set_half_target and
   renhdrum_dualsense_dsp_destroy on the handle. One report every RENHDRUM_VALVE_GRIP_PERIOD_NS_*.
   build_report returns 1 when vibrating, 0 when silent, 2 (RENHDRUM_VALVE_GRIP_SKIP) when 40 ppm drift
   compensation skips this packet (only if created with drift_compensation = true; do not send, phase
   unchanged), < 0 on error (unknown format -> RENHDRUM_ERR_INVALID_ARG). out_report64: 64 bytes.
   mode_tick evaluates the 0x86 PCM mode keep-alive (on at first vibration, refresh every 3 s or on
   stalled=true, off after 100 ms silence) and writes 0 or 2 reports (4 bytes each) into out_reports8.
   mode_off writes 2 off reports (channels 2 and 5) into out_reports8 for detach/exit paths. */
#define RENHDRUM_VALVE_GRIP_REPORT_SIZE 64
#define RENHDRUM_VALVE_GRIP_FORMAT_MULAW_8K 8
#define RENHDRUM_VALVE_GRIP_FORMAT_MULAW_4K 9
#define RENHDRUM_VALVE_GRIP_PERIOD_NS_8K 3875000ULL
#define RENHDRUM_VALVE_GRIP_PERIOD_NS_4K 7750000ULL
#define RENHDRUM_VALVE_GRIP_SKIP 2
#define RENHDRUM_VALVE_GRIP_MODE_REPORT_SIZE 4
#define RENHDRUM_VALVE_GRIP_MODE_REPORT_COUNT 2
#define RENHDRUM_VALVE_GRIP_MODE_REPORTS_SIZE 8
RenHdcDualSenseDsp *renhdrum_valve_grip_dsp_create(int32_t volume_percent, bool crisp_notes);
RenHdcDualSenseDsp *renhdrum_valve_grip_dsp_create_ex(
    int32_t volume_percent,
    bool crisp_notes,
    bool drift_compensation
);
int32_t renhdrum_valve_grip_build_report(
    RenHdcDualSenseDsp *dsp,
    uint8_t format,
    uint8_t *out_report64
);
int32_t renhdrum_valve_grip_mode_tick(
    RenHdcDualSenseDsp *dsp,
    uint8_t format,
    uint64_t now_ns,
    bool stalled,
    uint8_t *out_reports8
);
int32_t renhdrum_valve_grip_mode_off(
    uint8_t format,
    uint8_t *out_reports8
);
/* Vibration-reactive trigger (one handle per trigger). update: 1 = changed (11 bytes written to
   out11, pack with the DS5 effects state), 0 = unchanged, <0 = error. */
typedef struct RenHdcTriTrigger RenHdcTriTrigger;
RenHdcTriTrigger *renhdrum_tri_trigger_create(void);
void renhdrum_tri_trigger_destroy(RenHdcTriTrigger *h);
int32_t renhdrum_tri_trigger_update(
    RenHdcTriTrigger *h,
    uint32_t now_ms,
    uint8_t depth,
    float amp_low,
    float freq_low,
    float amp_high,
    float freq_high,
    bool fresh,
    uint8_t *out11
);
/* Player is pressing another button (call every tick while held, before update, same clock):
   a streak started strictly before this trigger was held counts as a posture and does not mute;
   a streak started with or after trigger hold (gap > 30 ms starts a new streak) mutes new pulses
   while held and for 500 ms after the last call. */
void renhdrum_tri_trigger_note_action(RenHdcTriTrigger *h, uint32_t now_ms);
/* Shared adaptive trigger presets (Tier 2, one handle per trigger).
   Priority rule (the core does not blend Tier 1 and Tier 2): while a preset is active and non-Off
   (out11[0] != 0x05), the host sends the preset's 11 bytes; when the preset is Off, the host falls
   back to renhdrum_tri_trigger_update as before.
   tick returns 1 = send out11, 0 = nothing to send, < 0 = error (null pointer). */
#define RENHDRUM_TRIG_OFF          0  /* Forced release: smooth free trigger */
#define RENHDRUM_TRIG_TENSION      1  /* Progressive pull: bow, slingshot, charge (caps after 10 s) */
#define RENHDRUM_TRIG_TWO_STAGE    2  /* Two-stage: half-pull aim/mode, break through to fire */
#define RENHDRUM_TRIG_CRISP_SHOT   3  /* Crisp break: semi-auto firearm trigger */
#define RENHDRUM_TRIG_AUTO_FIRE    4  /* Automatic fire: pulsing kick (caps at 5 s) */
#define RENHDRUM_TRIG_EMPTY_SOFT   5  /* Empty soft: dry trigger with light bottom tap */
#define RENHDRUM_TRIG_JAMMED       6  /* Jammed wall: blocked early (caps after 3 s, soft release) */
#define RENHDRUM_TRIG_BRAKE        7  /* Progressive brake: heavier near full pull (caps after 8 s) */
#define RENHDRUM_TRIG_ABS_BRAKE    8  /* ABS brake: pulsed release (falls back to Brake after 4 s) */
#define RENHDRUM_TRIG_WHEELSPIN    9  /* Wheelspin / drift vibration (stops after 5 s) */
#define RENHDRUM_TRIG_THROTTLE     10 /* Throttle pedal: light continuous curve (capped at strength 4) */
#define RENHDRUM_TRIG_HEAVY_LOAD   11 /* Heavy load: carrying weight / raised shield (caps after 12 s) */
#define RENHDRUM_TRIG_IMPACT_PULSE 12 /* One-shot 100 ms impact pulse overlay (>= 80 ms gap) */
#define RENHDRUM_TRIG_ENGINE_ROUGH 13 /* Engine / rough surface vibration (caps after 10 s) */
typedef struct RenHdcTriggerPreset RenHdcTriggerPreset;
RenHdcTriggerPreset *renhdrum_trigger_preset_create(void);
void renhdrum_trigger_preset_destroy(RenHdcTriggerPreset *h);
void renhdrum_trigger_preset_set(RenHdcTriggerPreset *h, uint8_t preset_id, float level, uint32_t now_ms);
int32_t renhdrum_trigger_preset_tick(RenHdcTriggerPreset *h, uint32_t now_ms, uint8_t *out11);
/* 47-byte DS5EffectsState_t USB blocks for HD Haptics (speaker open/release, single-trigger update/off).
   Returns 0 on success, -1 (RENHDRUM_ERR_NULL_PTR) if out47 is NULL. */
#define RENHDRUM_DS5_EFFECTS_SIZE 47
int32_t renhdrum_hd_haptics_speaker_block(int32_t on, uint8_t *out47);
int32_t renhdrum_hd_haptics_trigger_block(
    int32_t is_right,
    const uint8_t *effect11_or_null,
    uint8_t *out47
);
int32_t renhdrum_dualsense_evaluate_triggers(
    uint64_t title_id,
    bool zl_held,
    bool zr_held,
    bool rumble_on,
    int64_t rumble_on_ms,
    int64_t pulse_age_ms,
    uint8_t *out_ds5_effects47
);

RenHdcXboxRumble *renhdrum_xbox_rumble_create(bool is_bluetooth);
void renhdrum_xbox_rumble_destroy(RenHdcXboxRumble *rumble);
int32_t renhdrum_xbox_rumble_push_half_sample(
    RenHdcXboxRumble *rumble,
    int64_t now_us,
    bool is_right,
    float low_amp,
    float low_freq,
    float high_amp,
    float high_freq
);
/* out_motors4: left grip, right grip, left trigger, right trigger. Returns 1 when they must be sent now. */
int32_t renhdrum_xbox_rumble_tick(RenHdcXboxRumble *rumble, int64_t now_us, uint16_t *out_motors4);
int32_t renhdrum_xbox_rumble_resend(RenHdcXboxRumble *rumble);

/* Experimental: Switch 2 Pro Controller (USB only, 057E:2069) HD Rumble 2 report 0x02 builder.
   build_report returns 1 when out_report64 must be sent, 0 when nothing to send, < 0 on error. */
typedef struct RenHdcPro2 RenHdcPro2;
#define RENHDRUM_PRO2_REPORT_SIZE 64
#define RENHDRUM_PRO2_PERIOD_US 15000
#define RENHDRUM_PRO2_VID 0x057E
#define RENHDRUM_PRO2_PID 0x2069
#define RENHDRUM_PRO2_AMP_CEILING_DEFAULT 450
RenHdcPro2 *renhdrum_pro2_create(uint16_t amp_ceiling);
void renhdrum_pro2_destroy(RenHdcPro2 *h);
int32_t renhdrum_pro2_push_half_sample(
    RenHdcPro2 *h,
    int64_t now_us,
    bool is_right,
    float al,
    float fl,
    float ah,
    float fh
);
int32_t renhdrum_pro2_build_report(RenHdcPro2 *h, int64_t now_us, uint8_t *out_report64);
int32_t renhdrum_pro2_silent_report(RenHdcPro2 *h, uint8_t *out_report64);

/* Switch 2 rumble mode: 0 = Switch 1 default / untouched, 2 = Switch 2 Mode 2 translation */
void renhdrum_set_nx2_mode(int32_t mode);

/* Host compiled against this header can use DLL version v (renhdrum_version()). */
#define RENHDRUM_ABI_COMPATIBLE(v) ((((uint32_t)(v)) >> 16) == ((uint32_t)RENHDRUM_VERSION >> 16) && ((uint32_t)(v)) >= (uint32_t)RENHDRUM_VERSION)

#define RENHDRUM_FAMILY_NONE 0
#define RENHDRUM_FAMILY_SWITCH1 1
#define RENHDRUM_FAMILY_SWITCH2 2
#define RENHDRUM_FAMILY_DUALSENSE 3
#define RENHDRUM_FAMILY_DUALSENSE_EDGE 4
#define RENHDRUM_FAMILY_XBOX 5
#define RENHDRUM_FAMILY_STEAM 6
#define RENHDRUM_DEVICE_EXPERIMENTAL 1u
#define RENHDRUM_DEVICE_NO_HD_WRITE 2u      /* never send HD Rumble packets: the pad powers off (057E:2073) */
#define RENHDRUM_DEVICE_COMBINED_JOYCON 4u  /* SDL's virtual Joy-Con pair */
typedef struct RenHdcDeviceInfo {
    uint32_t struct_size;       /* host sets sizeof(RenHdcDeviceInfo) */
    int32_t family;
    uint32_t flags;
    int32_t steam_grip_format;  /* RENHDRUM_VALVE_GRIP_FORMAT_* or 0 */
} RenHdcDeviceInfo;
/* RENHDRUM_OK = known family, RENHDRUM_CLAIM_UNSUPPORTED_DEVICE = family NONE (out still filled). */
int32_t renhdrum_device_classify(uint16_t vid, uint16_t pid, RenHdcDeviceInfo *out);

#define RENHDRUM_CLAIM_OK 0
#define RENHDRUM_CLAIM_UNSUPPORTED_DEVICE 1
#define RENHDRUM_CLAIM_BLOCKED_BY_HOST_DRIVER 2
#define RENHDRUM_CLAIM_RAW_PACKET_REFUSED 3
#define RENHDRUM_CLAIM_AUDIO_ENDPOINT_MISSING 4
#define RENHDRUM_CLAIM_AUDIO_FORMAT_MISMATCH 5
#define RENHDRUM_CLAIM_WINUSB_MI01_REQUIRED 6
#define RENHDRUM_CLAIM_ABI_MISMATCH 7
#define RENHDRUM_CLAIM_NO_HD_WRITE 8
/* Static English text with the fix, never NULL ("Unknown status." for other codes). */
const char *renhdrum_claim_status_text(int32_t code);

#define RENHDRUM_DEMO_QUICK_PING 0  /* 150 ms tap left, then right: which pad is this slot */
#define RENHDRUM_DEMO_TECH_SHOW 1   /* ~2.4 s: tap, low-band sweep, high-band notes */
#define RENHDRUM_DEMO_CHECKUP 2     /* ~4 s: tap, sweep, 4 notes, 4 strength steps, stop */
/* Sample at t_ms since start. Returns 1 while running (feed both sides into the pad's usual sink:
   engine push_sample, DualSense / Steam set_target, Xbox, Pro 2), 0 once finished (outputs zero),
   <0 on error. */
int32_t renhdrum_demo_sample(int32_t preset, uint32_t t_ms, RenHdSideSample *out_left, RenHdSideSample *out_right);

/* Thread-safe send-rate meter used during checkup tests to record real packet writes. */
typedef struct RenHdTestMeter RenHdTestMeter;
/* Allocates a new meter; free with renhdrum_test_meter_destroy. */
RenHdTestMeter *renhdrum_test_meter_create(void);
/* Frees a meter (NULL is a no-op). */
void renhdrum_test_meter_destroy(RenHdTestMeter *meter);
/* Records one real packet write at monotonic timestamp t_us (NULL is a no-op). */
void renhdrum_test_meter_record(RenHdTestMeter *meter, uint64_t t_us, bool ok);
/* Formats "sent <ok>/<total>, <avg> ms avg, stuck <yes|no>" into out; returns bytes written (<0 on error). */
int32_t renhdrum_test_meter_format(const RenHdTestMeter *meter, char *out, size_t cap);

#ifdef __cplusplus
}
#endif

#endif /* RENHDRUM_H */
