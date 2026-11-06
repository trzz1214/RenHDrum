// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#include <algorithm>
#include <cmath>

#include <QEnterEvent>
#include <QFontMetrics>
#include <QFrame>
#include <QGraphicsOpacityEffect>
#include <QHBoxLayout>
#include <QLabel>
#include <QLinearGradient>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPixmap>
#include <QVBoxLayout>

#include "yuzu/brontide/effects_gate.h"
#include "yuzu/brontide/hd_rumble_widgets.h"
#include "yuzu/brontide/rumble_pill.h"

namespace Brontide {

namespace {

// Measures from Brontide-App's theme.rs
constexpr int ROW_HEIGHT = 44;
constexpr int PAD = 14;
constexpr int INDENT_WIDTH = 24;
constexpr int ICON_SLOT = 28;

const QColor SEPARATOR{255, 255, 255, 16};
const QColor TRACK{255, 255, 255, 40};
const QColor SEGMENT_BACKGROUND{0, 0, 0, 80};
const QColor MUTED{0xb9, 0xab, 0x84};
const QColor ON_ACCENT{0x10, 0x13, 0x1a};

/// Gold into cyan across `rect`, the fill of everything switched on.
QLinearGradient AccentGradient(const QRectF& rect) {
    QLinearGradient g(rect.topLeft(), rect.topRight());
    g.setColorAt(0, QColor::fromRgba(STUDIO_GOLD));
    g.setColorAt(1, QColor::fromRgba(STUDIO_CYAN));
    return g;
}

void Line(QPainterPath& path, std::initializer_list<QPointF> points) {
    auto it = points.begin();
    path.moveTo(*it);
    for (++it; it != points.end(); ++it) {
        path.lineTo(*it);
    }
}

void Circle(QPainterPath& path, qreal x, qreal y, qreal r) {
    path.addEllipse(QPointF(x, y), r, r);
}

QPainterPath ProControllerPath() {
    QPainterPath p;
    p.moveTo(7, 5.5);
    p.cubicTo(9, 4.8, 15, 4.8, 17, 5.5);
    p.cubicTo(19.5, 5.5, 21.5, 7.5, 21.5, 11);
    p.cubicTo(21.5, 14.5, 19.8, 18.2, 18.5, 19.8);
    p.cubicTo(17.5, 20.8, 16, 20.2, 15, 18.2);
    p.cubicTo(14.2, 16.5, 13.5, 15.8, 12, 15.8);
    p.cubicTo(10.5, 15.8, 9.8, 16.5, 9, 18.2);
    p.cubicTo(8, 20.2, 6.5, 20.8, 5.5, 19.8);
    p.cubicTo(4.2, 18.2, 2.5, 14.5, 2.5, 11);
    p.cubicTo(2.5, 7.5, 4.5, 5.5, 7, 5.5);
    p.closeSubpath();
    Circle(p, 7, 9, 1.7);
    Line(p, {{9.5, 11.6}, {9.5, 14.4}});
    Line(p, {{8.1, 13}, {10.9, 13}});
    Circle(p, 14.5, 13, 1.7);
    Circle(p, 17, 7.6, 0.4);
    Circle(p, 18.4, 9, 0.4);
    Circle(p, 17, 10.4, 0.4);
    Circle(p, 15.6, 9, 0.4);
    return p;
}

QPainterPath DualSensePath() {
    QPainterPath p;
    p.moveTo(7, 4.5);
    p.lineTo(17, 4.5);
    p.cubicTo(19, 4.5, 21.5, 6.5, 21.5, 10.5);
    p.cubicTo(21.5, 14.5, 20.2, 18, 18.8, 20);
    p.cubicTo(17.8, 20.8, 16.5, 20.2, 15.6, 18.5);
    p.cubicTo(14.8, 17, 13.8, 16.8, 12, 16.8);
    p.cubicTo(10.2, 16.8, 9.2, 17, 8.4, 18.5);
    p.cubicTo(7.5, 20.2, 6.2, 20.8, 5.2, 20);
    p.cubicTo(3.8, 18, 2.5, 14.5, 2.5, 10.5);
    p.cubicTo(2.5, 6.5, 5, 4.5, 7, 4.5);
    p.closeSubpath();
    p.addRoundedRect(QRectF(8.9, 6.8, 6.2, 3.8), 1, 1); // touchpad
    Line(p, {{5.8, 7.8}, {5.8, 10.6}});
    Line(p, {{4.4, 9.2}, {7.2, 9.2}});
    Circle(p, 18.2, 7.8, 0.4);
    Circle(p, 19.6, 9.2, 0.4);
    Circle(p, 18.2, 10.6, 0.4);
    Circle(p, 16.8, 9.2, 0.4);
    Circle(p, 8.8, 14.2, 1.7);
    Circle(p, 15.2, 14.2, 1.7);
    return p;
}

QPainterPath GlyphPath(Glyph glyph) {
    QPainterPath p;
    switch (glyph) {
    case Glyph::ProController:
        return ProControllerPath();
    case Glyph::DualSense:
        return DualSensePath();
    case Glyph::Bluetooth:
        Line(p, {{6.5, 7.5}, {17.5, 16.5}, {12, 21}, {12, 3}, {17.5, 7.5}, {6.5, 16.5}});
        break;
    case Glyph::Usb:
        Line(p, {{12, 3}, {12, 18.5}});
        Line(p, {{9.5, 5.5}, {12, 3}, {14.5, 5.5}});
        Line(p, {{12, 14}, {7, 10.5}, {7, 8.5}});
        Line(p, {{12, 11.5}, {17, 8}, {17, 6.5}});
        Circle(p, 12, 20, 1.6);
        Circle(p, 7, 7.5, 1.1);
        p.addRect(QRectF(15.8, 4.2, 2.4, 2.4));
        break;
    case Glyph::Check:
        Line(p, {{4.5, 12.5}, {9.5, 17.5}, {19.5, 6.5}});
        break;
    case Glyph::Sliders:
        Line(p, {{4, 7}, {20, 7}});
        Line(p, {{4, 12}, {20, 12}});
        Line(p, {{4, 17}, {20, 17}});
        Circle(p, 9, 7, 2);
        Circle(p, 15, 12, 2);
        Circle(p, 8, 17, 2);
        break;
    }
    return p;
}

} // namespace

QIcon GlyphIcon(Glyph glyph, int size, const QColor& color) {
    QPixmap pixmap(size * 2, size * 2);
    pixmap.fill(Qt::transparent);
    pixmap.setDevicePixelRatio(2.0);
    QPainter painter(&pixmap);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.scale(size / 24.0, size / 24.0);
    painter.setPen(QPen(color, 1.6, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    painter.setBrush(Qt::NoBrush);
    painter.drawPath(GlyphPath(glyph));
    return QIcon(pixmap);
}

// ---------------------------------------------------------------- ToggleSwitch

ToggleSwitch::ToggleSwitch(QWidget* parent) : QAbstractButton(parent) {
    setObjectName(QStringLiteral("StudioPlain")); // the theme paints plain widgets opaque
    setCheckable(true);
    setCursor(Qt::PointingHandCursor);
    setFocusPolicy(Qt::TabFocus);
    setFixedSize(sizeHint());
}

QSize ToggleSwitch::sizeHint() const {
    return {51, 31};
}

void ToggleSwitch::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setOpacity(isEnabled() ? 1.0 : 0.4);
    painter.setPen(Qt::NoPen);
    if (isChecked()) {
        painter.setBrush(AccentGradient(rect()));
    } else {
        painter.setBrush(TRACK);
    }
    painter.drawRoundedRect(QRectF(0, 0, width(), height()), height() / 2.0, height() / 2.0);
    const qreal knob = height() - 4.0;
    const qreal x = isChecked() ? width() - knob - 2.0 : 2.0;
    painter.setBrush(Qt::white);
    painter.drawEllipse(QRectF(x, 2.0, knob, knob));
}

// ---------------------------------------------------------------- Segmented

Segmented::Segmented(const std::vector<std::pair<int, QString>>& options_, QWidget* parent)
    : QWidget(parent), options{options_} {
    setObjectName(QStringLiteral("StudioPlain"));
    setCursor(Qt::PointingHandCursor);
    const QFontMetrics metrics(font());
    for (const auto& option : options) {
        cell_width = std::max(cell_width, metrics.horizontalAdvance(option.second) + 24);
    }
    setFixedSize(sizeHint());
}

void Segmented::SetValue(int new_value) {
    value = new_value;
    update();
}

QSize Segmented::sizeHint() const {
    return {cell_width * static_cast<int>(options.size()) + 4, 30};
}

void Segmented::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setOpacity(isEnabled() ? 1.0 : 0.4);
    painter.setPen(Qt::NoPen);
    painter.setBrush(SEGMENT_BACKGROUND);
    painter.drawRoundedRect(rect(), 8, 8);
    for (int i = 0; i < static_cast<int>(options.size()); ++i) {
        const QRect cell(2 + i * cell_width, 2, cell_width, height() - 4);
        const bool selected = options[i].first == value;
        if (selected) {
            painter.setBrush(AccentGradient(cell));
            painter.drawRoundedRect(cell, 6, 6);
        }
        painter.setPen(selected ? ON_ACCENT : MUTED);
        painter.drawText(cell, Qt::AlignCenter, options[i].second);
        painter.setPen(Qt::NoPen);
    }
}

void Segmented::mouseReleaseEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton || options.empty()) {
        return;
    }
    const int index = std::clamp((event->position().toPoint().x() - 2) / cell_width, 0,
                                 static_cast<int>(options.size()) - 1);
    if (options[index].first != value) {
        value = options[index].first;
        update();
        emit ValueChanged(value);
    }
}

