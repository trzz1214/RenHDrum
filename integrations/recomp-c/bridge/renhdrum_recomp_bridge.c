/* host/rh_bridge.c - RenHDrum bridge for Hookshot
   Authentic HD Rumble for Nintendo Switch Pro Controller (USB & Bluetooth), DualSense 4-channel 48 kHz audio
   haptics, Xbox One S (4 motors) and Steam Controller 2026 grips (experimental) via renhdrum.dll.
   Loaded dynamically at run time.
   Co-Authored-By: Claude Sonnet 5.5 + Gemini 3.8 Flash (AGY) */
#include "rh_bridge.h"
#include "renhdrum.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#define RH_STALE_US 250000LL
#define RH_RESEND_US 40000LL
#define RH_MAX_PADS 8
#define DS_CHUNK_FRAMES 256

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

    /* DualSense DSP */
    RenHdcDualSenseDsp *(*ds_create)(int32_t, bool);
    void (*ds_destroy)(RenHdcDualSenseDsp *);
    int32_t (*ds_set_half_target)(RenHdcDualSenseDsp *, bool, float, float, float, float);
    int32_t (*ds_fill_f32)(RenHdcDualSenseDsp *, float *, size_t);
    int32_t (*ds_build_bt_report)(RenHdcDualSenseDsp *, uint8_t, uint8_t *);

    /* Xbox One S rumble (4 motors) and Steam Controller 2026 grips (valve DSP); optional like the DualSense ones */
    RenHdcXboxRumble *(*xb_create)(bool);
    void (*xb_destroy)(RenHdcXboxRumble *);
    int32_t (*xb_push)(RenHdcXboxRumble *, int64_t, bool, float, float, float, float);
    int32_t (*xb_tick)(RenHdcXboxRumble *, int64_t, uint16_t *);
    int32_t (*xb_resend)(RenHdcXboxRumble *);
    RenHdcDualSenseDsp *(*sv_create)(int32_t, bool);
    int32_t (*sv_build)(RenHdcDualSenseDsp *, uint8_t, uint8_t *);
} rh;

static struct {
    int tried;
    int is_sdl3;
    uint16_t (*get_vid)(void *);
    uint16_t (*get_pid)(void *);
    int (*get_conn)(void *);
    void *(*get_joy)(void *);
    int (*send_effect)(void *, const void *, int);
    void (*close_pad)(void *);
    int (*rumble_pad)(void *, uint16_t, uint16_t, uint32_t);
    void (*free_mem)(void *);
    int (*get_type)(void *);                       /* SDL_GetGamepadType: Xbox One S by type */
    uint64_t (*get_steam_handle)(void *);          /* not 0 = Steam Input's stand-in pad (host/steam_pads.h) */
    int (*rumble_joy)(void *, uint16_t, uint16_t, uint32_t);        /* SDL3 returns bool: see sdl_ok() */
    int (*rumble_joy_trig)(void *, uint16_t, uint16_t, uint32_t);

    /* SDL3 / SDL2 HID & device paths */
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
    void *(*open_audio_stream)(uint32_t, const void *, void *, void *);
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
        *(FARPROC *)&sdl.get_vid = GetProcAddress(m, "SDL_GetGamepadVendor");
        *(FARPROC *)&sdl.get_pid = GetProcAddress(m, "SDL_GetGamepadProduct");
        *(FARPROC *)&sdl.get_conn = GetProcAddress(m, "SDL_GetGamepadConnectionState");
        *(FARPROC *)&sdl.get_joy = GetProcAddress(m, "SDL_GetGamepadJoystick");
        *(FARPROC *)&sdl.send_effect = GetProcAddress(m, "SDL_SendJoystickEffect");
        *(FARPROC *)&sdl.close_pad = GetProcAddress(m, "SDL_CloseGamepad");
        *(FARPROC *)&sdl.rumble_pad = GetProcAddress(m, "SDL_RumbleGamepad");
        *(FARPROC *)&sdl.free_mem = GetProcAddress(m, "SDL_free");
        *(FARPROC *)&sdl.get_type = GetProcAddress(m, "SDL_GetGamepadType");
        *(FARPROC *)&sdl.get_steam_handle = GetProcAddress(m, "SDL_GetGamepadSteamHandle");
        *(FARPROC *)&sdl.rumble_joy = GetProcAddress(m, "SDL_RumbleJoystick");
        *(FARPROC *)&sdl.rumble_joy_trig = GetProcAddress(m, "SDL_RumbleJoystickTriggers");

        *(FARPROC *)&sdl.get_gamepad_path = GetProcAddress(m, "SDL_GetGamepadPath");
        *(FARPROC *)&sdl.get_joy_path = GetProcAddress(m, "SDL_GetJoystickPath");
        *(FARPROC *)&sdl.hid_open_path = GetProcAddress(m, "SDL_hid_open_path");
        *(FARPROC *)&sdl.hid_write = GetProcAddress(m, "SDL_hid_write");
        *(FARPROC *)&sdl.hid_close = GetProcAddress(m, "SDL_hid_close");
        *(FARPROC *)&sdl.hid_enumerate = GetProcAddress(m, "SDL_hid_enumerate");
        *(FARPROC *)&sdl.hid_free_enumeration = GetProcAddress(m, "SDL_hid_free_enumeration");

