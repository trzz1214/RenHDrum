# Monograph 07: Practical Integration Lessons & Edge Cases

This monograph documents critical engineering lessons and runtime failure modes discovered during emulator integrations across C++ and C# runtimes.

---

## 1. Running on Unpatched Upstream SDL3

When an application loads an unmodified, official `SDL3.dll` binary:
- Calls to `SDL_HDRumbleSetNoQueue`, `SDL_HDRumbleSetNoQueueBluetooth`, or `SDL_HDRumbleSetBluetoothAfterInput` fail to resolve or return `-1`.
- If the host ignores this and forces idle-gated flags on in `RenHdcSettings`, the RenHDrum engine assumes the transport will discard busy packets without queuing.
- In reality, unpatched SDL enqueues every single packet into its internal HID thread queue, introducing severe latency buildup and dropping the rapid transient spikes required for Crisp Notes.

> [!TIP]
> **Host Rule**: When running against unpatched SDL3, disable `usb_no_queue`, `bt_after_input`, and `bt_fast`. Fall back to standard rate-limited pacing (e.g. 15 ms USB interval without idle gating).

---

## 2. Dynamic Feature Negotiation & Flag Clearing

Controllers attached via USB bridges or third-party adapters may identify as Nintendo Switch devices (`VID 0x057E`) while lacking genuine HIDAPI Subcommand `0x10` support.

When negotiating transport features:
- Patched SDL functions return `1` on success, `0` if rejected, and `-1` if the device is not a supported HIDAPI joystick.
- **Mandatory Host Action**: If a call to `SDL_HDRumbleSet*` returns anything other than `1`, the host **must immediately clear the corresponding boolean flag in `RenHdcSettings`** before calling `renhdrum_engine_apply_settings`.
- Allowing a mismatched configuration between engine expectations and transport reality causes desynchronization and missed packets.

---

## 3. Hotplugging, Sudden Disconnects, and Deadlock Prevention

Controller disconnects (unplugging the USB cable or turning off Bluetooth) represent the most frequent source of use-after-free crashes and deadlocks:

1. **Deadlock in Driver Callbacks**:
   Never call `SDL_SendJoystickEffect` while holding an engine or controller lock. If the SDL driver thread attempts to trigger a disconnect callback that requests the same lock while the tick loop waits on SDL I/O, a permanent deadlock ensues.
2. **Orderly Teardown Protocol**:
   Before invoking `SDL_CloseJoystick`:
   - Send the canonical 10-byte neutral stop packet (`0x10, 0, 0x00, 0x01, 0x40, 0x40, ...`).
   - Call `renhdrum_engine_destroy(engine)`.
   - Set the engine and joystick pointers to null.
   - Finally, call `SDL_CloseJoystick(joystick)`.
3. **Lock Ordering Invariant**:
   In multi-threaded hosts, establish a strict lock hierarchy (e.g., dispatch lock first, synchronization lock second) and never invert the acquisition order.

---

## 4. Robust DualSense Audio Stream Safety

DualSense audio haptics operate directly inside the OS real-time audio pipeline:
- **Audio Device Latency**: On Windows, WASAPI may take up to 2 seconds after HID enumeration before the 4-channel audio stream can be opened. The host must implement an asynchronous retry loop rather than failing permanently on the first attempt.
- **Zero Buffer on DSP Error**: If `renhdrum_dualsense_dsp_fill_f32` returns a negative error code (e.g. invalid buffer size or null handle), the audio callback must immediately zero out the buffer with `memset`. Leaving uninitialized memory in the stream produces loud bursts of static or DC offset that can damage voice coil actuators.
- **Suppression of Default HID Rumble**: Once the audio stream is active, standard HID motor rumble must be silenced (`SDL_RumbleJoystick(joystick, 0, 0, 0)`), preventing physical motor counter-weights from canceling subtle audio haptic vibrations.