// ---------------------------------------------------------------- GradientText

GradientText::GradientText(const QString& text_, QWidget* parent) : QWidget(parent), text{text_} {
    setObjectName(QStringLiteral("StudioPlain"));
    text_font = font();
    text_font.setBold(true);
    text_font.setPixelSize(11);
    setFixedSize(sizeHint());
}

QSize GradientText::sizeHint() const {
    const QFontMetrics metrics(text_font);
    return {metrics.horizontalAdvance(text) + 4, metrics.height()};
}

void GradientText::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    QLinearGradient gradient(0, 0, width(), 0);
    gradient.setColorAt(0, QColor::fromRgba(STUDIO_AMBER));
    gradient.setColorAt(1, QColor::fromRgba(STUDIO_CYAN));
    painter.setFont(text_font);
    painter.setPen(QPen(QBrush(gradient), 1));
    painter.drawText(rect(), Qt::AlignLeft | Qt::AlignVCenter, text);
}

// ---------------------------------------------------------------- StudioRow

StudioRow::StudioRow(const QString& label_text, const QString& tip, int indent_, QWidget* right,
                     const QIcon& icon, QWidget* parent)
    : QWidget(parent), indent{indent_} {
    setObjectName(QStringLiteral("StudioRow"));
    setMinimumHeight(ROW_HEIGHT);
    setToolTip(tip);
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(PAD + indent * INDENT_WIDTH, 6, PAD, 6);
    layout->setSpacing(10);
    if (!icon.isNull()) {
        icon_label = new QLabel(this);
        icon_label->setPixmap(icon.pixmap(20, 20));
        icon_label->setFixedWidth(ICON_SLOT - 10);
        layout->addWidget(icon_label);
    }
    label = new QLabel(label_text, this);
    label->setWordWrap(true);
    layout->addWidget(label, 1);
    if (right) {
        layout->addWidget(right, 0, Qt::AlignVCenter);
    }
}

