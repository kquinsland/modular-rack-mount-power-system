use embedded_hal_async::i2c::I2c;
use pdcan_types::{CarrierPolicy, InvalidPdLimits, PdLimits};

/// Register addresses confirmed by SW3538 register-list revision `RG108_3_v1.2`.
pub mod register {
    pub const CHIP_VERSION: u16 = 0x0000;
    pub const SYSTEM_MAX_POWER: u16 = 0x0002;
    pub const FAST_CHARGE_STATUS: u16 = 0x0009;
    pub const SYSTEM_STATUS: u16 = 0x000A;
    pub const ONLINE_STATUS: u16 = 0x000D;
    pub const I2C_ENABLE_AND_BANK: u16 = 0x0010;
    pub const PORT_CONTROL: u16 = 0x0011;
    pub const FORCE_ENABLE: u16 = 0x0015;
    pub const FORCE_CONTROL: u16 = 0x0016;
    pub const APPLY_FORCED_CURRENT: u16 = 0x0017;
    pub const FORCED_CURRENT_LOW: u16 = 0x0038;
    pub const FORCED_CURRENT_HIGH: u16 = 0x0039;
    pub const ADC_SELECT: u16 = 0x0040;
    pub const ADC_DATA_LOW: u16 = 0x0041;
    pub const ADC_DATA_HIGH: u16 = 0x0042;
    pub const ADC_NTC_CONFIG: u16 = 0x0044;
    pub const PD_COMMAND: u16 = 0x00A7;
    pub const RETURN_TO_BASE_BANK: u16 = 0x0180;
    pub const PD_CONTROL: u16 = 0x0122;
    pub const PD_VOLTAGE_ENABLE: u16 = 0x0124;
    pub const PD_FIXED_CURRENT_CONTROL: u16 = 0x0125;
    pub const PD_5V_CURRENT_HIGH: u16 = 0x0126;
    pub const PD_9V_CURRENT_HIGH: u16 = 0x0127;
    pub const PD_12V_CURRENT_HIGH: u16 = 0x0128;
    pub const PD_15V_CURRENT_HIGH: u16 = 0x0129;
    pub const PD_20V_CURRENT_HIGH: u16 = 0x012A;
    pub const PD_FIXED_CURRENT_LOW: u16 = 0x012B;
}

pub const WRITE_ENABLE_SEQUENCE: [u8; 3] = [0x20, 0x40, 0x80];
pub const DEFAULT_ADDRESS: u8 = 0x3C;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterWrite {
    pub register: u16,
    pub value: u8,
}

