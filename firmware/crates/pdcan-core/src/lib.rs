#![no_std]

use pdcan_types::{
    BACKPLANE_FAN_COUNT, CarrierPolicy, CommissioningState, FanConfig, FanId, FaultFlags,
    ImageTarget, NodeDescriptor, NodeId, NodeRole, NodeUid, PdLimits, ServiceState, UpdateError,
    UpdateImpact, UpdateManifest, UpdateSessionId, UpdateState,
};

pub const ACTIVE_APPLICATION_BYTES: u32 = 110 * 1024;

/// Pure duplicate-address and commissioning state. Persistence is deliberately
/// owned by the board application; this state machine decides only when normal
/// Node-ID traffic is safe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Commissioning {
    uid: NodeUid,
    node_id: Option<NodeId>,
    state: CommissioningState,
    conflicting_uid: Option<NodeUid>,
}

impl Commissioning {
    pub const fn new(uid: NodeUid, node_id: Option<NodeId>) -> Self {
        Self {
            uid,
            node_id,
            state: if node_id.is_some() {
                CommissioningState::Claiming
            } else {
                CommissioningState::Uncommissioned
            },
            conflicting_uid: None,
        }
    }

    pub const fn uid(&self) -> NodeUid {
        self.uid
    }

    pub const fn node_id(&self) -> Option<NodeId> {
        self.node_id
    }

    pub const fn state(&self) -> CommissioningState {
        self.state
    }

    pub const fn conflicting_uid(&self) -> Option<NodeUid> {
        self.conflicting_uid
    }

    pub const fn normal_traffic_allowed(&self) -> bool {
        matches!(self.state, CommissioningState::Commissioned)
    }

    pub fn observe_claim(&mut self, uid: NodeUid, node_id: NodeId) {
        if self.node_id == Some(node_id) && uid != self.uid {
            self.state = CommissioningState::AddressConflict;
            self.conflicting_uid = Some(uid);
        }
    }

    pub fn claim_window_complete(&mut self) {
        if self.state == CommissioningState::Claiming {
            self.state = CommissioningState::Commissioned;
        }
    }

