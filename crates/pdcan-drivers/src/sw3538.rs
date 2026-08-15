use pdcan_types::{PolicyValidationError, PortPolicy};

/// Register addresses and sequences confirmed by the SW3538 I2C registry.
pub mod register {
    pub const I2C_ENABLE_AND_BANK: u16 = 0x0010;
    pub const PORT_CONTROL: u16 = 0x0011;
    pub const APPLY_FORCED_CURRENT: u16 = 0x0017;
    pub const FORCED_CURRENT_LOW: u16 = 0x0038;
    pub const FORCED_CURRENT_HIGH: u16 = 0x0039;
    pub const ADC_SELECT: u16 = 0x0040;
    pub const ADC_DATA_LOW: u16 = 0x0041;
    pub const ADC_DATA_HIGH: u16 = 0x0042;
    pub const ADC_NTC_CONFIG: u16 = 0x0044;
    pub const PD_COMMAND: u16 = 0x00A7;
}

pub const WRITE_ENABLE_SEQUENCE: [u8; 3] = [0x20, 0x40, 0x80];

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

pub const fn validate_v1_policy(policy: PortPolicy) -> Result<PortPolicy, PolicyValidationError> {
    policy.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let policy = PortPolicy {
            enabled: true,
            max_voltage_mv: 20_000,
            max_current_ma: 7_000,
            max_power_mw: 100_000,
        };
        assert_eq!(
            validate_v1_policy(policy),
            Err(PolicyValidationError::CurrentTooHigh)
        );
    }
}
