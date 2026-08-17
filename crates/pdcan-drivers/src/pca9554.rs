use embedded_hal_async::i2c::I2c;

pub const DEFAULT_ADDRESS: u8 = 0x20;

const OUTPUT_PORT_REGISTER: u8 = 0x01;
const CONFIGURATION_REGISTER: u8 = 0x03;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationError {
    NoOutputs,
    NotInitialized,
    InvalidOutputBit(u8),
    BitsOutsideOutputMask(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    Bus(E),
    Configuration(ConfigurationError),
}

/// Safety-oriented PCA9554 output driver.
///
/// The PCA9554 powers up with every port configured as an input and its output
/// latch set high. `initialize_outputs_low` writes the low output latch before
/// changing any selected port to an output, preventing an enable pulse during
/// initialization when the board supplies external pull-downs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pca9554 {
    address: u8,
    output_mask: u8,
    output_shadow: u8,
    initialized: bool,
}

impl Pca9554 {
    pub const fn new(address: u8) -> Self {
        Self {
            address,
            output_mask: 0,
            output_shadow: 0,
            initialized: false,
        }
    }

    pub const fn default_address() -> Self {
        Self::new(DEFAULT_ADDRESS)
    }

    pub const fn address(self) -> u8 {
        self.address
    }

    pub const fn initialized(self) -> bool {
        self.initialized
    }

    pub const fn output_mask(self) -> u8 {
        self.output_mask
    }

    pub const fn output_shadow(self) -> u8 {
        self.output_shadow
    }

    /// Latches all outputs low before enabling only the bits in `output_mask`
    /// as push-pull outputs. Unused bits remain high-impedance inputs.
    pub async fn initialize_outputs_low<B: I2c>(
        &mut self,
        bus: &mut B,
        output_mask: u8,
    ) -> Result<(), Error<B::Error>> {
        if output_mask == 0 {
            return Err(Error::Configuration(ConfigurationError::NoOutputs));
        }

        self.initialized = false;
        self.output_mask = 0;
        self.output_shadow = 0;
        self.write_register(bus, OUTPUT_PORT_REGISTER, 0).await?;
        self.write_register(bus, CONFIGURATION_REGISTER, !output_mask)
            .await?;
        self.output_mask = output_mask;
        self.initialized = true;
        Ok(())
    }

    /// Requests the fail-closed state even if initialization did not finish.
    ///
    /// On a cold device, writing the latch low remains safe while the pins are
    /// inputs. On an already configured device, it immediately drives every
    /// controlled output low.
    pub async fn all_off<B: I2c>(&mut self, bus: &mut B) -> Result<(), Error<B::Error>> {
        self.write_register(bus, OUTPUT_PORT_REGISTER, 0).await?;
        self.output_shadow = 0;
        Ok(())
    }

    pub async fn set_output<B: I2c>(
        &mut self,
        bus: &mut B,
        output_bit: u8,
        enabled: bool,
    ) -> Result<(), Error<B::Error>> {
        if !self.initialized {
            return Err(Error::Configuration(ConfigurationError::NotInitialized));
        }
        if output_bit >= 8 || self.output_mask & (1 << output_bit) == 0 {
            return Err(Error::Configuration(ConfigurationError::InvalidOutputBit(
                output_bit,
            )));
        }

        let bit = 1 << output_bit;
        let outputs = if enabled {
            self.output_shadow | bit
        } else {
            self.output_shadow & !bit
        };
        self.write_outputs(bus, outputs).await
    }

    pub async fn write_outputs<B: I2c>(
        &mut self,
        bus: &mut B,
        outputs: u8,
    ) -> Result<(), Error<B::Error>> {
        if !self.initialized {
            return Err(Error::Configuration(ConfigurationError::NotInitialized));
        }
        let invalid = outputs & !self.output_mask;
        if invalid != 0 {
            return Err(Error::Configuration(
                ConfigurationError::BitsOutsideOutputMask(invalid),
            ));
        }

        self.write_register(bus, OUTPUT_PORT_REGISTER, outputs)
            .await?;
        self.output_shadow = outputs;
        Ok(())
    }

