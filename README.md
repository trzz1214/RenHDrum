# RenHDrum

[![CI](https://github.com/trzz1214/RenHDrum/actions/workflows/ci.yml/badge.svg)](https://github.com/trzz1214/RenHDrum/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](NOTICE)
[![Website](https://img.shields.io/badge/Official_website-Three--Dola-success)](https://ryuhdr.threedola.workers.dev)

RenHDrum is an ultra-low-latency, real-time haptics engine written in safe Rust. It exports a zero-allocation C ABI (`renhdrum.dll`, `include/renhdrum.h`) with zero external crate dependencies, solving three core challenges of modern gaming haptics:

1. **5 ms AM/FM Multi-Sample Envelope & Crisp Notes**: Synthesizes micro-timed 5 ms amplitude and frequency sub-packets (`3x5-bit`, `3x7-bit`, `2x14-bit`) within 15 ms transmission frames, combined with dynamic attack pre-emphasis (+3 dB onset boost and -3 dB sustain decay) for Nintendo Switch Pro Controllers and Joy-Cons.
2. **Zero-Lag Bluetooth Pacing & Idle-Gating**: Eliminates wireless packet saturation and input report drops via 150 ms idle refresh, Write-After-Input gating, and a 3-tier Token Bucket scheduler (`HD_BT_LEVELS`).
3. **Sony PlayStation 5 DualSense Audio Haptics DSP**: Generates real-time 4-channel 48 kHz float32 audio haptics for DualSense voice coil motors (VCM) over USB, with non-linear $x^{0.5}$ perceptual scaling and a calibrated 0.70 power ceiling.

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
| **Ryuhdr** | Ryujinx 1.1.1403 (C# / Deep-Patched SDL2, embedded engine) | **Available** (`v0.4.0.1`) | [**Download →**](https://ryuhdr.threedola.workers.dev/#ryuhdr) |
| **Brontide** | Eden (C++ / Patched SDL3, `renhdrum.dll` thin hook + `Brontide-App`) | **Coming soon** | [Preview →](https://ryuhdr.threedola.workers.dev/#brontide) |
| **Ryu-HDRum** | Ryubing (C# / Patched SDL3, `renhdrum.dll` embedded, modern system support) | **Coming soon** | [Preview →](https://ryuhdr.threedola.workers.dev/#ryu-hdrum) |

*Emulator releases, setup guides and changelogs live on the [official website](https://ryuhdr.threedola.workers.dev).*

### About the website

The [official website](https://ryuhdr.threedola.workers.dev) is both a showcase and an open development journal for RenHDrum and the emulators built on it. Each project has its own page section covering:

- **The story**: why it was started, what went wrong along the way, and what was learned (Ryuhdr began as a small prototype to fix coarse rumble and swallowed button presses, then became the reference every RenHDrum algorithm is checked against).
- **How it's built**: the integration approach (embedded engine, SDL3 thin hook, companion app) and the trade-offs behind it.
- **Where it stands**: what is done, what is in progress, and what comes next.
- **Downloads**: builds that are ready to try, with SHA-256 checksums.

It is a plain static page run by the author: no sign-up, no ads, no prod.keys, firmware or games. Donations are optional and never unlock features. If you are integrating RenHDrum into your own project, the site shows how it played out in three real emulators.

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

## License & Attribution

- **Core Library**: Dual-licensed under either **MIT License** ([LICENSE-MIT](LICENSE-MIT)) or **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE)) at your option.
- **SDL3 Patches**: Licensed under the **zlib License** ([`integrations/sdl3/sdl3-patches/LICENSE`](integrations/sdl3/sdl3-patches/LICENSE)).
- **Brontide Patches**: Licensed under the **GNU General Public License v3.0 or later** ([`integrations/brontide/LICENSE`](integrations/brontide/LICENSE)).
- See [`NOTICE`](NOTICE) for complete third-party attributions (Ryujinx, dekuNukem, Switchbrew, SDL).

### Trademark Disclaimer

*Nintendo Switch, Joy-Con, and Pro Controller are registered trademarks of Nintendo Co., Ltd. PlayStation and DualSense are registered trademarks of Sony Interactive Entertainment Inc. RenHDrum is an independent open-source research project and is not affiliated with, endorsed by, or sponsored by Nintendo or Sony.*

---

Built by Three-Dola, vibe-coded with the help of Claude (Anthropic) and Gemini (Google).
