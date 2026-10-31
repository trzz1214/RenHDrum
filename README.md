# RenHDrum

[![CI](https://github.com/trzz1214/RenHDrum/actions/workflows/ci.yml/badge.svg)](https://github.com/trzz1214/RenHDrum/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](NOTICE)
[![Website](https://img.shields.io/badge/Research_Notebook-Three--Dola-success)](https://ryuhdr.threedola.workers.dev)

> **📓 Lab Notebook & Technical Journey**
> [ryuhdr.threedola.workers.dev](https://ryuhdr.threedola.workers.dev) is an open research notebook and dev log: documenting the journey of bringing authentic hardware HD Rumble and DualSense haptics into Switch emulation, demystifying low-level haptic challenges (5 ms AM/FM micro-timing, Bluetooth packet pacing, DualSense 48 kHz voice-coil DSP), and sharing physical measurements.
>
> Explore the notes: [The Journey](https://ryuhdr.threedola.workers.dev/#ryuhdr) · [How Emulators Handle HD Rumble](https://ryuhdr.threedola.workers.dev/compare) · [DualSense Haptics](https://ryuhdr.threedola.workers.dev/dualsense) · [Two Ancestors: yuzu & Ryujinx](https://ryuhdr.threedola.workers.dev/lineage)

RenHDrum is an ultra-low-latency, real-time haptics research and synthesis engine written in safe Rust. It exports a zero-allocation C ABI (`renhdrum.dll`, `include/renhdrum.h`) with zero external crate dependencies, solving three core challenges of modern gaming haptics:

1. **5 ms AM/FM Multi-Sample Envelope & Crisp Notes**: Synthesizes micro-timed 5 ms amplitude and frequency sub-packets (`3x5-bit`, `3x7-bit`, `2x14-bit`) within 15 ms transmission frames, combined with dynamic attack pre-emphasis (+3 dB onset boost and -3 dB sustain decay) for the Nintendo Switch Pro Controller.
2. **Zero-Lag Bluetooth Pacing & Idle-Gating**: Eliminates wireless packet saturation and input report drops via 150 ms idle refresh, Write-After-Input gating, and a 3-tier Token Bucket scheduler (`HD_BT_LEVELS`).
3. **Sony PlayStation 5 DualSense Audio Haptics Synthesizer**: Generates real-time 4-channel 48 kHz float32 audio haptics for DualSense voice coil motors (VCM) over USB, with non-linear x^0.5 perceptual scaling and a calibrated 0.8 output ceiling.

---

## Supported Controllers

| Controller | Status | Connection |
|---|---|---|
| Nintendo Switch Pro Controller | ✅ Supported | USB or Bluetooth |
| Sony DualSense (PS5) | ✅ Supported | USB or Bluetooth |
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

## Reference Testbeds & Implementations

These emulator forks are personal experimental testbeds used to validate RenHDrum in actual games and verify physical haptic fidelity. They serve as reference examples of how authentic HD Rumble can be hooked into existing architectures:

| Project | Base & Integration Architecture | Testbed Role | Research Notes |
|---|---|---|---|
| **Ryuhdr** | Ryujinx 1.1.1403 (C# / Deep-Patched SDL2, embedded engine) | Reference baseline & golden waveform source | [Lab Notes →](https://ryuhdr.threedola.workers.dev/#ryuhdr) |
| **Brontide** | Eden (C++ / Patched SDL3, `renhdrum.dll` thin hook, built-in HD Rumble Studio) | Pure C ABI validation & DualSense BT testbed | [Lab Notes →](https://ryuhdr.threedola.workers.dev/brontide.html) |
| **Ryu-HDRum** | Ryubing (C# / Patched SDL3, `renhdrum.dll` embedded, modern system support) | SDL3 C# interop & modern runtime testbed | [Lab Notes →](https://ryuhdr.threedola.workers.dev/#ryu-hdrum) |

New to HD Rumble? [See how this differs from other emulators →](https://ryuhdr.threedola.workers.dev/compare.html)

### About the Website & Online Notebook

The website at [ryuhdr.threedola.workers.dev](https://ryuhdr.threedola.workers.dev) serves as an online research notebook and dev journal rather than a distribution hub. It documents the personal engineering journey and explains complex technical hurdles in an accessible way:

- **The Journey & Measurements**: Why this research started, what went wrong along the way, and physical empirical data (smartphone microphone acoustic recordings, oscilloscope traces, Bluetooth packet pacing).
- **Technical Demystification**: Intuitive explanations of how HD Rumble actually functions (5 ms AM/FM micro-timing sub-packets, attack pre-emphasis, and DualSense 48 kHz voice-coil audio DSP).
- **Integration Lessons**: Detailed architectural breakdowns of hooking HD Rumble into different runtime stacks (SDL2 vs SDL3, embedded engine vs thin dynamic C ABI hook).

It is a non-commercial, personal research journal: no ads, no sign-ups, and strictly no game files or system keys.

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
├── tests/                # 241 regression, golden waveform, and guardrail tests
├── examples/             # Minimal C and C# host implementations
├── integrations/         # Patched SDL3 transport and reference Brontide patches
│   ├── sdl3/sdl3-patches # Upstream SDL 3.4.8 patches (zlib license)
│   └── brontide/         # Reference thin-hook patch series (GPL-3.0-or-later)
├── reference/            # Experimental runtime hook prototypes
└── docs/                 # Integration manuals and research monographs
```

### Building and Testing

```bash
# Run all 241 unit tests, golden tests, and guardrail tests
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
   - **RenHDrum Refinement**: Developed real-time reverse packaging (5 ms Multi-Sample Planner in `src/encoding/amfm.rs` and `src/planner/amfm_planner.rs`) that dynamically selects optimal 1/2/3-sample frames per 15 ms window, with automatic sub-5-bit fallback (falling back to 1-sample 7-bit mode when amplitude drops below `SILENT_BELOW = 0.04`) so faint micro-vibrations stay audible.

3. **felipearmat ([`OGX-Mini-2026` PR #116](https://github.com/MegaCadeDev/OGX-Mini-2026/pull/116), fork of `wiredopposite/OGX-Mini`)**
   - **Foundation**: In PR #116 (`HdRumble.cpp` & `HdRumble.h`), uncovered the 5-bit logarithmic amplitude scale (`-0.5 * (code - 1)` with a `-0.84375` offset relative to the 7-bit peak index 100) and documented Steam UI haptic tick packets (`00 00 05 C0`). Referenced only; no code copied.
   - **RenHDrum Refinement**: Derived the exact energy compensation constant `CODE5_GAME_SCALE = 1.7947091` (2^0.84375) to strictly equalize perceived energy between 7-bit and 5-bit sample streams; conducted physical acoustic spectrogram measurements using close-proximity smartphone microphone analysis to experimentally decode frequency preset codes (`12..16` with steps -12..+12) and frequency jump step codes (`STEP_UP = 23`, `STEP_DOWN = 25` in `src/encoding/amfm.rs`).

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
   - **RenHDrum Refinement**: Drop-in integration via clean C ABI (`renhdrum.dll`).

8. **egormanga ([SAxense](https://github.com/egormanga/SAxense), https://apps.sdore.me/SAxense)**
   - **Foundation**: DualSense Bluetooth haptics report 0x32 layout (packet 0x11 + 0x12, s8 3 kHz PCM, CRC seed 0xA2).
   - **RenHDrum Refinement**: Engineered zero-allocation report packaging module (`src/dualsense/bt_haptics.rs`) and C ABI export (`renhdrum_dualsense_dsp_build_bt_report`) converting internal 48 kHz DSP output to 3 kHz s8 with 16:1 block averaging and Sony CRC32 checksum.

### What we copied, what we referenced, what we built

| Part | Copied (code or data) | Referenced (ideas, specs, measurements) | Our own work | Evidence |
|---|---|---|---|---|
| HD Rumble 28-bit packet and frequency formula | - | dekuNukem: packet layout, `round(log2(f / 10) * 32)` | Zero-allocation encoder with stop priority | `src/encoding/standard.rs`, `docs/research/01` |
| Amplitude table `0x00`..`0x1FC0` (101 entries) | Copied (data) from dekuNukem's rumble data table | - | Binary-search lookup around it | `src/encoding/standard.rs` (`HD_AMP_THRESHOLDS`), `tests/guardrails.rs` |
| Multi-sample AM/FM formats and 5 ms planner | - | Switchbrew Wiki: bit-packing layouts, AmFmCodes tables | Planner that picks 1/2/3-sample frames per 15 ms window, with fallback | `src/encoding/amfm.rs`, `src/planner/amfm_planner.rs`, `docs/research/02` |
| 5-bit logarithmic amplitude scale | - | felipearmat, OGX-Mini PR #116 (no code copied) | Energy compensation constant `CODE5_GAME_SCALE = 1.7947091` | `src/encoding/amfm.rs`, `NOTICE` item 4 |
| Click pulse and Crisp Notes | - | rei-github oscilloscope measurements, CTCaer `jc_toolkit` captures | Click synthesizer and +3 dB / -3 dB transient shaping | `src/encoding/click.rs`, `src/encoding/crisp_notes.rs`, `docs/research/02` |
| Bluetooth pacing (3 levels), idle-gated writes, write-after-input | - | Behaviour of the SDL HIDAPI Switch driver | Token-bucket pacing levels, idle-gated no-queue writes, write-after-input | `tests/guardrails.rs`, `docs/research/03`, `docs/research/04` |
| DualSense USB audio haptics DSP | - | Nielk1 and the DualSense reverse-engineering community: 4-channel 48 kHz USB audio layout | The synthesizer itself (the DSP shared with Ryuhdr is also our own code) | `src/dualsense/haptics.rs`, `tests/golden_dualsense.rs`, `NOTICE` item 6 |
| DualSense Bluetooth haptics, report `0x32` | - | egormanga, SAxense: report layout only | Reimplementation in Rust, 48 kHz to 3 kHz conversion, CRC | `src/dualsense/bt_haptics.rs` (header comment: no SAxense code copied), `docs/research/05` |
| SDL3 patches | Modifies SDL source (zlib) | - | The HD Rumble patches | `integrations/sdl3/sdl3-patches/` |
| Brontide hook patches | Modifies Eden source (GPL-3.0-or-later) | - | The RenHDrum hook patches | `integrations/brontide/` |
| Rumble data from games and DSP origins | Parts of the DSP logic, packet structures and state transitions (MIT, from Ryujinx via Ryuhdr; `NOTICE` item 1) | Ryujinx / Ryubing / Eden HID vibration services | Experimental runtime hook (reference only) | `reference/ryubing-hook/`, `NOTICE` item 1 |

#### Sources we read but did not copy

| Source | License | How we used it |
|---|---|---|
| [SAxense](https://github.com/egormanga/SAxense) | MPL-2.0 | Report `0x32` layout only; code reimplemented |
| PadForge | License not checked; no code or text copied | Behaviour read only for the Steam Controller notes (`docs/research/08`); no code copied |

---

## License & Attribution

- **Core Library**: Dual-licensed under either **MIT License** ([LICENSE-MIT](LICENSE-MIT)) or **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE)) at your option.
- **SDL3 Patches**: Licensed under the **zlib License** ([`integrations/sdl3/sdl3-patches/LICENSE`](integrations/sdl3/sdl3-patches/LICENSE)).
- **Brontide Patches**: Licensed under the **GNU General Public License v3.0 or later** ([`integrations/brontide/LICENSE`](integrations/brontide/LICENSE)).
- **Documentation & Research** (`docs/`) and the text of the [project website](https://ryuhdr.threedola.workers.dev): licensed under **Creative Commons Attribution 4.0 International (CC BY 4.0)** ([LICENSE-DOCS](LICENSE-DOCS)). Third-party material quoted in the docs keeps its own license (see [`NOTICE`](NOTICE)). Product names and trademarks (Nintendo, Sony, Valve) are not covered.
- See [`NOTICE`](NOTICE) for complete third-party attributions (Ryujinx, Ryubing, dekuNukem, Switchbrew, felipearmat, rei-github, CTCaer, Nielk1, SDL, Eden, egormanga).

### Trademark Disclaimer

*Nintendo Switch, Joy-Con, and Pro Controller are registered trademarks of Nintendo Co., Ltd. PlayStation and DualSense are registered trademarks of Sony Interactive Entertainment Inc. RenHDrum is an independent open-source research project and is not affiliated with, endorsed by, or sponsored by Nintendo or Sony.*

## Reusing RenHDrum / How to credit

**Copying code** (Rust core, C header, patches): the license requires it. Keep the copyright line and the license text (MIT or Apache-2.0 for the core; zlib for the SDL3 patches; GPL-3.0-or-later for the Brontide patches). Under Apache-2.0, also keep the relevant parts of [`NOTICE`](NOTICE).

**Copying or adapting the docs or website text** (research write-ups, tables, diagrams): CC BY 4.0 requires credit. Name Three-Dola, link to the source and to the license, and say if you changed it.

**Using ideas or measurements** (pacing levels, timings, findings in `docs/research/`): copyright does not cover ideas or facts, so credit is not legally required. It is still appreciated, and it helps others find the original measurements.

Example credit:

> Based on RenHDrum by Three-Dola (https://github.com/trzz1214/RenHDrum), research notes licensed under CC BY 4.0.

For academic or technical citations, use [`CITATION.cff`](CITATION.cff) (GitHub shows a "Cite this repository" button).

This section is a plain-language summary, not legal advice; the license files are what count.

---

Built by Three-Dola, vibe-coded with the help of Claude (Anthropic) and Gemini (Google).