        *(FARPROC *)&sdl.pump_events = GetProcAddress(m, "SDL_PumpEvents");
        *(FARPROC *)&sdl.get_playback_devs = GetProcAddress(m, "SDL_GetAudioPlaybackDevices");
        *(FARPROC *)&sdl.get_dev_name = GetProcAddress(m, "SDL_GetAudioDeviceName");
        *(FARPROC *)&sdl.get_dev_format = GetProcAddress(m, "SDL_GetAudioDeviceFormat");
        *(FARPROC *)&sdl.open_audio_stream = GetProcAddress(m, "SDL_OpenAudioDeviceStream");
        *(FARPROC *)&sdl.resume_audio_stream = GetProcAddress(m, "SDL_ResumeAudioStreamDevice");
        *(FARPROC *)&sdl.pause_audio_stream = GetProcAddress(m, "SDL_PauseAudioStreamDevice");
        *(FARPROC *)&sdl.clear_audio_stream = GetProcAddress(m, "SDL_ClearAudioStream");
        *(FARPROC *)&sdl.destroy_audio_stream = GetProcAddress(m, "SDL_DestroyAudioStream");
        *(FARPROC *)&sdl.put_audio_data = GetProcAddress(m, "SDL_PutAudioStreamData");
    } else {
        sdl.is_sdl3 = 0;
        *(FARPROC *)&sdl.get_vid = GetProcAddress(m, "SDL_GameControllerGetVendor");
        *(FARPROC *)&sdl.get_pid = GetProcAddress(m, "SDL_GameControllerGetProduct");
        *(FARPROC *)&sdl.get_joy = GetProcAddress(m, "SDL_GameControllerGetJoystick");
        *(FARPROC *)&sdl.send_effect = GetProcAddress(m, "SDL_JoystickSendEffect");
        if (!sdl.send_effect) *(FARPROC *)&sdl.send_effect = GetProcAddress(m, "SDL_GameControllerSendEffect");
        *(FARPROC *)&sdl.close_pad = GetProcAddress(m, "SDL_GameControllerClose");
        *(FARPROC *)&sdl.rumble_pad = GetProcAddress(m, "SDL_GameControllerRumble");
        *(FARPROC *)&sdl.get_power = GetProcAddress(m, "SDL_JoystickCurrentPowerLevel");
        *(FARPROC *)&sdl.get_type = GetProcAddress(m, "SDL_GameControllerGetType");
        *(FARPROC *)&sdl.get_steam_handle = GetProcAddress(m, "SDL_GameControllerGetSteamHandle");
        *(FARPROC *)&sdl.rumble_joy = GetProcAddress(m, "SDL_JoystickRumble");
        *(FARPROC *)&sdl.rumble_joy_trig = GetProcAddress(m, "SDL_JoystickRumbleTriggers");

        *(FARPROC *)&sdl.get_controller_path = GetProcAddress(m, "SDL_GameControllerPath");
        *(FARPROC *)&sdl.get_joy_path = GetProcAddress(m, "SDL_JoystickPath");
        *(FARPROC *)&sdl.hid_open_path = GetProcAddress(m, "SDL_hid_open_path");
        *(FARPROC *)&sdl.hid_write = GetProcAddress(m, "SDL_hid_write");
        *(FARPROC *)&sdl.hid_close = GetProcAddress(m, "SDL_hid_close");
        *(FARPROC *)&sdl.hid_enumerate = GetProcAddress(m, "SDL_hid_enumerate");
        *(FARPROC *)&sdl.hid_free_enumeration = GetProcAddress(m, "SDL_hid_free_enumeration");

        *(FARPROC *)&sdl.get_num_audio_devs = GetProcAddress(m, "SDL_GetNumAudioDevices");
        *(FARPROC *)&sdl.get_audio_dev_name = GetProcAddress(m, "SDL_GetAudioDeviceName");
        *(FARPROC *)&sdl.open_audio_device = GetProcAddress(m, "SDL_OpenAudioDevice");
        *(FARPROC *)&sdl.pause_audio_device = GetProcAddress(m, "SDL_PauseAudioDevice");
        *(FARPROC *)&sdl.close_audio_device = GetProcAddress(m, "SDL_CloseAudioDevice");
    }
}

int rh_load(void) {
    if (rh.tried) return rh.create != NULL;
    rh.tried = 1;
    const char *off = getenv("HOOKSHOT_RENHDRUM");
    if (off && !strcmp(off, "0")) {
        hos_log("renhdrum: off (env)");
        return 0;
    }

    WCHAR exe_path[MAX_PATH];
    DWORD exe_len = GetModuleFileNameW(NULL, exe_path, MAX_PATH);
    if (exe_len == 0 || exe_len >= MAX_PATH) {
        hos_log("renhdrum: off (cannot resolve exe path)");
        return 0;
    }
    const WCHAR *slash = NULL;
    for (DWORD i = 0; i < exe_len; i++) {
        if (exe_path[i] == L'\\' || exe_path[i] == L'/') {
            slash = &exe_path[i];
        }
    }
    if (!slash) {
        hos_log("renhdrum: off (cannot resolve exe directory)");
        return 0;
    }
    static const WCHAR dll_name[] = L"renhdrum.dll";
    size_t dir_len = (size_t)(slash - exe_path) + 1;
    size_t name_len = sizeof(dll_name) / sizeof(dll_name[0]); /* includes NUL */
    if (dir_len + name_len > MAX_PATH) {
        hos_log("renhdrum: off (dll path too long)");
        return 0;
    }
    WCHAR dll_path[MAX_PATH];
    memcpy(dll_path, exe_path, dir_len * sizeof(WCHAR));
    memcpy(dll_path + dir_len, dll_name, name_len * sizeof(WCHAR));

#ifndef LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR
#define LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR 0x00000100
#endif
#ifndef LOAD_LIBRARY_SEARCH_SYSTEM32
#define LOAD_LIBRARY_SEARCH_SYSTEM32 0x00000800
#endif
    rh.dll = LoadLibraryExW(dll_path, NULL, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!rh.dll) {
        hos_log("renhdrum: off (renhdrum.dll not found beside exe)");
        return 0;
    }
    const char *why = NULL;
#define RH_FN(field, name) *(FARPROC *)&rh.field = GetProcAddress(rh.dll, name); if (!rh.field && !why) why = name;
    RH_FN(version, "renhdrum_version");
    RH_FN(create, "renhdrum_engine_create");
    RH_FN(destroy, "renhdrum_engine_destroy");
    RH_FN(defaults, "renhdrum_settings_default");
    RH_FN(apply, "renhdrum_engine_apply_settings");
    RH_FN(push, "renhdrum_engine_push_half_sample");
    RH_FN(tick, "renhdrum_engine_tick");
    RH_FN(set_nx2_mode, "renhdrum_set_nx2_mode");
#undef RH_FN
    if (why) {
        hos_log("renhdrum: off (%s missing)", why);
        FreeLibrary(rh.dll);
        memset(&rh, 0, sizeof(rh));
        rh.tried = 1;
        return 0;
    }
    uint32_t ver = rh.version();
    if ((ver >> 8) != ((uint32_t)RENHDRUM_VERSION >> 8)) {
        hos_log("renhdrum: off (incompatible renhdrum.dll version 0x%06x, expected branch 0x%04xxx)",
                (unsigned)ver, (unsigned)((uint32_t)RENHDRUM_VERSION >> 8));
        FreeLibrary(rh.dll);
        memset(&rh, 0, sizeof(rh));
        rh.tried = 1;
        return 0;
    }
    if (rh.set_nx2_mode) {
        rh.set_nx2_mode(2); /* Mode 2: Switch 2 translation for OoT */
    }

    /* DualSense DSP functions (optional if running against older renhdrum.dll) */
    *(FARPROC *)&rh.ds_create = GetProcAddress(rh.dll, "renhdrum_dualsense_dsp_create");
    *(FARPROC *)&rh.ds_destroy = GetProcAddress(rh.dll, "renhdrum_dualsense_dsp_destroy");
    *(FARPROC *)&rh.ds_set_half_target = GetProcAddress(rh.dll, "renhdrum_dualsense_dsp_set_half_target");
    *(FARPROC *)&rh.ds_fill_f32 = GetProcAddress(rh.dll, "renhdrum_dualsense_dsp_fill_f32");
    *(FARPROC *)&rh.ds_build_bt_report = GetProcAddress(rh.dll, "renhdrum_dualsense_dsp_build_bt_report");

    /* Xbox One S and Steam Controller grips: a dll without them leaves that pad on Hookshot's own rumble */
    *(FARPROC *)&rh.xb_create = GetProcAddress(rh.dll, "renhdrum_xbox_rumble_create");
    *(FARPROC *)&rh.xb_destroy = GetProcAddress(rh.dll, "renhdrum_xbox_rumble_destroy");
    *(FARPROC *)&rh.xb_push = GetProcAddress(rh.dll, "renhdrum_xbox_rumble_push_half_sample");
    *(FARPROC *)&rh.xb_tick = GetProcAddress(rh.dll, "renhdrum_xbox_rumble_tick");
    *(FARPROC *)&rh.xb_resend = GetProcAddress(rh.dll, "renhdrum_xbox_rumble_resend");
    *(FARPROC *)&rh.sv_create = GetProcAddress(rh.dll, "renhdrum_valve_grip_dsp_create");
    *(FARPROC *)&rh.sv_build = GetProcAddress(rh.dll, "renhdrum_valve_grip_build_report");

    return 1;
}

