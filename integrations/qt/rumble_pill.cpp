// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later
// Port from Brontide (src/yuzu/brontide/rumble_pill.cpp), recoloured to Song Rung.

#include "rumble_pill.h"

#include <cmath>

#include <QConicalGradient>
#include <QCoreApplication>
#include <QElapsedTimer>
#include <QFontMetrics>
#include <QMouseEvent>
#include <QPainter>
#include <QWidget>

#include "song_rung_theme.h"

namespace RenHDrumQt {

namespace {
constexpr int HEIGHT = 32;
constexpr int PAD_LEFT = 10;
constexpr int PAD_RIGHT = 16;
constexpr int ICON = 18;
constexpr int GAP = 8;
constexpr qreal RADIUS = 10.0;
constexpr qreal MS_PER_TURN = 3000.0;
constexpr qint64 FLASH_MS = 200;
constexpr int TICK_MS = 33;

qint64 NowMs() {
    static QElapsedTimer clock = [] {
        QElapsedTimer t;
        t.start();
        return t;
    }();
    return clock.elapsed();
}

QFont PillFont(const QFont& base) {
    QFont font = base;
    font.setPixelSize(13);
    font.setWeight(QFont::DemiBold);
    return font;
}

QColor DefaultSecondary() {
    const bool oot = QCoreApplication::applicationFilePath().contains(QStringLiteral("oot"),
                                                                      Qt::CaseInsensitive);
    return oot ? BrandAccentOotMoss() : BrandAccentNs2();
}

void PaintGlowRing(QPainter& p, const QRectF& ring, qreal radius, qreal angle, int halo_alpha,
                   const QColor& primary, const QColor& secondary) {
    const auto streak = [&](int alpha) {
        QConicalGradient g(ring.center(), angle);
        const auto with = [alpha](QColor c, qreal k) {
            c.setAlphaF(alpha / 255.0 * k);
            return c;
        };
        g.setColorAt(0.0, with(secondary, 0.0));
        g.setColorAt(0.0001, with(secondary, 0.0));
        g.setColorAt(0.04, with(secondary, 1.0));
        g.setColorAt(0.14, with(primary, 1.0));
        g.setColorAt(0.25, with(primary, 0.0));
        g.setColorAt(1.0, with(primary, 0.0));
        return g;
    };
    p.save();
    p.setRenderHint(QPainter::Antialiasing);
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(QBrush(streak(halo_alpha)), 4.0));
    p.drawRoundedRect(ring, radius, radius);
    p.setPen(QPen(QBrush(streak(255)), 1.5));
    p.drawRoundedRect(ring, radius, radius);
    p.restore();
}

// Three bars of different heights with an outer arc each side: the Song Rung wave mark.
void PaintWave(QPainter& p, const QRectF& box, const QColor& primary, const QColor& secondary) {
    p.save();
    p.setRenderHint(QPainter::Antialiasing);
    p.setBrush(Qt::NoBrush);
    const qreal cx = box.center().x();
    const qreal cy = box.center().y();
    const qreal u = box.width() / 18.0;
    p.setPen(QPen(primary, 1.8 * u, Qt::SolidLine, Qt::RoundCap));
    p.drawLine(QPointF(cx - 3.5 * u, cy - 2.5 * u), QPointF(cx - 3.5 * u, cy + 2.5 * u));
    p.drawLine(QPointF(cx, cy - 6 * u), QPointF(cx, cy + 6 * u));
    p.drawLine(QPointF(cx + 3.5 * u, cy - 3.5 * u), QPointF(cx + 3.5 * u, cy + 3.5 * u));
    p.setPen(QPen(secondary, 1.5 * u, Qt::SolidLine, Qt::RoundCap));
    p.drawArc(QRectF(cx - 12 * u, cy - 5 * u, 10 * u, 10 * u), 135 * 16, 90 * 16);
    p.drawArc(QRectF(cx + 2 * u, cy - 5 * u, 10 * u, 10 * u), -45 * 16, 90 * 16);
    p.restore();
}
} // namespace

