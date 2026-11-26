/* renhdrum_recomp_bridge.h - Generic RenHDrum 0.6.0 bridge for static recompilations
   Supports Pro Controller (USB & BT), Switch 2 Pro Controller (USB, experimental),
   DualSense / Edge (USB HD Haptics + BT 0x36 Opus), Xbox One S (4 motors + Direct mode),
   and Steam Controller 2026 grips (experimental).
   MIT OR Apache-2.0 / GPL-3.0 compatibility */
#pragma once
#include <stdint.h>
#include <stdbool.h>
#include "renhdrum.h"

#ifdef __cplusplus
extern "C" {
#endif

int rh_load(void);
void rh_attach(void *pad);
int rh_detach(void *pad);
int rh_vibration(int side, float al, float fl, float ah, float fh);
void rh_permit(int on);
void rh_cleanup(void);
void rh_shutdown(void);
int rh_is_active(void *pad);
int rh_pad_claimed(int i);
void rh_set_enabled(int on);
void rh_set_switch_gen(int gen);
const char *rh_status_line(void);
int rh_ds_render_frames(float *out, int frames);

/* Fallback trigger preset IDs when compiling against an older host/renhdrum.h (v0.5.0). */
#ifndef RENHDRUM_TRIG_OFF
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
#endif

void rh_trigger_preset(int side, uint8_t id, float level);
/* The game's last rumble amplitude 0..1 (the larger of both sides' al / ah), 0 when the last sample is older than 60 ms. */
float rh_game_rumble_amp(void);

int rh_option_count(void);
const RenHdcOption *rh_option_at(int i);
int rh_option_get(const char *key, int *out);
int rh_option_set(const char *key, int value);
void rh_option_reset(void);

#ifdef __cplusplus
}
#endif
