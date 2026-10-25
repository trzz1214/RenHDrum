# Brontide Reference Integration Patches

This directory contains a 6-part reference patch series demonstrating how to integrate `renhdrum.dll` into an Eden / Yuzu C++ SDL3 emulator codebase at base commit `989768aa46`.

## Patch Sequence Summary

1. **`0002-input-RenHDrum-HD-Rumble-thin-hook-SDL3-patch-stereo.patch`**:
   - Thin-hook C++ integration into `sdl_driver.cpp`.
   - Dynamic loading of `renhdrum.dll` C ABI symbols.
   - Stereo latching mechanism (`renhdrum_engine_push_half_sample`) to handle split left/right vibration submissions from the emulator frontend.
2. **`0003-input-DualSense-haptics-audio-init-SDL-audio-require.patch`**:
   - Initializes 4-channel 48 kHz float32 SDL audio stream for DualSense controllers (`0x054C:0x0CE6` / `0x0DF2`) over USB.
   - Retries audio stream opening if Windows audio endpoint initialization lags behind HID device discovery.
3. **`0004-input-DualSense-haptics-zero-buffer-on-DSP-error-log.patch`**:
   - Robust error handling: zeroes the audio stream output buffer if DSP processing reports an error or underflow.
4. **`0005-input-lock-RenHDrum-engine-DSP-against-controller-di.patch`**:
   - Thread safety: protects `RenHdcEngine` and DualSense DSP state with mutexes during hotplug and disconnect events.
5. **`0006-input-SDL3-idle-gated-HD-Rumble-writes-USB-no-queue-.patch`**:
   - Connects patched SDL3 idle-gated entry points (`SDL_HDRumbleSetNoQueue`, `SDL_HDRumbleSetBluetoothAfterInput`, `SDL_HDRumbleSetNoQueueBluetooth`) to emulator settings.
6. **`0007-input-Ryuhdr-0.2.6-recommended-HD-Rumble-mode-by-def.patch`**:
   - Sets recommended default vibration configuration: wired 15 ms with full AM/FM envelope, in-packet frequency switching, and crisp notes; Bluetooth with token bucket level 3, Write-After-Input gating, and idle 5 ms framing.

## Applying Patches

```bash
cd /path/to/eden
git checkout 989768aa46
git am /path/to/RenHDrum/integrations/brontide/*.patch
```
