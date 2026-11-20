// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later
// Port from Brontide (src/yuzu/brontide/ambient_backdrop.h).

#pragma once

#include <optional>

#include <QObject>
#include <QPixmap>
#include <QPointer>
#include <QVariantAnimation>
#include <QVector>

class QAbstractItemView;
class QWidget;

namespace RenHDrumQt {

/// Ambient backdrop behind a widget or item view list: the selected game's cover scaled down to
/// 24 px and blown up again (cheap blur), tinted with its average color glow and a dark or light
/// veil matching the active theme. Cross-fades in 300 ms on cover changes and only draws when the
/// target paints.
class AmbientBackdrop : public QObject {
    Q_OBJECT

public:
    explicit AmbientBackdrop(QWidget* target);

    /// Watches an item view's viewport so scrolling and selection paint over the backdrop.
    void AttachItemView(QAbstractItemView* view);

    /// Sets the cover pixmap to derive the ambient backdrop from (cross-fades in 300 ms).
    void SetCover(const QPixmap& cover);

    void SetEnabled(bool enabled);
    bool IsEnabled() const {
        return enabled;
    }

    /// Forces dark or light veil; std::nullopt follows `target->palette()`.
    void SetDarkMode(std::optional<bool> dark);

protected:
    bool eventFilter(QObject* obj, QEvent* event) override;

private:
    bool EffectiveDark() const;
    void RepaintAll();
    QPixmap Render(const QPixmap& source, const QSize& size, bool dark) const;

    QPointer<QWidget> target;
    QVector<QPointer<QAbstractItemView>> views;
    QPixmap cover;
    QPixmap previous_cover;
    QPixmap cached;
    QPixmap previous_cached;
    bool cached_dark{};
    bool previous_cached_dark{};
    QVariantAnimation fade;
    bool enabled{true};
    std::optional<bool> forced_dark;
};

} // namespace RenHDrumQt
