#![no_std]

use pdcan_drivers::ina237::{AdcRange, Config as Ina237Config};
use pdcan_types::{CapabilityFlags, HardwareRevision, NodeDescriptor, NodeRole, UpdateImpact};

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

    pub const FAN_1_PWM: Pin = Pin::new(GpioPort::A, 2);
    pub const FAN_1_TACH: Pin = Pin::new(GpioPort::B, 8);
    pub const FAN_2_PWM: Pin = Pin::new(GpioPort::A, 0);
    pub const FAN_2_TACH: Pin = Pin::new(GpioPort::A, 8);
    pub const LED_DATA: Pin = Pin::new(GpioPort::A, 1);
    pub const TWELVE_VOLT_POWER_GOOD: Pin = Pin::new(GpioPort::A, 3);
    pub const INA_ALERT: Pin = Pin::new(GpioPort::A, 4);
    pub const I2C_SDA: Pin = Pin::new(GpioPort::A, 6);
    pub const I2C_SCL: Pin = Pin::new(GpioPort::A, 7);
    pub const CAN_RX: Pin = Pin::new(GpioPort::B, 0);
    pub const CAN_TX: Pin = Pin::new(GpioPort::B, 1);
    pub const DS18B20_DATA: Pin = Pin::new(GpioPort::A, 15);
}

pub const CAPABILITIES: CapabilityFlags = CapabilityFlags::FIRMWARE_UPDATE
    .union(CapabilityFlags::FAN_CONTROL)
    .union(CapabilityFlags::FAN_TACHOMETER)
    .union(CapabilityFlags::INTERNAL_TEMPERATURE)
    .union(CapabilityFlags::EXTERNAL_TEMPERATURE)
    .union(CapabilityFlags::AGGREGATE_POWER);

pub const DESCRIPTOR: NodeDescriptor = NodeDescriptor {
    role: NodeRole::Backplane,
    carrier_profile: None,
    hardware_revision: HardwareRevision::REV_A,
    capabilities: CAPABILITIES,
    update_impact: UpdateImpact::Interrupt,
    output_count: 0,
    fan_count: 2,
    external_temperature_capacity: 0xFF,
};

/// The 1 mOhm aggregate shunt uses the INA237 narrow range. A 1 mA current LSB
/// covers the intended 30 A working range while retaining useful resolution.
pub const INA237_CONFIG: Ina237Config = Ina237Config {
    adc_range: AdcRange::Narrow,
    adc_config: 0xFB68,
    shunt_microohms: 1_000,
    current_lsb_microamps: 1_000,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalized_descriptor_has_two_fans_and_dynamic_one_wire_capacity() {
        assert_eq!(DESCRIPTOR.validate(), Ok(DESCRIPTOR));
        assert_eq!(DESCRIPTOR.fan_count, 2);
        assert_eq!(DESCRIPTOR.external_temperature_capacity, 0xFF);
        assert!(CAPABILITIES.contains(CapabilityFlags::FIRMWARE_UPDATE));
    }

    #[test]
    fn pin_contract_matches_the_newest_production_netlist() {
        assert_eq!(pins::CAN_RX, Pin::new(GpioPort::B, 0));
        assert_eq!(pins::CAN_TX, Pin::new(GpioPort::B, 1));
        assert_eq!(pins::LED_DATA, Pin::new(GpioPort::A, 1));
        assert_eq!(pins::FAN_1_PWM, Pin::new(GpioPort::A, 2));
        assert_eq!(pins::FAN_1_TACH, Pin::new(GpioPort::B, 8));
        assert_eq!(pins::FAN_2_PWM, Pin::new(GpioPort::A, 0));
        assert_eq!(pins::FAN_2_TACH, Pin::new(GpioPort::A, 8));
        assert_eq!(pins::DS18B20_DATA, Pin::new(GpioPort::A, 15));
    }

    #[test]
    fn aggregate_ina237_configuration_matches_the_one_milliohm_shunt() {
        assert_eq!(INA237_CONFIG.shunt_calibration(), Ok(819));
        assert_eq!(INA237_CONFIG.adc_range, AdcRange::Narrow);
    }
}
