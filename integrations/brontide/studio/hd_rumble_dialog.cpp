// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <span>
#include <string>
#include <utility>

#include <QAbstractButton>
#include <QButtonGroup>
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QProcess>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSlider>
#include <QSpinBox>
#include <QStackedWidget>
#include <QStyle>
#include <QTimer>
#include <QVBoxLayout>

#ifdef _WIN32
#include <windows.h>
#endif

#include "common/param_package.h"
#include "common/settings.h"
#include "hid_core/frontend/emulated_controller.h"
#include "hid_core/hid_core.h"
#include "qt_common/config/uisettings.h"
#include "yuzu/brontide/hd_rumble_dialog.h"
#include "yuzu/brontide/hd_rumble_widgets.h"
#include "yuzu/brontide/header_bar.h"
#include "yuzu/brontide/controller_report.h"
#include "yuzu/brontide/thunder.h"
#include "yuzu/brontide/version.h"

namespace Brontide {

namespace {

// renhdrum-settings.txt: the 24 keys of RenHdcConfig in the order Brontide-App writes them, with
// RumbleConfigController::default() (RenHDrum/src/settings.rs), which is the recommended mode.
const std::array<std::pair<const char*, const char*>, 24> SETTINGS_DEFAULTS{{
    {"enable_hd_rumble", "true"},
    {"hd_rumble_usb_interval_ms", "15"},
    {"hd_rumble_bluetooth_level", "4"},
    {"hd_rumble_usb_auto_slow", "true"},
    {"hd_rumble_experimental_usb_no_queue", "true"},
    {"hd_rumble_experimental_envelope_5ms", "true"},
    {"hd_rumble_experimental_frequency_in_packet", "true"},
    {"hd_rumble_experimental_frequency_both_bands", "true"},
    {"hd_rumble_experimental_bt_after_input", "true"},
    {"hd_rumble_experimental_bt_fast", "true"},
    {"hd_rumble_experimental_crisp_notes", "true"},
    {"hd_rumble_experimental_bt_crisp_notes", "true"},
    {"hd_rumble_experimental_sync_to_input", "false"},
    {"hd_rumble_experimental_click_pulse", "false"},
    {"hd_rumble_experimental_sync_bluetooth", "false"},
    {"hd_rumble_sync_auto_slow", "false"},
    {"strong_rumble", "1"},
    {"weak_rumble", "1"},
    {"enable_rumble", "true"},
    {"enable_adaptive_triggers", "true"},
    {"enable_dualsense_haptics", "true"},
    {"dualsense_haptics_volume", "100"},
    {"dualsense_crisp_notes", "true"},
    {"dualsense_bt_crisp_notes", "false"},
}};

// recommended.rs: which keys belong to a connection
constexpr std::array WIRED_KEYS{"hd_rumble_usb_interval_ms",
                                "hd_rumble_usb_auto_slow",
                                "hd_rumble_experimental_usb_no_queue",
                                "hd_rumble_experimental_envelope_5ms",
                                "hd_rumble_experimental_frequency_in_packet",
                                "hd_rumble_experimental_frequency_both_bands",
                                "hd_rumble_experimental_crisp_notes"};
constexpr std::array BLUETOOTH_KEYS{"hd_rumble_bluetooth_level", "hd_rumble_experimental_bt_after_input",
                                    "hd_rumble_experimental_bt_fast",
                                    "hd_rumble_experimental_bt_crisp_notes"};
constexpr std::array DUALSENSE_WIRED_KEYS{"dualsense_crisp_notes"};
constexpr std::array DUALSENSE_BLUETOOTH_KEYS{"dualsense_bt_crisp_notes"};

std::span<const char* const> ConnectionKeys(bool wired, bool dualsense) {
    if (dualsense) {
        return wired ? std::span<const char* const>(DUALSENSE_WIRED_KEYS) : DUALSENSE_BLUETOOTH_KEYS;
    }
    return wired ? std::span<const char* const>(WIRED_KEYS) : BLUETOOTH_KEYS;
}

// Flag files next to the executable beat the settings file (renhdrum_settings_resolve), so the
// ones an earlier version of this dialog wrote are read once and then removed on save.
constexpr char OFF_FLAG[] = "hdrumble_off";
constexpr char BLUETOOTH_LEVEL_FLAG[] = "hdrumble_bt_level.txt";
constexpr char USB_INTERVAL_FLAG[] = "hdrumble_interval_ms.txt";
constexpr int MIN_DUALSENSE_VOLUME = 10;
constexpr int MAX_DUALSENSE_VOLUME = 100;
constexpr int PAGE_MAX_WIDTH = 640;

const QColor CYAN = QColor::fromRgba(STUDIO_CYAN);

QString FlagPath(const char* name) {
    return QDir(QCoreApplication::applicationDirPath()).filePath(QString::fromLatin1(name));
}

QString SettingsPath() {
    const QString appdata = qEnvironmentVariable("APPDATA");
    return appdata.isEmpty() ? QString{}
                             : QDir(appdata).filePath(QStringLiteral("RenHDrum/renhdrum-settings.txt"));
}

int ReadFlagInt(const char* name, int fallback) {
    QFile file(FlagPath(name));
    if (!file.open(QIODevice::ReadOnly)) {
        return fallback;
    }
    bool ok = false;
    const int value = QString::fromLatin1(file.readAll()).trimmed().toInt(&ok);
    return ok ? value : fallback;
}

QString DefaultValue(const char* key) {
    for (const auto& [name, value] : SETTINGS_DEFAULTS) {
        if (QByteArray(name) == key) {
            return QString::fromLatin1(value);
        }
    }
    return {};
}

bool BrontideAppIsOpen() {
#ifdef _WIN32
    // Same name as RenHDrum/src/ipc.rs SHMEM_NAME
    if (HANDLE mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, L"Local\\RenHDrum_SharedState_v1")) {
        CloseHandle(mapping);
        return true;
    }
#endif
    return false;
}

QLabel* Label(const QString& text, const char* name, QWidget* parent) {
    auto* label = new QLabel(text, parent);
    label->setObjectName(QString::fromLatin1(name));
    label->setWordWrap(true);
    return label;
}

/// One scrolling page: a centered column of at most 640 px (Brontide-App's PAGE_MAX_W).
QScrollArea* NewPage(QVBoxLayout*& column) {
    auto* scroll = new QScrollArea;
    scroll->setObjectName(QStringLiteral("StudioScroll"));
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* holder = new QWidget;
    auto* row = new QHBoxLayout(holder);
    row->setContentsMargins(24, 14, 24, 24);
    auto* content = new QWidget;
    content->setObjectName(QStringLiteral("StudioPlain"));
    content->setMaximumWidth(PAGE_MAX_WIDTH);
    column = new QVBoxLayout(content);
    column->setContentsMargins(0, 0, 0, 0);
    column->setSpacing(0);
    row->addStretch(1);
    row->addWidget(content, 100, Qt::AlignTop);
    row->addStretch(1);
    scroll->setWidget(holder);
    return scroll;
}

void AddTitle(QVBoxLayout* column, const QString& title, const QString& subtitle) {
    column->addWidget(Label(title, "StudioTitle", nullptr));
    if (!subtitle.isEmpty()) {
        column->addWidget(Label(subtitle, "StudioSubtitle", nullptr));
    }
}

/// Settings are read when a controller connects; closing the dialog re-runs that for every connected one
QString SavedText() {
    return HdRumbleDialog::tr("Saved. Applies to connected controllers when you press Done.");
}

QLabel* Chevron() {
    auto* chevron = new QLabel(QStringLiteral("›"));
    chevron->setObjectName(QStringLiteral("StudioChevron"));
    return chevron;
}

} // namespace