/* ---- Connected slot tracking (matched to pad_sdl.c MAX_PADS slots) ---- */
static SRWLOCK rh_pads_lock = SRWLOCK_INIT;
static void *rh_conn_pads[RH_MAX_PADS];
static int rh_slot_claimed[RH_MAX_PADS];
static int rh_slot_stand_in[RH_MAX_PADS];   /* Steam Input's stand-in pad: never claimed (see rh_attach) */
static volatile LONG rh_enabled = 1;

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
    for (int i = 0; i < RH_MAX_PADS; i++)
        if (rh_conn_pads[i] && rh_conn_pads[i] == pad) rh_slot_stand_in[i] = 1;
    ReleaseSRWLockExclusive(&rh_pads_lock);
}

static int rh_stand_in_tracked(void) {
    int any = 0;
    AcquireSRWLockShared(&rh_pads_lock);
    for (int i = 0; i < RH_MAX_PADS; i++) any |= rh_conn_pads[i] && rh_slot_stand_in[i];
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
static uint8_t rh_last_pkt[10];
static int rh_has_last_pkt;
static int64_t rh_last_send_us;
static int64_t rh_at[2];
static int rh_muted[2];

static int rh_pkt_is_silent(const uint8_t p[10]) {
    static const uint8_t neutral[4] = {0x00, 0x01, 0x40, 0x40};
    return memcmp(p + 2, neutral, 4) == 0 && memcmp(p + 6, neutral, 4) == 0;
}

/* ---- DualSense state ---- */
static SRWLOCK rh_ds_lock = SRWLOCK_INIT;
static RenHdcDualSenseDsp *rh_ds_dsp;
static void *rh_ds_active_pad;
static void *rh_ds_stream;
static uint32_t rh_ds_dev2;
static int rh_ds_is_bt;
static void *rh_ds_bt_hid;
static HANDLE rh_ds_bt_thread;
static volatile LONG rh_ds_bt_run;
static int rh_ds_audio_attempts;
static int64_t rh_ds_next_audio_retry_us;
static int64_t rh_ds_at[2];
static int rh_ds_muted[2];

/* ---- Xbox One S state (4 motors: grips, triggers) ---- */
static SRWLOCK rh_xb_lock = SRWLOCK_INIT;
static RenHdcXboxRumble *rh_xb;
static void *rh_xb_pad;
static void *rh_xb_joy;
static int64_t rh_xb_at[2];
static int rh_xb_muted[2];

/* ---- Steam Controller 2026 grips (HID report 0x88 from a thread; experimental) ---- */
#define RH_STEAM_FAILS_MAX 100
#define RH_STEAM_TAIL_NS 100000000LL
static SRWLOCK rh_sv_lock = SRWLOCK_INIT;
static RenHdcDualSenseDsp *rh_sv_dsp;
static void *rh_sv_pad;
static void *rh_sv_hid;
static HANDLE rh_sv_thread;
static volatile LONG rh_sv_run;
static uint8_t rh_sv_format;
static int64_t rh_sv_at[2];
static int rh_sv_muted[2];

/* SDL3's rumble calls return bool (only the low byte is set), SDL2's int 0 when done */
static int sdl_ok(int r) { return sdl.is_sdl3 ? (r & 0xFF) != 0 : r == 0; }

/* ---- Worker thread ---- */
static HANDLE rh_thread;
static volatile LONG rh_run;

int rh_ds_render_frames(float *out, int frames) {
    if (!out || frames <= 0) return -1;
    AcquireSRWLockExclusive(&rh_ds_lock);
    int res = -1;
    if (rh_ds_dsp && rh.ds_fill_f32) {
        res = rh.ds_fill_f32(rh_ds_dsp, out, (size_t)frames);
    }
    ReleaseSRWLockExclusive(&rh_ds_lock);
    if (res < 0) {
        memset(out, 0, (size_t)frames * 4 * sizeof(float));
    }
    return res;
}

static void SDLCALL ds_audio_stream_cb(void *userdata, void *stream, int additional_amount, int total_amount) {
    (void)userdata; (void)total_amount;
    if (additional_amount <= 0 || !stream) return;
    int bytes_per_frame = 4 * (int)sizeof(float);
    int frames_needed = additional_amount / bytes_per_frame;
    if (frames_needed <= 0) return;

    float chunk[DS_CHUNK_FRAMES * 4];
    while (frames_needed > 0) {
        int cur_frames = frames_needed < DS_CHUNK_FRAMES ? frames_needed : DS_CHUNK_FRAMES;
        rh_ds_render_frames(chunk, cur_frames);
        if (sdl.put_audio_data) {
            sdl.put_audio_data(stream, chunk, cur_frames * bytes_per_frame);
        }
        frames_needed -= cur_frames;
    }
}

static void SDLCALL ds_audio_dev_cb(void *userdata, uint8_t *stream, int len) {
    (void)userdata;
    if (len <= 0 || !stream) return;
    int bytes_per_frame = 4 * (int)sizeof(float);
    int frames_needed = len / bytes_per_frame;
    float chunk[DS_CHUNK_FRAMES * 4];
    uint8_t *dst = stream;

    while (frames_needed > 0) {
        int cur_frames = frames_needed < DS_CHUNK_FRAMES ? frames_needed : DS_CHUNK_FRAMES;
        rh_ds_render_frames(chunk, cur_frames);
        memcpy(dst, chunk, cur_frames * bytes_per_frame);
        dst += cur_frames * bytes_per_frame;
        frames_needed -= cur_frames;
    }
    int rem = len - (int)(dst - stream);
    if (rem > 0) memset(dst, 0, (size_t)rem);
}

static void rh_ds_try_open_audio(void) {
    AcquireSRWLockExclusive(&rh_ds_lock);
    if (!rh_ds_dsp || rh_ds_stream || rh_ds_dev2) {
        ReleaseSRWLockExclusive(&rh_ds_lock);
        return;
    }
    rh_ds_audio_attempts++;
    ReleaseSRWLockExclusive(&rh_ds_lock);

    hos_sdl_init(0x10 /* HOS_SDL_AUDIO */);

    if (sdl.is_sdl3) {
        if (!sdl.get_playback_devs || !sdl.get_dev_name || !sdl.get_dev_format || !sdl.open_audio_stream) return;
        /* The Windows build never pumps SDL events (no SDL window), and SDL3 only applies audio hotplug
         * while pumping: without this a DualSense plugged in after start never shows up as a device. */
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
                    matched = devs[i];
                    matched_name = name;
                    break;
                }
            }
        }
        if (!matched) {
            if (sdl.free_mem) sdl.free_mem(devs);
            return;
        }

        struct { uint32_t format; int channels, freq; } desired = {0x8120 /* SDL_AUDIO_F32 */, 4, 48000};
        void *st = sdl.open_audio_stream(matched, &desired, ds_audio_stream_cb, NULL);
        if (sdl.free_mem) sdl.free_mem(devs);
        if (!st) {
            hos_log("renhdrum: dualsense audio stream open failed");
            return;
        }
        if (sdl.resume_audio_stream) sdl.resume_audio_stream(st);

        int keep = 0;
        void *pad = NULL;
        AcquireSRWLockExclusive(&rh_ds_lock);
        if (rh_ds_dsp != NULL && rh_ds_stream == NULL) {
            rh_ds_stream = st;
            pad = rh_ds_active_pad;
            keep = 1;
        }
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (!keep) {
            if (sdl.pause_audio_stream) sdl.pause_audio_stream(st);
            if (sdl.clear_audio_stream) sdl.clear_audio_stream(st);
            if (sdl.destroy_audio_stream) sdl.destroy_audio_stream(st);
            return;
        }

        if (sdl.rumble_pad && pad) {
            sdl.rumble_pad(pad, 0, 0, 0);
        }
        hos_log("renhdrum: dualsense audio opened on \"%s\" (4 ch, 48000 Hz, attempt %d)",
                matched_name ? matched_name : "DualSense", rh_ds_audio_attempts);
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
        want.userdata = NULL;

        uint32_t dev = sdl.open_audio_device(matched_name, 0, &want, &have, 0);
        if (!dev) {
            hos_log("renhdrum: dualsense audio open failed (SDL2)");
            return;
        }
        if (sdl.pause_audio_device) sdl.pause_audio_device(dev, 0);

        int keep = 0;
        void *pad = NULL;
        AcquireSRWLockExclusive(&rh_ds_lock);
        if (rh_ds_dsp != NULL && rh_ds_dev2 == 0) {
            rh_ds_dev2 = dev;
            pad = rh_ds_active_pad;
            keep = 1;
        }
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (!keep) {
            if (sdl.pause_audio_device) sdl.pause_audio_device(dev, 1);
            if (sdl.close_audio_device) sdl.close_audio_device(dev);
            return;
        }

        if (sdl.rumble_pad && pad) {
            sdl.rumble_pad(pad, 0, 0, 0);
        }
        hos_log("renhdrum: dualsense audio opened on \"%s\" (4 ch, 48000 Hz, attempt %d, SDL2)",
                matched_name, rh_ds_audio_attempts);
    }
}

