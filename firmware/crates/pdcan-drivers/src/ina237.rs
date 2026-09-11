use embedded_hal_async::i2c::I2c;

pub const DEFAULT_ADDRESS: u8 = 0x40;

pub mod register {
    pub const CONFIG: u8 = 0x00;
    pub const ADC_CONFIG: u8 = 0x01;
    pub const SHUNT_CAL: u8 = 0x02;
    pub const VSHUNT: u8 = 0x04;
    pub const VBUS: u8 = 0x05;
    pub const DIE_TEMP: u8 = 0x06;
    pub const CURRENT: u8 = 0x07;
    pub const POWER: u8 = 0x08;
    pub const DIAG_ALERT: u8 = 0x0B;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdcRange {
    Wide,
    Narrow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub adc_range: AdcRange,
    pub adc_config: u16,
    pub shunt_microohms: u32,
    pub current_lsb_microamps: u32,
}

impl Config {
    pub fn shunt_calibration(self) -> Result<u16, ConfigError> {
        if self.shunt_microohms == 0 || self.current_lsb_microamps == 0 {
            return Err(ConfigError::ZeroScale);
        }
        // Datasheet equation:
        // SHUNT_CAL = 819.2e6 * CURRENT_LSB[A] * RSHUNT[ohm].
        let numerator = u64::from(self.current_lsb_microamps)
            .checked_mul(u64::from(self.shunt_microohms))
            .and_then(|value| value.checked_mul(8_192))
            .ok_or(ConfigError::CalibrationOutOfRange)?;
        let rounded = numerator
            .checked_add(5_000_000)
            .ok_or(ConfigError::CalibrationOutOfRange)?
            / 10_000_000;
        u16::try_from(rounded)
            .ok()
            .filter(|value| *value <= 0x7FFF && *value != 0)
            .ok_or(ConfigError::CalibrationOutOfRange)
    }

    pub const fn config_register(self) -> u16 {
        match self.adc_range {
            AdcRange::Wide => 0,
            AdcRange::Narrow => 1 << 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    ZeroScale,
    CalibrationOutOfRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    Bus(E),
    Config(ConfigError),
    ConversionOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Measurement {
    pub bus_voltage_mv: u32,
    pub shunt_voltage_uv: i32,
    pub current_ma: i32,
    pub power_mw: u32,
    pub die_temperature_centi_c: i16,
    pub diagnostic_flags: u16,
}

pub struct Ina237 {
    address: u8,
    config: Config,
}

impl Ina237 {
    pub const fn new(address: u8, config: Config) -> Self {
        Self { address, config }
    }

    pub const fn address(&self) -> u8 {
        self.address
    }

    pub const fn config(&self) -> Config {
        self.config
    }

    pub async fn configure<B: I2c>(&self, bus: &mut B) -> Result<(), Error<B::Error>> {
        let shunt_cal = self.config.shunt_calibration().map_err(Error::Config)?;
        self.write_u16(bus, register::CONFIG, self.config.config_register())
            .await?;
        self.write_u16(bus, register::ADC_CONFIG, self.config.adc_config)
            .await?;
        self.write_u16(bus, register::SHUNT_CAL, shunt_cal).await
    }

    pub async fn read_measurement<B: I2c>(
        &self,
        bus: &mut B,
    ) -> Result<Measurement, Error<B::Error>> {
        let shunt = self.read_u16(bus, register::VSHUNT).await?.cast_signed();
        let bus_voltage = self.read_u16(bus, register::VBUS).await?;
        let temperature = self.read_u16(bus, register::DIE_TEMP).await?.cast_signed();
        let current = self.read_u16(bus, register::CURRENT).await?.cast_signed();
        let power = self.read_u24(bus, register::POWER).await?;
        let diagnostic_flags = self.read_u16(bus, register::DIAG_ALERT).await?;
        Ok(Measurement {
            bus_voltage_mv: bus_voltage_mv(bus_voltage),
            shunt_voltage_uv: shunt_voltage_uv(shunt, self.config.adc_range),
            current_ma: current_ma(current, self.config.current_lsb_microamps)
                .ok_or(Error::ConversionOverflow)?,
            power_mw: power_mw(power, self.config.current_lsb_microamps)
                .ok_or(Error::ConversionOverflow)?,
            die_temperature_centi_c: die_temperature_centi_c(temperature),
            diagnostic_flags,
        })
    }

    async fn write_u16<B: I2c>(
        &self,
        bus: &mut B,
        register: u8,
        value: u16,
    ) -> Result<(), Error<B::Error>> {
        let bytes = value.to_be_bytes();
        bus.write(self.address, &[register, bytes[0], bytes[1]])
            .await
            .map_err(Error::Bus)
    }

    async fn read_u16<B: I2c>(&self, bus: &mut B, register: u8) -> Result<u16, Error<B::Error>> {
        let mut bytes = [0; 2];
        bus.write_read(self.address, &[register], &mut bytes)
            .await
            .map_err(Error::Bus)?;
        Ok(u16::from_be_bytes(bytes))
    }

    async fn read_u24<B: I2c>(&self, bus: &mut B, register: u8) -> Result<u32, Error<B::Error>> {
        let mut bytes = [0; 3];
        bus.write_read(self.address, &[register], &mut bytes)
            .await
            .map_err(Error::Bus)?;
        Ok(u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]))
    }
}

pub const fn bus_voltage_mv(raw: u16) -> u32 {
    // 3.125 mV/LSB, rounded to the nearest millivolt.
    (raw as u32 * 25 + 4) / 8
}

pub const fn shunt_voltage_uv(raw: i16, range: AdcRange) -> i32 {
    match range {
        AdcRange::Wide => raw as i32 * 5,
        // 1.25 uV/LSB, rounded away from zero at half-microvolt.
        AdcRange::Narrow => {
            let scaled = raw as i32 * 5;
            if scaled >= 0 {
                (scaled + 2) / 4
            } else {
                (scaled - 2) / 4
            }
        }
    }
}

pub fn die_temperature_centi_c(raw_register: i16) -> i16 {
    // The signed 12-bit sample occupies bits 15:4 at 0.125 C/LSB.
    let sample = raw_register >> 4;
    let scaled = i32::from(sample) * 25;
    let rounded = if scaled >= 0 {
        (scaled + 1) / 2
    } else {
        (scaled - 1) / 2
    };
    i16::try_from(rounded).expect("signed 12-bit temperature always fits i16 centi-degrees")
}

pub fn current_ma(raw: i16, current_lsb_microamps: u32) -> Option<i32> {
    let microamps = i64::from(raw).checked_mul(i64::from(current_lsb_microamps))?;
    i32::try_from(microamps / 1_000).ok()
}

pub fn power_mw(raw: u32, current_lsb_microamps: u32) -> Option<u32> {
    // POWER_LSB = 0.2 * CURRENT_LSB. Convert W to mW.
    let numerator = u64::from(raw).checked_mul(u64::from(current_lsb_microamps))?;
    u32::try_from(numerator / 5_000).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use futures_lite::future::block_on;
    use std::collections::VecDeque;
    use std::vec;
    use std::vec::Vec;

    use embedded_hal_async::i2c::{ErrorType, Operation};

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Expected {
        Write(Vec<u8>),
        Read { register: u8, response: Vec<u8> },
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
            assert!(self.expected.is_empty());
        }
    }