QString CoreVersion() {
#ifdef _WIN32
    if (HMODULE core = GetModuleHandleW(L"renhdrum.dll")) {
        if (void* symbol = reinterpret_cast<void*>(GetProcAddress(core, "renhdrum_version"))) {
            const uint32_t version = reinterpret_cast<uint32_t (*)()>(symbol)();
            return QStringLiteral("%1.%2.%3").arg(version >> 16).arg((version >> 8) & 0xff).arg(version & 0xff);
        }
    }
#endif
    return {};
}

HdRumbleDialog::HdRumbleDialog(QWidget* parent, Core::HID::HIDCore& hid_core_)
    : QDialog(parent), hid_core{hid_core_} {
    setObjectName(QStringLiteral("BrontideHdRumbleDialog"));
    setWindowTitle(tr("HD Rumble"));
    setMinimumSize(740, 520);
    resize(860, 620);
    Load();

    auto* root = new QVBoxLayout(this);
    root->setContentsMargins(0, 0, 0, 0);
    root->setSpacing(0);
    auto* strip = new QFrame(this);
    strip->setObjectName(QStringLiteral("StudioTopStrip"));
    strip->setFixedHeight(2);
    root->addWidget(strip);

    auto* body = new QHBoxLayout;
    body->setSpacing(0);
    root->addLayout(body, 1);
    body->addWidget(BuildSidebar());

    auto* right = new QVBoxLayout;
    right->setSpacing(0);
    body->addLayout(right, 1);
    right->addWidget(BuildMainSwitchBar());
    stack = new QStackedWidget(this);
    right->addWidget(stack, 1);

    // The page order is the Page enum
    QVBoxLayout* column = nullptr;
    stack->addWidget(NewPage(column));
    BuildOverview(column);
    stack->addWidget(NewPage(column));
    BuildSwitch(column);
    stack->addWidget(NewPage(column));
    BuildDetails(column);
    stack->addWidget(NewPage(column));
    BuildDualSense(column);
    stack->addWidget(NewPage(column));
    BuildXbox(column);

    // Saved line and Done, like Brontide-App's status line
    auto* bottom = new QWidget(this);
    bottom->setObjectName(QStringLiteral("StudioBottom"));
    auto* bottom_layout = new QHBoxLayout(bottom);
    bottom_layout->setContentsMargins(24, 10, 24, 10);
    auto* notes = new QVBoxLayout;
    notes->setSpacing(2);
    if (BrontideAppIsOpen()) {
        notes->addWidget(Label(tr("Brontide-App is open: its settings win until it is closed."),
                               "StudioFootnote", bottom));
    }
    auto* status = new QHBoxLayout;
    status->setSpacing(6);
    status_dot = new QLabel(QStringLiteral("●"), bottom);
    status_dot->setObjectName(QStringLiteral("StudioDot"));
    status_text = Label(SavedText(),
                        "StudioFootnote", bottom);
    status_dot->hide(); // shown by the first save
    status_text->hide();
    status->addWidget(status_dot);
    status->addWidget(status_text, 1);
    notes->addLayout(status);
    bottom_layout->addLayout(notes, 1);
    auto* report = new QPushButton(tr("Report a problem"), bottom);
    report->setObjectName(QStringLiteral("StudioPill"));
    report->setCursor(Qt::PointingHandCursor);
    report->setToolTip(tr("Save the rumble logs and controller list to your Desktop and open GitHub to describe the problem"));
    connect(report, &QPushButton::clicked, this, [this] { ReportControllerProblem(this, hid_core); });
    bottom_layout->addWidget(report);
    auto* done = new QPushButton(tr("Done"), bottom);
    done->setObjectName(QStringLiteral("StudioDone"));
    done->setCursor(Qt::PointingHandCursor);
    done->setDefault(true);
    connect(done, &QPushButton::clicked, this, &QDialog::accept);
    // Done, Esc or the close box: connected controllers pick up what was saved, no replug needed
    connect(this, &QDialog::finished, this, [this] {
        if (saved_any) {
            InputCommon::ReapplyRenHDrumSettings();
        }
    });
    bottom_layout->addWidget(done);
    right->addWidget(bottom);

    SyncWidgets();

    // Only ticks while this dialog is open (it is modal and destroyed with it)
    auto* refresh = new QTimer(this);
    connect(refresh, &QTimer::timeout, this, &HdRumbleDialog::RefreshPads);
    refresh->start(1000);
    RefreshPads();
}

// ---------------------------------------------------------------- building

