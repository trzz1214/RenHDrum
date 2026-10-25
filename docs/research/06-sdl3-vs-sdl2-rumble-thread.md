# Monograph 06: SDL3 vs. SDL2 HIDAPI Rumble Thread Architecture

This monograph compares the HIDAPI background rumble thread architecture between **SDL 2.30** and **SDL 3** (`libsdl-org/SDL` target tag [`release-3.4.8`](https://github.com/libsdl-org/SDL/tree/release-3.4.8), commit `d9d5536704d585616d4db3c8ba3c4ff6fc2757e1`). It provides code analysis and direct source links explaining why unpatched SDL3 degrades high-frequency HD Rumble.

---

## 1. Upstream Source Code References (SDL 3.4.8)

The following analysis references specific locations in the official `libsdl-org/SDL` repository at tag `release-3.4.8`:

1. **Mandatory 10 ms Sleep in Rumble Worker**:  
   See [`src/joystick/hidapi/SDL_hidapi_rumble.c#L87-L97`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_rumble.c#L87-L97).  
   After dequeuing and writing a rumble packet, the worker thread unconditionally calls `SDL_Delay(10)`. In a 15 ms frame cycle, sleeping for 10 ms leaves a narrow, jitter-prone 5 ms window for HID transaction completion before the next tick.

2. **Input Report Starvation while Rumble Is Pending**:  
   See [`src/joystick/hidapi/SDL_hidapi_switch.c#L357-L366`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_switch.c#L357-L366).  
   When `SDL_GetAtomicInt(&ctx->device->rumble_pending) > 0`, the driver deliberately skips calling `ReadInput()`. On Bluetooth half-duplex links where output writes can take 15-40 ms, incoming motion and button reports are dropped, introducing acute input lag.

3. **Queue Locking & Need for Atomic Idle Tracking**:  
   See [`src/joystick/hidapi/SDL_hidapi_rumble.c#L181-L199`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_rumble.c#L181-L199).  
   In `SDL_HIDAPI_GetPendingRumbleLocked`, the queue is inspected under a lock, but external threads have no mechanism to determine if the hardware pipe is currently executing an I/O transaction. The patch introduces `rumble_busy` and `SDL_HIDAPI_RumbleIsIdle()` to provide atomic, non-blocking hardware state visibility.

4. **Missing Peak Merging for Subcommand 0x10**:  
   See [`src/joystick/hidapi/SDL_hidapi_switch.c#L1719-L1748`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_switch.c#L1719-L1748) and [`src/joystick/hidapi/SDL_hidapi_rumble.c#L266-L273`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_rumble.c#L266-L273).  
   Upstream SDL replaces pending simple rumble packets with the newest value, but treats Subcommand `0x10` as an opaque generic command, enqueueing every packet sequentially or dropping without energy inspection. The patch adds `HDSync_MergeSide` to retain peak tactile energy when skipping congested writes.

5. **Update Device Loop & Periodic Resend**:  
   See [`src/joystick/hidapi/SDL_hidapi_switch.c#L2763-L2832`](https://github.com/libsdl-org/SDL/blob/release-3.4.8/src/joystick/hidapi/SDL_hidapi_switch.c#L2763-L2832).  
   In `HIDAPI_DriverSwitch_UpdateDevice`, the driver executes input polling and triggers automatic retransmission of the active rumble state every 50 ms. On Bluetooth, this hardcoded 50 ms retransmission collides with half-duplex input packets.

---

## 2. Comparative Architecture: SDL 2.30 vs. SDL 3.4.8

| Architectural Characteristic | Upstream SDL 2.30 | Upstream SDL 3.4.8 | RenHDrum Patched SDL 3.4.8 |
|---|---|---|---|
| **Rumble Worker Sleep** | `SDL_Delay(10)` | `SDL_Delay(10)` | Bypassed when idle-gated no-queue is active. |
| **Bluetooth Refresh Rate** | Hardcoded 50 ms | Hardcoded 50 ms | Extended to 150 ms (`HD_RUMBLE_BT_REFRESH_MS`). |
| **Input Read During Rumble** | Blocked on pending | Blocked on pending | Interleaved via Write-After-Input gating. |
| **Subcommand 0x10 Queueing** | Unbounded FIFO queue | Unbounded FIFO queue | Non-blocking idle gating with `HDSync_MergeSide`. |
| **Public Pacing Control** | None | None | 3 C APIs (`SDL_HDRumbleSet*`). |

---

## 3. The Need for Upstream Patches

In SDL2, deep-forked emulators bypassed these limitations by modifying internal driver loops directly. In SDL3, modular design isolates controller drivers behind clean internal interfaces. 

By applying the two minimal patches in `integrations/sdl3/sdl3-patches/`, host applications can retain standard dynamic linking to `SDL3.dll` while unlocking the full, low-latency micro-timing capabilities of the RenHDrum engine.
