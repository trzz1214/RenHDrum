# Static Recompilation Games HD Rumble Integration Guide

This guide describes how to integrate `renhdrum.dll` into C and C++ static recompilation runtimes using SDL3 to achieve authentic HD Rumble on physical gamepads. It also documents standalone hardware reliability fixes for the Nintendo Switch Pro Controller.

> [!NOTE]
> **Supported Controllers**: `✅ DualSense` · `✅ Nintendo Pro Controller` · `🔬 Xbox One S (research)`.

---

## 1. Overview & Architectural Fit

Static recompilation projects translate native console machine code into native C or C++ sources that link with an SDL3-based platform abstraction layer. In titles where the guest calls vibration services (such as `nn::hid::detail::SendVibrationValue` or `SendVibrationValues`), vanilla recomp runtimes typically downmix the 4-parameter haptic signal (`amp_low`, `freq_low`, `amp_high`, `freq_high`) into standard two-motor low/high frequency vibration (`SDL_RumbleGamepad`).

By dynamically loading `renhdrum.dll` at runtime, the host runtime can synthesize authentic HD Rumble directly to connected hardware:
- **Nintendo Switch Pro Controller 1**: Receives sub-command `0x10` multi-sample packets with dekuNukem LUT encoding at 15 ms cadence (USB) or token-bucket pacing (Bluetooth).
- **PlayStation 5 DualSense**: Receives continuous 48 kHz audio-rate waveforms over wired USB or periodic HID report `0x32` over Bluetooth.
- **Xbox One S**: Receives heuristic 4-motor tactile synthesis across grip and trigger motors.

If `renhdrum.dll` is absent, or if the user disables it via an environment variable, the runtime cleanly falls back to two-motor `SDL_RumbleGamepad`.

---

## 2. Runtime Dynamic Loading (`SDL_LoadObject`)

Using `SDL_LoadObject` and `SDL_LoadFunction` avoids hard-linking against `renhdrum.dll.lib`, keeping compiler toolchains (such as MinGW Clang or MSVC) free of build-time dependencies.

### 2.1 Function Pointer Table

```c
#include <SDL3/SDL.h>
#include <stdint.h>
#include <stdbool.h>

#define RH_STALE_US 250000

typedef struct RenHdcEngine RenHdcEngine;

typedef struct {
    uint32_t struct_size;
    int32_t controller_type;
    int32_t bt_level;
    bool bt_after_input;
    bool bt_fast;
    bool usb_no_queue;
    bool clamp_amplitudes;
    bool zero_stale_half;
    float amp_gain;
    float high_freq_scale;
    uint32_t flags;
    uint8_t reserved[32];
} RenHdcSettings;

static struct {
    int tried;
    SDL_SharedObject *dll;
    RenHdcEngine *(*create)(bool, bool);
    void (*destroy)(RenHdcEngine *);
    int32_t (*defaults)(RenHdcSettings *);
    int32_t (*apply)(RenHdcEngine *, const RenHdcSettings *);
    int32_t (*push)(RenHdcEngine *, int64_t, bool, float, float, float, float);
    int32_t (*tick)(RenHdcEngine *, int64_t, uint8_t *);
} rh;

static int rh_load(void) {
    if (rh.tried) return rh.create != NULL;
    rh.tried = 1;
    const char *why = NULL;
    rh.dll = SDL_LoadObject("renhdrum.dll");
    if (!rh.dll) {
        why = "renhdrum.dll not found";
    } else {
#define RH_FN(f, name) \
        *(SDL_FunctionPointer *)&rh.f = SDL_LoadFunction(rh.dll, name); \
        if (!rh.f && !why) why = name " missing"

        RH_FN(create, "renhdrum_engine_create");
        RH_FN(destroy, "renhdrum_engine_destroy");
        RH_FN(defaults, "renhdrum_settings_default");
        RH_FN(apply, "renhdrum_engine_apply_settings");
        RH_FN(push, "renhdrum_engine_push_half_sample");
        RH_FN(tick, "renhdrum_engine_tick");
#undef RH_FN
    }
    if (why) {
        rh.create = NULL;
        SDL_Log("renhdrum: disabled (%s)", why);
    }
    return rh.create != NULL;
}
```

---

## 3. 1 ms Tick Thread, 250 ms Silence Watchdog & Lock Ordering

Because physical HD Rumble requires steady packet pacing (15 ms intervals for USB Pro Controller) regardless of game rendering framerates, the runtime dispatches a dedicated 1 ms background worker thread.

### 3.1 Silence Watchdog (250 ms)

