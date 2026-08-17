#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

pub const WS2812_DATA_BITS: usize = 24;
pub const WS2812_RESET_WORDS: usize = 64;
pub const STATUS_WORDS: usize = WS2812_DATA_BITS + WS2812_RESET_WORDS;

impl Rgb {
    pub const OFF: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
    };

    pub const fn ws2812_grb(self) -> [u8; 3] {
        [self.green, self.red, self.blue]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusState {
    Booting,
    Uncommissioned,
    Healthy,
    Degraded,
    AddressConflict,
    EmergencyLatched,
    Fault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusCommand {
    SetBase(StatusState),
    IdentifySeconds(u16),
}

pub const IDENTIFY_COLOR: Rgb = Rgb {
    red: 24,
    green: 24,
    blue: 24,
};

pub const fn render(state: StatusState) -> Rgb {
    match state {
        StatusState::Booting => Rgb {
            red: 0,
            green: 0,
            blue: 24,
        },
        StatusState::Uncommissioned => Rgb {
            red: 20,
            green: 12,
            blue: 0,
        },
        StatusState::Healthy => Rgb {
            red: 0,
            green: 24,
            blue: 0,
        },
        StatusState::Degraded => Rgb {
            red: 24,
            green: 8,
            blue: 0,
        },
        StatusState::AddressConflict => Rgb {
            red: 24,
            green: 0,
            blue: 24,
        },
        StatusState::EmergencyLatched | StatusState::Fault => Rgb {
            red: 24,
            green: 0,
            blue: 0,
        },
    }
}

/// Encodes one GRB pixel for an 800 kHz timer. Duty timings are intentionally
/// conservative and must be measured on Rev A before they are treated as final.
pub fn encode_ws2812_pwm(color: Rgb, period_ticks: u16, output: &mut [u16; STATUS_WORDS]) {
    let zero_high = period_ticks.saturating_mul(28) / 100;
    let one_high = period_ticks.saturating_mul(56) / 100;
    let mut output_index = 0;

    for byte in color.ws2812_grb() {
        for bit in (0..u8::BITS).rev() {
            output[output_index] = if byte & (1 << bit) == 0 {
                zero_high
            } else {
                one_high
            };
            output_index += 1;
        }
    }
    output[output_index..].fill(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws2812_encoding_uses_grb_wire_order() {
        assert_eq!(
            Rgb {
                red: 1,
                green: 2,
                blue: 3,
            }
            .ws2812_grb(),
            [2, 1, 3]
        );
    }

    #[test]
    fn emergency_and_fault_render_as_highest_severity_red() {
        assert_eq!(
            render(StatusState::EmergencyLatched),
            render(StatusState::Fault)
        );
    }

    #[test]
    fn pwm_encoding_is_msb_first_grb_and_ends_with_reset_low() {
        let mut words = [u16::MAX; STATUS_WORDS];
        encode_ws2812_pwm(
            Rgb {
                red: 0,
                green: 0x80,
                blue: 0,
            },
            100,
            &mut words,
        );

        assert_eq!(words[0], 56);
        assert!(words[1..WS2812_DATA_BITS].iter().all(|word| *word == 28));
        assert!(words[WS2812_DATA_BITS..].iter().all(|word| *word == 0));
    }
}
