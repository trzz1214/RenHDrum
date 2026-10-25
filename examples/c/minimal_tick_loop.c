#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include "renhdrum.h"

int main(void) {
    printf("RenHDrum C Integration Example (Version 0x%06X)\n", renhdrum_version());

    // 1. Create engine for Nintendo Switch Pro Controller over USB
    RenHdcEngine *engine = renhdrum_engine_create(false, true);
    if (!engine) {
        fprintf(stderr, "Failed to create RenHDrum engine\n");
        return 1;
    }

    // 2. Initialize default recommended settings
    RenHdcSettings settings;
    if (renhdrum_settings_default(&settings) != RENHDRUM_OK) {
        settings.enabled = true;
        settings.usb_interval_ms = 15;
        settings.bluetooth_level = 1;
        settings.usb_auto_slow = true;
        settings.sync_to_input = false;
        settings.click_pulse = false;
        settings.sync_bluetooth = false;
        settings.usb_no_queue = false;
        settings.usb_pause_ms = 0;
        settings.envelope_5ms = true;
        settings.frequency_in_packet = true;
        settings.frequency_both_bands = true;
        settings.bt_after_input = false;
        settings.crisp_notes = true;
        settings.bt_crisp_notes = false;
        settings.bt_fast = false;
    }
    renhdrum_engine_apply_settings(engine, &settings);

    // 3. Push a test sample (Left: 160 Hz / 320 Hz @ 0.5 amp; Right: same)
    int64_t now_us = 1000;
    renhdrum_engine_push_sample(
        engine,
        now_us,
        0.5f, 160.0f, 0.5f, 320.0f,
        0.5f, 160.0f, 0.5f, 320.0f
    );

    // 4. Tick simulation loop at 1 ms steps until a 10-byte 0x10 packet is emitted
    uint8_t packet10[10] = {0};
    bool packet_emitted = false;

    for (int ms = 1; ms <= 30; ++ms) {
        now_us += 1000;
        int32_t status = renhdrum_engine_tick(engine, now_us, packet10);
        if (status == 1) {
            printf("Emitted Subcommand 0x10 packet at t=%d ms:\n  ", ms);
            for (int i = 0; i < 10; ++i) {
                printf("%02X ", packet10[i]);
            }
            printf("\n");
            packet_emitted = true;
            break;
        }
    }

    // 5. Cleanup: neutral stop packet and destroy engine
    const uint8_t stop_packet[10] = { 0x10, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40 };
    (void)stop_packet; // Available for host to dispatch to HID transport
    renhdrum_engine_destroy(engine);

    if (packet_emitted) {
        printf("RenHDrum tick loop completed successfully.\n");
        return 0;
    } else {
        fprintf(stderr, "No packet emitted within 30 ms window.\n");
        return 1;
    }
}
