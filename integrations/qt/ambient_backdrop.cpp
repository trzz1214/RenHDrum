// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later
// Port from Brontide (src/yuzu/brontide/ambient_backdrop.cpp).

#include "ambient_backdrop.h"

#include <QAbstractItemView>
#include <QEasingCurve>
#include <QEvent>
#include <QLinearGradient>
#include <QPainter>
#include <QPalette>
#include <QRadialGradient>
#include <QScrollBar>
#include <QWidget>

namespace RenHDrumQt {

namespace {
const QColor DARK_BASE(0x15, 0x14, 0x13);
const QColor LIGHT_BASE(0xF7, 0xF5, 0xF2);
constexpr int FADE_MS = 300;
} // namespace

AmbientBackdrop::AmbientBackdrop(QWidget* target_) : QObject(target_), target{target_} {
    if (target) {
        target->installEventFilter(this);
    }
    fade.setStartValue(0.0);
    fade.setEndValue(1.0);
    fade.setDuration(FADE_MS);
    fade.setEasingCurve(QEasingCurve::OutCubic);
    connect(&fade, &QVariantAnimation::valueChanged, this, [this] { RepaintAll(); });
}

void AmbientBackdrop::AttachItemView(QAbstractItemView* view) {
    if (!view || views.contains(view)) {
        return;
    }
    views.push_back(view);
    QWidget* viewport = view->viewport();
    if (!viewport) {
        return;
    }
    viewport->installEventFilter(this);
    for (auto* bar : {view->verticalScrollBar(), view->horizontalScrollBar()}) {
        if (bar) {
            connect(bar, &QScrollBar::valueChanged, viewport, [viewport] { viewport->update(); });
        }
    }
}

void AmbientBackdrop::SetCover(const QPixmap& next) {
    if (next.cacheKey() == cover.cacheKey()) {
        return;
    }
    previous_cover = cover;
    previous_cached = cached;
    previous_cached_dark = cached_dark;
    cover = next;
    cached = {};
    fade.stop();
    fade.start();
    RepaintAll();
}

void AmbientBackdrop::SetEnabled(bool on) {
    if (enabled == on) {
        return;
    }
    enabled = on;
    RepaintAll();
}

void AmbientBackdrop::SetDarkMode(std::optional<bool> dark) {
    if (forced_dark == dark) {
        return;
    }
    forced_dark = dark;
    cached = {};
    previous_cached = {};
    RepaintAll();
}

bool AmbientBackdrop::EffectiveDark() const {
    if (forced_dark.has_value()) {
        return *forced_dark;
    }
    if (target) {
        return target->palette().color(QPalette::Window).lightness() < 128;
    }
    return true;
}

void AmbientBackdrop::RepaintAll() {
    if (!target || !target->isVisible()) {
        return;
    }
    target->update();
    for (const auto& view : views) {
        if (view && view->isVisible() && view->viewport()) {
            view->viewport()->update();
        }
    }
}

QPixmap AmbientBackdrop::Render(const QPixmap& source, const QSize& size, bool dark) const {
    if (size.isEmpty()) {
        return {};
    }
    QPixmap out(size);
    out.fill(dark ? DARK_BASE : LIGHT_BASE);
    if (source.isNull()) {
        return out;
    }

    QPainter painter(&out);
    painter.setRenderHint(QPainter::SmoothPixmapTransform);

    const QColor average = source.scaled(1, 1, Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
                               .toImage()
                               .pixelColor(0, 0);
    QRadialGradient glow(QPointF(size.width() * 0.5, size.height() * 0.2), size.width() * 0.75);
    QColor glow_color = dark ? average.darker(175) : average;
    glow_color.setAlpha(dark ? 115 : 75);
    glow.setColorAt(0.0, glow_color);
    glow.setColorAt(1.0, Qt::transparent);
    painter.fillRect(out.rect(), glow);

    const QPixmap tiny = source.scaledToWidth(24, Qt::SmoothTransformation)
                             .scaledToWidth(96, Qt::SmoothTransformation);
    const QSize expanded = size * 1.25;
    const QPixmap blurred =
        tiny.scaled(expanded, Qt::KeepAspectRatioByExpanding, Qt::SmoothTransformation);
    painter.setOpacity(dark ? 0.34 : 0.24);
    painter.drawPixmap((size.width() - blurred.width()) / 2,
                       (size.height() - blurred.height()) / 2, blurred);
    painter.setOpacity(1.0);

    const QColor veil_rgb = dark ? DARK_BASE : LIGHT_BASE;
    auto veil_at = [&veil_rgb](int alpha) {
        QColor c = veil_rgb;
        c.setAlpha(alpha);
        return c;
    };
    QLinearGradient veil(0, 0, 0, size.height());
    veil.setColorAt(0.0, veil_at(dark ? 135 : 165));
    veil.setColorAt(0.42, veil_at(dark ? 115 : 145));
    veil.setColorAt(1.0, veil_at(dark ? 232 : 242));
    painter.fillRect(out.rect(), veil);
    return out;
}

bool AmbientBackdrop::eventFilter(QObject* obj, QEvent* event) {
    if (event->type() == QEvent::PaletteChange ||
        event->type() == QEvent::ApplicationPaletteChange) {
        cached = {};
        previous_cached = {};
        return QObject::eventFilter(obj, event);
    }

    auto* surface = qobject_cast<QWidget*>(obj);
    if (!surface || event->type() != QEvent::Paint || !enabled || !target || !target->isVisible()) {
        return QObject::eventFilter(obj, event);
    }

    const QSize size = target->size();
    if (size.isEmpty()) {
        return false;
    }
    const bool dark = EffectiveDark();
    if (cached.size() != size || cached_dark != dark) {
        cached = Render(cover, size, dark);
        cached_dark = dark;
    }
    if (fade.state() == QAbstractAnimation::Running &&
        (previous_cached.size() != size || previous_cached_dark != dark)) {
        previous_cached = Render(previous_cover, size, dark);
        previous_cached_dark = dark;
    }

    QPainter painter(surface);
    if (surface != target) {
        painter.translate(-surface->mapTo(target, QPoint(0, 0)));
    }
    if (fade.state() == QAbstractAnimation::Running && !previous_cached.isNull()) {
        painter.drawPixmap(0, 0, previous_cached);
        painter.setOpacity(fade.currentValue().toReal());
    }
    if (!cached.isNull()) {
        painter.drawPixmap(0, 0, cached);
    }
    return false;
}

} // namespace RenHDrumQt