static DWORD WINAPI rh_ds_bt_worker(void *arg) {
    (void)arg;
    uint8_t report[RENHDRUM_DS5_BT_REPORT_SIZE];
    int64_t next = hos_now_ns();
    int64_t last_signal = 0;
    int64_t stats_at = next;
    int sent = 0, failed = 0, fails_in_row = 0;
    HANDLE timer = CreateWaitableTimerExW(NULL, NULL, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);

    while (InterlockedCompareExchange(&rh_ds_bt_run, 1, 1) == 1) {
        next += RENHDRUM_DS5_BT_PERIOD_NS;

        AcquireSRWLockExclusive(&rh_ds_lock);
        RenHdcDualSenseDsp *dsp = rh_ds_dsp;
        void *hid = rh_ds_bt_hid;
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (!dsp || !hid || !rh.ds_build_bt_report) {
            break;
        }

        int r = rh.ds_build_bt_report(dsp, RENHDRUM_DS5_BT_BUFFER_LENGTH_DEFAULT, report);
        int64_t now = hos_now_ns();
        if (r == 1) {
            last_signal = now;
        }

        /* Silence gate: keep sending 2 s after the last vibration, then stay quiet on the radio */
        if (r >= 0 && last_signal != 0 && (now - last_signal < 2000000000LL)) {
            int w = -1;
            if (sdl.hid_write) {
                w = sdl.hid_write(hid, report, sizeof(report));
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
            hos_log("renhdrum: dualsense bt haptics writes failing, stopping");
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
            next = now; /* stalled > 100 ms: resync instead of bursting */
        }
    }

    if (timer) CloseHandle(timer);
    InterlockedExchange(&rh_ds_bt_run, 0);
    return 0;
}

/* Steam Controller 2026 grips (protocol by iafx, RenHDrum#5): HID report 0x86 chooses the PCM mode (op 2 = on, 1 = off,
   once per channel 2 and 5), then report 0x88 every period while vibrating and for 100 ms after. Only this thread
   writes to rh_sv_hid; rh_sv_stop joins it before closing the handle. */
static void rh_sv_mode(void *hid, uint8_t op, uint8_t format) {
    static const uint8_t channels[2] = {2, 5};
    for (int i = 0; i < 2; i++) {
        const uint8_t r[4] = {0x86, op, channels[i], format};
        sdl.hid_write(hid, r, sizeof r);
    }
}

static DWORD WINAPI rh_sv_worker(void *arg) {
    (void)arg;
    uint8_t report[RENHDRUM_VALVE_GRIP_REPORT_SIZE];
    AcquireSRWLockExclusive(&rh_sv_lock);
    void *hid = rh_sv_hid;
    const uint8_t format = rh_sv_format;
    void *pad = rh_sv_pad;
    ReleaseSRWLockExclusive(&rh_sv_lock);
    const int64_t period = format == RENHDRUM_VALVE_GRIP_FORMAT_MULAW_8K ? (int64_t)RENHDRUM_VALVE_GRIP_PERIOD_NS_8K
                                                                         : (int64_t)RENHDRUM_VALVE_GRIP_PERIOD_NS_4K;
    int gave_up = 0;

    rh_sv_mode(hid, 1, 0);
    Sleep(10);
    rh_sv_mode(hid, 2, format);
    Sleep(200); /* the controller needs a moment between off and on */

    HANDLE timer = CreateWaitableTimerExW(NULL, NULL, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);
    int64_t next = hos_now_ns();
    int64_t last_signal = 0;
    int fails_in_row = 0;
    while (InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) {
        next += period;
        AcquireSRWLockExclusive(&rh_sv_lock);
        RenHdcDualSenseDsp *dsp = rh_sv_dsp;
        ReleaseSRWLockExclusive(&rh_sv_lock);
        if (!dsp) break;

        int r = rh.sv_build(dsp, format, report);
        int64_t now = hos_now_ns();
        if (r == 1) last_signal = now;
        if (r >= 0 && last_signal != 0 && now - last_signal < RH_STEAM_TAIL_NS) {
            if (sdl.hid_write(hid, report, sizeof report) < 0) fails_in_row++;
            else fails_in_row = 0;
        }
        if (fails_in_row >= RH_STEAM_FAILS_MAX) {
            hos_log("renhdrum: steam controller grip writes failing, using standard rumble");
            gave_up = 1;
            break;
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
            next = now; /* stalled > 100 ms: resync instead of bursting */
        }
    }
    if (timer) CloseHandle(timer);
    rh_sv_mode(hid, 1, format);
    rh_sv_mode(hid, 1, 0);
    InterlockedExchange(&rh_sv_run, 0);
    if (gave_up) rh_pads_set_claimed(pad, 0); /* the pad goes back to Hookshot's own rumble */
    return 0;
}

/* Stops the Steam grips (pad == NULL: whichever pad has them); returns the pad it released, or NULL */
static void *rh_sv_stop(void *pad) {
    AcquireSRWLockExclusive(&rh_sv_lock);
    if (!rh_sv_pad || (pad && rh_sv_pad != pad)) {
        ReleaseSRWLockExclusive(&rh_sv_lock);
        return NULL;
    }
    void *had = rh_sv_pad;
    void *hid = rh_sv_hid;
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
    }
    if (hid && sdl.hid_close) sdl.hid_close(hid);
    if (dsp && rh.ds_destroy) rh.ds_destroy(dsp);
    return had;
}

