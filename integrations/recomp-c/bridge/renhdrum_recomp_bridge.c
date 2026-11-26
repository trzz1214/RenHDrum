/* renhdrum_recomp_bridge.c - Generic RenHDrum 0.6.0 bridge for static recompilations
   Authentic HD Rumble for Nintendo Switch Pro Controller (USB & Bluetooth),
   Switch 2 Pro Controller (USB, experimental), DualSense / Edge (USB 4-ch 48 kHz +
   speaker + reactive triggers, Bluetooth 0x36 Opus + triggers / 0x32), Xbox One S
   (4 motors + Direct mode), and Steam Controller 2026 grips (experimental).
   Loaded dynamically at run time (no import library needed).
   MIT OR Apache-2.0 / GPL-3.0 compatibility */
#ifndef _CRT_SECURE_NO_WARNINGS
#define _CRT_SECURE_NO_WARNINGS
#endif

#if defined(__has_include)
#  if __has_include("renhdrum_recomp_bridge.h")
#    include "renhdrum_recomp_bridge.h"
#  else
#    include "rh_bridge.h"
#  endif
#else
#  include "renhdrum_recomp_bridge.h"
#endif

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#define RH_STALE_US 250000LL
#define RH_RESEND_US 40000LL
#define RH_MAX_PADS 8
#define DS_CHUNK_FRAMES 256
#define RH_DS_AUDIO_MAX_ATTEMPTS 10
#define RH_STEAM_FAILS_MAX 100
#define RH_STEAM_TAIL_NS 100000000LL
#define RH_PRO2_FAILS_MAX 100

#ifndef RENHDRUM_PRO2_REPORT_SIZE
typedef struct RenHdcPro2 RenHdcPro2;
#define RENHDRUM_PRO2_REPORT_SIZE 64
#endif

#ifndef RENHDRUM_ABI_COMPATIBLE
#define RENHDRUM_ABI_COMPATIBLE(v) ((((uint32_t)(v)) >> 16) == ((uint32_t)RENHDRUM_VERSION >> 16) && ((uint32_t)(v)) >= (uint32_t)RENHDRUM_VERSION)
#endif

#ifndef SDLCALL
#if defined(_WIN32) && !defined(_WIN64)
#define SDLCALL __cdecl
#else
#define SDLCALL
#endif
#endif

void hos_log(const char *fmt, ...);
int64_t hos_now_ns(void);
int hos_sdl_init(uint32_t subsystems);

static void rh_load_proc(void *dst, HMODULE mod, const char *name) {
    FARPROC p = GetProcAddress(mod, name);
    memcpy(dst, &p, sizeof(p));
}

static int rh_env_disabled(void) {
    const char *e1 = getenv("HOOKSHOT_RENHDRUM");
    const char *e2 = getenv("HOS_RENHDRUM");
    if (e1 && strcmp(e1, "0") == 0) return 1;
    if (e2 && strcmp(e2, "0") == 0) return 1;
    return 0;
}

static int rh_env_bt_level(void) {
    const char *e = getenv("HOOKSHOT_BT_LEVEL");
    if (!e || !*e) e = getenv("HOS_BT_LEVEL");
    if (!e || !*e) return 0;
    int lv = atoi(e);
    return (lv >= 1 && lv <= 4) ? lv : 0;
}

/* ---- RenHDrum DLL function table ---- */
static SRWLOCK rh_init_lock = SRWLOCK_INIT;
static volatile LONG rh_switch_gen = 2; /* 1 = Switch 1, 2 = Switch 2 (default) */
static WCHAR rh_exe_dir[MAX_PATH];
static int rh_has_exe_dir = 0;
static char rh_load_err[256];

static struct {
    int tried;
    HMODULE dll;
    uint32_t (*version)(void);
    RenHdcEngine *(*create)(bool, bool);
    void (*destroy)(RenHdcEngine *);
    int32_t (*defaults)(RenHdcSettings *);
    int32_t (*apply)(RenHdcEngine *, const RenHdcSettings *);
    int32_t (*push)(RenHdcEngine *, int64_t, bool, float, float, float, float);
    int32_t (*tick)(RenHdcEngine *, int64_t, uint8_t *);
    void (*set_nx2_mode)(int32_t);

    /* Settings table & value store (optional for minimal mock DLLs) */
    int32_t (*resolve_pure)(const RenHdcConfig *, RenHdcSettings *);
    int32_t (*resolve)(const RenHdcConfig *, RenHdcSettings *);
    const RenHdcOption *(*options)(int32_t *, int32_t *);
    RenHdcStore *(*store_create)(void);
    void (*store_destroy)(RenHdcStore *);
    int32_t (*store_get)(const RenHdcStore *, const char *, int32_t *);
    int32_t (*store_set)(RenHdcStore *, const char *, int32_t);
    int32_t (*store_reset)(RenHdcStore *);
    int32_t (*store_is_rec)(const RenHdcStore *, bool *);
    int32_t (*store_to_config)(const RenHdcStore *, RenHdcConfig *);
    float (*store_shape)(const RenHdcStore *, float);

    /* DualSense DSP & HD Haptics */
    RenHdcDualSenseDsp *(*ds_create)(int32_t, bool);
    void (*ds_destroy)(RenHdcDualSenseDsp *);
    int32_t (*ds_set_half_target)(RenHdcDualSenseDsp *, bool, float, float, float, float);
    int32_t (*ds_set_speaker)(RenHdcDualSenseDsp *, int32_t);
    int32_t (*ds_fill_f32)(RenHdcDualSenseDsp *, float *, size_t);
    int32_t (*ds_build_bt_report)(RenHdcDualSenseDsp *, uint8_t, uint8_t *);
    int32_t (*ds_build_bt36_report)(RenHdcDualSenseDsp *, const uint8_t *, uint8_t, uint8_t *);
    RenHdcTriTrigger *(*tri_create)(void);
    void (*tri_destroy)(RenHdcTriTrigger *);
    int32_t (*tri_update)(RenHdcTriTrigger *, uint32_t, uint8_t, float, float, float, float, bool, uint8_t *);
    void (*tri_note_action)(RenHdcTriTrigger *, uint32_t);
    RenHdcTriggerPreset *(*preset_create)(void);
    void (*preset_destroy)(RenHdcTriggerPreset *);
    void (*preset_set)(RenHdcTriggerPreset *, uint8_t, float, uint32_t);
    int32_t (*preset_tick)(RenHdcTriggerPreset *, uint32_t, uint8_t *);
    int32_t (*hd_speaker_block)(int32_t, uint8_t *);
    int32_t (*hd_trigger_block)(int32_t, const uint8_t *, uint8_t *);

    /* Xbox One S rumble & Direct mode */
    RenHdcXboxRumble *(*xb_create)(bool);
    void (*xb_destroy)(RenHdcXboxRumble *);
    int32_t (*xb_push)(RenHdcXboxRumble *, int64_t, bool, float, float, float, float);
    int32_t (*xb_tick)(RenHdcXboxRumble *, int64_t, uint16_t *);
    int32_t (*xb_resend)(RenHdcXboxRumble *);
    RenHdcXboxDirect *(*xb_direct_create)(void);
    void (*xb_direct_destroy)(RenHdcXboxDirect *);
    int32_t (*xb_direct_push)(RenHdcXboxDirect *, const RenHdcStore *, int64_t, bool, float, float, bool, uint16_t *);
    int32_t (*xb_direct_tick)(RenHdcXboxDirect *, int64_t, bool, uint16_t *);

    /* Steam Controller 2026 grips */
    RenHdcDualSenseDsp *(*sv_create)(int32_t, bool);
    RenHdcDualSenseDsp *(*sv_create_ex)(int32_t, bool, bool);
    int32_t (*sv_build)(RenHdcDualSenseDsp *, uint8_t, uint8_t *);
    int32_t (*sv_mode_tick)(RenHdcDualSenseDsp *, uint8_t, uint64_t, bool, uint8_t *);
    int32_t (*sv_mode_off)(uint8_t, uint8_t *);

    /* Switch 2 Pro Controller (experimental) */
    RenHdcPro2 *(*pro2_create)(uint16_t);
    void (*pro2_destroy)(RenHdcPro2 *);
    int32_t (*pro2_push)(RenHdcPro2 *, int64_t, bool, float, float, float, float);
    int32_t (*pro2_build)(RenHdcPro2 *, int64_t, uint8_t *);
    int32_t (*pro2_silent)(RenHdcPro2 *, uint8_t *);
} rh;

/* ---- SDL3 / SDL2 dynamic dispatch ---- */
static struct {
    int tried;
    int is_sdl3;
    uint16_t (*get_vid)(void *);
    uint16_t (*get_pid)(void *);
    int (*get_conn)(void *);
    void *(*get_joy)(void *);
    uint32_t (*get_joy_id)(void *);
    bool (*is_joy_virtual)(uint32_t);
    int (*send_effect)(void *, const void *, int);
    void (*close_pad)(void *);
    int (*rumble_pad)(void *, uint16_t, uint16_t, uint32_t);
    void (*free_mem)(void *);
    int (*get_type)(void *);
    uint64_t (*get_steam_handle)(void *);
    int (*rumble_joy)(void *, uint16_t, uint16_t, uint32_t);
    int (*rumble_joy_trig)(void *, uint16_t, uint16_t, uint32_t);
    int16_t (*get_axis)(void *, int);
    int (*get_button3)(void *, int);
    uint8_t (*get_button2)(void *, int);
    uint32_t (*get_joy_props)(void *);
    int (*get_bool_prop)(uint32_t, const char *, int);

    /* Patched SDL3 HD Rumble transport modes (optional; NULL on stock SDL3) */
    int (SDLCALL *hd_set_no_queue)(void *, int, int, int);
    int (SDLCALL *hd_set_no_queue_bt)(void *, int, int);
    int (SDLCALL *hd_set_bt_after_input)(void *, int);

    /* HID & device paths */
    const char *(*get_gamepad_path)(void *);
    const char *(*get_controller_path)(void *);
    const char *(*get_joy_path)(void *);
    void *(*hid_open_path)(const char *);
    int (*hid_write)(void *, const void *, size_t);
    int (*hid_close)(void *);
    void *(*hid_enumerate)(unsigned short, unsigned short);
    void (*hid_free_enumeration)(void *);

    /* SDL3 audio */
    void (*pump_events)(void);
    uint32_t *(*get_playback_devs)(int *);
    const char *(*get_dev_name)(uint32_t);
    int (*get_dev_format)(uint32_t, void *, int *);
    void *(*open_audio_stream)(uint32_t, const void *, void (SDLCALL *)(void *, void *, int, int), void *);
    int (*resume_audio_stream)(void *);
    int (*pause_audio_stream)(void *);
    int (*clear_audio_stream)(void *);
    void (*destroy_audio_stream)(void *);
    int (*put_audio_data)(void *, const void *, int);

    /* SDL2 audio & power */
    int (*get_power)(void *);
    int (*get_num_audio_devs)(int);
    const char *(*get_audio_dev_name)(int, int);
    uint32_t (*open_audio_device)(const char *, int, const void *, void *, int);
    void (*pause_audio_device)(uint32_t, int);
    void (*close_audio_device)(uint32_t);
} sdl;

static void init_sdl(void) {
    if (sdl.tried) return;
    HMODULE m = GetModuleHandleA("SDL3.dll");
    if (!m) m = GetModuleHandleA("SDL2.dll");
    if (!m) return;
    sdl.tried = 1;
    if (GetProcAddress(m, "SDL_GetGamepadVendor") != NULL) {
        sdl.is_sdl3 = 1;
        rh_load_proc(&sdl.get_vid, m, "SDL_GetGamepadVendor");
        rh_load_proc(&sdl.get_pid, m, "SDL_GetGamepadProduct");
        rh_load_proc(&sdl.get_conn, m, "SDL_GetGamepadConnectionState");
        rh_load_proc(&sdl.get_joy, m, "SDL_GetGamepadJoystick");
        rh_load_proc(&sdl.get_joy_id, m, "SDL_GetJoystickID");
        rh_load_proc(&sdl.is_joy_virtual, m, "SDL_IsJoystickVirtual");
        rh_load_proc(&sdl.send_effect, m, "SDL_SendJoystickEffect");
        rh_load_proc(&sdl.close_pad, m, "SDL_CloseGamepad");
        rh_load_proc(&sdl.rumble_pad, m, "SDL_RumbleGamepad");
        rh_load_proc(&sdl.free_mem, m, "SDL_free");
        rh_load_proc(&sdl.get_type, m, "SDL_GetGamepadType");
        rh_load_proc(&sdl.get_steam_handle, m, "SDL_GetGamepadSteamHandle");
        rh_load_proc(&sdl.rumble_joy, m, "SDL_RumbleJoystick");
        rh_load_proc(&sdl.rumble_joy_trig, m, "SDL_RumbleJoystickTriggers");
        rh_load_proc(&sdl.get_axis, m, "SDL_GetGamepadAxis");
        rh_load_proc(&sdl.get_button3, m, "SDL_GetGamepadButton");
        rh_load_proc(&sdl.get_joy_props, m, "SDL_GetJoystickProperties");
        rh_load_proc(&sdl.get_bool_prop, m, "SDL_GetBooleanProperty");
        rh_load_proc(&sdl.hd_set_no_queue, m, "SDL_HDRumbleSetNoQueue");
        rh_load_proc(&sdl.hd_set_no_queue_bt, m, "SDL_HDRumbleSetNoQueueBluetooth");
        rh_load_proc(&sdl.hd_set_bt_after_input, m, "SDL_HDRumbleSetBluetoothAfterInput");

        rh_load_proc(&sdl.get_gamepad_path, m, "SDL_GetGamepadPath");
        rh_load_proc(&sdl.get_joy_path, m, "SDL_GetJoystickPath");
        rh_load_proc(&sdl.hid_open_path, m, "SDL_hid_open_path");
        rh_load_proc(&sdl.hid_write, m, "SDL_hid_write");
        rh_load_proc(&sdl.hid_close, m, "SDL_hid_close");
        rh_load_proc(&sdl.hid_enumerate, m, "SDL_hid_enumerate");
        rh_load_proc(&sdl.hid_free_enumeration, m, "SDL_hid_free_enumeration");

        rh_load_proc(&sdl.pump_events, m, "SDL_PumpEvents");
        rh_load_proc(&sdl.get_playback_devs, m, "SDL_GetAudioPlaybackDevices");
        rh_load_proc(&sdl.get_dev_name, m, "SDL_GetAudioDeviceName");
        rh_load_proc(&sdl.get_dev_format, m, "SDL_GetAudioDeviceFormat");
        rh_load_proc(&sdl.open_audio_stream, m, "SDL_OpenAudioDeviceStream");
        rh_load_proc(&sdl.resume_audio_stream, m, "SDL_ResumeAudioStreamDevice");
        rh_load_proc(&sdl.pause_audio_stream, m, "SDL_PauseAudioStreamDevice");
        rh_load_proc(&sdl.clear_audio_stream, m, "SDL_ClearAudioStream");
        rh_load_proc(&sdl.destroy_audio_stream, m, "SDL_DestroyAudioStream");
        rh_load_proc(&sdl.put_audio_data, m, "SDL_PutAudioStreamData");
    } else {
        sdl.is_sdl3 = 0;
        rh_load_proc(&sdl.get_vid, m, "SDL_GameControllerGetVendor");
        rh_load_proc(&sdl.get_pid, m, "SDL_GameControllerGetProduct");
        rh_load_proc(&sdl.get_joy, m, "SDL_GameControllerGetJoystick");
        rh_load_proc(&sdl.send_effect, m, "SDL_JoystickSendEffect");
        if (!sdl.send_effect) rh_load_proc(&sdl.send_effect, m, "SDL_GameControllerSendEffect");
        rh_load_proc(&sdl.close_pad, m, "SDL_GameControllerClose");
        rh_load_proc(&sdl.rumble_pad, m, "SDL_GameControllerRumble");
        rh_load_proc(&sdl.get_power, m, "SDL_JoystickCurrentPowerLevel");
        rh_load_proc(&sdl.get_type, m, "SDL_GameControllerGetType");
        rh_load_proc(&sdl.get_steam_handle, m, "SDL_GameControllerGetSteamHandle");
        rh_load_proc(&sdl.rumble_joy, m, "SDL_JoystickRumble");
        rh_load_proc(&sdl.rumble_joy_trig, m, "SDL_JoystickRumbleTriggers");
        rh_load_proc(&sdl.get_axis, m, "SDL_GameControllerGetAxis");
        rh_load_proc(&sdl.get_button2, m, "SDL_GameControllerGetButton");

        rh_load_proc(&sdl.get_controller_path, m, "SDL_GameControllerPath");
        rh_load_proc(&sdl.get_joy_path, m, "SDL_JoystickPath");
        rh_load_proc(&sdl.hid_open_path, m, "SDL_hid_open_path");
        rh_load_proc(&sdl.hid_write, m, "SDL_hid_write");
        rh_load_proc(&sdl.hid_close, m, "SDL_hid_close");
        rh_load_proc(&sdl.hid_enumerate, m, "SDL_hid_enumerate");
        rh_load_proc(&sdl.hid_free_enumeration, m, "SDL_hid_free_enumeration");

        rh_load_proc(&sdl.get_num_audio_devs, m, "SDL_GetNumAudioDevices");
        rh_load_proc(&sdl.get_audio_dev_name, m, "SDL_GetAudioDeviceName");
        rh_load_proc(&sdl.open_audio_device, m, "SDL_OpenAudioDevice");
        rh_load_proc(&sdl.pause_audio_device, m, "SDL_PauseAudioDevice");
        rh_load_proc(&sdl.close_audio_device, m, "SDL_CloseAudioDevice");
    }
}

static int sdl_ok(int r) {
    return sdl.is_sdl3 ? (r & 0xFF) != 0 : r == 0;
}

static int rh_sdl_get_axis(void *pad, int axis) {
    if (!pad || !sdl.get_axis) return 0;
    return (int)sdl.get_axis(pad, axis);
}

static int rh_sdl_get_button(void *pad, int button) {
    if (!pad) return 0;
    if (sdl.is_sdl3 && sdl.get_button3) return (sdl.get_button3(pad, button) & 0xFF) != 0;
    if (!sdl.is_sdl3 && sdl.get_button2) return sdl.get_button2(pad, button) != 0;
    return 0;
}

/* ---- Settings store + renhdrum.ini ---- */
static SRWLOCK rh_store_lock = SRWLOCK_INIT;
static const RenHdcOption *rh_opts = NULL;
static int32_t rh_opt_count = 0;
static int32_t rh_opt_stride = 0;
static RenHdcStore *rh_store = NULL;

