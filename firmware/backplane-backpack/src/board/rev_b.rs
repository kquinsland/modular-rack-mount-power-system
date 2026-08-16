use pdcan_types::{BoardDefinition, FanMode, HardwareRevision, PortBitmap};

/// Prototype of the first backplane revision with per-slot input-power gates.
/// The PCA9554 has room for all eight logical ports; this PCB populates six.
pub const REV_B: BoardDefinition = BoardDefinition {
    hardware_revision: HardwareRevision::RevB,
    supported_ports: PortBitmap::from_bits(0x3F),
    mux_channel_by_port: [
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(4),
        Some(5),
        None,
        None,
    ],
    power_gate_bit_by_port: [
        Some(0),
        Some(1),
        Some(2),
        Some(3),
        Some(4),
        Some(5),
        None,
        None,
    ],
    default_fan_mode: FanMode::ThreeWire,
};

pub const CAN_NOMINAL_BITRATE: u32 = 1_000_000;
pub const CAN_DATA_BITRATE: u32 = 2_000_000;
pub const CAN_BIT_RATE_SWITCHING: bool = true;
pub const EMBASSY_TIME_DRIVER: &str = "TIM3";
pub const CONFIG_FLASH_START: u32 = 0x0803_E000;
pub const CONFIG_FLASH_LENGTH: u32 = 8 * 1024;
pub const MUX_RESET_AVAILABLE: bool = false;

/// Conservative placeholder until rail rise and SW3538 reset release can be
/// measured on Rev B hardware.
pub const POWER_GATE_SETTLE_MS_PROVISIONAL: u64 = 10;
pub const POWER_EXPANDER_ADDRESS: u8 = 0x20;

const _: () = assert!(REV_B.supported_ports.bits() == 0x3F);
const _: () = assert!(REV_B.power_gate_mask() == 0x3F);
const _: () = assert!(REV_B.power_gate_bit_by_port[6].is_none());
const _: () = assert!(REV_B.power_gate_bit_by_port[7].is_none());

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_types::PortId;

    #[test]
    fn six_prototype_slots_map_directly_to_p0_through_p5() {
        for raw_port in 0..=7 {
            let port = PortId::new(raw_port).unwrap();
            assert_eq!(REV_B.supports(port), raw_port < 6);
            assert_eq!(REV_B.mux_channel(port), (raw_port < 6).then_some(raw_port));
            assert_eq!(
                REV_B.power_gate_bit(port),
                (raw_port < 6).then_some(raw_port)
            );
        }
    }

    #[test]
    fn fixed_board_contract_matches_prototype_schematic() {
        assert_eq!(REV_B.power_gate_mask(), 0x3F);
        assert!(!MUX_RESET_AVAILABLE);
        assert_eq!(POWER_EXPANDER_ADDRESS, 0x20);
        assert_eq!(
            POWER_EXPANDER_ADDRESS,
            pdcan_drivers::pca9554::DEFAULT_ADDRESS
        );
        assert_ne!(
            POWER_EXPANDER_ADDRESS,
            pdcan_drivers::tca9548a::DEFAULT_ADDRESS
        );
        assert_eq!(POWER_GATE_SETTLE_MS_PROVISIONAL, 10);
        assert_eq!(CONFIG_FLASH_START + CONFIG_FLASH_LENGTH, 0x0804_0000);
    }
}
