# Monograph 11: Switch 2 HD Rumble — Signal Path, Commands, and Emulation Realities

The Switch 2 console has no official DualSense support. On PC, however, a wired DualSense is currently the most expressive controller for playing Switch 2 games with HD Rumble, reproducing high-definition transients through dual voice coil motors (VCM) driven by continuous audio-rate waveforms.

This monograph documents how Switch 2 titles generate vibration requests, how those requests travel across the IPC boundary in emulators, how RenHDrum routes the signal to distinct controller architectures, the root cause of early vibration bugs, and the quantitative findings from in-game trace sessions.

> [!IMPORTANT]
> **Status: Quantitatively verified across 25 in-game CSV trace sessions.** Vibration behavior and command streams were systematically measured across three commercial Switch 2 titles (*Donkey Kong Bananza*, *Mario Kart World*, and *Ocarina of Time* Switch 2 Edition) using DualSense and Switch Pro Controller 1. Command layouts and piecewise translation curves are cross-referenced with Switchbrew wiki documentation, public emulator pull requests, and published patent applications (US 20260295388-91, JP 2026015701-03).

---

## 1. What Switch 2 Games Actually Send

Marketing materials for next-generation console haptics emphasize wideband frequency response and high-dynamic-range actuators. However, reverse engineering of the guest OS interface reveals that **commercial games continue to send vibration as two frequency-and-amplitude bands per actuator**, exactly as they did on Switch 1.

The primary evolution is in the physical controller hardware (wideband linear resonant actuators, 10-bit quantization fields, and independent sub-sample packet streaming up to 3500 Hz), rather than an immediate architectural shift to raw PCM audio streaming in games.

### 1.1 IPC Vibration Commands (`nn::hid::IHidServer`)

Through OS version 23.0.1, the system exposes three key vibration commands on the `nn::hid::IHidServer` IPC interface:

| Command ID | Name (Switchbrew) | Introduced | Arguments & Structure | Status in Emulators & Reality |
|---|---|---|---|---|
| **214** | `SendVibrationValuesInMode` | 17.0.0+ | Pointer buffers of `VibrationDeviceHandle` (4 bytes each) and `VibrationValue` (16 bytes each), plus raw `0x10` metadata containing a 64-bit `Mode` flag and Applet Resource User ID (`aruid`). | **Active (100% of traffic)**. 100% of measured vibration samples across all three tested titles use command 214 with `Mode = 2`. |
| **213** | `SendVibrationValueInMode` | 17.0.0+ | Single-handle per-call layout. Raw `0x20` payload containing device handle, 64-bit `Mode`, 16-byte `VibrationValue`, and `aruid`. | **Implemented in dispatcher**. Fully implemented in reference testbeds; however, 0 calls were recorded across all commercial test sessions. |
| **220** | `IsOunceVibrationDeviceMounted` | 20.0.0+ (Switch 2 only) | Returns a 1-byte boolean indicating whether a native Switch 2 ("Ounce") controller is attached. | **Uncalled by games**. All three tested titles never call command 220. Perceived intensity differences stem from native amplitude variations, not this flag. |

### 1.2 The 16-Byte `VibrationValue` Struct

Each `VibrationValue` sent via command 214 uses the identical 16-byte structure established on Switch 1:

```cpp
struct VibrationValue {
    float amp_low;   // Low-band amplitude [0.0f, 1.0f]
    float freq_low;  // Low-band frequency (Hz, typically 40-400 Hz)
    float amp_high;  // High-band amplitude [0.0f, 1.0f]
    float freq_high; // High-band frequency (Hz, typically 400-1252 Hz)
};
```

Retail titles such as Donkey Kong Bananza, Mario Kart World, and Ocarina of Time dispatch command 214 with one sample per handle. Because the signal is dispatched in parametric form (amplitude + frequency), an emulator captures the **complete physical intent** of the sound designers.

### 1.3 The `Mode` Flag

Command 214 includes a 64-bit parameter labeled `unknown` in early reverse engineering, confirmed as `Mode`:
- Across 100% of recorded Switch 2 samples in Donkey Kong Bananza, Mario Kart World, and Ocarina of Time, `Mode` is strictly set to `2`.
- In backward-compatible Switch 1 execution, `Mode` is set to `0`.

### 1.4 Why No PCM Commands Exist

