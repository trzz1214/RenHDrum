# Monograph 01: HD Rumble Packet Format & Physical Calibration

This monograph details the packet structure, channel bitfields, frequency encoding formulas, and amplitude lookup table used by Nintendo Switch HD Rumble (Linear Resonant Actuators).

---

## 1. Subcommand 0x10 Packet Structure

Nintendo Switch controllers receive vibration data via HID Output Reports containing Subcommand `0x10` (Rumble and Subcommand). A dedicated rumble packet spans 10 bytes:

```
Byte Offset | Field Description
------------+---------------------------------------------------
0x00        | Subcommand ID (0x10)
0x01        | Subcommand packet sequence counter (counter & 0x0F)
0x02..0x05  | Left Channel Encoded Rumble Word (4 bytes: L0, L1, L2, L3)
0x06..0x09  | Right Channel Encoded Rumble Word (4 bytes: R0, R1, R2, R3)
```

### The Neutral Stop Packet

When vibration halts or when resetting actuators to idle state, the controller must be sent the canonical neutral packet:

```
[0x10, counter & 0x0F, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]
```

Where `0x00, 0x01, 0x40, 0x40` represents zero amplitude at default neutral center frequencies (160 Hz low band, 320 Hz high band).

---

## 2. Standard 4-Byte Channel Encoding

Each physical channel (Left or Right actuator) is driven by two resonant bands:
1. **Low Frequency Band (LF)**: Resonates primarily between 40.0 Hz and 640.0 Hz.
2. **High Frequency Band (HF)**: Resonates primarily between 80.0 Hz and 1250.0 Hz.

The 4 bytes $(B_0, B_1, B_2, B_3)$ packing these dual-band parameters are structured as follows:

```
Byte 0: [ HF_Freq[7:0]                                              ]
Byte 1: [ HF_Amp[6:0]                      | (HF_Freq[8] ? 1 : 0)   ]
Byte 2: [ LF_Freq[6:0]                     | LF_Amp[6]              ]
Byte 3: [ (LF_Amp[5:0] << 1) | (LF_Freq[7] ? 0x80 : 0)             ]
```

### Frequency Encoding Formulas

Let $f_{\text{raw}}$ be the target frequency in Hz clamped to $[40.0, 1250.0]$.

1. **High-Band Frequency Code ($HF_{\text{freq}}$)**:
   $$HF_{\text{code}} = \text{round}\left( \log_2\left(\frac{f_{\text{high}}}{10.0}\right) \times 32.0 \right)$$
   Clamped to $[0x0004, 0x01FC]$. The 8-bit encoded value is:
   $$HF_{\text{enc}} = (HF_{\text{code}} - 0x60) \ll 2$$

2. **Low-Band Frequency Code ($LF_{\text{freq}}$)**:
   $$LF_{\text{code}} = \text{round}\left( \log_2\left(\frac{f_{\text{low}}}{10.0}\right) \times 32.0 \right)$$
   Clamped to $[0x0001, 0x007F]$. The 8-bit encoded value is:
   $$LF_{\text{enc}} = LF_{\text{code}} - 0x40$$

---

## 3. dekuNukem 101-Threshold Amplitude Table

Amplitude conversion from normalized float $[0.0, 1.0]$ to the 7-bit hardware register uses dekuNukem's reverse-engineered 101-entry non-linear power curve.

Normalized input amplitude $a \in [0.0, 1.0]$ is scaled to an unsigned 16-bit integer $v = \text{round}(a \times 65535.0)$. The table index $[0, 100]$ is found by binary search:

```rust
pub const HD_AMP_THRESHOLDS: [u16; 101] = [
    0x0000, 0x0014, 0x0018, 0x001D, 0x0023, 0x002B, 0x0034, 0x003F, 0x004C, 0x005C,
    0x0070, 0x0087, 0x00A4, 0x00C6, 0x00F0, 0x0123, 0x0160, 0x01AA, 0x0204, 0x0272,
    0x02F6, 0x0396, 0x0458, 0x0542, 0x0657, 0x07A0, 0x0926, 0x0AF3, 0x0D19, 0x0FA9,
    0x12C1, 0x1674, 0x1AE5, 0x2032, 0x2688, 0x2E10, 0x36F4, 0x4169, 0x4DB4, 0x5C20,
    0x6D14, 0x8115, 0x98A1, 0xB44A, 0xD4C1, 0xFAEB, 0x12D75, 0x168F6, 0x1B029, 0x20556,
    0x26B1E, 0x2E41C, 0x37300, 0x41B53, 0x4E0F4, 0x5C8E5, 0x6DA86, 0x81C7E, 0x997C2, 0xB5570,
    0xD6078, 0xFC792, 0x12F3BE, 0x16B198, 0x1B2AEC, 0x20846C, 0x26E866, 0x2E81DE, 0x377C8E, 0x421062,
    0x4E7A24, 0x5D0DD4, 0x6E3E64, 0x827B88, 0x9A55B2, 0xB65452, 0xD73300, 0xFDEE2C, 0x130FCE, 0x16D316,
    0x1B530E, 0x20B342, 0x271EAE, 0x2EC19C, 0x37C91C, 0x426B70, 0x4EE5E4, 0x5D8DA8, 0x6ED43C, 0x832F28,
    0x9B2F2E, 0xB7507C, 0xD85D78, 0xFF621E, 0x132BD4, 0x16F48C, 0x1B7B28, 0x20E214, 0x2754E6, 0x2F014E,
    0x38159E,
];
```

### Table Invariant Guardrail

To guarantee calibration stability across compiler targets and toolchains, `tests/guardrails.rs` locks these invariants:
- `HD_AMP_THRESHOLDS.len() == 101`
- `HD_AMP_THRESHOLDS[0] == 0`
- `HD_AMP_THRESHOLDS[100] == 65535` (scaled value)
- The sum of all elements evaluates to exactly:
  $$\sum_{i=0}^{100} T_i = 2,595,520$$