typedef struct RhUserOpts {
    int enable_hd_rumble;
    int dualsense_haptics;
    int dualsense_hd_haptics;
    int dualsense_volume;
    int dualsense_crisp_notes;
    int xbox_enabled;
    int xbox_strength;
    int xbox_direct;
} RhUserOpts;

static int rh_store_get_locked(const char *key, int fallback) {
    int32_t v = fallback;
    if (rh_store && rh.store_get && rh.store_get(rh_store, key, &v) == RENHDRUM_OK) {
        return (int)v;
    }
    return fallback;
}

static RhUserOpts rh_read_opts(void) {
    RhUserOpts u;
    AcquireSRWLockShared(&rh_store_lock);
    u.enable_hd_rumble = rh_store_get_locked("enable_hd_rumble", 1) != 0;
    u.dualsense_haptics = rh_store_get_locked("dualsense_haptics", 1) != 0;
    u.dualsense_hd_haptics = rh_store_get_locked("dualsense_hd_haptics", 1) != 0;
    u.dualsense_volume = rh_store_get_locked("dualsense_volume", 100);
    if (u.dualsense_volume < 10) u.dualsense_volume = 10;
    if (u.dualsense_volume > 100) u.dualsense_volume = 100;
    u.dualsense_crisp_notes = rh_store_get_locked("dualsense_crisp_notes", 1) != 0;
    u.xbox_enabled = rh_store_get_locked("xbox_enabled", 1) != 0;
    u.xbox_strength = rh_store_get_locked("xbox_strength", 100);
    if (u.xbox_strength < 0) u.xbox_strength = 0;
    if (u.xbox_strength > 1000) u.xbox_strength = 1000;
    u.xbox_direct = rh_store_get_locked("xbox_mode", 0) == 1;
    ReleaseSRWLockShared(&rh_store_lock);
    return u;
}

static float rh_shape_amp(float amplitude) {
    float shaped = amplitude;
    AcquireSRWLockShared(&rh_store_lock);
    if (rh_store && rh.store_shape) {
        shaped = rh.store_shape(rh_store, amplitude);
    }
    ReleaseSRWLockShared(&rh_store_lock);
    return shaped;
}

static char *rh_trim(char *s) {
    while (*s && isspace((unsigned char)*s)) s++;
    size_t n = strlen(s);
    while (n > 0 && isspace((unsigned char)s[n - 1])) s[--n] = '\0';
    return s;
}

static int rh_build_exe_sibling_path(const WCHAR *filename, WCHAR *out_path, size_t out_cap) {
    if (!rh_has_exe_dir) return 0;
    size_t dlen = wcslen(rh_exe_dir);
    size_t flen = wcslen(filename) + 1;
    if (dlen + flen > out_cap) return 0;
    memcpy(out_path, rh_exe_dir, dlen * sizeof(WCHAR));
    memcpy(out_path + dlen, filename, flen * sizeof(WCHAR));
    return 1;
}

static void rh_ini_load_locked(void) {
    if (!rh_store || !rh.store_set) return;
    WCHAR ini_path[MAX_PATH];
    if (!rh_build_exe_sibling_path(L"renhdrum.ini", ini_path, MAX_PATH)) return;
    FILE *fp = _wfopen(ini_path, L"r");
    if (!fp) return;
    char line[512];
    while (fgets(line, sizeof(line), fp)) {
        char *p = rh_trim(line);
        if (*p == '\0' || *p == '#' || *p == ';' || *p == '[') continue;
        char *eq = strchr(p, '=');
        if (!eq) continue;
        *eq = '\0';
        char *key = rh_trim(p);
        char *val_str = rh_trim(eq + 1);
        if (!*key || !*val_str) continue;
        char *end = NULL;
        long val = strtol(val_str, &end, 10);
        if (end != val_str && *end == '\0') {
            rh.store_set(rh_store, key, (int32_t)val);
        }
    }
    fclose(fp);
}

static void rh_ini_save(void) {
    WCHAR ini_path[MAX_PATH];
    if (!rh_build_exe_sibling_path(L"renhdrum.ini", ini_path, MAX_PATH)) return;

    int32_t count = 0;
    int *vals = NULL;
    const char **keys = NULL;

    AcquireSRWLockShared(&rh_store_lock);
    if (rh_store && rh.store_get && rh_opts && rh_opt_count > 0 &&
        rh_opt_stride >= (int32_t)sizeof(RenHdcOption)) {
        count = rh_opt_count;
        vals = (int *)malloc((size_t)count * sizeof(int));
        keys = (const char **)malloc((size_t)count * sizeof(const char *));
        if (vals && keys) {
            for (int32_t i = 0; i < count; i++) {
                const RenHdcOption *opt = (const RenHdcOption *)((const uint8_t *)rh_opts +
                                          (size_t)i * (size_t)rh_opt_stride);
                keys[i] = opt->key;
                int32_t v = opt->def;
                if (opt->key) rh.store_get(rh_store, opt->key, &v);
                vals[i] = (int)v;
            }
        } else {
            free(vals);
            free(keys);
            vals = NULL;
            keys = NULL;
            count = 0;
        }
    }
    ReleaseSRWLockShared(&rh_store_lock);

    if (count <= 0) return;
    FILE *fp = _wfopen(ini_path, L"w");
    if (fp) {
        fputs("# RenHDrum settings\n[RenHDrum]\n", fp);
        for (int32_t i = 0; i < count; i++) {
            if (keys[i]) fprintf(fp, "%s=%d\n", keys[i], vals[i]);
        }
        fclose(fp);
    }
    free(vals);
    free(keys);
}

static int rh_resolve_pro_settings(RenHdcSettings *out, int bt) {
    if (!rh.defaults || rh.defaults(out) != RENHDRUM_OK) return 0;

    int used_store = 0;
    if (rh.store_to_config) {
        RenHdcConfig cfg;
        memset(&cfg, 0, sizeof(cfg));
        AcquireSRWLockShared(&rh_store_lock);
        if (rh_store && rh.store_to_config(rh_store, &cfg) == RENHDRUM_OK) {
            used_store = 1;
        }
        ReleaseSRWLockShared(&rh_store_lock);
        if (used_store) {
            if (rh.resolve_pure) {
                rh.resolve_pure(&cfg, out);
            } else if (rh.resolve) {
                rh.resolve(&cfg, out);
            }
        }
    }

    if (bt) {
        int env_lv = rh_env_bt_level();
        if (env_lv) {
            out->bluetooth_level = env_lv;
        } else if (!used_store) {
            out->bluetooth_level = 4;
        }
    }
    return 1;
}

#define RH_NO_QUEUE_ENVELOPE_GAP_MS 10

static void rh_clear_pro_transport(void *joy) {
    if (!sdl.is_sdl3 || !joy) return;
    if (sdl.hd_set_no_queue) sdl.hd_set_no_queue(joy, 0, 0, 0);
    if (sdl.hd_set_no_queue_bt) sdl.hd_set_no_queue_bt(joy, 0, 0);
    if (sdl.hd_set_bt_after_input) sdl.hd_set_bt_after_input(joy, 0);
}

static void rh_apply_pro_transport(void *joy, RenHdcSettings *s, int bt) {
    if (!s) return;
    if (!sdl.is_sdl3 || !joy || !sdl.hd_set_no_queue || !sdl.hd_set_no_queue_bt || !sdl.hd_set_bt_after_input) {
        /* Stock SDL3 queues every write: keep transport flags off so the core paces writes */
        s->usb_no_queue = false;
        s->bt_after_input = false;
        s->bt_fast = false;
        return;
    }
    rh_clear_pro_transport(joy);
    if (bt) {
        s->usb_no_queue = false;
        s->bt_fast = s->enabled && s->bt_fast && s->bt_after_input &&
                     sdl.hd_set_no_queue_bt(joy, 1, RH_NO_QUEUE_ENVELOPE_GAP_MS) == 1;
        s->bt_after_input = s->enabled && s->bt_after_input && !s->bt_fast &&
                            sdl.hd_set_bt_after_input(joy, 1) == 1;
    } else {
        int gap_ms = s->envelope_5ms ? RH_NO_QUEUE_ENVELOPE_GAP_MS : (int)s->usb_interval_ms;
        s->usb_no_queue = s->enabled && s->usb_no_queue &&
                          sdl.hd_set_no_queue(joy, 1, gap_ms, (int)s->usb_pause_ms) == 1;
        s->bt_after_input = false;
        s->bt_fast = false;
    }
}

/* ---- Connected slot tracking (matched to host MAX_PADS slots) ---- */
static SRWLOCK rh_pads_lock = SRWLOCK_INIT;
static void *rh_conn_pads[RH_MAX_PADS];
static int rh_slot_claimed[RH_MAX_PADS];
static int rh_slot_stand_in[RH_MAX_PADS];
static volatile LONG rh_enabled = 1;
static volatile LONG rh_enabled_host_set = 0;

static void rh_pads_clear_claimed(void) {
    AcquireSRWLockExclusive(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) rh_slot_claimed[i] = 0;
    ReleaseSRWLockExclusive(&rh_pads_lock);
}

static void rh_pads_track(void *pad) {
    if (!pad) return;
    AcquireSRWLockExclusive(&rh_pads_lock);
    int free_slot = -1;
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_conn_pads[i] == pad) {
            ReleaseSRWLockExclusive(&rh_pads_lock);
            return;
        }
        if (!rh_conn_pads[i] && free_slot < 0) free_slot = i;
    }
    if (free_slot >= 0) {
        rh_conn_pads[free_slot] = pad;
        rh_slot_claimed[free_slot] = 0;
        rh_slot_stand_in[free_slot] = 0;
    }
    ReleaseSRWLockExclusive(&rh_pads_lock);
}

static void rh_pads_set_claimed(void *pad, int claimed) {
    if (!pad) return;
    AcquireSRWLockExclusive(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_conn_pads[i] == pad) {
            rh_slot_claimed[i] = claimed ? 1 : 0;
            break;
        }
    }
    ReleaseSRWLockExclusive(&rh_pads_lock);
}

static void rh_pads_untrack(void *pad) {
    AcquireSRWLockExclusive(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (!pad || rh_conn_pads[i] == pad) {
            rh_conn_pads[i] = NULL;
            rh_slot_claimed[i] = 0;
            rh_slot_stand_in[i] = 0;
        }
    }
    ReleaseSRWLockExclusive(&rh_pads_lock);
}

static void rh_pads_mark_stand_in(void *pad) {
    AcquireSRWLockExclusive(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_conn_pads[i] && rh_conn_pads[i] == pad) rh_slot_stand_in[i] = 1;
    }
    ReleaseSRWLockExclusive(&rh_pads_lock);
}

static int rh_stand_in_tracked(void) {
    int any = 0;
    AcquireSRWLockShared(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_conn_pads[i] && rh_slot_stand_in[i]) any = 1;
    }
    ReleaseSRWLockShared(&rh_pads_lock);
    return any;
}

int rh_pad_claimed(int i) {
    if (i < 0 || i >= RH_MAX_PADS) return 0;
    AcquireSRWLockShared(&rh_pads_lock);
    int claimed = (rh_conn_pads[i] != NULL && rh_slot_claimed[i] != 0) ? 1 : 0;
    ReleaseSRWLockShared(&rh_pads_lock);
    return claimed;
}

static int rh_all_pads_claimed(void) {
    AcquireSRWLockShared(&rh_pads_lock);
    int total = 0, claimed = 0;
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_conn_pads[i]) {
            total++;
            if (rh_slot_claimed[i]) claimed++;
        }
    }
    ReleaseSRWLockShared(&rh_pads_lock);
    return total > 0 && claimed == total;
}

/* ---- Pro Controller state ---- */
static SRWLOCK rh_lock = SRWLOCK_INIT;
static RenHdcEngine *rh_engine;
static void *rh_active_pad;
static void *rh_joy;
static int rh_is_bt;
static int rh_usb_no_queue;
static uint8_t rh_last_pkt[10];
static int rh_has_last_pkt;
static int64_t rh_last_send_us;
static int64_t rh_at[2];
static int rh_muted[2];

static int rh_pkt_is_silent(const uint8_t p[10]) {
    static const uint8_t neutral[4] = {0x00, 0x01, 0x40, 0x40};
    return memcmp(p + 2, neutral, 4) == 0 && memcmp(p + 6, neutral, 4) == 0;
}

/* ---- Switch 2 Pro Controller state (experimental) ---- */
static SRWLOCK rh_pro2_lock = SRWLOCK_INIT;
static RenHdcPro2 *rh_pro2;
static void *rh_pro2_pad;
static void *rh_pro2_joy;
static int64_t rh_pro2_at[2];
static int rh_pro2_muted[2];
static int rh_pro2_fails;

static void rh_pro2_send_silent(RenHdcPro2 *h, void *target, int count) {
    if (!h || !target || !rh.pro2_silent || !sdl.send_effect) return;
    uint8_t pkt[RENHDRUM_PRO2_REPORT_SIZE];
    for (int i = 0; i < count; i++) {
        if (rh.pro2_silent(h, pkt) == RENHDRUM_OK) {
            sdl.send_effect(target, pkt, RENHDRUM_PRO2_REPORT_SIZE);
        }
    }
}

static int rh_pro2_detach(void *pad) {
    AcquireSRWLockExclusive(&rh_pro2_lock);
    if (!rh_pro2 || (pad && rh_pro2_pad != pad)) {
        ReleaseSRWLockExclusive(&rh_pro2_lock);
        return 0;
    }
    RenHdcPro2 *h = rh_pro2;
    void *target = rh_pro2_joy ? rh_pro2_joy : rh_pro2_pad;
    rh_pro2 = NULL;
    rh_pro2_pad = NULL;
    rh_pro2_joy = NULL;
    rh_pro2_at[0] = rh_pro2_at[1] = 0;
    rh_pro2_muted[0] = rh_pro2_muted[1] = 0;
    rh_pro2_fails = 0;
    ReleaseSRWLockExclusive(&rh_pro2_lock);

    rh_pro2_send_silent(h, target, 3);
    if (rh.pro2_destroy) rh.pro2_destroy(h);
    return 1;
}

/* ---- DualSense state (per-pad slots; lock order: restart_lock -> tri_lock -> lock -> dsp_lock -> audio_claim_lock) ---- */
/* An IMPACT_PULSE beats layer 1 this long: the core pulse length (IMPACT_DURATION_MS 150 in RenHDrum). */
#define RH_PULSE_BEATS_TRI_MS 150
typedef struct RhDsSlot {
    SRWLOCK restart_lock;
    SRWLOCK tri_lock;
    SRWLOCK lock;
    SRWLOCK dsp_lock;
    int active;
    void *pad;
    void *joy;
    uint16_t vid;
    uint16_t pid;
    int is_bt;
    char hid_path[512];
    RenHdcDualSenseDsp *dsp;
    int dsp_vol;
    int dsp_crisp;
    void *audio_stream;
    uint32_t audio_device;
    uint32_t dev2;
    int audio_attempts;
    int64_t next_audio_retry_us;
    int64_t next_audio_check_us;
    void *bt_hid;
    HANDLE bt_thread;
    volatile LONG bt_run;
    volatile LONG bt_warned;
    int bt_unsupported;
    int bt_starting;
    int64_t bt_next_retry_us;
    int speaker_on;
    int speaker_logged;
    int trigger_logged;
    int bt_report36;
    RenHdcTriTrigger *tri[2];
    float tri_sample[2][4];
    int tri_fresh[2];
    RenHdcTriggerPreset *preset[2];
    LONG preset_gen[2];
    int preset_active[2];
    uint32_t pulse_until_ms[2];   /* 6c round 1: until then an IMPACT_PULSE beats layer 1 (0 = none pending) */
    uint8_t preset_last[2][11];   /* the last bytes a running preset sent (6a: back after a layer-1 effect) */
    uint8_t tri_last[2][11];
    int tri_last_valid[2];
    uint8_t bt_effect[2][11];
    int bt_effect_set[2];
    int64_t at[2];
    int muted[2];
} RhDsSlot;

static RhDsSlot rh_ds_slots[RH_MAX_PADS];
static SRWLOCK rh_ds_audio_claim_lock = SRWLOCK_INIT;
static uint32_t rh_claimed_audio_devs[RH_MAX_PADS];
static volatile LONG rh_presets_ready = 0;
static volatile LONG rh_preset_packed[2];
static volatile LONG rh_preset_gen[2];

/* ---- Xbox One S state ---- */
static SRWLOCK rh_xb_lock = SRWLOCK_INIT;
static RenHdcXboxRumble *rh_xb;
static RenHdcXboxDirect *rh_xb_direct;
static void *rh_xb_pad;
static void *rh_xb_joy;
static int rh_xb_triggers;
static int rh_xb_strength = 100;
static int rh_xb_direct_mode;
static int64_t rh_xb_at[2];
static int rh_xb_muted[2];

/* ---- Steam Controller 2026 grips state ---- */
static SRWLOCK rh_sv_lock = SRWLOCK_INIT;
static RenHdcDualSenseDsp *rh_sv_dsp;
static void *rh_sv_pad;
static void *rh_sv_hid;
static HANDLE rh_sv_thread;
static volatile LONG rh_sv_run;
static uint8_t rh_sv_format;
static int rh_sv_vol = 100;
static int rh_sv_crisp = 1;
static int64_t rh_sv_at[2];
static int rh_sv_muted[2];

/* ---- Worker thread ---- */
static HANDLE rh_thread;
static volatile LONG rh_run;

/* ---- Helper: HID path resolution ---- */
static const char *rh_pad_hid_path(void *pad) {
    void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
    const char *path = NULL;
    if (sdl.is_sdl3 && sdl.get_gamepad_path) path = sdl.get_gamepad_path(pad);
    if (!sdl.is_sdl3 && sdl.get_controller_path) path = sdl.get_controller_path(pad);
    if ((!path || !*path) && sdl.get_joy_path && joy) path = sdl.get_joy_path(joy);
    return path;
}

static void *rh_open_hid_by_path_or_enum(const char *path, uint16_t vid, uint16_t pid) {
    void *hid_dev = NULL;
    if (path && *path && sdl.hid_open_path) {
        hid_dev = sdl.hid_open_path(path);
    }
    if (!hid_dev && sdl.hid_enumerate && sdl.hid_open_path) {
        struct {
            char *path;
            unsigned short vendor_id;
            unsigned short product_id;
            wchar_t *serial_number;
            unsigned short release_number;
            wchar_t *manufacturer_string;
            wchar_t *product_string;
            unsigned short usage_page;
            unsigned short usage;
            int interface_number;
            int interface_class;
            int interface_subclass;
            int interface_protocol;
            int bus_type;
            void *next;
        } *devs = (void *)sdl.hid_enumerate(vid, pid);
        if (devs) {
            if (devs->path) {
                hid_dev = sdl.hid_open_path(devs->path);
            }
            if (sdl.hid_free_enumeration) sdl.hid_free_enumeration(devs);
        }
    }
    return hid_dev;
}

