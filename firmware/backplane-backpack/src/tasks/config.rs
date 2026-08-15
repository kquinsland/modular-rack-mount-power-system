use backplane_backpack_firmware::action_executor::ControllerCompletion;
use backplane_backpack_firmware::config_store::ConfigStore;
use backplane_backpack_firmware::config_store::stm32::Stm32ConfigFlash;
use pdcan_core::CompletionOutcome;
use portable_atomic::Ordering;

use crate::channels::{CONTROLLER_EVENTS, ControllerEvent, FLASH_ACTIVE, PERSIST_COMMANDS};

#[embassy_executor::task]
pub async fn config_task(mut store: ConfigStore<Stm32ConfigFlash<'static>>) {
    loop {
        let command = PERSIST_COMMANDS.receive().await;
        FLASH_ACTIVE.store(true, Ordering::Release);
        let outcome = if store.save(command.settings).is_ok() {
            CompletionOutcome::Succeeded
        } else {
            CompletionOutcome::Failed
        };
        FLASH_ACTIVE.store(false, Ordering::Release);
        CONTROLLER_EVENTS
            .send(ControllerEvent::Completion(ControllerCompletion::Persist {
                revision: command.revision,
                outcome,
            }))
            .await;
    }
}
