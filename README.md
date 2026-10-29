# RenHDrum

[![CI](https://github.com/trzz1214/RenHDrum/actions/workflows/ci.yml/badge.svg)](https://github.com/trzz1214/RenHDrum/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](NOTICE)
[![Website](https://img.shields.io/badge/Official_website-Three--Dola-success)](https://ryuhdr.threedola.workers.dev)

> **📓 This repo is the engine. The story is on the website.**
> [ryuhdr.threedola.workers.dev](https://ryuhdr.threedola.workers.dev) is my online lab notebook about HD Rumble in Switch emulators: how RenHDrum grew from a rumble patch for one emulator into a shared haptics core, what I measured along the way (holding a phone mic to a vibrating controller, Bluetooth packet pacing, DualSense voice coils), the surprises, and the documents worth reading.
> Start here: [the journey](https://ryuhdr.threedola.workers.dev/#ryuhdr) · [how emulators handle HD Rumble](https://ryuhdr.threedola.workers.dev/compare) · [DualSense haptics](https://ryuhdr.threedola.workers.dev/dualsense) · [Two Ancestors: yuzu & Ryujinx](https://ryuhdr.threedola.workers.dev/lineage)

**▶ Want to play? [Download an emulator with RenHDrum built in →](https://ryuhdr.threedola.workers.dev/#download)**
- **Brontide 1.1.0**: DualSense USB + Bluetooth haptics, Switch Pro Controller, built-in HD Rumble Studio.
- **Ryuhdr 0.5.0**: Switch Pro Controller, DualSense USB haptics (Bluetooth coming in 0.5.1).

Found a problem? Use *Report a controller problem* in the emulator's Help menu, or open a [controller problem report](https://github.com/trzz1214/RenHDrum/issues/new?template=controller_problem.md).

RenHDrum is an ultra-low-latency, real-time haptics engine written in safe Rust. It exports a zero-allocation C ABI (`renhdrum.dll`, `include/renhdrum.h`) with zero external crate dependencies, solving three core challenges of modern gaming haptics:

1. **5 ms AM/FM Multi-Sample Envelope & Crisp Notes**: Synthesizes micro-timed 5 ms amplitude and frequency sub-packets (`3x5-bit`, `3x7-bit`, `2x14-bit`) within 15 ms transmission frames, combined with dynamic attack pre-emphasis (+3 dB onset boost and -3 dB sustain decay) for the Nintendo Switch Pro Controller.
2. **Zero-Lag Bluetooth Pacing & Idle-Gating**: Eliminates wireless packet saturation and input report drops via 150 ms idle refresh, Write-After-Input gating, and a 3-tier Token Bucket scheduler (`HD_BT_LEVELS`).
3. **Sony PlayStation 5 DualSense Audio Haptics Synthesizer**: Generates real-time 4-channel 48 kHz float32 audio haptics for DualSense voice coil motors (VCM) over USB, with non-linear x^0.5 perceptual scaling and a calibrated 0.8 output ceiling.

---

## Supported Controllers

| Controller | Status | Connection |
|---|---|---|
| Nintendo Switch Pro Controller | ✅ Supported | USB or Bluetooth |
| Sony DualSense (PS5) | ✅ Supported | **USB:** Brontide 1.1.0, Ryuhdr 0.5.0<br>**Bluetooth:** Brontide 1.1.0 · Ryuhdr 0.5.1 (coming soon) |
| Switch 2 Pro Controller, Steam Controller, Xbox controllers (HD Rumble emulation) | 🔬 Research — not playable yet | — |

Only the two supported controllers get HD Rumble today. Please don't open issues for research controllers yet.

---

## Comparison Table

| Feature / Capability | Standard Emulation (ERM / Raw Queue) | RenHDrum Core Engine |
|---|---|---|
| **Actuator Driving Model** | Coarse ERM rumble conversion or raw 15 ms two-wave peak hold | 5 ms AM/FM multi-sample envelope with in-packet frequency switching |
| **Attack Transient Fidelity** | Sluggish onset due to mechanical actuator inertia | **Crisp Notes** dynamic pre-emphasis (+3 dB attack / -3 dB sustain decay) |
| **Bluetooth Packet Management** | Floods half-duplex radio every 50 ms, causing severe gyro/button lag | **Write-After-Input** + 150 ms refresh + Token Bucket rate limiter |
| **USB Packet Congestion** | Unbounded FIFO queue blocks thread or drops stale bursts | **Idle-Gated No-Queue** non-blocking writes with peak energy merging |
| **PS5 DualSense Actuation** | Basic low-frequency motor rumble emulation | **4-channel 48 kHz PCM DSP** driving stereo VCM voice coils directly |
| **C ABI & Memory Model** | Complex managed dependencies or heap allocations in inner loops | **Zero-allocation** C ABI (`renhdrum.h`), zero external crate dependencies |

---

## Emulators using RenHDrum

The table below lists emulator projects utilizing RenHDrum:

| Emulator | Base & Integration Architecture | Status | Page |
|---|---|---|---|
| **Ryuhdr** | Ryujinx 1.1.1403 (C# / Deep-Patched SDL2, embedded engine) | **Available** (`v0.5.0`) | [**Download →**](https://ryuhdr.threedola.workers.dev/#ryuhdr) |
| **Brontide** | Eden (C++ / Patched SDL3, renhdrum.dll thin hook, built-in HD Rumble Studio) | **Available** (`v1.1.0`) | [**Download →**](https://ryuhdr.threedola.workers.dev/brontide.html) |
| **Ryu-HDRum** | Ryubing (C# / Patched SDL3, `renhdrum.dll` embedded, modern system support) | **Coming soon** | [Preview →](https://ryuhdr.threedola.workers.dev/#ryu-hdrum) |

New to HD Rumble? [See how this differs from other emulators →](https://ryuhdr.threedola.workers.dev/compare.html)

*Emulator releases, setup guides and changelogs live on the [official website](https://ryuhdr.threedola.workers.dev).*

### About the website

The [official website](https://ryuhdr.threedola.workers.dev) is both a showcase and an open development journal for RenHDrum and the emulators built on it. Each project has its own page section covering:

- **The story**: why it was started, what went wrong along the way, and what was learned (Ryuhdr began as a small prototype to fix coarse rumble and swallowed button presses, then became the reference every RenHDrum algorithm is checked against).
- **How it's built**: the integration approach (embedded engine, SDL3 thin hook, companion app) and the trade-offs behind it.
- **Where it stands**: what is done, what is in progress, and what comes next.
- **Downloads**: builds that are ready to try, with SHA-256 checksums.

It is a plain static page run by the author: no sign-up, no ads, no decryption keys, system software or games. Donations are optional and never unlock features. If you are integrating RenHDrum into your own project, the site shows how it played out in three real emulators.

---

## Quick Integration (5 Steps)

Integrating `renhdrum.dll` with an SDL3-based C/C++ or C# host requires 5 straightforward steps:

1. **Create the Engine**: When a compatible controller is opened, call `renhdrum_engine_create(is_bluetooth, is_pro_controller)`.
2. **Synchronize Transport Settings**: Configure patched SDL3 transport flags (`SDL_HDRumbleSetNoQueue`, etc.), and pass the resolved `RenHdcSettings` to `renhdrum_engine_apply_settings`.
3. **Push Vibration Data**: Feed stereo vibration parameters via `renhdrum_engine_push_sample` (or `renhdrum_engine_push_half_sample` if left/right are processed independently).
4. **Tick Engine at 1 ms**: In a 1 ms timer loop, call `renhdrum_engine_tick(engine, now_us, packet10)`. When it returns `1`, dispatch the 10-byte Subcommand `0x10` buffer via `SDL_SendJoystickEffect`.
5. **Teardown**: Send the canonical neutral stop packet (`0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40`) and call `renhdrum_engine_destroy`.

> [!NOTE]
> **Known Limitation in v0.1.0**: `renhdrum_settings_resolve` currently queries local override files and IPC shared memory managed by `Brontide-App`. A pure in-memory C ABI configuration resolver will be provided in `v0.2.0`.

### Documentation Links

- **C ABI Reference**: [`include/renhdrum.h`](include/renhdrum.h) & [C ABI Documentation](docs/integration/c-abi-reference.md)
- **Integration Guides**:
  - [C++ / SDL3 Host Guide](docs/integration/sdl3-host.md)
  - [C# / .NET Host Guide](docs/integration/csharp-host.md)
  - [DualSense 48 kHz Audio Haptics Guide](docs/integration/dualsense.md)
  - [Architecture & Module Overview](docs/integration/architecture.md)
- **Technical Research Monographs**:
  - [01: Packet Format & Calibration](docs/research/01-hd-rumble-packet-format.md)
  - [02: 5 ms AM/FM Envelope & Crisp Notes](docs/research/02-amfm-multisample-and-crisp-notes.md)
  - [03: Bluetooth Congestion & Token Bucket Pacing](docs/research/03-bluetooth-congestion-and-pacing.md)
  - [04: USB Idle-Gated Writes & Dense Detection](docs/research/04-usb-idle-gated-writes.md)
  - [05: DualSense 48 kHz Audio Haptics DSP](docs/research/05-dualsense-audio-haptics.md)
  - [06: SDL3 vs SDL2 HIDAPI Rumble Architecture](docs/research/06-sdl3-vs-sdl2-rumble-thread.md)
  - [07: Practical Integration Lessons](docs/research/07-integration-lessons.md)
- **Code Examples**:
  - [Minimal C Example](examples/c/)
  - [Minimal C# Example](examples/csharp/)

---

## Repository Structure & Building from Source

```text
RenHDrum/
├── include/              # Public C ABI header (renhdrum.h)
├── src/                  # Core Rust implementation (zero external crate dependencies)
│   ├── backend/          # Nintendo Switch HD Rumble lifecycle
│   ├── detectors/        # Dense rate, click pulse, and short pulse detectors
│   ├── dualsense/        # 48 kHz float32 VCM audio haptics synthesizer
│   ├── encoding/         # Standard dekuNukem, AM/FM 5 ms, and Crisp Notes encoders
│   ├── planner/          # AM/FM micro-timing envelope planner
│   ├── ffi.rs            # C ABI export layer
│   └── lib.rs            # Library entry point
├── tests/                # 240 regression, golden waveform, and guardrail tests
├── examples/             # Minimal C and C# host implementations
├── integrations/         # Patched SDL3 transport and reference Brontide patches
│   ├── sdl3/sdl3-patches # Upstream SDL 3.4.8 patches (zlib license)
│   └── brontide/         # Reference thin-hook patch series (GPL-3.0-or-later)
├── reference/            # Experimental runtime hook prototypes
└── docs/                 # Integration manuals and research monographs
```

### Building and Testing

```bash
# Run all 240 unit tests, golden tests, and guardrail tests
cargo test

# Run clippy with strict zero-warning policy
cargo clippy -- -D warnings

# Build optimized release library (renhdrum.dll / librenhdrum.so)
cargo build --release
```

---

## Support & Donations

If RenHDrum improves your emulation or gaming experience, you can support ongoing research and development:

- **Ko-fi**: [https://ko-fi.com/threedola](https://ko-fi.com/threedola)
- **USDT (BEP20 - BNB Smart Chain)**: `0x2fac84f94d05c550ed604f5867da62ac1bba7f7c`
- **USDT (TRC20 - Tron)**: `TAAEy72WPFAJ5h3g8GYFnZ7bm7CDnTuwTS`

---

## Research Credits & Acknowledgments

RenHDrum stands on the shoulders of open-source reverse-engineering and emulation pioneers. The fidelity, low latency, and physical nuance achieved in this engine are directly built upon the empirical measurements, protocol analyses, and public documentation contributed by the community:

1. **dekuNukem ([`Nintendo_Switch_Reverse_Engineering`](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering))**
   - **Foundation**: Decoded the standard 1-sample 7-bit dual-wave packet structure (`FormatOne28bit`), logarithmic frequency formula `round(log2(f / 10) * 32)`, 101-entry non-linear amplitude lookup table (`0x00`..`0x1FC0`), neutral stop packet (`00 01 40 40`), and Subcommand `0x10` protocol.
   - **RenHDrum Refinement**: Standardized into a zero-allocation 28-bit streaming encoder (`src/encoding/standard.rs`) with stop priority (`stop_priority`) handling on Linear Resonant Actuators (LRAs).

2. **Switchbrew Wiki Contributors ([`Joy-Con`](https://switchbrew.org/wiki/Joy-Con))**
   - **Foundation**: Documented `HidMotorData` bit-packing layouts (`VibrationAmFmPackTag`) across 8 four-byte frame configurations (`FormatZero`..`FormatThree7bit`), high-bit framing flags (`0x8080`, `0x8000`, `0x80`), and `AmFmCodes` unpacking tables identifying code `24` (`0x18`) as a hold-state command.
   - **RenHDrum Refinement**: Developed real-time reverse packaging (5 ms Multi-Sample Planner in `src/encoding/amfm.rs` and `src/planner/amfm_planner.rs`) that dynamically selects optimal 1/2/3-sample frames per 15 ms window, with automatic sub-5-bit fallback (falling back to 1-sample 7-bit mode when amplitude drops below `SILENT_BELOW = 0.04`) to maintain silky smooth micro-vibrations.

3. **felipearmat ([`OGX-Mini-2026` PR #116](https://github.com/MegaCadeDev/OGX-Mini-2026/pull/116), fork of `wiredopposite/OGX-Mini`)**
   - **Foundation**: In PR #116 (`HdRumble.cpp` & `HdRumble.h`), uncovered the 5-bit logarithmic amplitude scale (`-0.5 * (code - 1)` with a `-0.84375` offset relative to the 7-bit peak index 100) and documented Steam UI haptic tick packets (`00 00 05 C0`). Referenced only; no code copied.
   - **RenHDrum Refinement**: Derived the exact energy compensation constant `Code5GameScale = 1.7947091` (2^0.84375) to strictly equalize perceived energy between 7-bit and 5-bit sample streams; conducted physical acoustic spectrogram measurements using close-proximity smartphone microphone analysis to experimentally decode frequency preset codes (`12..16` with steps -12..+12) and frequency jump step codes (`STEP_UP = 23`, `STEP_DOWN = 25` in `src/encoding/amfm.rs`).

4. **rei-github, ClearlyClaire ([Issue #11](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/issues/11)) & CTCaer ([`jc_toolkit`](https://github.com/CTCaer/jc_toolkit))**
   - **Foundation**: 2018 oscilloscope measurements by `rei-github` confirming exponential amplitude decay `2^(-0.5 * (c - 1))` and alternative bit-layouts; real hardware packet captures by `CTCaer` validating `FormatThree7bit` framing.
   - **RenHDrum Refinement**: Engineered a 15 ms transient pulse synthesizer (`src/encoding/click.rs`) and the Crisp Notes algorithm (+3 dB attack / -3 dB sustain in `src/encoding/crisp_notes.rs`) providing snappy, instantaneous transient punch without muddying actuator response.

5. **Nielk1 ([DualSense Trigger Effect Gist](https://gist.github.com/Nielk1/6d54cc2c00d2201ccb8c2720ad7538db)) & DualSense Reverse-Engineering Community**
   - **Foundation**: Identified DualSense 4-channel 48 kHz USB Audio Class architecture (Channels 3–4 directly driving stereo Voice Coil Motors) alongside the standard HID rumble cancellation flag; decoded 10-zone bitpacking for Adaptive Triggers (`0x02`).
   - **RenHDrum Refinement**: Built a real-time 48 kHz haptics synthesizer (`src/dualsense/haptics.rs`) mapping low/high rumble bands to dual sine waves on Channels 3–4 with perceived intensity curve `x^0.5`, 0.8 output ceiling, 1-pole 4 ms smoothing (`DUALSENSE_SMOOTHING_MS`) to preserve crisp 5 ms transient edges, and automatic legacy HID rumble silencing.

6. **Sam Lantinga, Valve Corporation & SDL Contributors ([`libsdl-org/SDL`](https://github.com/libsdl-org/SDL))**
   - **Foundation**: Multi-platform HIDAPI architecture (`SDL_hidapi_switch.c`, `SDL_hidapi_rumble.c`, `SDL_hidapi_ps5.c`), full report 0x30 handshake, USB/Bluetooth routing, and low-latency audio output.
   - **RenHDrum Refinement**: Patched SDL2 input-dropping bug during rumble backlog, and created SDL3 patches (`integrations/sdl3/sdl3-patches/`) implementing Idle-Gated No-Queue, Write-After-Input (B1) for CYW20734 half-duplex bus collision avoidance, and 3-level Token Bucket rate limiting (B3) to prevent Bluetooth input lag spikes.

7. **Ryujinx Team (`gdkchan`), Ryubing (`GreemDev`), and Eden / Yuzu Contributors**
   - **Foundation**: HLE `Npad` / `hid` emulation service decoding in-game vibration telemetry into 5 ms `(FrequencyLow, AmplitudeLow, FrequencyHigh, AmplitudeHigh)` pairs, and foundational C# / C++ emulator runtime platforms.
   - **RenHDrum Refinement**: Seamless drop-in integration via clean C ABI (`renhdrum.dll`).

8. **egormanga ([SAxense](https://github.com/egormanga/SAxense), https://apps.sdore.me/SAxense)**
   - **Foundation**: DualSense Bluetooth haptics report 0x32 layout (packet 0x11 + 0x12, s8 3 kHz PCM, CRC seed 0xA2).
   - **RenHDrum Refinement**: Engineered zero-allocation report packaging module (`src/dualsense/bt_haptics.rs`) and C ABI export (`renhdrum_dualsense_dsp_build_bt_report`) converting internal 48 kHz DSP output to 3 kHz s8 with 16:1 block averaging and Sony CRC32 checksum.

---

## License & Attribution

- **Core Library**: Dual-licensed under either **MIT License** ([LICENSE-MIT](LICENSE-MIT)) or **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE)) at your option.
- **SDL3 Patches**: Licensed under the **zlib License** ([`integrations/sdl3/sdl3-patches/LICENSE`](integrations/sdl3/sdl3-patches/LICENSE)).
- **Brontide Patches**: Licensed under the **GNU General Public License v3.0 or later** ([`integrations/brontide/LICENSE`](integrations/brontide/LICENSE)).
- See [`NOTICE`](NOTICE) for complete third-party attributions (Ryujinx, Ryubing, dekuNukem, Switchbrew, felipearmat, rei-github, CTCaer, Nielk1, SDL, Eden, egormanga).

### Trademark Disclaimer

*Nintendo Switch, Joy-Con, and Pro Controller are registered trademarks of Nintendo Co., Ltd. PlayStation and DualSense are registered trademarks of Sony Interactive Entertainment Inc. RenHDrum is an independent open-source research project and is not affiliated with, endorsed by, or sponsored by Nintendo or Sony.*

---

Built by Three-Dola, vibe-coded with the help of Claude (Anthropic) and Gemini (Google).