void StudioRow::SetLabel(const QString& text) {
    label->setText(text);
}

void StudioRow::SetIcon(const QIcon& icon) {
    if (icon_label) {
        icon_label->setPixmap(icon.pixmap(20, 20));
    }
}

void StudioRow::SetClickable(bool value) {
    clickable = value;
    setCursor(value ? Qt::PointingHandCursor : Qt::ArrowCursor);
}

bool StudioRow::IsFirstVisible() const {
    for (const auto* row : parentWidget()->findChildren<StudioRow*>(QString(), Qt::FindDirectChildrenOnly)) {
        if (!row->isHidden()) {
            return row == this;
        }
    }
    return true;
}

void StudioRow::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    if (hovered && clickable) {
        // Inset and rounded, so it never pokes out of the card's rounded corners
        painter.setRenderHint(QPainter::Antialiasing);
        painter.setPen(Qt::NoPen);
        painter.setBrush(QColor(255, 255, 255, 14));
        painter.drawRoundedRect(QRectF(rect()).adjusted(3, 3, -3, -3), 9, 9);
        painter.setRenderHint(QPainter::Antialiasing, false);
    }
    painter.setPen(SEPARATOR);
    if (!IsFirstVisible()) {
        painter.drawLine(PAD + (indent > 0 ? indent * INDENT_WIDTH : 0), 0, width(), 0);
    }
    for (int level = 1; level <= indent; ++level) {
        const int x = PAD + (level - 1) * INDENT_WIDTH + 10;
        if (level < indent) {
            painter.drawLine(x, 0, x, height());
        } else {
            painter.drawLine(x, 0, x, height() / 2);
            painter.drawLine(x, height() / 2, x + 10, height() / 2);
        }
    }
}