RumblePill::RumblePill(const QString& text, QWidget* parent)
    : QPushButton(text, parent), secondary(DefaultSecondary()) {
    setAttribute(Qt::WA_Hover);
    setFixedHeight(HEIGHT);
    setCursor(Qt::PointingHandCursor);
    setFocusPolicy(Qt::NoFocus);
    timer.setInterval(TICK_MS);
    connect(&timer, &QTimer::timeout, this, &RumblePill::OnTick);
}

void RumblePill::SetSecondary(const QColor& color) {
    secondary = color;
    update();
}

bool RumblePill::IsDark() const {
    return palette().color(QPalette::Window).lightness() < 128;
}

QSize RumblePill::sizeHint() const {
    const QFontMetrics fm(PillFont(font()));
    return {PAD_LEFT + ICON + GAP + fm.horizontalAdvance(text()) + PAD_RIGHT, HEIGHT};
}

void RumblePill::UpdateClock() {
    const bool run = isVisible() && window() && !window()->isMinimized();
    if (run && !timer.isActive()) {
        last_ms = NowMs();
        timer.start();
    } else if (!run && timer.isActive()) {
        timer.stop();
    }
}

void RumblePill::showEvent(QShowEvent* e) {
    QPushButton::showEvent(e);
    UpdateClock();
}

void RumblePill::hideEvent(QHideEvent* e) {
    QPushButton::hideEvent(e);
    UpdateClock();
}

bool RumblePill::event(QEvent* e) {
    const bool handled = QPushButton::event(e);
    if (e->type() == QEvent::WindowStateChange || e->type() == QEvent::ParentChange) {
        UpdateClock();
    }
    return handled;
}

void RumblePill::OnTick() {
    if (!isVisible() || (window() && window()->isMinimized())) {
        UpdateClock();
        return;
    }
    const qint64 now = NowMs();
    const qreal dt = static_cast<qreal>(now - last_ms);
    last_ms = now;
    const qreal speed = underMouse() ? 2.0 : 1.0;
    angle = std::fmod(angle - dt * 360.0 / MS_PER_TURN * speed + 360.0, 360.0);
    update();
}

void RumblePill::mousePressEvent(QMouseEvent* event) {
    flash_ms = NowMs();
    QPushButton::mousePressEvent(event);
}

void RumblePill::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    const bool hover = underMouse();
    const PaletteTokens t = Tokens(IsDark());
    const QColor primary = t.accent;

    const QRectF full(rect());
    const QRectF body = full.adjusted(0.5, 0.5, -0.5, -0.5);
    QColor fill = t.surface_raised;
    if (isDown()) {
        fill = t.accent_soft;
    } else if (hover) {
        fill = t.accent_soft;
        fill.setAlphaF(0.6);
    }
    p.setPen(QPen(t.line, 1.0));
    p.setBrush(fill);
    p.drawRoundedRect(body, RADIUS, RADIUS);

    const QRectF ring = full.adjusted(0.75, 0.75, -0.75, -0.75);
    PaintGlowRing(p, ring, RADIUS, angle, hover ? 110 : 60, primary, secondary);

    const qint64 since = NowMs() - flash_ms;
    if (since >= 0 && since < FLASH_MS) {
        QColor flash = primary;
        flash.setAlphaF(0.9 * (1.0 - static_cast<qreal>(since) / FLASH_MS));
        p.setBrush(Qt::NoBrush);
        p.setPen(QPen(flash, 1.5));
        p.drawRoundedRect(ring, RADIUS, RADIUS);
        if (!timer.isActive()) {
            QTimer::singleShot(FLASH_MS, this, qOverload<>(&QWidget::update));
        }
    }

    const QFont font = PillFont(this->font());
    const QFontMetrics fm(font);
    const int text_w = fm.horizontalAdvance(text());
    const int content_w = ICON + GAP + text_w;
    const int left = PAD_LEFT + (width() - PAD_LEFT - PAD_RIGHT - content_w) / 2;
    PaintWave(p, QRectF(left, (HEIGHT - ICON) / 2.0, ICON, ICON), primary, secondary);
    p.setFont(font);
    p.setPen(t.text);
    p.drawText(QRect(left + ICON + GAP, 0, text_w + 2, HEIGHT), Qt::AlignVCenter | Qt::AlignLeft,
               text());
}

} // namespace RenHDrumQt