    pub fn apply_persisted_node_id(&mut self, node_id: Option<NodeId>) {
        self.node_id = node_id;
        self.conflicting_uid = None;
        self.state = if node_id.is_some() {
            CommissioningState::Claiming
        } else {
            CommissioningState::Uncommissioned
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierSettings {
    pub policy: CarrierPolicy,
    pub emergency_latched: bool,
}

impl CarrierSettings {
    pub const SW3538_FACTORY_DEFAULT: Self = Self {
        policy: CarrierPolicy::SW3538_SAFE_DISABLED,
        emergency_latched: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierAction {
    PersistSettings(CarrierSettings),
    DriveOutput(bool),
    DriveOutputThenPersistEmergency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CarrierError {
    InvalidDescriptor,
    InvalidPolicy,
    EmergencyLatched,
    EmergencyAlreadyClear,
    HardwareNotReady,
}

/// Safety state for a single finalized carrier node. The output defaults off;
/// persisted enable is restored only after the board application has completed
/// its UID-derived startup delay and reported healthy local hardware.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarrierController {
    descriptor: NodeDescriptor,
    ceiling: Option<PdLimits>,
    settings: CarrierSettings,
    service: ServiceState,
    faults: FaultFlags,
    housekeeping_good: bool,
    output_power_good: bool,
}

impl CarrierController {
    pub fn new(
        descriptor: NodeDescriptor,
        ceiling: Option<PdLimits>,
        settings: CarrierSettings,
    ) -> Result<Self, CarrierError> {
        descriptor
            .validate()
            .map_err(|_| CarrierError::InvalidDescriptor)?;
        if !matches!(descriptor.role, NodeRole::Carrier) {
            return Err(CarrierError::InvalidDescriptor);
        }
        settings
            .policy
            .validate_for(descriptor, ceiling)
            .map_err(|_| CarrierError::InvalidPolicy)?;
        Ok(Self {
            descriptor,
            ceiling,
            settings,
            service: ServiceState::Starting,
            faults: if settings.emergency_latched {
                FaultFlags::EMERGENCY_LATCHED
            } else {
                FaultFlags::NONE
            },
            housekeeping_good: false,
            output_power_good: false,
        })
    }

    pub const fn descriptor(&self) -> NodeDescriptor {
        self.descriptor
    }

    pub const fn settings(&self) -> CarrierSettings {
        self.settings
    }

    pub const fn service(&self) -> ServiceState {
        self.service
    }

    pub const fn faults(&self) -> FaultFlags {
        self.faults
    }

    pub const fn output_power_good(&self) -> bool {
        self.output_power_good
    }

    pub fn set_housekeeping_good(&mut self, good: bool) {
        self.housekeeping_good = good;
        if good {
            if self.service == ServiceState::Starting {
                self.service = ServiceState::Disabled;
            }
        } else {
            self.faults = self.faults.union(FaultFlags::HOUSEKEEPING_POWER_BAD);
            self.service = ServiceState::Faulted;
        }
    }

    pub fn restore_after_startup_delay(&mut self) -> Result<Option<CarrierAction>, CarrierError> {
        if !self.housekeeping_good {
            return Err(CarrierError::HardwareNotReady);
        }
        if self.settings.emergency_latched {
            return Err(CarrierError::EmergencyLatched);
        }
        if self.settings.policy.enabled {
            self.service = ServiceState::Enabling;
            Ok(Some(CarrierAction::DriveOutput(true)))
        } else {
            self.service = ServiceState::Disabled;
            Ok(None)
        }
    }

    pub fn set_policy(&mut self, policy: CarrierPolicy) -> Result<CarrierAction, CarrierError> {
        policy
            .validate_for(self.descriptor, self.ceiling)
            .map_err(|_| CarrierError::InvalidPolicy)?;
        if policy.enabled && self.settings.emergency_latched {
            return Err(CarrierError::EmergencyLatched);
        }
        self.settings.policy = policy;
        self.service = if policy.enabled {
            ServiceState::Enabling
        } else {
            ServiceState::Disabled
        };
        Ok(CarrierAction::PersistSettings(self.settings))
    }

    pub fn policy_persisted(&self) -> CarrierAction {
        CarrierAction::DriveOutput(self.settings.policy.enabled)
    }

    pub fn observe_output_power_good(&mut self, good: bool) {
        self.output_power_good = good;
        if good && self.service == ServiceState::Enabling {
            self.service = ServiceState::Enabled;
        } else if !good && self.service == ServiceState::Enabled {
            self.faults = self.faults.union(FaultFlags::OUTPUT_POWER_BAD);
            self.service = ServiceState::Faulted;
        }
    }

    pub fn emergency_disable(&mut self) -> CarrierAction {
        self.settings.emergency_latched = true;
        self.settings.policy.enabled = false;
        self.output_power_good = false;
        self.faults = self.faults.union(FaultFlags::EMERGENCY_LATCHED);
        self.service = ServiceState::Disabled;
        CarrierAction::DriveOutputThenPersistEmergency
    }

    pub fn acknowledge_emergency(&mut self) -> Result<CarrierAction, CarrierError> {
        if !self.settings.emergency_latched {
            return Err(CarrierError::EmergencyAlreadyClear);
        }
        self.settings.emergency_latched = false;
        self.faults = FaultFlags::from_bits_retain(
            self.faults.bits() & !FaultFlags::EMERGENCY_LATCHED.bits(),
        );
        Ok(CarrierAction::PersistSettings(self.settings))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackplaneSettings {
    pub fans: [FanConfig; BACKPLANE_FAN_COUNT],
    pub emergency_latched: bool,
}

impl BackplaneSettings {
    pub const FACTORY_DEFAULT: Self = Self {
        fans: [FanConfig::SAFE_DEFAULT; BACKPLANE_FAN_COUNT],
        emergency_latched: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackplaneAction {
    PersistSettings(BackplaneSettings),
    DriveFan { fan: FanId, config: FanConfig },
    DriveAllFansSafeThenPersistEmergency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackplaneError {
    InvalidDescriptor,
    InvalidFanDuty,
    EmergencyAlreadyClear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackplaneController {
    descriptor: NodeDescriptor,
    settings: BackplaneSettings,
    faults: FaultFlags,
}

impl BackplaneController {
    pub fn new(
        descriptor: NodeDescriptor,
        settings: BackplaneSettings,
    ) -> Result<Self, BackplaneError> {
        descriptor
            .validate()
            .map_err(|_| BackplaneError::InvalidDescriptor)?;
        if !matches!(descriptor.role, NodeRole::Backplane)
            || usize::from(descriptor.fan_count) != BACKPLANE_FAN_COUNT
        {
            return Err(BackplaneError::InvalidDescriptor);
        }
        for fan in settings.fans {
            fan.validate().map_err(|_| BackplaneError::InvalidFanDuty)?;
        }
        Ok(Self {
            descriptor,
            settings,
            faults: if settings.emergency_latched {
                FaultFlags::EMERGENCY_LATCHED
            } else {
                FaultFlags::NONE
            },
        })
    }

    pub const fn descriptor(&self) -> NodeDescriptor {
        self.descriptor
    }

    pub const fn settings(&self) -> BackplaneSettings {
        self.settings
    }

    pub const fn faults(&self) -> FaultFlags {
        self.faults
    }

    pub fn set_fan(
        &mut self,
        fan: FanId,
        config: FanConfig,
    ) -> Result<BackplaneAction, BackplaneError> {
        config
            .validate()
            .map_err(|_| BackplaneError::InvalidFanDuty)?;
        self.settings.fans[fan.index()] = config;
        Ok(BackplaneAction::PersistSettings(self.settings))
    }

    pub fn fan_settings_persisted(&self, fan: FanId) -> BackplaneAction {
        BackplaneAction::DriveFan {
            fan,
            config: self.settings.fans[fan.index()],
        }
    }

    pub fn emergency_disable(&mut self) -> BackplaneAction {
        self.settings.emergency_latched = true;
        self.settings.fans = [FanConfig::SAFE_DEFAULT; BACKPLANE_FAN_COUNT];
        self.faults = self.faults.union(FaultFlags::EMERGENCY_LATCHED);
        BackplaneAction::DriveAllFansSafeThenPersistEmergency
    }

    pub fn acknowledge_emergency(&mut self) -> Result<BackplaneAction, BackplaneError> {
        if !self.settings.emergency_latched {
            return Err(BackplaneError::EmergencyAlreadyClear);
        }
        self.settings.emergency_latched = false;
        self.faults = FaultFlags::from_bits_retain(
            self.faults.bits() & !FaultFlags::EMERGENCY_LATCHED.bits(),
        );
        Ok(BackplaneAction::PersistSettings(self.settings))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkDisposition {
    Write,
    Duplicate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationPlan {
    pub disable_service: bool,
}

/// Transport-independent update sequencing. Flash writes, hashing, and
/// Embassy Boot calls remain owned by the board application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpdateController {
    target: ImageTarget,
    impact: UpdateImpact,
    state: UpdateState,
    error: UpdateError,
    session: UpdateSessionId,
    manifest: Option<UpdateManifest>,
    next_offset: u32,
}

impl UpdateController {
    pub const fn new(target: ImageTarget, impact: UpdateImpact) -> Self {
        Self {
            target,
            impact,
            state: UpdateState::Idle,
            error: UpdateError::None,
            session: UpdateSessionId(0),
            manifest: None,
            next_offset: 0,
        }
    }

    pub const fn state(&self) -> UpdateState {
        self.state
    }

    pub const fn error(&self) -> UpdateError {
        self.error
    }

    pub const fn session(&self) -> UpdateSessionId {
        self.session
    }

    pub const fn manifest(&self) -> Option<UpdateManifest> {
        self.manifest
    }

    pub const fn next_offset(&self) -> u32 {
        self.next_offset
    }

    pub fn begin(
        &mut self,
        session: UpdateSessionId,
        manifest: UpdateManifest,
    ) -> Result<(), UpdateError> {
        if !matches!(self.state, UpdateState::Idle | UpdateState::Failed) {
            return self.reject(UpdateError::Busy);
        }
        if manifest.target != self.target {
            return self.fail(UpdateError::WrongTarget);
        }
        if manifest.image_size == 0 || manifest.image_size > ACTIVE_APPLICATION_BYTES {
            return self.fail(UpdateError::Oversize);
        }
        self.session = session;
        self.manifest = Some(manifest);
        self.next_offset = 0;
        self.error = UpdateError::None;
        self.state = UpdateState::Receiving;
        Ok(())
    }

    pub fn accept_chunk(
        &mut self,
        session: UpdateSessionId,
        offset: u32,
        len: u8,
    ) -> Result<ChunkDisposition, UpdateError> {
        if self.state != UpdateState::Receiving || session != self.session {
            return self.reject(UpdateError::Busy);
        }
        let Some(manifest) = self.manifest else {
            return self.fail(UpdateError::InvalidManifest);
        };
        let end = offset
            .checked_add(u32::from(len))
            .ok_or(UpdateError::Offset)
            .or_else(|error| self.reject(error))?;
        if len == 0 || end > manifest.image_size {
            return self.reject(UpdateError::Offset);
        }
        let disposition = if offset == self.next_offset {
            self.next_offset = end;
            ChunkDisposition::Write
        } else if end <= self.next_offset {
            // The storage owner must compare duplicate bytes with DFU before
            // acknowledging them. The sequencer deliberately does not claim
            // that an offset/length match proves the content is identical.
            ChunkDisposition::Duplicate
        } else {
            return self.reject(UpdateError::Offset);
        };
        self.error = UpdateError::None;
        Ok(disposition)
    }

    pub fn begin_verification(
        &mut self,
        session: UpdateSessionId,
    ) -> Result<UpdateManifest, UpdateError> {
        if self.state != UpdateState::Receiving || session != self.session {
            return self.reject(UpdateError::Busy);
        }
        let Some(manifest) = self.manifest else {
            return self.fail(UpdateError::InvalidManifest);
        };
        if self.next_offset != manifest.image_size {
            return self.reject(UpdateError::Offset);
        }
        self.state = UpdateState::Verifying;
        Ok(manifest)
    }

    pub fn verification_complete(&mut self, valid: bool) -> Result<(), UpdateError> {
        if self.state != UpdateState::Verifying {
            return self.fail(UpdateError::Busy);
        }
        if !valid {
            return self.fail(UpdateError::Digest);
        }
        self.state = UpdateState::Staged;
        self.error = UpdateError::None;
        Ok(())
    }

    pub fn activate(
        &mut self,
        session: UpdateSessionId,
        allow_interruption: bool,
    ) -> Result<ActivationPlan, UpdateError> {
        if self.state != UpdateState::Staged || session != self.session {
            return self.reject(UpdateError::NotStaged);
        }
        if self.impact == UpdateImpact::Interrupt && !allow_interruption {
            return self.reject(UpdateError::InterruptionNotAuthorized);
        }
        self.state = UpdateState::Activating;
        self.error = UpdateError::None;
        Ok(ActivationPlan {
            disable_service: self.impact == UpdateImpact::Interrupt,
        })
    }

    pub fn abort(&mut self, session: UpdateSessionId) -> Result<(), UpdateError> {
        if session != self.session
            || matches!(self.state, UpdateState::Activating | UpdateState::Trial)
        {
            return self.reject(UpdateError::Busy);
        }
        self.state = UpdateState::Idle;
        self.error = UpdateError::None;
        self.session = UpdateSessionId(0);
        self.manifest = None;
        self.next_offset = 0;
        Ok(())
    }

    fn fail<T>(&mut self, error: UpdateError) -> Result<T, UpdateError> {
        self.error = error;
        self.state = UpdateState::Failed;
        Err(error)
    }

    fn reject<T>(&mut self, error: UpdateError) -> Result<T, UpdateError> {
        self.error = error;
        Err(error)
    }
}

/// A deterministic delay in the inclusive 100-899 ms range used to stagger
/// carrier power restoration after a shared supply starts.
pub fn carrier_startup_delay_ms(uid: NodeUid) -> u16 {
    let mut hash = 0x811C_9DC5_u32;
    for byte in uid.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    100 + u16::try_from(hash % 800).expect("modulo 800 fits u16")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_types::{
        CapabilityFlags, CarrierProfile, FirmwareVersion, HardwareRevision, Sha256Digest,
    };

    const CARRIER_CAPABILITIES: CapabilityFlags = CapabilityFlags::FIRMWARE_UPDATE
        .union(CapabilityFlags::SWITCHABLE_OUTPUT)
        .union(CapabilityFlags::LOAD_MONITORING)
        .union(CapabilityFlags::USB_PD_CONTROL);
    const BACKPLANE_CAPABILITIES: CapabilityFlags = CapabilityFlags::FIRMWARE_UPDATE
        .union(CapabilityFlags::FAN_CONTROL)
        .union(CapabilityFlags::FAN_TACHOMETER)
        .union(CapabilityFlags::AGGREGATE_POWER);

    fn carrier_descriptor() -> NodeDescriptor {
        NodeDescriptor {
            role: NodeRole::Carrier,
            carrier_profile: Some(CarrierProfile::Sw3538),
            hardware_revision: HardwareRevision::REV_A,
            capabilities: CARRIER_CAPABILITIES,
            update_impact: UpdateImpact::Interrupt,
            output_count: 1,
            fan_count: 0,
            external_temperature_capacity: 0,
        }
    }

    fn backplane_descriptor() -> NodeDescriptor {
        NodeDescriptor {
            role: NodeRole::Backplane,
            carrier_profile: None,
            hardware_revision: HardwareRevision::REV_A,
            capabilities: BACKPLANE_CAPABILITIES,
            update_impact: UpdateImpact::Interrupt,
            output_count: 0,
            fan_count: 2,
            external_temperature_capacity: 0xFF,
        }
    }

    fn manifest() -> UpdateManifest {
        UpdateManifest {
            target: ImageTarget {
                role: NodeRole::Carrier,
                carrier_profile: Some(CarrierProfile::Sw3538),
                hardware_revision: HardwareRevision::REV_A,
                partition_layout: 1,
            },
            version: FirmwareVersion {
                major: 2,
                minor: 0,
                patch: 0,
            },
            image_size: 96,
            digest: Sha256Digest::from_bytes([0xA5; 32]),
        }
    }

    #[test]
    fn duplicate_node_claims_suppress_normal_traffic() {
        let uid = NodeUid::from_bytes([1; 12]);
        let other = NodeUid::from_bytes([2; 12]);
        let id = NodeId::new(7).unwrap();
        let mut commissioning = Commissioning::new(uid, Some(id));
        commissioning.claim_window_complete();
        assert!(commissioning.normal_traffic_allowed());
        commissioning.observe_claim(other, id);
        assert!(!commissioning.normal_traffic_allowed());
        assert_eq!(commissioning.conflicting_uid(), Some(other));
    }

    #[test]
    fn carrier_output_is_off_until_health_and_stagger_complete() {
        let mut carrier = CarrierController::new(
            carrier_descriptor(),
            Some(PdLimits::SW3538_SAFE_MAX),
            CarrierSettings {
                policy: CarrierPolicy {
                    enabled: true,
                    pd_limits: Some(PdLimits::SW3538_SAFE_MAX),
                },
                emergency_latched: false,
            },
        )
        .unwrap();
        assert_eq!(carrier.service(), ServiceState::Starting);
        assert_eq!(
            carrier.restore_after_startup_delay(),
            Err(CarrierError::HardwareNotReady)
        );
        carrier.set_housekeeping_good(true);
        assert_eq!(
            carrier.restore_after_startup_delay(),
            Ok(Some(CarrierAction::DriveOutput(true)))
        );
    }

    #[test]
    fn emergency_disables_carrier_and_forces_backplane_cooling_safe() {
        let mut carrier = CarrierController::new(
            carrier_descriptor(),
            Some(PdLimits::SW3538_SAFE_MAX),
            CarrierSettings::SW3538_FACTORY_DEFAULT,
        )
        .unwrap();
        assert_eq!(
            carrier.emergency_disable(),
            CarrierAction::DriveOutputThenPersistEmergency
        );
        assert!(carrier.settings().emergency_latched);

        let mut backplane =
            BackplaneController::new(backplane_descriptor(), BackplaneSettings::FACTORY_DEFAULT)
                .unwrap();
        assert_eq!(
            backplane.emergency_disable(),
            BackplaneAction::DriveAllFansSafeThenPersistEmergency
        );
        assert_eq!(
            backplane.settings().fans,
            [FanConfig::SAFE_DEFAULT; BACKPLANE_FAN_COUNT]
        );
    }

    #[test]
    fn update_is_sequential_and_partial_images_never_stage() {
        let mut update = UpdateController::new(manifest().target, UpdateImpact::Interrupt);
        let session = UpdateSessionId(5);
        update.begin(session, manifest()).unwrap();
        assert_eq!(
            update.accept_chunk(session, 0, 48),
            Ok(ChunkDisposition::Write)
        );
        assert_eq!(
            update.accept_chunk(session, 0, 48),
            Ok(ChunkDisposition::Duplicate)
        );
        assert_eq!(update.begin_verification(session), Err(UpdateError::Offset));
        assert_eq!(update.state(), UpdateState::Receiving);
        assert_eq!(update.next_offset(), 48);
        assert_eq!(
            update.accept_chunk(session, 48, 48),
            Ok(ChunkDisposition::Write)
        );
        assert_eq!(update.error(), UpdateError::None);
    }

    #[test]
    fn gaps_and_stale_sessions_are_rejected_without_destroying_resume_state() {
        let mut update = UpdateController::new(manifest().target, UpdateImpact::Interrupt);
        let session = UpdateSessionId(5);
        update.begin(session, manifest()).unwrap();
        update.accept_chunk(session, 0, 48).unwrap();

        assert_eq!(
            update.accept_chunk(session, 72, 24),
            Err(UpdateError::Offset)
        );
        assert_eq!(update.state(), UpdateState::Receiving);
        assert_eq!(update.next_offset(), 48);
        assert_eq!(
            update.accept_chunk(UpdateSessionId(6), 48, 48),
            Err(UpdateError::Busy)
        );
        assert_eq!(update.state(), UpdateState::Receiving);
        assert_eq!(update.next_offset(), 48);
    }

    #[test]
    fn interrupt_profiles_require_explicit_activation_authority() {
        let mut update = UpdateController::new(manifest().target, UpdateImpact::Interrupt);
        let session = UpdateSessionId(8);
        update.begin(session, manifest()).unwrap();
        update.accept_chunk(session, 0, 48).unwrap();
        update.accept_chunk(session, 48, 48).unwrap();
        update.begin_verification(session).unwrap();
        update.verification_complete(true).unwrap();
        assert_eq!(
            update.activate(session, false),
            Err(UpdateError::InterruptionNotAuthorized)
        );
        assert_eq!(update.state(), UpdateState::Staged);
        assert_eq!(
            update.activate(session, true),
            Ok(ActivationPlan {
                disable_service: true
            })
        );
    }

    #[test]
    fn wrong_role_images_and_oversized_images_are_rejected() {
        let mut update = UpdateController::new(manifest().target, UpdateImpact::Interrupt);
        let mut wrong = manifest();
        wrong.target.role = NodeRole::Backplane;
        wrong.target.carrier_profile = None;
        assert_eq!(
            update.begin(UpdateSessionId(1), wrong),
            Err(UpdateError::WrongTarget)
        );

        let mut update = UpdateController::new(manifest().target, UpdateImpact::Interrupt);
        let mut oversized = manifest();
        oversized.image_size = ACTIVE_APPLICATION_BYTES + 1;
        assert_eq!(
            update.begin(UpdateSessionId(2), oversized),
            Err(UpdateError::Oversize)
        );
    }

    #[test]
    fn startup_stagger_is_deterministic_and_bounded() {
        let uid = NodeUid::from_bytes([0x42; 12]);
        let delay = carrier_startup_delay_ms(uid);
        assert!((100..=899).contains(&delay));
        assert_eq!(carrier_startup_delay_ms(uid), delay);
    }
}
