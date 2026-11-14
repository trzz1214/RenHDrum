/* host/rh_bridge.h - RenHDrum bridge for Hookshot (HD Rumble on Pro Controller & DualSense)
   MIT OR Apache-2.0 / GPL-3.0 compatibility */
#pragma once
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

int rh_load(void);
void rh_attach(void *pad);
int rh_detach(void *pad);
int rh_vibration(int side, float al, float fl, float ah, float fh);
void rh_permit(int on);
void rh_cleanup(void);
int rh_is_active(void *pad);
int rh_pad_claimed(int i);
int rh_ds_render_frames(float *out, int frames);

#ifdef __cplusplus
}
#endif
