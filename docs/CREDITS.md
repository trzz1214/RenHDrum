# Credits & Reuse

The short version is in the [README](../README.md#credits). This page keeps the full record: who contributed what, what was copied, what was only referenced, and how to credit RenHDrum.

## Research credits

RenHDrum stands on the shoulders of open-source reverse-engineering and emulation pioneers. The fidelity, low latency, and physical nuance achieved in this engine are directly built upon the empirical measurements, protocol analyses, and public documentation contributed by the community:

1. **dekuNukem ([`Nintendo_Switch_Reverse_Engineering`](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering))**
   - **Foundation**: Decoded the standard 1-sample 7-bit dual-wave packet structure (`FormatOne28bit`), logarithmic frequency formula `round(log2(f / 10) * 32)`, 101-entry non-linear amplitude lookup table (`0x00`..`0x1FC0`), neutral stop packet (`00 01 40 40`), and Subcommand `0x10` protocol.
   - **RenHDrum Refinement**: Standardized into a zero-allocation 28-bit streaming encoder (`src/encoding/standard.rs`) with stop priority (`stop_priority`) handling on Linear Resonant Actuators (LRAs).

2. **Switchbrew Wiki Contributors ([`Joy-Con`](https://switchbrew.org/wiki/Joy-Con))**
   - **Foundation**: Documented `HidMotorData` bit-packing layouts (`VibrationAmFmPackTag`) across 8 four-byte frame configurations (`FormatZero`..`FormatThree7bit`), high-bit framing flags (`0x8080`, `0x8000`, `0x80`), `AmFmCodes` unpacking tables identifying code `24` (`0x18`) as a hold-state command, and `nn::hid::IHidServer` IPC vibration command structures (213, 214, 220).
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

9. **Franseven ([`PokeDinner`](https://github.com/Franseven/PokeDinner))**
   - **Foundation**: Created PokeDinner 2.5.0 (`216b5034c6`), the baseline Switch 2 emulator testbed used for Brontide-NS2.
   - **RenHDrum Refinement**: Serves as the host emulator base for validating RenHDrum multi-controller HD Rumble on retail Switch 2 games.

10. **neo Luigi ([Eglantine PR #155](https://github.com/eglantine-emu/eglantine/pull/155))**
    - **Foundation**: Implemented genuine `nn::hid::IHidServer` command 214 (`SendVibrationValuesInMode`) dispatching (`2b413a46a2`, `465aa33077`), capturing guest Switch 2 vibration handles and values and forwarding them into the emulator core.
    - **RenHDrum Refinement**: Integrated command 214 routing into the Brontide-NS2 input layer, channeling vibration parameters directly into RenHDrum's `_half_` C ABI functions to eliminate hanging rumble loops and preserve parametric frequency response.

11. **samaBR85 ([`renhdrum-eden`](https://github.com/samaBR85/renhdrum-eden))**
    - **Foundation**: Created experimental patch adaptations porting RenHDrum into Eglantine and Lichii, captured independent in-game vibration traces for Switch 2 titles, and resolved Raw Input SDL deadlocks.
    - **RenHDrum Refinement**: Informed the Switch 2 amplitude scaling investigation and empirical calibration for per-controller piecewise response curves (`src/nx2.rs`).

12. **Frizti ([`Fritchii`](https://github.com/frizti)), zortos293 ([`Lichii`](https://github.com/zortos293/Lichii)), Dark ([`Dynamic FPS`](https://github.com/darkxex/dynamic-fps))**
    - **Foundation**: Created Fritchii v5.5, Lichii, and Dynamic FPS, providing specialized rendering and timing fixes for Switch 2 titles.
    - **RenHDrum Refinement**: Serves as the host emulator platform for validating RenHDrum HD Rumble in Switch 2 titles via Brontide-oot 1.0.1.

13. **NotRexed ([`Hookshot`](https://github.com/NotRexed/Hookshot))**
    - **Foundation**: Created the Hookshot static recompiler for *Ocarina of Time* (Switch 2 Edition).
    - **RenHDrum Refinement**: Serves as the reference testbed for validating the Generic Recomp C Kit (`integrations/recomp-c/`), enabling turnkey zero-lag HD Rumble synthesis in native PC static recompilations.

14. **MagicBeanDev ([`Longshot`](https://git.magicbean.dev/MagicBeanDev))**
    - **Foundation**: Former Hookshot collaborators who created the Longshot static recompiler.
    - **RenHDrum Refinement**: Second real fork used to test the Generic Recomp C Kit; its lessons are in [recomp-games.md section 11](integration/recomp-games.md#11-lessons-from-two-real-forks-hookshot-v220-rc2-longshot-v300-rc3).

15. **ragnarok179 ([`Enhanced-PS5-DualSense-Haptics-Bridge`](https://github.com/ragnarok179/Enhanced-PS5-DualSense-Haptics-Bridge))**
    - **Foundation**: DualSense Bluetooth combined report `0x36` layout (`0x11` / `0x10` / `0x12` / `0x13`), Sony CRC32 seed `0xA2`, SetState speaker routing flags, and 10 ms / 480-sample Opus CBR 160 kb/s speaker framing (MIT License).
    - **RenHDrum Refinement**: Reimplemented in Rust inside `src/dualsense/bt_haptics.rs` and `src/dualsense/opus_codec.rs` with zero heap allocation on the packet path and a 2 s idle rest gate.

16. **Xiph.Org Foundation, Skype Limited, Jean-Marc Valin, Koen Vos & Opus Contributors ([`xiph/opus`](https://github.com/xiph/opus))**
    - **Foundation**: Reference `libopus` (`v1.5.2`, 3-Clause BSD License, [`vendor/opus/COPYING`](../vendor/opus/COPYING)).
    - **RenHDrum Refinement**: Vendored in `vendor/opus/` and statically compiled via `build.rs` using stack allocation (`USE_ALLOCA`) and fixed encoder state storage for DualSense Bluetooth speaker packets (`0x93`).

## Copied · Referenced · Built in this project

| Part | Copied (code or data) | Referenced (ideas, specs, measurements) | Built in this project | Evidence |
|---|---|---|---|---|
| HD Rumble 28-bit packet and frequency formula | - | dekuNukem: packet layout, `round(log2(f / 10) * 32)` | Zero-allocation encoder with stop priority | `src/encoding/standard.rs`, `docs/research/01` |
| Amplitude table `0x00`..`0x1FC0` (101 entries) | Copied (data) from dekuNukem's rumble data table | - | Binary-search lookup around it | `src/encoding/standard.rs` (`HD_AMP_THRESHOLDS`), `tests/guardrails.rs` |
| Multi-sample AM/FM formats and 5 ms planner | - | Switchbrew Wiki: bit-packing layouts, AmFmCodes tables | Planner that picks 1/2/3-sample frames per 15 ms window, with fallback | `src/encoding/amfm.rs`, `src/planner/amfm_planner.rs`, `docs/research/02` |
| 5-bit logarithmic amplitude scale | - | felipearmat, OGX-Mini PR #116 (no code copied) | Energy compensation constant `CODE5_GAME_SCALE = 1.7947091` | `src/encoding/amfm.rs`, `NOTICE` item 4 |
| Click pulse and Crisp Notes | - | rei-github oscilloscope measurements, CTCaer `jc_toolkit` captures | Click synthesizer and +3 dB / -3 dB transient shaping | `src/encoding/click.rs`, `src/encoding/crisp_notes.rs`, `docs/research/02` |
| Bluetooth pacing (Detailed 5 ms / Compatible + legacy 3 tiers), idle-gated writes, write-after-input | - | Behaviour of the SDL HIDAPI Switch driver; newest-sample fallback idea from Hookshot PR #1213 (no code copied) | Detailed 5 ms mode, Compatible fallback, token-bucket tiers, idle-gated no-queue writes, write-after-input | `tests/guardrails.rs`, `docs/research/03`, `docs/research/04` |
| DualSense USB audio haptics DSP | - | Nielk1 and the DualSense reverse-engineering community: 4-channel 48 kHz USB audio layout | The synthesizer itself (the DSP shared with Ryuhdr is also my own code) | `src/dualsense/haptics.rs`, `tests/golden_dualsense.rs`, `NOTICE` item 6 |
| DualSense Bluetooth haptics, report `0x32` | - | egormanga, SAxense: report layout only | Reimplementation in Rust, 48 kHz to 3 kHz conversion, CRC | `src/dualsense/bt_haptics.rs` (header comment: no SAxense code copied), `docs/research/05` |
| DualSense Bluetooth combined report `0x36` and speaker Opus `0x93` | - | ragnarok179, `Enhanced-PS5-DualSense-Haptics-Bridge` (MIT): report layout and 512→480 10 ms CBR 160 kb/s Opus framing | Reimplementation in Rust, `0x90` speaker flags, zero-allocation `opus_encode_float` + silence frame | `src/dualsense/bt_haptics.rs`, `src/dualsense/opus_codec.rs`, `NOTICE` item 16 |
| `libopus` (`v1.5.2`) | Copied (`vendor/opus/`, BSD-3-Clause) | - | Static build via `build.rs` (`USE_ALLOCA`), fixed encoder buffer | `vendor/opus/`, `build.rs`, `NOTICE` item 17 |
| Switch 2 command 214 IPC dispatching | - | neo Luigi (Eglantine PR #155): command 214 routing pattern | Adapted into Brontide-NS2 input driver to feed RenHDrum `_half_` C ABI | `docs/research/11`, `docs/integration/switch2-games.md` |
| Switch 2 testbed emulator | - | Franseven (PokeDinner 2.5.0): emulator core | Base emulator for Brontide-NS2 reference testbed | `docs/research/11`, `docs/integration/switch2-games.md` |
| Switch 2 IPC command names (213/214/220) | - | Switchbrew Wiki: command identifiers | Verified against Switchbrew documentation | `docs/research/11` |
| Switch 2 per-controller response curves & traces | - | samaBR85: Switch 2 traces and Lichii patch port | Piecewise transfer curves per controller | `src/nx2.rs`, `docs/integration/switch2-games.md` |
| SDL3 patches | Modifies SDL source (zlib) | - | The HD Rumble patches | `integrations/sdl3/sdl3-patches/` |
| Brontide hook patches | Modifies Eden source (GPL-3.0-or-later) | - | The RenHDrum hook patches | `integrations/brontide/` |
| Static recompilation C bridge | MIT OR Apache-2.0 / reference patch (GPL-3.0) | NotRexed (Hookshot v2.1.0): reference testbed | Generic recomp C bridge and Hookshot v2.1.0 reference patch | `integrations/recomp-c/`, `docs/integration/recomp-games.md` |
| Rumble data from games and DSP origins | Parts of the DSP logic, packet structures and state transitions (MIT, from Ryujinx via Ryuhdr; `NOTICE` item 1) | Ryujinx / Ryubing / Eden HID vibration services | Experimental runtime hook (reference only) | `reference/ryubing-hook/`, `NOTICE` item 1 |

### Sources referenced without copying code

| Source | License | How it was used |
|---|---|---|
| [SAxense](https://github.com/egormanga/SAxense) | MPL-2.0 | Report `0x32` layout only; code reimplemented |
| PadForge | License not checked; no code or text copied | Behaviour read only for the Steam Controller notes (`docs/research/08`); no code copied |


## Reusing RenHDrum

**Copying code** (Rust core, C header, patches): the license requires it. Keep the copyright line and the license text (MIT or Apache-2.0 for the core; zlib for the SDL3 patches; GPL-3.0-or-later for the Brontide patches). Under Apache-2.0, also keep the relevant parts of [`NOTICE`](../NOTICE).

**Copying or adapting the docs or website text** (research write-ups, tables, diagrams): CC BY 4.0 requires credit. Name Three-Dola, link to the source and to the license, and say if you changed it.

**Using ideas or measurements** (pacing levels, timings, findings in `docs/research/`): copyright does not cover ideas or facts, so credit is not legally required. It is still appreciated, and it helps others find the original measurements.

Example credit:

> Based on RenHDrum by Three-Dola (https://github.com/trzz1214/RenHDrum), research notes licensed under CC BY 4.0.

For academic or technical citations, use [`CITATION.cff`](../CITATION.cff) (GitHub shows a "Cite this repository" button).

This section is a plain-language summary, not legal advice; the license files are what count.
