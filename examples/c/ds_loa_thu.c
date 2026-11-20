#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <SDL3/SDL.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

static void send_restore_speaker(SDL_Gamepad *gamepad) {
    uint8_t effect_off[47];
    memset(effect_off, 0, sizeof(effect_off));
    effect_off[0] = 0x80;
    effect_off[7] = 0x00;
    if (!SDL_SendGamepadEffect(gamepad, effect_off, sizeof(effect_off))) {
        fprintf(stderr, "Warning: failed to restore DualSense audio path: %s\n", SDL_GetError());
    }
}

static bool play_tone(SDL_AudioStream *stream, double freq_hz, float amp, int ch_mask, int duration_ms) {
    const int sample_rate = 48000;
    const int frames = (sample_rate * duration_ms) / 1000;
    const size_t total_samples = (size_t)frames * 4;
    float *buf = (float *)calloc(total_samples, sizeof(float));
    if (!buf) {
        fprintf(stderr, "Error: out of memory allocating %zu audio samples.\n", total_samples);
        return false;
    }

    const double phase_step = (2.0 * M_PI * freq_hz) / (double)sample_rate;
    for (int i = 0; i < frames; ++i) {
        const float sample = amp * (float)sin(phase_step * (double)i);
        for (int ch = 0; ch < 4; ++ch) {
            if ((ch_mask & (1 << ch)) != 0) {
                buf[(size_t)i * 4 + (size_t)ch] = sample;
            }
        }
    }

    const int bytes = (int)(total_samples * sizeof(float));
    if (!SDL_PutAudioStreamData(stream, buf, bytes)) {
        fprintf(stderr, "Error: SDL_PutAudioStreamData failed: %s\n", SDL_GetError());
        free(buf);
        return false;
    }
    free(buf);

    while (SDL_GetAudioStreamQueued(stream) > 0) {
        SDL_Delay(20);
    }
    SDL_Delay(150);
    return true;
}

