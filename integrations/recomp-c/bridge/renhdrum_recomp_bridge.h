/* renhdrum_recomp_bridge.h - Generic RenHDrum 0.5.0 bridge for static recompilations
   Supports Pro Controller (USB & BT), DualSense / Edge (USB HD Haptics + BT 0x36 Opus),
   Xbox One S (4 motors + Direct mode), and Steam Controller 2026 grips (experimental).
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

int rh_option_count(void);
const RenHdcOption *rh_option_at(int i);
int rh_option_get(const char *key, int *out);
int rh_option_set(const char *key, int value);
void rh_option_reset(void);

#ifdef __cplusplus
}
#endif
