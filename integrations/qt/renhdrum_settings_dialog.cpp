// SPDX-License-Identifier: GPL-3.0-or-later

#include "renhdrum_settings_dialog.h"

#include <algorithm>
#include <utility>

#include <QCheckBox>
#include <QColor>
#include <QComboBox>
#include <QDesktopServices>
#include <QEvent>
#include <QFontDatabase>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QMouseEvent>
#include <QPainter>
#include <QPalette>
#include <QPixmap>
#include <QPushButton>
#include <QSignalBlocker>
#include <QSlider>
#include <QSpinBox>
#include <QTimer>
#include <QToolButton>
#include <QUrl>
#include <QVBoxLayout>

namespace RenHDrumQt {

namespace {

constexpr unsigned DEV_SWITCH = 1;
constexpr unsigned DEV_DUALSENSE = 2;
constexpr unsigned DEV_XBOX = 4;
constexpr unsigned DEV_STEAM = 8;
constexpr unsigned DEV_ALL = 15;
constexpr const char REPO_URL[] = "https://github.com/trzz1214/RenHDrum";

bool IsDarkPalette(const QPalette& pal) {
    // A parent container's QSS (`background: transparent`) sets Window to transparent black
    // (lightness 0), so check WindowText lightness whenever Window is translucent.
    const QColor window = pal.color(QPalette::Window);
    const QColor text = pal.color(QPalette::WindowText);
    if (window.alpha() < 255) {
        return text.lightness() > 128;
    }
    return window.lightness() < text.lightness();
}

QFont MonospaceFont() {
    const QString plex = QStringLiteral("IBM Plex Mono");
    if (QFontDatabase::families().contains(plex)) {
        QFont f(plex);
        f.setFixedPitch(true);
        return f;
    }
    return QFontDatabase::systemFont(QFontDatabase::FixedFont);
}

QString DeviceName(unsigned device) {
    switch (device) {
    case DEV_SWITCH:
        return QObject::tr("Switch Pro Controller / Joy-Con");
    case DEV_DUALSENSE:
        return QObject::tr("DualSense / DualSense Edge");
    case DEV_XBOX:
        return QObject::tr("Xbox and Xbox-style controllers");
    case DEV_STEAM:
        return QObject::tr("Steam Controller (2026) · Experimental");
    case DEV_ALL:
        return QObject::tr("All controllers");
    default:
        return QObject::tr("Controllers");
    }
}

QString LinkName(bool bluetooth) {
    return bluetooth ? QObject::tr("Bluetooth") : QObject::tr("USB");
}

void StyleMutedLabel(QLabel* label, const QPalette& pal) {
    if (!label) {
        return;
    }
    const QColor c = pal.color(QPalette::WindowText);
    label->setStyleSheet(QStringLiteral("color: rgba(%1, %2, %3, 178);")
                             .arg(c.red())
                             .arg(c.green())
                             .arg(c.blue()));
}

void StyleDivider(QFrame* line, const QPalette& pal) {
    if (!line) {
        return;
    }
    const QColor c = pal.color(QPalette::WindowText);
    line->setStyleSheet(QStringLiteral("background-color: rgba(%1, %2, %3, 28); border: none;")
                            .arg(c.red())
                            .arg(c.green())
                            .arg(c.blue()));
}

class ToggleSwitch final : public QCheckBox {
public:
    explicit ToggleSwitch(QWidget* parent = nullptr) : QCheckBox(parent) {
        setFixedSize(36, 20);
        setCursor(Qt::PointingHandCursor);
    }

    QSize sizeHint() const override {
        return {36, 20};
    }

protected:
    bool hitButton(const QPoint& pos) const override {
        return rect().contains(pos);
    }

    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.setRenderHint(QPainter::Antialiasing);

        const QRectF track = QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5);
        const qreal radius = track.height() / 2.0;

