use pdcan_types::{FanConfig, FanMode, InvalidFanDuty};

pub const THREE_WIRE_FREQUENCY_HZ_PROVISIONAL: u32 = 30;
pub const FOUR_WIRE_FREQUENCY_HZ_PROVISIONAL: u32 = 25_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanDrive {
    pub frequency_hz: u32,
    /// Percentage of time PA0 drives high and turns Q2 on.
    pub pa0_high_percent: u8,
}

impl FanDrive {
    pub const BOOT_SAFE: Self = Self {
        frequency_hz: THREE_WIRE_FREQUENCY_HZ_PROVISIONAL,
        pa0_high_percent: 100,
    };

    pub fn from_config(config: FanConfig, force_full_speed: bool) -> Result<Self, InvalidFanDuty> {
        let config = config.validate()?;
        let requested = if force_full_speed {
            100
        } else {
            config.duty_percent
        };
        Ok(match config.mode {
            FanMode::ThreeWire => Self {
                frequency_hz: THREE_WIRE_FREQUENCY_HZ_PROVISIONAL,
                pa0_high_percent: requested,
            },
            // Q2 inverts the four-wire fan's open-drain control input. A full-speed
            // command leaves Q2 off, so PA0's active-high percentage is inverted.
            FanMode::FourWire => Self {
                frequency_hz: FOUR_WIRE_FREQUENCY_HZ_PROVISIONAL,
                pa0_high_percent: 100 - requested,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_safe_state_turns_on_the_default_three_wire_fan() {
        assert_eq!(FanDrive::BOOT_SAFE.pa0_high_percent, 100);
        assert_eq!(FanDrive::BOOT_SAFE.frequency_hz, 30);
    }

    #[test]
    fn four_wire_drive_accounts_for_the_open_drain_inversion() {
        let drive = FanDrive::from_config(
            FanConfig {
                mode: FanMode::FourWire,
                duty_percent: 70,
            },
            false,
        )
        .unwrap();
        assert_eq!(drive.frequency_hz, 25_000);
        assert_eq!(drive.pa0_high_percent, 30);
        assert_eq!(
            FanDrive::from_config(
                FanConfig {
                    mode: FanMode::FourWire,
                    duty_percent: 20,
                },
                true,
            )
            .unwrap()
            .pa0_high_percent,
            0
        );
    }
}