int main(void) {
    if (!SDL_Init(SDL_INIT_GAMEPAD | SDL_INIT_AUDIO)) {
        fprintf(stderr, "Error: SDL_Init failed: %s\n", SDL_GetError());
        return 1;
    }
    SDL_PumpEvents();

    int pad_count = 0;
    SDL_JoystickID *pads = SDL_GetGamepads(&pad_count);
    SDL_Gamepad *gamepad = NULL;
    for (int i = 0; pads && i < pad_count; ++i) {
        SDL_Gamepad *cand = SDL_OpenGamepad(pads[i]);
        if (!cand) {
            continue;
        }
        if (SDL_GetRealGamepadType(cand) == SDL_GAMEPAD_TYPE_PS5) {
            gamepad = cand;
            break;
        }
        SDL_CloseGamepad(cand);
    }
    SDL_free(pads);

    if (!gamepad) {
        fprintf(stderr, "Error: No PS5 DualSense controller found (detected %d gamepad(s)).\n", pad_count);
        SDL_Quit();
        return 1;
    }

    const char *pad_name = SDL_GetGamepadName(gamepad);
    if (!pad_name) {
        pad_name = "DualSense";
    }
    const SDL_JoystickConnectionState conn = SDL_GetGamepadConnectionState(gamepad);
    if (conn != SDL_JOYSTICK_CONNECTION_WIRED) {
        fprintf(stderr,
                "Error: '%s' is not connected via USB cable (state=%d). Please connect via USB.\n",
                pad_name, (int)conn);
        SDL_CloseGamepad(gamepad);
        SDL_Quit();
        return 1;
    }
    printf("Opened wired DualSense: %s\n", pad_name);

    uint8_t effect_on[47];
    memset(effect_on, 0, sizeof(effect_on));
    effect_on[0] = 0xA0;
    effect_on[1] = 0x80;
    effect_on[5] = 100;
    effect_on[7] = 0x30;
    effect_on[37] = 0x02;
    if (!SDL_SendGamepadEffect(gamepad, effect_on, sizeof(effect_on))) {
        fprintf(stderr, "Error: SDL_SendGamepadEffect(enable speaker) failed: %s\n", SDL_GetError());
        SDL_CloseGamepad(gamepad);
        SDL_Quit();
        return 1;
    }
    printf("Sent USB 47-byte speaker enable effect.\n");

    int dev_count = 0;
    SDL_AudioDeviceID *devices = SDL_GetAudioPlaybackDevices(&dev_count);
    SDL_AudioDeviceID matched_dev = 0;
    char matched_name[256];
    matched_name[0] = '\0';

    for (int i = 0; devices && i < dev_count; ++i) {
        const char *name = SDL_GetAudioDeviceName(devices[i]);
        SDL_AudioSpec dev_spec;
        SDL_zero(dev_spec);
        const int channels =
            SDL_GetAudioDeviceFormat(devices[i], &dev_spec, NULL) ? dev_spec.channels : 0;
        if (name && channels >= 4 &&
            (strstr(name, "DualSense") != NULL || strstr(name, "Wireless Controller") != NULL)) {
            matched_dev = devices[i];
            snprintf(matched_name, sizeof(matched_name), "%s", name);
            break;
        }
    }

    if (matched_dev == 0) {
        fprintf(stderr, "Error: No 4-channel DualSense audio playback device found. Available devices:\n");
        for (int i = 0; devices && i < dev_count; ++i) {
            const char *name = SDL_GetAudioDeviceName(devices[i]);
            SDL_AudioSpec dev_spec;
            SDL_zero(dev_spec);
            const int channels =
                SDL_GetAudioDeviceFormat(devices[i], &dev_spec, NULL) ? dev_spec.channels : 0;
            fprintf(stderr, "  - \"%s\" (%d ch)\n", name ? name : "?", channels);
        }
        SDL_free(devices);
        send_restore_speaker(gamepad);
        SDL_CloseGamepad(gamepad);
        SDL_Quit();
        return 1;
    }
    SDL_free(devices);

    printf("Opened 4-channel audio device: %s\n", matched_name);
    const SDL_AudioSpec spec = {SDL_AUDIO_F32, 4, 48000};
    SDL_AudioStream *stream = SDL_OpenAudioDeviceStream(matched_dev, &spec, NULL, NULL);
    if (!stream) {
        fprintf(stderr, "Error: SDL_OpenAudioDeviceStream failed: %s\n", SDL_GetError());
        send_restore_speaker(gamepad);
        SDL_CloseGamepad(gamepad);
        SDL_Quit();
        return 1;
    }
    if (!SDL_ResumeAudioStreamDevice(stream)) {
        fprintf(stderr, "Error: SDL_ResumeAudioStreamDevice failed: %s\n", SDL_GetError());
        SDL_DestroyAudioStream(stream);
        send_restore_speaker(gamepad);
        SDL_CloseGamepad(gamepad);
        SDL_Quit();
        return 1;
    }

    bool ok = true;
    printf("(a) Playing 1.5 s sine 1000 Hz (amp 0.2) on channel 1 ONLY (DualSense speaker)...\n");
    fflush(stdout);
    ok = ok && play_tone(stream, 1000.0, 0.2f, 1 << 1, 1500);
    SDL_Delay(200);

    printf("(b) Playing 1.5 s sine 1000 Hz (amp 0.2) on channel 0 ONLY (headphone left)...\n");
    fflush(stdout);
    ok = ok && play_tone(stream, 1000.0, 0.2f, 1 << 0, 1500);
    SDL_Delay(200);

    printf("(c) Playing 1.5 s sine 150 Hz (amp 0.3) on channels 2 + 3 (haptic motors)...\n");
    fflush(stdout);
    ok = ok && play_tone(stream, 150.0, 0.3f, (1 << 2) | (1 << 3), 1500);

    SDL_PauseAudioStreamDevice(stream);
    SDL_ClearAudioStream(stream);
    SDL_DestroyAudioStream(stream);
    send_restore_speaker(gamepad);
    SDL_CloseGamepad(gamepad);
    SDL_Quit();

    printf("Done (restored audio path).\n");
    return ok ? 0 : 1;
}
