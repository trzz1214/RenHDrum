# SDL3 HD Rumble Integration Patches

This directory contains upstream patches for [Simple DirectMedia Layer (SDL)](https://github.com/libsdl-org/SDL). Applies cleanly to every `release-3.4.x` from 3.4.8 to 3.4.20 (checked 2026-11-19), built on 3.4.14 and 3.4.20.

## Overview of Patches

### `0001-hdrumble.patch`
- **Zero-Latency USB Direct Writes**: Allows low-latency rumble packets to bypass buffering bottlenecks when the HID thread is ready.
- **Write-After-Input Timing**: Synchronizes Bluetooth rumble packets immediately after reading an input report, avoiding half-duplex radio collisions.
- **Extended Bluetooth Refresh (150 ms)**: Relaxes the hardcoded 50 ms Bluetooth refresh rate to 150 ms, preventing wireless packet starvation and input lag on Nintendo Switch Pro Controllers and Joy-Cons.
- **New Public C APIs**:
  - `SDL_HDRumbleSetNoQueue(SDL_Joystick *joystick, bool enabled)`
  - `SDL_HDRumbleSetBluetoothAfterInput(SDL_Joystick *joystick, bool enabled)`

### `0002-hdrumble-idle-write.patch`
- **Idle-Gated Non-Blocking Rumble Writes**: Implements atomic `rumble_busy` tracking and peak merging (`HDSync_MergeSide`) so the host never blocks waiting for HID output reports.
- **Bluetooth Idle-Gated 5 ms Framing**: Enables smooth multi-sample packet transmission on Bluetooth links capable of low-latency bursts.
- **New Public C API**:
  - `SDL_HDRumbleSetNoQueueBluetooth(SDL_Joystick *joystick, bool enabled)`

## Applying Patches

To apply these patches to your SDL source tree:

```bash
cd /path/to/SDL
git checkout release-3.4.20
git am /path/to/RenHDrum/integrations/sdl3/sdl3-patches/0001-hdrumble.patch
git am /path/to/RenHDrum/integrations/sdl3/sdl3-patches/0002-hdrumble-idle-write.patch
```