void StudioRow::mouseReleaseEvent(QMouseEvent* event) {
    if (clickable && event->button() == Qt::LeftButton && rect().contains(event->position().toPoint())) {
        emit Clicked();
    }
}

void StudioRow::enterEvent(QEnterEvent*) {
    hovered = true;
    update();
}

void StudioRow::leaveEvent(QEvent*) {
    hovered = false;
    update();
}

// ---------------------------------------------------------------- PadCard

PadCard::PadCard(const QString& name, const QString& connection, const QString& mode,
                 const QString& in_use, QWidget* parent)
    : QWidget(parent), active{!in_use.isEmpty()} {
    setObjectName(QStringLiteral("StudioPadCard"));
    setMinimumHeight(56);
    setCursor(Qt::PointingHandCursor);
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(18, 8, PAD + 2, 8);
    layout->setSpacing(8);
    auto* names = new QVBoxLayout;
    names->setSpacing(1);
    auto* title = new QLabel(name, this);
    title->setObjectName(QStringLiteral("StudioPadName"));
    names->addWidget(title);
    if (active) {
        auto* use = new QLabel(in_use, this);
        use->setObjectName(QStringLiteral("StudioPadInUse"));
        names->addWidget(use);
    }
    layout->addLayout(names, 1);
    auto* chip = new QLabel(connection, this);
    chip->setObjectName(QStringLiteral("StudioChip"));
    layout->addWidget(chip, 0, Qt::AlignVCenter);
    auto* mode_label = new QLabel(mode, this);
    mode_label->setObjectName(QStringLiteral("StudioMode"));
    layout->addWidget(mode_label, 0, Qt::AlignVCenter);

    if (active) {
        auto& gate = EffectsGate::Instance();
        last_ms = gate.Elapsed();
        connect(&gate, &EffectsGate::Tick, this, [this] {
            const qint64 now = EffectsGate::Instance().Elapsed();
            const qreal dt = static_cast<qreal>(now - last_ms);
            last_ms = now;
            if (isVisible()) {
                // Clockwise, one turn in 4 s: a card is longer than the pill
                angle = std::fmod(angle - dt * 360.0 / 4000.0 + 360.0, 360.0);
                update();
            }
        });
        connect(&gate, &EffectsGate::ActiveChanged, this, [this] { update(); });
    }
}

