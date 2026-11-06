// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <vector>

#include <QDialog>
#include <QMap>
#include <QString>

#include "input_common/drivers/renhdrum_pads.h"

class QButtonGroup;
class QLabel;
class QPushButton;
class QSlider;
class QSpinBox;
class QStackedWidget;
class QVBoxLayout;
class QWidget;

namespace Core::HID {
class HIDCore;
}

namespace Brontide {

class Segmented;
class StudioRow;
class StudioSection;
class ToggleSwitch;

/// "HD Rumble Studio": a window of its own that follows companion app settings pages,
/// in the gold / cyan of the HD Rumble pill so it reads as the part of Brontide that is different.
/// It reads and writes %APPDATA%\RenHDrum\renhdrum-settings.txt, the file Brontide-App writes and
/// renhdrum.dll resolves its config from, so both stay in sync and no new emulator setting is needed.
/// Every change is saved at once; the hook applies it when a controller opens. The pages follow the
/// controllers that are connected (checked once a second while the window is open).
class HdRumbleDialog : public QDialog {
    Q_OBJECT

public:
    HdRumbleDialog(QWidget* parent, Core::HID::HIDCore& hid_core);

private:
    enum Page { Overview, Switch, Details, DualSense, Xbox, PageCount };

    /// One connection's settings card on a page, and the folded row that stands in for it.
    struct ConnectionCard {
        StudioSection* section = nullptr;
        QWidget* fold = nullptr;
        bool expanded = false;
        bool wired = false;
    };

    struct ToggleBinding {
        ToggleSwitch* toggle;
        QString key;
    };

    /// "Custom settings" with a reset button, shown only when off the recommended mode
    /// (Brontide-App's recommended.rs)
    struct RecommendedRow {
        StudioRow* row;
        bool wired;
        bool dualsense;
    };

    struct PadState {
        bool has_switch = false;
        bool switch_usb = false;
        bool switch_bluetooth = false;
        bool has_dualsense = false;
        bool dualsense_usb = false;
        bool dualsense_bluetooth = false;
        bool dualsense_haptics = false;
        bool has_xbox = false;
        bool xbox_usb = false;
        bool xbox_bluetooth = false;
    };

    void BuildOverview(QVBoxLayout* column);
    void BuildSwitch(QVBoxLayout* column);
    void BuildDetails(QVBoxLayout* column);
    void BuildDualSense(QVBoxLayout* column);
    void BuildXbox(QVBoxLayout* column);
    QWidget* BuildSidebar();
    QWidget* BuildMainSwitchBar();
    QLabel* NewOffNote();

    StudioRow* AddToggleRow(StudioSection* section, const QString& label, const QString& tip,
                            const char* key, int indent);
    void AddRecommendedRow(StudioSection* section, bool wired, bool dualsense = false);
    void MakeCard(ConnectionCard& card, QVBoxLayout* layout, bool wired, StudioSection* section);

    void Load();
    bool Save();
    void Changed();
    void SyncWidgets();
    void ResetConnection(bool wired, bool dualsense);
    bool IsRecommended(bool wired, bool dualsense) const;
    bool Flag(const char* key) const;
    int Number(const char* key) const;
    void SetValue(const char* key, const QString& value);

    void RefreshPads();
    void UpdateXbox();
    void RestartBrontide();
    void RebuildPadRows(const std::vector<InputCommon::RenHDrumPadInfo>& pads);
    QString PlayerUsing(const InputCommon::RenHDrumPadInfo& pad) const;
    QString ModeText(const InputCommon::RenHDrumPadInfo& pad) const;
    void UpdateNav();
    void Arrange(QVBoxLayout* layout, ConnectionCard& bluetooth, ConnectionCard& wired,
                 bool wired_first_by_default, bool has_pad, bool usb, bool bt, const QString& bluetooth_note,
                 const QString& wired_note);
    void ArrangeCards();
    void GoToPage(int page);

    /// Every key of the settings file, so the ones this dialog doesn't show survive a save.
    QMap<QString, QString> values;

    QStackedWidget* stack = nullptr;
    QButtonGroup* nav_group = nullptr;
    std::vector<QPushButton*> nav_buttons;
    std::vector<QLabel*> nav_dots;
    /// "HD Rumble is off" lines at the top of the pages that obey the main switch
    std::vector<QLabel*> off_notes;

    std::vector<ToggleBinding> toggles;
    std::vector<RecommendedRow> recommended_rows;

    QSlider* volume_slider = nullptr;
    QSpinBox* volume_box = nullptr;
    Segmented* bluetooth_level = nullptr;
    Segmented* interval = nullptr;

    // Rows whose enabled state follows the cards they depend on
    StudioRow* row3 = nullptr;
    StudioRow* row5 = nullptr;
    StudioRow* row7 = nullptr;
    StudioRow* row8 = nullptr;
    StudioRow* row_interval = nullptr;
    StudioRow* row_auto_slow = nullptr;
    StudioRow* row_b3 = nullptr;

    QVBoxLayout* switch_cards_layout = nullptr;
    QVBoxLayout* detail_cards_layout = nullptr;
    ConnectionCard switch_bluetooth;
    ConnectionCard switch_wired;
    ConnectionCard detail_bluetooth;
    ConnectionCard detail_wired;
    QVBoxLayout* dualsense_cards_layout = nullptr;
    ConnectionCard dualsense_bluetooth;
    ConnectionCard dualsense_wired;

    StudioSection* pads_section = nullptr;
    Core::HID::HIDCore& hid_core;
    StudioSection* dualsense_section = nullptr;
    QLabel* dualsense_note = nullptr;

    // Xbox One S page
    StudioRow* xbox_status_row = nullptr;
    ToggleSwitch* xbox_raw_toggle = nullptr;
    QPushButton* xbox_restart = nullptr;
    StudioRow* xbox_restart_row = nullptr;

    QLabel* status_dot = nullptr;
    QLabel* status_text = nullptr;
    bool saved_any = false; ///< Something was saved: re-apply to the connected controllers on close

    PadState pad_state;
    QString pad_signature;
    bool first_refresh = true;
};

} // namespace Brontide
