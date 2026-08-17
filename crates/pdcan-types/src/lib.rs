#![no_std]

use core::fmt;

pub const MAX_PORTS: usize = 8;
pub const MAX_PORT_VOLTAGE_MV: u16 = 20_000;
pub const MAX_PORT_CURRENT_MA: u16 = 5_000;
pub const MAX_PORT_POWER_MW: u32 = 100_000;
pub const MIN_ENABLED_PORT_VOLTAGE_MV: u16 = 5_000;
pub const MIN_ENABLED_PORT_CURRENT_MA: u16 = 10;
pub const MIN_ENABLED_PORT_POWER_MW: u32 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PortId(u8);

impl PortId {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 7;

    pub const fn new(value: u8) -> Result<Self, InvalidPortId> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(InvalidPortId(value))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidPortId(pub u8);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PortBitmap(u8);

impl PortBitmap {
    pub const EMPTY: Self = Self(0);
    pub const ALL: Self = Self(u8::MAX);

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, port: PortId) -> bool {
        self.0 & (1 << port.get()) != 0
    }

    pub fn insert(&mut self, port: PortId) {
        self.0 |= 1 << port.get();
    }

    pub fn remove(&mut self, port: PortId) {
        self.0 &= !(1 << port.get());
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn take_lowest(&mut self) -> Option<PortId> {
        let bit = self.0.trailing_zeros();
        if bit >= u8::BITS {
            return None;
        }

        let port = PortId::new(u8::try_from(bit).ok()?).ok()?;
        self.remove(port);
        Some(port)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct NodeId(u8);

impl NodeId {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 0xFE;

    pub const fn new(value: u8) -> Result<Self, InvalidNodeId> {
        if value >= Self::MIN && value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(InvalidNodeId(value))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidNodeId(pub u8);

/// The STM32 factory-programmed 96-bit unique device identifier.
///
/// This is deliberately named a UID rather than a UUID: the silicon value does
/// not have UUID variant or version semantics.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeUid([u8; Self::LENGTH]);

impl NodeUid {
    pub const LENGTH: usize = 12;

    pub const fn from_bytes(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    pub const fn to_bytes(self) -> [u8; Self::LENGTH] {
        self.0
    }

    pub const fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }
}

impl fmt::Debug for NodeUid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NodeUid(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CommissioningState {
    Uncommissioned = 0,
    Claiming = 1,
    Commissioned = 2,
    AddressConflict = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RequesterId(u8);

impl RequesterId {
    pub const PROTOCOL: Self = Self(0);
    pub const PDCAN_DEFAULT: Self = Self(15);
    pub const MAX: u8 = 15;

    pub const fn new(value: u8) -> Result<Self, InvalidRequesterId> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(InvalidRequesterId(value))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn is_protocol_reserved(self) -> bool {
        self.0 == Self::PROTOCOL.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidRequesterId(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RequestId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct OperationId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SlotEpoch(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ConfigRevision(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum HardwareRevision {
    RevA = 1,
    RevB = 2,
    Simulator = 0xFE,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardDefinition {
    pub hardware_revision: HardwareRevision,
    pub supported_ports: PortBitmap,
    pub mux_channel_by_port: [Option<u8>; MAX_PORTS],
    /// Power-controller output bit controlling input power for each logical port.
    ///
    /// `None` means that the board has no firmware-controlled input-power
    /// switch for that port. The bit number is deliberately independent from
    /// the mux channel so future boards do not have to preserve either wiring
    /// order.
    pub power_gate_bit_by_port: [Option<u8>; MAX_PORTS],
    pub default_fan_mode: FanMode,
}

impl BoardDefinition {
    pub const fn supports(self, port: PortId) -> bool {
        self.supported_ports.contains(port)
    }

    pub const fn mux_channel(self, port: PortId) -> Option<u8> {
        self.mux_channel_by_port[port.index()]
    }

    pub const fn power_gate_bit(self, port: PortId) -> Option<u8> {
        self.power_gate_bit_by_port[port.index()]
    }

    pub const fn has_power_gates(self) -> bool {
        let mut index = 0;
        while index < MAX_PORTS {
            if self.power_gate_bit_by_port[index].is_some() {
                return true;
            }
            index += 1;
        }
        false
    }

    pub const fn power_gate_mask(self) -> u8 {
        let mut mask = 0;
        let mut index = 0;
        while index < MAX_PORTS {
            if let Some(bit) = self.power_gate_bit_by_port[index]
                && bit < 8
            {
                mask |= 1 << bit;
            }
            index += 1;
        }
        mask
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FanMode {
    ThreeWire = 0,
    FourWire = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanConfig {
    pub mode: FanMode,
    pub duty_percent: u8,
}

impl FanConfig {
    pub const SAFE_DEFAULT: Self = Self {
        mode: FanMode::ThreeWire,
        duty_percent: 100,
    };

    pub const fn validate(self) -> Result<Self, InvalidFanDuty> {
        if self.duty_percent <= 100 {
            Ok(self)
        } else {
            Err(InvalidFanDuty(self.duty_percent))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidFanDuty(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortPolicy {
    pub enabled: bool,
    pub max_voltage_mv: u16,
    pub max_current_ma: u16,
    pub max_power_mw: u32,
}

impl PortPolicy {
    pub const SAFE_DISABLED: Self = Self {
        enabled: false,
        max_voltage_mv: MAX_PORT_VOLTAGE_MV,
        max_current_ma: MAX_PORT_CURRENT_MA,
        max_power_mw: MAX_PORT_POWER_MW,
    };

    pub const fn validate(self) -> Result<Self, PolicyValidationError> {
        if self.max_voltage_mv > MAX_PORT_VOLTAGE_MV {
            return Err(PolicyValidationError::VoltageTooHigh);
        }
        if self.max_current_ma > MAX_PORT_CURRENT_MA {
            return Err(PolicyValidationError::CurrentTooHigh);
        }
        if self.max_power_mw > MAX_PORT_POWER_MW {
            return Err(PolicyValidationError::PowerTooHigh);
        }
        if self.enabled
            && (self.max_voltage_mv == 0 || self.max_current_ma == 0 || self.max_power_mw == 0)
        {
            return Err(PolicyValidationError::EnabledLimitIsZero);
        }
        if self.enabled && self.max_voltage_mv < MIN_ENABLED_PORT_VOLTAGE_MV {
            return Err(PolicyValidationError::EnabledVoltageBelowUsbMinimum);
        }
        if self.enabled && self.max_current_ma < MIN_ENABLED_PORT_CURRENT_MA {
            return Err(PolicyValidationError::EnabledCurrentNotRepresentable);
        }
        if self.enabled && self.max_power_mw < MIN_ENABLED_PORT_POWER_MW {
            return Err(PolicyValidationError::EnabledPowerNotRepresentable);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyValidationError {
    VoltageTooHigh,
    CurrentTooHigh,
    PowerTooHigh,
    EnabledLimitIsZero,
    EnabledVoltageBelowUsbMinimum,
    EnabledCurrentNotRepresentable,
    EnabledPowerNotRepresentable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentSettings {
    pub node_id: Option<NodeId>,
    pub fan: FanConfig,
    pub port_policy: [PortPolicy; MAX_PORTS],
    pub emergency_latched: bool,
}

impl PersistentSettings {
    pub const FORMAT_VERSION: u16 = 1;
    pub const FACTORY_DEFAULT: Self = Self {
        node_id: None,
        fan: FanConfig::SAFE_DEFAULT,
        port_policy: [PortPolicy::SAFE_DISABLED; MAX_PORTS],
        emergency_latched: false,
    };

    pub fn validate(self) -> Result<Self, SettingsValidationError> {
        self.fan
            .validate()
            .map_err(|error| SettingsValidationError::Fan(error.0))?;
        for (index, policy) in self.port_policy.iter().enumerate() {
            policy
                .validate()
                .map_err(|error| SettingsValidationError::PortPolicy {
                    port: u8::try_from(index).expect("MAX_PORTS fits in a u8"),
                    error,
                })?;
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsValidationError {
    Fan(u8),
    PortPolicy {
        port: u8,
        error: PolicyValidationError,
    },
}

impl Default for PersistentSettings {
    fn default() -> Self {
        Self::FACTORY_DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct FaultFlags(u32);

impl FaultFlags {
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for FaultFlags {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FaultFlags({:#010x})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_id_covers_exactly_eight_zero_based_ports() {
        for value in 0..=7 {
            assert_eq!(PortId::new(value).unwrap().get(), value);
        }
        assert_eq!(PortId::new(8), Err(InvalidPortId(8)));
    }

    #[test]
    fn bitmap_returns_each_set_port_once() {
        let mut ports = PortBitmap::from_bits(0b1010_0101);
        let mut observed = [None; 4];
        for item in &mut observed {
            *item = ports.take_lowest().map(PortId::get);
        }

        assert_eq!(observed, [Some(0), Some(2), Some(5), Some(7)]);
        assert!(ports.is_empty());
    }

    #[test]
    fn firmware_policy_enforces_every_100_w_ceiling() {
        let baseline = PortPolicy::SAFE_DISABLED;
        assert!(baseline.validate().is_ok());

        assert_eq!(
            PortPolicy {
                max_voltage_mv: 20_001,
                ..baseline
            }
            .validate(),
            Err(PolicyValidationError::VoltageTooHigh)
        );
        assert_eq!(
            PortPolicy {
                max_current_ma: 5_001,
                ..baseline
            }
            .validate(),
            Err(PolicyValidationError::CurrentTooHigh)
        );
        assert_eq!(
            PortPolicy {
                max_power_mw: 100_001,
                ..baseline
            }
            .validate(),
            Err(PolicyValidationError::PowerTooHigh)
        );
    }

    #[test]
    fn enabled_policy_must_represent_a_nonzero_five_volt_pdo() {
        let enabled = PortPolicy {
            enabled: true,
            ..PortPolicy::SAFE_DISABLED
        };
        assert_eq!(
            PortPolicy {
                max_voltage_mv: MIN_ENABLED_PORT_VOLTAGE_MV - 1,
                ..enabled
            }
            .validate(),
            Err(PolicyValidationError::EnabledVoltageBelowUsbMinimum)
        );
        assert_eq!(
            PortPolicy {
                max_current_ma: MIN_ENABLED_PORT_CURRENT_MA - 1,
                ..enabled
            }
            .validate(),
            Err(PolicyValidationError::EnabledCurrentNotRepresentable)
        );
        assert_eq!(
            PortPolicy {
                max_power_mw: MIN_ENABLED_PORT_POWER_MW - 1,
                ..enabled
            }
            .validate(),
            Err(PolicyValidationError::EnabledPowerNotRepresentable)
        );
    }

    #[test]
    fn factory_defaults_are_disabled_and_fan_full_speed() {
        let settings = PersistentSettings::FACTORY_DEFAULT;
        assert!(settings.node_id.is_none());
        assert!(!settings.emergency_latched);
        assert_eq!(settings.fan, FanConfig::SAFE_DEFAULT);
        assert!(settings.port_policy.iter().all(|policy| !policy.enabled));
    }
}
