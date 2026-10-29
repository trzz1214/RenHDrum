# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