    impl ErrorType for FakeBus {
        type Error = Infallible;
    }

    #[allow(clippy::unused_async_trait_impl)]
    impl I2c for FakeBus {
        async fn read(&mut self, _address: u8, _bytes: &mut [u8]) -> Result<(), Self::Error> {
            unreachable!()
        }

        async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            assert_eq!(address, DEFAULT_ADDRESS);
            let Expected::Write(expected) = self.expected.pop_front().expect("unexpected write")
            else {
                panic!("expected register read")
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
            assert_eq!(address, DEFAULT_ADDRESS);
            let Expected::Read { register, response } =
                self.expected.pop_front().expect("unexpected read")
            else {
                panic!("expected register write")
            };
            assert_eq!(write, [register]);
            assert_eq!(read.len(), response.len());
            read.copy_from_slice(&response);
            Ok(())
        }

        async fn transaction(
            &mut self,
            _address: u8,
            _operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            unreachable!()
        }
    }

    const CARRIER_CONFIG: Config = Config {
        adc_range: AdcRange::Wide,
        adc_config: 0xFB68,
        shunt_microohms: 6_000,
        current_lsb_microamps: 500,
    };

    #[test]
    fn carrier_and_backplane_calibration_values_fit_the_register() {
        assert_eq!(CARRIER_CONFIG.shunt_calibration(), Ok(2_458));
        let backplane = Config {
            shunt_microohms: 1_000,
            current_lsb_microamps: 1_000,
            adc_range: AdcRange::Narrow,
            adc_config: 0xFB68,
        };
        assert_eq!(backplane.shunt_calibration(), Ok(819));
    }

    #[test]
    fn configure_writes_big_endian_register_values() {
        let mut bus = FakeBus::new([
            Expected::Write(vec![register::CONFIG, 0x00, 0x00]),
            Expected::Write(vec![register::ADC_CONFIG, 0xFB, 0x68]),
            Expected::Write(vec![register::SHUNT_CAL, 0x09, 0x9A]),
        ]);
        block_on(Ina237::new(DEFAULT_ADDRESS, CARRIER_CONFIG).configure(&mut bus)).unwrap();
        bus.assert_done();
    }

    #[test]
    fn measurement_reads_voltage_current_power_temperature_and_alerts() {
        let mut bus = FakeBus::new([
            Expected::Read {
                register: register::VSHUNT,
                response: vec![0x03, 0xE8],
            },
            Expected::Read {
                register: register::VBUS,
                response: vec![0x1E, 0x00],
            },
            Expected::Read {
                register: register::DIE_TEMP,
                response: vec![0x19, 0x00],
            },
            Expected::Read {
                register: register::CURRENT,
                response: vec![0x13, 0x88],
            },
            Expected::Read {
                register: register::POWER,
                response: vec![0x01, 0xE8, 0x48],
            },
            Expected::Read {
                register: register::DIAG_ALERT,
                response: vec![0x00, 0x02],
            },
        ]);
        let measurement =
            block_on(Ina237::new(DEFAULT_ADDRESS, CARRIER_CONFIG).read_measurement(&mut bus))
                .unwrap();
        assert_eq!(measurement.bus_voltage_mv, 24_000);
        assert_eq!(measurement.shunt_voltage_uv, 5_000);
        assert_eq!(measurement.current_ma, 2_500);
        assert_eq!(measurement.power_mw, 12_500);
        assert_eq!(measurement.die_temperature_centi_c, 5_000);
        assert_eq!(measurement.diagnostic_flags, 2);
        bus.assert_done();
    }

    #[test]
    fn signed_narrow_range_and_temperature_scaling_round_correctly() {
        assert_eq!(shunt_voltage_uv(-3, AdcRange::Narrow), -4);
        assert_eq!(die_temperature_centi_c((-16_i16) << 4), -200);
    }
}