        QColor track_color;
        if (isChecked()) {
            track_color = palette().color(QPalette::Highlight);
            if (!isEnabled()) {
                track_color.setAlpha(110);
            }
        } else {
            track_color = palette().color(QPalette::WindowText);
            track_color.setAlpha(isEnabled() ? 48 : 24);
        }
        p.setPen(Qt::NoPen);
        p.setBrush(track_color);
        p.drawRoundedRect(track, radius, radius);

        const qreal knob_d = height() - 6.0;
        const qreal knob_x = isChecked() ? width() - knob_d - 3.0 : 3.0;
        QColor knob_color(Qt::white);
        if (!isEnabled()) {
            knob_color.setAlpha(170);
        }
        p.setBrush(knob_color);
        p.drawEllipse(QRectF(knob_x, 3.0, knob_d, knob_d));
    }
};

class ClickableLogo final : public QLabel {
public:
    explicit ClickableLogo(QWidget* parent = nullptr) : QLabel(parent) {
        setCursor(Qt::PointingHandCursor);
    }

protected:
    void mouseReleaseEvent(QMouseEvent* event) override {
        if (event->button() == Qt::LeftButton && rect().contains(event->pos())) {
            QDesktopServices::openUrl(QUrl(QString::fromLatin1(REPO_URL)));
        }
        QLabel::mouseReleaseEvent(event);
    }
};

} // namespace

struct SettingsWidget::Row {
    int index{};
    QWidget* box{};
    bool link_off{};
};

QLabel* SettingsWidget::MakeMuted(const QString& text, bool body_item, QWidget* parent) {
    auto* label = new QLabel(text, parent);
    label->setWordWrap(true);
    StyleMutedLabel(label, palette());
    if (body_item) {
        body_muted_labels.push_back(label);
    } else {
        static_muted_labels.push_back(label);
    }
    return label;
}

QFrame* SettingsWidget::MakeDivider(bool body_item, QWidget* parent) {
    auto* line = new QFrame(parent);
    line->setFixedHeight(1);
    StyleDivider(line, palette());
    if (body_item) {
        body_dividers.push_back(line);
    } else {
        static_dividers.push_back(line);
    }
    return line;
}