    async fn write_register<B: I2c>(
        &self,
        bus: &mut B,
        register: u8,
        value: u8,
    ) -> Result<(), Error<B::Error>> {
        bus.write(self.address, &[register, value])
            .await
            .map_err(Error::Bus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_async::i2c::{ErrorKind, ErrorType, Operation};
    use futures_lite::future::block_on;
    use std::vec;
    use std::vec::Vec;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestError;

    impl embedded_hal_async::i2c::Error for TestError {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    #[derive(Default)]
    struct FakeBus {
        writes: Vec<(u8, Vec<u8>)>,
        fail_write: Option<usize>,
    }

    impl ErrorType for FakeBus {
        type Error = TestError;
    }

    impl I2c for FakeBus {
        async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            let write_index = self.writes.len();
            self.writes.push((address, bytes.to_vec()));
            if self.fail_write == Some(write_index) {
                Err(TestError)
            } else {
                Ok(())
            }
        }

        async fn read(&mut self, _address: u8, _bytes: &mut [u8]) -> Result<(), Self::Error> {
            unreachable!("the driver does not read registers")
        }

        async fn transaction(
            &mut self,
            _address: u8,
            _operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            unreachable!("the driver uses explicit writes")
        }
    }

    #[test]
    fn initialization_latches_low_before_enabling_six_outputs() {
        let mut bus = FakeBus::default();
        let mut expander = Pca9554::default_address();

        block_on(expander.initialize_outputs_low(&mut bus, 0x3F)).unwrap();

        assert_eq!(
            bus.writes,
            [
                (DEFAULT_ADDRESS, vec![OUTPUT_PORT_REGISTER, 0]),
                (DEFAULT_ADDRESS, vec![CONFIGURATION_REGISTER, 0xC0]),
            ]
        );
        assert!(expander.initialized());
        assert_eq!(expander.output_mask(), 0x3F);
        assert_eq!(expander.output_shadow(), 0);
    }

    #[test]
    fn port_updates_preserve_the_other_output_bits() {
        let mut bus = FakeBus::default();
        let mut expander = Pca9554::default_address();

        block_on(async {
            expander
                .initialize_outputs_low(&mut bus, 0x3F)
                .await
                .unwrap();
            expander.set_output(&mut bus, 1, true).await.unwrap();
            expander.set_output(&mut bus, 4, true).await.unwrap();
            expander.set_output(&mut bus, 1, false).await.unwrap();
        });

        assert_eq!(expander.output_shadow(), 0x10);
        assert_eq!(
            &bus.writes[2..],
            [
                (DEFAULT_ADDRESS, vec![OUTPUT_PORT_REGISTER, 0x02]),
                (DEFAULT_ADDRESS, vec![OUTPUT_PORT_REGISTER, 0x12]),
                (DEFAULT_ADDRESS, vec![OUTPUT_PORT_REGISTER, 0x10]),
            ]
        );
    }

    #[test]
    fn all_off_is_available_before_configuration() {
        let mut bus = FakeBus::default();
        let mut expander = Pca9554::default_address();

        block_on(expander.all_off(&mut bus)).unwrap();

        assert_eq!(
            bus.writes,
            [(DEFAULT_ADDRESS, vec![OUTPUT_PORT_REGISTER, 0])]
        );
        assert!(!expander.initialized());
    }

    #[test]
    fn failed_configuration_never_reports_initialized() {
        let mut bus = FakeBus {
            writes: Vec::new(),
            fail_write: Some(1),
        };
        let mut expander = Pca9554::default_address();

        assert_eq!(
            block_on(expander.initialize_outputs_low(&mut bus, 0x3F)),
            Err(Error::Bus(TestError))
        );
        assert!(!expander.initialized());
        assert_eq!(expander.output_mask(), 0);
        assert_eq!(expander.output_shadow(), 0);
    }

    #[test]
    fn unused_bits_and_uninitialized_updates_are_rejected_without_bus_io() {
        let mut bus = FakeBus::default();
        let mut expander = Pca9554::default_address();

        assert_eq!(
            block_on(expander.set_output(&mut bus, 0, true)),
            Err(Error::Configuration(ConfigurationError::NotInitialized))
        );
        block_on(expander.initialize_outputs_low(&mut bus, 0x3F)).unwrap();
        let writes_before = bus.writes.len();
        assert_eq!(
            block_on(expander.set_output(&mut bus, 6, true)),
            Err(Error::Configuration(ConfigurationError::InvalidOutputBit(
                6
            )))
        );
        assert_eq!(
            block_on(expander.write_outputs(&mut bus, 0x80)),
            Err(Error::Configuration(
                ConfigurationError::BitsOutsideOutputMask(0x80)
            ))
        );
        assert_eq!(bus.writes.len(), writes_before);
    }
}
