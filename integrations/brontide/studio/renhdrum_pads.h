// SPDX-FileCopyrightText: Copyright 2026 Brontide
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <string>
#include <vector>

// Kept free of SDL so the Qt UI can include it.
namespace InputCommon {

struct RenHDrumPadInfo {
    std::string name;
    bool is_dualsense;
    bool is_bluetooth;
    std::string mode;
    bool is_vibrating{false};
    std::string guid;
    int port{0};
};

/// Controllers RenHDrum is driving right now (a copy, safe from any thread).
std::vector<RenHDrumPadInfo> GetRenHDrumPads();

/// Re-reads the HD Rumble settings for every connected controller, as a reconnect would.
/// Safe from any thread: the input thread does the work on its next pass.
void ReapplyRenHDrumSettings();

} // namespace InputCommon
