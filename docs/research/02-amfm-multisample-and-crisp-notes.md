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

---

## 5. Measuring Pitch with a Phone Microphone

Two questions blocked pitch changes inside a packet: what the 19 five-bit codes `12..31` (except `24`) do to frequency, and whether the high band of the 14-bit format reads its 7-bit frequency field on the same scale as the low band. A controller sounds at the frequency it vibrates, so a phone recording can answer both without trusting hand feel.

**Method.** The controller (a wired Pro Controller) was held against the phone's microphone while a fixed test sequence played: a three-beep sync marker (two 320 Hz high-band waves, 105 ms each), then 30 segments of three phases, 67 slots of 15 ms per phase, with a 1.5 s pause between segments. The recording (149 s, 31 of 31 segments captured; SDL logged 198-202 packets per segment) was analyzed offline by spectrum, one peak frequency per phase. Each segment changes only one thing (a packet format, a band, or one code) so any change in pitch is caused by that one thing.

**Calibration.** Every measured pitch was about 1.3 % below its target in every packet format (631.5 Hz for 640, 315.7 for 320, 157.9 for 160): a constant offset of the controller and microphone, ignored when comparing segments.

| Segment | Measured | Conclusion |
|---|---|---|
| Two-wave reference vs 14-bit format, high band (316 / 631 / 995 Hz) | Same pitches and loudness | The high band of the 14-bit format is correct |
| 7-bit `IsFm` format on the high band | 632 -> 316 -> 995 Hz | This format can change the high-band frequency |
| Low band, two-wave and 14-bit (79 / 158 / 316 Hz) | Identical | Low band behaves as already known |
| Code `24` only, 2 s | Pitch and loudness unchanged | Code `24` = hold (third confirmation) |
| Codes `12..16`, high band | 243 / 276 / 316 / 359 / 409 Hz; one application = ten applications | Preset pitches around the band default (320 Hz): -12 / -6 / 0 / +6 / +12 steps, strength kept (code `13` on the high band was about 20 dB quieter, cause unknown, not used) |
| Codes `12..16`, low band | 121 / 138 / 158 / 180 / 204 Hz | Same presets around 160 Hz |
| Codes `17..31` except `24` | Three groups: first code +1 step, middle code hold, last code -1 step, accumulating (ten applications = +-10 steps for codes 17 / 19 / 23 / 29); `23` / `25` are pure +1 / -1 step | Relative frequency steps of +-1 (a step is 1/32 octave) with an amplitude step per the 5-bit scale |

A step is 1/32 octave, the unit of the frequency formula in monograph 01. A weak earlier result for the high band of the 14-bit format came from 640 Hz vibrating very faintly in the hand (the reference segment was also weak), not from an encoding error.

**Open issue.** 6 of 10 step-code segments reached only 6-9 steps instead of 10, and two single applications (codes `22`, `25`) had no effect, so some samples are lost on the way (SDL logged 1-3 packets fewer in a few segments, or the controller dropped them). A relative step that is lost makes pitch drift, so the planner re-sends an absolute frequency for any band that last moved by a relative step, at the next packet where that band sounds. Preset codes and code `0` are absolute and need no such repair.

**Use in code.** Counting a 7-minute session of a music-heavy game before these findings, 9,253 of 20,038 packet sides (46 %) fell back to a single two-wave peak; 68 % of those fallbacks needed a new frequency only in the high band and 32 % in both bands. Using the preset codes and the +-1 step codes lets the planner change frequency every 5 ms for either band. In a later 10-minute session, the logged fallback rate was about 62 % with the change off and about 12 % with it on (a different session and count from the 46 % above).
