# DualSense HD Haptics Integration

This guide details how to integrate PlayStation 5 DualSense (`0x054C:0x0CE6` / `0x0DF2`) with `renhdrum.dll`: 4-channel 48 kHz audio haptics over a wired USB connection using SDL3 (sections 1-5), and **HD Haptics**, the motors, speaker and L2/R2 triggers driven together, over USB and Bluetooth (section 6).

> [!NOTE]
> **Supported Controllers**: `✅ DualSense` · `✅ Nintendo Pro Controller` · `✅ Xbox One S` · `🧪 Steam Controller 2026 (experimental)`.

> [!WARNING]
> **CRITICAL: Enable Accurate Vibration / 15 ms Throttle Bypass**
> For authentic HD Rumble synthesis, host emulators **must** enable `Accurate Vibration` (`enable_accurate_vibrations = true`) or bypass the 15 ms rate-limiting block in `src/hid_core/frontend/emulated_controller.cpp`. If accurate vibration is disabled, the host rate limiter swallows and drops micro-timed HD Rumble sub-packets before they reach RenHDrum!

---

## 1. Hardware Architecture

The DualSense controller features dual voice coil motors (VCM) driven by a dedicated onboard stereo audio DAC.
- **Audio Channels 0 & 1 (Front Left / Front Right)**: Headphone jack and built-in speaker.
- **Audio Channels 2 & 3 (Rear Left / Rear Right / Channels 3 & 4 in 1-indexed notation)**: Left and Right VCM haptic actuators.

> [!IMPORTANT]
> The 4-channel audio path below needs a **wired USB connection**: Windows and generic Bluetooth stacks do not expose the multichannel USB Audio Class endpoint. Over Bluetooth, RenHDrum sends the same three parts inside one HID report instead (section 6.3).

---

## 2. Audio Device Discovery & Retry Loop

When a DualSense controller is plugged in via USB, Windows enumerates the HID interface almost instantly, while the USB Audio Class endpoint can take 1 to 2 seconds to initialize in the WASAPI audio subsystem.

Implement a periodic retry check (e.g. every 2 seconds) to find the playback device:

```cpp
int FindDualSenseAudioDevice() {
    int num_devices = 0;
    SDL_AudioDeviceID *devices = SDL_GetAudioPlaybackDevices(&num_devices);
    if (!devices) return 0;

    SDL_AudioDeviceID target_id = 0;
    for (int i = 0; i < num_devices; ++i) {
        const char *name = SDL_GetAudioDeviceName(devices[i]);
        if (!name) continue;

        // Search for DualSense or Wireless Controller in device name
        if (strstr(name, "DualSense") || strstr(name, "Wireless Controller")) {
            SDL_AudioSpec spec;
            if (SDL_GetAudioDeviceFormat(devices[i], &spec, nullptr)) {
                // Must support at least 4 channels
                if (spec.channels >= 4) {
                    target_id = devices[i];
                    break;
                }
            }
        }
    }

    SDL_free(devices);
    return target_id;
}
```

---

## 3. Opening the 4-Channel 48 kHz Audio Stream

Configure the audio stream with exact hardware parameters:

```cpp
SDL_AudioSpec desired_spec;
SDL_zero(desired_spec);
desired_spec.format = SDL_AUDIO_F32;
desired_spec.channels = 4;
desired_spec.freq = 48000;

SDL_AudioStream *stream = SDL_OpenAudioDeviceStream(
    target_device_id,
    &desired_spec,
    AudioStreamCallback,
    user_data
);

if (!stream) {
    // Log and schedule retry
    return;
}

// Disable SDL HID rumble once audio stream is active to prevent competing motor vibrations
SDL_RumbleJoystick(joystick, 0, 0, 0);
SDL_ResumeAudioStreamDevice(stream);
```

---

## 4. Audio Callback & Zeroing Buffer on Error

In the real-time audio callback:
1. Call `renhdrum_dualsense_dsp_fill_f32`.
2. **Safety rule**: If the synthesizer returns an error code `< 0`, immediately zero out the entire output buffer using `memset` to prevent loud pops or DC offset humming.

```cpp
void SDLCALL AudioStreamCallback(void *userdata, SDL_AudioStream *stream, int additional_amount, int total_amount) {
    if (additional_amount <= 0) return;

    int total_floats = additional_amount / sizeof(float);
    // Ensure float count is a multiple of 4 channels
    total_floats -= (total_floats % 4);

    std::vector<float> buffer(total_floats);

    std::lock_guard<std::mutex> lock(dsp_mutex);
    if (dsp) {
        int res = renhdrum_dualsense_dsp_fill_f32(dsp, buffer.data(), total_floats);
        if (res < 0) {
            // Error occurred: fill with silence
            std::fill(buffer.begin(), buffer.end(), 0.0f);
        }
    } else {
        std::fill(buffer.begin(), buffer.end(), 0.0f);
    }

    SDL_PutAudioStreamData(stream, buffer.data(), buffer.size() * sizeof(float));
}
```

---

## 5. Routing Emulated Vibration to DSP

Whenever new vibration data arrives from the emulated Switch game:

```cpp
std::lock_guard<std::mutex> lock(dsp_mutex);
if (dsp) {
    renhdrum_dualsense_dsp_set_target(dsp,
        left_al, left_fl, left_ah, left_fh,
        right_al, right_fl, right_ah, right_fh
    );
}
```

