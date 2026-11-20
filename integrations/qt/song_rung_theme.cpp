// SPDX-License-Identifier: GPL-3.0-or-later

#include "song_rung_theme.h"

#include <utility>

#include <QApplication>
#include <QFile>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QPointer>
#include <QProxyStyle>
#include <QStyle>
#include <QStyleFactory>
#include <QStyleHints>

static void InitSongRungBrandQrc() {
#ifdef RENHDRUM_HAS_BRAND_QRC
    Q_INIT_RESOURCE(song_rung_brand);
#endif
}

namespace RenHDrumQt {
namespace {

class SongRungProxyStyle final : public QProxyStyle {
public:
    SongRungProxyStyle() : QProxyStyle(QStyleFactory::create(QStringLiteral("Fusion"))) {}

    int styleHint(StyleHint hint, const QStyleOption* option, const QWidget* widget,
                  QStyleHintReturn* returnData) const override {
        if (hint == SH_UnderlineShortcut) {
            return 0;
        }
        return QProxyStyle::styleHint(hint, option, widget, returnData);
    }
};

struct ThemeState {
    bool initialized{};
    bool active{};
    QString saved_style_name;
    QFont saved_font;
    QPointer<SongRungProxyStyle> proxy_style;
    bool fonts_loaded{};
    QString sans_family;
    QString mono_family;
};

ThemeState& State() {
    static ThemeState state;
    return state;
}

} // namespace

void LoadBrandResources() {
    static bool loaded = false;
    if (!loaded) {
        loaded = true;
        InitSongRungBrandQrc();
    }
}

PaletteTokens Tokens(bool dark) {
    if (dark) {
        return {
            .window = QColor(0x15, 0x14, 0x13),
            .surface = QColor(0x20, 0x1E, 0x1C),
            .surface_raised = QColor(0x28, 0x25, 0x22),
            .text = QColor(0xF3, 0xEF, 0xEA),
            .muted = QColor(0xA3, 0x9B, 0x93),
            .disabled = QColor(0x70, 0x69, 0x62),
            .accent = QColor(0xE6, 0x8A, 0x2E),
            .accent_hover = QColor(0xF0, 0x9B, 0x45),
            .accent_soft = QColor(0x42, 0x30, 0x1E),
            .on_accent = QColor(0x15, 0x14, 0x13),
            .danger = QColor(0xE0, 0x52, 0x52),
            .line = QColor(0x2E, 0x2B, 0x28),
            .ok = QColor(0x65, 0xA3, 0x0D),
        };
    }
    return {
        .window = QColor(0xF7, 0xF5, 0xF2),
        .surface = QColor(0xFF, 0xFF, 0xFF),
        .surface_raised = QColor(0xF0, 0xEC, 0xE6),
        .text = QColor(0x1E, 0x1B, 0x18),
        .muted = QColor(0x66, 0x5E, 0x57),
        .disabled = QColor(0x9B, 0x93, 0x8B),
        .accent = QColor(0xC4, 0x68, 0x12),
        .accent_hover = QColor(0xD6, 0x76, 0x19),
        .accent_soft = QColor(0xFD, 0xEB, 0xD6),
        .on_accent = QColor(0xFF, 0xFF, 0xFF),
        .danger = QColor(0xC9, 0x3B, 0x3B),
        .line = QColor(0xE6, 0xE1, 0xDB),
        .ok = QColor(0x4D, 0x7C, 0x0F),
    };
}

QColor BrandAccentNs2() {
    return QColor(0x38, 0xBD, 0xF8);
}

QColor BrandAccentNs2Muted() {
    return QColor(0x1E, 0x4E, 0x68);
}

QColor BrandAccentOot() {
    return QColor(0xE6, 0x8A, 0x2E);
}

QColor BrandAccentOotMoss() {
    return QColor(0x65, 0xA3, 0x0D);
}

bool IsSystemDark() {
    const QString forced = qEnvironmentVariable("BRONTIDE_COLOR_SCHEME").trimmed().toLower();
    if (forced == QStringLiteral("dark")) {
        return true;
    }
    if (forced == QStringLiteral("light")) {
        return false;
    }
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    if (QGuiApplication::instance()) {
        const auto scheme = QGuiApplication::styleHints()->colorScheme();
        if (scheme == Qt::ColorScheme::Dark) {
            return true;
        }
        if (scheme == Qt::ColorScheme::Light) {
            return false;
        }
    }
#endif
    const QPalette pal;
    return pal.color(QPalette::Window).lightness() < 128;
}

void LoadFonts() {
    LoadBrandResources();
    auto& state = State();
    if (state.fonts_loaded) {
        return;
    }
    state.fonts_loaded = true;

    const int sans_id =
        QFontDatabase::addApplicationFont(QStringLiteral(":/brand/fonts/IBMPlexSans-Variable.ttf"));
    if (sans_id != -1) {
        const QStringList families = QFontDatabase::applicationFontFamilies(sans_id);
        if (!families.isEmpty()) {
            state.sans_family = families.first();
        }
    }

    for (const QString& path : {QStringLiteral(":/brand/fonts/IBMPlexMono-Regular.ttf"),
                                QStringLiteral(":/brand/fonts/IBMPlexMono-Medium.ttf"),
                                QStringLiteral(":/brand/fonts/IBMPlexMono-SemiBold.ttf")}) {
        const int id = QFontDatabase::addApplicationFont(path);
        if (id != -1 && state.mono_family.isEmpty()) {
            const QStringList families = QFontDatabase::applicationFontFamilies(id);
            if (!families.isEmpty()) {
                state.mono_family = families.first();
            }
        }
    }
}

QFont UiFont(qreal point_size) {
    LoadFonts();
    const auto& state = State();
    QFont font = state.sans_family.isEmpty() ? QFont(QStringLiteral("IBM Plex Sans"))
                                             : QFont(state.sans_family);
    font.setPointSizeF(point_size);
    return font;
}

QFont MonoFont(qreal point_size) {
    LoadFonts();
    const auto& state = State();
    QFont font = state.mono_family.isEmpty()
                     ? QFontDatabase::systemFont(QFontDatabase::FixedFont)
                     : QFont(state.mono_family);
    font.setFixedPitch(true);
    font.setPointSizeF(point_size);
    return font;
}

QPalette BuildPalette(bool dark) {
    const PaletteTokens t = Tokens(dark);
    QPalette p;
    p.setColor(QPalette::Window, t.window);
    p.setColor(QPalette::WindowText, t.text);
    p.setColor(QPalette::Base, t.surface);
    p.setColor(QPalette::AlternateBase, t.surface_raised);
    p.setColor(QPalette::Text, t.text);
    p.setColor(QPalette::Button, t.surface);
    p.setColor(QPalette::ButtonText, t.text);
    p.setColor(QPalette::BrightText, dark ? QColor(Qt::white) : QColor(Qt::black));
    p.setColor(QPalette::ToolTipBase, t.surface_raised);
    p.setColor(QPalette::ToolTipText, t.text);
    p.setColor(QPalette::PlaceholderText, t.muted);
    p.setColor(QPalette::Highlight, t.accent);
    p.setColor(QPalette::HighlightedText, t.on_accent);
    p.setColor(QPalette::Link, t.accent);
    p.setColor(QPalette::LinkVisited, t.accent_hover);
    p.setColor(QPalette::Light, dark ? QColor(0x3C, 0x38, 0x34) : QColor(0xFF, 0xFF, 0xFF));
    p.setColor(QPalette::Midlight, t.surface_raised);
    p.setColor(QPalette::Mid, t.line);
    p.setColor(QPalette::Dark, dark ? QColor(0x0E, 0x0D, 0x0C) : QColor(0xCE, 0xC8, 0xC0));
    p.setColor(QPalette::Shadow, dark ? QColor(0x07, 0x06, 0x06) : QColor(0x8E, 0x87, 0x7E));

    for (const auto role : {QPalette::WindowText, QPalette::Text, QPalette::ButtonText}) {
        p.setColor(QPalette::Disabled, role, t.disabled);
    }
    p.setColor(QPalette::Disabled, QPalette::Highlight, t.line);
    p.setColor(QPalette::Disabled, QPalette::HighlightedText, t.disabled);
    return p;
}

QString LoadStyleSheet(bool dark) {
    LoadBrandResources();
    QFile file(QStringLiteral(":/brand/theme/song-rung.qss"));
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return {};
    }
    const PaletteTokens t = Tokens(dark);
    QString qss = QString::fromUtf8(file.readAll());
    qss.replace(QStringLiteral("@WINDOW@"), t.window.name());
    qss.replace(QStringLiteral("@SURFACE@"), t.surface.name());
    qss.replace(QStringLiteral("@TEXT@"), t.text.name());
    qss.replace(QStringLiteral("@MUTED@"), t.muted.name());
    qss.replace(QStringLiteral("@DISABLED@"), t.disabled.name());
    qss.replace(QStringLiteral("@ACCENT_HOVER@"), t.accent_hover.name());
    qss.replace(QStringLiteral("@ACCENT_SOFT@"), t.accent_soft.name());
    qss.replace(QStringLiteral("@ACCENT@"), t.accent.name());
    qss.replace(QStringLiteral("@ON_ACCENT@"), t.on_accent.name());
    qss.replace(QStringLiteral("@DANGER@"), t.danger.name());
    qss.replace(QStringLiteral("@LINE@"), t.line.name());
    qss.replace(QStringLiteral("@OK@"), t.ok.name());
    qss.replace(QStringLiteral("@HOVER_ROW@"),
                dark ? QStringLiteral("rgba(66, 48, 30, 125)")
                     : QStringLiteral("rgba(253, 235, 214, 145)"));
    qss.replace(QStringLiteral("@SEL_ROW@"),
                dark ? QStringLiteral("rgba(66, 48, 30, 215)")
                     : QStringLiteral("rgba(253, 235, 214, 220)"));
    qss.replace(QStringLiteral("@ROW_LINE@"),
                dark ? QStringLiteral("rgba(46, 43, 40, 150)")
                     : QStringLiteral("rgba(230, 225, 219, 175)"));
    return qss;
}

