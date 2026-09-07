#![no_std]

use pdcan_types::{
    BoardDefinition, CommissioningState, ConfigRevision, FanConfig, MAX_PORTS, NodeId, NodeUid,
    OperationId, PersistentSettings, PolicyValidationError, PortBitmap, PortId, PortPolicy,
    SlotEpoch,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mutation {
    /// The configuration revision which must be durably committed before a
    /// requester may be told that the mutation succeeded.
    pub persist_revision: Option<ConfigRevision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistCompletion {
    IgnoredStale,
    RetryScheduled,
    DurableThrough(ConfigRevision),
}

/// Pure commissioning/duplicate-address state. Persistence is intentionally
/// handled by [`Controller`]; this type only decides whether ordinary Node-ID
/// traffic is safe to emit.
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

    /// Observe an authoritative full-UID claim. A conflict never erases the
    /// persisted address; it must remain diagnosable and UID-addressable.
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
pub enum PdActionKind {
    ApplyPolicy(PortPolicy),
    EmergencyDisable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerOutputsActionKind {
    /// Initialize the board's power-control mechanism in a known all-off state.
    ArmAllOff,
    /// Command every firmware-controlled input-power output off.
    EmergencyDisableAll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PowerGateState {
    Off = 0,
    SwitchingOn = 1,
    On = 2,
    SwitchingOff = 3,
    Faulted = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Pd {
        operation: OperationId,
        port: PortId,
        slot_epoch: SlotEpoch,
        kind: PdActionKind,
    },
    PersistConfig {
        revision: ConfigRevision,
        settings: PersistentSettings,
    },
    PowerGate {
        operation: OperationId,
        port: PortId,
        slot_epoch: SlotEpoch,
        output_bit: u8,
        enabled: bool,
    },
    PowerOutputs {
        operation: OperationId,
        kind: PowerOutputsActionKind,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    pub stale_operation_completions: u32,
    pub stale_power_completions: u32,
    pub stale_persistence_completions: u32,
    pub rejected_unsupported_targets: u32,
    pub failed_pd_operations: u32,
    pub failed_power_operations: u32,
    pub failed_persistence_operations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerError {
    UnsupportedPort(PortId),
    InvalidPolicy(PolicyValidationError),
    InvalidFanDuty(u8),
    EmergencyLatched,
    EmergencyAlreadyClear,
    EmergencyClearInProgress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionOutcome {
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SlotState {
    epoch: SlotEpoch,
    online: bool,
    power_gate: PowerGateState,
    active_operation: Option<OperationId>,
    active_kind: Option<ActiveOperationKind>,
    policy_after_revision: Option<ConfigRevision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActiveOperationKind {
    Pd(PdActionKind),
    PowerGate(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PowerOutputsState {
    Disabled,
    Enabled,
}

impl SlotState {
    const INITIAL: Self = Self {
        epoch: SlotEpoch(0),
        online: false,
        power_gate: PowerGateState::Off,
        active_operation: None,
        active_kind: None,
        policy_after_revision: None,
    };
}

pub struct Controller {
    board: BoardDefinition,
    settings: PersistentSettings,
    slots: [SlotState; MAX_PORTS],
    pending_emergency: PortBitmap,
    pending_policy: PortBitmap,
    pending_power_on: PortBitmap,
    pending_power_off: PortBitmap,
    pending_power_emergency: bool,
    pending_power_arm: bool,
    power_outputs_state: PowerOutputsState,
    active_power_outputs: Option<(OperationId, PowerOutputsActionKind)>,
    pending_persist: Option<ConfigRevision>,
    persist_in_flight: Option<ConfigRevision>,
    durable_revision: ConfigRevision,
    clearing_emergency_at: Option<ConfigRevision>,
    emergency_latched_runtime: bool,
    next_operation: u32,
    next_config_revision: u32,
    emergency_cursor: u8,
    policy_cursor: u8,
    power_on_cursor: u8,
    power_off_cursor: u8,
    diagnostics: Diagnostics,
}

impl Controller {
    pub fn new(board: BoardDefinition, settings: PersistentSettings) -> Self {
        let mut pending_power_on = PortBitmap::EMPTY;
        if board.has_power_gates() && !settings.emergency_latched {
            for raw_port in PortId::MIN..=PortId::MAX {
                let port = PortId::new(raw_port).expect("MAX_PORTS defines valid port IDs");
                if board.supports(port)
                    && board.power_gate_bit(port).is_some()
                    && settings.port_policy[port.index()].enabled
                {
                    pending_power_on.insert(port);
                }
            }
        }
        Self {
            board,
            settings,
            slots: [SlotState::INITIAL; MAX_PORTS],
            pending_emergency: PortBitmap::EMPTY,
            pending_policy: PortBitmap::EMPTY,
            pending_power_on,
            pending_power_off: PortBitmap::EMPTY,
            pending_power_emergency: board.has_power_gates() && settings.emergency_latched,
            pending_power_arm: board.has_power_gates() && !settings.emergency_latched,
            power_outputs_state: if board.has_power_gates() {
                PowerOutputsState::Disabled
            } else {
                PowerOutputsState::Enabled
            },
            active_power_outputs: None,
            pending_persist: None,
            persist_in_flight: None,
            durable_revision: ConfigRevision(0),
            clearing_emergency_at: None,
            emergency_latched_runtime: settings.emergency_latched,
            next_operation: 0,
            next_config_revision: 0,
            emergency_cursor: PortId::MIN,
            policy_cursor: PortId::MIN,
            power_on_cursor: PortId::MIN,
            power_off_cursor: PortId::MIN,
            diagnostics: Diagnostics {
                stale_operation_completions: 0,
                stale_power_completions: 0,
                stale_persistence_completions: 0,
                rejected_unsupported_targets: 0,
                failed_pd_operations: 0,
                failed_power_operations: 0,
                failed_persistence_operations: 0,
            },
        }
    }

    pub const fn board(&self) -> BoardDefinition {
        self.board
    }

    pub const fn settings(&self) -> PersistentSettings {
        self.settings
    }

    pub const fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }

    pub const fn emergency_latched(&self) -> bool {
        self.emergency_latched_runtime
    }

    pub const fn port_online(&self, port: PortId) -> Option<bool> {
        if self.board.supports(port) {
            Some(self.slots[port.index()].online)
        } else {
            None
        }
    }

    pub const fn port_powered(&self, port: PortId) -> Option<bool> {
        if !self.board.supports(port) {
            return None;
        }
        if self.board.power_gate_bit(port).is_none() {
            return Some(true);
        }
        Some(matches!(
            self.slots[port.index()].power_gate,
            PowerGateState::On
        ))
    }

    pub const fn power_gate_state(&self, port: PortId) -> Option<PowerGateState> {
        if self.board.supports(port) && self.board.power_gate_bit(port).is_some() {
            Some(self.slots[port.index()].power_gate)
        } else {
            None
        }
    }

    pub const fn slot_epoch(&self, port: PortId) -> Option<SlotEpoch> {
        if self.board.supports(port) {
            Some(self.slots[port.index()].epoch)
        } else {
            None
        }
    }

    pub fn port_policy_pending(&self, port: PortId) -> Option<bool> {
        if !self.board.supports(port) {
            return None;
        }
        let slot = self.slots[port.index()];
        Some(
            self.pending_policy.contains(port)
                || self.pending_power_on.contains(port)
                || self.pending_power_off.contains(port)
                || matches!(
                    slot.active_kind,
                    Some(
                        ActiveOperationKind::Pd(PdActionKind::ApplyPolicy(_))
                            | ActiveOperationKind::PowerGate(_)
                    )
                )
                || (self.board.power_gate_bit(port).is_some()
                    && self.settings.port_policy[port.index()].enabled
                    && !slot.online
                    && matches!(slot.power_gate, PowerGateState::On)),
        )
    }

    pub fn online_ports(&self) -> PortBitmap {
        let mut ports = PortBitmap::EMPTY;
        for raw in PortId::MIN..=PortId::MAX {
            let port = PortId::new(raw).expect("MAX_PORTS defines valid port IDs");
            if self.board.supports(port) && self.slots[port.index()].online {
                ports.insert(port);
            }
        }
        ports
    }

    pub fn enabled_ports(&self) -> PortBitmap {
        let mut ports = PortBitmap::EMPTY;
        for raw in PortId::MIN..=PortId::MAX {
            let port = PortId::new(raw).expect("MAX_PORTS defines valid port IDs");
            if self.board.supports(port) && self.settings.port_policy[port.index()].enabled {
                ports.insert(port);
            }
        }
        ports
    }

    pub fn module_detected(&mut self, port: PortId) -> Result<SlotEpoch, ControllerError> {
        self.require_supported(port)?;
        if self.board.power_gate_bit(port).is_some()
            && !matches!(self.slots[port.index()].power_gate, PowerGateState::On)
        {
            // A late result from a prior powered session must not revive an
            // intentionally gated-off port.
            return Ok(self.slots[port.index()].epoch);
        }
        self.slots[port.index()].online = true;

        if self.board.power_gate_bit(port).is_some()
            && (self.emergency_latched_runtime || !self.settings.port_policy[port.index()].enabled)
        {
            self.schedule_power_off(port);
        } else if self.emergency_latched_runtime {
            self.pending_emergency.insert(port);
        } else {
            self.pending_policy.insert(port);
        }
        Ok(self.slots[port.index()].epoch)
    }

    pub fn module_removed(&mut self, port: PortId) -> Result<SlotEpoch, ControllerError> {
        self.require_supported(port)?;
        let should_advance_epoch = {
            let slot = &self.slots[port.index()];
            slot.online
                || slot.active_operation.is_some()
                || !matches!(slot.power_gate, PowerGateState::Off)
        };
        if should_advance_epoch {
            self.invalidate_slot(port);
        }
        self.slots[port.index()].online = false;
        self.pending_emergency.remove(port);
        self.pending_policy.remove(port);
        self.pending_power_on.remove(port);
        if self.board.power_gate_bit(port).is_some()
            && !matches!(self.slots[port.index()].power_gate, PowerGateState::Off)
        {
            self.pending_power_off.insert(port);
        }
        Ok(self.slots[port.index()].epoch)
    }

    pub fn set_port_policy(
        &mut self,
        port: PortId,
        policy: PortPolicy,
    ) -> Result<Mutation, ControllerError> {
        self.require_supported(port)?;
        policy.validate().map_err(ControllerError::InvalidPolicy)?;
        if self.emergency_latched_runtime && policy.enabled {
            return Err(ControllerError::EmergencyLatched);
        }

        let changed = self.settings.port_policy[port.index()] != policy;
        self.settings.port_policy[port.index()] = policy;
        let persist_revision = changed.then(|| self.request_persist());
        if self.board.power_gate_bit(port).is_some() {
            if policy.enabled {
                self.slots[port.index()].policy_after_revision = persist_revision;
                match self.slots[port.index()].power_gate {
                    PowerGateState::On if self.slots[port.index()].online => {
                        self.pending_policy.insert(port);
                    }
                    PowerGateState::Off
                    | PowerGateState::Faulted
                    | PowerGateState::SwitchingOff => {
                        self.pending_power_on.insert(port);
                    }
                    PowerGateState::SwitchingOn | PowerGateState::On => {}
                }
            } else {
                self.slots[port.index()].policy_after_revision = None;
                self.schedule_power_off(port);
            }
        } else if self.slots[port.index()].online {
            self.slots[port.index()].policy_after_revision = persist_revision;
            if self.emergency_latched_runtime {
                self.pending_emergency.insert(port);
            } else {
                self.pending_policy.insert(port);
            }
        }
        Ok(Mutation { persist_revision })
    }

    pub fn emergency_disable(&mut self) -> Mutation {
        self.emergency_latched_runtime = true;
        self.clearing_emergency_at = None;
        let persist_revision = if self.settings.emergency_latched {
            None
        } else {
            self.settings.emergency_latched = true;
            Some(self.request_persist())
        };

        if self.board.has_power_gates() {
            self.pending_power_emergency = true;
            self.pending_power_arm = false;
            self.power_outputs_state = PowerOutputsState::Disabled;
            self.active_power_outputs = None;
        }

        for raw_port in PortId::MIN..=PortId::MAX {
            let port = PortId::new(raw_port).expect("MAX_PORTS defines valid port IDs");
            if !self.board.supports(port) {
                continue;
            }
            if self.board.power_gate_bit(port).is_some() {
                let slot = self.slots[port.index()];
                if slot.online
                    || slot.active_operation.is_some()
                    || matches!(
                        slot.power_gate,
                        PowerGateState::On | PowerGateState::SwitchingOn
                    )
                {
                    self.invalidate_slot(port);
                }
                self.slots[port.index()].online = false;
                if !matches!(self.slots[port.index()].power_gate, PowerGateState::Off) {
                    self.slots[port.index()].power_gate = PowerGateState::SwitchingOff;
                }
                self.pending_power_on.remove(port);
                self.pending_power_off.remove(port);
                self.pending_policy.remove(port);
            } else if self.slots[port.index()].online {
                self.pending_emergency.insert(port);
                self.pending_policy.remove(port);
            }
        }
        Mutation { persist_revision }
    }

    pub fn set_node_id(&mut self, node_id: Option<NodeId>) -> Mutation {
        let persist_revision = if self.settings.node_id == node_id {
            None
        } else {
            self.settings.node_id = node_id;
            Some(self.request_persist())
        };
        Mutation { persist_revision }
    }

    pub fn set_fan_config(&mut self, fan: FanConfig) -> Result<Mutation, ControllerError> {
        fan.validate()
            .map_err(|error| ControllerError::InvalidFanDuty(error.0))?;
        let persist_revision = if self.settings.fan == fan {
            None
        } else {
            self.settings.fan = fan;
            Some(self.request_persist())
        };
        Ok(Mutation { persist_revision })
    }

    pub fn acknowledge_emergency_resolved(&mut self) -> Result<ConfigRevision, ControllerError> {
        if self.clearing_emergency_at.is_some() {
            return Err(ControllerError::EmergencyClearInProgress);
        }
        if !self.emergency_latched_runtime {
            return Err(ControllerError::EmergencyAlreadyClear);
        }

        self.settings.emergency_latched = false;
        let revision = self.request_persist();
        self.clearing_emergency_at = Some(revision);
        Ok(revision)
    }

    pub fn next_action(&mut self) -> Option<Action> {
        if self.pending_power_emergency {
            self.pending_power_emergency = false;
            return Some(
                self.start_power_outputs_action(PowerOutputsActionKind::EmergencyDisableAll),
            );
        }

        if let Some(port) = self.take_dispatchable_power_off() {
            return Some(self.start_power_gate_action(port, false));
        }

        if let Some(port) = self.take_dispatchable_emergency() {
            return Some(self.start_pd_action(port, PdActionKind::EmergencyDisable));
        }

        if self.persist_in_flight.is_none()
            && let Some(revision) = self.pending_persist.take()
        {
            self.persist_in_flight = Some(revision);
            return Some(Action::PersistConfig {
                revision,
                settings: self.settings,
            });
        }

        if !self.emergency_latched_runtime
            && self.pending_power_arm
            && self.active_power_outputs.is_none()
        {
            self.pending_power_arm = false;
            return Some(self.start_power_outputs_action(PowerOutputsActionKind::ArmAllOff));
        }

        if !self.emergency_latched_runtime
            && self.power_outputs_state == PowerOutputsState::Enabled
            && let Some(port) = self.take_dispatchable_power_on()
        {
            return Some(self.start_power_gate_action(port, true));
        }

        if !self.emergency_latched_runtime
            && let Some(port) = self.take_dispatchable_policy()
        {
            let policy = self.settings.port_policy[port.index()];
            return Some(self.start_pd_action(port, PdActionKind::ApplyPolicy(policy)));
        }

        None
    }

    pub fn complete_power_outputs_operation(
        &mut self,
        operation: OperationId,
        kind: PowerOutputsActionKind,
        outcome: CompletionOutcome,
    ) {
        if self.active_power_outputs != Some((operation, kind)) {
            self.diagnostics.stale_power_completions =
                self.diagnostics.stale_power_completions.saturating_add(1);
            return;
        }
        self.active_power_outputs = None;

        if outcome == CompletionOutcome::Failed {
            self.diagnostics.failed_power_operations =
                self.diagnostics.failed_power_operations.saturating_add(1);
            // Without physical rail feedback, a failed global output command
            // makes every controlled port unknown. Report them faulted, stop
            // all per-port work, and keep retrying the all-off request.
            self.fail_all_power_closed();
            return;
        }

        match kind {
            PowerOutputsActionKind::EmergencyDisableAll => {
                self.power_outputs_state = PowerOutputsState::Disabled;
                for raw_port in PortId::MIN..=PortId::MAX {
                    let port = PortId::new(raw_port).expect("MAX_PORTS defines valid port IDs");
                    if self.board.power_gate_bit(port).is_some() {
                        self.slots[port.index()].power_gate = PowerGateState::Off;
                        self.slots[port.index()].online = false;
                    }
                }
            }
            PowerOutputsActionKind::ArmAllOff => {
                self.power_outputs_state = PowerOutputsState::Enabled;
                for raw_port in PortId::MIN..=PortId::MAX {
                    let port = PortId::new(raw_port).expect("MAX_PORTS defines valid port IDs");
                    if self.board.power_gate_bit(port).is_some() {
                        self.slots[port.index()].power_gate = PowerGateState::Off;
                    }
                }
            }
        }
    }

    pub fn complete_power_gate_operation(
        &mut self,
        port: PortId,
        operation: OperationId,
        slot_epoch: SlotEpoch,
        enabled: bool,
        outcome: CompletionOutcome,
    ) {
        let Some(slot) = self.slots.get_mut(port.index()) else {
            self.diagnostics.stale_power_completions =
                self.diagnostics.stale_power_completions.saturating_add(1);
            return;
        };
        if slot.epoch != slot_epoch
            || slot.active_operation != Some(operation)
            || slot.active_kind != Some(ActiveOperationKind::PowerGate(enabled))
        {
            self.diagnostics.stale_power_completions =
                self.diagnostics.stale_power_completions.saturating_add(1);
            return;
        }
        slot.active_operation = None;
        slot.active_kind = None;

        if outcome == CompletionOutcome::Failed {
            self.diagnostics.failed_power_operations =
                self.diagnostics.failed_power_operations.saturating_add(1);
            self.fail_all_power_closed();
            return;
        }

        slot.power_gate = if enabled {
            PowerGateState::On
        } else {
            PowerGateState::Off
        };
        if !enabled {
            slot.online = false;
        }
    }

    pub fn complete_pd_operation(
        &mut self,
        port: PortId,
        operation: OperationId,
        slot_epoch: SlotEpoch,
        outcome: CompletionOutcome,
    ) {
        let Some(slot) = self.slots.get_mut(port.index()) else {
            self.diagnostics.stale_operation_completions = self
                .diagnostics
                .stale_operation_completions
                .saturating_add(1);
            return;
        };

        if slot.epoch != slot_epoch
            || slot.active_operation != Some(operation)
            || !matches!(slot.active_kind, Some(ActiveOperationKind::Pd(_)))
        {
            self.diagnostics.stale_operation_completions = self
                .diagnostics
                .stale_operation_completions
                .saturating_add(1);
            return;
        }

        let completed_kind = match slot.active_kind.take() {
            Some(ActiveOperationKind::Pd(kind)) => Some(kind),
            Some(ActiveOperationKind::PowerGate(_)) | None => None,
        };
        slot.active_operation = None;
        if outcome == CompletionOutcome::Failed {
            self.diagnostics.failed_pd_operations =
                self.diagnostics.failed_pd_operations.saturating_add(1);
            if self.board.power_gate_bit(port).is_some()
                && matches!(completed_kind, Some(PdActionKind::ApplyPolicy(_)))
            {
                self.schedule_power_off(port);
            } else if slot.online {
                if self.emergency_latched_runtime
                    || completed_kind == Some(PdActionKind::EmergencyDisable)
                {
                    self.pending_emergency.insert(port);
                } else {
                    self.pending_policy.insert(port);
                }
            }
        }
    }

    pub fn complete_persist(
        &mut self,
        revision: ConfigRevision,
        outcome: CompletionOutcome,
    ) -> PersistCompletion {
        if self.persist_in_flight != Some(revision) {
            self.diagnostics.stale_persistence_completions = self
                .diagnostics
                .stale_persistence_completions
                .saturating_add(1);
            return PersistCompletion::IgnoredStale;
        }
        self.persist_in_flight = None;

        if outcome == CompletionOutcome::Failed {
            self.diagnostics.failed_persistence_operations = self
                .diagnostics
                .failed_persistence_operations
                .saturating_add(1);
            self.request_persist();
            return PersistCompletion::RetryScheduled;
        }

        self.durable_revision = revision;

        if self
            .clearing_emergency_at
            .is_some_and(|clear_revision| revision_covers(revision, clear_revision))
        {
            self.clearing_emergency_at = None;
            self.emergency_latched_runtime = false;
            if self.board.has_power_gates() {
                // Clearing the latch only restores permission to use the
                // outputs. It deliberately does not restore any enabled port.
                self.pending_power_arm = true;
                self.pending_power_on = PortBitmap::EMPTY;
            }
        }
        PersistCompletion::DurableThrough(revision)
    }

    fn require_supported(&mut self, port: PortId) -> Result<(), ControllerError> {
        if self.board.supports(port) {
            Ok(())
        } else {
            self.diagnostics.rejected_unsupported_targets = self
                .diagnostics
                .rejected_unsupported_targets
                .saturating_add(1);
            Err(ControllerError::UnsupportedPort(port))
        }
    }

    fn request_persist(&mut self) -> ConfigRevision {
        self.next_config_revision = self.next_config_revision.wrapping_add(1);
        let revision = ConfigRevision(self.next_config_revision);
        self.pending_persist = Some(revision);
        revision
    }

    fn start_pd_action(&mut self, port: PortId, kind: PdActionKind) -> Action {
        self.next_operation = self.next_operation.wrapping_add(1);
        let operation = OperationId(self.next_operation);
        let slot = &mut self.slots[port.index()];
        slot.active_operation = Some(operation);
        slot.active_kind = Some(ActiveOperationKind::Pd(kind));
        Action::Pd {
            operation,
            port,
            slot_epoch: slot.epoch,
            kind,
        }
    }

    fn start_power_gate_action(&mut self, port: PortId, enabled: bool) -> Action {
        self.next_operation = self.next_operation.wrapping_add(1);
        let operation = OperationId(self.next_operation);
        let slot = &mut self.slots[port.index()];
        slot.active_operation = Some(operation);
        slot.active_kind = Some(ActiveOperationKind::PowerGate(enabled));
        slot.power_gate = if enabled {
            PowerGateState::SwitchingOn
        } else {
            PowerGateState::SwitchingOff
        };
        Action::PowerGate {
            operation,
            port,
            slot_epoch: slot.epoch,
            output_bit: self
                .board
                .power_gate_bit(port)
                .expect("only power-gated ports enter the power scheduler"),
            enabled,
        }
    }

    fn start_power_outputs_action(&mut self, kind: PowerOutputsActionKind) -> Action {
        self.next_operation = self.next_operation.wrapping_add(1);
        let operation = OperationId(self.next_operation);
        self.active_power_outputs = Some((operation, kind));
        Action::PowerOutputs { operation, kind }
    }

    fn take_dispatchable_emergency(&mut self) -> Option<PortId> {
        Self::take_dispatchable(
            &mut self.pending_emergency,
            &self.slots,
            &mut self.emergency_cursor,
        )
    }

    fn take_dispatchable_policy(&mut self) -> Option<PortId> {
        let mut raw_port = self.policy_cursor;
        for _ in 0..MAX_PORTS {
            let port = PortId::new(raw_port).expect("scheduler cursor is always a valid port");
            let slot = &self.slots[port.index()];
            let persistence_ready = slot
                .policy_after_revision
                .is_none_or(|required| revision_covers(self.durable_revision, required));
            if self.pending_policy.contains(port)
                && slot.online
                && slot.active_operation.is_none()
                && persistence_ready
            {
                self.pending_policy.remove(port);
                self.slots[port.index()].policy_after_revision = None;
                self.policy_cursor = next_port(raw_port);
                return Some(port);
            }
            raw_port = next_port(raw_port);
        }
        None
    }

    fn take_dispatchable_power_on(&mut self) -> Option<PortId> {
        let mut raw_port = self.power_on_cursor;
        for _ in 0..MAX_PORTS {
            let port = PortId::new(raw_port).expect("scheduler cursor is always a valid port");
            let slot = &self.slots[port.index()];
            let persistence_ready = slot
                .policy_after_revision
                .is_none_or(|required| revision_covers(self.durable_revision, required));
            if self.pending_power_on.contains(port)
                && slot.active_operation.is_none()
                && matches!(
                    slot.power_gate,
                    PowerGateState::Off | PowerGateState::Faulted
                )
                && persistence_ready
            {
                self.pending_power_on.remove(port);
                self.power_on_cursor = next_port(raw_port);
                return Some(port);
            }
            raw_port = next_port(raw_port);
        }
        None
    }

    fn take_dispatchable_power_off(&mut self) -> Option<PortId> {
        Self::take_dispatchable_power(
            &mut self.pending_power_off,
            &self.slots,
            &mut self.power_off_cursor,
        )
    }

    fn take_dispatchable_power(
        pending: &mut PortBitmap,
        slots: &[SlotState; MAX_PORTS],
        cursor: &mut u8,
    ) -> Option<PortId> {
        let mut raw_port = *cursor;
        for _ in 0..MAX_PORTS {
            let port = PortId::new(raw_port).expect("scheduler cursor is always a valid port");
            let slot = &slots[port.index()];
            if pending.contains(port) && slot.active_operation.is_none() {
                pending.remove(port);
                *cursor = next_port(raw_port);
                return Some(port);
            }
            raw_port = next_port(raw_port);
        }
        None
    }

    fn take_dispatchable(
        pending: &mut PortBitmap,
        slots: &[SlotState; MAX_PORTS],
        cursor: &mut u8,
    ) -> Option<PortId> {
        let mut raw_port = *cursor;
        for _ in 0..MAX_PORTS {
            let port = PortId::new(raw_port).expect("scheduler cursor is always a valid port");
            let slot = &slots[port.index()];
            if pending.contains(port) && slot.online && slot.active_operation.is_none() {
                pending.remove(port);
                *cursor = next_port(raw_port);
                return Some(port);
            }
            raw_port = next_port(raw_port);
        }
        None
    }

    fn schedule_power_off(&mut self, port: PortId) {
        self.pending_power_on.remove(port);
        self.pending_policy.remove(port);
        self.pending_emergency.remove(port);
        if matches!(self.slots[port.index()].power_gate, PowerGateState::Off) {
            self.slots[port.index()].online = false;
            return;
        }
        self.invalidate_slot(port);
        self.slots[port.index()].online = false;
        self.pending_power_off.insert(port);
    }

    fn fail_all_power_closed(&mut self) {
        self.power_outputs_state = PowerOutputsState::Disabled;
        self.pending_power_emergency = true;
        self.pending_power_arm = false;
        for raw_port in PortId::MIN..=PortId::MAX {
            let port = PortId::new(raw_port).expect("MAX_PORTS defines valid port IDs");
            if self.board.power_gate_bit(port).is_none() {
                continue;
            }
            self.invalidate_slot(port);
            let slot = &mut self.slots[port.index()];
            slot.online = false;
            slot.power_gate = PowerGateState::Faulted;
            self.pending_power_on.remove(port);
            self.pending_power_off.remove(port);
            self.pending_policy.remove(port);
        }
    }

    fn invalidate_slot(&mut self, port: PortId) {
        let slot = &mut self.slots[port.index()];
        slot.epoch = SlotEpoch(slot.epoch.0.wrapping_add(1));
        slot.active_operation = None;
        slot.active_kind = None;
    }
}

const fn next_port(port: u8) -> u8 {
    if port == PortId::MAX {
        PortId::MIN
    } else {
        port + 1
    }
}

/// Returns true when a successful coalesced write at `durable` includes the
/// settings mutation requested at `required`. This uses the same half-range
/// wrapping ordering as the on-flash journal sequence.
pub const fn revision_covers(durable: ConfigRevision, required: ConfigRevision) -> bool {
    durable.0 == required.0 || durable.0.wrapping_sub(required.0) < 0x8000_0000
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_types::{FanMode, HardwareRevision};

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
        power_gate_bit_by_port: [None; MAX_PORTS],
        default_fan_mode: FanMode::ThreeWire,
    };

    const REV_B: BoardDefinition = BoardDefinition {
        hardware_revision: HardwareRevision::RevB,
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
        power_gate_bit_by_port: [
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

    fn port(value: u8) -> PortId {
        PortId::new(value).unwrap()
    }

    fn enabled_policy() -> PortPolicy {
        PortPolicy {
            enabled: true,
            ..PortPolicy::SAFE_DISABLED
        }
    }

    fn arm_power_outputs(controller: &mut Controller) {
        let Action::PowerOutputs { operation, kind } = controller.next_action().unwrap() else {
            panic!("power-gated boards must establish all-off before port power")
        };
        assert_eq!(kind, PowerOutputsActionKind::ArmAllOff);
        controller.complete_power_outputs_operation(operation, kind, CompletionOutcome::Succeeded);
    }

    #[test]
    fn rev_a_rejects_ports_six_and_seven_as_unsupported() {
        let mut controller = Controller::new(REV_A, PersistentSettings::FACTORY_DEFAULT);
        assert_eq!(
            controller.module_detected(port(6)),
            Err(ControllerError::UnsupportedPort(port(6)))
        );
        assert_eq!(controller.diagnostics().rejected_unsupported_targets, 1);
        assert!(controller.next_action().is_none());
    }

    #[test]
    fn rev_b_restores_a_persisted_enabled_port_in_safe_order() {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.port_policy[0] = enabled_policy();
        let mut controller = Controller::new(REV_B, settings);

        assert_eq!(controller.port_powered(port(0)), Some(false));
        arm_power_outputs(&mut controller);
        let Action::PowerGate {
            operation,
            port: action_port,
            slot_epoch,
            output_bit,
            enabled,
        } = controller.next_action().unwrap()
        else {
            panic!("persisted policy must schedule its input gate")
        };
        assert_eq!(action_port, port(0));
        assert_eq!(output_bit, 0);
        assert!(enabled);
        controller.complete_power_gate_operation(
            action_port,
            operation,
            slot_epoch,
            enabled,
            CompletionOutcome::Succeeded,
        );
        assert_eq!(controller.port_powered(port(0)), Some(true));
        assert_eq!(controller.port_online(port(0)), Some(false));

        controller.module_detected(port(0)).unwrap();
        assert!(matches!(
            controller.next_action(),
            Some(Action::Pd {
                kind: PdActionKind::ApplyPolicy(policy),
                ..
            }) if policy == enabled_policy()
        ));
    }

    #[test]
    fn late_detection_cannot_revive_an_unpowered_rev_b_port() {
        let mut controller = Controller::new(REV_B, PersistentSettings::FACTORY_DEFAULT);
        let epoch = controller.slot_epoch(port(0)).unwrap();

        assert_eq!(controller.module_detected(port(0)), Ok(epoch));
        assert_eq!(controller.port_online(port(0)), Some(false));
        assert_eq!(controller.port_powered(port(0)), Some(false));
        assert!(matches!(
            controller.next_action(),
            Some(Action::PowerOutputs {
                kind: PowerOutputsActionKind::ArmAllOff,
                ..
            })
        ));
    }

    #[test]
    fn a_new_enable_is_durable_before_rev_b_energizes_the_port() {
        let mut controller = Controller::new(REV_B, PersistentSettings::FACTORY_DEFAULT);
        arm_power_outputs(&mut controller);
        let required = controller
            .set_port_policy(port(2), enabled_policy())
            .unwrap()
            .persist_revision
            .unwrap();

        let Action::PersistConfig { revision, .. } = controller.next_action().unwrap() else {
            panic!("persistence must precede power-on")
        };
        assert_eq!(revision, required);
        assert_eq!(controller.port_powered(port(2)), Some(false));
        controller.complete_persist(revision, CompletionOutcome::Succeeded);

        assert!(matches!(
            controller.next_action(),
            Some(Action::PowerGate {
                port: action_port,
                output_bit: 2,
                enabled: true,
                ..
            }) if action_port == port(2)
        ));
    }

    #[test]
    fn failed_policy_application_on_rev_b_fails_closed() {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.port_policy[0] = enabled_policy();
        let mut controller = Controller::new(REV_B, settings);
        arm_power_outputs(&mut controller);
        let Action::PowerGate {
            operation,
            slot_epoch,
            ..
        } = controller.next_action().unwrap()
        else {
            panic!("expected gate-on")
        };
        controller.complete_power_gate_operation(
            port(0),
            operation,
            slot_epoch,
            true,
            CompletionOutcome::Succeeded,
        );
        controller.module_detected(port(0)).unwrap();
        let Action::Pd {
            operation,
            slot_epoch,
            ..
        } = controller.next_action().unwrap()
        else {
            panic!("expected policy application")
        };
        controller.complete_pd_operation(port(0), operation, slot_epoch, CompletionOutcome::Failed);

        assert!(matches!(
            controller.next_action(),
            Some(Action::PowerGate {
                port: action_port,
                enabled: false,
                ..
            }) if action_port == port(0)
        ));
    }

    #[test]
    fn emergency_uses_global_output_disable_and_clear_does_not_restore_ports() {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.port_policy[0] = enabled_policy();
        let mut controller = Controller::new(REV_B, settings);
        arm_power_outputs(&mut controller);
        let stale_gate = controller.next_action().unwrap();

        controller.emergency_disable();
        let Action::PowerOutputs { operation, kind } = controller.next_action().unwrap() else {
            panic!("global output disable must outrank persistence")
        };
        assert_eq!(kind, PowerOutputsActionKind::EmergencyDisableAll);
        controller.complete_power_outputs_operation(operation, kind, CompletionOutcome::Succeeded);
        assert_eq!(controller.port_powered(port(0)), Some(false));

        let Action::PersistConfig { revision, .. } = controller.next_action().unwrap() else {
            panic!("emergency latch must be persisted")
        };
        controller.complete_persist(revision, CompletionOutcome::Succeeded);
        let clear = controller.acknowledge_emergency_resolved().unwrap();
        let Action::PersistConfig { revision, .. } = controller.next_action().unwrap() else {
            panic!("emergency clear must be persisted")
        };
        assert_eq!(revision, clear);
        controller.complete_persist(revision, CompletionOutcome::Succeeded);
        arm_power_outputs(&mut controller);
        assert!(controller.next_action().is_none());

        let Action::PowerGate {
            operation,
            port: stale_port,
            slot_epoch,
            enabled,
            ..
        } = stale_gate
        else {
            panic!("expected the pre-emergency gate command")
        };
        controller.complete_power_gate_operation(
            stale_port,
            operation,
            slot_epoch,
            enabled,
            CompletionOutcome::Succeeded,
        );
        assert_eq!(controller.diagnostics().stale_power_completions, 1);
        assert_eq!(controller.port_powered(port(0)), Some(false));
    }

    #[test]
    fn failed_global_output_command_faults_every_gate_and_retries_all_off() {
        let mut controller = Controller::new(REV_B, PersistentSettings::FACTORY_DEFAULT);
        let Action::PowerOutputs { operation, kind } = controller.next_action().unwrap() else {
            panic!("Rev B must initialize its power controller all-off")
        };
        assert_eq!(kind, PowerOutputsActionKind::ArmAllOff);

        controller.complete_power_outputs_operation(operation, kind, CompletionOutcome::Failed);

        for raw_port in 0..6 {
            assert_eq!(
                controller.power_gate_state(port(raw_port)),
                Some(PowerGateState::Faulted)
            );
        }
        assert_eq!(controller.diagnostics().failed_power_operations, 1);
        assert!(matches!(
            controller.next_action(),
            Some(Action::PowerOutputs {
                kind: PowerOutputsActionKind::EmergencyDisableAll,
                ..
            })
        ));
    }

    #[test]
    fn emergency_actions_outrank_persistence_and_policy() {
        let mut controller = Controller::new(REV_A, PersistentSettings::FACTORY_DEFAULT);
        for raw_port in 0..6 {
            controller.module_detected(port(raw_port)).unwrap();
        }
        controller.emergency_disable();

        for raw_port in 0..6 {
            let Action::Pd {
                port: action_port,
                kind,
                ..
            } = controller.next_action().unwrap()
            else {
                panic!("PD emergency action must precede persistence");
            };
            assert_eq!(action_port, port(raw_port));
            assert_eq!(kind, PdActionKind::EmergencyDisable);
        }

        assert!(matches!(
            controller.next_action(),
            Some(Action::PersistConfig { .. })
        ));
    }

    #[test]
    fn a_failed_port_does_not_starve_other_pending_ports() {
        let mut controller = Controller::new(REV_A, PersistentSettings::FACTORY_DEFAULT);
        controller.module_detected(port(0)).unwrap();
        controller.module_detected(port(1)).unwrap();

        let Action::Pd {
            operation,
            port: first_port,
            slot_epoch,
            ..
        } = controller.next_action().unwrap()
        else {
            panic!("module detection must schedule a policy action");
        };
        assert_eq!(first_port, port(0));
        controller.complete_pd_operation(
            first_port,
            operation,
            slot_epoch,
            CompletionOutcome::Failed,
        );

        let Action::Pd {
            port: second_port, ..
        } = controller.next_action().unwrap()
        else {
            panic!("the other pending port must remain dispatchable");
        };
        assert_eq!(second_port, port(1));
    }

    #[test]
    fn emergency_clear_takes_effect_only_after_durable_completion() {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.port_policy[0] = enabled_policy();
        let mut controller = Controller::new(REV_A, settings);
        controller.module_detected(port(0)).unwrap();
        let initial_policy = controller.next_action().unwrap();
        let Action::Pd {
            operation,
            slot_epoch,
            ..
        } = initial_policy
        else {
            panic!("expected initial policy action");
        };
        controller.complete_pd_operation(
            port(0),
            operation,
            slot_epoch,
            CompletionOutcome::Succeeded,
        );

        controller.emergency_disable();
        let emergency = controller.next_action().unwrap();
        let Action::Pd {
            operation,
            slot_epoch,
            ..
        } = emergency
        else {
            panic!("expected emergency action");
        };
        controller.complete_pd_operation(
            port(0),
            operation,
            slot_epoch,
            CompletionOutcome::Succeeded,
        );

        let Action::PersistConfig {
            revision: latch_revision,
            settings: persisted,
        } = controller.next_action().unwrap()
        else {
            panic!("expected latched persistence");
        };
        assert!(persisted.emergency_latched);
        controller.complete_persist(latch_revision, CompletionOutcome::Succeeded);

        let clear_revision = controller.acknowledge_emergency_resolved().unwrap();
        assert!(controller.emergency_latched());
        let Action::PersistConfig {
            revision,
            settings: persisted,
        } = controller.next_action().unwrap()
        else {
            panic!("expected cleared persistence");
        };
        assert_eq!(revision, clear_revision);
        assert!(!persisted.emergency_latched);
        assert!(controller.emergency_latched());

        controller.complete_persist(clear_revision, CompletionOutcome::Succeeded);
        assert!(!controller.emergency_latched());
        assert!(controller.next_action().is_none());

        controller
            .set_port_policy(port(0), enabled_policy())
            .unwrap();
        assert!(matches!(
            controller.next_action(),
            Some(Action::Pd {
                kind: PdActionKind::ApplyPolicy(_),
                ..
            })
        ));
    }

    #[test]
    fn stale_operation_completion_is_ignored_and_recorded() {
        let mut controller = Controller::new(REV_A, PersistentSettings::FACTORY_DEFAULT);
        let epoch = controller.module_detected(port(0)).unwrap();
        let Action::Pd { operation, .. } = controller.next_action().unwrap() else {
            panic!("expected policy action");
        };
        controller.module_removed(port(0)).unwrap();

        controller.complete_pd_operation(port(0), operation, epoch, CompletionOutcome::Succeeded);

        assert_eq!(controller.diagnostics().stale_operation_completions, 1);
        assert!(!controller.slots[0].online);
    }

    #[test]
    fn enabled_policy_is_rejected_while_emergency_is_latched() {
        let mut controller = Controller::new(REV_A, PersistentSettings::FACTORY_DEFAULT);
        controller.emergency_disable();
        assert_eq!(
            controller.set_port_policy(port(0), enabled_policy()),
            Err(ControllerError::EmergencyLatched)
        );
    }

    #[test]
    fn coalesced_write_durably_covers_every_earlier_mutation() {
        let mut controller = Controller::new(REV_A, PersistentSettings::FACTORY_DEFAULT);
        let first = controller
            .set_port_policy(port(0), enabled_policy())
            .unwrap()
            .persist_revision
            .unwrap();
        let second = controller
            .set_node_id(Some(pdcan_types::NodeId::new(9).unwrap()))
            .persist_revision
            .unwrap();

        let Action::PersistConfig { revision, .. } = controller.next_action().unwrap() else {
            panic!("expected the coalesced configuration write");
        };
        assert_eq!(revision, second);
        assert!(revision_covers(revision, first));
        assert_eq!(
            controller.complete_persist(revision, CompletionOutcome::Succeeded),
            PersistCompletion::DurableThrough(second)
        );
    }

    #[test]
    fn failed_emergency_clear_remains_latched_and_retries_the_clear() {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.emergency_latched = true;
        let mut controller = Controller::new(REV_A, settings);
        let requested = controller.acknowledge_emergency_resolved().unwrap();

        let Action::PersistConfig { revision, settings } = controller.next_action().unwrap() else {
            panic!("expected clear persistence");
        };
        assert_eq!(revision, requested);
        assert!(!settings.emergency_latched);
        assert_eq!(
            controller.complete_persist(revision, CompletionOutcome::Failed),
            PersistCompletion::RetryScheduled
        );
        assert!(controller.emergency_latched());
        assert!(!controller.settings().emergency_latched);

        let Action::PersistConfig {
            revision: retry,
            settings,
        } = controller.next_action().unwrap()
        else {
            panic!("expected retried clear persistence");
        };
        assert!(!settings.emergency_latched);
        assert!(revision_covers(retry, requested));
        assert_eq!(
            controller.complete_persist(retry, CompletionOutcome::Succeeded),
            PersistCompletion::DurableThrough(retry)
        );
        assert!(!controller.emergency_latched());
    }

    #[test]
    fn duplicate_claim_keeps_persisted_id_but_suppresses_normal_traffic() {
        let own_uid = NodeUid::from_bytes([1; NodeUid::LENGTH]);
        let other_uid = NodeUid::from_bytes([2; NodeUid::LENGTH]);
        let node = pdcan_types::NodeId::new(7).unwrap();
        let mut commissioning = Commissioning::new(own_uid, Some(node));

        assert_eq!(commissioning.state(), CommissioningState::Claiming);
        assert!(!commissioning.normal_traffic_allowed());
        commissioning.claim_window_complete();
        assert!(commissioning.normal_traffic_allowed());

        commissioning.observe_claim(other_uid, node);
        assert_eq!(commissioning.state(), CommissioningState::AddressConflict);
        assert_eq!(commissioning.node_id(), Some(node));
        assert_eq!(commissioning.conflicting_uid(), Some(other_uid));
        assert!(!commissioning.normal_traffic_allowed());
    }

    #[test]
    fn assigning_or_clearing_a_node_restarts_commissioning_state() {
        let uid = NodeUid::from_bytes([3; NodeUid::LENGTH]);
        let node = pdcan_types::NodeId::new(42).unwrap();
        let mut commissioning = Commissioning::new(uid, None);
        assert_eq!(commissioning.state(), CommissioningState::Uncommissioned);

        commissioning.apply_persisted_node_id(Some(node));
        assert_eq!(commissioning.state(), CommissioningState::Claiming);
        commissioning.claim_window_complete();
        assert_eq!(commissioning.state(), CommissioningState::Commissioned);

        commissioning.apply_persisted_node_id(None);
        assert_eq!(commissioning.state(), CommissioningState::Uncommissioned);
        assert!(!commissioning.normal_traffic_allowed());
    }
}