/* ---- DualSense audio claim & rendering ---- */
static int rh_ds_claim_audio_dev(uint32_t dev_id) {
    if (dev_id == 0) return 0;
    AcquireSRWLockExclusive(&rh_ds_audio_claim_lock);
    int free_i = -1;
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_claimed_audio_devs[i] == dev_id) {
            ReleaseSRWLockExclusive(&rh_ds_audio_claim_lock);
            return 0;
        }
        if (rh_claimed_audio_devs[i] == 0 && free_i < 0) free_i = i;
    }
    if (free_i < 0) {
        ReleaseSRWLockExclusive(&rh_ds_audio_claim_lock);
        return 0;
    }
    rh_claimed_audio_devs[free_i] = dev_id;
    ReleaseSRWLockExclusive(&rh_ds_audio_claim_lock);
    return 1;
}

static void rh_ds_release_audio_dev(uint32_t dev_id) {
    if (dev_id == 0) return;
    AcquireSRWLockExclusive(&rh_ds_audio_claim_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_claimed_audio_devs[i] == dev_id) {
            rh_claimed_audio_devs[i] = 0;
        }
    }
    ReleaseSRWLockExclusive(&rh_ds_audio_claim_lock);
}

static int rh_ds_render_slot_frames(int slot_idx, float *out, int frames) {
    if (!out || frames <= 0) return -1;
    if (slot_idx < 0 || slot_idx >= RH_MAX_PADS) {
        memset(out, 0, (size_t)frames * 4 * sizeof(float));
        return -1;
    }
    RhDsSlot *slot = &rh_ds_slots[slot_idx];
    AcquireSRWLockExclusive(&slot->dsp_lock);
    int res = -1;
    if (slot->dsp && rh.ds_fill_f32) {
        res = rh.ds_fill_f32(slot->dsp, out, (size_t)frames);
    }
    ReleaseSRWLockExclusive(&slot->dsp_lock);
    if (res < 0) {
        memset(out, 0, (size_t)frames * 4 * sizeof(float));
    }
    return res;
}

int rh_ds_render_frames(float *out, int frames) {
    if (!out || frames <= 0) return -1;
    for (int i = 0; i < RH_MAX_PADS; i++) {
        RhDsSlot *slot = &rh_ds_slots[i];
        AcquireSRWLockExclusive(&slot->dsp_lock);
        if (slot->dsp && rh.ds_fill_f32) {
            int res = rh.ds_fill_f32(slot->dsp, out, (size_t)frames);
            ReleaseSRWLockExclusive(&slot->dsp_lock);
            if (res < 0) memset(out, 0, (size_t)frames * 4 * sizeof(float));
            return res;
        }
        ReleaseSRWLockExclusive(&slot->dsp_lock);
    }
    memset(out, 0, (size_t)frames * 4 * sizeof(float));
    return -1;
}

static void SDLCALL ds_audio_stream_cb(void *userdata, void *stream, int additional_amount, int total_amount) {
    (void)total_amount;
    if (additional_amount <= 0 || !stream) return;
    int slot_idx = (int)(intptr_t)userdata;
    int bytes_per_frame = 4 * (int)sizeof(float);
    int frames_needed = additional_amount / bytes_per_frame;
    if (frames_needed <= 0) return;

    float chunk[DS_CHUNK_FRAMES * 4];
    while (frames_needed > 0) {
        int cur_frames = frames_needed < DS_CHUNK_FRAMES ? frames_needed : DS_CHUNK_FRAMES;
        rh_ds_render_slot_frames(slot_idx, chunk, cur_frames);
        if (sdl.put_audio_data) {
            sdl.put_audio_data(stream, chunk, cur_frames * bytes_per_frame);
        }
        frames_needed -= cur_frames;
    }
}

static void SDLCALL ds_audio_dev_cb(void *userdata, uint8_t *stream, int len) {
    if (len <= 0 || !stream) return;
    int slot_idx = (int)(intptr_t)userdata;
    int bytes_per_frame = 4 * (int)sizeof(float);
    int frames_needed = len / bytes_per_frame;
    float chunk[DS_CHUNK_FRAMES * 4];
    uint8_t *dst = stream;

    while (frames_needed > 0) {
        int cur_frames = frames_needed < DS_CHUNK_FRAMES ? frames_needed : DS_CHUNK_FRAMES;
        rh_ds_render_slot_frames(slot_idx, chunk, cur_frames);
        memcpy(dst, chunk, (size_t)cur_frames * (size_t)bytes_per_frame);
        dst += (size_t)cur_frames * (size_t)bytes_per_frame;
        frames_needed -= cur_frames;
    }
    int rem = len - (int)(dst - stream);
    if (rem > 0) memset(dst, 0, (size_t)rem);
}

static int rh_has_trigger_preset_api(void) {
    return rh.preset_create && rh.preset_destroy && rh.preset_set && rh.preset_tick;
}

static int rh_has_hd_haptics_api(void) {
    return rh.ds_set_speaker && rh.hd_speaker_block && rh.hd_trigger_block &&
           rh.tri_create && rh.tri_destroy && rh.tri_update;
}