QWidget* HdRumbleDialog::BuildSidebar() {
    auto* side = new QWidget(this);
    side->setObjectName(QStringLiteral("StudioSidebar"));
    side->setFixedWidth(210);
    auto* layout = new QVBoxLayout(side);
    layout->setContentsMargins(12, 16, 12, 12);
    layout->setSpacing(6);

    auto* brand = new QHBoxLayout;
    brand->setContentsMargins(4, 0, 0, 10);
    auto* logo = new QLabel(side);
    logo->setPixmap(WaveIcon(28).pixmap(28, 28));
    brand->addWidget(logo);
    brand->addWidget(new QLabel(tr("HD Rumble"), side));
    brand->itemAt(1)->widget()->setObjectName(QStringLiteral("StudioBrand"));
    brand->addStretch(1);
    layout->addLayout(brand);

    struct Item {
        QString text;
        int indent;
    };
    const std::array<Item, PageCount> items{{
        {tr("Controllers"), 0},
        {tr("Switch Controller"), 0},
        {tr("Details"), 1},
        {tr("DualSense"), 0},
        {tr("Xbox One S"), 0},
    }};
    nav_group = new QButtonGroup(this);
    nav_group->setExclusive(true);
    for (int page = 0; page < PageCount; ++page) {
        auto* button = new QPushButton(side);
        button->setObjectName(QStringLiteral("StudioNavItem"));
        button->setCheckable(true);
        button->setCursor(Qt::PointingHandCursor);
        button->setFocusPolicy(Qt::NoFocus);
        auto* item = new QHBoxLayout(button);
        item->setContentsMargins(14 + items[page].indent * 16, 0, 10, 0);
        item->setSpacing(8);
        // A dot before the name tells whether such a controller is connected
        auto* dot = new QLabel(QStringLiteral("●"), button);
        dot->setObjectName(QStringLiteral("StudioNavDot"));
        auto* name = new QLabel(items[page].text, button);
        name->setObjectName(QStringLiteral("StudioNavText"));
        const bool has_dot = page != Overview && page != Details;
        dot->setVisible(has_dot);
        item->addWidget(dot);
        item->addWidget(name, 1);
        for (auto* child : {static_cast<QWidget*>(name), static_cast<QWidget*>(dot)}) {
            child->setAttribute(Qt::WA_TransparentForMouseEvents);
        }
        // Row height lives in the QSS: the theme's QPushButton min-height would undo a
        // setFixedHeight each time the row is re-polished to dim it
        button->setProperty("sub", page == Details);
        // Details opens from the Switch Controller page, so it keeps no row of its own
        button->setVisible(page != Details);
        nav_group->addButton(button, page);
        nav_buttons.push_back(button);
        nav_dots.push_back(dot);
        layout->addWidget(button);
    }
    nav_buttons[Overview]->setChecked(true);
    connect(nav_group, &QButtonGroup::idClicked, this, [this](int page) { stack->setCurrentIndex(page); });

    layout->addStretch(1);
    // The RenHDrum name appears here, nowhere else
    if (const QString version = CoreVersion(); !version.isEmpty()) {
        auto* credit_box = new QVBoxLayout;
        credit_box->setContentsMargins(4, 0, 0, 0);
        credit_box->setSpacing(0);
        auto* credit = new QHBoxLayout;
        credit->setSpacing(3);
        credit_box->addLayout(credit);
        const auto credit_label = [side](const QString& text) {
            auto* label = new QLabel(text, side);
            label->setObjectName(QStringLiteral("StudioCredit"));
            return label;
        };
        credit->addWidget(credit_label(tr("Powered by")));
        credit->addWidget(new GradientText(QStringLiteral("RenHDrum"), side));
        credit->addStretch(1);
        credit_box->addWidget(credit_label(tr("Brontide %1 · core v%2").arg(QString::fromLatin1(VERSION), version)));
        layout->addLayout(credit_box);
    }
    return side;
}

QWidget* HdRumbleDialog::BuildMainSwitchBar() {
    auto* bar = new QWidget(this);
    bar->setObjectName(QStringLiteral("StudioHeaderBar"));
    bar->setAttribute(Qt::WA_StyledBackground);
    auto* layout = new QHBoxLayout(bar);
    layout->setContentsMargins(24, 12, 24, 12);
    layout->setSpacing(12);
    layout->addWidget(Label(tr("HD Rumble"), "StudioHeaderTitle", bar), 1);
    auto* toggle = new ToggleSwitch(bar);
    bar->setToolTip(tr("On: HD Rumble on every controller (Switch, DualSense, Xbox). Off: every "
                       "controller falls back to standard rumble. Applies when you press Done."));
    toggles.push_back({toggle, QStringLiteral("enable_hd_rumble")});
    connect(toggle, &QAbstractButton::toggled, this, [this](bool on) {
        SetValue("enable_hd_rumble", on ? QStringLiteral("true") : QStringLiteral("false"));
        Changed();
    });
    layout->addWidget(toggle);
    return bar;
}

QLabel* HdRumbleDialog::NewOffNote() {
    auto* note = Label(tr("HD Rumble is off: this controller uses plain rumble. Turn it on at the top."),
                       "StudioNote", nullptr);
    off_notes.push_back(note);
    return note;
}

void HdRumbleDialog::BuildOverview(QVBoxLayout* column) {
    AddTitle(column, tr("Controllers"),
             tr("HD Rumble for Switch controllers, DualSense and Xbox. Pick a controller to see its settings."));

    pads_section = new StudioSection(tr("Connected controllers"), GlyphIcon(Glyph::ProController, 18, CYAN));
    pads_section->SetCardList();
    column->addWidget(pads_section);

    auto* thunder = new Thunder(hid_core, this);
    auto* try_thunder = new QPushButton(BoltIcon(16, QColor(0x1a, 0x13, 0x00)), tr("Try the thunder"));
    try_thunder->setObjectName(QStringLiteral("BrontideThunderButton"));
    try_thunder->setCursor(Qt::PointingHandCursor);
    try_thunder->setToolTip(tr("A crack and a rolling rumble on every connected controller"));
    connect(try_thunder, &QPushButton::clicked, thunder, &Thunder::Play);

    auto* on_connect = new StudioSection;
    auto* on_connect_toggle = new ToggleSwitch;
    on_connect_toggle->setChecked(UISettings::values.brontide_thunder_on_connect.GetValue());
    connect(on_connect_toggle, &QAbstractButton::toggled, this, [](bool on) {
        UISettings::values.brontide_thunder_on_connect = on;
    });
    on_connect->AddRow(new StudioRow(tr("Test rumble on every connected controller"), QString(), 0, try_thunder));
    on_connect->AddRow(new StudioRow(
        tr("Rumble a thunder when a controller connects"),
        tr("Plays the thunder once when a Pro Controller or DualSense connects while no game is "
           "running."),
        0, on_connect_toggle));
    column->addWidget(on_connect);
    column->addStretch(1);
}

void HdRumbleDialog::BuildSwitch(QVBoxLayout* column) {
    AddTitle(column, tr("Switch Controller"), tr("HD Rumble for Pro Controller and Joy-Con."));
    column->addWidget(NewOffNote());

    auto* switch_cards = new QWidget;
    switch_cards->setObjectName(QStringLiteral("StudioPlain"));
    switch_cards_layout = new QVBoxLayout(switch_cards);
    switch_cards_layout->setContentsMargins(0, 0, 0, 0);
    switch_cards_layout->setSpacing(0);

    auto* bluetooth = new StudioSection(tr("Bluetooth"), GlyphIcon(Glyph::Bluetooth, 18, CYAN));
    AddRecommendedRow(bluetooth, false);
    bluetooth_level = new Segmented({{1, tr("Detailed (5 ms)")}, {0, tr("Compatible")}});
    bluetooth->AddRow(new StudioRow(
        tr("Mode"),
        tr("Detailed (5 ms) plays the 5 ms envelope while the Bluetooth link is idle. Compatible "
           "sends the newest value at least 32 ms apart and resends every 40 ms while rumbling. "
           "Recommended: Detailed (5 ms). If unsure, keep the recommended setting."),
        0, bluetooth_level));
    connect(bluetooth_level, &Segmented::ValueChanged, this, [this](int detailed) {
        SetValue("hd_rumble_experimental_bt_fast",
                 detailed ? QStringLiteral("true") : QStringLiteral("false"));
        SetValue("hd_rumble_bluetooth_level", QStringLiteral("4"));
        Changed();
    });
    MakeCard(switch_bluetooth, switch_cards_layout, false, bluetooth);

    auto* wired = new StudioSection(tr("Wired"), GlyphIcon(Glyph::Usb, 18, CYAN));
    AddRecommendedRow(wired, true);
    MakeCard(switch_wired, switch_cards_layout, true, wired);
    column->addWidget(switch_cards);

    auto* more = new StudioSection;
    auto* link = new StudioRow(tr("Detailed settings"), QString(), 0, Chevron(),
                               GlyphIcon(Glyph::Sliders, 20, CYAN));
    link->SetClickable(true);
    connect(link, &StudioRow::Clicked, this, [this] { GoToPage(Details); });
    more->AddRow(link);
    column->addWidget(more);
    column->addStretch(1);
}

