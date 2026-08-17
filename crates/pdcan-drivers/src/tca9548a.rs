use embedded_hal_async::i2c::I2c;
use pdcan_types::{BoardDefinition, PortId};

pub const DEFAULT_ADDRESS: u8 = 0x70;
pub const DESELECT_ALL: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelSelectionError {
    UnsupportedPort(PortId),
    InvalidMuxChannel(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    Bus(E),
    Selection(ChannelSelectionError),
    InvalidSelectionReadback(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tca9548a {
    address: u8,
}

impl Tca9548a {
    pub const fn new(address: u8) -> Self {
        Self { address }
    }

    pub const fn default_address() -> Self {
        Self::new(DEFAULT_ADDRESS)
    }

    pub const fn address(self) -> u8 {
        self.address
    }

    pub async fn select_port<B: I2c>(
        self,
        bus: &mut B,
        board: BoardDefinition,
        port: PortId,
    ) -> Result<(), Error<B::Error>> {
        let selection = selection_for_port(board, port).map_err(Error::Selection)?;
        self.select_mask(bus, selection).await
    }

    pub async fn select_mask<B: I2c>(
        self,
        bus: &mut B,
        selection: u8,
    ) -> Result<(), Error<B::Error>> {
        if selection.count_ones() > 1 {
            return Err(Error::InvalidSelectionReadback(selection));
        }
        bus.write(self.address, &[selection])
            .await
            .map_err(Error::Bus)
    }

    pub async fn deselect_all<B: I2c>(self, bus: &mut B) -> Result<(), Error<B::Error>> {
        self.select_mask(bus, DESELECT_ALL).await
    }

    pub async fn read_selection<B: I2c>(self, bus: &mut B) -> Result<u8, Error<B::Error>> {
        let mut selection = [0];
        bus.read(self.address, &mut selection)
            .await
            .map_err(Error::Bus)?;
        if selection[0].count_ones() > 1 {
            return Err(Error::InvalidSelectionReadback(selection[0]));
        }
        Ok(selection[0])
    }
}

pub fn selection_for_port(
    board: BoardDefinition,
    port: PortId,
) -> Result<u8, ChannelSelectionError> {
    if !board.supports(port) {
        return Err(ChannelSelectionError::UnsupportedPort(port));
    }
    let channel = board
        .mux_channel(port)
        .ok_or(ChannelSelectionError::UnsupportedPort(port))?;
    if channel >= 8 {
        return Err(ChannelSelectionError::InvalidMuxChannel(channel));
    }
    Ok(1 << channel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use embedded_hal_async::i2c::{ErrorType, Operation};
    use futures_lite::future::block_on;
    use pdcan_types::{FanMode, HardwareRevision, PortBitmap};
    use std::vec;
    use std::vec::Vec;

    const REV_A: BoardDefinition = BoardDefinition {
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
        power_gate_bit_by_port: [None; pdcan_types::MAX_PORTS],
        default_fan_mode: FanMode::ThreeWire,
    };

    #[test]
    fn rev_a_maps_ports_zero_through_five_to_one_hot_channels() {
        for raw_port in 0..6 {
            let port = PortId::new(raw_port).unwrap();
            assert_eq!(selection_for_port(REV_A, port), Ok(1 << raw_port));
        }
    }

    #[test]
    fn rev_a_never_selects_test_pad_channels_as_ports() {
        for raw_port in 6..8 {
            let port = PortId::new(raw_port).unwrap();
            assert_eq!(
                selection_for_port(REV_A, port),
                Err(ChannelSelectionError::UnsupportedPort(port))
            );
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    enum Transaction {
        Write(u8, Vec<u8>),
        Read(u8, usize),
    }

    #[derive(Default)]
    struct FakeBus {
        transactions: Vec<Transaction>,
        read_value: u8,
    }

    impl ErrorType for FakeBus {
        type Error = Infallible;
    }

    impl I2c for FakeBus {
        async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            self.transactions
                .push(Transaction::Write(address, bytes.to_vec()));
            Ok(())
        }

        async fn read(&mut self, address: u8, bytes: &mut [u8]) -> Result<(), Self::Error> {
            self.transactions
                .push(Transaction::Read(address, bytes.len()));
            bytes.fill(self.read_value);
            Ok(())
        }

        async fn transaction(
            &mut self,
            _address: u8,
            _operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            unreachable!("the driver uses explicit read/write operations")
        }
    }

    #[test]
    fn async_driver_selects_exactly_one_rev_a_channel_then_deselects() {
        let mut bus = FakeBus::default();
        let mux = Tca9548a::default_address();

        block_on(async {
            mux.select_port(&mut bus, REV_A, PortId::new(4).unwrap())
                .await
                .unwrap();
            mux.deselect_all(&mut bus).await.unwrap();
        });

        assert_eq!(
            bus.transactions,
            [
                Transaction::Write(DEFAULT_ADDRESS, vec![0x10]),
                Transaction::Write(DEFAULT_ADDRESS, vec![0x00]),
            ]
        );
    }

    #[test]
    fn driver_rejects_multi_channel_readback() {
        let mut bus = FakeBus {
            transactions: Vec::new(),
            read_value: 0b11,
        };
        assert_eq!(
            block_on(Tca9548a::default_address().read_selection(&mut bus)),
            Err(Error::InvalidSelectionReadback(0b11))
        );
    }
}