SettingsWidget::SettingsWidget(Host host_, QWidget* parent)
    : QWidget(parent), host{std::move(host_)} {
    if (!host.api.Complete()) {
        auto* layout = new QVBoxLayout(this);
        layout->addWidget(MakeMuted(tr("renhdrum.dll was not found next to the program, so "
                                       "the controllers use the standard rumble."),
                                    false));
        return;
    }
    options = host.api.options(&option_count, &stride);
    store = host.api.store_create();
    if (!options || !store) {
        return;
    }
    // Saved values over the recommended defaults; unknown saved keys are never asked for
    for (int i = 0; i < option_count; ++i) {
        int value = 0;
        const char* key = ((const RenHdcOption*)((const char*)options + i * stride))->key;
        if (host.load && host.load(key, &value)) {
            host.api.store_set(store, key, value);
        }
    }
    if (host.active_pad) {
        pad = host.active_pad();
    }

    auto* root = new QVBoxLayout(this);
    root->setSpacing(10);

    // 1. Header: title, optional test button, and master switch
    auto* top = new QHBoxLayout;
    top->setSpacing(10);
    QString master_help;
    int master_index = -1;
    bool master_on = true;
    for (int i = 0; i < option_count; ++i) {
        const auto& o = *(const RenHdcOption*)((const char*)options + i * stride);
        if (QByteArray(o.key) == "enable_hd_rumble") {
            master_index = i;
            master_help = QString::fromUtf8(o.help);
            master_on = ValueAt(i) != 0;
            break;
        }
    }

    auto* heading = new QLabel(tr("HD Rumble"));
    QFont heading_font = heading->font();
    heading_font.setPointSizeF(heading_font.pointSizeF() * 1.2);
    heading_font.setWeight(QFont::DemiBold);
    heading->setFont(heading_font);
    if (!master_help.isEmpty()) {
        heading->setToolTip(master_help);
    }
    top->addWidget(heading);
    top->addStretch(1);

    if (host.test_rumble) {
        test_button = new QPushButton(tr("Test rumble"));
        connect(test_button, &QPushButton::clicked, this, [this] { host.test_rumble(); });
        top->addWidget(test_button);
    }

    if (master_index >= 0) {
        auto* master = new ToggleSwitch;
        master->setToolTip(master_help);
        master->setChecked(master_on);
        master->setAccessibleName(tr("HD Rumble"));
        master_box = master;
        top->addWidget(master);
        connect(master, &QCheckBox::toggled, this, [this, master_index](bool on) {
            SetValue(master_index, on ? 1 : 0);
            if (body) {
                body->setEnabled(on);
            }
            if (test_button) {
                test_button->setEnabled(on);
            }
            if (host.master_changed) {
                host.master_changed(on);
            }
        });
    }
    root->addLayout(top);
    root->addWidget(MakeDivider(false));

    // 2-4. The part that follows the controller in use
    body = new QWidget;
    body_layout = new QVBoxLayout(body);
    body_layout->setContentsMargins(0, 0, 0, 0);
    body_layout->setSpacing(10);
    root->addWidget(body, 1);

    // 5. Footer: divider + RenHDrum logo (18 px) + Powered by (70 % opacity) + Report button
    root->addWidget(MakeDivider(false));
    auto* footer = new QHBoxLayout;
    footer->setSpacing(10);

    auto* logo = new ClickableLogo;
    logo->setToolTip(QString::fromLatin1(REPO_URL));
    logo_label = logo;
    footer->addWidget(logo, 0, Qt::AlignVCenter);
    footer->addSpacing(4);

    if (host.credit_suffix) {
        credit_suffix_text = host.credit_suffix();
    }
    credit_label = MakeMuted(QString{}, false);
    credit_label->setWordWrap(true);
    credit_label->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    credit_label->setOpenExternalLinks(true);
    footer->addWidget(credit_label, 1, Qt::AlignVCenter);

    if (host.report_problem) {
        auto* report = new QPushButton(tr("Report a problem"));
        connect(report, &QPushButton::clicked, this, [this] { host.report_problem(); });
        footer->addWidget(report, 0, Qt::AlignVCenter);
    }
    root->addLayout(footer);

    Rebuild();
    ApplyThemeColors();

    poll = new QTimer(this);
    poll->setInterval(1000);
    connect(poll, &QTimer::timeout, this, [this] {
        if (!host.active_pad) {
            return;
        }
        const ActivePad now = host.active_pad();
        if (!(now == pad)) {
            pad = now;
            Rebuild();
        }
    });
    poll->start();
}

SettingsWidget::~SettingsWidget() {
    for (Row* row : rows) {
        delete row;
    }
    if (store) {
        host.api.store_destroy(store);
    }
}

void SettingsWidget::changeEvent(QEvent* event) {
    QWidget::changeEvent(event);
    if (event->type() == QEvent::PaletteChange ||
        event->type() == QEvent::ApplicationPaletteChange) {
        ApplyThemeColors();
    }
}

