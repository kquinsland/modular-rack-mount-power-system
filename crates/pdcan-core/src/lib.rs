#![no_std]

use pdcan_types::{
    BoardDefinition, ConfigRevision, MAX_PORTS, OperationId, PersistentSettings,
    PolicyValidationError, PortBitmap, PortId, PortPolicy, SlotEpoch,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PdActionKind {
    ApplyPolicy(PortPolicy),
    EmergencyDisable,
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    pub stale_operation_completions: u32,
    pub stale_persistence_completions: u32,
    pub rejected_unsupported_targets: u32,
    pub failed_pd_operations: u32,
    pub failed_persistence_operations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerError {
    UnsupportedPort(PortId),
    InvalidPolicy(PolicyValidationError),
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
    active_operation: Option<OperationId>,
    active_kind: Option<PdActionKind>,
}

impl SlotState {
    const INITIAL: Self = Self {
        epoch: SlotEpoch(0),
        online: false,
        active_operation: None,
        active_kind: None,
    };
}

pub struct Controller {
    board: BoardDefinition,
    settings: PersistentSettings,
    slots: [SlotState; MAX_PORTS],
    pending_emergency: PortBitmap,
    pending_policy: PortBitmap,
    pending_persist: Option<ConfigRevision>,
    persist_in_flight: Option<ConfigRevision>,
    clearing_emergency_at: Option<ConfigRevision>,
    emergency_latched_runtime: bool,
    next_operation: u32,
    next_config_revision: u32,
    emergency_cursor: u8,
    policy_cursor: u8,
    diagnostics: Diagnostics,
}

impl Controller {
    pub const fn new(board: BoardDefinition, settings: PersistentSettings) -> Self {
        Self {
            board,
            settings,
            slots: [SlotState::INITIAL; MAX_PORTS],
            pending_emergency: PortBitmap::EMPTY,
            pending_policy: PortBitmap::EMPTY,
            pending_persist: None,
            persist_in_flight: None,
            clearing_emergency_at: None,
            emergency_latched_runtime: settings.emergency_latched,
            next_operation: 0,
            next_config_revision: 0,
            emergency_cursor: PortId::MIN,
            policy_cursor: PortId::MIN,
            diagnostics: Diagnostics {
                stale_operation_completions: 0,
                stale_persistence_completions: 0,
                rejected_unsupported_targets: 0,
                failed_pd_operations: 0,
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

    pub fn module_detected(&mut self, port: PortId) -> Result<SlotEpoch, ControllerError> {
        self.require_supported(port)?;
        let slot = &mut self.slots[port.index()];
        slot.online = true;

        if self.emergency_latched_runtime {
            self.pending_emergency.insert(port);
        } else {
            self.pending_policy.insert(port);
        }
        Ok(slot.epoch)
    }

    pub fn module_removed(&mut self, port: PortId) -> Result<SlotEpoch, ControllerError> {
        self.require_supported(port)?;
        let slot = &mut self.slots[port.index()];
        slot.online = false;
        slot.epoch = SlotEpoch(slot.epoch.0.wrapping_add(1));
        slot.active_operation = None;
        slot.active_kind = None;
        self.pending_emergency.remove(port);
        self.pending_policy.remove(port);
        Ok(slot.epoch)
    }

    pub fn set_port_policy(
        &mut self,
        port: PortId,
        policy: PortPolicy,
    ) -> Result<(), ControllerError> {
        self.require_supported(port)?;
        policy.validate().map_err(ControllerError::InvalidPolicy)?;
        if self.emergency_latched_runtime && policy.enabled {
            return Err(ControllerError::EmergencyLatched);
        }

        let changed = self.settings.port_policy[port.index()] != policy;
        self.settings.port_policy[port.index()] = policy;
        if changed {
            self.request_persist();
        }
        if self.slots[port.index()].online {
            if self.emergency_latched_runtime {
                self.pending_emergency.insert(port);
            } else {
                self.pending_policy.insert(port);
            }
        }
        Ok(())
    }

    pub fn emergency_disable(&mut self) {
        self.emergency_latched_runtime = true;
        self.clearing_emergency_at = None;
        if !self.settings.emergency_latched {
            self.settings.emergency_latched = true;
            self.request_persist();
        }

        for raw_port in PortId::MIN..=PortId::MAX {
            let port = PortId::new(raw_port).expect("MAX_PORTS defines valid port IDs");
            if self.board.supports(port) && self.slots[port.index()].online {
                self.pending_emergency.insert(port);
                self.pending_policy.remove(port);
            }
        }
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
            && let Some(port) = self.take_dispatchable_policy()
        {
            let policy = self.settings.port_policy[port.index()];
            return Some(self.start_pd_action(port, PdActionKind::ApplyPolicy(policy)));
        }

        None
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

        if slot.epoch != slot_epoch || slot.active_operation != Some(operation) {
            self.diagnostics.stale_operation_completions = self
                .diagnostics
                .stale_operation_completions
                .saturating_add(1);
            return;
        }

        let completed_kind = slot.active_kind.take();
        slot.active_operation = None;
        if outcome == CompletionOutcome::Failed {
            self.diagnostics.failed_pd_operations =
                self.diagnostics.failed_pd_operations.saturating_add(1);
            if slot.online {
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

    pub fn complete_persist(&mut self, revision: ConfigRevision, outcome: CompletionOutcome) {
        if self.persist_in_flight != Some(revision) {
            self.diagnostics.stale_persistence_completions = self
                .diagnostics
                .stale_persistence_completions
                .saturating_add(1);
            return;
        }
        self.persist_in_flight = None;

        if outcome == CompletionOutcome::Failed {
            self.diagnostics.failed_persistence_operations = self
                .diagnostics
                .failed_persistence_operations
                .saturating_add(1);
            if self.clearing_emergency_at == Some(revision) {
                self.settings.emergency_latched = true;
                self.clearing_emergency_at = None;
            }
            self.request_persist();
            return;
        }

        if self.clearing_emergency_at == Some(revision) {
            self.clearing_emergency_at = None;
            self.emergency_latched_runtime = false;
        }
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
        slot.active_kind = Some(kind);
        Action::Pd {
            operation,
            port,
            slot_epoch: slot.epoch,
            kind,
        }
    }

    fn take_dispatchable_emergency(&mut self) -> Option<PortId> {
        Self::take_dispatchable(
            &mut self.pending_emergency,
            &self.slots,
            &mut self.emergency_cursor,
        )
    }

    fn take_dispatchable_policy(&mut self) -> Option<PortId> {
        Self::take_dispatchable(
            &mut self.pending_policy,
            &self.slots,
            &mut self.policy_cursor,
        )
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
}

const fn next_port(port: u8) -> u8 {
    if port == PortId::MAX {
        PortId::MIN
    } else {
        port + 1
    }
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
}
