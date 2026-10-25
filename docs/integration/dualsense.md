# DualSense USB Audio Haptics Integration

This guide details how to integrate PlayStation 5 DualSense (`0x054C:0x0CE6` / `0x0DF2`) 4-channel 48 kHz audio haptics over a wired USB connection using SDL3 and `renhdrum.dll`.

---

## 1. Hardware Architecture

The DualSense controller features dual voice coil motors (VCM) driven by a dedicated onboard stereo audio DAC.
- **Audio Channels 0 & 1 (Front Left / Front Right)**: Headphone jack and built-in speaker.
- **Audio Channels 2 & 3 (Rear Left / Rear Right / Channels 3 & 4 in 1-indexed notation)**: Left and Right VCM haptic actuators.

> [!IMPORTANT]
> Audio haptics on DualSense requires a **wired USB connection**. Over standard Bluetooth, Windows and generic Bluetooth stacks do not expose the multichannel USB Audio Class endpoint.

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
