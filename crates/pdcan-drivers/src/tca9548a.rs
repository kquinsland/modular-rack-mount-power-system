use pdcan_types::{BoardDefinition, PortId};

pub const DEFAULT_ADDRESS: u8 = 0x70;
pub const DESELECT_ALL: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelSelectionError {
    UnsupportedPort(PortId),
    InvalidMuxChannel(u8),
}

pub fn selection_for_port(
    board: BoardDefinition,
    port: PortId,
) -> Result<u8, ChannelSelectionError> {
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
    use pdcan_types::{FanMode, HardwareRevision, PortBitmap};

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
}