Patent filings (e.g. US 20260295388-91) describe theoretical playback modes including continuous PCM streaming. However, **no public IPC command exists for streaming PCM to controllers**. All examined retail games rely exclusively on parametric command 214. Consequently, a trace tap placed at the emulator's vibration boundary captures 100% of the game's transmitted haptic output.

---

## 2. Why Raw Switch 2 Amplitudes Feel Weak or Flat & How RenHDrum 0.3.0 Solves It

Empirical trace logs revealed a striking physical mismatch:
1. **Extremely Low Native Amplitudes**: Retail Switch 2 games output very low raw amplitudes. In *Ocarina of Time*, the 95th percentile amplitude (`p95`) is only `0.046` with a median active amplitude of `0.009`. In *Donkey Kong Bananza*, `p95` is `0.14`. Over 80% to 90% of all vibration events operate below `0.04`.
2. **"The Bell vs. The Speaker" Physical Divergence**:
   - **Switch Pro Controller 1 ("The Bell")**: Driven by dual narrow-band Linear Resonant Actuators (LRA, tuned for 160 Hz and 320 Hz). The legacy protocol enforces an absolute silence cutoff at `0.008` and a 5 ms multi-sample packet floor at `0.04` (`SILENT_BELOW`). Sending raw Switch 2 values directly causes over 80% of small vibrations (grass cuts, footsteps, gentle rolls) to fall entirely silent.
   - **DualSense ("The Speaker")**: Uses wideband voice-coil motors acting like acoustic drivers. However, RenHDrum's internal DSP includes a square-root perceptual compression curve (`x^0.5`) tuned for classic Switch 1 transients. If raw Switch 2 values are passed into this stage, or if a single generic exponential curve is applied to both controllers, Donkey Kong Bananza clips violently (hitting 1.0 ceiling on 17.6% of active samples), flattening dynamic contrast.

### 2.1 The RenHDrum 0.3.0 Dual-Curve Architecture (`src/nx2.rs`)

When the emulator detects `Mode == 2`, it signals RenHDrum via `renhdrum_set_nx2_mode(2)`. RenHDrum 0.3.0 applies tailored 4-point piecewise curves based on published patent specifications (JP 2026015701-03):

- **DualSense Preset `[3]`**:
  - Parameters: `T = 0.001, P1 = 0.004, P2 = 0.550, P3 = 0.900, P4 = 1.000`.
  - Counterbalances the `x^0.5` DSP compression layer, maintaining a rich dynamic spread (`p90/p10` contrast ratio of `20x` to `43x`) without ceiling clipping.
- **Switch Pro Controller 1 Preset `[4]`**:
  - Parameters: `T = 0.001, P1 = 0.048, P2 = 0.240, P3 = 0.640, P4 = 0.950`.
  - Accompanied by a **1.2x transient boost for the first 25 ms** on low frequencies (`freq < 150 Hz`).
  - Lifts 100% of active micro-vibrations above the `0.04` hardware silence floor, restoring crisp impacts and footstep tactile feedback.
- **Mode 0 (Classic Switch 1 Games)**:
  - Input amplitudes pass through 100% bit-exact and untouched.

### 2.2 Preserving Short Menu Ticks: 5.3 ms Slot Peak-Hold (`src/dualsense/haptics.rs`)

Commercial Switch 2 titles dispatch vibration updates at 5 ms intervals (200 Hz). The DualSense audio mixer processes audio in 256-frame blocks at 48 kHz (~5.33 ms slots, 187.5 Hz).

Because 5.0 ms and 5.33 ms slip out of phase every ~80 ms, two consecutive game packets periodically fall within a single mixer slot. In menu navigation, titles frequently transmit a single-packet tick followed immediately by `0.0`. Without compensation, the trailing zero packet overwrites the tick before the mixer renders it.

RenHDrum 0.3.0 implements a **5.3 ms slot peak-hold**: within each 256-frame window, the DSP retains the sample with the maximum combined low-and-high amplitude before releasing the hold. This guarantees 100% of single-packet menu ticks are faithfully rendered while matching reference baseline audio synthesis tests with sub-microscopic error (`max diff <= 2.4e-6`).

### 2.3 Critical Warning: Emulator 15 ms Frontend Rate Limiting (`Accurate Vibration`)

In standard Eden, yuzu, and PokeDinner codebases, `src/hid_core/frontend/emulated_controller.cpp` includes a rate-limiting filter that discards any vibration update arriving less than 15 ms after the preceding update when `enable_accurate_vibrations == false`.

