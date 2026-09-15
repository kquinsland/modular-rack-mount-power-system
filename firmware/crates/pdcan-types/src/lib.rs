#![no_std]

use core::fmt;

pub const SHA256_DIGEST_LENGTH: usize = 32;
pub const BACKPLANE_FAN_COUNT: usize = 2;

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
pub struct UpdateSessionId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CommissioningState {
    Uncommissioned = 0,
    Claiming = 1,
    Commissioned = 2,
    AddressConflict = 3,
}

impl TryFrom<u8> for CommissioningState {
    type Error = InvalidCommissioningState;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Uncommissioned),
            1 => Ok(Self::Claiming),
            2 => Ok(Self::Commissioned),
            3 => Ok(Self::AddressConflict),
            _ => Err(InvalidCommissioningState(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCommissioningState(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NodeRole {
    Backplane = 1,
    Carrier = 2,
}

impl TryFrom<u8> for NodeRole {
    type Error = InvalidNodeRole;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Backplane),
            2 => Ok(Self::Carrier),
            _ => Err(InvalidNodeRole(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidNodeRole(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CarrierProfile {
    Sw3538 = 1,
    Basic = 2,
    Accessory = 3,
    HighPower240W = 4,
}

impl TryFrom<u8> for CarrierProfile {
    type Error = InvalidCarrierProfile;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Sw3538),
            2 => Ok(Self::Basic),
            3 => Ok(Self::Accessory),
            4 => Ok(Self::HighPower240W),
            _ => Err(InvalidCarrierProfile(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCarrierProfile(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UpdateImpact {
    /// The profile guarantees continuity of its declared service throughout
    /// staging, activation, reset, trial boot, and rollback.
    Live = 1,
    /// Staging may be live, but activation can interrupt the declared service.
    Interrupt = 2,
}

impl TryFrom<u8> for UpdateImpact {
    type Error = InvalidUpdateImpact;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Live),
            2 => Ok(Self::Interrupt),
            _ => Err(InvalidUpdateImpact(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidUpdateImpact(pub u8);

/// Capability bits are the behavioral contract. Profile names identify
/// hardware but must not be used as a substitute for capability discovery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapabilityFlags(u64);

impl CapabilityFlags {
    pub const NONE: Self = Self(0);
    pub const SWITCHABLE_OUTPUT: Self = Self(1 << 0);
    pub const LOAD_MONITORING: Self = Self(1 << 1);
    pub const USB_PD_CONTROL: Self = Self(1 << 2);
    pub const USB_PD_STATUS: Self = Self(1 << 3);
    pub const FAN_CONTROL: Self = Self(1 << 4);
    pub const FAN_TACHOMETER: Self = Self(1 << 5);
    pub const INTERNAL_TEMPERATURE: Self = Self(1 << 6);
    pub const EXTERNAL_TEMPERATURE: Self = Self(1 << 7);
    pub const AUXILIARY_IO: Self = Self(1 << 8);
    pub const AGGREGATE_POWER: Self = Self(1 << 9);
    pub const USER_BUTTON: Self = Self(1 << 10);
    pub const FIRMWARE_UPDATE: Self = Self(1 << 11);

    pub const fn from_bits_retain(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct HardwareRevision(u8);

impl HardwareRevision {
    pub const REV_A: Self = Self(1);
    pub const SIMULATOR: Self = Self(0xFE);

    pub const fn new(value: u8) -> Result<Self, InvalidHardwareRevision> {
        if value == 0 || value == 0xFF {
            Err(InvalidHardwareRevision(value))
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidHardwareRevision(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeDescriptor {
    pub role: NodeRole,
    pub carrier_profile: Option<CarrierProfile>,
    pub hardware_revision: HardwareRevision,
    pub capabilities: CapabilityFlags,
    /// Carrier profiles use this field to declare whether activation preserves
    /// their service. It is also reported by updateable backplanes.
    pub update_impact: UpdateImpact,
    pub output_count: u8,
    pub fan_count: u8,
    /// `0xFF` means that external sensor capacity is dynamically discovered.
    pub external_temperature_capacity: u8,
}

impl NodeDescriptor {
    pub const fn validate(self) -> Result<Self, InvalidNodeDescriptor> {
        match (self.role, self.carrier_profile) {
            (NodeRole::Backplane, Some(_)) => {
                return Err(InvalidNodeDescriptor::BackplaneHasCarrierProfile);
            }
            (NodeRole::Carrier, None) => {
                return Err(InvalidNodeDescriptor::CarrierMissingProfile);
            }
            _ => {}
        }

        if !self.capabilities.contains(CapabilityFlags::FIRMWARE_UPDATE) {
            return Err(InvalidNodeDescriptor::FirmwareUpdateRequired);
        }
        match self.role {
            NodeRole::Backplane if self.output_count != 0 => {
                return Err(InvalidNodeDescriptor::BackplaneHasOutputs);
            }
            NodeRole::Carrier if self.fan_count != 0 => {
                return Err(InvalidNodeDescriptor::CarrierHasBackplaneFans);
            }
            _ => {}
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidNodeDescriptor {
    BackplaneHasCarrierProfile,
    CarrierMissingProfile,
    FirmwareUpdateRequired,
    BackplaneHasOutputs,
    CarrierHasBackplaneFans,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl FirmwareVersion {
    pub const ZERO: Self = Self {
        major: 0,
        minor: 0,
        patch: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SlotIndex(u8);

impl SlotIndex {
    pub const MAX: u8 = 0xFE;

    pub const fn new(value: u8) -> Result<Self, InvalidSlotIndex> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(InvalidSlotIndex(value))
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidSlotIndex(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierBinding {
    pub backplane_uid: NodeUid,
    pub slot: SlotIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct FanId(u8);

impl FanId {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 1;

    pub const fn new(value: u8) -> Result<Self, InvalidFanId> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(InvalidFanId(value))
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
pub struct InvalidFanId(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanConfig {
    pub duty_percent: u8,
}

impl FanConfig {
    pub const SAFE_DEFAULT: Self = Self { duty_percent: 100 };

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
pub struct PdLimits {
    pub max_voltage_mv: u32,
    pub max_current_ma: u32,
    pub max_power_mw: u32,
}

impl PdLimits {
    pub const SW3538_SAFE_MAX: Self = Self {
        max_voltage_mv: 20_000,
        max_current_ma: 5_000,
        max_power_mw: 100_000,
    };

    pub const fn validate_for(self, ceiling: Self) -> Result<Self, InvalidPdLimits> {
        if self.max_voltage_mv == 0 || self.max_voltage_mv > ceiling.max_voltage_mv {
            return Err(InvalidPdLimits::Voltage);
        }
        if self.max_current_ma == 0 || self.max_current_ma > ceiling.max_current_ma {
            return Err(InvalidPdLimits::Current);
        }
        if self.max_power_mw == 0 || self.max_power_mw > ceiling.max_power_mw {
            return Err(InvalidPdLimits::Power);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidPdLimits {
    Voltage,
    Current,
    Power,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierPolicy {
    pub enabled: bool,
    /// Absent for carriers that do not expose negotiated USB-PD limits.
    pub pd_limits: Option<PdLimits>,
}

impl CarrierPolicy {
    pub const SW3538_SAFE_DISABLED: Self = Self {
        enabled: false,
        pd_limits: Some(PdLimits::SW3538_SAFE_MAX),
    };

    pub fn validate_for(
        self,
        descriptor: NodeDescriptor,
        ceiling: Option<PdLimits>,
    ) -> Result<Self, InvalidCarrierPolicy> {
        if !matches!(descriptor.role, NodeRole::Carrier) {
            return Err(InvalidCarrierPolicy::NotCarrier);
        }
        let supports_pd = descriptor
            .capabilities
            .contains(CapabilityFlags::USB_PD_CONTROL);
        match (supports_pd, self.pd_limits, ceiling) {
            (true, Some(limits), Some(maximum)) => limits
                .validate_for(maximum)
                .map(|_| self)
                .map_err(InvalidCarrierPolicy::Limits),
            (true, _, _) => Err(InvalidCarrierPolicy::PdLimitsRequired),
            (false, None, _) => Ok(self),
            (false, Some(_), _) => Err(InvalidCarrierPolicy::PdLimitsUnsupported),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCarrierPolicy {
    NotCarrier,
    PdLimitsRequired,
    PdLimitsUnsupported,
    Limits(InvalidPdLimits),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FaultFlags(u32);

impl FaultFlags {
    pub const NONE: Self = Self(0);
    pub const EMERGENCY_LATCHED: Self = Self(1 << 0);
    pub const HOUSEKEEPING_POWER_BAD: Self = Self(1 << 1);
    pub const OUTPUT_POWER_BAD: Self = Self(1 << 2);
    pub const POWER_MONITOR: Self = Self(1 << 3);
    pub const LOCAL_BUS: Self = Self(1 << 4);
    pub const PD_CONTROLLER: Self = Self(1 << 5);
    pub const FAN_STALLED: Self = Self(1 << 6);
    pub const TEMPERATURE: Self = Self(1 << 7);
    pub const CONFIGURATION: Self = Self(1 << 8);
    pub const UPDATE: Self = Self(1 << 9);
    pub const WATCHDOG: Self = Self(1 << 10);

    pub const fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ServiceState {
    Starting = 0,
    Disabled = 1,
    Enabling = 2,
    Enabled = 3,
    Faulted = 4,
    Updating = 5,
}

impl TryFrom<u8> for ServiceState {
    type Error = InvalidServiceState;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Starting),
            1 => Ok(Self::Disabled),
            2 => Ok(Self::Enabling),
            3 => Ok(Self::Enabled),
            4 => Ok(Self::Faulted),
            5 => Ok(Self::Updating),
            _ => Err(InvalidServiceState(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidServiceState(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PowerSource {
    BackplaneInput = 1,
    CarrierInput = 2,
}

impl TryFrom<u8> for PowerSource {
    type Error = InvalidPowerSource;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::BackplaneInput),
            2 => Ok(Self::CarrierInput),
            _ => Err(InvalidPowerSource(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidPowerSource(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerReading {
    pub source: PowerSource,
    pub sequence: u16,
    pub voltage_mv: u32,
    pub current_ma: i32,
    pub power_mw: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TemperatureSource {
    Stm32Die = 1,
    Ina237Die = 2,
    OneWire = 3,
}

impl TryFrom<u8> for TemperatureSource {
    type Error = InvalidTemperatureSource;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Stm32Die),
            2 => Ok(Self::Ina237Die),
            3 => Ok(Self::OneWire),
            _ => Err(InvalidTemperatureSource(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidTemperatureSource(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemperatureSensorId {
    pub source: TemperatureSource,
    /// Zero for board-local die sensors; the full ROM code for 1-Wire sensors.
    pub identifier: u64,
}

impl TemperatureSensorId {
    pub const STM32_DIE: Self = Self {
        source: TemperatureSource::Stm32Die,
        identifier: 0,
    };
    pub const INA237_DIE: Self = Self {
        source: TemperatureSource::Ina237Die,
        identifier: 0,
    };

    pub const fn one_wire(rom_code: u64) -> Self {
        Self {
            source: TemperatureSource::OneWire,
            identifier: rom_code,
        }
    }

    pub const fn validate(self) -> Result<Self, InvalidTemperatureSensorId> {
        match (self.source, self.identifier) {
            (TemperatureSource::Stm32Die | TemperatureSource::Ina237Die, 0)
            | (TemperatureSource::OneWire, 1..=u64::MAX) => Ok(self),
            _ => Err(InvalidTemperatureSensorId),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidTemperatureSensorId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SampleStatus {
    Valid = 0,
    Stale = 1,
    Unavailable = 2,
    Invalid = 3,
}

impl TryFrom<u8> for SampleStatus {
    type Error = InvalidSampleStatus;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Valid),
            1 => Ok(Self::Stale),
            2 => Ok(Self::Unavailable),
            3 => Ok(Self::Invalid),
            _ => Err(InvalidSampleStatus(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidSampleStatus(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemperatureReading {
    pub sensor: TemperatureSensorId,
    pub sequence: u16,
    pub status: SampleStatus,
    pub temperature_centi_c: Option<i16>,
}

impl TemperatureReading {
    pub const fn validate(self) -> Result<Self, InvalidTemperatureReading> {
        if self.sensor.validate().is_err() {
            return Err(InvalidTemperatureReading::Sensor);
        }
        match (self.status, self.temperature_centi_c) {
            (SampleStatus::Valid | SampleStatus::Stale, Some(_))
            | (SampleStatus::Unavailable | SampleStatus::Invalid, None) => Ok(self),
            (SampleStatus::Valid | SampleStatus::Stale, None) => {
                Err(InvalidTemperatureReading::ValueMissing)
            }
            (SampleStatus::Unavailable | SampleStatus::Invalid, Some(_)) => {
                Err(InvalidTemperatureReading::UnexpectedValue)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidTemperatureReading {
    Sensor,
    ValueMissing,
    UnexpectedValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256Digest([u8; SHA256_DIGEST_LENGTH]);

impl Sha256Digest {
    pub const fn from_bytes(bytes: [u8; SHA256_DIGEST_LENGTH]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; SHA256_DIGEST_LENGTH] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageTarget {
    pub role: NodeRole,
    pub carrier_profile: Option<CarrierProfile>,
    pub hardware_revision: HardwareRevision,
    pub partition_layout: u8,
}

impl ImageTarget {
    pub const fn validate(self) -> Result<Self, InvalidImageTarget> {
        match (self.role, self.carrier_profile) {
            (NodeRole::Backplane, None) | (NodeRole::Carrier, Some(_)) => Ok(self),
            _ => Err(InvalidImageTarget),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidImageTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpdateManifest {
    pub target: ImageTarget,
    pub version: FirmwareVersion,
    pub image_size: u32,
    pub digest: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UpdateState {
    Idle = 0,
    Receiving = 1,
    Verifying = 2,
    Staged = 3,
    Activating = 4,
    Trial = 5,
    Confirmed = 6,
    Rollback = 7,
    Failed = 8,
}

impl TryFrom<u8> for UpdateState {
    type Error = InvalidUpdateState;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Idle),
            1 => Ok(Self::Receiving),
            2 => Ok(Self::Verifying),
            3 => Ok(Self::Staged),
            4 => Ok(Self::Activating),
            5 => Ok(Self::Trial),
            6 => Ok(Self::Confirmed),
            7 => Ok(Self::Rollback),
            8 => Ok(Self::Failed),
            _ => Err(InvalidUpdateState(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidUpdateState(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UpdateError {
    None = 0,
    Busy = 1,
    InvalidManifest = 2,
    WrongTarget = 3,
    Oversize = 4,
    Offset = 5,
    ConflictingData = 6,
    Storage = 7,
    Digest = 8,
    NotStaged = 9,
    InterruptionNotAuthorized = 10,
    SafetyFault = 11,
    Internal = 12,
}

impl TryFrom<u8> for UpdateError {
    type Error = InvalidUpdateError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Busy),
            2 => Ok(Self::InvalidManifest),
            3 => Ok(Self::WrongTarget),
            4 => Ok(Self::Oversize),
            5 => Ok(Self::Offset),
            6 => Ok(Self::ConflictingData),
            7 => Ok(Self::Storage),
            8 => Ok(Self::Digest),
            9 => Ok(Self::NotStaged),
            10 => Ok(Self::InterruptionNotAuthorized),
            11 => Ok(Self::SafetyFault),
            12 => Ok(Self::Internal),
            _ => Err(InvalidUpdateError(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidUpdateError(pub u8);

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATEABLE: CapabilityFlags =
        CapabilityFlags::FIRMWARE_UPDATE.union(CapabilityFlags::INTERNAL_TEMPERATURE);

    fn sw3538_descriptor() -> NodeDescriptor {
        NodeDescriptor {
            role: NodeRole::Carrier,
            carrier_profile: Some(CarrierProfile::Sw3538),
            hardware_revision: HardwareRevision::REV_A,
            capabilities: UPDATEABLE
                .union(CapabilityFlags::SWITCHABLE_OUTPUT)
                .union(CapabilityFlags::LOAD_MONITORING)
                .union(CapabilityFlags::USB_PD_CONTROL)
                .union(CapabilityFlags::USB_PD_STATUS)
                .union(CapabilityFlags::USER_BUTTON),
            update_impact: UpdateImpact::Interrupt,
            output_count: 1,
            fan_count: 0,
            external_temperature_capacity: 0,
        }
    }

    #[test]
    fn finalized_profiles_are_distinct_and_sw3538_interrupts_on_activation() {
        let descriptor = sw3538_descriptor();
        assert_eq!(descriptor.validate(), Ok(descriptor));
        assert_eq!(descriptor.update_impact, UpdateImpact::Interrupt);
        assert_eq!(descriptor.carrier_profile, Some(CarrierProfile::Sw3538));
        assert!(
            descriptor
                .capabilities
                .contains(CapabilityFlags::FIRMWARE_UPDATE)
        );
    }

    #[test]
    fn role_and_profile_relationship_is_strict() {
        let invalid = NodeDescriptor {
            role: NodeRole::Backplane,
            carrier_profile: Some(CarrierProfile::Accessory),
            ..sw3538_descriptor()
        };
        assert_eq!(
            invalid.validate(),
            Err(InvalidNodeDescriptor::BackplaneHasCarrierProfile)
        );
    }

    #[test]
    fn sw3538_policy_enforces_the_current_hardware_ceiling() {
        let valid = CarrierPolicy {
            enabled: true,
            pd_limits: Some(PdLimits::SW3538_SAFE_MAX),
        };
        assert_eq!(
            valid.validate_for(sw3538_descriptor(), Some(PdLimits::SW3538_SAFE_MAX)),
            Ok(valid)
        );

        let excessive = CarrierPolicy {
            pd_limits: Some(PdLimits {
                max_power_mw: 100_001,
                ..PdLimits::SW3538_SAFE_MAX
            }),
            ..valid
        };
        assert_eq!(
            excessive.validate_for(sw3538_descriptor(), Some(PdLimits::SW3538_SAFE_MAX)),
            Err(InvalidCarrierPolicy::Limits(InvalidPdLimits::Power))
        );
    }

    #[test]
    fn temperature_identity_and_validity_cannot_be_conflated() {
        let valid = TemperatureReading {
            sensor: TemperatureSensorId::one_wire(0x28AA_0102_0304_056B),
            sequence: 3,
            status: SampleStatus::Valid,
            temperature_centi_c: Some(2_550),
        };
        assert_eq!(valid.validate(), Ok(valid));

        let missing = TemperatureReading {
            temperature_centi_c: None,
            ..valid
        };
        assert_eq!(
            missing.validate(),
            Err(InvalidTemperatureReading::ValueMissing)
        );
        assert!(TemperatureSensorId::one_wire(0).validate().is_err());
    }

    #[test]
    fn binding_uses_a_full_uid_and_zero_based_slot() {
        let binding = CarrierBinding {
            backplane_uid: NodeUid::from_bytes([0xA5; 12]),
            slot: SlotIndex::new(0).unwrap(),
        };
        assert_eq!(binding.slot.get(), 0);
        assert!(SlotIndex::new(0xFF).is_err());
    }
}
