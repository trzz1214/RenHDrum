# Monograph 04: USB Idle-Gated Writes & Dense Rate Detection

This monograph details the USB Idle-Gated No-Queue transmission protocol, the peak energy merging algorithm (`HDSync_MergeSide`), and the real-time sample density detector (`HdDenseDetector`).

---

## 1. USB Idle-Gated No-Queue Architecture

Over a wired USB connection, Nintendo Switch controllers accept HID Output Reports at an optimal frame spacing of **15 ms** (approximately 66.6 Hz).

In conventional implementations:
- Every rumble sample creates an asynchronous HID write request that enters a FIFO queue.
- If the operating system USB host controller delays packet delivery by even a few milliseconds, the FIFO queue backs up.
- The player experiences tactile latency as stale packets slowly drain, followed by stuttering bursts.

### The No-Queue Gating Paradigm

With **USB Idle-Gated No-Queue** enabled:
1. When a new 15 ms window ticks, the driver inspects an atomic flag (`rumble_busy`).
2. If the USB HID driver thread is currently idle, the new packet is written immediately.
3. If the driver thread is currently busy executing an earlier write, the incoming packet **does not queue**. Instead, its peak energy is merged with the pending packet, and the host continues without blocking.

---

## 2. Peak Energy Merging (`HDSync_MergeSide`)

When a write cannot be dispatched immediately, dropping the packet entirely would risk losing sharp transient impacts. To solve this, patched SDL3 implements the `HDSync_MergeSide` algorithm:

```c
// In integrations/sdl3/sdl3-patches/0002-hdrumble-idle-write.patch
static void HDSync_MergeSide(Uint8 *dst, const Uint8 *src) {
    // Decode incoming and pending amplitude levels
    Uint16 dst_amp = DecodeAmplitude(dst);
    Uint16 src_amp = DecodeAmplitude(src);

    // Retain the maximum energy envelope
    if (src_amp > dst_amp) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = src[3];
    }
}
```

By computing the supremum of both signals, the strongest tactile impulse is always preserved, while eliminating queue latency entirely.

---

## 3. High-Density Sample Rate Detection (`HdDenseDetector`)

Certain emulated games stream vibration updates at unusually high frequencies (e.g. musical stage mechanics or rapid UI scrolling that generate 140 to 155 samples per second).

At standard 15 ms spacing:
- Packets arrive faster than the 15 ms frame rate, forcing erratic 15-23 ms packet gaps.
- Peak-hold interpolation jumps erratically between successive micro-notes, producing chaotic, dissonant buzz.
- Relaxing the frame spacing to **30 ms** during high-density bursts restores clean rhythmic articulation.

### Detection State Machine

`HdDenseDetector` continuously monitors incoming sample frequency across rolling **250 ms fixed windows**:

```
               >= 90 samples/s in 2 consecutive windows
   [ Normal 15 ms ] ----------------------------------> [ Dense 30 ms ]
                    <----------------------------------
                < 60 samples/s for 4 consecutive windows
```

1. **Entering Dense State**:
   - The detector counts samples in 250 ms windows.
   - If two consecutive windows exhibit $\ge 90\text{ samples/sec}$ (i.e. $\ge 22\text{ samples}$ per 250 ms window), the engine enters the `Dense` state.
   - Target pacing relaxes smoothly from 15 ms to 30 ms (`usb_auto_slow`).
2. **Leaving Dense State**:
   - If sample density drops below $60\text{ samples/sec}$ for four consecutive windows (1 full second of quiet or moderate traffic), the engine returns to the standard 15 ms pacing.

### Mathematical Invariants

- **Window Duration**: Exactly $250\text{ ms}$ (measured on monotonic microsecond clock).
- **Trigger Threshold**: $90\text{ samples/sec}$ over $\ge 500\text{ ms}$.
- **Hysteresis Recovery Window**: $60\text{ samples/sec}$ over $\ge 1000\text{ ms}$.
- **Zero-Allocation**: The detector maintains state in fixed primitive fields (`i64`, `i32`, `bool`) without allocating memory.