Empirical trace logs revealed that this default filter silently drops **50.0% of vibration packets in Ocarina of Time** and **76.3% of packets in Mario Kart World**!

Host integrations **must** ensure `enable_accurate_vibrations = true` or bypass this 15 ms suppression block whenever HD Rumble is enabled, allowing all micro-samples to reach RenHDrum. This applies to the upstream forks; Brontide builds no longer drop packets when this option is off.

---

## 3. Signal Routing Architecture

In an emulator equipped with RenHDrum (such as Brontide-NS2 and Brontide-oot), vibration values flow from the guest game process directly to physical PC gamepads through dedicated low-latency pipelines:

```mermaid
flowchart TD
    subgraph Guest ["Guest Switch 2 Title"]
        Game["Game Process\n(DK Bananza, MKW, OoT)"]
        Cmd214["nn::hid::IHidServer\nCommand 214 (SendVibrationValuesInMode)"]
        Game -->|16-byte VibrationValue per handle| Cmd214
    end

    subgraph HLE ["Emulator Core (Eden / PokeDinner / Fritchii)"]
        HidServer["IHidServer::SendNx2VibrationValues\n(Eglantine PR #155 by neo Luigi)"]
        ResMgr["ResourceManager::SendVibrationValue"]
        Driver["sdl_driver.cpp\n(RenHDrum Driver Integration)"]
        Cmd214 --> HidServer
        HidServer --> ResMgr
        ResMgr --> Driver
    end

    subgraph RenHDrumCore ["RenHDrum Engine (C ABI)"]
        Mode_Check["renhdrum_set_nx2_mode(2)\nMode 2 Presets: [3] DualSense / [4] Pro 1"]
        DS_Entry["renhdrum_dualsense_dsp_set_half_target\n(with 5.3 ms slot peak-hold)"]
        Pro_Entry["renhdrum_engine_push_half_sample\n(with 25 ms transient boost)"]
        Xbox_Entry["renhdrum_xbox_rumble_push_half_sample"]

        Driver --> Mode_Check
        Mode_Check --> DS_Entry
        Mode_Check --> Pro_Entry
        Mode_Check --> Xbox_Entry
    end

    subgraph Hardware ["Physical Controller Actuation"]
        DS_DSP["DualSense Audio DSP\n4-ch 48 kHz float32 / BT Report 0x32"]
        Pro_Encoder["Switch 1 HD Rumble Encoder\n8-byte Packet / dekuNukem LUT / 15 ms"]
        Xbox_Mapper["Xbox Intent Synthesizer\n4-motor ERM Model / Overdrive / Trigger Pacing"]

        DS_Entry --> DS_DSP
        Pro_Entry --> Pro_Encoder
        Xbox_Entry --> Xbox_Mapper

        DS_Act["Dual Voice Coil Motors (VCM)\nContinuous Audio Waveforms"]
        Pro_Act["Dual LRA Actuators\nResonant 160/320 Hz Bands (Max 1252 Hz)"]
        Xbox_Act["4 ERM Motors\n2 Grip Weights + 2 Trigger Motors"]

        DS_DSP --> DS_Act
        Pro_Encoder --> Pro_Act
        Xbox_Mapper --> Xbox_Act
    end
```

---

## 4. Reproduction Fidelity by Controller Architecture

No physical controller reproduces the Switch 2 signal identically to native Joy-Con 2 hardware. Each architecture makes specific trade-offs:

| Controller | Reproduction Method & Tuning | Preserved Fidelity | Hardware & Protocol Constraints |
|---|---|---|---|
| **PlayStation 5 DualSense** | Continuous audio-rate DSP (48 kHz USB / 3 kHz BT). Automatically activates Preset `[3]` with 5.3 ms peak-hold under Switch 2 mode. | **Highest**. Clean attack transients, wide `20x` to `43x` dynamic contrast, and zero dropped menu ticks. | High-frequency ceiling on Bluetooth (~1.5 kHz) is lower than wired USB. Power ceiling set to 0.70 to protect coils. |
| **Switch Pro Controller 1** | Sub-command `0x10` packets with dekuNukem LUT. Automatically activates Preset `[4]` with 25 ms transient boost under Switch 2 mode. | **Native Switch 1 feel**. 0% samples dropped below `0.04` floor. Crisp steps, drift, and impacts. | Narrow-band LRA physics produce lower contrast (`4x`) on sustained heavy rumbles compared to voice-coil actuators. |
| **Xbox One S (Model 1708)** | Four ERM motors driven by the RenHDrum heuristic translator (`rawinput` / WGI). | **Rhythmic & directional envelope**. Distinct trigger taps for impacts, separate left/right grip rumble, smooth decay. | **In research for Switch 2 games**. Motor rotation speed physically limits micro-vibration detail. |