Or for individual channels:
```cpp
renhdrum_dualsense_dsp_set_half_target(dsp, is_right, al, fl, ah, fh);
```

### Switch 2 Mode Routing
When dispatching Switch 2 guest IPC command 214 (`SendNx2VibrationValues`), call `renhdrum_set_nx2_mode(2)`. The DSP automatically maps vibration amplitudes through the fixed Mode 2 law (`a <= 0.001 -> 0`, else `min(1, 3.5 * a^0.75)`, no EQ) before audio synthesis, matching physical voice-coil dynamics. For classic Switch 1 games, set `renhdrum_set_nx2_mode(0)` to keep amplitudes 100% bit-exact and untouched.

---

## 6. HD Haptics: motors, speaker and triggers

HD Haptics upgrades HD Rumble on the DualSense. The game still sends only HD Rumble; RenHDrum splits that one signal between the three parts of the controller. It is **on by default** and has one switch (`dualsense_hd_haptics`, shown as **HD Haptics** in the settings page; in `RenHdcConfig` the field is `dualsense_hd_haptics_off`, `false` = on). With the switch off the DualSense rumbles exactly like before.

| | Wired USB | Bluetooth |
|---|---|---|
| Motors | Yes (audio channels 2-3) | Yes (report `0x36`) |
| Speaker | Yes (audio channel 1) | Yes (Opus inside report `0x36`) |
| L2/R2 triggers | Yes | Yes |
| Switch | HD Haptics | same switch |
| Restart needed | No | No |

### 6.1 What each part does
- **Motors, studio style.** The usual rumble law, plus heavy hits and explosions pulled down to 75 Hz for a deeper thump (lowered only, never raised).
- **Speaker.** The speaker taps by kind of sound (rain and menu clicks, gravel, metal, long friction as noise), with four variants of each so it does not repeat. It stays quiet when the game plays music notes. The level is `RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT` (`60`).
- **Triggers.** They kick back only while the player holds them, with one rule for every game (sharp hit, heavy hit, explosion). The rule uses only the rumble signal and the buttons the player presses; nothing is read from the game. Music and constant rumble leave the triggers quiet.

### 6.2 Wired USB
Keep the 4-channel stream of sections 3-5. The speaker and the triggers are opened and driven by 47-byte effects blocks that the core builds and the host sends with HID:

```c
uint8_t block[47];
renhdrum_hd_haptics_speaker_block(1 /* on, 0 = release */, block);   // open the speaker
renhdrum_dualsense_dsp_set_speaker(dsp, RENHDRUM_HD_HAPTICS_SPEAKER_PERCENT);

// Triggers: one RenHdcTriTrigger per pad, updated from the rumble and the trigger buttons.
RenHdcTriTrigger *tri = renhdrum_tri_trigger_create();
// renhdrum_tri_trigger_update(...) fills an 11-byte effect per trigger (1 = changed, 0 = unchanged);
// renhdrum_hd_haptics_trigger_block(is_right, effect11_or_null, block) turns it into the block (NULL = release).
// Call renhdrum_tri_trigger_note_action(tri, now_ms) every tick while the player holds another button.
```

Send trigger blocks from **one thread only**, release the speaker and the triggers on unplug and on exit, and give each wired DualSense its own audio stream. Signatures: [c-abi-reference.md](c-abi-reference.md) and `include/renhdrum.h`.

### 6.3 Bluetooth
There is no audio device over Bluetooth, so everything goes into the combined HID report `0x36` (398 bytes, one every `RENHDRUM_DS5_BT_PERIOD_NS`, 10.667 ms): rumble, triggers and the speaker. The host only calls one function:

```c
uint8_t report[RENHDRUM_DS5_BT36_REPORT_SIZE];
int32_t n = renhdrum_dualsense_dsp_build_bt36_report(dsp, state47_or_null, 0 /* seq, unused */, report);
if (n == 1) { /* send report with HID output report 0x36 */ }
// 0 = nothing to send (idle for 2 s), < 0 = error.
```

- `state47_or_null` carries the trigger block (the same 47-byte block as USB); `NULL` leaves the triggers alone.
- Also call `renhdrum_dualsense_dsp_set_speaker` on Bluetooth. If you never do, the core turns the speaker on by itself at the HD Haptics level, and turns it off again when you go back to the plain `0x32` builder.
- The speaker audio is compressed with Opus (48 kHz stereo, constant 160 kb/s, exactly 200 bytes per report) inside the core. The encoder (libopus 1.5.2, BSD-3-Clause) is linked into `renhdrum.dll`: nothing extra to install, no extra DLL, and the send path allocates no memory.
- With HD Haptics off, or the speaker at 0, the report is the same as before the speaker was added.
- If a Bluetooth write fails, restart the Bluetooth worker.

The `0x36` layout and the Opus speaker framing were learned from Enhanced-PS5-DualSense-Haptics-Bridge by ragnarok179 (MIT), then written again in Rust. See [CREDITS](../CREDITS.md).

### 6.4 Where it runs
Brontide-NS2, Brontide-oot, Brontide (Switch 1), totk-pc-recomp, dkb-pc-recomp, mkw-recomp and Longshot, through the `recomp-c` bridge or the Eden patch series. The host pages are the [recompiled PC ports guide](recomp-games.md) and [Switch 2 games](switch2-games.md).
