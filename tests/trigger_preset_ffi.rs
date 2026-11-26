use renhdrum::{
    renhdrum_trigger_preset_create, renhdrum_trigger_preset_destroy, renhdrum_trigger_preset_set,
    renhdrum_trigger_preset_tick, DualSenseTriggerEffect, RENHDRUM_ERR_NULL_PTR,
};

#[test]
fn trigger_preset_c_abi_roundtrip_and_null_safety() {
    unsafe {
        let mut out = [0u8; 11];

        // Null pointer guards
        renhdrum_trigger_preset_destroy(std::ptr::null_mut());
        renhdrum_trigger_preset_set(std::ptr::null_mut(), 3, 1.0, 0);
        assert_eq!(
            renhdrum_trigger_preset_tick(std::ptr::null_mut(), 0, out.as_mut_ptr()),
            RENHDRUM_ERR_NULL_PTR
        );

        let h = renhdrum_trigger_preset_create();
        assert!(!h.is_null());
        assert_eq!(
            renhdrum_trigger_preset_tick(h, 0, std::ptr::null_mut()),
            RENHDRUM_ERR_NULL_PTR
        );

        // Initial state is Off -> tick returns 0 (unchanged)
        assert_eq!(renhdrum_trigger_preset_tick(h, 0, out.as_mut_ptr()), 0);

        // Set crisp shot (id = 3) -> tick returns 1 with weapon(2, 4, 5)
        renhdrum_trigger_preset_set(h, 3, 1.0, 10);
        assert_eq!(renhdrum_trigger_preset_tick(h, 10, out.as_mut_ptr()), 1);
        assert_eq!(out, DualSenseTriggerEffect::weapon("c", 2, 4, 5).data);

        // Refresh without change -> tick returns 0
        renhdrum_trigger_preset_set(h, 3, 1.0, 20);
        assert_eq!(renhdrum_trigger_preset_tick(h, 20, out.as_mut_ptr()), 0);

        // Release to Off (id = 0) -> tick returns 1 with Off bytes
        renhdrum_trigger_preset_set(h, 0, 0.0, 30);
        assert_eq!(renhdrum_trigger_preset_tick(h, 30, out.as_mut_ptr()), 1);
        assert_eq!(out, DualSenseTriggerEffect::off().data);

        renhdrum_trigger_preset_destroy(h);
    }
}