static const char *rh_pad_hid_path(void *pad) {
    void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
    const char *path = NULL;
    if (sdl.is_sdl3 && sdl.get_gamepad_path) path = sdl.get_gamepad_path(pad);
    if (!sdl.is_sdl3 && sdl.get_controller_path) path = sdl.get_controller_path(pad);
    if ((!path || !*path) && sdl.get_joy_path && joy) path = sdl.get_joy_path(joy);
    return path;
}

static DWORD WINAPI rh_worker(void *arg) {
    (void)arg;
    uint8_t pkt[10];
    while (InterlockedCompareExchange(&rh_run, 1, 1) == 1) {
        int64_t now_us = hos_now_ns() / 1000;
        int send_pkt = 0;
        void *target = NULL;

        /* Pro Controller watchdog, tick, and wired-only 40 ms resend */
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
            } else if (!rh_is_bt && rh_has_last_pkt && (now_us - rh_last_send_us) >= RH_RESEND_US) {
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

        /* Xbox One S: stale watchdog and tick; the four motors go out of the lock. The numbers come from the core as
           they are (grips last longer than a tick, trigger pulses stay short so a lost stop cannot leave them on) */
        uint16_t motors[4];
        int send_xb = 0;
        void *xb_target = NULL;
        AcquireSRWLockExclusive(&rh_xb_lock);
        if (rh_xb) {
            for (int s = 0; s < 2; s++) {
                if (rh_xb_at[s] && !rh_xb_muted[s] && (now_us - rh_xb_at[s] > RH_STALE_US)) {
                    rh.xb_push(rh_xb, now_us, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    rh_xb_muted[s] = 1;
                }
            }
            send_xb = rh.xb_tick(rh_xb, now_us, motors) == 1;
            xb_target = rh_xb_joy;
        }
        ReleaseSRWLockExclusive(&rh_xb_lock);
        if (send_xb && xb_target) {
            int ok = sdl_ok(sdl.rumble_joy(xb_target, motors[0], motors[1], 2000));
            if (sdl.rumble_joy_trig) sdl.rumble_joy_trig(xb_target, motors[2], motors[3], 200);
            if (!ok && rh.xb_resend) { /* some paths refuse rumble until the first press: try again next tick */
                AcquireSRWLockExclusive(&rh_xb_lock);
                if (rh_xb) rh.xb_resend(rh_xb);
                ReleaseSRWLockExclusive(&rh_xb_lock);
            }
        }

        /* Steam grips watchdog (the sender is its own thread) */
        AcquireSRWLockExclusive(&rh_sv_lock);
        if (rh_sv_dsp) {
            for (int s = 0; s < 2; s++) {
                if (rh_sv_at[s] && !rh_sv_muted[s] && (now_us - rh_sv_at[s] > RH_STALE_US)) {
                    rh.ds_set_half_target(rh_sv_dsp, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    rh_sv_muted[s] = 1;
                }
            }
        }
        ReleaseSRWLockExclusive(&rh_sv_lock);

        /* DualSense watchdog & periodic audio retry */
        int need_audio_retry = 0;
        AcquireSRWLockExclusive(&rh_ds_lock);
        if (rh_ds_dsp) {
            for (int s = 0; s < 2; s++) {
                if (rh_ds_at[s] && !rh_ds_muted[s] && (now_us - rh_ds_at[s] > RH_STALE_US)) {
                    if (rh.ds_set_half_target) {
                        rh.ds_set_half_target(rh_ds_dsp, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    }
                    rh_ds_muted[s] = 1;
                }
            }
            if (!rh_ds_is_bt && !rh_ds_stream && !rh_ds_dev2 && now_us >= rh_ds_next_audio_retry_us) {
                rh_ds_next_audio_retry_us = now_us + 2000000LL; /* retry every 2s */
                need_audio_retry = 1;
            }
        }
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (need_audio_retry) {
            rh_ds_try_open_audio();
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

void rh_attach(void *pad) {
    if (!pad) return;
    rh_pads_track(pad);
    const char *off = getenv("HOOKSHOT_RENHDRUM");
    if (off && !strcmp(off, "0")) return;
    if (InterlockedCompareExchange(&rh_enabled, 1, 1) == 0) return;
    init_sdl();
    if (!sdl.get_vid || !sdl.get_pid) return;

    uint16_t vid = sdl.get_vid(pad);
    uint16_t pid = sdl.get_pid(pad);

    /* Steam Input's stand-in pad (host/steam_pads.h: it has a Steam handle, a real pad has none) is never claimed: Steam
       owns the real pad's rumble then, and a second sender on it (HID grips, or SDL rumble on the stand-in) would play
       twice. A stand-in that opens after the Steam grips started takes them back. */
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
            if (p >= 0 && p != 4 /* SDL_JOYSTICK_POWER_WIRED */) {
                bt = 1;
            }
        }
    }

    /* Nintendo Switch Pro Controller (VID 0x057E, PID 0x2009) */
    if (vid == 0x057E && pid == 0x2009) {
        if (!rh_load()) return;

        AcquireSRWLockExclusive(&rh_lock);
        int pro_busy = (rh_engine != NULL);
        ReleaseSRWLockExclusive(&rh_lock);
        if (pro_busy) return;

        RenHdcEngine *e = rh.create(bt != 0, true);
        if (!e) {
            hos_log("renhdrum: off (engine create failed)");
            return;
        }
        RenHdcSettings s;
        if (rh.defaults(&s) != RENHDRUM_OK) {
            rh.destroy(e);
            hos_log("renhdrum: off (defaults failed)");
            return;
        }
        if (bt) {
            s.bluetooth_level = 4; /* Level 4 (Compatible): core handles pacing & 40 ms resend */
            const char *env_bt = getenv("HOOKSHOT_BT_LEVEL");
            if (env_bt && atoi(env_bt) >= 1 && atoi(env_bt) <= 4) {
                s.bluetooth_level = atoi(env_bt);
            }
        }
        s.usb_no_queue = s.bt_after_input = s.bt_fast = false;
        rh.apply(e, &s);

        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;

        AcquireSRWLockExclusive(&rh_lock);
        if (rh_engine) {
            ReleaseSRWLockExclusive(&rh_lock);
            rh.destroy(e);
            return;
        }
        rh_engine = e;
        rh_active_pad = pad;
        rh_joy = joy;
        rh_is_bt = bt ? 1 : 0;
        memset(rh_last_pkt, 0, sizeof(rh_last_pkt));
        rh_has_last_pkt = 0;
        rh_last_send_us = 0;
        rh_at[0] = rh_at[1] = 0;
        rh_muted[0] = rh_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_lock);

        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        if (bt) {
            hos_log("renhdrum: on (bluetooth level %d)", s.bluetooth_level);
        } else {
            hos_log("renhdrum: on (usb)");
        }
        return;
    }

    /* Sony DualSense (VID 0x054C, PID 0x0CE6 or 0x0DF2) */
    if (vid == 0x054C && (pid == 0x0CE6 || pid == 0x0DF2)) {
        if (!rh_load()) return;
        if (!rh.ds_create || !rh.ds_fill_f32) {
            hos_log("renhdrum: off (dualsense dsp functions missing in renhdrum.dll)");
            return;
        }

        AcquireSRWLockExclusive(&rh_ds_lock);
        int ds_busy = (rh_ds_dsp != NULL);
        ReleaseSRWLockExclusive(&rh_ds_lock);
        if (ds_busy) return;

        if (bt) {
            if (!rh.ds_build_bt_report) {
                hos_log("renhdrum: off (dualsense bt report function missing in renhdrum.dll)");
                return;
            }

            void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
            const char *path = NULL;
            if (sdl.is_sdl3) {
                if (sdl.get_gamepad_path) path = sdl.get_gamepad_path(pad);
                if ((!path || !*path) && sdl.get_joy_path && joy) path = sdl.get_joy_path(joy);
            } else {
                if (sdl.get_controller_path) path = sdl.get_controller_path(pad);
                if ((!path || !*path) && sdl.get_joy_path && joy) path = sdl.get_joy_path(joy);
            }

            void *hid_dev = NULL;
            if (path && *path && sdl.hid_open_path) {
                hid_dev = sdl.hid_open_path(path);
            }

            if (!hid_dev && sdl.hid_enumerate) {
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
                    if (devs->path && sdl.hid_open_path) {
                        hid_dev = sdl.hid_open_path(devs->path);
                    }
                    if (sdl.hid_free_enumeration) sdl.hid_free_enumeration(devs);
                }
            }

            if (!hid_dev) {
                hos_log("renhdrum: dualsense bt haptics: no HID path or open failed, using standard rumble");
                return;
            }

            RenHdcDualSenseDsp *dsp = rh.ds_create(100, false); /* false for crisp notes on Bluetooth */
            if (!dsp) {
                if (sdl.hid_close) sdl.hid_close(hid_dev);
                hos_log("renhdrum: off (dualsense dsp create failed)");
                return;
            }

            AcquireSRWLockExclusive(&rh_ds_lock);
            rh_ds_dsp = dsp;
            rh_ds_active_pad = pad;
            rh_ds_is_bt = 1;
            rh_ds_bt_hid = hid_dev;
            rh_ds_at[0] = rh_ds_at[1] = 0;
            rh_ds_muted[0] = rh_ds_muted[1] = 0;
            ReleaseSRWLockExclusive(&rh_ds_lock);

            /* Stop SDL's HID rumble once so voice-coil haptics play cleanly */
            if (sdl.rumble_pad) {
                sdl.rumble_pad(pad, 0, 0, 0);
            }

            InterlockedExchange(&rh_ds_bt_run, 1);
            rh_ds_bt_thread = CreateThread(NULL, 0, rh_ds_bt_worker, NULL, 0, NULL);
            if (!rh_ds_bt_thread) {
                InterlockedExchange(&rh_ds_bt_run, 0);
                hos_log("renhdrum: off (cannot create dualsense bt worker thread)");
                rh_detach(pad);
                rh_pads_track(pad);
                return;
            }

            rh_pads_set_claimed(pad, 1);
            rh_ensure_worker();
            hos_log("renhdrum: on (dualsense bluetooth)");
            return;
        }

        RenHdcDualSenseDsp *dsp = rh.ds_create(100, true);
        if (!dsp) {
            hos_log("renhdrum: off (dualsense dsp create failed)");
            return;
        }

        AcquireSRWLockExclusive(&rh_ds_lock);
        if (rh_ds_dsp) {
            ReleaseSRWLockExclusive(&rh_ds_lock);
            if (rh.ds_destroy) rh.ds_destroy(dsp);
            return;
        }
        rh_ds_dsp = dsp;
        rh_ds_active_pad = pad;
        rh_ds_is_bt = 0;
        rh_ds_audio_attempts = 0;
        rh_ds_next_audio_retry_us = 0;
        rh_ds_at[0] = rh_ds_at[1] = 0;
        rh_ds_muted[0] = rh_ds_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_ds_lock);

        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        hos_log("renhdrum: on (dualsense usb)");
        rh_ds_try_open_audio();
        return;
    }

    /* Steam Controller 2026 (VID 0x28DE, PID 1302 wired / 1303 Bluetooth / 1304 dongle): grips over HID, experimental */
    if (vid == 0x28DE && (pid == 0x1302 || pid == 0x1303 || pid == 0x1304)) {
        if (rh_stand_in_tracked()) {
            hos_log("renhdrum: steam controller: Steam Input's pad is open, its rumble stays with Steam");
            return;
        }
        if (!rh_load()) return;
        if (!rh.sv_create || !rh.sv_build || !rh.ds_set_half_target || !rh.ds_destroy) {
            hos_log("renhdrum: steam controller: valve grip functions missing in renhdrum.dll, using standard rumble");
            return;
        }
        if (!sdl.hid_open_path || !sdl.hid_write) return;

        AcquireSRWLockExclusive(&rh_sv_lock);
        int sv_busy = (rh_sv_dsp != NULL);
        ReleaseSRWLockExclusive(&rh_sv_lock);
        if (sv_busy) return;

        const char *path = rh_pad_hid_path(pad);
        void *hid = (path && *path) ? sdl.hid_open_path(path) : NULL;
        if (!hid) {
            hos_log("renhdrum: steam controller: no HID path or open failed, using standard rumble");
            return;
        }
        RenHdcDualSenseDsp *dsp = rh.sv_create(100, true);
        if (!dsp) {
            if (sdl.hid_close) sdl.hid_close(hid);
            hos_log("renhdrum: off (steam grip dsp create failed)");
            return;
        }

        AcquireSRWLockExclusive(&rh_sv_lock);
        rh_sv_dsp = dsp;
        rh_sv_pad = pad;
        rh_sv_hid = hid;
        rh_sv_format = pid == 0x1302 ? RENHDRUM_VALVE_GRIP_FORMAT_MULAW_8K : RENHDRUM_VALVE_GRIP_FORMAT_MULAW_4K;
        rh_sv_at[0] = rh_sv_at[1] = 0;
        rh_sv_muted[0] = rh_sv_muted[1] = 0;
        InterlockedExchange(&rh_sv_run, 1);
        rh_sv_thread = CreateThread(NULL, 0, rh_sv_worker, NULL, 0, NULL);
        const int started = rh_sv_thread != NULL;
        ReleaseSRWLockExclusive(&rh_sv_lock);
        if (!started) {
            hos_log("renhdrum: off (cannot create steam grips thread)");
            rh_sv_stop(pad);
            return;
        }
        /* The grips' rumble motors would fight the audio */
        if (sdl.rumble_pad) sdl.rumble_pad(pad, 0, 0, 0);
        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker(); /* its watchdog mutes a rumble the game stopped sending */
        hos_log("renhdrum: on (steam controller grips, experimental, %s kHz)", pid == 0x1302 ? "8" : "4");
        return;
    }

    /* Xbox One S: SDL type XBOXONE, or Microsoft's vendor id (a third-party XInput pad may get another type).
       Motor numbers and curves come from the core; the bridge sets none. */
    const int xb_type = sdl.get_type ? sdl.get_type(pad) : 0;
    if (xb_type == (sdl.is_sdl3 ? 3 : 2) /* XBOXONE */ || vid == 0x045E) {
        if (!rh_load()) return;
        if (!rh.xb_create || !rh.xb_destroy || !rh.xb_push || !rh.xb_tick) {
            hos_log("renhdrum: xbox: rumble functions missing in renhdrum.dll, using standard rumble");
            return;
        }
        if (!sdl.rumble_joy) {
            hos_log("renhdrum: xbox: this SDL has no joystick rumble call, using standard rumble");
            return;
        }
        AcquireSRWLockExclusive(&rh_xb_lock);
        int xb_busy = (rh_xb != NULL);
        ReleaseSRWLockExclusive(&rh_xb_lock);
        if (xb_busy) return;

        /* a connection SDL cannot tell counts as Bluetooth (the safer 20 ms ticks) */
        const int xb_bt = (sdl.is_sdl3 && sdl.get_conn) ? sdl.get_conn(pad) != 1 /* WIRED */ : bt;
        RenHdcXboxRumble *x = rh.xb_create(xb_bt != 0);
        if (!x) {
            hos_log("renhdrum: off (xbox rumble create failed)");
            return;
        }
        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;
        AcquireSRWLockExclusive(&rh_xb_lock);
        if (rh_xb) {
            ReleaseSRWLockExclusive(&rh_xb_lock);
            rh.xb_destroy(x);
            return;
        }
        rh_xb = x;
        rh_xb_pad = pad;
        rh_xb_joy = joy;
        rh_xb_at[0] = rh_xb_at[1] = 0;
        rh_xb_muted[0] = rh_xb_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_xb_lock);

        rh_pads_set_claimed(pad, 1);
        rh_ensure_worker();
        hos_log("renhdrum: on (xbox %s)", xb_bt ? "bluetooth" : "usb");
        return;
    }
}

