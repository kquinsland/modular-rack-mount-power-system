use backplane_backpack_firmware::BOARD;
use backplane_backpack_firmware::action_executor::{ControllerCompletion, ExecutorCommand};
use backplane_backpack_firmware::status::StatusState;
use embassy_futures::select::{Either, select};
use embassy_time::Timer;
use pdcan_core::{Controller, PdActionKind};
use pdcan_types::PersistentSettings;

use crate::channels::{
    CONTROLLER_EVENTS, CONTROLLER_PROGRESS, ControllerEvent, FAN_REQUEST, FanRequest,
    PD_EMERGENCY_COMMANDS, PD_POLICY_COMMANDS, PERSIST_COMMANDS, STATUS_REQUEST, advance,
};

#[embassy_executor::task]
pub async fn controller_task(settings: PersistentSettings) {
    let mut controller = Controller::new(BOARD, settings);
    FAN_REQUEST.signal(FanRequest {
        config: settings.fan,
        force_full_speed: settings.emergency_latched,
    });
    STATUS_REQUEST.signal(initial_status(settings));

    loop {
        dispatch_ready_actions(&mut controller).await;
        advance(&CONTROLLER_PROGRESS);

        match select(CONTROLLER_EVENTS.receive(), Timer::after_millis(100)).await {
            Either::First(event) => apply_event(&mut controller, event),
            Either::Second(()) => {}
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
        }
    }
}

fn apply_event(controller: &mut Controller, event: ControllerEvent) {
    match event {
        ControllerEvent::Completion(ControllerCompletion::Pd {
            operation,
            port,
            slot_epoch,
            outcome,
        }) => controller.complete_pd_operation(port, operation, slot_epoch, outcome),
        ControllerEvent::Completion(ControllerCompletion::Persist { revision, outcome }) => {
            controller.complete_persist(revision, outcome);
            FAN_REQUEST.signal(FanRequest {
                config: controller.settings().fan,
                force_full_speed: controller.emergency_latched(),
            });
            STATUS_REQUEST.signal(if controller.emergency_latched() {
                StatusState::EmergencyLatched
            } else {
                initial_status(controller.settings())
            });
        }
        ControllerEvent::ModuleDetected(port) => {
            let _ = controller.module_detected(port);
        }
        ControllerEvent::ModuleRemoved(port) => {
            let _ = controller.module_removed(port);
        }
    }
}

const fn initial_status(settings: PersistentSettings) -> StatusState {
    if settings.emergency_latched {
        StatusState::EmergencyLatched
    } else if settings.node_id.is_none() {
        StatusState::Uncommissioned
    } else {
        StatusState::Healthy
    }
}
