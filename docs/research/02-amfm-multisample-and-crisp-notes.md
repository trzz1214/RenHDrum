# Monograph 02: 5 ms AM/FM Multi-Sample Envelope & Crisp Notes

This monograph examines the micro-timing mechanisms within the Switch controller's 15 ms HID transmission frame, detailing 5 ms sub-packet sampling formats, hardware envelope slew limits, sub-5-bit silent floor handling, and crisp note transient shaping.

---

## 1. Multi-Sample Sub-Packet Formats

Standard HD Rumble transmits one sample pair every 15 ms. Inside the physical Joy-Con and Pro Controller MCU, the vibration synthesizer actually processes **5 ms sub-frames** (3 time slots per 15 ms HID report). The controller MCU supports three compressed sub-sample formats:

```
+-------------------------------------------------------------------+
|               15 ms Transmission Window (3 Time Slots)            |
|       Slot 0 (0..5 ms)    |   Slot 1 (5..10 ms)   | Slot 2 (10..15 ms)|
+---------------------------+-----------------------+-------------------+
```

### 1. `FormatThree` (3 $\times$ 5-bit Amplitude)
Packs three 5-bit amplitude values for the low band and three 5-bit amplitude values for the high band into a single 32-bit word, while preserving the existing frequencies stored in the controller's hardware registers:
```
[ Lo0 (5b) | Hi0 (5b) | Lo1 (5b) | Hi1 (5b) | Lo2 (5b) | Hi2 (5b) | Mode (2b) ]
```

### 2. `FormatThree7bit` (In-Packet 7-bit Frequency Update)
Carries one full 7-bit frequency update for either the low or high band, followed by two 5-bit amplitude slots for the remaining duration:
```
[ Band (1b) | IsFM (1b) | Freq7 (7b) | Lo1 (5b) | Hi1 (5b) | Lo2 (5b) | Hi2 (5b) | Mode ]
```

### 3. `FormatTwo14bit` (Dual 14-bit Mixed Representation)
Encodes a new frequency along with two high-precision 14-bit amplitude and pitch interpolation coefficients.

---

## 2. Hardware Envelope Constraints

The physical linear actuators and internal drive circuitry enforce strict physical slew rate limits:
- **Amplitude Step Constraint**: The amplitude index cannot jump by more than **1 table step** across consecutive 5 ms slots unless resetting from silence.
- **Frequency Step Constraint**: Frequency changes within a single 15 ms window are bounded by at most **2 frequency steps**.

When an emulated game submits an instantaneous vibration transient that exceeds these slew boundaries, attempting to force a 3-sample compression packet would cause audible clipping or phase distortion. In such cases, the `HdAmFmPlanner` gracefully falls back to a single two-wave peak report.

---

## 3. Sub-5-Bit Low-Amplitude Floor Handling

A critical defect in naive 5-bit decoders occurs during faint vibrations. The 5-bit amplitude table maps normalized amplitudes below `SILENT_BELOW = 0.04` to code `0`. 

If a game produces a gentle ambient hum (such as subtle rotor vibrations or distant rumble at amplitude $0.005 - 0.035$):
- A naive 3-sample encoder packs code `0` across all three slots (`18 63 8C F1`), rendering the actuator completely silent.
- However, in standard two-wave mode, `hd_amp_index(a) > 0` resolves to non-zero 7-bit amplitude indices.

### The RenHDrum Fallback Rule

```rust
// In src/planner/amfm_planner.rs
let no_level = low.iter().chain(high.iter()).all(|&a| HdAmFmEncoding::code(a) == 0);
let felt = low.iter().chain(high.iter()).any(|&a| hd_amp_index(a) > 0);

if no_level && felt {
    // Drop back to standard two-wave packet so subtle buzz remains audible
    return false;
}
```

This prevents faint environmental cues from vanishing while still preserving 5 ms envelope fidelity for stronger waveforms.

---

## 4. Crisp Notes Transient Shaping

Linear resonant actuators suffer from mechanical inertia: an actuator starting from rest takes several milliseconds to reach peak displacement. Without compensation, rapid musical notes and button clicks feel mushy.

**Crisp Notes** introduces dynamic attack transient pre-emphasis:
1. **Attack Boost (+3 dB)**: At the onset of a new vibration note (rising from silence or transitioning rapidly), the first 5 ms slot receives a $+3\text{ dB}$ amplitude boost ($\approx 1.414\times$ multiplier, clamped to $1.0$).
2. **Sustain Decay (-3 dB)**: On subsequent slots, the amplitude is attenuated by $-3\text{ dB}$ ($\approx 0.707\times$) to prevent acoustic overhang and ensure rapid dampening.

```
Amplitude
  ^
  |        +3 dB Peak Attack
  |        |---
  |        |  |
  |        |  |     -3 dB Sustain Decay
  |        |  |    +--------+
  |        |  |    |        |
  +--------+--+----+--------+---------> Time (ms)
          0   5    10       15
```

This transient shaping restores sharp tactile clicks and crisp musical note separation on Nintendo Switch Pro Controllers and Joy-Cons over both USB and low-latency Bluetooth links.
