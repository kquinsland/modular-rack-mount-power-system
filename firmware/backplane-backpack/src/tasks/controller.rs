use backplane_backpack_firmware::BOARD;
use backplane_backpack_firmware::action_executor::{
    ControllerCompletion, ExecutorCommand, PowerCommand,
};
use backplane_backpack_firmware::command_service::{CommandService, ServiceOutput};
use backplane_backpack_firmware::status::{StatusCommand, StatusState};
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer};
use pdcan_core::{Controller, PdActionKind, PowerOutputsActionKind};
use pdcan_protocol::{ControlCommand, ResetFlags};
use pdcan_types::{CommissioningState, FirmwareVersion, NodeUid, PersistentSettings};

use crate::channels::{
    CAN_TX, CONTROLLER_EVENTS, CONTROLLER_PROGRESS, ControllerEvent, FAN_REQUEST, FanRequest,
    PD_EMERGENCY_COMMANDS, PD_POLICY_COMMANDS, PERSIST_COMMANDS, POWER_COMMANDS,
    POWER_EMERGENCY_COMMANDS, POWER_OFF_COMMANDS, STATUS_REQUEST, advance,
};

const CLAIM_WINDOW_MS: u64 = 500;
const CLAIM_ROUND_INTERVAL_MS: u64 = 150;
const LAST_CLAIM_NONCE: u16 = 2;
const HEARTBEAT_INTERVAL_MS: u64 = 1_000;
const FIRMWARE_VERSION: FirmwareVersion = FirmwareVersion {
    major: 0,
    minor: 1,
    patch: 0,
};

#[embassy_executor::task]
pub async fn controller_task(settings: PersistentSettings, uid: NodeUid, reset_flags: ResetFlags) {
    let started_at = Instant::now();
    let mut controller = Controller::new(BOARD, settings);
    let mut service = CommandService::new(BOARD, FIRMWARE_VERSION, uid, settings.node_id);
    service.set_reset_flags(reset_flags);
    let mut claim_deadline = settings
        .node_id
        .map(|_| Instant::now() + Duration::from_millis(CLAIM_WINDOW_MS));
    let mut next_claim_round_deadline = settings
        .node_id
        .map(|_| Instant::now() + Duration::from_millis(CLAIM_ROUND_INTERVAL_MS));
    let mut next_claim_nonce = 1;
    let mut last_status = status_for(&controller, &service);
    let mut heartbeat_deadline = Instant::now() + Duration::from_millis(HEARTBEAT_INTERVAL_MS);

    FAN_REQUEST.signal(FanRequest {
        config: settings.fan,
        force_full_speed: settings.emergency_latched,
    });
    STATUS_REQUEST.signal(StatusCommand::SetBase(last_status));
    let startup = service.startup_claim();
    dispatch_service_output(&startup).await;

    loop {
        dispatch_ready_actions(&mut controller).await;
        advance(&CONTROLLER_PROGRESS);

        match select(CONTROLLER_EVENTS.receive(), Timer::after_millis(100)).await {
            Either::First(event) => {
                service.set_uptime_seconds(
                    u32::try_from((Instant::now() - started_at).as_secs()).unwrap_or(u32::MAX),
                );
                let emergency_requested = matches!(
                    event,
                    ControllerEvent::CanControl(request)
                        if matches!(request.command, ControlCommand::EmergencyDisable)
                );
                let persist_completed = matches!(
                    event,
                    ControllerEvent::Completion(ControllerCompletion::Persist { .. })
                );
                let output = apply_event(&mut controller, &mut service, event);
                dispatch_service_output(&output).await;
                if emergency_requested || persist_completed {
                    FAN_REQUEST.signal(FanRequest {
                        config: controller.settings().fan,
                        force_full_speed: controller.emergency_latched(),
                    });
                }
            }
            Either::Second(()) => {}
        }

        if service.commissioning().state() == CommissioningState::Claiming
            && next_claim_round_deadline.is_some_and(|deadline| Instant::now() >= deadline)
            && next_claim_nonce <= LAST_CLAIM_NONCE
        {
            let claim = service.claim_round(next_claim_nonce);
            dispatch_service_output(&claim).await;
            next_claim_nonce = next_claim_nonce.wrapping_add(1);
            next_claim_round_deadline = (next_claim_nonce <= LAST_CLAIM_NONCE)
                .then(|| Instant::now() + Duration::from_millis(CLAIM_ROUND_INTERVAL_MS));
        }
        if claim_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            service.claim_window_complete();
            claim_deadline = None;
            next_claim_round_deadline = None;
        }
        if service.commissioning().state() == CommissioningState::Claiming
            && claim_deadline.is_none()
        {
            claim_deadline = Some(Instant::now() + Duration::from_millis(CLAIM_WINDOW_MS));
            next_claim_nonce = 1;
            next_claim_round_deadline =
                Some(Instant::now() + Duration::from_millis(CLAIM_ROUND_INTERVAL_MS));
        } else if service.commissioning().state() != CommissioningState::Claiming {
            claim_deadline = None;
            next_claim_round_deadline = None;
        }

        if Instant::now() >= heartbeat_deadline {
            let heartbeat = service.heartbeat(&controller);
            dispatch_service_output(&heartbeat).await;
            heartbeat_deadline = Instant::now() + Duration::from_millis(HEARTBEAT_INTERVAL_MS);
        }

        let status = status_for(&controller, &service);
        if status != last_status {
            STATUS_REQUEST.signal(StatusCommand::SetBase(status));
            last_status = status;
        }
    }
}