void HdRumbleDialog::BuildDetails(QVBoxLayout* column) {
    auto* back = new QPushButton(QStringLiteral("‹ %1").arg(tr("Switch Controller")));
    back->setObjectName(QStringLiteral("StudioBack"));
    back->setCursor(Qt::PointingHandCursor);
    connect(back, &QPushButton::clicked, this, [this] { GoToPage(Switch); });
    column->addWidget(back, 0, Qt::AlignLeft);
    AddTitle(column, tr("Detailed settings"), QString());

    detail_cards_layout = new QVBoxLayout;
    detail_cards_layout->setSpacing(0);
    auto* cards = new QWidget;
    cards->setObjectName(QStringLiteral("StudioPlain"));
    cards->setLayout(detail_cards_layout);
    detail_cards_layout->setContentsMargins(0, 0, 0, 0);

    const QString on_unsure = tr("Recommended: On. If unsure, keep the recommended setting.");
    auto* wired = new StudioSection(tr("Wired"), GlyphIcon(Glyph::Usb, 18, CYAN));
    AddRecommendedRow(wired, true);
    AddToggleRow(wired, tr("Send rumble without queuing"),
                 tr("Rumble packets go out right away instead of waiting in a queue, so buttons stay "
                    "responsive during heavy rumble. Turning this off also disables sub-items below. ") +
                     on_unsure,
                 "hd_rumble_experimental_usb_no_queue", 0);
    row3 = AddToggleRow(wired, tr("Detailed 5 ms rumble"),
                        tr("Game rumble is split into 5 ms moments, each 15 ms packet carrying three "
                           "moments like a real Switch, separating tight notes cleanly. May add ~15 ms "
                           "rumble latency. Turning this off also disables sub-items below. ") +
                            on_unsure,
                        "hd_rumble_experimental_envelope_5ms", 1);
    row5 = AddToggleRow(wired, tr("Accurate pitch, low band"),
                        tr("Allows dynamic frequency changes inside each 15 ms packet on the low band, "
                           "reproducing the game's exact pitch. Only active when parent items above "
                           "are on. ") +
                            on_unsure,
                        "hd_rumble_experimental_frequency_in_packet", 2);
    row7 = AddToggleRow(wired, tr("Accurate pitch, high and both bands"),
                        tr("Extends frequency updates to both high and low bands within the same "
                           "packet, especially noticeable in musical moments. "
                           "Requires parent items above to be on. ") +
                            on_unsure,
                        "hd_rumble_experimental_frequency_both_bands", 2);
    row8 = AddToggleRow(wired, tr("Accent each note (crisp notes)"),
                        tr("Gives a slight boost to the first few milliseconds of each new rumble note "
                           "so impacts, footsteps and clicks feel crisp and distinct. Requires parent "
                           "items above to be on. ") +
                            on_unsure,
                        "hd_rumble_experimental_crisp_notes", 2);
    auto* sub = new StudioRow(tr("When detailed rumble is off"), QString(), 0, nullptr);
    sub->setObjectName(QStringLiteral("StudioSubheader"));
    sub->setMinimumHeight(28);
    wired->AddRow(sub);
    interval = new Segmented({{10, tr("10 ms")}, {15, tr("15 ms")}, {20, tr("20 ms")}, {30, tr("30 ms")}});
    row_interval = new StudioRow(
        tr("Send interval"),
        tr("Only active when Send rumble without queuing or Detailed 5 ms rumble is off. Time between "
           "rumble packets over USB. Recommended: 15 ms. If unsure, keep the recommended setting."),
        0, interval);
    wired->AddRow(row_interval);
    connect(interval, &Segmented::ValueChanged, this, [this](int ms) {
        SetValue("hd_rumble_usb_interval_ms", QString::number(ms));
        Changed();
    });
    row_auto_slow = AddToggleRow(
        wired, tr("Slow down during heavy rumble"),
        tr("Only active when Send rumble without queuing is off. Automatically slows packet rate to "
           "30 ms during heavy rumble to keep vibrations smooth and steady. ") +
            on_unsure,
        "hd_rumble_usb_auto_slow", 0);
    MakeCard(detail_wired, detail_cards_layout, true, wired);

    auto* bluetooth = new StudioSection(tr("Bluetooth"), GlyphIcon(Glyph::Bluetooth, 18, CYAN));
    AddRecommendedRow(bluetooth, false);
    AddToggleRow(bluetooth, tr("Send rumble between button reads"),
                 tr("Bluetooth shares a single pipe for buttons and rumble. This sends rumble right "
                    "after a button read, keeping buttons responsive. Rumble may arrive about 15 ms "
                    "later, barely noticeable. ") +
                     on_unsure,
                 "hd_rumble_experimental_bt_after_input", 0);
    row_b3 = AddToggleRow(bluetooth, tr("Fast Bluetooth: 5 ms rumble like wired"),
                          tr("Speeds up Bluetooth rumble to near wired performance during rich music, "
                             "giving very detailed haptics. Requires Send rumble between button reads; "
                             "turn off if buttons feel laggy. ") +
                              on_unsure,
                          "hd_rumble_experimental_bt_fast", 1);
    AddToggleRow(bluetooth, tr("Accent each note (crisp notes)"),
                 tr("Enhances the opening burst of each rumble note over Bluetooth for punchier clicks "
                    "and impacts without consuming extra bandwidth. ") +
                     on_unsure,
                 "hd_rumble_experimental_bt_crisp_notes", 0);
    MakeCard(detail_bluetooth, detail_cards_layout, false, bluetooth);
    column->addWidget(cards);
    column->addStretch(1);
}

