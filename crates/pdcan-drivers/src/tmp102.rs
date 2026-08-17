use embedded_hal_async::i2c::I2c;

/// TMP102 address when ADD0 is tied to ground.
pub const DEFAULT_ADDRESS: u8 = 0x48;

const TEMPERATURE_REGISTER: u8 = 0x00;

/// Minimal TMP102 driver for the power-up-default 12-bit continuous-conversion
/// mode. Alert and threshold registers are intentionally not configured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tmp102 {
    address: u8,
}

impl Tmp102 {
    pub const fn new(address: u8) -> Self {
        Self { address }
    }

    pub const fn default_address() -> Self {
        Self::new(DEFAULT_ADDRESS)
    }

    pub const fn address(self) -> u8 {
        self.address
    }

    /// Reads the temperature register and returns the nearest centi-degree
    /// Celsius. The default 12-bit result has a 0.0625 deg C LSB, so some
    /// samples require rounding to fit the wire protocol's 0.01 deg C unit.
    pub async fn read_temperature_centi_c<B: I2c>(self, bus: &mut B) -> Result<i16, B::Error> {
        let mut register = [0; 2];
        bus.write_read(self.address, &[TEMPERATURE_REGISTER], &mut register)
            .await?;
        Ok(decode_temperature_centi_c(register))
    }
}

fn decode_temperature_centi_c(register: [u8; 2]) -> i16 {
    let raw = i16::from_be_bytes(register) >> 4;
    let scaled = i32::from(raw) * 25;
    let rounded = if scaled < 0 {
        (scaled - 2) / 4
    } else {
        (scaled + 2) / 4
    };
    i16::try_from(rounded).expect("a 12-bit TMP102 sample fits centi-degrees in i16")
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use embedded_hal_async::i2c::{ErrorType, Operation};
    use futures_lite::future::block_on;
    use std::{vec, vec::Vec};

    #[derive(Default)]
    struct FakeBus {
        temperature_register: [u8; 2],
        transactions: Vec<(u8, Vec<u8>, usize)>,
    }

    impl ErrorType for FakeBus {
        type Error = Infallible;
    }

    impl I2c for FakeBus {
        async fn transaction(
            &mut self,
            address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            let mut written = Vec::new();
            let mut read_len = 0;
            for operation in operations {
                match operation {
                    Operation::Write(bytes) => written.extend_from_slice(bytes),
                    Operation::Read(bytes) => {
                        read_len += bytes.len();
                        bytes.copy_from_slice(&self.temperature_register[..bytes.len()]);
                    }
                }
            }
            self.transactions.push((address, written, read_len));
            Ok(())
        }
    }

    #[test]
    fn reads_temperature_register_at_ground_strapped_address() {
        let mut bus = FakeBus {
            temperature_register: [0x19, 0x00],
            ..FakeBus::default()
        };

        let temperature =
            block_on(Tmp102::default_address().read_temperature_centi_c(&mut bus)).unwrap();

        assert_eq!(temperature, 2_500);
        assert_eq!(bus.transactions, [(DEFAULT_ADDRESS, vec![0x00], 2)]);
    }

    #[test]
    fn decodes_signed_12_bit_samples_and_rounds_to_nearest_centi_degree() {
        assert_eq!(decode_temperature_centi_c([0x19, 0x10]), 2_506);
        assert_eq!(decode_temperature_centi_c([0xff, 0xf0]), -6);
        assert_eq!(decode_temperature_centi_c([0xe7, 0x00]), -2_500);
        assert_eq!(decode_temperature_centi_c([0x7f, 0xf0]), 12_794);
        assert_eq!(decode_temperature_centi_c([0x80, 0x00]), -12_800);
    }

    #[test]
    fn supports_non_default_address_straps() {
        assert_eq!(Tmp102::new(0x4b).address(), 0x4b);
    }
}