void SettingsWidget::ApplyThemeColors() {
    const QPalette pal = palette();
    const bool dark = IsDarkPalette(pal);

    for (const auto& label : static_muted_labels) {
        StyleMutedLabel(label, pal);
    }
    for (const auto& label : body_muted_labels) {
        StyleMutedLabel(label, pal);
    }
    for (const auto& line : static_dividers) {
        StyleDivider(line, pal);
    }
    for (const auto& line : body_dividers) {
        StyleDivider(line, pal);
    }

    if (logo_label) {
        const QPixmap pm(dark ? QStringLiteral(":/brand/logo/renhdrum-light-18.png")
                              : QStringLiteral(":/brand/logo/renhdrum-dark-18.png"));
        if (!pm.isNull()) {
            logo_label->setPixmap(pm);
            logo_label->setFixedSize(pm.deviceIndependentSize().toSize());
            logo_label->setVisible(true);
        } else {
            logo_label->setVisible(false);
        }
    }

    if (credit_label) {
        const QColor fg = pal.color(QPalette::WindowText);
        const QColor window = pal.color(QPalette::Window);
        const QColor bg =
            (window.alpha() == 255)
                ? window
                : (dark ? QColor(0x12, 0x11, 0x10) : QColor(0xF6, 0xF5, 0xF0));
        const QColor link_color((fg.red() * 7 + bg.red() * 3) / 10,
                                (fg.green() * 7 + bg.green() * 3) / 10,
                                (fg.blue() * 7 + bg.blue() * 3) / 10);
        QString credit = tr("Powered by <a href=\"%1\" style=\"color:%2;text-decoration:none;\">"
                            "RenHDrum</a>")
                             .arg(QString::fromLatin1(REPO_URL), link_color.name());
        if (!credit_suffix_text.isEmpty()) {
            credit += QStringLiteral(" ") + credit_suffix_text.toHtmlEscaped();
        }
        credit_label->setText(credit);
    }

    if (active_pill_label && !active_pill_text.isEmpty()) {
        const QString dot = dark ? QStringLiteral("#65A30D") : QStringLiteral("#4D7C0F");
        active_pill_label->setText(
            QStringLiteral("<span style=\"color:%1;\">&#9679;</span> %2")
                .arg(dot, active_pill_text.toHtmlEscaped()));
    }
}

int SettingsWidget::Value(const char* key) const {
    int value = 0;
    if (store) {
        host.api.store_get(store, key, &value);
    }
    return value;
}

int SettingsWidget::ValueAt(int index) const {
    const auto& o = *(const RenHdcOption*)((const char*)options + index * stride);
    int value = o.def;
    host.api.store_get(store, o.key, &value);
    return value;
}

void SettingsWidget::SetValue(int index, int value) {
    const auto& o = *(const RenHdcOption*)((const char*)options + index * stride);
    host.api.store_set(store, o.key, value);
    if (host.save) {
        host.save(o.key, ValueAt(index));
    }
    Refresh();
}

bool SettingsWidget::RowVisibleOnLink(int index, const ActivePad& active) const {
    const auto& o = *(const RenHdcOption*)((const char*)options + index * stride);
    return o.link == 0 || (o.link == 2) == active.bluetooth;
}

