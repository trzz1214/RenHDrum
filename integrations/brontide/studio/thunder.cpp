// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#include <algorithm>

#include "common/settings.h"
#include "hid_core/frontend/emulated_controller.h"
#include "hid_core/hid_core.h"
#include "hid_core/hid_types.h"
#include "yuzu/brontide/thunder.h"

namespace Brontide {

namespace {

// Hand-tuning knobs: the owner feels these on a real pad and adjusts.
// Phase 1, the crack: short, high and hard.
constexpr qint64 CRACK_END_MS = 60;
constexpr float CRACK_HIGH_AMP = 1.0f;
constexpr float CRACK_HIGH_HZ = 320.0f;
constexpr float CRACK_LOW_AMP = 0.6f;
constexpr float CRACK_LOW_HZ = 160.0f;
// Phase 2, silence, so the rumble lands as a second hit.
constexpr qint64 SILENCE_END_MS = 90;
// Phase 3, the rolling rumble: low band slides down and fades out.
constexpr qint64 RUMBLE_END_MS = 750;
constexpr float RUMBLE_LOW_AMP = 0.9f;
constexpr float RUMBLE_LOW_HZ_START = 80.0f;
constexpr float RUMBLE_LOW_HZ_END = 41.0f; // Switch low band floor is 40.875 Hz
constexpr float RUMBLE_HIGH_AMP = 0.3f;
constexpr float RUMBLE_HIGH_HZ = 180.0f;
constexpr qint64 JITTER_STEP_MS = 30; // the roll: amplitude wobbles by up to +-JITTER
constexpr float JITTER = 0.1f;

constexpr int TICK_MS = 10;

} // namespace

Thunder::Thunder(Core::HID::HIDCore& hid_core_, QObject* parent)
    : QObject(parent), hid_core{hid_core_}, random{QRandomGenerator::securelySeeded()} {
    timer.setInterval(TICK_MS);
    timer.setTimerType(Qt::PreciseTimer);
    connect(&timer, &QTimer::timeout, this, &Thunder::Tick);
}

void Thunder::Play() {
    jitter = 0.0f;
    next_jitter_ms = SILENCE_END_MS;
    clock.start();
    timer.start();
    Tick();
}

void Thunder::Tick() {
    const qint64 t = clock.elapsed();

    if (t < CRACK_END_MS) {
        Send(CRACK_LOW_AMP, CRACK_LOW_HZ, CRACK_HIGH_AMP, CRACK_HIGH_HZ);
    } else if (t < SILENCE_END_MS) {
        Send(0.0f, CRACK_LOW_HZ, 0.0f, CRACK_HIGH_HZ);
    } else if (t < RUMBLE_END_MS) {
        if (t >= next_jitter_ms) {
            jitter = static_cast<float>(random.bounded(2.0) - 1.0) * JITTER;
            next_jitter_ms = t + JITTER_STEP_MS;
        }
        const float p = static_cast<float>(t - SILENCE_END_MS) /
                        static_cast<float>(RUMBLE_END_MS - SILENCE_END_MS);
        const float fade = (1.0f - p) * (1.0f - p); // ease out, like thunder rolling away
        const float low_amp = std::clamp(RUMBLE_LOW_AMP * fade + jitter * fade, 0.0f, 1.0f);
        const float low_hz = RUMBLE_LOW_HZ_START + (RUMBLE_LOW_HZ_END - RUMBLE_LOW_HZ_START) * p;
        Send(low_amp, low_hz, RUMBLE_HIGH_AMP * fade, RUMBLE_HIGH_HZ);
    } else {
        timer.stop();
        Send(0.0f, Core::HID::DEFAULT_VIBRATION_VALUE.low_frequency, 0.0f,
             Core::HID::DEFAULT_VIBRATION_VALUE.high_frequency);
    }
}

void Thunder::Send(float low_amp, float low_freq, float high_amp, float high_freq) {
    const Core::HID::VibrationValue value{
        .low_amplitude = low_amp,
        .low_frequency = low_freq,
        .high_amplitude = high_amp,
        .high_frequency = high_freq,
    };

    auto& players = Settings::values.players.GetValue();
    for (std::size_t i = 0; i < players.size(); ++i) {
        auto* controller = hid_core.GetEmulatedControllerByIndex(i);
        if (!controller || !(controller->IsConnected() || controller->IsConnected(true))) {
            continue;
        }
        // Full strength for the test even if the player turned rumble down, like Eden's own
        // vibration test (configure_vibration.cpp); restored right after
        auto& player = players[i];
        const bool old_enabled = player.vibration_enabled;
        const int old_strength = player.vibration_strength;
        player.vibration_enabled = true;
        player.vibration_strength = 100;
        controller->SetVibration(Core::HID::DeviceIndex::Left, value);
        controller->SetVibration(Core::HID::DeviceIndex::Right, value);
        player.vibration_enabled = old_enabled;
        player.vibration_strength = old_strength;
    }
}

} // namespace Brontide