---

## 5. The 2000 ms Hanging Rumble Bug in Legacy Emulators

Prior to integrating RenHDrum, testers of early Switch 2 emulator branches reported a frequent defect: controllers would begin vibrating during an action or loading transition and **continue vibrating loudly for several seconds** without stopping (documented in Eglantine Issue #513).

### 5.1 Root Cause in Generic SDL Fallbacks

In vanilla PokeDinner and early Eglantine branches, the vibration fallback path in `sdl_driver.cpp` simply called:

```cpp
SDL_RumbleGamepad(joystick, low_freq_intensity, high_freq_intensity, 2000);
```

This produced two critical failure modes:
1. **Total Frequency Loss**: All spectral information was discarded; motors spun at fixed rates regardless of whether the game signaled a high metal chime or a low engine idle.
2. **2-Second Hold Duration**: Every invocation requested a 2000 ms duration from the SDL driver. Switch 2 games frequently stop vibrating simply by **ceasing to transmit commands**, rather than sending an explicit zero-amplitude packet `(0.0f, 0.0f, 0.0f, 0.0f)`. For instance, when Donkey Kong Bananza freezes the render thread during level transitions, the command stream halts immediately. The gamepad driver kept the motors spinning until the 2-second timeout expired.

### 5.2 How RenHDrum Eliminates the Bug

RenHDrum decouples guest IPC dispatch from physical hardware actuation:
- **Idle Watchdogs**: Independent watchdog timers monitor incoming sample intervals on each channel. If sample delivery ceases without an explicit zero, the engine decays amplitude to silence within a controlled decay window.
- **Micro-Cadence Generation**: Output packets to the hardware are scheduled at precise physical intervals (15 ms for USB Pro Controller, 10.67 ms for Bluetooth DualSense, 10-20 ms for Xbox). When the incoming queue dries up, neutral silence packets are immediately dispatched, terminating motor drive without lingering vibrations.

---

## 6. Pro Controller Reliability Enhancements

In addition to vibration synthesis, testing highlighted two critical input transport issues on Switch Pro Controller 1:
1. **Right Stick Upward Drift**: Stale calibration offsets (such as `offset_y: 0.939940`) occasionally persist from configuration freezes. Clamping abnormal center offsets where `|offset| > 0.5f` to `0.0f` eliminates permanent upward drift.
2. **Bluetooth Connection Stability**: Setting Bluetooth pacing to Level 2 (45 ms interval, burst of 3 packets, 80 ms refill) avoids saturation and input lag under dense Switch 2 packet streams.
3. **Clean Controller Teardown**: Dispatching a standard `ClearUSB` command before controller close prevents USB Pro Controllers from remaining trapped in extended reporting mode upon game exit.

---

## References & Credits

- **neo Luigi**: Implementation of genuine command 214 dispatching in Eglantine PR #155 (`2b413a46a2`, `465aa33077`).
- **Franseven**: Author of PokeDinner 2.5.0 (`216b5034c6`), baseline platform for Brontide-NS2.
- **Frizti**: Author of Fritchii v5.5, baseline platform for Brontide-oot.
- **zortos293**: Author of Lichii emulator.
- **Dark**: Author of Dynamic FPS.
- **samaBR85**: Author of `renhdrum-eden`, independent Switch 2 trace verification, Eglantine/Lichii port, and Raw Input deadlock fix.
- **NotRexed & Hookshot Contributors**: Authors of the Hookshot OoT Switch 2 Recompiler (v2.0.6), serving as reference testbed for static recompilation integration.
- **Switchbrew Wiki**: Documentation of `nn::hid::IHidServer` IPC commands 213, 214, and 220.
- **Nintendo Co., Ltd.**: Published patent applications US 20260295388-91 (haptic feedback modes) and JP 2026015701-03 (equalization between hardware generations).
- **RenHDrum / Brontide Team**: Physical multi-controller DSP routing, idle watchdog architecture, and Xbox One intent translation.