QWidget* SettingsWidget::MakeRow(int index, QWidget* parent) {
    const auto& o = *(const RenHdcOption*)((const char*)options + index * stride);
    auto* box = new QWidget(parent);
    auto* row_layout = new QHBoxLayout(box);
    row_layout->setContentsMargins(0, 4, 0, 4);
    row_layout->setSpacing(12);

    const QString label_text = QString::fromUtf8(o.label);
    const QString help_text = QString::fromUtf8(o.help);
    const int value = ValueAt(index);

    auto* left = new QVBoxLayout;
    left->setContentsMargins(0, 0, 0, 0);
    left->setSpacing(2);
    auto* title_label = new QLabel(label_text);
    title_label->setWordWrap(true);
    left->addWidget(title_label);
    if (!help_text.isEmpty()) {
        left->addWidget(MakeMuted(help_text, true));
    }
    row_layout->addLayout(left, 1);

    if (o.kind == 0) {
        auto* toggle = new ToggleSwitch;
        toggle->setChecked(value != 0);
        toggle->setAccessibleName(label_text);
        connect(toggle, &QCheckBox::toggled, this,
                [this, index](bool on) { SetValue(index, on ? 1 : 0); });
        row_layout->addWidget(toggle, 0, Qt::AlignVCenter);
    } else if (o.kind == 2) {
        auto* combo = new QComboBox;
        combo->addItems(QString::fromUtf8(o.choices).split(QLatin1Char('|')));
        combo->setCurrentIndex(std::clamp(value, 0, std::max(0, combo->count() - 1)));
        connect(combo, qOverload<int>(&QComboBox::currentIndexChanged), this,
                [this, index](int i) { SetValue(index, i); });
        row_layout->addWidget(combo, 0, Qt::AlignVCenter);
    } else {
        auto* controls = new QHBoxLayout;
        controls->setContentsMargins(0, 0, 0, 0);
        controls->setSpacing(8);
        auto* slider = new QSlider(Qt::Horizontal);
        slider->setMinimumWidth(120);
        slider->setRange(o.min, o.max);
        slider->setSingleStep(std::max(1, o.step));
        slider->setPageStep(std::max(1, o.step) * 4);
        slider->setValue(value);

        auto* spin = new QSpinBox;
        spin->setFont(MonospaceFont());
        spin->setRange(o.min, o.max);
        spin->setSingleStep(std::max(1, o.step));
        spin->setSuffix(QStringLiteral(" %"));
        spin->setValue(value);

        connect(slider, &QSlider::valueChanged, this, [this, index, spin](int v) {
            {
                const QSignalBlocker block{spin};
                spin->setValue(v);
            }
            SetValue(index, v);
        });
        connect(spin, qOverload<int>(&QSpinBox::valueChanged), this, [slider](int v) {
            slider->setValue(v);
        });
        controls->addWidget(slider);
        controls->addWidget(spin);
        row_layout->addLayout(controls, 0);
    }
    return box;
}

QWidget* SettingsWidget::MakeCard(const QString& title, unsigned device, bool only_matching_link,
                                  const ActivePad& active) {
    auto* card = new QFrame;
    card->setObjectName(QStringLiteral("RenHDrumCard"));
    card->setProperty("songRungCard", true);
    card->setFrameShape(QFrame::StyledPanel);

    auto* col = new QVBoxLayout(card);
    col->setContentsMargins(14, 12, 14, 12);
    col->setSpacing(6);

    auto* card_header = new QHBoxLayout;
    card_header->setSpacing(8);
    auto* card_title = new QLabel(title);
    QFont title_font = card_title->font();
    title_font.setWeight(QFont::DemiBold);
    card_title->setFont(title_font);
    card_header->addWidget(card_title);

    if (only_matching_link) {
        auto* pill = new QLabel;
        pill->setProperty("songRungPill", true);
        pill->setTextFormat(Qt::RichText);
        active_pill_text = tr("%1 · active").arg(LinkName(active.bluetooth));
        active_pill_label = pill;
        card_header->addWidget(pill);
    }
    card_header->addStretch(1);
    col->addLayout(card_header);

    bool any = false;
    bool first_row = true;
    auto add_row = [&](QWidget* w) {
        if (!first_row) {
            col->addWidget(MakeDivider(true, card));
        }
        first_row = false;
        col->addWidget(w);
        any = true;
    };

    for (int i = 0; i < option_count; ++i) {
        const auto& o = *(const RenHdcOption*)((const char*)options + i * stride);
        if (o.tier != 0 || !(o.devices & device) || o.devices == DEV_ALL) {
            continue; // Advanced rows, and the master switch (shown on top)
        }
        if (only_matching_link && !RowVisibleOnLink(i, active)) {
            continue;
        }
        auto* box = MakeRow(i, card);
        rows.push_back(new Row{i, box, false});
        add_row(box);
    }
    if (device == DEV_STEAM) {
        add_row(MakeMuted(tr("Grips · Experimental. Follows the HD Rumble switch. If rumble "
                             "stops, close Steam or reconnect the controller."),
                          true, card));
    }
    if (device == DEV_XBOX && host.xbox_extra_row) {
        if (QWidget* extra = host.xbox_extra_row(card)) {
            add_row(extra);
        }
    }
    if (!any) {
        card->deleteLater();
        return nullptr;
    }
    return card;
}

