# RenHDrum Technical Research Monographs

This series of technical monographs documents the reverse engineering, physical calibration, and real-time DSP algorithms developed for Nintendo Switch HD Rumble and PlayStation 5 DualSense Audio Haptics.

---

## Terminology & Legacy Flag Mapping

During early experimental development, several features were assigned internal working codes. The following table maps these legacy feature codes to their standardized technical designations used throughout this documentation and the public C ABI:

| Legacy Code | Internal Configuration Key | Standard Technical Name | Description |
|---|---|---|---|
| **Card 1** | `usb_no_queue` | **USB Idle-Gated No-Queue** | Non-blocking USB writes gated on HID driver idle state with peak energy merging. |
| **Card 3** | `envelope_5ms` | **5 ms Multi-Sample AM/FM Envelope (USB)** | Time-sliced 3-slot sub-packet amplitude and frequency envelope within 15 ms transmission frames. |
| **Card 5** | `frequency_in_packet` | **In-Packet Frequency Encoding** | Real-time frequency modification inside packet payloads (`FormatThree7bit` and `FormatTwo14bit`). |
| **Card 7** | `frequency_both_bands` | **Dual-Band Frequency Switching** | Independent dynamic frequency updates across both low and high resonance bands. |
| **Card 8** | `crisp_notes` | **Crisp Notes Transient Shaping (USB)** | Dynamic attack transient accentuation (+3 dB onset boost followed by -3 dB sustain decay). |
| **Card B1** | `bt_after_input` | **Bluetooth Write-After-Input Gating** | Transmitting wireless rumble packets immediately after reading an input report. |
| **Card B2** | `bt_crisp_notes` | **Crisp Notes Transient Shaping (Bluetooth)** | Attack transient shaping adapted for wireless Bluetooth transmission windows. |
| **Card B3** | `bt_fast` | **Bluetooth Idle-Gated 5 ms Framing** | Low-latency 5 ms multi-sample packet streaming over wireless links when radio conditions permit. |

---

## Monographs in This Series

1. [**01: HD Rumble Packet Format & Physical Calibration**](01-hd-rumble-packet-format.md)  
   Subcommand `0x10` byte layouts, the dekuNukem 101-threshold amplitude table, and standard 4-byte channel encoding.
2. [**02: 5 ms AM/FM Multi-Sample Envelope & Crisp Notes**](02-amfm-multisample-and-crisp-notes.md)  
   Multi-sample packet formats (`FormatThree`, `FormatThree7bit`, `FormatTwo14bit`), hardware envelope constraints, sub-5-bit floor handling, and crisp note shaping.
3. [**03: Bluetooth Congestion & Token Bucket Pacing**](03-bluetooth-congestion-and-pacing.md)  
   Half-duplex Bluetooth collision dynamics, the 150 ms refresh interval, Write-After-Input gating, and the 3-tier Token Bucket scheduler (`HD_BT_LEVELS`).
4. [**04: USB Idle-Gated Writes & Dense Rate Detection**](04-usb-idle-gated-writes.md)  
   USB 15 ms idle gating, peak energy merging (`HDSync_MergeSide`), and rolling 250 ms sample rate auto-slowdown detection (`HdDenseDetector`).
5. [**05: DualSense 48 kHz Audio Haptics DSP**](05-dualsense-audio-haptics.md)  
   4-channel 48 kHz float32 synthesis, non-linear $x^{0.5}$ perceptual power scaling, 4 ms sliding low-pass frequency filtering, and golden waveform verification.
6. [**06: SDL3 vs SDL2 HIDAPI Rumble Thread Architecture**](06-sdl3-vs-sdl2-rumble-thread.md)  
   In-depth comparative analysis of `libsdl-org/SDL` tag `release-3.4.8`, blocking delays, input report starvation, and atomic idle state tracking.
7. [**07: Integration Lessons & Edge Cases**](07-integration-lessons.md)  
   Practical lessons from production emulator integrations: unpatched runtime degradation, disconnect safety, and audio callback reliability.
8. [**08: Steam Controller Haptics — SDL3 Analysis & Planned Architecture**](08-steam-controller-haptics.md)  
   *Research only (not yet implemented, beta planned)*: SDL 3.4.8 HIDAPI transport audit for 2015 (`0x8F`) and 2026 "Triton" (`0x83`) controllers, practical roadblocks, and the planned 3rd output backend.
