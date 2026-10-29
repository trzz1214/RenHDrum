# Monograph 05: DualSense 48 kHz Audio Haptics DSP

This monograph explains the mathematical synthesis of real-time 4-channel 48 kHz PCM audio haptics for Sony PlayStation 5 DualSense voice coil motors (VCM) over USB.

---

## 1. Physical Actuator Mapping & Signal Routing

The DualSense controller's internal DAC drives its voice coil motors via audio channels:
- **Channels 0 & 1 (Front Left / Front Right)**: Headphone audio and internal speaker.
- **Channels 2 & 3 (Rear Left / Rear Right)**: Left and Right VCM haptic coils (also referenced as Channels 3 and 4 in 1-indexed audio documentation).

RenHDrum generates pure silence ($0.0$) on channels 0 and 1, routing synthesized tactile waveforms exclusively to channels 2 and 3.

```
Host Stream Output (48000 Hz, Float32 Interleaved):
[ Ch0: 0.0 | Ch1: 0.0 | Ch2: Left VCM Haptics | Ch3: Right VCM Haptics ]
```

---

## 2. Non-Linear Power Curve & Power Ceiling

Physical voice coil motors exhibit a non-linear displacement-to-voltage response curve. Human fingertips perceive mechanical power non-linearly:

1. **Square-Root Amplitude Compression ($x^{0.5}$)**:
   Input game amplitude $a \in [0.0, 1.0]$ is mapped through a non-linear square-root curve:
   $$A_{\text{perceptual}} = a^{0.5} = \sqrt{a}$$
   This lifts delicate, subtle texture vibrations into the physical actuator's active tactile sensitivity zone without overdriving heavy impacts.
2. **Thermal & Mechanical Power Ceiling (`0.70`)**:
   Continuous maximum excursion on DualSense VCM coils generates substantial heat and distortion. Output amplitude is strictly limited:
   $$A_{\text{out}} = \min(A_{\text{perceptual}} \times V_{\text{gain}}, 0.70)$$
   Where `0.70` represents the maximum safe sustained power ceiling (`DUALSENSE_OUTPUT_CEILING = 0.70`).

---

## 3. Real-Time Sliding Smoothing & Frequency Phase Synthesis

To prevent acoustic pops and transient clicks when games rapidly modulate vibration targets:

### 4 ms Sliding Low-Pass Filter
Frequency transitions are smoothed across a 4 ms window (`DUALSENSE_SMOOTHING_MS = 4.0`):
$$\Delta_{\text{max}} = \frac{\Delta t}{4.0\text{ ms}}$$
$$f_{\text{current}}(t) = f_{\text{current}}(t - \Delta t) + \text{clamp}(f_{\text{target}} - f_{\text{current}}, -\Delta_{\text{max}}, \Delta_{\text{max}})$$

### Continuous Double-Precision Phase Accumulation
Phase stepping utilizes 64-bit precision (`f64`) before conversion to 32-bit float (`f32`) audio frames:
$$\phi(t + 1) = \left( \phi(t) + \frac{2\pi \cdot f_{\text{current}}}{48000.0} \right) \pmod{2\pi}$$
$$s(t) = A_{\text{out}} \times \sin(\phi(t))$$

---

## 4. Chunk-Independent 256-Frame Crisp Windows

Host audio engines request variable buffer chunk sizes depending on driver configurations (e.g. 128, 256, 512, or 1024 frames per callback).

If crisp note attack shaping were tied to the host's callback chunk size, varying buffer configurations would alter the note's perceived sharpness:
- RenHDrum decouples attack shaping by maintaining an internal **fixed 256-frame crisp window** ($256 / 48000 \approx 5.33\text{ ms}$).
- Regardless of whether the host pulls audio in 64-frame or 1024-frame chunks, the crisp attack envelope remains strictly invariant.

---

## 5. Golden Waveform Verification Methodology

To verify bit-exact DSP behavior across toolchain updates and platform architectures, RenHDrum includes a golden waveform regression test (`tests/golden_dualsense.rs`).

- **Test Suite**: Measures synthesized waveforms across four test scenarios: basic continuous tone, crisp transient note, frequency glide, and physical amplitude limits.
- **Precision Guarantee**: Evaluates maximum absolute deviation against reference recording benchmarks:
  $$\max |s_{\text{renhdrum}}(t) - s_{\text{benchmark}}(t)| \le 2.4 \times 10^{-6}$$
- **Verification Threshold**: Guaranteed pass at `TOLERANCE = 1e-5`, ensuring negligible drift from hardware-verified benchmarks.

---

## 6. Bluetooth Transport (Report 0x32)

Over Bluetooth, DualSense voice-coil haptics are driven by HID output report `0x32` rather than USB Audio Class streaming. The report carries 32 stereo frames of signed 8-bit PCM at 3000 Hz, delivered every 10.667 ms (~93.75 reports/s). RenHDrum converts internal 48 kHz DSP frames via 16:1 block averaging into 3 kHz s8 samples:

| Bytes | Value | Description |
|---|---|---|
| `0` | `0x32` | HID Bluetooth output report ID |
| `1` | `seq << 4` | 4-bit sequence counter (0..15), incremented per report |
| `2..3` | `0x91, 7` | Packet 0x11 with size header (7 bytes) |
| `4` | `0xFE` | Microphone state (0xFE = session closed) |
| `5..8` | `0` | Reserved / padding |
| `9` | buffer length | Target buffer length (`0xFF` default) |
| `10` | counter | 8-bit wrapping packet counter |
| `11..12` | `0x92, 64` | Packet 0x12 with size header (64 PCM bytes) |
| `13..76` | PCM `[s8; 64]` | 32 interleaved stereo pairs (Left, Right) at 3000 Hz |
| `77..137` | `0` | Trailing padding |
| `138..141` | CRC32 (LE) | Standard CRC32 (`0xEDB88320`) over `0xA2` + bytes 0..137 |

Report structure and framing layout reverse-engineered by egormanga ([SAxense](https://github.com/egormanga/SAxense), MPL-2.0). Reimplemented cleanly in `src/dualsense/bt_haptics.rs` with zero heap allocation. Shipped in Brontide 1.1.0.