QWidget* SettingsWidget::MakeCollapsible(const QString& title, QWidget* content,
                                         bool* open_state) {
    auto* holder = new QWidget;
    auto* col = new QVBoxLayout(holder);
    col->setContentsMargins(0, 0, 0, 0);
    col->setSpacing(6);

    auto* head = new QToolButton;
    head->setText(title);
    head->setCheckable(true);
    head->setChecked(*open_state);
    head->setAutoRaise(true);
    head->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    head->setArrowType(*open_state ? Qt::DownArrow : Qt::RightArrow);
    head->setCursor(Qt::PointingHandCursor);
    content->setVisible(*open_state);
    connect(head, &QToolButton::toggled, this, [head, content, open_state](bool open) {
        *open_state = open;
        head->setArrowType(open ? Qt::DownArrow : Qt::RightArrow);
        content->setVisible(open);
    });
    col->addWidget(head);
    col->addWidget(content);
    return holder;
}

QWidget* SettingsWidget::MakeAdvanced(const ActivePad& active) {
    auto* content = new QWidget;
    auto* col = new QVBoxLayout(content);
    col->setContentsMargins(0, 0, 0, 0);
    col->setSpacing(8);

    // First row: Recommended / Custom + reset
    auto* status = new QHBoxLayout;
    recommended_label = new QLabel;
    reset_button = new QPushButton(tr("Reset to recommended"));
    connect(reset_button, &QPushButton::clicked, this, [this] {
        host.api.store_reset(store);
        // Save every value back, then redraw so the widgets show them
        for (int i = 0; i < option_count; ++i) {
            const auto& o = *(const RenHdcOption*)((const char*)options + i * stride);
            if (host.save) {
                host.save(o.key, ValueAt(i));
            }
        }
        if (auto* check = qobject_cast<QCheckBox*>(master_box)) {
            const QSignalBlocker block{check};
            int value = 1;
            host.api.store_get(store, "enable_hd_rumble", &value);
            check->setChecked(value != 0);
            body->setEnabled(value != 0);
            if (test_button) {
                test_button->setEnabled(value != 0);
            }
        }
        Rebuild();
    });
    status->addWidget(recommended_label);
    status->addStretch(1);
    status->addWidget(reset_button);
    col->addLayout(status);

    // Groups: controller family, then link, in the order the table lists them
    std::vector<std::pair<unsigned, int>> groups;
    for (int i = 0; i < option_count; ++i) {
        const auto& o = *(const RenHdcOption*)((const char*)options + i * stride);
        if (o.tier != 1) {
            continue;
        }
        const std::pair<unsigned, int> key{o.devices, o.link};
        if (std::find(groups.begin(), groups.end(), key) == groups.end()) {
            groups.push_back(key);
        }
    }
    for (const auto& [devices, link] : groups) {
        QString title = DeviceName(devices);
        if (link == 1) {
            title += tr(" · Wired (USB)");
        } else if (link == 2) {
            title += tr(" · Bluetooth");
        }
        auto* group = new QFrame;
        group->setObjectName(QStringLiteral("RenHDrumCard"));
        group->setProperty("songRungCard", true);
        group->setFrameShape(QFrame::StyledPanel);

        auto* gcol = new QVBoxLayout(group);
        gcol->setContentsMargins(14, 10, 14, 10);
        gcol->setSpacing(6);

        auto* group_title = new QLabel(title);
        QFont gf = group_title->font();
        gf.setWeight(QFont::DemiBold);
        group_title->setFont(gf);
        gcol->addWidget(group_title);

        for (int i = 0; i < option_count; ++i) {
            const auto& o = *(const RenHdcOption*)((const char*)options + i * stride);
            if (o.tier != 1 || o.devices != devices || o.link != link) {
                continue;
            }
            gcol->addWidget(MakeDivider(true, group));
            auto* box = MakeRow(i, group);
            // A row for one link only: greyed when the controller of its family is on the other
            const bool link_off = active.device != 0 && (o.devices & active.device) &&
                                  !RowVisibleOnLink(i, active);
            if (link_off) {
                auto* wrapper = new QWidget(group);
                auto* wcol = new QVBoxLayout(wrapper);
                wcol->setContentsMargins(0, 0, 0, 0);
                wcol->setSpacing(2);
                wcol->addWidget(box);
                wcol->addWidget(MakeMuted(tr("Not in use: the controller is connected by %1.")
                                              .arg(LinkName(active.bluetooth)),
                                          true, wrapper));
                box = wrapper;
            }
            rows.push_back(new Row{i, box, link_off});
            gcol->addWidget(box);
        }
        col->addWidget(group);
    }
    return content;
}