void HdRumbleDialog::BuildDualSense(QVBoxLayout* column) {
    AddTitle(column, tr("DualSense"), tr("Rumble through the haptic speakers in the PS5 controller."));
    dualsense_note = Label(tr("Haptics did not start: standard rumble only"), "StudioNote", nullptr);
    dualsense_note->hide();
    column->addWidget(NewOffNote());
    column->addWidget(dualsense_note);

    dualsense_section = new StudioSection;
    volume_slider = new QSlider(Qt::Horizontal);
    volume_slider->setRange(MIN_DUALSENSE_VOLUME, MAX_DUALSENSE_VOLUME);
    volume_slider->setObjectName(QStringLiteral("StudioPlain"));
    volume_slider->setFixedWidth(150);
    volume_slider->setCursor(Qt::PointingHandCursor);
    volume_box = new QSpinBox;
    volume_box->setRange(MIN_DUALSENSE_VOLUME, MAX_DUALSENSE_VOLUME);
    volume_box->setSuffix(QStringLiteral("%"));
    volume_box->setKeyboardTracking(false);
    auto* volume = new QWidget;
    volume->setObjectName(QStringLiteral("StudioPlain"));
    auto* volume_layout = new QHBoxLayout(volume);
    volume_layout->setContentsMargins(0, 0, 0, 0);
    volume_layout->setSpacing(10);
    volume_layout->addWidget(volume_slider);
    volume_layout->addWidget(volume_box);
    dualsense_section->AddRow(new StudioRow(
        tr("Volume"),
        tr("Overall strength of DualSense haptic rumble. Lower this if rumble feels too strong or your "
           "hands tire during long sessions. Recommended: 100%. If unsure, keep the recommended "
           "setting."),
        0, volume));
    column->addWidget(dualsense_section);

    auto* cards = new QWidget;
    cards->setObjectName(QStringLiteral("StudioPlain"));
    dualsense_cards_layout = new QVBoxLayout(cards);
    dualsense_cards_layout->setContentsMargins(0, 0, 0, 0);
    dualsense_cards_layout->setSpacing(0);

    auto* wired = new StudioSection(tr("Wired"), GlyphIcon(Glyph::Usb, 18, CYAN));
    AddRecommendedRow(wired, true, true);
    AddToggleRow(wired, tr("Crisp notes"),
                 tr("Adds snappy punch to short vibration pulses such as footsteps and notes on "
                    "DualSense. Recommended: On. If unsure, keep the recommended setting."),
                 "dualsense_crisp_notes", 0);
    MakeCard(dualsense_wired, dualsense_cards_layout, true, wired);

    auto* bluetooth = new StudioSection(tr("Bluetooth"), GlyphIcon(Glyph::Bluetooth, 18, CYAN));
    AddRecommendedRow(bluetooth, false, true);
    AddToggleRow(bluetooth, tr("Crisp notes"),
                 tr("Adds punch to short pulses over Bluetooth. The Bluetooth stream carries fewer "
                    "samples, so the accent comes out too strong and unnatural. Recommended: Off. "
                    "If unsure, keep the recommended setting."),
                 "dualsense_bt_crisp_notes", 0);
    MakeCard(dualsense_bluetooth, dualsense_cards_layout, false, bluetooth);
    column->addWidget(cards);

    connect(volume_slider, &QSlider::valueChanged, this, [this](int percent) {
        SetValue("dualsense_haptics_volume", QString::number(percent));
        QSignalBlocker blocker(volume_box);
        volume_box->setValue(percent);
        if (!volume_slider->isSliderDown()) {
            Save(); // while dragging, the release saves once
        }
    });
    connect(volume_slider, &QSlider::sliderReleased, this, &HdRumbleDialog::Save);
    connect(volume_box, &QSpinBox::valueChanged, this, [this](int percent) {
        SetValue("dualsense_haptics_volume", QString::number(percent));
        QSignalBlocker blocker(volume_slider);
        volume_slider->setValue(percent);
        Save();
    });

    auto* why = new StudioSection;
    auto* why_chevron = Chevron();
    auto* why_head = new StudioRow(tr("Why DualSense rumbles better"), QString(), 0, why_chevron);
    why_head->SetClickable(true);
    why->AddRow(why_head);
    std::vector<StudioRow*> why_rows;
    for (const QString& reason :
         {tr("A Pro Controller receives rumble in packets: every 15 ms over USB, 35–60 ms over "
             "Bluetooth. DualSense receives a continuous 48,000 samples/sec waveform, producing smooth "
             "transitions without stepping."),
          tr("DualSense features two large voice-coil haptic actuators with a wide 20–1000 Hz frequency "
             "response, delivering stronger and crisper feedback than the small linear resonant "
             "actuators in a Pro Controller."),
          tr("RenHDrum faithfully reconstructs the pitch and amplitude of both frequency bands sent by "
             "the game, rendering them across the left and right actuators.")}) {
        auto* row = new StudioRow(QStringLiteral("•  ") + reason, QString(), 0, nullptr);
        row->hide();
        why->AddRow(row);
        why_rows.push_back(row);
    }
    connect(why_head, &StudioRow::Clicked, this, [why_chevron, why_rows] {
        const bool open = why_rows.front()->isHidden();
        why_chevron->setText(open ? QStringLiteral("▾") : QStringLiteral("›"));
        for (auto* row : why_rows) {
            row->setVisible(open);
        }
    });
    column->addWidget(why);
    column->addStretch(1);
}

void HdRumbleDialog::BuildXbox(QVBoxLayout* column) {
    AddTitle(column, tr("Xbox One S"),
             tr("Xbox One Rumble: the game's HD Rumble, translated for the four motors of your Xbox "
                "controller."));

    // Which controller is plugged in
    auto* status = new StudioSection;
    xbox_status_row = new StudioRow(QString(), QString(), 0, nullptr, GlyphIcon(Glyph::Bluetooth, 18, CYAN));
    status->AddRow(xbox_status_row);
    column->addWidget(NewOffNote());
    column->addWidget(status);

    // Raw Input (SDL reads it once at startup, so it is not part of the renhdrum settings file)
    static const bool raw_input_at_start = Settings::values.enable_raw_input.GetValue();
    auto* step2 = new StudioSection;
    xbox_raw_toggle = new ToggleSwitch;
    xbox_raw_toggle->setChecked(Settings::values.enable_raw_input.GetValue());
    connect(xbox_raw_toggle, &QAbstractButton::toggled, this, [this](bool on) {
        Settings::values.enable_raw_input = on;
        UpdateXbox();
    });
    step2->AddRow(new StudioRow(tr("Trigger rumble (all 4 motors)"),
                                tr("Lets the two trigger motors join in. Off by default."), 0, xbox_raw_toggle));

    // Shown only while the switch differs from what this Brontide started with
    xbox_restart = new QPushButton(tr("Restart Brontide"));
    xbox_restart->setObjectName(QStringLiteral("StudioPill"));
    xbox_restart->setCursor(Qt::PointingHandCursor);
    connect(xbox_restart, &QPushButton::clicked, this, &HdRumbleDialog::RestartBrontide);
    xbox_restart->setProperty("startup", raw_input_at_start);
    xbox_restart_row = new StudioRow(tr("Restart Brontide to apply. Afterwards, map the controller again in "
                                        "Controls if needed; rumble starts after the first button press."),
                                     QString(), 0, xbox_restart);
    step2->AddRow(xbox_restart_row);
    column->addWidget(step2);

    column->addWidget(Label(tr("Built and tested on the Xbox One S controller. Other Microsoft Xbox One and "
                               "Series X|S controllers share the same four motors and should work, but are "
                               "untested."),
                            "StudioFootnote", nullptr));
    column->addStretch(1);
    UpdateXbox();
}

