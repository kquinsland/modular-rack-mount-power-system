use pdcan_core::{Action, CompletionOutcome, PdActionKind, PowerOutputsActionKind};
use pdcan_types::{ConfigRevision, OperationId, PersistentSettings, PortId, SlotEpoch};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PdBusCommand {
    pub operation: OperationId,
    pub port: PortId,
    pub slot_epoch: SlotEpoch,
    pub kind: PdActionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistCommand {
    pub revision: ConfigRevision,
    pub settings: PersistentSettings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerGateCommand {
    pub operation: OperationId,
    pub port: PortId,
    pub slot_epoch: SlotEpoch,
    pub output_bit: u8,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerOutputsCommand {
    pub operation: OperationId,
    pub kind: PowerOutputsActionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerCommand {
    Gate(PowerGateCommand),
    Outputs(PowerOutputsCommand),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutorCommand {
    Pd(PdBusCommand),
    Persist(PersistCommand),
    PowerGate(PowerGateCommand),
    PowerOutputs(PowerOutputsCommand),
}

impl From<Action> for ExecutorCommand {
    fn from(action: Action) -> Self {
        match action {
            Action::Pd {
                operation,
                port,
                slot_epoch,
                kind,
            } => Self::Pd(PdBusCommand {
                operation,
                port,
                slot_epoch,
                kind,
            }),
            Action::PersistConfig { revision, settings } => {
                Self::Persist(PersistCommand { revision, settings })
            }
            Action::PowerGate {
                operation,
                port,
                slot_epoch,
                output_bit,
                enabled,
            } => Self::PowerGate(PowerGateCommand {
                operation,
                port,
                slot_epoch,
                output_bit,
                enabled,
            }),
            Action::PowerOutputs { operation, kind } => {
                Self::PowerOutputs(PowerOutputsCommand { operation, kind })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerCompletion {
    Pd {
        operation: OperationId,
        port: PortId,
        slot_epoch: SlotEpoch,
        outcome: CompletionOutcome,
    },
    Persist {
        revision: ConfigRevision,
        outcome: CompletionOutcome,
    },
    PowerGate {
        operation: OperationId,
        port: PortId,
        slot_epoch: SlotEpoch,
        enabled: bool,
        outcome: CompletionOutcome,
    },
    PowerOutputs {
        operation: OperationId,
        kind: PowerOutputsActionKind,
        outcome: CompletionOutcome,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_types::{ConfigRevision, PortPolicy};

    #[test]
    fn action_mapping_preserves_stale_completion_guards() {
        let command = ExecutorCommand::from(Action::Pd {
            operation: OperationId(17),
            port: PortId::new(5).unwrap(),
            slot_epoch: SlotEpoch(9),
            kind: PdActionKind::ApplyPolicy(PortPolicy::SAFE_DISABLED),
        });
        assert_eq!(
            command,
            ExecutorCommand::Pd(PdBusCommand {
                operation: OperationId(17),
                port: PortId::new(5).unwrap(),
                slot_epoch: SlotEpoch(9),
                kind: PdActionKind::ApplyPolicy(PortPolicy::SAFE_DISABLED),
            })
        );
    }

    #[test]
    fn persistence_mapping_preserves_revision_and_emergency_latch() {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.emergency_latched = true;
        assert_eq!(
            ExecutorCommand::from(Action::PersistConfig {
                revision: ConfigRevision(8),
                settings,
            }),
            ExecutorCommand::Persist(PersistCommand {
                revision: ConfigRevision(8),
                settings,
            })
        );
    }

    #[test]
    fn power_gate_mapping_preserves_output_and_epoch() {
        assert_eq!(
            ExecutorCommand::from(Action::PowerGate {
                operation: OperationId(3),
                port: PortId::new(2).unwrap(),
                slot_epoch: SlotEpoch(4),
                output_bit: 5,
                enabled: true,
            }),
            ExecutorCommand::PowerGate(PowerGateCommand {
                operation: OperationId(3),
                port: PortId::new(2).unwrap(),
                slot_epoch: SlotEpoch(4),
                output_bit: 5,
                enabled: true,
            })
        );
    }
}
