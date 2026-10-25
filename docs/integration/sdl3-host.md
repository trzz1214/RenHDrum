# Integrating RenHDrum with a C++ / SDL3 Host

This guide explains how to integrate `renhdrum.dll` into a C / C++ emulator or game engine using SDL3 (`libsdl-org/SDL`).

---

## 1. Separation of Concerns: Algorithm vs. Transport

High-fidelity HD Rumble requires two cooperating layers:
1. **RenHDrum Core Engine (`renhdrum.dll`)**: Computes 5 ms multi-sample envelopes, frequency switching, crisp note transients, and token-bucket pacing. It operates in pure memory and outputs standard 10-byte Subcommand `0x10` packets.
2. **Patched SDL3 Transport (`SDL3.dll`)**: Dispatches raw packets to controller hardware without blocking the host thread or colliding with incoming HID input reports.

```
+-------------------------------------------------------------+
|                      Host Application                       |
|   (Emulation core pushes rumble samples at 60-120+ Hz)      |
+------------------------------+------------------------------+
                               |
                               v
                     +-------------------+
                     |  RenHDrum Engine  |
                     |  (renhdrum.dll)   |
                     +---------+---------+
                               |
               Emits 10-byte   | Subcommand 0x10
                               v
                     +-------------------+
                     | Patched SDL3 Core |
                     |    (SDL3.dll)     |
                     +---------+---------+
                               |
          +--------------------+--------------------+
          |                                         |
          v USB (15 ms)                             v Bluetooth (Token Bucket)
+-----------------------+                 +-----------------------+
|  Idle-Gated No-Queue  |                 |   Write-After-Input   |
|   Non-blocking HID    |                 |   Non-colliding link  |
+-----------------------+                 +-----------------------+
```

---

## 2. Upstream SDL3 Patches

Upstream SDL3 (`libsdl-org/SDL` tag `release-3.4.8`, commit `d9d5536704d585616d4db3c8ba3c4ff6fc2757e1`) requires two patches located in `integrations/sdl3/sdl3-patches/`:

| Patch | Key Changes | Purpose |
|---|---|---|
| `0001-hdrumble.patch` | Extends BT refresh to 150 ms; adds `SDL_HDRumbleSetNoQueue` and `SDL_HDRumbleSetBluetoothAfterInput`. | Eliminates Bluetooth packet flooding and synchronizes writes immediately following input reports. |
| `0002-hdrumble-idle-write.patch` | Tracks atomic `rumble_busy`; adds `HDSync_MergeSide` peak merging and `SDL_HDRumbleSetNoQueueBluetooth`. | Prevents HID write queue blocking on USB and allows idle-gated 5 ms framing over Bluetooth. |

### C Function Signatures in Patched SDL3

```c
#ifdef __cplusplus
extern "C" {
#endif

/* Enables non-blocking, idle-gated rumble writes over USB.
   Returns: 1 on success, 0 if rejected, -1 if joystick is not a supported Switch HIDAPI device. */
int SDL_HDRumbleSetNoQueue(SDL_Joystick *joystick, bool enabled);

/* Enables non-blocking, idle-gated 5 ms multi-sample framing over Bluetooth.
   Returns: 1 on success, 0 if rejected, -1 if joystick is not a supported Switch HIDAPI device. */
int SDL_HDRumbleSetNoQueueBluetooth(SDL_Joystick *joystick, bool enabled);

/* Enables Write-After-Input synchronization over Bluetooth.
   Returns: 1 on success, 0 if rejected, -1 if joystick is not a supported Switch HIDAPI device. */
int SDL_HDRumbleSetBluetoothAfterInput(SDL_Joystick *joystick, bool enabled);

#ifdef __cplusplus
}
#endif
```

> [!IMPORTANT]
> **Priority Rule on Bluetooth**: `SDL_HDRumbleSetNoQueueBluetooth` (Idle-Gated 5 ms Framing) and `SDL_HDRumbleSetBluetoothAfterInput` (Write-After-Input) must **never** be enabled simultaneously on the same controller. Idle-gated 5 ms framing takes precedence; if it fails or is disabled, Write-After-Input should be used.

---

## 3. Seven-Step Integration Lifecycle

