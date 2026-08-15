use pdcan_types::{BoardDefinition, FanMode, HardwareRevision, PortBitmap};

pub const REV_A: BoardDefinition = BoardDefinition {
    hardware_revision: HardwareRevision::RevA,
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
    default_fan_mode: FanMode::ThreeWire,
};

pub const CAN_NOMINAL_BITRATE: u32 = 1_000_000;
pub const CAN_DATA_BITRATE: u32 = 2_000_000;
pub const CAN_BIT_RATE_SWITCHING: bool = true;
pub const EMBASSY_TIME_DRIVER: &str = "TIM3";
pub const CONFIG_FLASH_START: u32 = 0x0803_E000;
pub const CONFIG_FLASH_LENGTH: u32 = 8 * 1024;

const _: () = assert!(REV_A.supported_ports.bits() == 0x3F);
const _: () = assert!(REV_A.mux_channel_by_port[6].is_none());
const _: () = assert!(REV_A.mux_channel_by_port[7].is_none());

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_types::PortId;

    #[test]
    fn only_ports_zero_through_five_are_supported() {
        for raw_port in 0..=7 {
            let port = PortId::new(raw_port).unwrap();
            assert_eq!(REV_A.supports(port), raw_port < 6);
            assert_eq!(REV_A.mux_channel(port), (raw_port < 6).then_some(raw_port));
        }
    }

    #[test]
    fn fixed_board_contract_matches_v1_plan() {
        assert_eq!(CAN_NOMINAL_BITRATE, 1_000_000);
        assert_eq!(CAN_DATA_BITRATE, 2_000_000);
        assert!(CAN_BIT_RATE_SWITCHING);
        assert_eq!(EMBASSY_TIME_DRIVER, "TIM3");
        assert_eq!(CONFIG_FLASH_START + CONFIG_FLASH_LENGTH, 0x0804_0000);
    }
}
