// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later
// Port from Brontide (src/yuzu/brontide/rumble_pill.h), recoloured to Song Rung.

#pragma once

#include <QColor>
#include <QPushButton>
#include <QTimer>

namespace RenHDrumQt {

/// The HD Rumble pill: rounded button with a wave icon and a light streak that runs around its
/// border (one turn in 3 s, twice as fast on hover, a flash on click). Paints itself entirely.
/// The clock only runs while the pill is visible and its window is not minimised.
/// Streak colours: Song Rung orange plus the app colour (sky for Brontide-NS2, moss for
/// Brontide-oot, picked from the executable name unless `SetSecondary` is called).
class RumblePill : public QPushButton {
    Q_OBJECT

public:
    explicit RumblePill(const QString& text, QWidget* parent = nullptr);

    void SetSecondary(const QColor& color);

    QSize sizeHint() const override;
    QSize minimumSizeHint() const override {
        return sizeHint();
    }

protected:
    void paintEvent(QPaintEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void showEvent(QShowEvent* event) override;
    void hideEvent(QHideEvent* event) override;
    bool event(QEvent* event) override;

private:
    void OnTick();
    void UpdateClock();
    bool IsDark() const;

    QTimer timer;
    QColor secondary;
    qreal angle = 0.0; // degrees
    qint64 last_ms = 0;
    qint64 flash_ms = -1000;
};

} // namespace RenHDrumQt