void PadCard::paintEvent(QPaintEvent*) {
    constexpr qreal RADIUS = 12.0;
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    const QRectF body = QRectF(rect()).adjusted(1.5, 1.5, -1.5, -1.5);
    p.setPen(QPen(QColor(255, 255, 255, 18), 1.0));
    p.setBrush(QColor(255, 255, 255, hovered ? 16 : (active ? 10 : 6)));
    p.drawRoundedRect(body, RADIUS, RADIUS);
    if (!active) {
        return;
    }
    if (EffectsGate::Instance().IsActive()) {
        PaintGlowRing(p, body, RADIUS, angle, hovered ? 110 : 70);
    } else {
        // Effects stopped (a game runs): a still ring keeps the mark
        p.setBrush(Qt::NoBrush);
        p.setPen(QPen(QBrush(AccentGradient(body)), 1.5));
        p.drawRoundedRect(body, RADIUS, RADIUS);
    }
}

void PadCard::mouseReleaseEvent(QMouseEvent* event) {
    if (event->button() == Qt::LeftButton && rect().contains(event->position().toPoint())) {
        emit Clicked();
    }
}

void PadCard::enterEvent(QEnterEvent*) {
    hovered = true;
    update();
}

void PadCard::leaveEvent(QEvent*) {
    hovered = false;
    update();
}

// ---------------------------------------------------------------- StudioSection

StudioSection::StudioSection(const QString& title, const QIcon& icon, QWidget* parent)
    : QWidget(parent) {
    setObjectName(QStringLiteral("StudioSection"));
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 18, 0, 0);
    layout->setSpacing(6);
    if (!title.isEmpty()) {
        auto* header = new QHBoxLayout;
        header->setContentsMargins(PAD, 0, 0, 0);
        header->setSpacing(8);
        if (!icon.isNull()) {
            auto* icon_label = new QLabel(this);
            icon_label->setPixmap(icon.pixmap(18, 18));
            header->addWidget(icon_label);
        }
        auto* title_label = new QLabel(title, this);
        title_label->setObjectName(QStringLiteral("StudioSectionTitle"));
        title_label->setWordWrap(true);
        header->addWidget(title_label);
        badge = new QLabel(this);
        badge->setObjectName(QStringLiteral("StudioBadge"));
        badge->hide();
        header->addWidget(badge);
        header->addStretch(1);
        layout->addLayout(header);
    }
    group = new QFrame(this);
    group->setObjectName(QStringLiteral("StudioGroup"));
    rows = new QVBoxLayout(group);
    rows->setContentsMargins(0, 0, 0, 0);
    rows->setSpacing(0);
    layout->addWidget(group);
    footnote = new QLabel(this);
    footnote->setObjectName(QStringLiteral("StudioFootnote"));
    footnote->setWordWrap(true);
    footnote->setContentsMargins(PAD, 0, PAD, 0);
    footnote->hide();
    layout->addWidget(footnote);
}

void StudioSection::AddRow(QWidget* row) {
    rows->addWidget(row);
}

void StudioSection::SetCardList() {
    group->setObjectName(QStringLiteral("StudioCardList"));
    rows->setSpacing(8);
}

void StudioSection::Clear() {
    while (QLayoutItem* item = rows->takeAt(0)) {
        // A row can be the sender of the click that clears its own list
        item->widget()->hide();
        item->widget()->deleteLater();
        delete item;
    }
}

void StudioSection::SetBadge(const QString& text) {
    if (badge) {
        badge->setText(text);
        badge->setVisible(!text.isEmpty());
    }
}

void StudioSection::SetFootnote(const QString& text) {
    footnote->setText(text);
    footnote->setVisible(!text.isEmpty());
}

void StudioSection::SetDimmed(bool dimmed) {
    if (dimmed == (effect != nullptr)) {
        return;
    }
    if (dimmed) {
        effect = new QGraphicsOpacityEffect(this);
        effect->setOpacity(0.45);
        setGraphicsEffect(effect);
    } else {
        setGraphicsEffect(nullptr); // deletes the effect
        effect = nullptr;
    }
}

} // namespace Brontide