static void rh_ds_sync_hd_haptics_slot(int slot_idx) {
    if (slot_idx < 0 || slot_idx >= RH_MAX_PADS) return;
    RhDsSlot *slot = &rh_ds_slots[slot_idx];
    RhUserOpts u = rh_read_opts();

    AcquireSRWLockExclusive(&slot->tri_lock);
    void *joy = NULL;
    int open_speaker = 0;
    int close_speaker = 0;
    int off_triggers = 0;
    int log_speaker = 0;
    int log_trigger = 0;

    AcquireSRWLockExclusive(&slot->lock);
    if (!slot->active || !slot->dsp || !slot->joy) {
        ReleaseSRWLockExclusive(&slot->lock);
        ReleaseSRWLockExclusive(&slot->tri_lock);
        return;
    }
    joy = slot->joy;
    int want = rh_has_hd_haptics_api() && u.enable_hd_rumble &&
               u.dualsense_haptics && u.dualsense_hd_haptics;
    int bt36 = want && slot->is_bt && rh.ds_build_bt36_report != NULL;
    slot->bt_report36 = bt36;

    if (want && slot->pad) {
        for (int s = 0; s < 2; s++) {
            if (!slot->tri[s]) {
                slot->tri[s] = rh.tri_create();
                if (slot->tri[s] && !slot->trigger_logged) {
                    slot->trigger_logged = 1;
                    log_trigger = 1;
                }
            }
            if (rh_has_trigger_preset_api() && !slot->preset[s]) {
                slot->preset[s] = rh.preset_create();
                slot->preset_gen[s] = InterlockedCompareExchange(&rh_preset_gen[s], 0, 0);
                slot->preset_active[s] = 0;
                slot->pulse_until_ms[s] = 0;
                slot->tri_last_valid[s] = 0;
                memset(slot->tri_last[s], 0, sizeof(slot->tri_last[s]));
                slot->tri_last[s][0] = 0x05;
            }
        }
    } else {
        for (int s = 0; s < 2; s++) {
            if (slot->tri[s]) {
                rh.tri_destroy(slot->tri[s]);
                slot->tri[s] = NULL;
                off_triggers = 1;
            }
            if (slot->preset[s]) {
                if (rh.preset_destroy) rh.preset_destroy(slot->preset[s]);
                slot->preset[s] = NULL;
                slot->preset_active[s] = 0;
                slot->pulse_until_ms[s] = 0;
                off_triggers = 1;
            }
        }
    }

    if (!slot->is_bt) {
        if (want && (slot->audio_stream || slot->dev2) && !slot->speaker_on) {
            AcquireSRWLockExclusive(&slot->dsp_lock);
            if (slot->dsp && rh.ds_set_speaker) {
                rh.ds_set_speaker(slot->dsp, RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT);
            }
            ReleaseSRWLockExclusive(&slot->dsp_lock);
            slot->speaker_on = 1;
            open_speaker = 1;
            if (!slot->speaker_logged) {
                slot->speaker_logged = 1;
                log_speaker = 1;
            }
        } else if (!want && slot->speaker_on) {
            AcquireSRWLockExclusive(&slot->dsp_lock);
            if (slot->dsp && rh.ds_set_speaker) {
                rh.ds_set_speaker(slot->dsp, 0);
            }
            ReleaseSRWLockExclusive(&slot->dsp_lock);
            slot->speaker_on = 0;
            close_speaker = 1;
        }
    } else {
        AcquireSRWLockExclusive(&slot->dsp_lock);
        if (slot->dsp && rh.ds_set_speaker) {
            rh.ds_set_speaker(slot->dsp, bt36 ? RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT : 0);
        }
        if (!want) {
            memset(slot->bt_effect_set, 0, sizeof(slot->bt_effect_set));
        }
        ReleaseSRWLockExclusive(&slot->dsp_lock);
    }
    ReleaseSRWLockExclusive(&slot->lock);

    if (off_triggers) {
        AcquireSRWLockExclusive(&slot->dsp_lock);
        memset(slot->bt_effect_set, 0, sizeof(slot->bt_effect_set));
        ReleaseSRWLockExclusive(&slot->dsp_lock);
        if (rh.hd_trigger_block && sdl.send_effect) {
            for (int is_right = 0; is_right <= 1; is_right++) {
                uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
                if (rh.hd_trigger_block(is_right, NULL, blk) == RENHDRUM_OK) {
                    sdl.send_effect(joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
                }
            }
        }
    }
    ReleaseSRWLockExclusive(&slot->tri_lock);

    if (open_speaker && rh.hd_speaker_block && sdl.send_effect) {
        uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
        if (rh.hd_speaker_block(1, blk) == RENHDRUM_OK) {
            sdl.send_effect(joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
        }
        if (log_speaker) {
            hos_log("renhdrum: dualsense speaker on (%d%%)", RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT);
        }
    }
    if (log_trigger) {
        hos_log("renhdrum: dualsense trigger kick on");
    }
    if (close_speaker && rh.hd_speaker_block && sdl.send_effect) {
        uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
        if (rh.hd_speaker_block(0, blk) == RENHDRUM_OK) {
            sdl.send_effect(joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
        }
    }
}

static void rh_ds_try_open_audio(int slot_idx) {
    if (slot_idx < 0 || slot_idx >= RH_MAX_PADS) return;
    RhDsSlot *slot = &rh_ds_slots[slot_idx];
    void *pad = NULL;
    int attempts = 0;

    AcquireSRWLockExclusive(&slot->lock);
    if (!slot->active || !slot->dsp || slot->is_bt || slot->audio_stream || slot->dev2) {
        ReleaseSRWLockExclusive(&slot->lock);
        return;
    }
    slot->audio_attempts++;
    attempts = slot->audio_attempts;
    pad = slot->pad;
    ReleaseSRWLockExclusive(&slot->lock);

    hos_sdl_init(0x10 /* HOS_SDL_AUDIO */);

    if (sdl.is_sdl3) {
        if (!sdl.get_playback_devs || !sdl.get_dev_name || !sdl.get_dev_format || !sdl.open_audio_stream) return;
        if (sdl.pump_events) sdl.pump_events();
        int count = 0;
        uint32_t *devs = sdl.get_playback_devs(&count);
        if (!devs) return;
        uint32_t matched = 0;
        const char *matched_name = NULL;
        for (int i = 0; i < count; i++) {
            const char *name = sdl.get_dev_name(devs[i]);
            if (!name) continue;
            if (strstr(name, "DualSense") || strstr(name, "Wireless Controller")) {
                struct { uint32_t format; int channels, freq; } spec = {0, 0, 0};
                int sample_frames = 0;
                if (sdl.get_dev_format(devs[i], &spec, &sample_frames) && spec.channels >= 4) {
                    if (rh_ds_claim_audio_dev(devs[i])) {
                        matched = devs[i];
                        matched_name = name;
                        break;
                    }
                }
            }
        }
        if (!matched) {
            if (sdl.free_mem) sdl.free_mem(devs);
            return;
        }

        struct { uint32_t format; int channels, freq; } desired = {0x8120 /* SDL_AUDIO_F32 */, 4, 48000};
        void *st = sdl.open_audio_stream(matched, &desired, ds_audio_stream_cb, (void *)(intptr_t)slot_idx);
        if (sdl.free_mem) sdl.free_mem(devs);
        if (!st) {
            rh_ds_release_audio_dev(matched);
            hos_log("renhdrum: dualsense audio stream open failed");
            return;
        }
        if (sdl.resume_audio_stream) sdl.resume_audio_stream(st);

        int keep = 0;
        AcquireSRWLockExclusive(&slot->lock);
        if (slot->active && slot->pad == pad && slot->dsp != NULL && slot->audio_stream == NULL) {
            slot->audio_stream = st;
            slot->audio_device = matched;
            slot->next_audio_check_us = hos_now_ns() / 1000 + 5000000LL;
            keep = 1;
        }
        ReleaseSRWLockExclusive(&slot->lock);

        if (!keep) {
            if (sdl.pause_audio_stream) sdl.pause_audio_stream(st);
            if (sdl.clear_audio_stream) sdl.clear_audio_stream(st);
            if (sdl.destroy_audio_stream) sdl.destroy_audio_stream(st);
            rh_ds_release_audio_dev(matched);
            return;
        }

        if (sdl.rumble_pad && pad) {
            sdl.rumble_pad(pad, 0, 0, 0);
        }
        rh_pads_set_claimed(pad, 1);
        hos_log("renhdrum: dualsense audio opened on \"%s\" (4 ch, 48000 Hz, attempt %d)",
                matched_name ? matched_name : "DualSense", attempts);
        rh_ds_sync_hd_haptics_slot(slot_idx);
    } else {
        if (!sdl.get_num_audio_devs || !sdl.get_audio_dev_name || !sdl.open_audio_device) return;
        int count = sdl.get_num_audio_devs(0);
        const char *matched_name = NULL;
        for (int i = 0; i < count; i++) {
            const char *name = sdl.get_audio_dev_name(i, 0);
            if (!name) continue;
            if (strstr(name, "DualSense") || strstr(name, "Wireless Controller")) {
                matched_name = name;
                break;
            }
        }
        if (!matched_name) return;

        typedef struct {
            int freq;
            uint16_t format;
            uint8_t channels;
            uint8_t silence;
            uint16_t samples;
            uint16_t padding;
            uint32_t size;
            void (SDLCALL *callback)(void *, uint8_t *, int);
            void *userdata;
        } SDL2_AudioSpec;

        SDL2_AudioSpec want, have;
        memset(&want, 0, sizeof(want));
        want.freq = 48000;
        want.format = 0x8120;
        want.channels = 4;
        want.samples = 512;
        want.callback = ds_audio_dev_cb;
        want.userdata = (void *)(intptr_t)slot_idx;

        uint32_t dev = sdl.open_audio_device(matched_name, 0, &want, &have, 0);
        if (!dev) {
            hos_log("renhdrum: dualsense audio open failed (SDL2)");
            return;
        }
        if (sdl.pause_audio_device) sdl.pause_audio_device(dev, 0);

        int keep = 0;
        AcquireSRWLockExclusive(&slot->lock);
        if (slot->active && slot->pad == pad && slot->dsp != NULL && slot->dev2 == 0) {
            slot->dev2 = dev;
            keep = 1;
        }
        ReleaseSRWLockExclusive(&slot->lock);

        if (!keep) {
            if (sdl.pause_audio_device) sdl.pause_audio_device(dev, 1);
            if (sdl.close_audio_device) sdl.close_audio_device(dev);
            return;
        }

        if (sdl.rumble_pad && pad) {
            sdl.rumble_pad(pad, 0, 0, 0);
        }
        rh_pads_set_claimed(pad, 1);
        hos_log("renhdrum: dualsense audio opened on \"%s\" (4 ch, 48000 Hz, attempt %d, SDL2)",
                matched_name, attempts);
        rh_ds_sync_hd_haptics_slot(slot_idx);
    }
}

/* ---- DualSense Bluetooth sender & restart ---- */
static int rh_ds_build_bt_trigger_state(RhDsSlot *slot, uint8_t out47[RENHDRUM_DS5_EFFECTS_SIZE]) {
    if (!slot->bt_effect_set[0] && !slot->bt_effect_set[1]) return 0;
    if (!rh.hd_trigger_block) return 0;
    memset(out47, 0, RENHDRUM_DS5_EFFECTS_SIZE);
    int any = 0;
    for (int s = 0; s < 2; s++) {
        if (!slot->bt_effect_set[s]) continue;
        uint8_t one[RENHDRUM_DS5_EFFECTS_SIZE];
        if (rh.hd_trigger_block(s, slot->bt_effect[s], one) == RENHDRUM_OK) {
            for (size_t k = 0; k < RENHDRUM_DS5_EFFECTS_SIZE; k++) out47[k] |= one[k];
            any = 1;
        }
    }
    return any;
}

static DWORD WINAPI rh_ds_bt_worker(void *arg) {
    int slot_idx = (int)(intptr_t)arg;
    RhDsSlot *slot = &rh_ds_slots[slot_idx];
    uint8_t report142[RENHDRUM_DS5_BT_REPORT_SIZE];
    uint8_t report36[RENHDRUM_DS5_BT36_REPORT_SIZE];
    int64_t next = hos_now_ns();
    int64_t last_signal = 0;
    int64_t stats_at = next;
    int sent = 0, failed = 0, fails_in_row = 0;
    HANDLE timer = CreateWaitableTimerExW(NULL, NULL, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);

    AcquireSRWLockShared(&slot->lock);
    void *hid = slot->active ? slot->bt_hid : NULL;
    ReleaseSRWLockShared(&slot->lock);

    while (hid && InterlockedCompareExchange(&slot->bt_run, 1, 1) == 1) {
        next += RENHDRUM_DS5_BT_PERIOD_NS;

        AcquireSRWLockExclusive(&slot->dsp_lock);
        RenHdcDualSenseDsp *dsp = slot->dsp;
        int combined = slot->bt_report36 && rh.ds_build_bt36_report != NULL;
        int r = -1;
        if (dsp) {
            if (combined) {
                uint8_t state47[RENHDRUM_DS5_EFFECTS_SIZE];
                int have_state = rh_ds_build_bt_trigger_state(slot, state47);
                r = rh.ds_build_bt36_report(dsp, have_state ? state47 : NULL, 0, report36);
            } else if (rh.ds_build_bt_report) {
                r = rh.ds_build_bt_report(dsp, RENHDRUM_DS5_BT_BUFFER_LENGTH_DEFAULT, report142);
            }
        }
        ReleaseSRWLockExclusive(&slot->dsp_lock);

        if (!dsp || (!combined && !rh.ds_build_bt_report)) {
            break;
        }

        int64_t now = hos_now_ns();
        if (r == 1) last_signal = now;

        int send_now = combined
            ? (r == 1)
            : (r >= 0 && last_signal != 0 && (now - last_signal < 2000000000LL));
        if (send_now) {
            int w = -1;
            if (sdl.hid_write) {
                w = combined ? sdl.hid_write(hid, report36, sizeof(report36))
                             : sdl.hid_write(hid, report142, sizeof(report142));
            }
            if (w < 0) {
                failed++;
                fails_in_row++;
            } else {
                sent++;
                fails_in_row = 0;
            }
        }

        if (fails_in_row >= 50) {
            if (InterlockedExchange(&slot->bt_warned, 1) == 0) {
                hos_log("renhdrum: dualsense bt haptics writes failing, using standard rumble");
            }
            AcquireSRWLockShared(&slot->lock);
            void *pad = slot->active ? slot->pad : NULL;
            if (pad) rh_pads_set_claimed(pad, 0);
            ReleaseSRWLockShared(&slot->lock);
            break;
        }

        if (now - stats_at >= 5000000000LL) {
            if (sent > 0 || failed > 0) {
                hos_log("renhdrum: dualsense bt haptics: %d reports/s, %d failed", sent / 5, failed);
            }
            sent = failed = 0;
            stats_at = now;
        }

        if (next > now) {
            int64_t ns = next - now;
            if (timer) {
                LARGE_INTEGER due;
                due.QuadPart = -(ns / 100 > 0 ? ns / 100 : 1);
                if (SetWaitableTimerEx(timer, &due, 0, NULL, NULL, NULL, 0)) {
                    WaitForSingleObject(timer, INFINITE);
                    continue;
                }
            }
            if (ns >= 2000000LL) Sleep((DWORD)((ns - 1000000LL) / 1000000LL));
            while (hos_now_ns() < next) SwitchToThread();
        } else if (now - next > 100000000LL) {
            next = now;
        }
    }

    if (timer) CloseHandle(timer);
    InterlockedExchange(&slot->bt_run, 0);
    return 0;
}

static void rh_ds_restart_bt(int slot_idx) {
    if (slot_idx < 0 || slot_idx >= RH_MAX_PADS) return;
    RhDsSlot *slot = &rh_ds_slots[slot_idx];
    AcquireSRWLockExclusive(&slot->restart_lock);

    HANDLE old_th = NULL;
    void *old_hid = NULL;
    char hid_path[512];
    uint16_t vid = 0, pid = 0;

    AcquireSRWLockExclusive(&slot->lock);
    if (!slot->active || !slot->is_bt || !slot->dsp) {
        slot->bt_starting = 0;
        ReleaseSRWLockExclusive(&slot->lock);
        ReleaseSRWLockExclusive(&slot->restart_lock);
        return;
    }
    old_th = slot->bt_thread;
    old_hid = slot->bt_hid;
    slot->bt_thread = NULL;
    slot->bt_hid = NULL;
    InterlockedExchange(&slot->bt_run, 0);
    memcpy(hid_path, slot->hid_path, sizeof(hid_path));
    vid = slot->vid;
    pid = slot->pid;
    ReleaseSRWLockExclusive(&slot->lock);

    if (old_th) {
        DWORD w = WaitForSingleObject(old_th, 2000);
        CloseHandle(old_th);
        if (w != WAIT_OBJECT_0) {
            AcquireSRWLockExclusive(&slot->lock);
            slot->bt_starting = 0;
            ReleaseSRWLockExclusive(&slot->lock);
            ReleaseSRWLockExclusive(&slot->restart_lock);
            return;
        }
    }
    if (old_hid && sdl.hid_close) sdl.hid_close(old_hid);

    void *hid_dev = rh_open_hid_by_path_or_enum(hid_path, vid, pid);
    if (!hid_dev) {
        AcquireSRWLockExclusive(&slot->lock);
        slot->bt_starting = 0;
        ReleaseSRWLockExclusive(&slot->lock);
        ReleaseSRWLockExclusive(&slot->restart_lock);
        return;
    }

    void *pad = NULL;
    int started = 0;
    AcquireSRWLockExclusive(&slot->lock);
    if (slot->active && slot->is_bt && slot->dsp && !slot->bt_hid && !slot->bt_thread) {
        slot->bt_hid = hid_dev;
        pad = slot->pad;
        InterlockedExchange(&slot->bt_warned, 0);
        InterlockedExchange(&slot->bt_run, 1);
        slot->bt_thread = CreateThread(NULL, 0, rh_ds_bt_worker, (void *)(intptr_t)slot_idx, 0, NULL);
        started = slot->bt_thread != NULL;
        if (!started) {
            InterlockedExchange(&slot->bt_run, 0);
            slot->bt_hid = NULL;
        }
    }
    slot->bt_starting = 0;
    ReleaseSRWLockExclusive(&slot->lock);

    if (!started) {
        if (sdl.hid_close) sdl.hid_close(hid_dev);
        ReleaseSRWLockExclusive(&slot->restart_lock);
        return;
    }

    if (sdl.rumble_pad && pad) sdl.rumble_pad(pad, 0, 0, 0);
    rh_pads_set_claimed(pad, 1);
    ReleaseSRWLockExclusive(&slot->restart_lock);
    rh_ds_sync_hd_haptics_slot(slot_idx);
    hos_log("renhdrum: dualsense bt haptics restarted");
}

static const uint8_t OFF_EFFECT11[11] = {0x05};

static void rh_ds_trigger_step(int slot_idx, int64_t now_us, int is_right) {
    if (slot_idx < 0 || slot_idx >= RH_MAX_PADS) return;
    RhDsSlot *slot = &rh_ds_slots[slot_idx];
    if (!rh.tri_update) return;

    AcquireSRWLockExclusive(&slot->tri_lock);
    void *pad = NULL;
    void *joy = NULL;
    AcquireSRWLockShared(&slot->lock);
    if (slot->active && slot->tri[is_right] && slot->pad && slot->joy) {
        pad = slot->pad;
        joy = slot->joy;
    }
    ReleaseSRWLockShared(&slot->lock);
    if (!pad || !joy) {
        ReleaseSRWLockExclusive(&slot->tri_lock);
        return;
    }

    int axis = rh_sdl_get_axis(pad, is_right ? 5 : 4);
    if (axis < 0) axis = 0;
    if (axis > 32767) axis = 32767;
    uint8_t depth = (uint8_t)(axis * 255 / 32767);

    /* Action buttons (exclude BACK=4, GUIDE=5, START=6, paddles, touchpad, misc) + opposite trigger */
    static const int action_buttons[12] = {0, 1, 2, 3, 7, 8, 9, 10, 11, 12, 13, 14};
    int other_axis = rh_sdl_get_axis(pad, is_right ? 4 : 5);
    if (other_axis < 0) other_axis = 0;
    int other_action = (other_axis * 255 / 32767) > 12;
    for (size_t i = 0; !other_action && i < 12; i++) {
        other_action = rh_sdl_get_button(pad, action_buttons[i]);
    }

    uint8_t effect11[11];
    int changed = 0;
    int bt36 = 0;
    int log_pulse = 0, log_pulse_tri = 0;   /* logged after slot->lock is released */
    static uint32_t pulse_log_ms[2];
    AcquireSRWLockExclusive(&slot->lock);
    if (slot->active && slot->tri[is_right]) {
        uint32_t now_ms = (uint32_t)(now_us / 1000);
        if (other_action && rh.tri_note_action) {
            rh.tri_note_action(slot->tri[is_right], now_ms);
        }
        int fresh = slot->tri_fresh[is_right];
        slot->tri_fresh[is_right] = 0;
        const float *s = slot->tri_sample[is_right];
        uint8_t tri_eff[11];
        int tri_changed = rh.tri_update(slot->tri[is_right], now_ms, depth,
                                        s[0], s[1], s[2], s[3], fresh != 0, tri_eff);
        if (tri_changed == 1) {
            memcpy(slot->tri_last[is_right], tri_eff, 11);
            slot->tri_last_valid[is_right] = (tri_eff[0] != 0x05);
        }
        if (slot->preset[is_right] && rh.preset_set && rh.preset_tick) {
            LONG g = InterlockedCompareExchange(&rh_preset_gen[is_right], 0, 0);
            if (g != slot->preset_gen[is_right]) {
                slot->preset_gen[is_right] = g;
                LONG packed = InterlockedCompareExchange(&rh_preset_packed[is_right], 0, 0);
                uint8_t pid = (uint8_t)((packed >> 16) & 0xFF);
                float plvl = (float)(packed & 0xFFFF) / 1000.0f;
                rh.preset_set(slot->preset[is_right], pid, plvl, now_ms);
                if (pid == 12) {   /* IMPACT_PULSE: for RH_PULSE_BEATS_TRI_MS it beats layer 1 (plan 143 6c round 1) */
                    slot->pulse_until_ms[is_right] = now_ms + RH_PULSE_BEATS_TRI_MS;
                    if (now_ms - pulse_log_ms[is_right] >= 200) {
                        pulse_log_ms[is_right] = now_ms;
                        log_pulse = 1;
                        log_pulse_tri = slot->tri_last_valid[is_right];
                    }
                }
            }
            const int in_pulse = slot->pulse_until_ms[is_right] && now_ms < slot->pulse_until_ms[is_right];
            uint8_t p11[11];
            int pt = rh.preset_tick(slot->preset[is_right], now_ms, p11);
            /* Which bytes go out (plan 143 6c round 7), in this order: (a) a pulse is on: the preset's bytes; (b) layer 1 (the game's
               rumble-following trigger) has an effect: its new bytes whenever they change, whatever the preset does; (c) layer 1 has
               none: the preset's bytes (Off too); (d) layer 1 just went Off: the running preset's last bytes, else Off. */
            if (pt == 1) {
                slot->preset_active[is_right] = p11[0] != 0x05;
                if (slot->preset_active[is_right]) memcpy(slot->preset_last[is_right], p11, 11);
            }
            if (in_pulse && pt == 1) {
                memcpy(effect11, p11, 11);
                changed = 1;
            } else if (in_pulse) {
                /* pulse holds the trigger */
            } else if (slot->tri_last_valid[is_right]) {
                if (tri_changed == 1) memcpy(effect11, tri_eff, 11), changed = 1;
            } else if (pt == 1) {
                memcpy(effect11, p11, 11);
                changed = 1;
            } else if (tri_changed == 1) {
                memcpy(effect11, slot->preset_active[is_right] ? slot->preset_last[is_right] : OFF_EFFECT11, 11);
                changed = 1;
            }
            /* The pulse is over: give the trigger back to layer 1 if it has an effect, else the running preset, else Off. */
            if (slot->pulse_until_ms[is_right] && !in_pulse) {
                slot->pulse_until_ms[is_right] = 0;
                if (!changed) {
                    memcpy(effect11, slot->tri_last_valid[is_right] ? slot->tri_last[is_right]
                                     : slot->preset_active[is_right] ? slot->preset_last[is_right] : OFF_EFFECT11, 11);
                    changed = 1;
                }
            }
        } else if (tri_changed == 1) {
            memcpy(effect11, tri_eff, 11);
            changed = 1;
        }
        bt36 = slot->bt_report36;
        joy = slot->joy;
    }
    ReleaseSRWLockExclusive(&slot->lock);

    if (log_pulse) hos_log("trigger: pulse side=%d tri=%d", is_right, log_pulse_tri);
    if (changed == 1) {
        if (bt36) {
            AcquireSRWLockExclusive(&slot->dsp_lock);
            memcpy(slot->bt_effect[is_right], effect11, 11);
            slot->bt_effect_set[is_right] = 1;
            ReleaseSRWLockExclusive(&slot->dsp_lock);
        } else if (joy && rh.hd_trigger_block && sdl.send_effect) {
            uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
            if (rh.hd_trigger_block(is_right, effect11, blk) == RENHDRUM_OK) {
                sdl.send_effect(joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
            }
        }
    }
    ReleaseSRWLockExclusive(&slot->tri_lock);
}

static int rh_ds_detach_slot(int slot_idx, int untrack_pad) {
    if (slot_idx < 0 || slot_idx >= RH_MAX_PADS) return 0;
    RhDsSlot *slot = &rh_ds_slots[slot_idx];

    AcquireSRWLockExclusive(&slot->restart_lock);
    AcquireSRWLockExclusive(&slot->tri_lock);
    AcquireSRWLockExclusive(&slot->lock);
    if (!slot->active) {
        ReleaseSRWLockExclusive(&slot->lock);
        ReleaseSRWLockExclusive(&slot->tri_lock);
        ReleaseSRWLockExclusive(&slot->restart_lock);
        return 0;
    }

    void *pad = slot->pad;
    void *joy = slot->joy;
    void *stream = slot->audio_stream;
    uint32_t audio_dev = slot->audio_device;
    uint32_t dev2 = slot->dev2;
    void *bt_hid = slot->bt_hid;
    HANDLE bt_th = slot->bt_thread;
    int speaker_on = slot->speaker_on;
    RenHdcTriTrigger *tri0 = slot->tri[0];
    RenHdcTriTrigger *tri1 = slot->tri[1];
    RenHdcTriggerPreset *preset0 = slot->preset[0];
    RenHdcTriggerPreset *preset1 = slot->preset[1];
    int had_tri = (tri0 != NULL || tri1 != NULL || preset0 != NULL || preset1 != NULL);

    AcquireSRWLockExclusive(&slot->dsp_lock);
    RenHdcDualSenseDsp *dsp = slot->dsp;
    slot->dsp = NULL;
    memset(slot->bt_effect_set, 0, sizeof(slot->bt_effect_set));
    ReleaseSRWLockExclusive(&slot->dsp_lock);

    /* Destroy audio stream under slot->lock (AudioCallback only takes dsp_lock, which sees dsp == NULL) */
    if (stream) {
        if (sdl.pause_audio_stream) sdl.pause_audio_stream(stream);
        if (sdl.clear_audio_stream) sdl.clear_audio_stream(stream);
        if (sdl.destroy_audio_stream) sdl.destroy_audio_stream(stream);
    }
    rh_ds_release_audio_dev(audio_dev);

    slot->active = 0;
    slot->pad = NULL;
    slot->joy = NULL;
    slot->vid = 0;
    slot->pid = 0;
    slot->is_bt = 0;
    slot->hid_path[0] = '\0';
    slot->audio_stream = NULL;
    slot->audio_device = 0;
    slot->dev2 = 0;
    slot->bt_hid = NULL;
    slot->bt_thread = NULL;
    slot->speaker_on = 0;
    slot->bt_report36 = 0;
    slot->tri[0] = NULL;
    slot->tri[1] = NULL;
    slot->tri_fresh[0] = slot->tri_fresh[1] = 0;
    slot->preset[0] = NULL;
    slot->preset[1] = NULL;
    slot->preset_gen[0] = slot->preset_gen[1] = 0;
    slot->preset_active[0] = slot->preset_active[1] = 0;
    slot->pulse_until_ms[0] = slot->pulse_until_ms[1] = 0;
    slot->tri_last_valid[0] = slot->tri_last_valid[1] = 0;
    memset(slot->tri_last, 0, sizeof(slot->tri_last));
    InterlockedExchange(&slot->bt_run, 0);
    ReleaseSRWLockExclusive(&slot->lock);

    if (tri0 && rh.tri_destroy) rh.tri_destroy(tri0);
    if (tri1 && rh.tri_destroy) rh.tri_destroy(tri1);
    if (preset0 && rh.preset_destroy) rh.preset_destroy(preset0);
    if (preset1 && rh.preset_destroy) rh.preset_destroy(preset1);
    if (had_tri && joy && rh.hd_trigger_block && sdl.send_effect) {
        for (int is_right = 0; is_right <= 1; is_right++) {
            uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
            if (rh.hd_trigger_block(is_right, NULL, blk) == RENHDRUM_OK) {
                sdl.send_effect(joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
            }
        }
    }
    ReleaseSRWLockExclusive(&slot->tri_lock);

    if (speaker_on && joy && rh.hd_speaker_block && sdl.send_effect) {
        uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
        if (rh.hd_speaker_block(0, blk) == RENHDRUM_OK) {
            sdl.send_effect(joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
        }
    }
    if (dev2) {
        if (sdl.pause_audio_device) sdl.pause_audio_device(dev2, 1);
        if (sdl.close_audio_device) sdl.close_audio_device(dev2);
    }
    if (bt_th) {
        DWORD w = WaitForSingleObject(bt_th, 2000);
        CloseHandle(bt_th);
        if (w != WAIT_OBJECT_0) {
            hos_log("renhdrum: DualSense BT worker did not stop within 2000 ms (wait=0x%lx); skipping hid_close/ds_destroy",
                    (unsigned long)w);
            bt_hid = NULL;
            dsp = NULL;
        }
    }
    if (bt_hid && sdl.hid_close) sdl.hid_close(bt_hid);
    if (dsp && rh.ds_destroy) rh.ds_destroy(dsp);

    ReleaseSRWLockExclusive(&slot->restart_lock);

    if (pad) {
        if (untrack_pad) rh_pads_untrack(pad);
        else rh_pads_set_claimed(pad, 0);
    }
    return 1;
}

/* ---- Steam Controller 2026 grips (HID 0x86 mode + 0x88 reports) ---- */
static void rh_sv_legacy_mode(void *hid, uint8_t op, uint8_t format) {
    static const uint8_t channels[2] = {2, 5};
    if (!sdl.hid_write) return;
    for (int i = 0; i < 2; i++) {
        const uint8_t r[4] = {0x86, op, channels[i], format};
        sdl.hid_write(hid, r, sizeof(r));
    }
}

static void rh_sv_send_mode_off(void *hid, uint8_t format) {
    if (!hid || !sdl.hid_write) return;
    if (rh.sv_mode_off) {
        uint8_t reports[RENHDRUM_VALVE_GRIP_MODE_REPORTS_SIZE];
        const uint8_t fmts[2] = {format, 0};
        for (int f = 0; f < 2; f++) {
            if (rh.sv_mode_off(fmts[f], reports) == RENHDRUM_VALVE_GRIP_MODE_REPORT_COUNT) {
                sdl.hid_write(hid, reports, RENHDRUM_VALVE_GRIP_MODE_REPORT_SIZE);
                sdl.hid_write(hid, reports + RENHDRUM_VALVE_GRIP_MODE_REPORT_SIZE,
                              RENHDRUM_VALVE_GRIP_MODE_REPORT_SIZE);
            }
        }
        return;
    }
    rh_sv_legacy_mode(hid, 1, format);
    rh_sv_legacy_mode(hid, 1, 0);
}

static DWORD WINAPI rh_sv_worker(void *arg) {
    (void)arg;
    uint8_t report[RENHDRUM_VALVE_GRIP_REPORT_SIZE];
    uint8_t mode_reports[RENHDRUM_VALVE_GRIP_MODE_REPORTS_SIZE];

    AcquireSRWLockExclusive(&rh_sv_lock);
    void *hid = rh_sv_hid;
    const uint8_t format = rh_sv_format;
    void *pad = rh_sv_pad;
    ReleaseSRWLockExclusive(&rh_sv_lock);

    const int use_mode_tick = (rh.sv_mode_tick != NULL && rh.sv_mode_off != NULL);
    const int64_t period = format == RENHDRUM_VALVE_GRIP_FORMAT_MULAW_8K
        ? (int64_t)RENHDRUM_VALVE_GRIP_PERIOD_NS_8K
        : (int64_t)RENHDRUM_VALVE_GRIP_PERIOD_NS_4K;
    int gave_up = 0;

    if (!use_mode_tick) {
        rh_sv_legacy_mode(hid, 1, 0);
        Sleep(10);
        rh_sv_legacy_mode(hid, 2, format);
        Sleep(200);
    }

    HANDLE timer = CreateWaitableTimerExW(NULL, NULL, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);
    int64_t next = hos_now_ns();
    int64_t last_signal = 0;
    int fails_in_row = 0;
    bool write_stalled = false;

    while (InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) {
        next += period;
        int64_t now = hos_now_ns();
        int built = -1;
        int mode_count = 0;

        AcquireSRWLockExclusive(&rh_sv_lock);
        RenHdcDualSenseDsp *dsp = rh_sv_dsp;
        if (dsp && rh.sv_build) {
            built = rh.sv_build(dsp, format, report);
            if (use_mode_tick && built >= 0) {
                mode_count = rh.sv_mode_tick(dsp, format, (uint64_t)now, write_stalled, mode_reports);
            }
        }
        ReleaseSRWLockExclusive(&rh_sv_lock);

        if (!dsp) break;

        write_stalled = false;
        if (mode_count == RENHDRUM_VALVE_GRIP_MODE_REPORT_COUNT && sdl.hid_write) {
            for (int i = 0; i < mode_count; i++) {
                if (sdl.hid_write(hid, mode_reports + i * RENHDRUM_VALVE_GRIP_MODE_REPORT_SIZE,
                                  RENHDRUM_VALVE_GRIP_MODE_REPORT_SIZE) < 0) {
                    write_stalled = true;
                    fails_in_row++;
                } else {
                    fails_in_row = 0;
                }
            }
        }

        if (built == 1 || built == RENHDRUM_VALVE_GRIP_SKIP) {
            last_signal = now;
        }
        if (built >= 0 && built != RENHDRUM_VALVE_GRIP_SKIP &&
            last_signal != 0 && (now - last_signal) < RH_STEAM_TAIL_NS) {
            if (sdl.hid_write) {
                int64_t t0 = hos_now_ns();
                int w = sdl.hid_write(hid, report, sizeof(report));
                int64_t t1 = hos_now_ns();
                if (w < 0) {
                    write_stalled = true;
                    fails_in_row++;
                } else {
                    fails_in_row = 0;
                    if ((t1 - t0) > period) write_stalled = true;
                }
            }
        }
        if (fails_in_row >= RH_STEAM_FAILS_MAX) {
            hos_log("renhdrum: steam controller grip writes failing, using standard rumble");
            gave_up = 1;
            break;
        }

        int64_t after = hos_now_ns();
        if (next > after) {
            int64_t ns = next - after;
            if (timer) {
                LARGE_INTEGER due;
                due.QuadPart = -(ns / 100 > 0 ? ns / 100 : 1);
                if (SetWaitableTimerEx(timer, &due, 0, NULL, NULL, NULL, 0)) {
                    WaitForSingleObject(timer, INFINITE);
                    continue;
                }
            }
            if (ns >= 2000000LL) Sleep((DWORD)((ns - 1000000LL) / 1000000LL));
            while (hos_now_ns() < next) SwitchToThread();
        } else if (after - next > 100000000LL) {
            next = after;
            write_stalled = true;
        }
    }

    if (timer) CloseHandle(timer);
    rh_sv_send_mode_off(hid, format);
    InterlockedExchange(&rh_sv_run, 0);
    if (gave_up) rh_pads_set_claimed(pad, 0);
    return 0;
}

static void *rh_sv_stop(void *pad) {
    AcquireSRWLockExclusive(&rh_sv_lock);
    if (!rh_sv_pad || (pad && rh_sv_pad != pad)) {
        ReleaseSRWLockExclusive(&rh_sv_lock);
        return NULL;
    }
    void *had = rh_sv_pad;
    void *hid = rh_sv_hid;
    uint8_t format = rh_sv_format;
    HANDLE th = rh_sv_thread;
    RenHdcDualSenseDsp *dsp = rh_sv_dsp;
    rh_sv_pad = NULL;
    rh_sv_hid = NULL;
    rh_sv_thread = NULL;
    rh_sv_dsp = NULL;
    InterlockedExchange(&rh_sv_run, 0);
    ReleaseSRWLockExclusive(&rh_sv_lock);

    if (th) {
        DWORD w = WaitForSingleObject(th, 2000);
        CloseHandle(th);
        if (w != WAIT_OBJECT_0) {
            hos_log("renhdrum: Steam grips worker did not stop within 2000 ms (wait=0x%lx); skipping hid_close/dsp destroy",
                    (unsigned long)w);
            return had;
        }
    } else if (hid) {
        rh_sv_send_mode_off(hid, format);
    }
    if (hid && sdl.hid_close) sdl.hid_close(hid);
    if (dsp && rh.ds_destroy) rh.ds_destroy(dsp);
    return had;
}

/* ---- Xbox detach helper ---- */
static int rh_xb_detach(void *pad) {
    AcquireSRWLockExclusive(&rh_xb_lock);
    if (!rh_xb_pad || (pad && rh_xb_pad != pad)) {
        ReleaseSRWLockExclusive(&rh_xb_lock);
        return 0;
    }
    void *joy = rh_xb_joy ? rh_xb_joy : rh_xb_pad;
    RenHdcXboxRumble *x = rh_xb;
    RenHdcXboxDirect *xd = rh_xb_direct;
    rh_xb = NULL;
    rh_xb_direct = NULL;
    rh_xb_pad = NULL;
    rh_xb_joy = NULL;
    rh_xb_triggers = 0;
    ReleaseSRWLockExclusive(&rh_xb_lock);

    if (joy && sdl.rumble_joy) {
        sdl.rumble_joy(joy, 0, 0, 0);
        if (sdl.rumble_joy_trig) sdl.rumble_joy_trig(joy, 0, 0, 0);
    }
    if (xd && rh.xb_direct_destroy) rh.xb_direct_destroy(xd);
    if (x && rh.xb_destroy) rh.xb_destroy(x);
    return 1;
}

/* ---- Worker thread ---- */
static DWORD WINAPI rh_worker(void *arg) {
    (void)arg;
    uint8_t pkt[10];
    while (InterlockedCompareExchange(&rh_run, 1, 1) == 1) {
        int64_t now_us = hos_now_ns() / 1000;

        /* 1. Pro Controller watchdog, tick, and wired-only 40 ms resend */
        int send_pkt = 0;
        void *target = NULL;
        AcquireSRWLockExclusive(&rh_lock);
        if (rh_engine) {
            for (int s = 0; s < 2; s++) {
                if (rh_at[s] && !rh_muted[s] && (now_us - rh_at[s] > RH_STALE_US)) {
                    rh.push(rh_engine, now_us, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    rh_muted[s] = 1;
                }
            }
            int r = rh.tick(rh_engine, now_us, pkt);
            if (r == 1) {
                memcpy(rh_last_pkt, pkt, sizeof(pkt));
                rh_has_last_pkt = !rh_pkt_is_silent(pkt);
                rh_last_send_us = now_us;
                send_pkt = 1;
            } else if (!rh_is_bt && !rh_usb_no_queue && rh_has_last_pkt && (now_us - rh_last_send_us) >= RH_RESEND_US) {
                memcpy(pkt, rh_last_pkt, sizeof(pkt));
                rh_last_send_us = now_us;
                send_pkt = 1;
            }
            target = rh_joy ? rh_joy : rh_active_pad;
        }
        ReleaseSRWLockExclusive(&rh_lock);

        if (send_pkt && target && sdl.send_effect) {
            sdl.send_effect(target, pkt, 10);
        }

        /* 1b. Switch 2 Pro Controller (experimental) */
        int send_pro2 = 0;
        uint8_t pro2_pkt[RENHDRUM_PRO2_REPORT_SIZE];
        void *pro2_target = NULL;
        void *pro2_pad_cur = NULL;
        AcquireSRWLockExclusive(&rh_pro2_lock);
        if (rh_pro2 && rh.pro2_push && rh.pro2_build) {
            for (int s = 0; s < 2; s++) {
                if (rh_pro2_at[s] && !rh_pro2_muted[s] && (now_us - rh_pro2_at[s] > RH_STALE_US)) {
                    rh.pro2_push(rh_pro2, now_us, s == 1, 0.0f, 80.0f, 0.0f, 160.0f);
                    rh_pro2_muted[s] = 1;
                }
            }
            if (rh.pro2_build(rh_pro2, now_us, pro2_pkt) == 1) {
                pro2_target = rh_pro2_joy ? rh_pro2_joy : rh_pro2_pad;
                pro2_pad_cur = rh_pro2_pad;
                send_pro2 = 1;
            }
        }
        ReleaseSRWLockExclusive(&rh_pro2_lock);

        if (send_pro2 && pro2_target && sdl.send_effect) {
            int ok = sdl_ok(sdl.send_effect(pro2_target, pro2_pkt, RENHDRUM_PRO2_REPORT_SIZE));
            RenHdcPro2 *drop = NULL;
            void *drop_pad = NULL;
            AcquireSRWLockExclusive(&rh_pro2_lock);
            if (rh_pro2 && rh_pro2_pad == pro2_pad_cur) {
                if (ok) {
                    rh_pro2_fails = 0;
                } else if (++rh_pro2_fails >= RH_PRO2_FAILS_MAX) {
                    drop = rh_pro2;
                    drop_pad = rh_pro2_pad;
                    rh_pro2 = NULL;
                    rh_pro2_pad = NULL;
                    rh_pro2_joy = NULL;
                    rh_pro2_at[0] = rh_pro2_at[1] = 0;
                    rh_pro2_muted[0] = rh_pro2_muted[1] = 0;
                    rh_pro2_fails = 0;
                }
            }
            ReleaseSRWLockExclusive(&rh_pro2_lock);
            if (drop) {
                if (drop_pad) rh_pads_set_claimed(drop_pad, 0);
                if (rh.pro2_destroy) rh.pro2_destroy(drop);
                hos_log("renhdrum: Switch 2 Pro Controller refused %d packets, falling back to normal rumble",
                        RH_PRO2_FAILS_MAX);
            }
        }

        /* 2. Xbox One S: stale watchdog & tick (RenHDrum and Direct modes) */
        uint16_t motors[4] = {0, 0, 0, 0};
        int send_xb = 0;
        int xb_direct_mask = 0;
        void *xb_target = NULL;
        int xb_trig = 0;
        int xb_strength = 100;

        AcquireSRWLockExclusive(&rh_xb_lock);
        if (rh_xb) {
            xb_target = rh_xb_joy;
            xb_trig = rh_xb_triggers;
            xb_strength = rh_xb_strength;
            if (rh_xb_direct_mode && rh_xb_direct && rh.xb_direct_push && rh.xb_direct_tick) {
                for (int s = 0; s < 2; s++) {
                    if (rh_xb_at[s] && !rh_xb_muted[s] && (now_us - rh_xb_at[s] > RH_STALE_US)) {
                        uint16_t sm[4] = {0, 0, 0, 0};
                        AcquireSRWLockShared(&rh_store_lock);
                        int smask = rh_store ? rh.xb_direct_push(rh_xb_direct, rh_store, now_us, s == 1,
                                                                 0.0f, 0.0f, xb_trig != 0, sm) : 0;
                        ReleaseSRWLockShared(&rh_store_lock);
                        if (smask > 0) {
                            xb_direct_mask |= smask;
                            if (smask & 1) { motors[0] = sm[0]; motors[1] = sm[1]; }
                            if (smask & 2) { motors[2] = sm[2]; motors[3] = sm[3]; }
                        }
                        rh_xb_muted[s] = 1;
                    }
                }
                uint16_t tm[4] = {0, 0, 0, 0};
                int tmask = rh.xb_direct_tick(rh_xb_direct, now_us, xb_trig != 0, tm);
                if (tmask > 0) {
                    xb_direct_mask |= tmask;
                    if (tmask & 1) { motors[0] = tm[0]; motors[1] = tm[1]; }
                    if (tmask & 2) { motors[2] = tm[2]; motors[3] = tm[3]; }
                }
            } else {
                for (int s = 0; s < 2; s++) {
                    if (rh_xb_at[s] && !rh_xb_muted[s] && (now_us - rh_xb_at[s] > RH_STALE_US)) {
                        rh.xb_push(rh_xb, now_us, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                        rh_xb_muted[s] = 1;
                    }
                }
                send_xb = rh.xb_tick(rh_xb, now_us, motors) == 1;
            }
        }
        ReleaseSRWLockExclusive(&rh_xb_lock);

        if (xb_direct_mask > 0 && xb_target) {
            if ((xb_direct_mask & 1) && sdl.rumble_joy) {
                sdl.rumble_joy(xb_target, motors[0], motors[1], 2000);
            }
            if ((xb_direct_mask & 2) && xb_trig && sdl.rumble_joy_trig) {
                sdl.rumble_joy_trig(xb_target, motors[2], motors[3], 2000);
            }
        } else if (send_xb && xb_target) {
            if (xb_strength != 100) {
                for (int k = 0; k < 4; k++) {
                    uint32_t scaled = (uint32_t)motors[k] * (uint32_t)xb_strength / 100u;
                    motors[k] = (uint16_t)(scaled > 65535u ? 65535u : scaled);
                }
            }
            int ok = sdl.rumble_joy ? sdl_ok(sdl.rumble_joy(xb_target, motors[0], motors[1], 2000)) : 0;
            if (xb_trig && sdl.rumble_joy_trig) {
                sdl.rumble_joy_trig(xb_target, motors[2], motors[3], 200);
            }
            if (!ok && rh.xb_resend) {
                AcquireSRWLockExclusive(&rh_xb_lock);
                if (rh_xb) rh.xb_resend(rh_xb);
                ReleaseSRWLockExclusive(&rh_xb_lock);
            }
        }

        /* 3. Steam grips watchdog */
        AcquireSRWLockExclusive(&rh_sv_lock);
        if (rh_sv_dsp && rh.ds_set_half_target) {
            for (int s = 0; s < 2; s++) {
                if (rh_sv_at[s] && !rh_sv_muted[s] && (now_us - rh_sv_at[s] > RH_STALE_US)) {
                    rh.ds_set_half_target(rh_sv_dsp, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    rh_sv_muted[s] = 1;
                }
            }
        }
        ReleaseSRWLockExclusive(&rh_sv_lock);

        /* 4. DualSense slots: watchdog, audio open/loss recovery, BT restart, trigger kick */
        for (int i = 0; i < RH_MAX_PADS; i++) {
            RhDsSlot *slot = &rh_ds_slots[i];
            int need_audio_retry = 0;
            int need_bt_restart = 0;
            void *check_stream = NULL;
            uint32_t check_audio_dev = 0;
            int has_tri = 0;

            AcquireSRWLockExclusive(&slot->lock);
            if (slot->active && slot->dsp) {
                for (int s = 0; s < 2; s++) {
                    if (slot->at[s] && !slot->muted[s] && (now_us - slot->at[s] > RH_STALE_US)) {
                        if (rh.ds_set_half_target) {
                            AcquireSRWLockExclusive(&slot->dsp_lock);
                            if (slot->dsp) {
                                rh.ds_set_half_target(slot->dsp, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                            }
                            ReleaseSRWLockExclusive(&slot->dsp_lock);
                        }
                        slot->muted[s] = 1;
                    }
                }
                if (!slot->is_bt) {
                    if (!slot->audio_stream && !slot->dev2 && now_us >= slot->next_audio_retry_us) {
                        slot->next_audio_retry_us = now_us +
                            (slot->audio_attempts < RH_DS_AUDIO_MAX_ATTEMPTS ? 2000000LL : 5000000LL);
                        need_audio_retry = 1;
                    } else if (sdl.is_sdl3 && slot->audio_stream && slot->audio_device != 0 &&
                               now_us >= slot->next_audio_check_us) {
                        slot->next_audio_check_us = now_us + 5000000LL;
                        check_stream = slot->audio_stream;
                        check_audio_dev = slot->audio_device;
                    }
                } else if (InterlockedCompareExchange(&slot->bt_run, 1, 1) == 0 &&
                           !slot->bt_unsupported && !slot->bt_starting &&
                           now_us >= slot->bt_next_retry_us) {
                    slot->bt_next_retry_us = now_us + 10000000LL;
                    slot->bt_starting = 1;
                    need_bt_restart = 1;
                }
                has_tri = (slot->tri[0] != NULL || slot->tri[1] != NULL);
            }
            ReleaseSRWLockExclusive(&slot->lock);

            if (need_audio_retry) {
                rh_ds_try_open_audio(i);
            }
            if (check_stream && check_audio_dev != 0 && sdl.get_playback_devs) {
                if (sdl.pump_events) sdl.pump_events();
                int count = 0;
                uint32_t *devs = sdl.get_playback_devs(&count);
                if (devs != NULL) {
                    int present = 0;
                    for (int k = 0; k < count; k++) {
                        if (devs[k] == check_audio_dev) {
                            present = 1;
                            break;
                        }
                    }
                    if (sdl.free_mem) sdl.free_mem(devs);
                    if (!present) {
                        int lost = 0;
                        int lost_speaker = 0;
                        void *lost_pad = NULL;
                        void *lost_joy = NULL;
                        AcquireSRWLockExclusive(&slot->lock);
                        if (slot->active && slot->audio_stream == check_stream) {
                            if (sdl.pause_audio_stream) sdl.pause_audio_stream(slot->audio_stream);
                            if (sdl.clear_audio_stream) sdl.clear_audio_stream(slot->audio_stream);
                            if (sdl.destroy_audio_stream) sdl.destroy_audio_stream(slot->audio_stream);
                            slot->audio_stream = NULL;
                            rh_ds_release_audio_dev(slot->audio_device);
                            slot->audio_device = 0;
                            slot->audio_attempts = RH_DS_AUDIO_MAX_ATTEMPTS;
                            slot->next_audio_retry_us = now_us + 2000000LL;
                            lost = 1;
                            lost_pad = slot->pad;
                            lost_joy = slot->joy;
                            lost_speaker = slot->speaker_on;
                            slot->speaker_on = 0;
                            if (lost_speaker && rh.ds_set_speaker) {
                                AcquireSRWLockExclusive(&slot->dsp_lock);
                                if (slot->dsp) rh.ds_set_speaker(slot->dsp, 0);
                                ReleaseSRWLockExclusive(&slot->dsp_lock);
                            }
                        }
                        ReleaseSRWLockExclusive(&slot->lock);

                        if (lost_speaker && lost_joy && rh.hd_speaker_block && sdl.send_effect) {
                            uint8_t blk[RENHDRUM_DS5_EFFECTS_SIZE];
                            if (rh.hd_speaker_block(0, blk) == RENHDRUM_OK) {
                                sdl.send_effect(lost_joy, blk, RENHDRUM_DS5_EFFECTS_SIZE);
                            }
                        }
                        if (lost) {
                            if (lost_pad) rh_pads_set_claimed(lost_pad, 0);
                            hos_log("renhdrum: dualsense speaker went away, standard rumble until it returns");
                        }
                    }
                }
            }
            if (need_bt_restart) {
                rh_ds_restart_bt(i);
            }
            if (has_tri) {
                rh_ds_trigger_step(i, now_us, 0);
                rh_ds_trigger_step(i, now_us, 1);
            }
        }

        Sleep(1);
    }
    return 0;
}

static void rh_ensure_worker(void) {
    if (InterlockedCompareExchange(&rh_run, 1, 0) == 0) {
        rh_thread = CreateThread(NULL, 0, rh_worker, NULL, 0, NULL);
        if (!rh_thread) {
            InterlockedExchange(&rh_run, 0);
            hos_log("renhdrum: off (cannot create worker thread)");
        }
    }
}

static int rh_detach_engines(void *pad, int untrack) {
    if (untrack) {
        rh_pads_untrack(pad);
    } else {
        rh_pads_clear_claimed();
    }

    /* Pro Controller detach */
    AcquireSRWLockExclusive(&rh_lock);
    if (rh_active_pad && (!pad || rh_active_pad == pad)) {
        void *joy = rh_joy;
        void *target = joy ? joy : rh_active_pad;
        RenHdcEngine *e = rh_engine;
        rh_engine = NULL;
        rh_active_pad = NULL;
        rh_joy = NULL;
        rh_is_bt = 0;
        rh_usb_no_queue = 0;
        memset(rh_last_pkt, 0, sizeof(rh_last_pkt));
        rh_has_last_pkt = 0;
        rh_last_send_us = 0;
        ReleaseSRWLockExclusive(&rh_lock);

        rh_clear_pro_transport(joy);
        static const uint8_t stop_pkt[10] = {0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40};
        if (target && sdl.send_effect) sdl.send_effect(target, stop_pkt, sizeof(stop_pkt));
        if (e && rh.destroy) rh.destroy(e);
    } else {
        ReleaseSRWLockExclusive(&rh_lock);
    }

    /* DualSense slots detach */
    for (int i = 0; i < RH_MAX_PADS; i++) {
        void *slot_pad = NULL;
        AcquireSRWLockShared(&rh_ds_slots[i].lock);
        if (rh_ds_slots[i].active) slot_pad = rh_ds_slots[i].pad;
        ReleaseSRWLockShared(&rh_ds_slots[i].lock);
        if (slot_pad && (!pad || slot_pad == pad)) {
            rh_ds_detach_slot(i, untrack);
        }
    }

    /* Switch 2 Pro Controller detach */
    rh_pro2_detach(pad);

    /* Xbox detach */
    rh_xb_detach(pad);

    /* Steam grips detach */
    rh_sv_stop(pad);

    /* Stop worker if no pad it serves is active */
    AcquireSRWLockShared(&rh_lock);
    int pro_active = (rh_engine != NULL);
    ReleaseSRWLockShared(&rh_lock);

    AcquireSRWLockShared(&rh_pro2_lock);
    int pro2_active = (rh_pro2 != NULL);
    ReleaseSRWLockShared(&rh_pro2_lock);

    int ds_active = 0;
    for (int i = 0; i < RH_MAX_PADS; i++) {
        AcquireSRWLockShared(&rh_ds_slots[i].lock);
        if (rh_ds_slots[i].active) ds_active = 1;
        ReleaseSRWLockShared(&rh_ds_slots[i].lock);
    }

    AcquireSRWLockShared(&rh_xb_lock);
    int xb_active = (rh_xb != NULL);
    ReleaseSRWLockShared(&rh_xb_lock);

    AcquireSRWLockShared(&rh_sv_lock);
    int sv_active = (rh_sv_dsp != NULL);
    ReleaseSRWLockShared(&rh_sv_lock);

    if (!pro_active && !pro2_active && !ds_active && !xb_active && !sv_active) {
        HANDLE t = rh_thread;
        rh_thread = NULL;
        InterlockedExchange(&rh_run, 0);
        if (t) {
            WaitForSingleObject(t, 2000);
            CloseHandle(t);
        }
    }
    return 1;
}

static void rh_apply_settings_live(void) {
    RhUserOpts u = rh_read_opts();

    /* 1. Pro Controller live reconfigure */
    void *pro_joy = NULL;
    int pro_bt = 0;
    int pro_have = 0;
    AcquireSRWLockShared(&rh_lock);
    if (rh_engine && rh.apply) {
        pro_joy = rh_joy;
        pro_bt = rh_is_bt;
        pro_have = 1;
    }
    ReleaseSRWLockShared(&rh_lock);
    if (pro_have) {
        RenHdcSettings s;
        if (rh_resolve_pro_settings(&s, pro_bt)) {
            rh_apply_pro_transport(pro_joy, &s, pro_bt);
            AcquireSRWLockExclusive(&rh_lock);
            if (rh_engine && rh.apply) {
                rh.apply(rh_engine, &s);
                rh_usb_no_queue = (!rh_is_bt && s.usb_no_queue) ? 1 : 0;
            }
            ReleaseSRWLockExclusive(&rh_lock);
        }
    }

    /* 2. DualSense slots live reconfigure */
    for (int i = 0; i < RH_MAX_PADS; i++) {
        RhDsSlot *slot = &rh_ds_slots[i];
        AcquireSRWLockShared(&slot->lock);
        int active = slot->active;
        int vol = slot->dsp_vol;
        int crisp = slot->dsp_crisp;
        ReleaseSRWLockShared(&slot->lock);
        if (!active) continue;

        if (!u.dualsense_haptics) {
            rh_ds_detach_slot(i, 0);
            continue;
        }

        if ((vol != u.dualsense_volume || crisp != u.dualsense_crisp_notes) && rh.ds_create && rh.ds_destroy) {
            RenHdcDualSenseDsp *new_dsp = rh.ds_create(u.dualsense_volume, u.dualsense_crisp_notes != 0);
            if (new_dsp) {
                RenHdcDualSenseDsp *old_dsp = new_dsp;
                AcquireSRWLockExclusive(&slot->lock);
                if (slot->active && slot->dsp) {
                    if (rh.ds_set_speaker && (slot->speaker_on || (slot->is_bt && slot->bt_report36))) {
                        rh.ds_set_speaker(new_dsp, RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT);
                    }
                    AcquireSRWLockExclusive(&slot->dsp_lock);
                    old_dsp = slot->dsp;
                    slot->dsp = new_dsp;
                    ReleaseSRWLockExclusive(&slot->dsp_lock);
                    slot->dsp_vol = u.dualsense_volume;
                    slot->dsp_crisp = u.dualsense_crisp_notes;
                }
                ReleaseSRWLockExclusive(&slot->lock);
                if (old_dsp) rh.ds_destroy(old_dsp);
            }
        }
        rh_ds_sync_hd_haptics_slot(i);
    }

    /* 3. Xbox live reconfigure */
    if (!u.xbox_enabled) {
        if (rh_xb_detach(NULL)) {
            /* Clear claim bit only on the Xbox pad if other pads exist */
            void *pads[RH_MAX_PADS];
            int n = 0;
            AcquireSRWLockShared(&rh_pads_lock);
            for (int i = 0; i < RH_MAX_PADS; i++) {
                if (rh_conn_pads[i] && rh_slot_claimed[i]) pads[n++] = rh_conn_pads[i];
            }
            ReleaseSRWLockShared(&rh_pads_lock);
            for (int i = 0; i < n; i++) {
                if (!rh_is_active(pads[i])) rh_pads_set_claimed(pads[i], 0);
            }
        }
    } else {
        AcquireSRWLockExclusive(&rh_xb_lock);
        if (rh_xb) {
            rh_xb_strength = u.xbox_strength;
            rh_xb_direct_mode = u.xbox_direct;
            if (u.xbox_direct && !rh_xb_direct && rh.xb_direct_create) {
                rh_xb_direct = rh.xb_direct_create();
            }
        }
        ReleaseSRWLockExclusive(&rh_xb_lock);
    }

    /* 4. Steam live reconfigure */
    int recreate_sv = 0;
    AcquireSRWLockShared(&rh_sv_lock);
    if (rh_sv_dsp && (rh_sv_vol != u.dualsense_volume || rh_sv_crisp != u.dualsense_crisp_notes)) {
        recreate_sv = 1;
    }
    ReleaseSRWLockShared(&rh_sv_lock);
    if (recreate_sv && (rh.sv_create_ex || rh.sv_create) && rh.ds_destroy) {
        RenHdcDualSenseDsp *new_sv = rh.sv_create_ex
            ? rh.sv_create_ex(u.dualsense_volume, u.dualsense_crisp_notes != 0, true)
            : rh.sv_create(u.dualsense_volume, u.dualsense_crisp_notes != 0);
        if (new_sv) {
            RenHdcDualSenseDsp *old_sv = NULL;
            AcquireSRWLockExclusive(&rh_sv_lock);
            if (rh_sv_dsp) {
                old_sv = rh_sv_dsp;
                rh_sv_dsp = new_sv;
                rh_sv_vol = u.dualsense_volume;
                rh_sv_crisp = u.dualsense_crisp_notes;
            } else {
                old_sv = new_sv;
            }
            ReleaseSRWLockExclusive(&rh_sv_lock);
            rh.ds_destroy(old_sv);
        }
    }

    /* 5. Re-attach any tracked unclaimed pads whose family switch was turned back on */
    if (u.enable_hd_rumble && InterlockedCompareExchange(&rh_enabled, 1, 1) == 1) {
        void *pads[RH_MAX_PADS];
        int n = 0;
        AcquireSRWLockShared(&rh_pads_lock);
        for (int i = 0; i < RH_MAX_PADS; i++) {
            if (rh_conn_pads[i] && !rh_slot_claimed[i] && !rh_slot_stand_in[i]) {
                pads[n++] = rh_conn_pads[i];
            }
        }
        ReleaseSRWLockShared(&rh_pads_lock);
        for (int i = 0; i < n; i++) {
            if (!rh_is_active(pads[i])) rh_attach(pads[i]);
        }
    }
}

/* ---- Public API ---- */
void rh_set_switch_gen(int gen) {
    LONG g = (gen == 1) ? 1 : 2;
    InterlockedExchange(&rh_switch_gen, g);
    if (rh.set_nx2_mode) {
        rh.set_nx2_mode(g == 1 ? 0 : 2);
    }
}

int rh_load(void) {
    AcquireSRWLockExclusive(&rh_init_lock);
    if (rh.tried) {
        int ok = (rh.create != NULL);
        ReleaseSRWLockExclusive(&rh_init_lock);
        return ok;
    }
    rh.tried = 1;
    rh_load_err[0] = '\0';

    if (rh_env_disabled()) {
        hos_log("renhdrum: off (env)");
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }

    WCHAR exe_path[MAX_PATH];
    DWORD exe_len = GetModuleFileNameW(NULL, exe_path, MAX_PATH);
    if (exe_len == 0 || exe_len >= MAX_PATH) {
        hos_log("renhdrum: off (cannot resolve exe path)");
        snprintf(rh_load_err, sizeof(rh_load_err), "Cannot locate renhdrum.dll: normal rumble");
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }
    const WCHAR *slash = NULL;
    for (DWORD i = 0; i < exe_len; i++) {
        if (exe_path[i] == L'\\' || exe_path[i] == L'/') slash = &exe_path[i];
    }
    if (!slash) {
        hos_log("renhdrum: off (cannot resolve exe directory)");
        snprintf(rh_load_err, sizeof(rh_load_err), "Cannot locate renhdrum.dll: normal rumble");
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }
    size_t dir_len = (size_t)(slash - exe_path) + 1;
    if (dir_len + 16 > MAX_PATH) {
        hos_log("renhdrum: off (dll path too long)");
        snprintf(rh_load_err, sizeof(rh_load_err), "renhdrum.dll path too long: normal rumble");
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }
    memcpy(rh_exe_dir, exe_path, dir_len * sizeof(WCHAR));
    rh_exe_dir[dir_len] = L'\0';
    rh_has_exe_dir = 1;

    WCHAR dll_path[MAX_PATH];
    rh_build_exe_sibling_path(L"renhdrum.dll", dll_path, MAX_PATH);

#ifndef LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
#define LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR 0x00000100
#endif
#ifndef LOAD_LIBRARY_SEARCH_SYSTEM32
#define LOAD_LIBRARY_SEARCH_SYSTEM32 0x00000800
#endif
    rh.dll = LoadLibraryExW(dll_path, NULL, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!rh.dll) {
        hos_log("renhdrum: off (renhdrum.dll not found beside exe)");
        snprintf(rh_load_err, sizeof(rh_load_err), "renhdrum.dll not found beside exe: normal rumble");
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }

    const char *why = NULL;
#define RH_REQ(field, name) do { rh_load_proc(&rh.field, rh.dll, name); if (!rh.field && !why) why = name; } while (0)
    RH_REQ(version, "renhdrum_version");
    RH_REQ(create, "renhdrum_engine_create");
    RH_REQ(destroy, "renhdrum_engine_destroy");
    RH_REQ(defaults, "renhdrum_settings_default");
    RH_REQ(apply, "renhdrum_engine_apply_settings");
    RH_REQ(push, "renhdrum_engine_push_half_sample");
    RH_REQ(tick, "renhdrum_engine_tick");
    RH_REQ(set_nx2_mode, "renhdrum_set_nx2_mode");
#undef RH_REQ

    if (why) {
        hos_log("renhdrum: off (%s missing)", why);
        snprintf(rh_load_err, sizeof(rh_load_err), "renhdrum.dll missing %s: normal rumble", why);
        FreeLibrary(rh.dll);
        memset(&rh, 0, sizeof(rh));
        rh.tried = 1;
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }

    uint32_t ver = rh.version();
    if (!RENHDRUM_ABI_COMPATIBLE(ver)) {
        hos_log("renhdrum: off (incompatible renhdrum.dll version 0x%06x, expected >= 0x%06x)",
                (unsigned)ver, (unsigned)RENHDRUM_VERSION);
        snprintf(rh_load_err, sizeof(rh_load_err),
                 "Incompatible renhdrum.dll (0x%06x, need >= 0x%06x): normal rumble",
                 (unsigned)ver, (unsigned)RENHDRUM_VERSION);
        FreeLibrary(rh.dll);
        memset(&rh, 0, sizeof(rh));
        rh.tried = 1;
        ReleaseSRWLockExclusive(&rh_init_lock);
        return 0;
    }

    rh.set_nx2_mode(InterlockedCompareExchange(&rh_switch_gen, 0, 0) == 1 ? 0 : 2);

    /* Optional settings / store exports */
    rh_load_proc(&rh.resolve_pure, rh.dll, "renhdrum_settings_resolve_pure");
    rh_load_proc(&rh.resolve, rh.dll, "renhdrum_settings_resolve");
    rh_load_proc(&rh.options, rh.dll, "renhdrum_options");
    rh_load_proc(&rh.store_create, rh.dll, "renhdrum_store_create");
    rh_load_proc(&rh.store_destroy, rh.dll, "renhdrum_store_destroy");
    rh_load_proc(&rh.store_get, rh.dll, "renhdrum_store_get");
    rh_load_proc(&rh.store_set, rh.dll, "renhdrum_store_set");
    rh_load_proc(&rh.store_reset, rh.dll, "renhdrum_store_reset");
    rh_load_proc(&rh.store_is_rec, rh.dll, "renhdrum_store_is_recommended");
    rh_load_proc(&rh.store_to_config, rh.dll, "renhdrum_store_to_config");
    rh_load_proc(&rh.store_shape, rh.dll, "renhdrum_store_shape_amplitude");

    /* Optional DualSense & HD Haptics exports */
    rh_load_proc(&rh.ds_create, rh.dll, "renhdrum_dualsense_dsp_create");
    rh_load_proc(&rh.ds_destroy, rh.dll, "renhdrum_dualsense_dsp_destroy");
    rh_load_proc(&rh.ds_set_half_target, rh.dll, "renhdrum_dualsense_dsp_set_half_target");
    rh_load_proc(&rh.ds_set_speaker, rh.dll, "renhdrum_dualsense_dsp_set_speaker");
    rh_load_proc(&rh.ds_fill_f32, rh.dll, "renhdrum_dualsense_dsp_fill_f32");
    rh_load_proc(&rh.ds_build_bt_report, rh.dll, "renhdrum_dualsense_dsp_build_bt_report");
    rh_load_proc(&rh.ds_build_bt36_report, rh.dll, "renhdrum_dualsense_dsp_build_bt36_report");
    rh_load_proc(&rh.tri_create, rh.dll, "renhdrum_tri_trigger_create");
    rh_load_proc(&rh.tri_destroy, rh.dll, "renhdrum_tri_trigger_destroy");
    rh_load_proc(&rh.tri_update, rh.dll, "renhdrum_tri_trigger_update");
    rh_load_proc(&rh.tri_note_action, rh.dll, "renhdrum_tri_trigger_note_action");
    rh_load_proc(&rh.preset_create, rh.dll, "renhdrum_trigger_preset_create");
    rh_load_proc(&rh.preset_destroy, rh.dll, "renhdrum_trigger_preset_destroy");
    rh_load_proc(&rh.preset_set, rh.dll, "renhdrum_trigger_preset_set");
    rh_load_proc(&rh.preset_tick, rh.dll, "renhdrum_trigger_preset_tick");
    if (rh_has_trigger_preset_api()) {
        InterlockedExchange(&rh_presets_ready, 1);
        hos_log("renhdrum: trigger presets on");
    }
    rh_load_proc(&rh.hd_speaker_block, rh.dll, "renhdrum_hd_haptics_speaker_block");
    rh_load_proc(&rh.hd_trigger_block, rh.dll, "renhdrum_hd_haptics_trigger_block");

    /* Optional Xbox One S & Direct exports */
    rh_load_proc(&rh.xb_create, rh.dll, "renhdrum_xbox_rumble_create");
    rh_load_proc(&rh.xb_destroy, rh.dll, "renhdrum_xbox_rumble_destroy");
    rh_load_proc(&rh.xb_push, rh.dll, "renhdrum_xbox_rumble_push_half_sample");
    rh_load_proc(&rh.xb_tick, rh.dll, "renhdrum_xbox_rumble_tick");
    rh_load_proc(&rh.xb_resend, rh.dll, "renhdrum_xbox_rumble_resend");
    rh_load_proc(&rh.xb_direct_create, rh.dll, "renhdrum_xbox_direct_create");
    rh_load_proc(&rh.xb_direct_destroy, rh.dll, "renhdrum_xbox_direct_destroy");
    rh_load_proc(&rh.xb_direct_push, rh.dll, "renhdrum_xbox_direct_push");
    rh_load_proc(&rh.xb_direct_tick, rh.dll, "renhdrum_xbox_direct_tick");

    /* Optional Steam Controller 2026 exports */
    rh_load_proc(&rh.sv_create, rh.dll, "renhdrum_valve_grip_dsp_create");
    rh_load_proc(&rh.sv_create_ex, rh.dll, "renhdrum_valve_grip_dsp_create_ex");
    rh_load_proc(&rh.sv_build, rh.dll, "renhdrum_valve_grip_build_report");
    rh_load_proc(&rh.sv_mode_tick, rh.dll, "renhdrum_valve_grip_mode_tick");
    rh_load_proc(&rh.sv_mode_off, rh.dll, "renhdrum_valve_grip_mode_off");

    /* Optional Switch 2 Pro Controller exports */
    rh_load_proc(&rh.pro2_create, rh.dll, "renhdrum_pro2_create");
    rh_load_proc(&rh.pro2_destroy, rh.dll, "renhdrum_pro2_destroy");
    rh_load_proc(&rh.pro2_push, rh.dll, "renhdrum_pro2_push_half_sample");
    rh_load_proc(&rh.pro2_build, rh.dll, "renhdrum_pro2_build_report");
    rh_load_proc(&rh.pro2_silent, rh.dll, "renhdrum_pro2_silent_report");

    int ini_enable = 1;
    AcquireSRWLockExclusive(&rh_store_lock);
    if (rh.options) {
        rh_opts = rh.options(&rh_opt_count, &rh_opt_stride);
    }
    if (rh.store_create && !rh_store) {
        rh_store = rh.store_create();
        rh_ini_load_locked();
        ini_enable = rh_store_get_locked("enable_hd_rumble", 1);
    }
    ReleaseSRWLockExclusive(&rh_store_lock);

    if (InterlockedCompareExchange(&rh_enabled_host_set, 0, 0) == 0) {
        InterlockedExchange(&rh_enabled, ini_enable ? 1 : 0);
    }

    ReleaseSRWLockExclusive(&rh_init_lock);
    return 1;
}

void rh_attach(void *pad) {
    if (!pad) return;
    init_sdl();
    uint16_t vid = sdl.get_vid ? sdl.get_vid(pad) : 0;
    uint16_t pid = sdl.get_pid ? sdl.get_pid(pad) : 0;
    void *early_joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
    int is_virtual = 0;
    if (sdl.is_sdl3 && sdl.is_joy_virtual && sdl.get_joy_id && early_joy) {
        uint32_t jid = sdl.get_joy_id(early_joy);
        if (jid != 0) is_virtual = sdl.is_joy_virtual(jid) ? 1 : 0;
    }
    /* Ignore SDL virtual joysticks except a Switch 2 Pro pad (057E:2069). */
    if (is_virtual && !(vid == 0x057E && pid == 0x2069)) {
        return;
    }
    rh_pads_track(pad);
    if (rh_env_disabled()) return;
    if (InterlockedCompareExchange(&rh_enabled, 1, 1) == 0) return;
    if (!sdl.get_vid || !sdl.get_pid) return;

    /* Steam Input stand-in pad: never claimed; stops direct HID grips so Steam Input takes over */
    if (sdl.get_steam_handle && sdl.get_steam_handle(pad) != 0) {
        rh_pads_mark_stand_in(pad);
        void *old = rh_sv_stop(NULL);
        if (old) {
            rh_pads_set_claimed(old, 0);
            hos_log("renhdrum: steam stand-in pad opened, Steam controller grips go back to Steam Input");
        }
        return;
    }

    int bt = 0;
    if (sdl.is_sdl3 && sdl.get_conn) {
        bt = (sdl.get_conn(pad) == 2 /* SDL_JOYSTICK_CONNECTION_WIRELESS */);
    } else if (!sdl.is_sdl3) {
        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
        if (joy && sdl.get_power) {
            int p = sdl.get_power(joy);
            if (p >= 0 && p != 4 /* SDL_JOYSTICK_POWER_WIRED */) bt = 1;
        }
    }

    /* 1. Nintendo Switch Pro Controller (VID 0x057E, PID 0x2009) */
    if (vid == 0x057E && pid == 0x2009) {
        if (!rh_load()) return;

        AcquireSRWLockShared(&rh_lock);
        int pro_busy = (rh_engine != NULL);
        ReleaseSRWLockShared(&rh_lock);
        if (pro_busy) return;

        RenHdcEngine *e = rh.create(bt != 0, true);
        if (!e) {
            hos_log("renhdrum: off (engine create failed)");
            return;
        }
        RenHdcSettings s;
        if (!rh_resolve_pro_settings(&s, bt)) {
            rh.destroy(e);
            hos_log("renhdrum: off (defaults failed)");
            return;
        }
        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
        static const uint8_t stop_pkt[10] = {0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40};
        if (joy && sdl.send_effect) sdl.send_effect(joy, stop_pkt, sizeof(stop_pkt));
        rh_apply_pro_transport(joy, &s, bt);
        rh.apply(e, &s);

        AcquireSRWLockExclusive(&rh_lock);
        if (rh_engine) {
            ReleaseSRWLockExclusive(&rh_lock);
            rh_clear_pro_transport(joy);
            rh.destroy(e);
            return;
        }
        rh_engine = e;
        rh_active_pad = pad;
        rh_joy = joy;
        rh_is_bt = bt ? 1 : 0;
        rh_usb_no_queue = (!bt && s.usb_no_queue) ? 1 : 0;
        memset(rh_last_pkt, 0, sizeof(rh_last_pkt));
        rh_has_last_pkt = 0;
        rh_last_send_us = 0;
        rh_at[0] = rh_at[1] = 0;
        rh_muted[0] = rh_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_lock);

        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        if (bt) {
            if (s.bt_fast) hos_log("renhdrum: on (bluetooth detailed 5ms)");
            else if (s.bt_after_input) hos_log("renhdrum: on (bluetooth compatible, after-input)");
            else hos_log("renhdrum: on (bluetooth level %d)", s.bluetooth_level);
        } else {
            if (s.usb_no_queue && s.envelope_5ms) hos_log("renhdrum: on (usb no-queue 5ms)");
            else if (s.usb_no_queue) hos_log("renhdrum: on (usb no-queue)");
            else hos_log("renhdrum: on (usb)");
        }
        return;
    }

    /* 1b. Nintendo Switch 2 Pro Controller (VID 0x057E, PID 0x2069, experimental) */
    if (vid == 0x057E && pid == 0x2069) {
        if (!rh_load()) return;
        if (!rh.pro2_create || !rh.pro2_destroy || !rh.pro2_push || !rh.pro2_build || !rh.pro2_silent) {
            hos_log("renhdrum: Switch 2 Pro Controller functions missing in renhdrum.dll, using normal rumble");
            return;
        }
        if (!sdl.send_effect) {
            hos_log("renhdrum: Switch 2 Pro Controller: SDL_SendJoystickEffect missing, using normal rumble");
            return;
        }

        AcquireSRWLockShared(&rh_pro2_lock);
        int pro2_busy = (rh_pro2 != NULL);
        ReleaseSRWLockShared(&rh_pro2_lock);
        if (pro2_busy) return;

        RenHdcPro2 *h = rh.pro2_create(0);
        if (!h) {
            hos_log("renhdrum: off (switch2 pro2 create failed)");
            return;
        }
        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
        uint8_t probe[RENHDRUM_PRO2_REPORT_SIZE];
        if (!joy || rh.pro2_silent(h, probe) != RENHDRUM_OK ||
            !sdl_ok(sdl.send_effect(joy, probe, RENHDRUM_PRO2_REPORT_SIZE))) {
            rh.pro2_destroy(h);
            hos_log("renhdrum: Switch 2 Pro Controller refused raw HD Rumble 2 report, using normal rumble");
            return;
        }

        AcquireSRWLockExclusive(&rh_pro2_lock);
        if (rh_pro2) {
            ReleaseSRWLockExclusive(&rh_pro2_lock);
            rh.pro2_destroy(h);
            return;
        }
        rh_pro2 = h;
        rh_pro2_pad = pad;
        rh_pro2_joy = joy;
        rh_pro2_at[0] = rh_pro2_at[1] = 0;
        rh_pro2_muted[0] = rh_pro2_muted[1] = 0;
        rh_pro2_fails = 0;
        ReleaseSRWLockExclusive(&rh_pro2_lock);

        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        hos_log("renhdrum: Switch 2 Pro Controller (USB, experimental)");
        return;
    }

    /* 2. Sony DualSense / DualSense Edge (VID 0x054C, PID 0x0CE6 or 0x0DF2) */
    if (vid == 0x054C && (pid == 0x0CE6 || pid == 0x0DF2)) {
        if (!rh_load()) return;
        RhUserOpts u = rh_read_opts();
        if (!u.dualsense_haptics) return;
        if (!rh.ds_create || !rh.ds_fill_f32) {
            hos_log("renhdrum: off (dualsense dsp functions missing in renhdrum.dll)");
            return;
        }

        int free_slot = -1;
        for (int i = 0; i < RH_MAX_PADS; i++) {
            AcquireSRWLockShared(&rh_ds_slots[i].lock);
            if (rh_ds_slots[i].active && rh_ds_slots[i].pad == pad) {
                ReleaseSRWLockShared(&rh_ds_slots[i].lock);
                return;
            }
            if (!rh_ds_slots[i].active && free_slot < 0) free_slot = i;
            ReleaseSRWLockShared(&rh_ds_slots[i].lock);
        }
        if (free_slot < 0) return;

        RhDsSlot *slot = &rh_ds_slots[free_slot];
        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;

        if (bt) {
            if (!rh.ds_build_bt_report && !rh.ds_build_bt36_report) {
                hos_log("renhdrum: off (dualsense bt report function missing in renhdrum.dll)");
                return;
            }
            const char *path = rh_pad_hid_path(pad);
            char hid_copy[512];
            hid_copy[0] = '\0';
            if (path && *path) {
                snprintf(hid_copy, sizeof(hid_copy), "%s", path);
            }
            void *hid_dev = rh_open_hid_by_path_or_enum(hid_copy, vid, pid);
            if (!hid_dev) {
                hos_log("renhdrum: dualsense bt haptics: no HID path or open failed, using standard rumble");
                return;
            }

            RenHdcDualSenseDsp *dsp = rh.ds_create(u.dualsense_volume, u.dualsense_crisp_notes != 0);
            if (!dsp) {
                if (sdl.hid_close) sdl.hid_close(hid_dev);
                hos_log("renhdrum: off (dualsense dsp create failed)");
                return;
            }

            int64_t now_us = hos_now_ns() / 1000;
            AcquireSRWLockExclusive(&slot->lock);
            if (slot->active) {
                ReleaseSRWLockExclusive(&slot->lock);
                if (sdl.hid_close) sdl.hid_close(hid_dev);
                if (rh.ds_destroy) rh.ds_destroy(dsp);
                return;
            }
            slot->active = 1;
            slot->pad = pad;
            slot->joy = joy;
            slot->vid = vid;
            slot->pid = pid;
            slot->is_bt = 1;
            memcpy(slot->hid_path, hid_copy, sizeof(slot->hid_path));
            slot->dsp = dsp;
            slot->dsp_vol = u.dualsense_volume;
            slot->dsp_crisp = u.dualsense_crisp_notes;
            slot->bt_hid = hid_dev;
            slot->bt_unsupported = 0;
            slot->bt_starting = 0;
            slot->bt_next_retry_us = now_us + 10000000LL;
            slot->speaker_on = 0;
            slot->speaker_logged = 0;
            slot->trigger_logged = 0;
            slot->bt_report36 = 0;
            slot->preset[0] = slot->preset[1] = NULL;
            slot->preset_gen[0] = slot->preset_gen[1] = 0;
            slot->preset_active[0] = slot->preset_active[1] = 0;
            slot->pulse_until_ms[0] = slot->pulse_until_ms[1] = 0;
            slot->tri_last_valid[0] = slot->tri_last_valid[1] = 0;
            memset(slot->tri_last, 0, sizeof(slot->tri_last));
            slot->tri_last[0][0] = 0x05;
            slot->tri_last[1][0] = 0x05;
            slot->at[0] = slot->at[1] = 0;
            slot->muted[0] = slot->muted[1] = 0;
            memset(slot->bt_effect_set, 0, sizeof(slot->bt_effect_set));
            InterlockedExchange(&slot->bt_warned, 0);
            ReleaseSRWLockExclusive(&slot->lock);

            if (sdl.rumble_pad) sdl.rumble_pad(pad, 0, 0, 0);
            rh_ds_sync_hd_haptics_slot(free_slot);

            AcquireSRWLockExclusive(&slot->lock);
            InterlockedExchange(&slot->bt_run, 1);
            slot->bt_thread = CreateThread(NULL, 0, rh_ds_bt_worker, (void *)(intptr_t)free_slot, 0, NULL);
            int started = (slot->bt_thread != NULL);
            ReleaseSRWLockExclusive(&slot->lock);

            if (!started) {
                InterlockedExchange(&slot->bt_run, 0);
                hos_log("renhdrum: off (cannot create dualsense bt worker thread)");
                rh_ds_detach_slot(free_slot, 0);
                return;
            }

            rh_pads_set_claimed(pad, 1);
            rh_ensure_worker();
            hos_log("renhdrum: on (dualsense bluetooth)");
            return;
        }

        RenHdcDualSenseDsp *dsp = rh.ds_create(u.dualsense_volume, u.dualsense_crisp_notes != 0);
        if (!dsp) {
            hos_log("renhdrum: off (dualsense dsp create failed)");
            return;
        }

        AcquireSRWLockExclusive(&slot->lock);
        if (slot->active) {
            ReleaseSRWLockExclusive(&slot->lock);
            if (rh.ds_destroy) rh.ds_destroy(dsp);
            return;
        }
        slot->active = 1;
        slot->pad = pad;
        slot->joy = joy;
        slot->vid = vid;
        slot->pid = pid;
        slot->is_bt = 0;
        slot->hid_path[0] = '\0';
        slot->dsp = dsp;
        slot->dsp_vol = u.dualsense_volume;
        slot->dsp_crisp = u.dualsense_crisp_notes;
        slot->audio_stream = NULL;
        slot->audio_device = 0;
        slot->dev2 = 0;
        slot->audio_attempts = 0;
        slot->next_audio_retry_us = 0;
        slot->next_audio_check_us = 0;
        slot->speaker_on = 0;
        slot->speaker_logged = 0;
        slot->trigger_logged = 0;
        slot->bt_report36 = 0;
        slot->preset[0] = slot->preset[1] = NULL;
        slot->preset_gen[0] = slot->preset_gen[1] = 0;
        slot->preset_active[0] = slot->preset_active[1] = 0;
        slot->pulse_until_ms[0] = slot->pulse_until_ms[1] = 0;
        slot->tri_last_valid[0] = slot->tri_last_valid[1] = 0;
        memset(slot->tri_last, 0, sizeof(slot->tri_last));
        slot->tri_last[0][0] = 0x05;
        slot->tri_last[1][0] = 0x05;
        slot->at[0] = slot->at[1] = 0;
        slot->muted[0] = slot->muted[1] = 0;
        ReleaseSRWLockExclusive(&slot->lock);

        rh_ensure_worker();
        hos_log("renhdrum: on (dualsense usb)");
        rh_ds_try_open_audio(free_slot);
        return;
    }

    /* 3. Steam Controller 2026 (VID 0x28DE, PID 1302 wired / 1303 BT / 1304 dongle) */
    if (vid == 0x28DE && (pid == 0x1302 || pid == 0x1303 || pid == 0x1304)) {
        if (rh_stand_in_tracked()) {
            hos_log("renhdrum: steam controller: Steam Input's pad is open, its rumble stays with Steam");
            return;
        }
        if (!rh_load()) return;
        if ((!rh.sv_create_ex && !rh.sv_create) || !rh.sv_build || !rh.ds_set_half_target || !rh.ds_destroy) {
            hos_log("renhdrum: steam controller: valve grip functions missing in renhdrum.dll, using standard rumble");
            return;
        }
        if (!sdl.hid_open_path || !sdl.hid_write) return;

        AcquireSRWLockShared(&rh_sv_lock);
        int sv_busy = (rh_sv_dsp != NULL);
        ReleaseSRWLockShared(&rh_sv_lock);
        if (sv_busy) return;

        const char *path = rh_pad_hid_path(pad);
        void *hid = (path && *path) ? sdl.hid_open_path(path) : NULL;
        if (!hid) {
            hos_log("renhdrum: steam controller: no HID path or open failed, using standard rumble");
            return;
        }

        RhUserOpts u = rh_read_opts();
        RenHdcDualSenseDsp *dsp = rh.sv_create_ex
            ? rh.sv_create_ex(u.dualsense_volume, u.dualsense_crisp_notes != 0, true)
            : rh.sv_create(u.dualsense_volume, u.dualsense_crisp_notes != 0);
        if (!dsp) {
            if (sdl.hid_close) sdl.hid_close(hid);
            hos_log("renhdrum: off (steam grip dsp create failed)");
            return;
        }

        uint8_t fmt = pid == 0x1302 ? RENHDRUM_VALVE_GRIP_FORMAT_MULAW_8K : RENHDRUM_VALVE_GRIP_FORMAT_MULAW_4K;
        if (rh.sv_mode_tick && rh.sv_mode_off) {
            rh_sv_send_mode_off(hid, fmt);
        }

        AcquireSRWLockExclusive(&rh_sv_lock);
        if (rh_sv_dsp) {
            ReleaseSRWLockExclusive(&rh_sv_lock);
            if (sdl.hid_close) sdl.hid_close(hid);
            rh.ds_destroy(dsp);
            return;
        }
        rh_sv_dsp = dsp;
        rh_sv_pad = pad;
        rh_sv_hid = hid;
        rh_sv_format = fmt;
        rh_sv_vol = u.dualsense_volume;
        rh_sv_crisp = u.dualsense_crisp_notes;
        rh_sv_at[0] = rh_sv_at[1] = 0;
        rh_sv_muted[0] = rh_sv_muted[1] = 0;
        InterlockedExchange(&rh_sv_run, 1);
        rh_sv_thread = CreateThread(NULL, 0, rh_sv_worker, NULL, 0, NULL);
        int started = (rh_sv_thread != NULL);
        ReleaseSRWLockExclusive(&rh_sv_lock);

        if (!started) {
            hos_log("renhdrum: off (cannot create steam grips thread)");
            rh_sv_stop(pad);
            return;
        }
        if (sdl.rumble_pad) sdl.rumble_pad(pad, 0, 0, 0);
        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        hos_log("renhdrum: on (steam controller grips, experimental, %s kHz)", pid == 0x1302 ? "8" : "4");
        return;
    }

    /* 4. Xbox One S: SDL type XBOXONE or Microsoft VID 0x045E */
    const int xb_type = sdl.get_type ? sdl.get_type(pad) : 0;
    if (xb_type == (sdl.is_sdl3 ? 3 : 2) || vid == 0x045E) {
        if (!rh_load()) return;
        RhUserOpts u = rh_read_opts();
        if (!u.xbox_enabled) return;
        if (!rh.xb_create || !rh.xb_destroy || !rh.xb_push || !rh.xb_tick) {
            hos_log("renhdrum: xbox: rumble functions missing in renhdrum.dll, using standard rumble");
            return;
        }
        if (!sdl.rumble_joy) {
            hos_log("renhdrum: xbox: this SDL has no joystick rumble call, using standard rumble");
            return;
        }
        AcquireSRWLockShared(&rh_xb_lock);
        int xb_busy = (rh_xb != NULL);
        ReleaseSRWLockShared(&rh_xb_lock);
        if (xb_busy) return;

        const int xb_bt = (sdl.is_sdl3 && sdl.get_conn) ? (sdl.get_conn(pad) != 1) : bt;
        RenHdcXboxRumble *x = rh.xb_create(xb_bt != 0);
        if (!x) {
            hos_log("renhdrum: off (xbox rumble create failed)");
            return;
        }
        RenHdcXboxDirect *xd = (u.xbox_direct && rh.xb_direct_create) ? rh.xb_direct_create() : NULL;
        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
        int has_trig = (sdl.rumble_joy_trig != NULL);
        if (has_trig && sdl.is_sdl3 && sdl.get_joy_props && sdl.get_bool_prop && joy) {
            has_trig = sdl.get_bool_prop(sdl.get_joy_props(joy), "SDL.joystick.cap.trigger_rumble", 0) != 0;
        }

        AcquireSRWLockExclusive(&rh_xb_lock);
        if (rh_xb) {
            ReleaseSRWLockExclusive(&rh_xb_lock);
            if (xd && rh.xb_direct_destroy) rh.xb_direct_destroy(xd);
            rh.xb_destroy(x);
            return;
        }
        rh_xb = x;
        rh_xb_direct = xd;
        rh_xb_pad = pad;
        rh_xb_joy = joy;
        rh_xb_triggers = has_trig;
        rh_xb_strength = u.xbox_strength;
        rh_xb_direct_mode = u.xbox_direct;
        rh_xb_at[0] = rh_xb_at[1] = 0;
        rh_xb_muted[0] = rh_xb_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_xb_lock);

        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        hos_log("renhdrum: on (xbox %s)", xb_bt ? "bluetooth" : "usb");
    }
}

int rh_detach(void *pad) {
    return rh_detach_engines(pad, 1);
}

void rh_shutdown(void) {
    rh_detach_engines(NULL, 1);
}

void rh_cleanup(void) {
    rh_shutdown();
}

void rh_set_enabled(int on) {
    InterlockedExchange(&rh_enabled_host_set, 1);
    LONG next = on ? 1 : 0;
    LONG prev = InterlockedExchange(&rh_enabled, next);
    if (prev == next) return;

    AcquireSRWLockExclusive(&rh_store_lock);
    if (rh_store && rh.store_set) {
        rh.store_set(rh_store, "enable_hd_rumble", next ? 1 : 0);
    }
    ReleaseSRWLockExclusive(&rh_store_lock);

    if (!next) {
        rh_detach_engines(NULL, 0);
        return;
    }
    void *pads[RH_MAX_PADS];
    int n = 0;
    AcquireSRWLockShared(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) {
        if (rh_conn_pads[i]) pads[n++] = rh_conn_pads[i];
    }
    ReleaseSRWLockShared(&rh_pads_lock);
    for (int i = 0; i < n; i++) rh_attach(pads[i]);
}

const char *rh_status_line(void) {
    if (InterlockedCompareExchange(&rh_enabled, 1, 1) == 0) {
        return "HD Rumble is off: using normal rumble";
    }
    if (rh_load_err[0] != '\0') {
        return rh_load_err;
    }

    AcquireSRWLockShared(&rh_lock);
    if (rh_engine) {
        int bt = rh_is_bt;
        ReleaseSRWLockShared(&rh_lock);
        return bt ? "Connected: Pro Controller (Bluetooth), HD Rumble"
                  : "Connected: Pro Controller (USB), HD Rumble";
    }
    ReleaseSRWLockShared(&rh_lock);

    AcquireSRWLockShared(&rh_pro2_lock);
    if (rh_pro2) {
        ReleaseSRWLockShared(&rh_pro2_lock);
        return "Connected: Switch 2 Pro Controller (USB, experimental), HD Rumble";
    }
    ReleaseSRWLockShared(&rh_pro2_lock);

    for (int i = 0; i < RH_MAX_PADS; i++) {
        RhDsSlot *slot = &rh_ds_slots[i];
        AcquireSRWLockShared(&slot->lock);
        if (slot->active && slot->dsp) {
            int bt = slot->is_bt;
            int ready = bt ? (InterlockedCompareExchange(&slot->bt_run, 1, 1) == 1)
                           : (slot->audio_stream != NULL || slot->dev2 != 0);
            ReleaseSRWLockShared(&slot->lock);
            if (ready) {
                return bt ? "Connected: DualSense (Bluetooth), HD Rumble"
                          : "Connected: DualSense (USB), HD Rumble";
            }
        } else {
            ReleaseSRWLockShared(&slot->lock);
        }
    }

    AcquireSRWLockShared(&rh_xb_lock);
    if (rh_xb) {
        ReleaseSRWLockShared(&rh_xb_lock);
        return "Connected: Xbox One S, HD Rumble (4 motors)";
    }
    ReleaseSRWLockShared(&rh_xb_lock);

    AcquireSRWLockShared(&rh_sv_lock);
    if (rh_sv_dsp && InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) {
        ReleaseSRWLockShared(&rh_sv_lock);
        return "Steam Controller: HD Rumble (experimental)";
    }
    ReleaseSRWLockShared(&rh_sv_lock);

    return "No controller HD Rumble supports: normal rumble";
}

/* The game's last rumble amplitude per side (x1000) and when (hos_now_ns() / 1000), for rh_game_rumble_amp (no lock). */
static volatile LONG rh_game_amp_milli[2];
static volatile LONG64 rh_game_amp_us[2];

float rh_game_rumble_amp(void) {
    const int64_t now_us = hos_now_ns() / 1000;
    LONG best = 0;
    for (int s = 0; s < 2; s++) {
        const LONG64 at = InterlockedCompareExchange64(&rh_game_amp_us[s], 0, 0);
        if (at <= 0 || now_us - at > 60000) continue;
        const LONG a = InterlockedCompareExchange(&rh_game_amp_milli[s], 0, 0);
        if (a > best) best = a;
    }
    return (float)best / 1000.0f;
}

int rh_vibration(int side, float al, float fl, float ah, float fh) {
    if (side < 0 || side > 1) return 0;
    if (InterlockedCompareExchange(&rh_enabled, 1, 1) == 0) return 0;
    int handled = 0;
    int64_t now_us = hos_now_ns() / 1000;
    float s_al = rh_shape_amp(al);
    float s_ah = rh_shape_amp(ah);

    /* Pro Controller */
    AcquireSRWLockExclusive(&rh_lock);
    if (rh_engine) {
        rh.push(rh_engine, now_us, side == 1, s_al, fl, s_ah, fh);
        rh_at[side] = now_us;
        rh_muted[side] = 0;
        handled = 1;
    }
    ReleaseSRWLockExclusive(&rh_lock);

    /* Switch 2 Pro Controller (experimental) */
    AcquireSRWLockExclusive(&rh_pro2_lock);
    if (rh_pro2 && rh.pro2_push) {
        rh.pro2_push(rh_pro2, now_us, side == 1, s_al, fl, s_ah, fh);
        rh_pro2_at[side] = now_us;
        rh_pro2_muted[side] = 0;
        handled = 1;
    }
    ReleaseSRWLockExclusive(&rh_pro2_lock);

    /* DualSense slots (DSP + reactive trigger sample; trigger packets go out from rh_worker only) */
    for (int i = 0; i < RH_MAX_PADS; i++) {
        RhDsSlot *slot = &rh_ds_slots[i];
        AcquireSRWLockExclusive(&slot->lock);
        if (slot->active && slot->dsp) {
            int ready = slot->is_bt
                ? (InterlockedCompareExchange(&slot->bt_run, 1, 1) == 1)
                : (slot->audio_stream != NULL || slot->dev2 != 0);
            if (ready) {
                if (rh.ds_set_half_target) {
                    AcquireSRWLockExclusive(&slot->dsp_lock);
                    if (slot->dsp) {
                        rh.ds_set_half_target(slot->dsp, side == 1, s_al, fl, s_ah, fh);
                    }
                    ReleaseSRWLockExclusive(&slot->dsp_lock);
                }
                if (slot->tri[side]) {
                    slot->tri_sample[side][0] = al;
                    slot->tri_sample[side][1] = fl;
                    slot->tri_sample[side][2] = ah;
                    slot->tri_sample[side][3] = fh;
                    slot->tri_fresh[side] = 1;
                    {
                        float m = al > ah ? al : ah;
                        m = m < 0.0f ? 0.0f : (m > 1.0f ? 1.0f : m);
                        InterlockedExchange(&rh_game_amp_milli[side], (LONG)(m * 1000.0f + 0.5f));
                        InterlockedExchange64(&rh_game_amp_us[side], now_us);
                    }
                }
                slot->at[side] = now_us;
                slot->muted[side] = 0;
                handled = 1;
            }
        }
        ReleaseSRWLockExclusive(&slot->lock);
    }

    /* Xbox One S */
    uint16_t xb_motors[4] = {0, 0, 0, 0};
    int xb_direct_mask = 0;
    void *xb_target = NULL;
    int xb_trig = 0;
    AcquireSRWLockExclusive(&rh_xb_lock);
    if (rh_xb) {
        if (rh_xb_direct_mode && rh_xb_direct && rh.xb_direct_push) {
            AcquireSRWLockShared(&rh_store_lock);
            if (rh_store) {
                xb_direct_mask = rh.xb_direct_push(rh_xb_direct, rh_store, now_us, side == 1,
                                                   al, ah, rh_xb_triggers != 0, xb_motors);
            }
            ReleaseSRWLockShared(&rh_store_lock);
            xb_target = rh_xb_joy;
            xb_trig = rh_xb_triggers;
            rh_xb_at[side] = now_us;
            rh_xb_muted[side] = 0;
            handled = 1;
        } else {
            rh.xb_push(rh_xb, now_us, side == 1, s_al, fl, s_ah, fh);
            rh_xb_at[side] = now_us;
            rh_xb_muted[side] = 0;
            handled = 1;
        }
    }
    ReleaseSRWLockExclusive(&rh_xb_lock);
    if (xb_direct_mask > 0 && xb_target) {
        if ((xb_direct_mask & 1) && sdl.rumble_joy) {
            sdl.rumble_joy(xb_target, xb_motors[0], xb_motors[1], 2000);
        }
        if ((xb_direct_mask & 2) && xb_trig && sdl.rumble_joy_trig) {
            sdl.rumble_joy_trig(xb_target, xb_motors[2], xb_motors[3], 2000);
        }
    }

    /* Steam Controller 2026 grips */
    AcquireSRWLockExclusive(&rh_sv_lock);
    if (rh_sv_dsp && InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1 && rh.ds_set_half_target) {
        rh.ds_set_half_target(rh_sv_dsp, side == 1, s_al, fl, s_ah, fh);
        rh_sv_at[side] = now_us;
        rh_sv_muted[side] = 0;
        handled = 1;
    }
    ReleaseSRWLockExclusive(&rh_sv_lock);

    return (handled && rh_all_pads_claimed()) ? 1 : 0;
}

void rh_permit(int on) {
    if (!on) {
        rh_vibration(0, 0.0f, 160.0f, 0.0f, 320.0f);
        rh_vibration(1, 0.0f, 160.0f, 0.0f, 320.0f);
    }
}

int rh_is_active(void *pad) {
    if (!pad) return 0;

    AcquireSRWLockShared(&rh_lock);
    if (rh_engine && rh_active_pad == pad) {
        ReleaseSRWLockShared(&rh_lock);
        return 1;
    }
    ReleaseSRWLockShared(&rh_lock);

    AcquireSRWLockShared(&rh_pro2_lock);
    if (rh_pro2 && rh_pro2_pad == pad) {
        ReleaseSRWLockShared(&rh_pro2_lock);
        return 1;
    }
    ReleaseSRWLockShared(&rh_pro2_lock);

    for (int i = 0; i < RH_MAX_PADS; i++) {
        RhDsSlot *slot = &rh_ds_slots[i];
        AcquireSRWLockShared(&slot->lock);
        if (slot->active && slot->dsp && slot->pad == pad) {
            int ok = slot->is_bt ? (InterlockedCompareExchange(&slot->bt_run, 1, 1) == 1) : 1;
            ReleaseSRWLockShared(&slot->lock);
            if (ok) return 1;
        } else {
            ReleaseSRWLockShared(&slot->lock);
        }
    }

    AcquireSRWLockShared(&rh_xb_lock);
    if (rh_xb && rh_xb_pad == pad) {
        ReleaseSRWLockShared(&rh_xb_lock);
        return 1;
    }
    ReleaseSRWLockShared(&rh_xb_lock);

    AcquireSRWLockShared(&rh_sv_lock);
    if (rh_sv_dsp && rh_sv_pad == pad && InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) {
        ReleaseSRWLockShared(&rh_sv_lock);
        return 1;
    }
    ReleaseSRWLockShared(&rh_sv_lock);

    return 0;
}

/* ---- Core settings table & INI persistence API ---- */
int rh_option_count(void) {
    rh_load();
    AcquireSRWLockShared(&rh_store_lock);
    int c = (rh_opts && rh_opt_count > 0 && rh_opt_stride >= (int32_t)sizeof(RenHdcOption))
        ? (int)rh_opt_count
        : 0;
    ReleaseSRWLockShared(&rh_store_lock);
    return c;
}

const RenHdcOption *rh_option_at(int i) {
    rh_load();
    AcquireSRWLockShared(&rh_store_lock);
    const RenHdcOption *opt = NULL;
    if (i >= 0 && i < rh_opt_count && rh_opts && rh_opt_stride >= (int32_t)sizeof(RenHdcOption)) {
        opt = (const RenHdcOption *)((const uint8_t *)rh_opts + (size_t)i * (size_t)rh_opt_stride);
    }
    ReleaseSRWLockShared(&rh_store_lock);
    return opt;
}

int rh_option_get(const char *key, int *out) {
    if (!key || !out) return RENHDRUM_ERR_NULL_PTR;
    rh_load();
    int32_t v = 0;
    int32_t res = RENHDRUM_ERR_INVALID_ARG;
    AcquireSRWLockShared(&rh_store_lock);
    if (rh_store && rh.store_get) {
        res = rh.store_get(rh_store, key, &v);
    }
    ReleaseSRWLockShared(&rh_store_lock);
    if (res == RENHDRUM_OK) *out = (int)v;
    return (int)res;
}

int rh_option_set(const char *key, int value) {
    if (!key) return RENHDRUM_ERR_NULL_PTR;
    rh_load();
    int32_t res = RENHDRUM_ERR_INVALID_ARG;
    AcquireSRWLockExclusive(&rh_store_lock);
    if (rh_store && rh.store_set) {
        res = rh.store_set(rh_store, key, (int32_t)value);
    }
    ReleaseSRWLockExclusive(&rh_store_lock);
    if (res != RENHDRUM_OK) return (int)res;

    if (strcmp(key, "enable_hd_rumble") == 0) {
        rh_set_enabled(value != 0);
    }
    rh_apply_settings_live();
    rh_ini_save();
    return RENHDRUM_OK;
}

void rh_trigger_preset(int side, uint8_t id, float level) {
    if ((side != 0 && side != 1) || InterlockedCompareExchange(&rh_presets_ready, 0, 0) == 0) return;
    int lvl_milli = 0;
    if (level > 0.0f) {
        lvl_milli = (level >= 1.0f) ? 1000 : (int)(level * 1000.0f + 0.5f);
    }
    InterlockedExchange(&rh_preset_packed[side], ((LONG)id << 16) | (LONG)lvl_milli);
    InterlockedIncrement(&rh_preset_gen[side]);
}

void rh_option_reset(void) {
    rh_load();
    int32_t res = RENHDRUM_ERR_INVALID_ARG;
    AcquireSRWLockExclusive(&rh_store_lock);
    if (rh_store && rh.store_reset) {
        res = rh.store_reset(rh_store);
    }
    ReleaseSRWLockExclusive(&rh_store_lock);
    if (res != RENHDRUM_OK) return;

    rh_set_enabled(1);
    rh_apply_settings_live();
    rh_ini_save();
}
