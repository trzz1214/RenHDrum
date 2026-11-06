// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <utility>
#include <vector>

#include <QAbstractButton>
#include <QFont>
#include <QIcon>
#include <QString>
#include <QWidget>

class QGraphicsOpacityEffect;
class QLabel;
class QVBoxLayout;

namespace Brontide {

// The HD Rumble Studio wears the colors of the HD Rumble pill's ring: thunder gold running into
// electric cyan. Amber stays for the logo's low band.
inline constexpr QRgb STUDIO_CYAN = 0xff4fd8ff;
inline constexpr QRgb STUDIO_GOLD = 0xffffcf3f;
inline constexpr QRgb STUDIO_AMBER = 0xffff7a3d;

enum class Glyph { Bluetooth, Usb, Sliders, Check, ProController, DualSense };

/// Line icons on a 24 x 24 grid, the same shapes Brontide-App uses.
QIcon GlyphIcon(Glyph glyph, int size, const QColor& color);

/// iOS-style switch (51 x 31), painted because QSS cannot draw one.
class ToggleSwitch : public QAbstractButton {
    Q_OBJECT

public:
    explicit ToggleSwitch(QWidget* parent = nullptr);
    QSize sizeHint() const override;

protected:
    void paintEvent(QPaintEvent* event) override;
};

/// Segmented control: one cell per option, the chosen one filled with the accent.
class Segmented : public QWidget {
    Q_OBJECT

public:
    Segmented(const std::vector<std::pair<int, QString>>& options, QWidget* parent = nullptr);
    int Value() const {
        return value;
    }
    void SetValue(int new_value);
    QSize sizeHint() const override;

signals:
    void ValueChanged(int value);

protected:
    void paintEvent(QPaintEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;

private:
    std::vector<std::pair<int, QString>> options;
    int cell_width = 56;
    int value = 0;
};

/// Text filled with the logo gradient (amber to cyan).
class GradientText : public QWidget {
    Q_OBJECT

public:
    GradientText(const QString& text, QWidget* parent = nullptr);
    QSize sizeHint() const override;

protected:
    void paintEvent(QPaintEvent* event) override;

private:
    QString text;
    QFont text_font;
};

/// One 44 px row of a card: label on the left, any widget on the right. Sub-options are indented
/// and joined to their parent by a tree line, like Brontide-App.
class StudioRow : public QWidget {
    Q_OBJECT

public:
    StudioRow(const QString& label, const QString& tip, int indent, QWidget* right,
              const QIcon& icon = {}, QWidget* parent = nullptr);

    void SetLabel(const QString& text);
    void SetIcon(const QIcon& icon);
    void SetClickable(bool clickable);

signals:
    void Clicked();

protected:
    void paintEvent(QPaintEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void enterEvent(QEnterEvent* event) override;
    void leaveEvent(QEvent* event) override;

private:
    bool IsFirstVisible() const;

    QLabel* label = nullptr;
    QLabel* icon_label = nullptr;
    int indent = 0;
    bool clickable = false;
    bool hovered = false;
};

/// One connected controller on the Overview page, a card of its own. The one a player uses wears
/// the running ring of the HD Rumble pill, so it is clear at a glance which controller is playing.
class PadCard : public QWidget {
    Q_OBJECT

public:
    PadCard(const QString& name, const QString& connection, const QString& mode, const QString& in_use,
            QWidget* parent = nullptr);

signals:
    void Clicked();

protected:
    void paintEvent(QPaintEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void enterEvent(QEnterEvent* event) override;
    void leaveEvent(QEvent* event) override;

private:
    bool active;
    bool hovered = false;
    qreal angle = 0.0;
    qint64 last_ms = 0;
};

/// A titled group of rows (optional icon and badge above, small note below).
class StudioSection : public QWidget {
    Q_OBJECT

public:
    explicit StudioSection(const QString& title = {}, const QIcon& icon = {}, QWidget* parent = nullptr);

    void AddRow(QWidget* row);
    /// Rows become separate cards (PadCard) instead of one framed list.
    void SetCardList();
    void Clear();
    void SetBadge(const QString& text);
    void SetFootnote(const QString& text);
    void SetDimmed(bool dimmed);

private:
    QVBoxLayout* rows = nullptr;
    QWidget* group = nullptr;
    QLabel* badge = nullptr;
    QLabel* footnote = nullptr;
    QGraphicsOpacityEffect* effect = nullptr;
};

} // namespace Brontide
