#include "renhdrum.h"

int main(void) {
    RenHdcDeviceInfo info;
    RenHdSideSample dl, dr;
    info.struct_size = (uint32_t)sizeof(info);
    if (!RENHDRUM_ABI_COMPATIBLE(renhdrum_version())) return 2;
    if (renhdrum_device_classify(0x057E, 0x2073, &info) != RENHDRUM_CLAIM_UNSUPPORTED_DEVICE) return 3;
    if (renhdrum_claim_status_text(RENHDRUM_CLAIM_NO_HD_WRITE) == 0) return 4;
    if (renhdrum_demo_sample(RENHDRUM_DEMO_TECH_SHOW, 75u, &dl, &dr) != 1) return 5;
    return renhdrum_version() == 0 ? 1 : 0;
}
