// SPDX-License-Identifier: GPL-3.0-or-later
// RenHDrum HD Rumble settings page for Qt Widgets hosts (emulators, launchers).
// Only Qt + renhdrum.h: no emulator code. Layout, names and defaults: docs/integration/ui-guidelines.md.
// The rows come from the core table (renhdrum_options), so a new core can add a row without a host
// rebuild. Colours: only palette(...), the host theme decides.

#pragma once

#include <functional>
#include <vector>

#include <QPointer>
#include <QString>
#include <QWidget>

#include <renhdrum.h>

class QEvent;
class QFrame;
class QLabel;
class QPushButton;
class QTimer;
class QVBoxLayout;

namespace RenHDrumQt {

/// The core entry points the page needs. A host that links renhdrum directly fills it with the
/// functions themselves; one that loads the DLL at run time fills it from the symbols.
struct Api {
    decltype(&renhdrum_options) options{};
    decltype(&renhdrum_store_create) store_create{};
    decltype(&renhdrum_store_destroy) store_destroy{};
    decltype(&renhdrum_store_get) store_get{};
    decltype(&renhdrum_store_set) store_set{};
    decltype(&renhdrum_store_reset) store_reset{};
    decltype(&renhdrum_store_is_recommended) store_is_recommended{};
    bool Complete() const {
        return options && store_create && store_destroy && store_get && store_set && store_reset &&
               store_is_recommended;
    }
};

/// The controller in use right now. `device` is one bit of RenHdcOption::devices (1 Switch,
/// 2 DualSense, 4 Xbox, 8 Steam); 0 = none connected.
struct ActivePad {
    unsigned device{};
    bool bluetooth{};
    QString name;
    bool operator==(const ActivePad&) const = default;
};

struct Host {
    Api api;
    /// Polled about once a second; the page redraws when the answer changes.
    std::function<ActivePad()> active_pad;
    /// A saved value for `key`; false when none was saved (the recommended default stays).
    std::function<bool(const char* key, int* value)> load;
    /// Called on every change, with the clamped value.
    std::function<void(const char* key, int value)> save;
    /// "Test rumble" button; leave empty to hide the button.
    std::function<void()> test_rumble;
    /// Optional extra row for the Xbox card (host settings the core table does not hold, e.g. a
    /// switch that hands the trigger motors to the app). Return nullptr for nothing.
    std::function<QWidget*(QWidget* parent)> xbox_extra_row;
    /// "Report a problem" button; leave empty to hide it.
    std::function<void()> report_problem;
    /// Text after "Powered by RenHDrum", e.g. the core version; may be empty.
    std::function<QString()> credit_suffix;
    /// Called after the master switch changes (the host may need to refresh its own widgets).
    std::function<void(bool on)> master_changed;
};

class SettingsWidget : public QWidget {
    Q_OBJECT

public:
    explicit SettingsWidget(Host host, QWidget* parent = nullptr);
    ~SettingsWidget() override;

    /// Value of `key` in the page's store (the recommended default when unknown).
    int Value(const char* key) const;

protected:
    void changeEvent(QEvent* event) override;

private:
    struct Row;
    void Rebuild();
    void Refresh();
    void ApplyThemeColors();
    QLabel* MakeMuted(const QString& text, bool body_item, QWidget* parent = nullptr);
    QFrame* MakeDivider(bool body_item, QWidget* parent = nullptr);
    void SetValue(int index, int value);
    int ValueAt(int index) const;
    QWidget* MakeRow(int index, QWidget* parent);
    QWidget* MakeCard(const QString& title, unsigned device, bool only_matching_link,
                      const ActivePad& pad);
    QWidget* MakeAdvanced(const ActivePad& pad);
    QWidget* MakeCollapsible(const QString& title, QWidget* body, bool* open_state);
    bool RowVisibleOnLink(int index, const ActivePad& pad) const;

    Host host;
    RenHdcStore* store{};
    const RenHdcOption* options{};
    int option_count{};
    int stride{};
    ActivePad pad;

    QVBoxLayout* body_layout{};
    QWidget* body{};
    QPushButton* test_button{};
    QLabel* recommended_label{};
    QPushButton* reset_button{};
    QTimer* poll{};
    std::vector<Row*> rows;
    bool blocked{};
    bool others_open{};
    bool advanced_open{};
    QWidget* master_box{};
    QPointer<QLabel> logo_label;
    QPointer<QLabel> credit_label;
    QString credit_suffix_text;
    QPointer<QLabel> active_pill_label;
    QString active_pill_text;
    std::vector<QPointer<QLabel>> static_muted_labels;
    std::vector<QPointer<QLabel>> body_muted_labels;
    std::vector<QPointer<QFrame>> static_dividers;
    std::vector<QPointer<QFrame>> body_dividers;
};

} // namespace RenHDrumQt
