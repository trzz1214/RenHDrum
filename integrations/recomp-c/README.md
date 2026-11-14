# Generic Recomp C Bridge for RenHDrum

A drop-in C/C++ bridge connecting Nintendo Switch static recompilation projects directly to **RenHDrum** (`renhdrum.dll`), enabling authentic, low-latency **HD Rumble** for **Nintendo Switch Pro Controllers** and **Sony DualSense** controllers.

---

## 1. Overview

Static recompilation projects (such as Hookshot, TotKPort, or LASRecomp) translate Nintendo Switch AArch64 binaries into native x86-64 code. While this delivers near-native CPU performance and minimal frame latency, games typically route vibration commands (`nn::hid::SendVibrationValue`) through fallback rumble APIs such as `SDL_RumbleGamepad`, collapsing four-parameter wideband HD Rumble waves into coarse, two-motor ERM vibration.

This bridge restores authentic HD Rumble by dynamically loading `renhdrum.dll` and feeding raw Switch vibration packets into the RenHDrum engine (newer Hookshot releases from `v2.1.0` onward already include native HD Rumble for Pro and Joy-Con controllers; this bridge keeps per-pad compatibility while adding DualSense haptics and onset punch):
- **Nintendo Switch Pro Controller (USB & Bluetooth):** Synthesizes true 10-byte sub-command packets (`0x10`) with 1 ms timing resolution and 40 ms wired packet resend. On Bluetooth, it defaults to **Bluetooth Level 4 (Compatible)** to keep input responsive while preserving wideband detail.
- **Sony DualSense / DualSense Edge (USB & Bluetooth):** Transmits wideband voice-coil audio haptics over a dedicated 4-channel 48 kHz float audio stream on USB, and transmits 142-byte vendor output reports (`0x32`) every 10.667 ms over a secondary HID handle on Bluetooth.
- **Switch 2 Waveform Compensation:** Calls `renhdrum_set_nx2_mode(2)` at initialization to apply perceptually balanced curves tailored for Switch 2 audio-range vibration commands.
- **Strict DLL Version Check & Zero-Crash Fallback:** Loads `renhdrum.dll` beside the executable via `LoadLibraryExW` and verifies a matching `major.minor` branch from `renhdrum_version()`. If `renhdrum.dll` is absent, incompatible, or `HOOKSHOT_RENHDRUM=0` is set, the bridge cleanly yields control back to Hookshot's built-in rumble.

---

## 2. Directory Structure

```
integrations/recomp-c/
├── bridge/
│   ├── renhdrum_recomp_bridge.c   # Self-contained worker thread, DSP loader, and device manager
│   └── renhdrum_recomp_bridge.h   # Minimal public C interface
├── reference/
│   └── recomp-c-reference.patch   # Complete reference patch against Hookshot v2.1.0 (GPL-3.0)
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

// In pads_poll() loop, before Hookshot's hd_poll() and rumble_due():
if (rh_pad_claimed(i)) {
    // RenHDrum owns slot i; skip both Hookshot HD Rumble and SDL motor rumble on this pad only
    continue;
}
```

### Step 4: Intercept Guest Vibration Calls
In your guest HLE HID implementation (e.g., `nn_hid.c:vibration_send`), before `rumble_map` / `hos_sdl_rumble_hd`:

```c
// Pass original unscaled float parameters: al, fl, ah, fh.
// Returns 1 only when RenHDrum claims every connected pad, so unclaimed secondary pads still rumble.
if (rh_vibration(dev, v[0], v[1], v[2], v[3])) {
    return;
}

// Fallback to Hookshot's rumble_map / hos_sdl_rumble_hd / hos_sdl_rumble when unclaimed or inactive
```

---

## 4. Build System & Packaging Recommendations

Based on real-world testbed experience with Hookshot v2.1.0:

1. **Automatic DLL Bundling:** Ensure your packaging scripts (e.g., `make_game.py` or CMake install targets) place `renhdrum.dll`, `SDL2.dll`, and `SDL3.dll` directly beside the game executable (`hookshot.exe`). This provides zero-configuration HD Rumble out of the box.
2. **FFmpeg / CPM Package Resolution:** When compiling host modules that decode video cutscenes (such as `nn_video.c`), ensure CPM cache headers (`libavcodec/avcodec.h`) are resolved properly via the `RX_CPM` environment variable or a `cpm` junction pointing to cached packages.
3. **Pro Controller Stick Centering Filter:** Pro Controllers on PC can occasionally transmit anomalous calibration offsets (`|offset| > 0.5f`). Hookshot and the bridge keep per-pad rumble isolated so other controllers remain unaffected.

---

## 5. Reference Implementation

The file [`reference/recomp-c-reference.patch`](reference/recomp-c-reference.patch) contains a turnkey implementation for **Hookshot v2.1.0** (OoT Switch 2 Recompiler).

To apply the patch to Hookshot v2.1.0:
```bash
git checkout v2.1.0
git apply integrations/recomp-c/reference/recomp-c-reference.patch
```

---

## 6. License

- The bridge implementation (`renhdrum_recomp_bridge.c`, `renhdrum_recomp_bridge.h`) is licensed under **MIT OR Apache-2.0**.
- The reference patch (`reference/recomp-c-reference.patch`) targets Hookshot, which is licensed under **GPL-3.0**. Credit to **NotRexed** and the Hookshot contributors.