/// Status card and the restart row, from what is connected and the settings now.
void HdRumbleDialog::UpdateXbox() {
    if (!xbox_status_row) {
        return;
    }
    xbox_status_row->SetIcon(
        GlyphIcon(pad_state.has_xbox && pad_state.xbox_usb ? Glyph::Usb : Glyph::Bluetooth, 18, CYAN));
    xbox_status_row->SetLabel(pad_state.has_xbox
                                  ? (pad_state.xbox_usb ? tr("Xbox controller found · Wired")
                                                        : tr("Xbox controller found · Bluetooth"))
                                  : tr("No Xbox controller found. Plug it in or pair it over Bluetooth."));
    xbox_restart_row->setVisible(xbox_raw_toggle->isChecked() != xbox_restart->property("startup").toBool());
}

/// Closes Brontide the normal way (it asks first when a game is running and saves the settings);
/// only if it really closes, a new Brontide starts once this process has fully exited (on Windows a
/// hidden PowerShell waits for it), so the settings are already on disk when the new one reads them.
void HdRumbleDialog::RestartBrontide() {
    QWidget* window = this;
    while (window->parentWidget()) {
        window = window->parentWidget();
    }
    accept(); // this dialog goes away; the Controls dialog, if it opened us, stays
    QTimer::singleShot(0, window, [window] {
        if (!window->close()) {
            return; // the player said no
        }
#ifdef _WIN32
        // A hidden PowerShell waits until this process is completely gone, then starts Brontide:
        // starting the new one while the old one is still shutting down could leave it stuck with
        // no window. YUZU_IS_CHILD is inherited from us and would skip the startup checks.
        const auto quote = [](QString text) {
            return QStringLiteral("'%1'").arg(text.replace(QLatin1Char('\''), QStringLiteral("''")));
        };
        QStringList arguments;
        for (const QString& argument : QCoreApplication::arguments().mid(1)) {
            arguments << quote(argument);
        }
        QString script = QStringLiteral("Wait-Process -Id %1 -ErrorAction SilentlyContinue; "
                                        "$env:YUZU_IS_CHILD = $null; "
                                        "Start-Process -FilePath %2 -WorkingDirectory %3")
                             .arg(QCoreApplication::applicationPid())
                             .arg(quote(QCoreApplication::applicationFilePath()),
                                  quote(QCoreApplication::applicationDirPath()));
        if (!arguments.isEmpty()) {
            script += QStringLiteral(" -ArgumentList @(%1)").arg(arguments.join(QLatin1Char(',')));
        }
        const QByteArray encoded =
            QByteArray(reinterpret_cast<const char*>(script.utf16()), script.size() * 2).toBase64();
        QProcess waiter;
        waiter.setProgram(QStringLiteral("powershell.exe"));
        waiter.setArguments({QStringLiteral("-NoProfile"), QStringLiteral("-NonInteractive"),
                             QStringLiteral("-EncodedCommand"), QString::fromLatin1(encoded)});
        waiter.setCreateProcessArgumentsModifier(
            [](QProcess::CreateProcessArguments* args) { args->flags |= CREATE_NO_WINDOW; });
        waiter.startDetached();
#else
        QObject::connect(qApp, &QCoreApplication::aboutToQuit, qApp, [program = QCoreApplication::applicationFilePath(),
                                                                       arguments = QCoreApplication::arguments().mid(1)] {
            QProcess::startDetached(program, arguments);
        });
#endif
        QCoreApplication::quit(); // a dialog still open behind us would otherwise keep the app alive
    });
}

StudioRow* HdRumbleDialog::AddToggleRow(StudioSection* section, const QString& label, const QString& tip,
                                        const char* key, int indent) {
    auto* toggle = new ToggleSwitch;
    auto* row = new StudioRow(label, tip, indent, toggle);
    section->AddRow(row);
    toggles.push_back({toggle, QString::fromLatin1(key)});
    connect(toggle, &QAbstractButton::toggled, this, [this, key](bool on) {
        SetValue(key, on ? QStringLiteral("true") : QStringLiteral("false"));
        Changed();
    });
    return row;
}

void HdRumbleDialog::AddRecommendedRow(StudioSection* section, bool wired, bool dualsense) {
    auto* reset = new QPushButton(tr("Reset to recommended"));
    reset->setObjectName(QStringLiteral("StudioPill"));
    reset->setCursor(Qt::PointingHandCursor);
    const QString tip =
        dualsense
            ? (wired ? tr("Crisp notes on over USB. Recommended: On. If unsure, keep the recommended setting.")
                     : tr("Crisp notes off over Bluetooth for natural rumble. Recommended: On. If unsure, "
                          "keep the recommended setting."))
        : wired ? tr("Automatically turns on all options in Detailed settings with 15 ms interval over "
                   "USB for the most authentic experience. Recommended: On. If unsure, keep the "
                   "recommended setting.")
              : tr("Automatically selects Detailed (5 ms) and turns on all Bluetooth options in Detailed "
                   "settings for responsive rumble without button lag. Recommended: On. If unsure, "
                   "keep the recommended setting.");
    // Only shown once the settings differ from the recommended ones (SyncWidgets)
    auto* row = new StudioRow(tr("Custom settings"), tip, 0, reset);
    section->AddRow(row);
    connect(reset, &QPushButton::clicked, this, [this, wired, dualsense] { ResetConnection(wired, dualsense); });
    recommended_rows.push_back({row, wired, dualsense});
}

void HdRumbleDialog::MakeCard(ConnectionCard& card, QVBoxLayout* layout, bool wired, StudioSection* section) {
    card.section = section;
    card.wired = wired;
    auto* fold = new StudioSection;
    auto* tail = new QWidget;
    tail->setObjectName(QStringLiteral("StudioPlain"));
    auto* tail_layout = new QHBoxLayout(tail);
    tail_layout->setContentsMargins(0, 0, 0, 0);
    tail_layout->setSpacing(8);
    tail_layout->addWidget(Label(tr("Not in use"), "StudioFootnote", tail));
    tail_layout->addWidget(Chevron());
    auto* row = new StudioRow(wired ? tr("Wired settings") : tr("Bluetooth settings"), QString(), 0, tail);
    row->SetClickable(true);
    connect(row, &StudioRow::Clicked, this, [this, &card] {
        card.expanded = true;
        ArrangeCards();
    });
    fold->AddRow(row);
    card.fold = fold;
    layout->addWidget(section);
    layout->addWidget(fold);
}

