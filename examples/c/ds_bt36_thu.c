/* DualSense Bluetooth test: combined report 0x36 (rumble + adaptive triggers) vs old 0x32.
   Plan 119 step 4. Pair the DualSense over Bluetooth (NOT USB), run, then answer:
   (a) 0x36: does it vibrate, do the triggers kick? (b) compared with 0x32: weak / medium / good?
   (c) any stutter or dropouts?
   Pattern: 1 s on / 1 s off, 160 Hz low band, both triggers held half way. 10 s each phase. */
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <SDL3/SDL.h>

#include "renhdrum.h"

#define SONY_VID 0x054C

static SDL_hid_device *g_devs[8];
static const char *g_paths[8];
static int g_dev_count = 0;
static int g_ok[8][2]; /* [dev][0] = 0x36 writes ok, [1] = 0x32 writes ok */
static int g_fail[8][2];

static void write_all(int kind, const uint8_t *buf, size_t len) {
    for (int i = 0; i < g_dev_count; ++i) {
        int n = SDL_hid_write(g_devs[i], buf, len);
        if (n < 0) {
            g_fail[i][kind]++;
        } else {
            g_ok[i][kind]++;
        }
    }
}

static void phase(const char *name, bool use36, RenHdcDualSenseDsp *dsp, RenHdcTriTrigger *tri_r,
                  RenHdcTriTrigger *tri_l) {
    const uint64_t period_ns = RENHDRUM_DS5_BT_PERIOD_NS;
    const uint64_t t0 = SDL_GetTicksNS();
    uint64_t next = t0;
    uint8_t r36[RENHDRUM_DS5_BT36_REPORT_SIZE];
    uint8_t r32[RENHDRUM_DS5_BT_REPORT_SIZE];
    uint8_t eff_r[11] = {0x05}, eff_l[11] = {0x05};
    uint8_t state[RENHDRUM_DS5_EFFECTS_SIZE];
    printf("%s ...\n", name);
    fflush(stdout);
    while (SDL_GetTicksNS() - t0 < 10ULL * 1000000000ULL) {
        const uint64_t now_ns = SDL_GetTicksNS();
        const uint32_t now_ms = (uint32_t)(now_ns / 1000000ULL);
        const bool on = (((now_ns - t0) / 1000000000ULL) % 2) == 0;
        const float a = on ? 0.8f : 0.0f;
        renhdrum_dualsense_dsp_set_target(dsp, a, 160.0f, 0.0f, 320.0f, a, 160.0f, 0.0f, 320.0f);
        if (use36) {
            renhdrum_tri_trigger_update(tri_r, now_ms, 128, a, 160.0f, 0.0f, 320.0f, on, eff_r);
            renhdrum_tri_trigger_update(tri_l, now_ms, 128, a, 160.0f, 0.0f, 320.0f, on, eff_l);
            memset(state, 0, sizeof(state));
            state[0] = 0x0C;
            memcpy(&state[10], eff_r, 11);
            memcpy(&state[21], eff_l, 11);
            const int rc = renhdrum_dualsense_dsp_build_bt36_report(dsp, state, 0, r36);
            if (rc > 0) {
                write_all(0, r36, sizeof(r36));
            }
        } else {
            const int rc = renhdrum_dualsense_dsp_build_bt_report(dsp, RENHDRUM_DS5_BT_BUFFER_LENGTH_DEFAULT, r32);
            if (rc >= 0) {
                write_all(1, r32, sizeof(r32));
            }
        }
        next += period_ns;
        while (SDL_GetTicksNS() < next) {
            SDL_DelayPrecise(500000);
        }
    }
    /* Release triggers + a few silent blocks (0x36 only). */
    if (use36) {
        uint8_t off[11] = {0x05};
        memset(state, 0, sizeof(state));
        state[0] = 0x0C;
        memcpy(&state[10], off, 11);
        memcpy(&state[21], off, 11);
        renhdrum_dualsense_dsp_set_target(dsp, 0, 160.0f, 0, 320.0f, 0, 160.0f, 0, 320.0f);
        for (int i = 0; i < 6; ++i) {
            if (renhdrum_dualsense_dsp_build_bt36_report(dsp, state, 0, r36) > 0) {
                write_all(0, r36, sizeof(r36));
            }
            SDL_Delay(11);
        }
    }
}

int main(void) {
    if (!SDL_Init(0)) {
        fprintf(stderr, "SDL_Init failed: %s\n", SDL_GetError());
        return 1;
    }
    SDL_hid_init();
    SDL_hid_device_info *list = SDL_hid_enumerate(SONY_VID, 0);
    for (SDL_hid_device_info *d = list; d && g_dev_count < 8; d = d->next) {
        if (d->product_id != 0x0CE6 && d->product_id != 0x0DF2) {
            continue;
        }
        printf("DualSense HID: usage_page=0x%04X usage=0x%04X iface=%d path=%s\n", d->usage_page, d->usage,
               d->interface_number, d->path);
        SDL_hid_device *h = SDL_hid_open_path(d->path);
        if (h) {
            g_devs[g_dev_count] = h;
            g_paths[g_dev_count] = d->path;
            g_dev_count++;
        } else {
            printf("  (could not open: %s)\n", SDL_GetError());
        }
    }
    if (g_dev_count == 0) {
        fprintf(stderr, "No DualSense HID handle opened. Pair it over Bluetooth and close other apps using it.\n");
        SDL_hid_free_enumeration(list);
        SDL_Quit();
        return 1;
    }
    printf("Opened %d HID handle(s). Writes go to every handle; see summary.\n", g_dev_count);

    RenHdcDualSenseDsp *dsp = renhdrum_dualsense_dsp_create(100, true);
    RenHdcTriTrigger *tri_r = renhdrum_tri_trigger_create();
    RenHdcTriTrigger *tri_l = renhdrum_tri_trigger_create();

    phase("Phase 1/2: report 0x36 (rumble + triggers), 10 s", true, dsp, tri_r, tri_l);
    SDL_Delay(1500);
    phase("Phase 2/2: report 0x32 (old, rumble only), 10 s", false, dsp, tri_r, tri_l);

    printf("\nSummary (writes ok / failed per handle):\n");
    for (int i = 0; i < g_dev_count; ++i) {
        printf("  handle %d: 0x36 %d/%d, 0x32 %d/%d  (%s)\n", i, g_ok[i][0], g_fail[i][0], g_ok[i][1],
               g_fail[i][1], g_paths[i]);
    }
    renhdrum_tri_trigger_destroy(tri_r);
    renhdrum_tri_trigger_destroy(tri_l);
    renhdrum_dualsense_dsp_destroy(dsp);
    for (int i = 0; i < g_dev_count; ++i) {
        SDL_hid_close(g_devs[i]);
    }
    SDL_hid_free_enumeration(list);
    SDL_Quit();
    return 0;
}