### Step 1: Initialize Engine on Controller Connect
When a Nintendo Switch controller (`0x057E:0x2009` Pro Controller, `0x057E:0x2006` Joy-Con L, or `0x057E:0x2007` Joy-Con R) is opened:
```cpp
bool is_bluetooth = (SDL_GetJoystickConnectionState(joystick) == SDL_JOYSTICK_CONNECTION_WIRELESS);
bool is_pro = (SDL_GetJoystickProduct(joystick) == 0x2009);

RenHdcEngine* engine = renhdrum_engine_create(is_bluetooth, is_pro);
```

### Step 2: Resolve and Synchronize Settings with SDL Transport
Query desired settings and negotiate capabilities with SDL:
```cpp
RenHdcSettings settings;
renhdrum_settings_default(&settings);

// Configure transport flags in patched SDL3
if (is_bluetooth) {
    if (settings.bt_fast) {
        if (SDL_HDRumbleSetNoQueueBluetooth(joystick, true) != 1) {
            settings.bt_fast = false; // Transport rejected: clear flag in engine
        }
    }
    if (!settings.bt_fast && settings.bt_after_input) {
        if (SDL_HDRumbleSetBluetoothAfterInput(joystick, true) != 1) {
            settings.bt_after_input = false;
        }
    }
} else {
    if (settings.usb_no_queue) {
        if (SDL_HDRumbleSetNoQueue(joystick, true) != 1) {
            settings.usb_no_queue = false;
        }
    }
}

// Apply validated settings back to the engine
renhdrum_engine_apply_settings(engine, &settings);
```

### Step 3: Push Vibration Samples
When the emulator core generates rumble data:
- If both left and right samples are available simultaneously:
  ```cpp
  renhdrum_engine_push_sample(engine, now_us,
      left_al, left_fl, left_ah, left_fh,
      right_al, right_fl, right_ah, right_fh);
  ```
- If the frontend dispatches channels independently, use the atomic stereo latch:
  ```cpp
  renhdrum_engine_push_half_sample(engine, now_us, false /* left */, al, fl, ah, fh);
  renhdrum_engine_push_half_sample(engine, now_us, true  /* right */, al, fl, ah, fh);
  ```

### Step 4: Tick the Engine at 1 ms Intervals
In a high-resolution timer worker (e.g., using `timeBeginPeriod(1)` on Windows):
```cpp
uint8_t packet10[10];
bool should_send = false;

{
    std::lock_guard<std::mutex> lock(engine_mutex);
    if (engine && renhdrum_engine_tick(engine, now_us, packet10) == 1) {
        should_send = true;
    }
}

// Send effect OUTSIDE the engine mutex to avoid deadlock with driver callbacks
if (should_send) {
    SDL_SendJoystickEffect(joystick, packet10, 10);
}
```

### Step 5: Lock Ordering Rules
Always release the engine lock before calling `SDL_SendJoystickEffect`. If the host maintains an overall controller lock, acquire the controller lock first, then the engine lock.

### Step 6: Graceful Disconnect & Destruction
When closing the controller:
```cpp
const uint8_t stop_packet[10] = { 0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40 };

{
    std::lock_guard<std::mutex> lock(engine_mutex);
    if (engine) {
        // Send neutral stop packet before closing
        SDL_SendJoystickEffect(joystick, stop_packet, 10);
        renhdrum_engine_destroy(engine);
        engine = nullptr;
    }
}

SDL_CloseJoystick(joystick);
```

### Step 7: Fallback Path for Other Controllers
Retain standard rumble mechanisms (e.g. `SDL_RumbleJoystick`) for non-Switch hardware (Xbox, standard DirectInput/XInput controllers).

---

## 4. Known Limitation in v0.1.0

In `v0.1.0`, `renhdrum_settings_resolve` reads local flag files adjacent to the host binary and queries IPC shared memory (`Local\RenHDrum_SharedState_v1` / `%APPDATA%\RenHDrum\renhdrum-settings.txt`) managed by `Brontide-App`. 

Hosts wishing to supply custom settings purely in-process without reading IPC or disk flags should directly populate `RenHdcSettings` and invoke `renhdrum_engine_apply_settings`. A pure C ABI configuration resolver that bypasses disk and IPC will be introduced in `v0.2.0`.