When a game pauses, displays a static menu, or hits a loading screen, guest vibration calls cease immediately without issuing an explicit zero-amplitude packet `(0.0, 160.0, 0.0, 320.0)`.

To prevent motors from buzzing indefinitely, the tick thread checks elapsed time for each side: if no update has arrived for longer than `RH_STALE_US` (250,000 microseconds), it injects a neutral silence sample `(0.0f, 160.0f, 0.0f, 320.0f)`.

### 3.2 Strict Lock Ordering

To prevent deadlocks between the host controller mutex and the engine mutex:
- **Lock Hierarchy**: `host_lock` -> `rh_lock`.
- **Golden Rule**: Never acquire `host_lock` while holding `rh_lock`.
- **Detaching Cleanly**: In `rh_detach()`, release `host_lock` before joining the tick worker thread (the worker may be waiting for `host_lock` to complete an in-flight `SDL_SendJoystickEffect`). Once joined, re-acquire `host_lock`, transmit a standard silence packet, and destroy the engine.

### 3.3 Implementation Example

```c
static RenHdcEngine *rh_engine;          // Guarded by rh_lock
static SDL_Joystick *rh_joy;             // Guarded by host_lock
static SDL_Mutex *rh_lock;
static SDL_Thread *rh_thread;            // Guarded by host_lock
static int rh_run = 0;
static int rh_detaching = 0;
static int64_t rh_at[2];                 // Guarded by rh_lock: timestamp in microseconds
static int rh_muted[2];                  // Guarded by rh_lock: muted state flag

static int rh_worker(void *data) {
    (void)data;
    uint8_t pkt[10];
    while (SDL_GetAtomicInt((SDL_AtomicInt *)&rh_run)) {
        int64_t now_us = (int64_t)(SDL_GetTicksNS() / 1000);
        int send_ready = 0;

        SDL_LockMutex(rh_lock);
        if (rh_engine) {
            for (int s = 0; s < 2; s++) {
                if (rh_at[s] && !rh_muted[s] && (now_us - rh_at[s] > RH_STALE_US)) {
                    rh.push(rh_engine, now_us, s == 1, 0.0f, 160.0f, 0.0f, 320.0f);
                    rh_muted[s] = 1;
                }
            }
            send_ready = rh.tick(rh_engine, now_us, pkt);
        }
        SDL_UnlockMutex(rh_lock);

        if (send_ready == 1) {
            SDL_LockMutex(host_lock);
            if (rh_joy) {
                SDL_SendJoystickEffect(rh_joy, pkt, 10);
            }
            SDL_UnlockMutex(host_lock);
        }
        SDL_DelayNS(1000000); // 1 ms pacing
    }
    return 0;
}

// Call under host_lock when a supported controller is opened
void rh_attach(SDL_Gamepad *pad) {
    const char *off = SDL_getenv("RECOMP_RENHDRUM");
    if (off && strcmp(off, "0") == 0) return;

    // Check for Switch Pro Controller (057E:2009)
    if (SDL_GetGamepadVendor(pad) != 0x057E || SDL_GetGamepadProduct(pad) != 0x2009) return;
    if (!rh_load()) return;

    bool is_bt = (SDL_GetGamepadConnectionState(pad) == SDL_JOYSTICK_CONNECTION_WIRELESS);
    RenHdcEngine *e = rh.create(is_bt, true);
    RenHdcSettings s;
    if (!e || rh.defaults(&s) != 0) {
        if (e) rh.destroy(e);
        return;
    }

    // Standard SDL3 without custom transport patches
    s.usb_no_queue = false;
    s.bt_after_input = false;
    s.bt_fast = false;
    rh.apply(e, &s);

    rh_joy = SDL_GetGamepadJoystick(pad);

    SDL_LockMutex(rh_lock);
    rh_engine = e;
    rh_at[0] = rh_at[1] = 0;
    rh_muted[0] = rh_muted[1] = 0;
    SDL_UnlockMutex(rh_lock);

    SDL_SetAtomicInt((SDL_AtomicInt *)&rh_run, 1);
    rh_thread = SDL_CreateThread(rh_worker, "renhdrum_tick", NULL);
}

// Call under host_lock before closing gamepad
void rh_detach(void) {
    if (!rh_thread) return;
    SDL_Thread *t = rh_thread;
    rh_thread = NULL;
    SDL_SetAtomicInt((SDL_AtomicInt *)&rh_run, 0);

    // Drop host_lock while joining to prevent deadlock with worker
    SDL_UnlockMutex(host_lock);
    SDL_WaitThread(t, NULL);
    SDL_LockMutex(host_lock);

    static const uint8_t stop_pkt[10] = {0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40};
    if (rh_joy) {
        SDL_SendJoystickEffect(rh_joy, stop_pkt, sizeof(stop_pkt));
        rh_joy = NULL;
    }

    SDL_LockMutex(rh_lock);
    if (rh_engine) {
        rh.destroy(rh_engine);
        rh_engine = NULL;
    }
    SDL_UnlockMutex(rh_lock);
}

// Public API called when guest sends vibration value
int rh_push_sample(int side, float al, float fl, float ah, float fh) {
    if (side < 0 || side > 1) return 0;
    int handled = 0;

    SDL_LockMutex(rh_lock);
    if (rh_engine) {
        int64_t now_us = (int64_t)(SDL_GetTicksNS() / 1000);
        rh.push(rh_engine, now_us, side == 1, al, fl, ah, fh);
        rh_at[side] = now_us;
        rh_muted[side] = 0;
        handled = 1;
    }
    SDL_UnlockMutex(rh_lock);
    return handled;
}
```