void ApplySongRungTheme(QApplication& app, bool dark) {
    auto& state = State();
    if (!state.active) {
        if (!state.initialized) {
            state.initialized = true;
            state.saved_style_name = app.style() ? app.style()->objectName() : QString{};
            state.saved_font = app.font();
        }
        state.active = true;
    }

    LoadFonts();
    if (!state.proxy_style || app.style() != state.proxy_style) {
        state.proxy_style = new SongRungProxyStyle;
        app.setStyle(state.proxy_style);
    }
    app.setFont(UiFont(9.5));
    app.setPalette(BuildPalette(dark));
    app.setStyleSheet(LoadStyleSheet(dark));
}

void RestoreSystemTheme(QApplication& app) {
    auto& state = State();
    if (!state.active) {
        return;
    }
    state.active = false;
    state.proxy_style = nullptr;
    if (!state.saved_style_name.isEmpty() &&
        QStyleFactory::keys().contains(state.saved_style_name, Qt::CaseInsensitive)) {
        app.setStyle(state.saved_style_name);
    }
    app.setFont(state.saved_font);
    app.setPalette(QPalette());
    app.setStyleSheet({});
}

QMetaObject::Connection WatchSystemColorScheme(QObject* context,
                                               std::function<void(bool dark)> callback) {
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    if (!QGuiApplication::instance() || !context || !callback) {
        return {};
    }
    return QObject::connect(QGuiApplication::styleHints(), &QStyleHints::colorSchemeChanged,
                            context, [cb = std::move(callback)](Qt::ColorScheme) {
                                cb(IsSystemDark());
                            });
#else
    Q_UNUSED(context);
    Q_UNUSED(callback);
    return {};
#endif
}

} // namespace RenHDrumQt