async fn dispatch_ready_actions(controller: &mut Controller) {
    while let Some(action) = controller.next_action() {
        match ExecutorCommand::from(action) {
            ExecutorCommand::Pd(command) => match command.kind {
                PdActionKind::EmergencyDisable => PD_EMERGENCY_COMMANDS.send(command).await,
                PdActionKind::ApplyPolicy(_) => PD_POLICY_COMMANDS.send(command).await,
            },
            ExecutorCommand::Persist(command) => PERSIST_COMMANDS.send(command).await,
            ExecutorCommand::PowerGate(command) => {
                let command = PowerCommand::Gate(command);
                if matches!(command, PowerCommand::Gate(command) if !command.enabled) {
                    POWER_OFF_COMMANDS.send(command).await;
                } else {
                    POWER_COMMANDS.send(command).await;
                }
            }
            ExecutorCommand::PowerOutputs(command) => {
                let emergency = matches!(command.kind, PowerOutputsActionKind::EmergencyDisableAll);
                let command = PowerCommand::Outputs(command);
                if emergency {
                    POWER_EMERGENCY_COMMANDS.send(command).await;
                } else {
                    POWER_COMMANDS.send(command).await;
                }
            }
        }
    }
}

fn apply_event(
    controller: &mut Controller,
    service: &mut CommandService,
    event: ControllerEvent,
) -> ServiceOutput {
    match event {
        ControllerEvent::Completion(ControllerCompletion::Pd {
            operation,
            port,
            slot_epoch,
            outcome,
        }) => {
            controller.complete_pd_operation(port, operation, slot_epoch, outcome);
            ServiceOutput::EMPTY
        }
        ControllerEvent::Completion(ControllerCompletion::Persist { revision, outcome }) => {
            let completion = controller.complete_persist(revision, outcome);
            service.complete_persist(completion)
        }
        ControllerEvent::Completion(ControllerCompletion::PowerGate {
            operation,
            port,
            slot_epoch,
            enabled,
            outcome,
        }) => {
            controller.complete_power_gate_operation(port, operation, slot_epoch, enabled, outcome);
            ServiceOutput::EMPTY
        }
        ControllerEvent::Completion(ControllerCompletion::PowerOutputs {
            operation,
            kind,
            outcome,
        }) => {
            controller.complete_power_outputs_operation(operation, kind, outcome);
            ServiceOutput::EMPTY
        }
        ControllerEvent::ModuleDetected(port) => {
            let _ = controller.module_detected(port);
            ServiceOutput::EMPTY
        }
        ControllerEvent::ModuleRemoved(port) => {
            let _ = controller.module_removed(port);
            ServiceOutput::EMPTY
        }
        ControllerEvent::CanControl(request) => service.handle_control(controller, request),
        ControllerEvent::CanCommissioning { requester, message } => {
            service.handle_commissioning(controller, requester, message)
        }
    }
}

async fn dispatch_service_output(output: &ServiceOutput) {
    for frame in output.frames.iter().flatten().copied() {
        CAN_TX.send(frame).await;
    }
    if let Some(seconds) = output.identify_seconds {
        STATUS_REQUEST.signal(StatusCommand::IdentifySeconds(seconds));
    }
}

const fn status_for(controller: &Controller, service: &CommandService) -> StatusState {
    if controller.emergency_latched() {
        StatusState::EmergencyLatched
    } else {
        match service.commissioning().state() {
            CommissioningState::AddressConflict => StatusState::AddressConflict,
            CommissioningState::Commissioned => StatusState::Healthy,
            CommissioningState::Uncommissioned | CommissioningState::Claiming => {
                StatusState::Uncommissioned
            }
        }
    }
}