---

## 4. Pro Controller Reliability Fixes (Usable Even Without HD Rumble)

Empirical testing across static recompilation projects uncovered two hardware and driver quirks specific to the Nintendo Switch Pro Controller on Windows. These fixes improve control stability regardless of whether HD Rumble is active.

### 4.1 Right Stick Upward Drift Fix (`|offset| > 0.5f -> 0.0f`)

#### The Problem
During analog stick calibration or when switching between standard HID reports and full-report sub-commands, stale calibration offsets (such as `offset_y: 0.939940`) can become permanently trapped in configuration memory. The runtime treats this spurious offset as genuine center position, causing the in-game camera to slowly pan upward continuously.

#### The Fix
When initializing stick axes or processing analog properties, validate that the calibrated center offset does not exceed a reasonable deadzone threshold (0.50f). If it exceeds 0.50f, clamp it to neutral zero:

```c
// When setting up analog stick mapping parameters:
if (fabsf(offset_x) > 0.5f) {
    offset_x = 0.0f;
}
if (fabsf(offset_y) > 0.5f) {
    offset_y = 0.0f;
}
```

### 4.2 Stuck Left Stick / Errant Button Inputs on Exit (`ClearUSB` / `SDL_CloseGamepad`)

#### The Problem
Many static recompilation runtimes terminate execution quickly via `plat_exit` (which calls `ExitProcess`) without executing standard controller cleanup.

When connected via USB cable, the Switch Pro Controller operates in high-rate proprietary reporting mode (`0x30`). If the host process exits abruptly without issuing a teardown command, the controller remains trapped in mode `0x30`. The Windows generic HID driver interprets these raw proprietary packets as standard DirectInput axes, causing:
- Left analog stick locked at extreme upper position (`Y = 9472` instead of neutral `32767`).
- Errant, continuous button cycling across the desktop.
- Requires physically unplugging and replugging the USB cable to reset the gamepad.

#### The Fix
1. **Always invoke `SDL_CloseGamepad` prior to process termination**: Never bypass controller shutdown in fast exit paths.
2. **Dispatch `ClearUSB` in SDL HIDAPI Driver**:
   In `src/joystick/hidapi/SDL_hidapi_switch.c` inside `HIDAPI_DriverSwitch_CloseJoystick`:

```c
// Ensure the controller leaves proprietary reporting mode
WriteProprietary(ctx, k_eSwitchProprietaryCommandIDs_ClearUSB, NULL, 0, false);
```

---

## 5. Validation Status & Turnkey Reference Kit

The integration architecture and reliability fixes described above have been empirically validated on physical hardware across static recompilation projects:
- **`Hookshot (v2.0.6)`** (OoT Switch 2 Recompiler): Confirmed fully functional authentic HD Rumble on **Nintendo Switch Pro Controller (Bluetooth Level 2)** and **Sony DualSense (4-channel 48 kHz float audio haptics via wired USB)** on physical hardware (November 9, 2026). Incorporates automatic `renhdrum_set_nx2_mode(2)` initialization for Switch 2 waveform curves.
- **`totk-pc-recomp (v0.3.7)`**: Confirmed functional HD Rumble with wired Switch Pro Controller (dynamic loading, 1 ms tick thread, and silence watchdog).
- **`LASRecomp`**: Verified runtime bridge operational at stable 60 FPS with controller recognition.

A ready-to-use drop-in bridge implementation and reference patch for Hookshot v2.0.6 are available in [`integrations/recomp-c/`](../../integrations/recomp-c/).