void SettingsWidget::Rebuild() {
    // Drop the old rows; the values live in the store, so nothing is lost
    for (Row* row : rows) {
        delete row;
    }
    rows.clear();
    body_muted_labels.clear();
    body_dividers.clear();
    active_pill_label = nullptr;
    active_pill_text.clear();
    recommended_label = nullptr;
    reset_button = nullptr;

    while (QLayoutItem* item = body_layout->takeAt(0)) {
        if (QWidget* w = item->widget()) {
            w->deleteLater();
        }
        delete item;
    }

    // 2. The controller in use
    if (pad.device == 0) {
        auto* card = new QFrame;
        card->setObjectName(QStringLiteral("RenHDrumCard"));
        card->setProperty("songRungCard", true);
        card->setFrameShape(QFrame::StyledPanel);
        auto* clayout = new QVBoxLayout(card);
        clayout->setContentsMargins(14, 12, 14, 12);
        clayout->addWidget(MakeMuted(tr("Connect a controller to see its settings."), true, card));
        body_layout->addWidget(card);
    } else {
        QString title = pad.name.isEmpty() ? DeviceName(pad.device) : pad.name;
        if (QWidget* card = MakeCard(title, pad.device, true, pad)) {
            body_layout->addWidget(card);
        }
    }

    // 3. Other controllers (collapsed)
    auto* others = new QWidget;
    auto* ocol = new QVBoxLayout(others);
    ocol->setContentsMargins(0, 0, 0, 0);
    ocol->setSpacing(8);
    for (const unsigned device : {DEV_SWITCH, DEV_DUALSENSE, DEV_XBOX, DEV_STEAM}) {
        if (device == pad.device) {
            continue;
        }
        if (QWidget* card = MakeCard(DeviceName(device), device, false, pad)) {
            ocol->addWidget(card);
        }
    }
    body_layout->addWidget(MakeCollapsible(tr("Other controllers"), others, &others_open));

    // 4. Advanced (collapsed)
    body_layout->addWidget(MakeDivider(true));
    body_layout->addWidget(MakeCollapsible(tr("Advanced"), MakeAdvanced(pad), &advanced_open));
    body_layout->addStretch(1);

    if (auto* check = qobject_cast<QCheckBox*>(master_box)) {
        body->setEnabled(check->isChecked());
        if (test_button) {
            test_button->setEnabled(check->isChecked());
        }
    }
    ApplyThemeColors();
    Refresh();
}

void SettingsWidget::Refresh() {
    if (!store) {
        return;
    }
    for (const Row* row : rows) {
        const auto& o = *(const RenHdcOption*)((const char*)options + row->index * stride);
        const bool needed = o.requires_row < 0 || ValueAt(o.requires_row) != 0;
        row->box->setEnabled(needed && !row->link_off);
    }
    if (recommended_label) {
        bool ok = true;
        host.api.store_is_recommended(store, &ok);
        recommended_label->setText(ok ? tr("Recommended ✓") : tr("Custom"));
        reset_button->setVisible(!ok);
    }
}

} // namespace RenHDrumQt
