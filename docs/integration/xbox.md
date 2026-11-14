# Xbox One Rumble Integration

This guide shows how to drive a Microsoft Xbox One controller (`0x045E`, tested on the Xbox One S) with the "Xbox One Rumble" converter in `renhdrum.dll`. It turns Switch HD Rumble (two linear actuators, low and high band) into the four motors of an Xbox One pad: left grip, right grip, left trigger, right trigger.

Background and measurements: [research/10: Xbox One Rumble](../research/10-xbox-one-rumble.md).

> [!NOTE]
> **Supported Controllers**: `✅ DualSense` · `✅ Nintendo Pro Controller` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)`.

> [!WARNING]
> **CRITICAL: Enable Accurate Vibration / 15 ms Throttle Bypass**
> For authentic HD Rumble synthesis, host emulators **must** enable `Accurate Vibration` (`enable_accurate_vibrations = true`) in controller settings or bypass the 15 ms rate-limiting block in `src/hid_core/frontend/emulated_controller.cpp`. If accurate vibration is disabled, the host rate limiter swallows and drops micro-timed HD Rumble sub-packets before they reach RenHDrum!

---

## 1. When to Create the Converter

Create one converter per controller when **both** hold:

- the vendor ID is Microsoft (`0x045E`), and
- your input layer (SDL3 in Brontide) reports the device as an Xbox One type.

Pass `is_bluetooth` from the connection state (wired or wireless). If your input layer cannot tell, treat a controller as wired only when you know it is.

```c
RenHdcXboxRumble *xbox = renhdrum_xbox_rumble_create(is_bluetooth);
if (!xbox) { /* fall back to plain rumble */ }
```

> [!IMPORTANT]
> The four motors are only reachable through an input path that exposes the trigger motors: SDL `rawinput` or Windows.Gaming.Input (WGI). XInput has only two motors, and GameInput does not see the controller's motors at all (research/10, section 2.1). With XInput the grips still work; the triggers are silently dropped.

---

## 2. Feeding Game Rumble

Each time the game sets a vibration for one hand, push it as a half sample. `now_us` is a monotonic clock in microseconds.

```c
renhdrum_xbox_rumble_push_half_sample(xbox, now_us, is_right,
                                      low_amp, low_freq, high_amp, high_freq);
```

Amplitudes are 0.0 to 1.0, frequencies in Hz, as in the Switch `VibrationValue`.

---

## 3. Tick, Then Send Four Values

Call `tick` on a steady timer: **10 ms over USB, 20 ms over Bluetooth**. It returns `1` when the four motor values must be sent now, `0` when nothing changed.

```c
uint16_t motors[4]; /* left grip, right grip, left trigger, right trigger */
if (renhdrum_xbox_rumble_tick(xbox, now_us, motors) == 1) {
    SDL_RumbleJoystick(joystick, motors[0], motors[1], 2000);
    SDL_RumbleJoystickTriggers(joystick, motors[2], motors[3], 200);
}
```

Keep the grip duration longer than the tick period, and the trigger duration short (about 200 ms), so a lost "stop" cannot leave a motor running. The converter also stops itself after 750 ms without new input and ramps every motor to zero.

Calling `tick` right after each `push_half_sample` (as well as on the timer) keeps latency low.

---

## 4. Resend

Some paths refuse a rumble command until the first button press (seen with `rawinput`). If the send failed, call:

```c
renhdrum_xbox_rumble_resend(xbox);
```

The next `tick` then returns `1` with the same values, so the command is retried instead of lost.

---

## 5. Controller Removal

```c
renhdrum_xbox_rumble_destroy(xbox);
xbox = NULL;
```

Send a final all-zero rumble (`SDL_RumbleJoystick(joystick, 0, 0, 0)` and the same for triggers) before releasing the controller.

---

## 6. Threading

Each call takes an internal lock, so the functions are safe to call from different threads. Do not call `destroy` while another thread may still use the pointer. Do the actual send outside your own mutex (`tick` only fills `motors`).

---

## 7. Supported Controllers

`✅ Xbox One S`

Built and tested on the Xbox One S controller (Switch 1 games, and Switch 2 games via the Pro curve followed by the four-motor converter—confirmed in *Ocarina of Time*, *Donkey Kong Bananza*, and *Mario Kart World*). Other Microsoft Xbox One and Series X|S controllers share the same four-motor design and should work too, but they have not been tested yet.
