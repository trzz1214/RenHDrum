/* host/rh_bridge.c - RenHDrum bridge for Hookshot
   Authentic HD Rumble for Nintendo Switch Pro Controller (USB & Bluetooth)
   and DualSense 4-channel 48 kHz audio haptics via renhdrum.dll.
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

        *(FARPROC *)&sdl.get_gamepad_path = GetProcAddress(m, "SDL_GetGamepadPath");
        *(FARPROC *)&sdl.get_joy_path = GetProcAddress(m, "SDL_GetJoystickPath");
        *(FARPROC *)&sdl.hid_open_path = GetProcAddress(m, "SDL_hid_open_path");
        *(FARPROC *)&sdl.hid_write = GetProcAddress(m, "SDL_hid_write");
        *(FARPROC *)&sdl.hid_close = GetProcAddress(m, "SDL_hid_close");
        *(FARPROC *)&sdl.hid_enumerate = GetProcAddress(m, "SDL_hid_enumerate");
        *(FARPROC *)&sdl.hid_free_enumeration = GetProcAddress(m, "SDL_hid_free_enumeration");

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
    rh.dll = LoadLibraryA("renhdrum.dll");
    if (!rh.dll) {
        hos_log("renhdrum: off (renhdrum.dll not found)");
        return 0;
    }
    const char *why = NULL;
#define RH_FN(field, name) *(FARPROC *)&rh.field = GetProcAddress(rh.dll, name); if (!rh.field && !why) why = name;
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
        rh.dll = NULL;
        rh.create = NULL;
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

    return 1;
}

/* ---- Pro Controller state ---- */
static SRWLOCK rh_lock = SRWLOCK_INIT;
static RenHdcEngine *rh_engine;
static void *rh_active_pad;
static void *rh_joy;
static int64_t rh_at[2];
static int rh_muted[2];

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

        AcquireSRWLockExclusive(&rh_ds_lock);
        rh_ds_stream = st;
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (sdl.rumble_pad && rh_ds_active_pad) {
            sdl.rumble_pad(rh_ds_active_pad, 0, 0, 0);
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

        AcquireSRWLockExclusive(&rh_ds_lock);
        rh_ds_dev2 = dev;
        ReleaseSRWLockExclusive(&rh_ds_lock);

        if (sdl.rumble_pad && rh_ds_active_pad) {
            sdl.rumble_pad(rh_ds_active_pad, 0, 0, 0);
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

static DWORD WINAPI rh_worker(void *arg) {
    (void)arg;
    uint8_t pkt[10];
    while (InterlockedCompareExchange(&rh_run, 1, 1) == 1) {
        int64_t now_us = hos_now_ns() / 1000;
        int r = 0;
        void *target = NULL;

        /* Pro Controller watchdog & tick */
        AcquireSRWLockExclusive(&rh_lock);
        if (rh_engine) {
            for (int s = 0; s < 2; s++) {
                if (rh_at[s] && !rh_muted[s] && (now_us - rh_at[s] > RH_STALE_US)) {
                    rh.push(rh_engine, now_us, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    rh_muted[s] = 1;
                }
            }
            r = rh.tick(rh_engine, now_us, pkt);
            target = rh_joy ? rh_joy : rh_active_pad;
        }
        ReleaseSRWLockExclusive(&rh_lock);

        if (r == 1 && target && sdl.send_effect) {
            sdl.send_effect(target, pkt, 10);
        }

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
    const char *off = getenv("HOOKSHOT_RENHDRUM");
    if (off && !strcmp(off, "0")) return;
    init_sdl();
    if (!sdl.get_vid || !sdl.get_pid) return;

    uint16_t vid = sdl.get_vid(pad);
    uint16_t pid = sdl.get_pid(pad);

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
            s.bluetooth_level = 2; /* Level 2 (45 ms spacing) stable, avoids button input latency */
            const char *env_bt = getenv("HOOKSHOT_BT_LEVEL");
            if (env_bt && atoi(env_bt) >= 1 && atoi(env_bt) <= 3) {
                s.bluetooth_level = atoi(env_bt);
            }
        }
        s.usb_no_queue = s.bt_after_input = s.bt_fast = false;
        rh.apply(e, &s);

        void *joy = sdl.get_joy ? sdl.get_joy(pad) : pad;

        AcquireSRWLockExclusive(&rh_lock);
        rh_engine = e;
        rh_active_pad = pad;
        rh_joy = joy;
        rh_at[0] = rh_at[1] = 0;
        rh_muted[0] = rh_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_lock);

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
                return;
            }

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
        rh_ds_dsp = dsp;
        rh_ds_active_pad = pad;
        rh_ds_is_bt = 0;
        rh_ds_audio_attempts = 0;
        rh_ds_next_audio_retry_us = 0;
        rh_ds_at[0] = rh_ds_at[1] = 0;
        rh_ds_muted[0] = rh_ds_muted[1] = 0;
        ReleaseSRWLockExclusive(&rh_ds_lock);

        rh_ensure_worker();
        hos_log("renhdrum: on (dualsense usb)");
        rh_ds_try_open_audio();
        return;
    }
}

int rh_detach(void *pad) {
    int detached_any = 0;

    /* Pro Controller detach */
    AcquireSRWLockExclusive(&rh_lock);
    if (rh_active_pad && (!pad || rh_active_pad == pad)) {
        void *target = rh_joy ? rh_joy : rh_active_pad;
        RenHdcEngine *e = rh_engine;
        rh_engine = NULL;
        rh_active_pad = NULL;
        rh_joy = NULL;
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
            WaitForSingleObject(bt_th, 2000);
            CloseHandle(bt_th);
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

    /* Stop worker if neither controller is active */
    AcquireSRWLockExclusive(&rh_lock);
    int pro_active = (rh_engine != NULL);
    ReleaseSRWLockExclusive(&rh_lock);

    AcquireSRWLockExclusive(&rh_ds_lock);
    int ds_active = (rh_ds_dsp != NULL);
    ReleaseSRWLockExclusive(&rh_ds_lock);

    if (!pro_active && !ds_active) {
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

int rh_vibration(int side, float al, float fl, float ah, float fh) {
    if (side < 0 || side > 1) return 0;
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

    return handled;
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
    return active;
}

void rh_cleanup(void) {
    rh_detach(NULL);
}