// ---------------------------------------------------------------- settings

void HdRumbleDialog::Load() {
    for (const auto& [key, fallback] : SETTINGS_DEFAULTS) {
        values[QString::fromLatin1(key)] = QString::fromLatin1(fallback);
    }
    if (QFile file(SettingsPath()); file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        while (!file.atEnd()) {
            const QString line = QString::fromUtf8(file.readLine()).trimmed();
            const int eq = line.indexOf(QLatin1Char('='));
            if (line.startsWith(QLatin1Char('#')) || eq < 0) {
                continue;
            }
            if (const QString key = line.left(eq).trimmed(); values.contains(key)) {
                values[key] = line.mid(eq + 1).trimmed();
            }
        }
    }
    // What the flag files of an older version said is what the hook still obeys
    if (QFile::exists(FlagPath(OFF_FLAG))) {
        values[QStringLiteral("enable_hd_rumble")] = QStringLiteral("false");
    }
    values[QStringLiteral("hd_rumble_usb_interval_ms")] =
        QString::number(ReadFlagInt(USB_INTERVAL_FLAG, Number("hd_rumble_usb_interval_ms")));
    values[QStringLiteral("hd_rumble_bluetooth_level")] =
        QString::number(ReadFlagInt(BLUETOOTH_LEVEL_FLAG, Number("hd_rumble_bluetooth_level")));
    values[QStringLiteral("dualsense_haptics_volume")] = QString::number(
        std::clamp(Number("dualsense_haptics_volume"), MIN_DUALSENSE_VOLUME, MAX_DUALSENSE_VOLUME));
}

bool HdRumbleDialog::Save() {
    const QString path = SettingsPath();
    QDir().mkpath(QFileInfo(path).absolutePath());
    QFile file(path);
    const bool ok = !path.isEmpty() && file.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text);
    if (ok) {
        file.write("# Brontide-App settings, read by renhdrum.dll when the App is closed\n");
        for (const auto& [key, fallback] : SETTINGS_DEFAULTS) {
            const QString name = QString::fromLatin1(key);
            file.write(QStringLiteral("%1=%2\n").arg(name, values.value(name)).toUtf8());
        }
        file.close();
        // Flag files win over the settings file, so the old ones must not outlive a save
        for (const char* flag : {OFF_FLAG, BLUETOOTH_LEVEL_FLAG, USB_INTERVAL_FLAG}) {
            QFile::remove(FlagPath(flag));
        }
    }
    saved_any = saved_any || ok;
    status_dot->show();
    status_text->show();
    status_dot->setProperty("failed", !ok);
    status_dot->style()->unpolish(status_dot);
    status_dot->style()->polish(status_dot);
    status_text->setText(ok ? SavedText()
                            : tr("Could not save the HD Rumble settings."));
    return ok;
}

void HdRumbleDialog::Changed() {
    Save();
    SyncWidgets();
}

bool HdRumbleDialog::Flag(const char* key) const {
    return values.value(QString::fromLatin1(key)) == QStringLiteral("true");
}

int HdRumbleDialog::Number(const char* key) const {
    return values.value(QString::fromLatin1(key)).toInt();
}

void HdRumbleDialog::SetValue(const char* key, const QString& value) {
    values[QString::fromLatin1(key)] = value;
}

bool HdRumbleDialog::IsRecommended(bool wired, bool dualsense) const {
    const auto keys = ConnectionKeys(wired, dualsense);
    return std::all_of(keys.begin(), keys.end(), [this](const char* key) {
        return values.value(QString::fromLatin1(key)) == DefaultValue(key);
    });
}

void HdRumbleDialog::ResetConnection(bool wired, bool dualsense) {
    for (const char* key : ConnectionKeys(wired, dualsense)) {
        SetValue(key, DefaultValue(key));
    }
    Changed();
}

/// Model to widgets, without sending their signals back.
void HdRumbleDialog::SyncWidgets() {
    for (const auto& [toggle, key] : toggles) {
        QSignalBlocker blocker(toggle);
        toggle->setChecked(Flag(key.toLatin1().constData()));
    }
    bluetooth_level->SetValue(Flag("hd_rumble_experimental_bt_fast") ? 1 : 0);
    interval->SetValue(Number("hd_rumble_usb_interval_ms"));
    {
        QSignalBlocker slider_blocker(volume_slider);
        QSignalBlocker box_blocker(volume_box);
        volume_slider->setValue(Number("dualsense_haptics_volume"));
        volume_box->setValue(Number("dualsense_haptics_volume"));
    }

    // The cards depend on each other: 3 needs 1, 5 / 7 / 8 need 1 + 3, B3 needs B1
    const bool card1 = Flag("hd_rumble_experimental_usb_no_queue");
    const bool card3 = Flag("hd_rumble_experimental_envelope_5ms");
    row3->setEnabled(card1);
    for (auto* row : {row5, row7, row8}) {
        row->setEnabled(card1 && card3);
    }
    row_interval->setEnabled(!(card1 && card3));
    row_auto_slow->setEnabled(!card1);
    row_b3->setEnabled(Flag("hd_rumble_experimental_bt_after_input"));
    UpdateXbox();

    for (auto* note : off_notes) {
        note->setVisible(!Flag("enable_hd_rumble"));
    }
    for (const auto& entry : recommended_rows) {
        entry.row->setVisible(!IsRecommended(entry.wired, entry.dualsense));
    }
}

// ---------------------------------------------------------------- following the controllers

void HdRumbleDialog::GoToPage(int page) {
    stack->setCurrentIndex(page);
    nav_buttons[page == Details ? Switch : page]->setChecked(true);
}

void HdRumbleDialog::RefreshPads() {
    const auto pads = InputCommon::GetRenHDrumPads();
    QString signature;
    PadState state;
    for (const auto& pad : pads) {
        signature += QStringLiteral("%1|%2|%3|%4;")
                         .arg(QString::fromStdString(pad.name), pad.is_bluetooth ? QStringLiteral("B") : QStringLiteral("U"),
                              QString::fromStdString(pad.mode), PlayerUsing(pad));
        if (pad.is_dualsense) {
            state.has_dualsense = true;
            state.dualsense_usb |= !pad.is_bluetooth;
            state.dualsense_bluetooth |= pad.is_bluetooth;
            // Switched off on purpose is not "haptics did not start"
            state.dualsense_haptics |= pad.mode.starts_with("DualSense haptics") || pad.mode == "off";
        } else if (pad.mode.starts_with("Xbox")) {
            state.has_xbox = true;
            state.xbox_usb |= !pad.is_bluetooth;
            state.xbox_bluetooth |= pad.is_bluetooth;
        } else {
            state.has_switch = true;
            state.switch_usb |= !pad.is_bluetooth;
            state.switch_bluetooth |= pad.is_bluetooth;
        }
    }
    if (signature == pad_signature && !first_refresh) {
        return;
    }
    pad_signature = signature;
    pad_state = state;

    RebuildPadRows(pads);
    ArrangeCards();
    UpdateNav();
    UpdateXbox();
    dualsense_note->setVisible(state.has_dualsense && !state.dualsense_haptics);
    dualsense_section->SetDimmed(state.has_dualsense && !state.dualsense_haptics);

    if (first_refresh) {
        first_refresh = false;
        GoToPage(state.has_switch ? Switch
                 : state.has_dualsense ? DualSense
                 : state.has_xbox ? Xbox
                                  : Overview);
    }
}

