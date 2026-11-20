// SPDX-License-Identifier: GPL-3.0-or-later
// Song Rung shared theme for Qt Widgets hosts (Brontide-NS2 and Brontide-oot).
// Fusion base style + QPalette (Dark / Light following the system) + thin QSS + IBM Plex fonts.

#pragma once

#include <functional>

#include <QColor>
#include <QFont>
#include <QMetaObject>
#include <QPalette>
#include <QString>

class QApplication;
class QObject;

namespace RenHDrumQt {

struct PaletteTokens {
    QColor window;
    QColor surface;
    QColor surface_raised;
    QColor text;
    QColor muted;
    QColor disabled;
    QColor accent;
    QColor accent_hover;
    QColor accent_soft;
    QColor on_accent;
    QColor danger;
    QColor line;
    QColor ok;
};

/// Explicitly registers the `song_rung_brand` Qt resources (required when compiled into a static lib).
void LoadBrandResources();

PaletteTokens Tokens(bool dark);

QColor BrandAccentNs2();
QColor BrandAccentNs2Muted();
QColor BrandAccentOot();
QColor BrandAccentOotMoss();

/// Follows Qt's system color scheme (or BRONTIDE_COLOR_SCHEME=dark|light when set).
bool IsSystemDark();

/// Loads embedded IBM Plex Sans and IBM Plex Mono fonts into QFontDatabase.
void LoadFonts();

QFont UiFont(qreal point_size = 9.5);
QFont MonoFont(qreal point_size = 9.5);

QPalette BuildPalette(bool dark);
QString LoadStyleSheet(bool dark);

void ApplySongRungTheme(QApplication& app, bool dark);
void RestoreSystemTheme(QApplication& app);

/// Invokes `callback(dark)` when the OS color scheme changes (Qt >= 6.5).
QMetaObject::Connection WatchSystemColorScheme(QObject* context,
                                               std::function<void(bool dark)> callback);

} // namespace RenHDrumQt
