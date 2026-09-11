#![no_std]

use pdcan_drivers::ina237::{AdcRange, Config as Ina237Config};
use pdcan_types::{
    CapabilityFlags, CarrierProfile, HardwareRevision, NodeDescriptor, NodeRole, PdLimits,
    UpdateImpact,
};

pub const MCU: &str = "STM32C092GCU6";
pub const CAN_NOMINAL_BITRATE: u32 = 500_000;
pub const CAN_DATA_BITRATE: u32 = 2_000_000;
pub const PARTITION_LAYOUT_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpioPort {
    A,
    B,
    C,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pin {
    pub port: GpioPort,
    pub number: u8,
}

impl Pin {
    pub const fn new(port: GpioPort, number: u8) -> Self {
        Self { port, number }
    }
}

pub mod pins {
    use super::{GpioPort, Pin};

    pub const USER_BUTTON: Pin = Pin::new(GpioPort::C, 15);
    pub const CAN_RX: Pin = Pin::new(GpioPort::B, 0);
    pub const CAN_TX: Pin = Pin::new(GpioPort::B, 1);
    pub const BUCK_POWER_GOOD: Pin = Pin::new(GpioPort::A, 8);
    pub const PD_IRQ: Pin = Pin::new(GpioPort::C, 6);
    pub const PD_POWER_GOOD: Pin = Pin::new(GpioPort::A, 15);
    pub const PD_ENABLE: Pin = Pin::new(GpioPort::B, 3);
    pub const INA_ALERT: Pin = Pin::new(GpioPort::B, 5);
    pub const I2C_SCL: Pin = Pin::new(GpioPort::B, 6);
    pub const I2C_SDA: Pin = Pin::new(GpioPort::B, 7);
    pub const LED_DATA: Pin = Pin::new(GpioPort::B, 8);
}

pub const CAPABILITIES: CapabilityFlags = CapabilityFlags::FIRMWARE_UPDATE
    .union(CapabilityFlags::SWITCHABLE_OUTPUT)
    .union(CapabilityFlags::LOAD_MONITORING)
    .union(CapabilityFlags::USB_PD_CONTROL)
    .union(CapabilityFlags::USB_PD_STATUS)
    .union(CapabilityFlags::INTERNAL_TEMPERATURE)
    .union(CapabilityFlags::USER_BUTTON);

pub const DESCRIPTOR: NodeDescriptor = NodeDescriptor {
    role: NodeRole::Carrier,
    carrier_profile: Some(CarrierProfile::Sw3538),
    hardware_revision: HardwareRevision::REV_A,
    capabilities: CAPABILITIES,
    update_impact: UpdateImpact::Interrupt,
    output_count: 1,
    fan_count: 0,
    external_temperature_capacity: 0,
};

pub const PD_LIMITS: PdLimits = PdLimits::SW3538_SAFE_MAX;

/// The 6 mOhm carrier shunt requires the INA237 wide range. A 500 uA current
/// LSB covers the hardware current-limit region with ample margin.
pub const INA237_CONFIG: Ina237Config = Ina237Config {
    adc_range: AdcRange::Wide,
    adc_config: 0xFB68,
    shunt_microohms: 6_000,
    current_lsb_microamps: 500,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_carrier_is_explicitly_interrupt_update_capable() {
        assert_eq!(DESCRIPTOR.validate(), Ok(DESCRIPTOR));
        assert_eq!(DESCRIPTOR.carrier_profile, Some(CarrierProfile::Sw3538));
        assert_eq!(DESCRIPTOR.update_impact, UpdateImpact::Interrupt);
        assert!(CAPABILITIES.contains(CapabilityFlags::FIRMWARE_UPDATE));
    }

    #[test]
    fn finalized_pin_contract_has_only_local_i2c_and_direct_can() {
        assert_eq!(pins::CAN_RX, Pin::new(GpioPort::B, 0));
        assert_eq!(pins::CAN_TX, Pin::new(GpioPort::B, 1));
        assert_eq!(pins::I2C_SCL, Pin::new(GpioPort::B, 6));
        assert_eq!(pins::I2C_SDA, Pin::new(GpioPort::B, 7));
        assert_eq!(pins::PD_ENABLE, Pin::new(GpioPort::B, 3));
        assert_eq!(pins::LED_DATA, Pin::new(GpioPort::B, 8));
    }

    #[test]
    fn carrier_ina237_configuration_matches_the_six_milliohm_shunt() {
        assert_eq!(INA237_CONFIG.shunt_calibration(), Ok(2_458));
        assert_eq!(INA237_CONFIG.adc_range, AdcRange::Wide);
    }
}
