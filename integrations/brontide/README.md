# Brontide Reference Integration Patches

This directory contains a 17-part reference patch series (0002-0018) demonstrating how to integrate `renhdrum.dll` into an Eden / Yuzu C++ SDL3 emulator codebase at base commit `989768aa46`.

For Switch 2 titles (IPC command 214) on a PokeDinner / Eglantine style base, see the separate [`../brontide-ns2/`](../brontide-ns2/) series. The in-emulator dialog source is in [`studio/`](studio/).

## Patch sequence

| # | Patch | What it does |
|---|---|---|
| 0002 | `input-RenHDrum-HD-Rumble-thin-hook-SDL3-patch-stereo` | Thin hook in `sdl_driver.cpp`: loads the `renhdrum.dll` C ABI, latches left/right half-samples into stereo submissions. |
| 0003 | `input-DualSense-haptics-audio-init-SDL-audio-require` | 4-channel 48 kHz float32 SDL audio stream for DualSense over USB; retries while the Windows audio endpoint appears. |
| 0004 | `input-DualSense-haptics-zero-buffer-on-DSP-error-log` | Zeroes the audio buffer if the DSP reports an error or underflow. |
| 0005 | `input-lock-RenHDrum-engine-DSP-against-controller-di` | Mutexes around the engine and DualSense DSP state during hotplug and disconnect. |
| 0006 | `input-SDL3-idle-gated-HD-Rumble-writes-USB-no-queue-` | Connects the patched SDL3 idle-gated entry points (no-queue USB, Bluetooth after input) to emulator settings. |
| 0007 | `input-Ryuhdr-0.2.6-recommended-HD-Rumble-mode-by-def` | Recommended default mode: wired 15 ms with full AM/FM envelope; Bluetooth level 3 with write-after-input gating. |
| 0008 | `ui-detailed-HD-Rumble-dialog-per-controller-connecti` | Hook side of the per-controller dialog (connection mode, DualSense section). |
| 0009 | `ui-Ryuhdr-style-performance-overlay-and-overlay-pill` | Hook side of the performance overlay and its pill. |
| 0010 | `ui-HD-Rumble-dialog-in-glass-with-gold-cyan-accents-` | Hook side of the restyled dialog (extra status fields). |
| 0011 | `input-DualSense-voice-coil-haptics-over-Bluetooth-vi` | DualSense voice-coil haptics over Bluetooth: a second HID handle and sender thread for report `0x32`. |
| 0012 | `input_common-wire-Xbox-One-S-rumble-mapper-into-SDLD` | Wires the Xbox One S four-motor rumble translator into `SDLDriver`. |
| 0013 | `input_common-tick-rumble-outside-joystick_map_mutex` | Runs the rumble tick outside `joystick_map_mutex`. |
| 0014 | `input_common-pass-band-frequencies-to-Xbox-rumble-ma` | Passes the low/high band frequencies to the Xbox translator. |
| 0015 | `Xbox-follows-the-main-HD-Rumble-switch-plain-rumble-` | Xbox follows the main HD Rumble switch; plain rumble leaves the HLE thread. |
| 0016 | `sdl3-release-a-wired-Switch-pad-back-to-the-OS-on-cl` | SDL3 patch: sends the USB clear command on close so a wired Pro Controller returns to the OS. |
| 0017 | `Xbox-One-Rumble-page-wired-detection-from-SDL-connec` | Wired Xbox detection from SDL connection state for the status page. |
| 0018 | `HD-Rumble-dialog-one-main-switch-controller-sidebar-` | Hook side of the single main switch, controller sidebar and apply-on-Done behaviour. |

> [!NOTE]
> Patches 0008, 0009, 0010 and 0018 are named `ui` but contain only their `input_common` part (`sdl_driver.cpp`, `renhdrum_pads.h`). The Qt dialog itself is not a patch: see the reference code in [`studio/`](studio/).

## Applying patches

```bash
cd /path/to/eden
git checkout 989768aa46
git am /path/to/RenHDrum/integrations/brontide/*.patch
```

All patches apply in order on a clean checkout of that commit. A build of `renhdrum.dll` from this repository is required (`cargo build --release`).

## HD Rumble Studio

[`studio/`](studio/) holds the in-emulator HD Rumble dialog (Qt), widgets and thunder effect as **reference code**, plus `renhdrum_pads.h`. It depends on Qt and Brontide's theme, so it is meant to be read and adapted, not applied. See [`studio/README.md`](studio/README.md).

## Switch 2 titles

[`../brontide-ns2/`](../brontide-ns2/) carries the command 214 dispatcher and the same thin hook for a PokeDinner 2.5.0 base. Its patches are independent of the series above.

## License

The patches modify GPL emulator code and are distributed under the GPL, see [LICENSE](LICENSE).
