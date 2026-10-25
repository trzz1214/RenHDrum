# Monograph 03: Bluetooth Congestion & Token Bucket Pacing

This monograph examines the root causes of packet congestion, input lag, and wireless disconnects over Bluetooth, presenting the mathematical design of the 150 ms refresh timer, Write-After-Input gating, and the 3-tier Token Bucket scheduler.

---

## 1. Anatomy of Bluetooth Congestion in Upstream SDL

Bluetooth Classic (Basic Rate / Enhanced Data Rate) operates as a **half-duplex** radio link: a device cannot transmit and receive simultaneously on the same channel. In standard emulation configurations:

1. **Aggressive 50 ms Resend Period**: Upstream SDL repeatedly re-sends the active vibration packet every 50 ms even when the vibration state has not changed.
2. **Blocking Sleep**: After dispatching a HID rumble report, upstream SDL executes a mandatory `SDL_Delay(10)` (10 ms sleep) inside the rumble worker thread.
3. **Input Starvation**: While an output report is queued (`SDL_GetAtomicInt(&ctx->device->rumble_pending) > 0`), the driver skips calling `ReadInput()`.

When a game dispatches vibration updates at 60 Hz (every ~16.6 ms) while the controller streams gyro and button input at 66 Hz (every ~15 ms), the Bluetooth radio queue saturates. Output reports collide with incoming packets, resulting in severe packet loss, delayed button inputs (up to 150-300 ms lag), and sudden wireless disconnects.

---

## 2. Pacing Solutions

To resolve radio saturation without sacrificing responsiveness, RenHDrum and the patched SDL3 transport layer introduce three coordinated mechanisms:

### A. Extended 150 ms Bluetooth Refresh Interval
When the vibration amplitude is steady, the keep-alive retransmission interval is increased from 50 ms to **150 ms** (`HD_RUMBLE_BT_REFRESH_MS`). This reduces wireless transmission traffic by $66.7\%$ during sustained rumble, keeping the radio clear for input reports.

### B. Write-After-Input Gating
Instead of dispatching rumble packets asynchronously at arbitrary clock intervals, the host gates Bluetooth writes immediately after receiving an input report:
1. When an input report arrives, the controller radio has just finished transmitting and entered receive mode.
2. The patched driver immediately dispatches the pending rumble packet.
3. A minimum spacing of **20 ms** (`HD_RUMBLE_BT_WRITE_MS`) is strictly enforced between consecutive output writes.

### C. Idle-Gated 5 ms Framing
On high-quality Bluetooth controllers (e.g. Intel/Broadcom Bluetooth 5.0+ adapters), the driver checks whether the HID output endpoint is idle before dispatching. If the radio is busy, the write is dropped or merged with the next window rather than queued.

---

## 3. The 3-Tier Token Bucket Scheduler (`HD_BT_LEVELS`)

To accommodate varying radio environments (crowded 2.4 GHz RF spectrum, USB dongle distances, and different Bluetooth stack implementations), RenHDrum provides a calibrated 3-tier Token Bucket pacing algorithm:

```
+---------------------------------------------------------------------------------------+
| Level 1: Stable       | Interval: 60 ms | Burst: 2.0 | Refill: 100 ms | Amp Steps: 2  |
| Level 2: Balanced     | Interval: 45 ms | Burst: 3.0 | Refill:  80 ms | Amp Steps: 1  |
| Level 3: Quality      | Interval: 35 ms | Burst: 4.0 | Refill:  65 ms | Amp Steps: 0  |
+---------------------------------------------------------------------------------------+
```

### Physical Calibration Parameters

```rust
// In src/encoding/standard.rs and tests/guardrails.rs
pub struct HdBtLevel {
    pub min_interval_ms: i64,
    pub burst_capacity: f32,
    pub refill_ms: i64,
    pub max_amp_step: i32,
    pub max_low_freq_step: i32,
    pub max_high_freq_step: i32,
    pub stop_priority: bool,
}

pub const HD_BT_LEVELS: [HdBtLevel; 3] = [
    // Level 1: Stable - Highly congested environments
    HdBtLevel {
        min_interval_ms: 60,
        burst_capacity: 2.0,
        refill_ms: 100,
        max_amp_step: 2,
        max_low_freq_step: 16,
        max_high_freq_step: 4,
        stop_priority: false,
    },
    // Level 2: Balanced - Typical desktop environments (Recommended default)
    HdBtLevel {
        min_interval_ms: 45,
        burst_capacity: 3.0,
        refill_ms: 80,
        max_amp_step: 1,
        max_low_freq_step: 8,
        max_high_freq_step: 2,
        stop_priority: true,
    },
    // Level 3: Quality - Clean RF conditions, maximum tactile fidelity
    HdBtLevel {
        min_interval_ms: 35,
        burst_capacity: 4.0,
        refill_ms: 65,
        max_amp_step: 0,
        max_low_freq_step: 4,
        max_high_freq_step: 1,
        stop_priority: true,
    },
];
```

### Operational Dynamics

- **`stop_priority = true`**: When the game commands a stop (amplitude zero), the token bucket allows the stop packet to preempt regular rate limits, ensuring vibration cuts off instantly without lingering buzz.
- **Slew Rate Clamping**: `max_amp_step`, `max_low_freq_step`, and `max_high_freq_step` restrict large instantaneous jumps, smoothing the frequency gradient across Bluetooth packets.
- **Burst Allowance**: During brief combat explosions or note attacks, the bucket permits up to $N$ rapid bursts before enforcing the steady-state interval.
