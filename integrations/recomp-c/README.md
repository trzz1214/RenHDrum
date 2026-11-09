# Generic Recomp C Bridge for RenHDrum

A drop-in C/C++ bridge connecting Nintendo Switch static recompilation projects directly to **RenHDrum** (`renhdrum.dll`), enabling authentic, low-latency **HD Rumble** for **Nintendo Switch Pro Controllers** and **Sony DualSense** controllers.

---

## 1. Overview

Static recompilation projects (such as Hookshot, TotKPort, or LASRecomp) translate Nintendo Switch AArch64 binaries into native x86-64 code. While this delivers near-native CPU performance and minimal frame latency, games typically route vibration commands (`nn::hid::SendVibrationValue`) through fallback rumble APIs such as `SDL_RumbleGamepad`, collapsing four-parameter wideband HD Rumble waves into coarse, two-motor ERM vibration.

This bridge restores authentic HD Rumble by dynamically loading `renhdrum.dll` and feeding raw Switch vibration packets into the RenHDrum engine:
- **Nintendo Switch Pro Controller (USB & Bluetooth):** Synthesizes true 10-byte sub-command packets (`0x10`) with 1 ms timing resolution. On Bluetooth, it automatically enforces **Bluetooth Level 2** (45 ms spacing, 3-packet burst, 80 ms refill) to prevent RF congestion and eliminate input button latency.
- **Sony DualSense / DualSense Edge (USB & Bluetooth):** Transmits wideband voice-coil audio haptics over a dedicated 4-channel 48 kHz float audio stream on USB, and transmits 142-byte vendor output reports (`0x32`) every 10.667 ms over a secondary HID handle on Bluetooth.
- **Switch 2 Waveform Compensation:** Calls `renhdrum_set_nx2_mode(2)` at initialization to apply perceptually balanced curves tailored for Switch 2 audio-range vibration commands.
- **Zero-Crash Fallback:** If `renhdrum.dll` is absent or `HOOKSHOT_RENHDRUM=0` is set in the environment, the bridge cleanly yields control back to standard emulator rumble without throwing errors.

---

## 2. Directory Structure

```
integrations/recomp-c/
├── bridge/
│   ├── renhdrum_recomp_bridge.c   # Self-contained worker thread, DSP loader, and device manager
│   └── renhdrum_recomp_bridge.h   # Minimal public C interface
├── reference/
│   └── recomp-c-reference.patch   # Complete reference patch against Hookshot v2.0.6 (GPL-3.0)
└── README.md
```

---

## 3. Integration Guide

### Step 1: Add the Bridge to Your Host Sources
Copy `renhdrum_recomp_bridge.c` and `renhdrum_recomp_bridge.h` into your project's host runtime directory (e.g., `host/`). Ensure `renhdrum.h` from `include/renhdrum.h` is accessible in your include path.

### Step 2: Initialize at Host Startup
In your main initialization routine (e.g., `hos_main.c` or `main.c`):

```c
#include "renhdrum_recomp_bridge.h"

// Dynamically loads renhdrum.dll and initializes Switch 2 curve parameters
rh_load();
```

Register `rh_cleanup` at process exit (e.g. via `atexit(rh_cleanup)`) to ensure all background worker threads terminate and controller handles are released cleanly.

### Step 3: Hook Controller Connect and Disconnect
In your SDL gamepad management code (e.g., `pad_sdl.c`):

```c
// When a gamepad is opened:
rh_attach(gamepad);

// When a gamepad is closed:
rh_detach(gamepad);

// When sending fallback motor rumble:
if (rh_is_active(gamepad)) {
    // RenHDrum is actively driving voice-coil/HD Rumble; suppress motor rumble
    return;
}
```

### Step 4: Intercept Guest Vibration Calls
In your guest HLE HID implementation (e.g., `nn_hid.c:vibration_send`):

```c
// Pass original unscaled float parameters: al, fl, ah, fh
if (rh_vibration(side, al, fl, ah, fh)) {
    // Successfully handled by RenHDrum; skip legacy SDL_RumbleGamepad
    return;
}

// Fallback to legacy SDL motor rumble if RenHDrum is inactive
```

---

## 4. Build System & Packaging Recommendations

Based on real-world testbed experience with Hookshot v2.0.6:

1. **Automatic DLL Bundling:** Ensure your packaging scripts (e.g., `make_game.py` or CMake install targets) place `renhdrum.dll`, `SDL2.dll`, and `SDL3.dll` directly beside the game executable (`hookshot.exe`). This provides zero-configuration HD Rumble out of the box.
2. **FFmpeg / CPM Package Resolution:** When compiling host modules that decode video cutscenes (such as `nn_video.c`), ensure CPM cache headers (`libavcodec/avcodec.h`) are resolved properly via the `RX_CPM` environment variable or a `cpm` junction pointing to cached packages.
3. **Pro Controller Stick Centering Filter:** Pro Controllers on PC can occasionally transmit anomalous calibration offsets (`|offset| > 0.5f`). The bridge includes a zero-filter to eliminate virtual stick drift when closing USB interfaces.

---

## 5. Reference Implementation

The file [`reference/recomp-c-reference.patch`](reference/recomp-c-reference.patch) contains a fully tested, turnkey implementation for **Hookshot v2.0.6** (OoT Switch 2 Recompiler).

To apply the patch to Hookshot v2.0.6:
```bash
git checkout v2.0.6
git apply integrations/recomp-c/reference/recomp-c-reference.patch
```

---

## 6. License

- The bridge implementation (`renhdrum_recomp_bridge.c`, `renhdrum_recomp_bridge.h`) is licensed under **MIT OR Apache-2.0**.
- The reference patch (`reference/recomp-c-reference.patch`) targets Hookshot, which is licensed under **GPL-3.0**. Credit to **NotRexed** and the Hookshot contributors.