static int rh_detach_engines(void *pad, int untrack) {
    int detached_any = 0;
    if (untrack) {
        rh_pads_untrack(pad);
    } else {
        rh_pads_clear_claimed();
    }

    /* Pro Controller detach */
    AcquireSRWLockExclusive(&rh_lock);
    if (rh_active_pad && (!pad || rh_active_pad == pad)) {
        void *target = rh_joy ? rh_joy : rh_active_pad;
        RenHdcEngine *e = rh_engine;
        rh_engine = NULL;
        rh_active_pad = NULL;
        rh_joy = NULL;
        rh_is_bt = 0;
        memset(rh_last_pkt, 0, sizeof(rh_last_pkt));
        rh_has_last_pkt = 0;
        rh_last_send_us = 0;
        ReleaseSRWLockExclusive(&rh_lock);

        static const uint8_t stop_pkt[10] = {0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40};
        if (target && sdl.send_effect) {
            sdl.send_effect(target, stop_pkt, sizeof(stop_pkt));
        }
        if (e && rh.destroy) {
            rh.destroy(e);
        }
        detached_any = 1;
    } else {
        ReleaseSRWLockExclusive(&rh_lock);
    }

    /* DualSense detach */
    AcquireSRWLockExclusive(&rh_ds_lock);
    if (rh_ds_active_pad && (!pad || rh_ds_active_pad == pad)) {
        void *stream = rh_ds_stream;
        uint32_t dev2 = rh_ds_dev2;
        void *bt_hid = rh_ds_bt_hid;
        HANDLE bt_th = rh_ds_bt_thread;
        RenHdcDualSenseDsp *dsp = rh_ds_dsp;
        rh_ds_stream = NULL;
        rh_ds_dev2 = 0;
        rh_ds_bt_hid = NULL;
        rh_ds_bt_thread = NULL;
        rh_ds_dsp = NULL;
        rh_ds_active_pad = NULL;
        rh_ds_is_bt = 0;
        InterlockedExchange(&rh_ds_bt_run, 0);
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (stream) {
            if (sdl.pause_audio_stream) sdl.pause_audio_stream(stream);
            if (sdl.clear_audio_stream) sdl.clear_audio_stream(stream);
            if (sdl.destroy_audio_stream) sdl.destroy_audio_stream(stream);
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
        if (bt_hid && sdl.hid_close) {
            sdl.hid_close(bt_hid);
        }
        if (dsp && rh.ds_destroy) {
            rh.ds_destroy(dsp);
        }
        detached_any = 1;
    } else {
        ReleaseSRWLockExclusive(&rh_ds_lock);
    }

    /* Xbox detach: all four motors to 0 */
    AcquireSRWLockExclusive(&rh_xb_lock);
    if (rh_xb_pad && (!pad || rh_xb_pad == pad)) {
        void *joy = rh_xb_joy ? rh_xb_joy : rh_xb_pad;
        RenHdcXboxRumble *x = rh_xb;
        rh_xb = NULL;
        rh_xb_pad = NULL;
        rh_xb_joy = NULL;
        ReleaseSRWLockExclusive(&rh_xb_lock);

        if (joy && sdl.rumble_joy) {
            sdl.rumble_joy(joy, 0, 0, 0);
            if (sdl.rumble_joy_trig) sdl.rumble_joy_trig(joy, 0, 0, 0);
        }
        if (x && rh.xb_destroy) rh.xb_destroy(x);
        detached_any = 1;
    } else {
        ReleaseSRWLockExclusive(&rh_xb_lock);
    }

    /* Steam grips detach: the thread turns the PCM mode off, then the handle closes (rh_sv_stop) */
    if (rh_sv_stop(pad)) detached_any = 1;

    /* Stop worker if no pad it serves is active */
    AcquireSRWLockExclusive(&rh_lock);
    int pro_active = (rh_engine != NULL);
    ReleaseSRWLockExclusive(&rh_lock);

    AcquireSRWLockExclusive(&rh_ds_lock);
    int ds_active = (rh_ds_dsp != NULL);
    ReleaseSRWLockExclusive(&rh_ds_lock);

    AcquireSRWLockExclusive(&rh_xb_lock);
    int xb_active = (rh_xb != NULL);
    ReleaseSRWLockExclusive(&rh_xb_lock);

    AcquireSRWLockExclusive(&rh_sv_lock);
    int sv_active = (rh_sv_dsp != NULL);
    ReleaseSRWLockExclusive(&rh_sv_lock);

    if (!pro_active && !ds_active && !xb_active && !sv_active) {
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

int rh_detach(void *pad) {
    return rh_detach_engines(pad, 1);
}

void rh_set_enabled(int on) {
    LONG next = on ? 1 : 0;
    LONG prev = InterlockedExchange(&rh_enabled, next);
    if (prev == next) return;
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
    AcquireSRWLockExclusive(&rh_lock);
    if (rh_engine) {
        int bt = rh_is_bt;
        ReleaseSRWLockExclusive(&rh_lock);
        return bt ? "Connected: Pro Controller (Bluetooth), HD Rumble" : "Connected: Pro Controller (USB), HD Rumble";
    }
    ReleaseSRWLockExclusive(&rh_lock);

    AcquireSRWLockExclusive(&rh_ds_lock);
    if (rh_ds_dsp && (!rh_ds_is_bt || InterlockedCompareExchange(&rh_ds_bt_run, 1, 1) == 1)) {
        int bt = rh_ds_is_bt;
        ReleaseSRWLockExclusive(&rh_ds_lock);
        return bt ? "Connected: DualSense (Bluetooth), HD Rumble" : "Connected: DualSense (USB), HD Rumble";
    }
    ReleaseSRWLockExclusive(&rh_ds_lock);

    AcquireSRWLockExclusive(&rh_xb_lock);
    if (rh_xb) {
        ReleaseSRWLockExclusive(&rh_xb_lock);
        return "Connected: Xbox One S, HD Rumble (4 motors)";
    }
    ReleaseSRWLockExclusive(&rh_xb_lock);

    AcquireSRWLockExclusive(&rh_sv_lock);
    if (rh_sv_dsp && InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) {
        ReleaseSRWLockExclusive(&rh_sv_lock);
        return "Steam Controller: HD Rumble (experimental)";
    }
    ReleaseSRWLockExclusive(&rh_sv_lock);

    return "No controller HD Rumble supports: normal rumble";
}

int rh_vibration(int side, float al, float fl, float ah, float fh) {
    if (side < 0 || side > 1) return 0;
    if (InterlockedCompareExchange(&rh_enabled, 1, 1) == 0) return 0;
    int handled = 0;
    int64_t now_us = hos_now_ns() / 1000;

    AcquireSRWLockExclusive(&rh_lock);
    if (rh_engine) {
        rh.push(rh_engine, now_us, side == 1, al, fl, ah, fh);
        rh_at[side] = now_us;
        rh_muted[side] = 0;
        handled = 1;
    }
    ReleaseSRWLockExclusive(&rh_lock);

    AcquireSRWLockExclusive(&rh_ds_lock);
    if (rh_ds_dsp) {
        if (!rh_ds_is_bt || InterlockedCompareExchange(&rh_ds_bt_run, 1, 1) == 1) {
            if (rh.ds_set_half_target) {
                rh.ds_set_half_target(rh_ds_dsp, side == 1, al, fl, ah, fh);
            }
            rh_ds_at[side] = now_us;
            rh_ds_muted[side] = 0;
            handled = 1;
        }
    }
    ReleaseSRWLockExclusive(&rh_ds_lock);

    AcquireSRWLockExclusive(&rh_xb_lock);
    if (rh_xb) {
        rh.xb_push(rh_xb, now_us, side == 1, al, fl, ah, fh);
        rh_xb_at[side] = now_us;
        rh_xb_muted[side] = 0;
        handled = 1;
    }
    ReleaseSRWLockExclusive(&rh_xb_lock);

    AcquireSRWLockExclusive(&rh_sv_lock);
    if (rh_sv_dsp && InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) {
        rh.ds_set_half_target(rh_sv_dsp, side == 1, al, fl, ah, fh);
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
    int active = 0;
    AcquireSRWLockExclusive(&rh_lock);
    if (rh_engine && rh_active_pad == pad) active = 1;
    ReleaseSRWLockExclusive(&rh_lock);
    if (active) return 1;

    AcquireSRWLockExclusive(&rh_ds_lock);
    if (rh_ds_dsp && rh_ds_active_pad == pad) {
        if (!rh_ds_is_bt || InterlockedCompareExchange(&rh_ds_bt_run, 1, 1) == 1) {
            active = 1;
        }
    }
    ReleaseSRWLockExclusive(&rh_ds_lock);
    if (active) return 1;

    AcquireSRWLockExclusive(&rh_xb_lock);
    if (rh_xb && rh_xb_pad == pad) active = 1;
    ReleaseSRWLockExclusive(&rh_xb_lock);
    if (active) return 1;

    AcquireSRWLockExclusive(&rh_sv_lock);
    if (rh_sv_dsp && rh_sv_pad == pad && InterlockedCompareExchange(&rh_sv_run, 1, 1) == 1) active = 1;
    ReleaseSRWLockExclusive(&rh_sv_lock);
    return active;
}

void rh_cleanup(void) {
    rh_detach(NULL);
}
