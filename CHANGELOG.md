# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-11-11

### Fixed
- A NaN amplitude could play as full-strength rumble on Switch pads in Switch 2 mode.

### Changed
- DualSense slot peak-hold now applies only in Switch 2 mode; Switch 1 games are back to last-write-wins, matching Ryuhdr. The 0.3.0 notes said Switch 1 was untouched; with a DualSense that was not accurate.

### Added
- Xbox One S pads now get a Switch 2 amplitude curve in Switch 2 mode (first version, research).

## [0.3.0] - 2026-11-08

### Added
- **Per-Controller Switch 2 Rumble Presets**: Calibrated piecewise transfer curves per controller hardware under Switch 2 mode (`renhdrum_set_nx2_mode(2)`, receiving guest IPC command 214 `SendVibrationValuesInMode`). DualSense audio haptics apply preset `[3]` (`T = 0.001, P1 = 0.004, P2 = 0.550, P3 = 0.900, P4 = 1.000`) designed for voice-coil response; Nintendo Switch Pro Controller 1 applies preset `[4]` (`T = 0.001, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950`) combined with an automated +20% (1.2x) attack transient boost for 25 ms on low frequencies below 150 Hz. Classic Switch 1 rumble (`mode == 0`) remains 100% bit-exact and untouched (B028).
- **Public C ABI Mode API**: Added `renhdrum_set_nx2_mode(int32_t mode)` in `renhdrum.h` and `renhdrum.dll` for host runtimes to notify the engine when Switch 2 rumble translation is active.
- **Static Recompilation Games Guide**: Integration guide (`docs/integration/recomp-games.md`) for C and C++ SDL3 static recompilation runtimes, detailing dynamic runtime loading via `SDL_LoadObject`, 1 ms tick thread, 250 ms silence watchdog, lock ordering, and standalone Pro Controller drift/exit fixes.
- **Generic Recomp C Bridge & Hookshot Reference Testbed**: Published `integrations/recomp-c/` providing a drop-in C bridge (`bridge/renhdrum_recomp_bridge.c/.h`) and a turnkey reference patch (`reference/recomp-c-reference.patch`) for Hookshot v2.0.6 (OoT Switch 2 Recompiler). Enables authentic HD Rumble on Nintendo Switch Pro Controller (USB & Bluetooth Level 2) and Sony DualSense (USB 4-channel 48 kHz float audio haptics & Bluetooth report 0x32) with zero configuration.
- **Switch 2 Trace Quantification & Monograph 11 Update**: Documented quantitative findings from 25 in-game trace sessions in `docs/research/11-switch2-hd-rumble.md` and updated `docs/integration/switch2-games.md` with command 213 dispatcher implementation details.
- **Brontide-NS2 1.1.0 & Brontide-oot Integration Patches**: Updated `integrations/brontide-ns2/` with patch 0005 adding real command 213 unpacking and dispatching, mode routing, and Pro Controller reliability fixes.

### Changed
- **DualSense 5.3 ms Peak Hold Window**: DualSense DSP retains attack peaks within a 5.3 ms slot to prevent transient clipping and preserve high-frequency tactile definition (B026).
- **Pro Controller Bluetooth Default Level 2**: Pro Controller over Bluetooth now defaults to Balanced level 2 (`HD_BT_LEVELS[2]`: 45 ms pacing, burst 3, recharge 80 ms, `bt_fast = false`) for optimal radio balance between input latency and haptic fidelity (B027).
- `RENHDRUM_VERSION` is now `0x000300`.