void HdRumbleDialog::RebuildPadRows(const std::vector<InputCommon::RenHDrumPadInfo>& pads) {
    pads_section->Clear();
    if (pads.empty()) {
        pads_section->AddRow(new StudioRow(
            tr("No controller connected: plug one in to see only its settings"), QString(), 0, nullptr));
        return;
    }
    for (const auto& pad : pads) {
        const QString player = PlayerUsing(pad);
        auto* card = new PadCard(QString::fromStdString(pad.name), pad.is_bluetooth ? tr("Bluetooth") : tr("USB"),
                                 ModeText(pad), player.isEmpty() ? QString() : tr("In use · %1").arg(player));
        card->setToolTip(tr("Open this controller's settings"));
        const int page = pad.is_dualsense                ? DualSense
                         : pad.mode.starts_with("Xbox") ? Xbox
                                                        : Switch;
        connect(card, &PadCard::Clicked, this, [this, page] { GoToPage(page); });
        pads_section->AddRow(card);
    }
}

/// The player this pad is mapped to in Controls (empty when none): the mapping being edited while
/// the Configure dialog is open, the saved one otherwise.
QString HdRumbleDialog::PlayerUsing(const InputCommon::RenHDrumPadInfo& pad) const {
    if (pad.guid.empty()) {
        return {};
    }
    const QString guid = QString::fromStdString(pad.guid);
    for (std::size_t index = 0; index < 9; ++index) { // 8 players, then handheld
        const auto* controller = hid_core.GetEmulatedControllerByIndex(index);
        if (!controller || !controller->IsConnected(true)) {
            continue;
        }
        for (const auto& device : controller->GetMappedDevices()) {
            if (device.Get("engine", "") == "sdl" && QString::fromStdString(device.Get("guid", "")) == guid &&
                device.Get("port", 0) == pad.port) {
                return index < 8 ? tr("Player %1").arg(index + 1) : tr("Handheld");
            }
        }
    }
    return {};
}

/// The mode in words; the driver's own string is shorthand for the log.
QString HdRumbleDialog::ModeText(const InputCommon::RenHDrumPadInfo& pad) const {
    const QString mode = QString::fromStdString(pad.mode);
    if (mode == QStringLiteral("off") || mode == QStringLiteral("Xbox off")) {
        return tr("HD Rumble off");
    }
    if (mode.startsWith(QStringLiteral("Xbox"))) {
        return Settings::values.enable_raw_input.GetValue() ? tr("4-motor rumble") : tr("2-motor rumble");
    }
    if (pad.is_dualsense) {
        return mode.startsWith(QStringLiteral("DualSense haptics")) ? tr("Haptics") : tr("Standard rumble");
    }
    if (pad.is_bluetooth) {
        const bool detailed = Flag("hd_rumble_experimental_bt_fast");
        return tr("HD Rumble · %1").arg(detailed ? tr("Detailed (5 ms)") : tr("Compatible"));
    }
    return tr("HD Rumble");
}

/// Sidebar dot: cyan when such a controller is connected, gray when not.
void HdRumbleDialog::UpdateNav() {
    const auto mark = [this](int page, bool dim) {
        auto* button = nav_buttons[page];
        button->setProperty("dim", dim);
        button->setToolTip(dim ? tr("Not connected") : QString());
        button->style()->unpolish(button);
        button->style()->polish(button);
        auto* dot = nav_dots[page];
        dot->setProperty("on", !dim);
        dot->style()->unpolish(dot);
        dot->style()->polish(dot);
    };
    mark(Switch, !pad_state.has_switch);
    mark(Details, !pad_state.has_switch);
    mark(DualSense, !pad_state.has_dualsense);
    mark(Xbox, !pad_state.has_xbox);
}

void HdRumbleDialog::ArrangeCards() {
    const bool has_switch = pad_state.has_switch;
    const bool switch_usb = pad_state.switch_usb;
    const bool switch_bt = pad_state.switch_bluetooth;
    Arrange(switch_cards_layout, switch_bluetooth, switch_wired, false, has_switch, switch_usb, switch_bt,
            tr("Close to wired, no button lag. Detailed (5 ms) is recommended."),
            tr("The best it gets: every detail of the game's rumble."));
    Arrange(detail_cards_layout, detail_bluetooth, detail_wired, true, has_switch, switch_usb, switch_bt, QString(),
            tr("Grayed rows have no effect while a switch above them is on."));
    Arrange(dualsense_cards_layout, dualsense_bluetooth, dualsense_wired, true, pad_state.has_dualsense,
            pad_state.dualsense_usb, pad_state.dualsense_bluetooth,
            tr("3,000 samples/sec: smooth, a little less detailed than wired."),
            tr("48,000 samples/sec: the most detailed rumble."));
}

/// The connection in use goes first with an "In use" badge; the other one folds into a single row
/// (unless it was opened). With no controller of that kind connected both show, as in the App.
void HdRumbleDialog::Arrange(QVBoxLayout* layout, ConnectionCard& bluetooth, ConnectionCard& wired,
                             bool wired_first_by_default, bool has_pad, bool usb, bool bt,
                             const QString& bluetooth_note, const QString& wired_note) {
    const bool wired_first = usb != bt ? usb : wired_first_by_default;
    while (QLayoutItem* item = layout->takeAt(0)) {
        delete item; // the widgets stay, they are re-added below
    }
    for (ConnectionCard* card : {wired_first ? &wired : &bluetooth, wired_first ? &bluetooth : &wired}) {
        const bool used = card->wired ? usb : bt;
        const bool shown = !has_pad || used || card->expanded;
        card->section->setVisible(shown);
        card->fold->setVisible(!shown);
        card->section->SetBadge(has_pad && used ? tr("In use") : QString());
        card->section->SetDimmed(has_pad && !used);
        if (has_pad && !used) {
            card->section->SetFootnote(
                tr("No effect right now: controller is connected via %1.")
                    .arg(card->wired ? tr("Bluetooth") : tr("USB cable")));
        } else {
            card->section->SetFootnote(card->wired ? wired_note : bluetooth_note);
        }
        layout->addWidget(card->section);
        layout->addWidget(card->fold);
    }
}

} // namespace Brontide