pub const WRITE_ENABLE_WRITES: [RegisterWrite; 3] = [
    RegisterWrite {
        register: register::I2C_ENABLE_AND_BANK,
        value: WRITE_ENABLE_SEQUENCE[0],
    },
    RegisterWrite {
        register: register::I2C_ENABLE_AND_BANK,
        value: WRITE_ENABLE_SEQUENCE[1],
    },
    RegisterWrite {
        register: register::I2C_ENABLE_AND_BANK,
        value: WRITE_ENABLE_SEQUENCE[2],
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Bank {
    Base,
    Extended,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    Bus(E),
    BankStateUnknown,
    UnsupportedRegister(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity {
    pub chip_version: u8,
    pub system_max_power_w: u8,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    pub buck_enabled: bool,
    pub port_1_enabled: bool,
    pub port_2_enabled: bool,
    pub port_1_online: bool,
    pub port_2_online: bool,
    pub pd_revision: u8,
    pub protocol: u8,
    pub fast_charge: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AdcChannel {
    Port1Current = 1,
    Port2Current = 2,
    OutputVoltage = 5,
    InputVoltage = 6,
    NtcVoltage = 7,
    ConvertedOutputVoltage = 11,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdcUnit {
    Microamps,
    Microvolts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdcSample {
    pub raw: u16,
    pub units_per_bit: u16,
    pub unit: AdcUnit,
}

impl AdcSample {
    pub const fn value(self) -> u32 {
        self.raw as u32 * self.units_per_bit as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForcedCurrentEncodingError {
    BelowHardwareMinimum,
    AboveV1Maximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForcedCurrentLimit {
    pub register_value: u8,
    pub effective_ma: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedPdoPlan {
    pub pd_enabled: bool,
    pub enabled_voltage_bits: u8,
    pub current_5v_10ma: u16,
    pub current_9v_10ma: u16,
    pub current_12v_10ma: u16,
    pub current_15v_10ma: u16,
    pub current_20v_10ma: u16,
}

impl FixedPdoPlan {
    pub fn from_policy(policy: CarrierPolicy) -> Result<Self, PolicyError> {
        let limits = policy.pd_limits.ok_or(PolicyError::PdLimitsRequired)?;
        limits
            .validate_for(PdLimits::SW3538_SAFE_MAX)
            .map_err(PolicyError::Limits)?;
        if !policy.enabled {
            return Ok(Self {
                pd_enabled: false,
                enabled_voltage_bits: 0,
                current_5v_10ma: 0,
                current_9v_10ma: 0,
                current_12v_10ma: 0,
                current_15v_10ma: 0,
                current_20v_10ma: 0,
            });
        }

        let current_for = |voltage_mv: u32| {
            let power_limited_ma = limits.max_power_mw.saturating_mul(1_000) / voltage_mv;
            let current_ma = limits.max_current_ma.min(power_limited_ma);
            u16::try_from(current_ma / 10)
                .unwrap_or(u16::MAX)
                .min(0x03FF)
        };
        let enabled = |voltage_mv| limits.max_voltage_mv >= voltage_mv;

        Ok(Self {
            pd_enabled: true,
            enabled_voltage_bits: u8::from(enabled(9_000))
                | (u8::from(enabled(12_000)) << 1)
                | (u8::from(enabled(15_000)) << 2)
                | (u8::from(enabled(20_000)) << 3),
            current_5v_10ma: current_for(5_000),
            current_9v_10ma: if enabled(9_000) {
                current_for(9_000)
            } else {
                0
            },
            current_12v_10ma: if enabled(12_000) {
                current_for(12_000)
            } else {
                0
            },
            current_15v_10ma: if enabled(15_000) {
                current_for(15_000)
            } else {
                0
            },
            current_20v_10ma: if enabled(20_000) {
                current_for(20_000)
            } else {
                0
            },
        })
    }

    pub const fn fixed_current_low(self) -> u8 {
        ((self.current_20v_10ma.to_le_bytes()[0] & 0x03) << 6)
            | ((self.current_15v_10ma.to_le_bytes()[0] & 0x03) << 4)
            | ((self.current_12v_10ma.to_le_bytes()[0] & 0x03) << 2)
            | (self.current_9v_10ma.to_le_bytes()[0] & 0x03)
    }
}

pub const fn encode_forced_current_limit(
    requested_ma: u16,
) -> Result<ForcedCurrentLimit, ForcedCurrentEncodingError> {
    if requested_ma < 1_000 {
        return Err(ForcedCurrentEncodingError::BelowHardwareMinimum);
    }
    if requested_ma > 5_000 {
        return Err(ForcedCurrentEncodingError::AboveV1Maximum);
    }

    let steps = (requested_ma - 1_000) / 50;
    Ok(ForcedCurrentLimit {
        register_value: steps.to_le_bytes()[0],
        effective_ma: 1_000 + steps * 50,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyError {
    PdLimitsRequired,
    Limits(InvalidPdLimits),
}

pub fn validate_v1_policy(policy: CarrierPolicy) -> Result<CarrierPolicy, PolicyError> {
    let limits = policy.pd_limits.ok_or(PolicyError::PdLimitsRequired)?;
    limits
        .validate_for(PdLimits::SW3538_SAFE_MAX)
        .map(|_| policy)
        .map_err(PolicyError::Limits)
}

/// Async local-I2C register driver. Construct it only when external reasoning
/// says bank 0 is active. Any I2C failure makes the tracked bank unknown; the
/// bus owner may call [`Self::assume_base_bank`] only after separately
/// establishing that the assumption is safe.
pub struct Sw3538 {
    address: u8,
    bank: Bank,
}

impl Sw3538 {
    pub const fn assuming_base_bank(address: u8) -> Self {
        Self {
            address,
            bank: Bank::Base,
        }
    }

    pub const fn address(&self) -> u8 {
        self.address
    }

    pub fn assume_base_bank(&mut self) {
        self.bank = Bank::Base;
    }

    pub async fn read_identity<B: I2c>(
        &mut self,
        bus: &mut B,
    ) -> Result<Identity, Error<B::Error>> {
        let chip_version = self.read_register(bus, register::CHIP_VERSION).await? & 0x03;
        let system_max_power_w = self.read_register(bus, register::SYSTEM_MAX_POWER).await? & 0x7F;
        Ok(Identity {
            chip_version,
            system_max_power_w,
        })
    }

    pub async fn read_status<B: I2c>(&mut self, bus: &mut B) -> Result<Status, Error<B::Error>> {
        let system = self.read_register(bus, register::SYSTEM_STATUS).await?;
        let online = self.read_register(bus, register::ONLINE_STATUS).await?;
        let fast_charge = self
            .read_register(bus, register::FAST_CHARGE_STATUS)
            .await?;
        Ok(Status {
            buck_enabled: system & (1 << 2) != 0,
            port_1_enabled: system & 1 != 0,
            port_2_enabled: system & (1 << 1) != 0,
            port_1_online: online & (1 << 1) != 0,
            port_2_online: online & 1 != 0,
            pd_revision: (fast_charge >> 4) & 0x03,
            protocol: fast_charge & 0x0F,
            fast_charge: fast_charge & 0xC0 != 0,
        })
    }

    pub async fn read_adc<B: I2c>(
        &mut self,
        bus: &mut B,
        channel: AdcChannel,
    ) -> Result<AdcSample, Error<B::Error>> {
        self.write_protected(bus, register::ADC_SELECT, channel as u8)
            .await?;
        let low = self.read_register(bus, register::ADC_DATA_LOW).await?;
        let high = self.read_register(bus, register::ADC_DATA_HIGH).await?;
        let high_mask = if channel == AdcChannel::ConvertedOutputVoltage {
            0x7F
        } else {
            0x0F
        };
        let raw = u16::from(low) | (u16::from(high & high_mask) << 8);
        let (units_per_bit, unit) = match channel {
            AdcChannel::Port1Current | AdcChannel::Port2Current => (2_500, AdcUnit::Microamps),
            AdcChannel::OutputVoltage => (6_000, AdcUnit::Microvolts),
            AdcChannel::InputVoltage => (10_000, AdcUnit::Microvolts),
            AdcChannel::NtcVoltage => (1_200, AdcUnit::Microvolts),
            AdcChannel::ConvertedOutputVoltage => (1_000, AdcUnit::Microvolts),
        };
        Ok(AdcSample {
            raw,
            units_per_bit,
            unit,
        })
    }

    /// Programs the registry-defined fixed-PDO fields conservatively. This sequence is
    /// compile/fake tested but remains provisional until the one-port hardware spike.
    pub async fn apply_fixed_pdo_plan<B: I2c>(
        &mut self,
        bus: &mut B,
        plan: FixedPdoPlan,
    ) -> Result<(), Error<B::Error>> {
        self.modify_protected(bus, register::PD_CONTROL, 0x01, 0)
            .await?;
        self.write_protected(bus, register::PD_VOLTAGE_ENABLE, plan.enabled_voltage_bits)
            .await?;

        let current_control = 0x20 | (plan.current_5v_10ma.to_le_bytes()[0] & 0x03);
        self.modify_protected(
            bus,
            register::PD_FIXED_CURRENT_CONTROL,
            0xF3,
            current_control,
        )
        .await?;
        self.write_protected(
            bus,
            register::PD_5V_CURRENT_HIGH,
            (plan.current_5v_10ma >> 2).to_le_bytes()[0],
        )
        .await?;
        self.write_protected(
            bus,
            register::PD_9V_CURRENT_HIGH,
            (plan.current_9v_10ma >> 2).to_le_bytes()[0],
        )
        .await?;
        self.write_protected(
            bus,
            register::PD_12V_CURRENT_HIGH,
            (plan.current_12v_10ma >> 2).to_le_bytes()[0],
        )
        .await?;
        self.write_protected(
            bus,
            register::PD_15V_CURRENT_HIGH,
            (plan.current_15v_10ma >> 2).to_le_bytes()[0],
        )
        .await?;
        self.write_protected(
            bus,
            register::PD_20V_CURRENT_HIGH,
            (plan.current_20v_10ma >> 2).to_le_bytes()[0],
        )
        .await?;
        self.write_protected(
            bus,
            register::PD_FIXED_CURRENT_LOW,
            plan.fixed_current_low(),
        )
        .await?;
        self.modify_protected(bus, register::PD_CONTROL, 0x01, u8::from(plan.pd_enabled))
            .await
    }

    pub async fn set_cc_undriven<B: I2c>(
        &mut self,
        bus: &mut B,
        undriven: bool,
    ) -> Result<(), Error<B::Error>> {
        self.modify_protected(bus, register::PORT_CONTROL, 0x02, u8::from(undriven) << 1)
            .await
    }

    pub async fn send_source_capabilities<B: I2c>(
        &mut self,
        bus: &mut B,
    ) -> Result<(), Error<B::Error>> {
        self.write_protected(bus, register::PD_COMMAND, 10).await
    }

    async fn modify_protected<B: I2c>(
        &mut self,
        bus: &mut B,
        register: u16,
        mask: u8,
        value: u8,
    ) -> Result<(), Error<B::Error>> {
        let current = self.read_register(bus, register).await?;
        self.write_protected(bus, register, (current & !mask) | (value & mask))
            .await
    }

    async fn read_register<B: I2c>(
        &mut self,
        bus: &mut B,
        register: u16,
    ) -> Result<u8, Error<B::Error>> {
        self.select_bank(bus, register).await?;
        let mut value = [0];
        if let Err(error) = bus
            .write_read(self.address, &[register.to_le_bytes()[0]], &mut value)
            .await
        {
            self.bank = Bank::Unknown;
            return Err(Error::Bus(error));
        }
        Ok(value[0])
    }

    async fn write_protected<B: I2c>(
        &mut self,
        bus: &mut B,
        register: u16,
        value: u8,
    ) -> Result<(), Error<B::Error>> {
        self.return_to_base_bank(bus).await?;
        for unlock in WRITE_ENABLE_SEQUENCE {
            self.write_wire(bus, register::I2C_ENABLE_AND_BANK.to_le_bytes()[0], unlock)
                .await?;
        }
        if register >= 0x0100 {
            if register > 0x014F {
                return Err(Error::UnsupportedRegister(register));
            }
            self.write_wire(bus, register::I2C_ENABLE_AND_BANK.to_le_bytes()[0], 0x81)
                .await?;
            self.bank = Bank::Extended;
        }
        self.write_wire(bus, register.to_le_bytes()[0], value)
            .await?;
        if register >= 0x0100 {
            self.return_to_base_bank(bus).await?;
        }
        Ok(())
    }

    async fn select_bank<B: I2c>(
        &mut self,
        bus: &mut B,
        register: u16,
    ) -> Result<(), Error<B::Error>> {
        match register {
            0x0000..=0x00FF => self.return_to_base_bank(bus).await,
            0x0100..=0x014F => {
                if self.bank == Bank::Extended {
                    return Ok(());
                }
                self.return_to_base_bank(bus).await?;
                for unlock in WRITE_ENABLE_SEQUENCE {
                    self.write_wire(bus, register::I2C_ENABLE_AND_BANK.to_le_bytes()[0], unlock)
                        .await?;
                }
                self.write_wire(bus, register::I2C_ENABLE_AND_BANK.to_le_bytes()[0], 0x81)
                    .await?;
                self.bank = Bank::Extended;
                Ok(())
            }
            _ => Err(Error::UnsupportedRegister(register)),
        }
    }

    async fn return_to_base_bank<B: I2c>(&mut self, bus: &mut B) -> Result<(), Error<B::Error>> {
        match self.bank {
            Bank::Base => Ok(()),
            Bank::Extended => {
                self.write_wire(bus, register::RETURN_TO_BASE_BANK.to_le_bytes()[0], 0)
                    .await?;
                self.bank = Bank::Base;
                Ok(())
            }
            Bank::Unknown => Err(Error::BankStateUnknown),
        }
    }

    async fn write_wire<B: I2c>(
        &mut self,
        bus: &mut B,
        wire_register: u8,
        value: u8,
    ) -> Result<(), Error<B::Error>> {
        if let Err(error) = bus.write(self.address, &[wire_register, value]).await {
            self.bank = Bank::Unknown;
            return Err(Error::Bus(error));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use embedded_hal_async::i2c::{ErrorType, Operation};
    use futures_lite::future::block_on;
    use std::collections::VecDeque;
    use std::vec;
    use std::vec::Vec;

    const DEVICE_ADDRESS: u8 = 0x3C;

    #[derive(Debug)]
    enum Expected {
        Write(Vec<u8>),
        WriteRead { register: u8, response: u8 },
    }

    struct FakeBus {
        expected: VecDeque<Expected>,
    }

    impl FakeBus {
        fn new(expected: impl IntoIterator<Item = Expected>) -> Self {
            Self {
                expected: expected.into_iter().collect(),
            }
        }

        fn assert_done(&self) {
            assert!(
                self.expected.is_empty(),
                "unused steps: {:?}",
                self.expected
            );
        }
    }

    impl ErrorType for FakeBus {
        type Error = Infallible;
    }

    #[allow(clippy::unused_async_trait_impl)]
    impl I2c for FakeBus {
        async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            assert_eq!(address, DEVICE_ADDRESS);
            let Expected::Write(expected) = self.expected.pop_front().expect("unexpected write")
            else {
                panic!("expected a read transaction")
            };
            assert_eq!(bytes, expected);
            Ok(())
        }

        async fn write_read(
            &mut self,
            address: u8,
            write: &[u8],
            read: &mut [u8],
        ) -> Result<(), Self::Error> {
            assert_eq!(address, DEVICE_ADDRESS);
            let Expected::WriteRead { register, response } =
                self.expected.pop_front().expect("unexpected write-read")
            else {
                panic!("expected a write transaction")
            };
            assert_eq!(write, [register]);
            assert_eq!(read.len(), 1);
            read[0] = response;
            Ok(())
        }

        async fn transaction(
            &mut self,
            _address: u8,
            _operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            unreachable!("the driver uses explicit write/write-read operations")
        }
    }

    fn unlock_steps() -> [Expected; 3] {
        [
            Expected::Write(vec![0x10, 0x20]),
            Expected::Write(vec![0x10, 0x40]),
            Expected::Write(vec![0x10, 0x80]),
        ]
    }

    #[test]
    fn write_enable_sequence_matches_registry_order() {
        assert_eq!(WRITE_ENABLE_SEQUENCE, [0x20, 0x40, 0x80]);
        assert!(
            WRITE_ENABLE_WRITES
                .iter()
                .all(|write| write.register == register::I2C_ENABLE_AND_BANK)
        );
    }

    #[test]
    fn nonstandard_seven_amp_policy_is_rejected() {
        let policy = CarrierPolicy {
            enabled: true,
            pd_limits: Some(PdLimits {
                max_voltage_mv: 20_000,
                max_current_ma: 7_000,
                max_power_mw: 100_000,
            }),
        };
        assert_eq!(
            validate_v1_policy(policy),
            Err(PolicyError::Limits(InvalidPdLimits::Current))
        );
    }

    #[test]
    fn identity_reads_only_documented_base_registers() {
        let mut bus = FakeBus::new([
            Expected::WriteRead {
                register: 0x00,
                response: 0x02,
            },
            Expected::WriteRead {
                register: 0x02,
                response: 100,
            },
        ]);
        let identity =
            block_on(Sw3538::assuming_base_bank(DEVICE_ADDRESS).read_identity(&mut bus)).unwrap();
        assert_eq!(
            identity,
            Identity {
                chip_version: 2,
                system_max_power_w: 100,
            }
        );
        bus.assert_done();
    }

    #[test]
    fn adc_read_unlocks_selects_and_then_reads_the_latched_pair() {
        let mut expected = Vec::from(unlock_steps());
        expected.extend([
            Expected::Write(vec![0x40, AdcChannel::OutputVoltage as u8]),
            Expected::WriteRead {
                register: 0x41,
                response: 0x34,
            },
            Expected::WriteRead {
                register: 0x42,
                response: 0x02,
            },
        ]);
        let mut bus = FakeBus::new(expected);
        let sample = block_on(
            Sw3538::assuming_base_bank(DEVICE_ADDRESS)
                .read_adc(&mut bus, AdcChannel::OutputVoltage),
        )
        .unwrap();
        assert_eq!(sample.raw, 0x234);
        assert_eq!(sample.unit, AdcUnit::Microvolts);
        assert_eq!(sample.value(), 0x234 * 6_000);
        bus.assert_done();
    }

    #[test]
    fn fixed_pdo_plan_never_exceeds_current_power_or_voltage_policy() {
        let plan = FixedPdoPlan::from_policy(CarrierPolicy {
            enabled: true,
            pd_limits: Some(PdLimits {
                max_voltage_mv: 20_000,
                max_current_ma: 5_000,
                max_power_mw: 60_000,
            }),
        })
        .unwrap();
        assert_eq!(plan.enabled_voltage_bits, 0x0F);
        assert_eq!(plan.current_5v_10ma, 500);
        assert_eq!(plan.current_9v_10ma, 500);
        assert_eq!(plan.current_12v_10ma, 500);
        assert_eq!(plan.current_15v_10ma, 400);
        assert_eq!(plan.current_20v_10ma, 300);

        let lower_voltage = FixedPdoPlan::from_policy(CarrierPolicy {
            enabled: true,
            pd_limits: Some(PdLimits {
                max_voltage_mv: 12_000,
                max_current_ma: 5_000,
                max_power_mw: 60_000,
            }),
        })
        .unwrap();
        assert_eq!(lower_voltage.enabled_voltage_bits, 0x03);
        assert_eq!(lower_voltage.current_15v_10ma, 0);
        assert_eq!(lower_voltage.current_20v_10ma, 0);
    }

    #[test]
    fn successful_extended_write_restores_the_base_bank() {
        let mut expected = Vec::from(unlock_steps());
        expected.extend([
            Expected::Write(vec![0x10, 0x81]),
            Expected::Write(vec![0x22, 0x01]),
            Expected::Write(vec![0x80, 0x00]),
            Expected::WriteRead {
                register: 0x00,
                response: 0x02,
            },
            Expected::WriteRead {
                register: 0x02,
                response: 100,
            },
        ]);
        let mut bus = FakeBus::new(expected);
        let mut device = Sw3538::assuming_base_bank(DEVICE_ADDRESS);

        block_on(device.write_protected(&mut bus, register::PD_CONTROL, 1)).unwrap();
        block_on(device.read_identity(&mut bus)).unwrap();
        bus.assert_done();
    }

    #[test]
    fn forced_current_encoding_rounds_down_and_holds_the_v1_cap() {
        assert_eq!(
            encode_forced_current_limit(1_049),
            Ok(ForcedCurrentLimit {
                register_value: 0,
                effective_ma: 1_000,
            })
        );
        assert_eq!(
            encode_forced_current_limit(5_000),
            Ok(ForcedCurrentLimit {
                register_value: 80,
                effective_ma: 5_000,
            })
        );
        assert_eq!(
            encode_forced_current_limit(5_001),
            Err(ForcedCurrentEncodingError::AboveV1Maximum)
        );
    }
}
