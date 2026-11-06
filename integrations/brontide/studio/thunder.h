// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QElapsedTimer>
#include <QObject>
#include <QRandomGenerator>
#include <QTimer>

namespace Core::HID {
class HIDCore;
}

namespace Brontide {

/// "Try the thunder": Brontide's signature rumble, a sharp high crack, a beat of
/// silence, then a low rolling rumble that fades out. Sent to every connected controller through
/// EmulatedController::SetVibration, so it takes the same road as a game's rumble (RenHDrum hook).
/// Runs on a 10 ms UI timer for under a second, only when the button is pressed.
class Thunder : public QObject {
    Q_OBJECT

public:
    Thunder(Core::HID::HIDCore& hid_core, QObject* parent);

    void Play();

private:
    void Tick();
    void Send(float low_amp, float low_freq, float high_amp, float high_freq);

    Core::HID::HIDCore& hid_core;
    QTimer timer;
    QElapsedTimer clock;
    QRandomGenerator random;
    float jitter = 0.0f;
    qint64 next_jitter_ms = 0;
};

} // namespace Brontide