### Acknowledgments
- Credit to **samaBR85** ([`renhdrum-eden`](https://github.com/samaBR85/renhdrum-eden)) for independent research, porting patches to Eglantine/Lichii, deadlock fixes in Raw Input, and early Switch 2 trace measurements that informed the rumble scaling model.
- Credit to **Frizti** (Fritchii v5.5), **zortos293** (Lichii), and **Dark** (Dynamic FPS) for providing the specialized Switch 2 platform used for Brontide-oot.
- Credit to **NotRexed** and the **Hookshot Contributors** ([`NotRexed/Hookshot`](https://github.com/NotRexed/Hookshot)) for authoring the reference static recompilation platform used for `integrations/recomp-c/`.

## [0.2.1] - 2026-11-06

### Added
- **Switch 2 HD Rumble Research & Integration Docs**: Monograph 11 (`docs/research/11-switch2-hd-rumble.md`) analyzing Switch 2 guest IPC vibration commands (`nn::hid::IHidServer` 213, 214, 220), signal routing across DualSense, Switch Pro Controller 1, and Xbox One, root cause of the 2000 ms hanging rumble defect, and early hardware limits.
- **Switch 2 Games Integration Guide**: Integration manual (`docs/integration/switch2-games.md`) detailing command 214 dispatching (credited to neo Luigi / Eglantine PR #155) and routing through RenHDrum's public `_half_` C ABI endpoints.
- **Brontide-NS2 Reference Testbed**: Documented reference test build (0.1.0 beta) based on PokeDinner 2.5.0 by Franseven, validating HD Rumble playback in retail Switch 2 titles.

### Integrations
- **Brontide hook series extended** (`integrations/brontide/`): 11 more patches (0008-0018) on top of 0007: DualSense voice-coil haptics over Bluetooth, Xbox One S rumble wiring, releasing a wired Pro Controller back to the OS on close, the per-controller HD Rumble dialog hooks, and the main on/off switch. Patches 0008, 0009, 0010 and 0018 are named `ui` but only contain their `input_common` part; the dialog itself is in `studio/`. All apply cleanly in order on Eden `989768aa46`.
- **HD Rumble Studio reference code** (`integrations/brontide/studio/`): the Qt dialog, widgets, thunder effect and `renhdrum_pads.h`, as reference code (not a patch).
- **Brontide-NS2 patch series** (`integrations/brontide-ns2/`): command 214 dispatcher (credit: neo Luigi, Eglantine PR #155) plus the RenHDrum thin hook, 4 patches on commit `216b5034c6`, GPL.

### Changed
- `RENHDRUM_VERSION` is now `0x000201`.
- Documentation: the "known limitation" notes about the settings resolver no longer promise a version.
## [0.2.0] - 2026-11-04

### Added
- **Xbox One Rumble**: translates Switch HD Rumble into the four motors of a Microsoft Xbox One controller (left grip, right grip, left trigger, right trigger). Eight kinds of rumble, a rotor model with overdrive, a trigger gate, and a melody that travels left to right. New C ABI: `renhdrum_xbox_rumble_create`, `_destroy`, `_push_half_sample`, `_tick`, `_resend` (`src/xbox/`). Report: `docs/research/10-xbox-one-rumble.md`; guide: `docs/integration/xbox.md`. First shipped in Brontide 1.2.0.

### Changed
- `RENHDRUM_VERSION` is now `0x000200`.

## [0.1.1] - 2026-10-30

### Added
- **DualSense Bluetooth haptics (HID output report `0x32`)**: `renhdrum_dualsense_dsp_build_bt_report` packs the 48 kHz DSP output into 32 stereo frames of signed 8-bit PCM at 3 kHz, with sequence counter and CRC32 (seed `0xA2`), for one report every 10.667 ms. Zero heap allocation (`src/dualsense/bt_haptics.rs`). Report layout learned from [SAxense](https://github.com/egormanga/SAxense) by egormanga (MPL-2.0); implementation written from scratch. First shipped in Brontide 1.1.0.
- **`dualsense_bt_crisp_notes`** in `RenHdcConfig`: crisp notes over Bluetooth, separate from USB and off by default (the 3 kHz stream is coarser than 48 kHz). Appended at the end of the struct; `sizeof(RenHdcConfig)` is unchanged (48 bytes), so hosts built against 0.1.0 keep working.

### Changed
- `RENHDRUM_VERSION` is now `0x000101`.

## [0.1.0] - 2026-10-25

### Added
- **Multi-sample 5 ms AM/FM Envelope & Crisp Notes**: Real-time tactile synthesis supporting `FormatThree` (3x5-bit), `FormatThree7bit` (3x7-bit), and `FormatTwo14bit` (2x14-bit) within 15 ms HID transmission frames, plus crisp attack accentuation (+3 dB / -3 dB) and subtle sub-5-bit low-amplitude floor fallback.
- **Congestion-Free Bluetooth Pacing**: Zero-lag pacing with 150 ms idle refresh, Write-After-Input gating, and a 3-tier Token Bucket scheduler (`HD_BT_LEVELS`: Stable 60 ms, Balanced 45 ms, Quality 35 ms) mitigating half-duplex Bluetooth saturation.
- **DualSense 4-Channel 48 kHz Audio Haptics DSP**: Real-time VCM coil haptics synthesis over USB (channels 3 & 4), 4 ms sliding low-pass frequency filter, non-linear power curve, 0.70 power ceiling, and fixed 256-frame crisp chunk processing.
- **Zero-Allocation C ABI**: Compact C header `include/renhdrum.h` providing stateless encoders, stateful `RenHdcEngine`, and `RenHdcDualSenseDsp` with strictly zero dynamic heap allocations in inner tick and push loops.
- **Upstream SDL3 Integration Patches**: Patch set for `libsdl-org/SDL` (`release-3.4.8`) adding non-blocking idle-gated rumble writes (`SDL_HDRumbleSetNoQueue`, `SDL_HDRumbleSetNoQueueBluetooth`, `SDL_HDRumbleSetBluetoothAfterInput`).
- **Brontide Reference Integration Patches**: Reference thin-hook patch series integrating `renhdrum.dll` into C++ SDL3 emulator runtimes.
- **Technical Documentation & Examples**: Comprehensive integration guides, research monographs, and minimal C / C# host examples.

### Known Limitations
- In `0.1.0`, `renhdrum_settings_resolve` and `renhdrum_dualsense_dsp_create` prioritize reading local override flag files adjacent to the host executable and IPC shared memory / configuration files managed by `Brontide-App` (`Local\RenHDrum_SharedState_v1` and `%APPDATA%\RenHDrum\renhdrum-settings.txt`). A fully pure C ABI configuration injection path (bypassing IPC and local disk files) is planned for `0.2.0`.
