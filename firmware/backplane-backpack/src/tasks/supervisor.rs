use backplane_backpack_firmware::supervisor::{
    ProgressDeadlines, ProgressSnapshot, Supervisor, SupervisorDecision,
};
use embassy_stm32::peripherals;
use embassy_stm32::wdg::IndependentWatchdog;
use embassy_time::{Instant, Timer};
use portable_atomic::Ordering;

use crate::channels::{CAN_PROGRESS, CONTROLLER_PROGRESS, FLASH_ACTIVE, PD_BUS_PROGRESS, load};

#[embassy_executor::task]
pub async fn supervisor_task(mut watchdog: IndependentWatchdog<'static, peripherals::IWDG>) {
    let mut supervisor = Supervisor::new(ProgressDeadlines::PROVISIONAL, now_ms());
    watchdog.unleash();

    loop {
        Timer::after_millis(250).await;
        let decision = supervisor.evaluate(
            now_ms(),
            ProgressSnapshot {
                controller: load(&CONTROLLER_PROGRESS),
                pd_bus: load(&PD_BUS_PROGRESS),
                can: load(&CAN_PROGRESS),
            },
            FLASH_ACTIVE.load(Ordering::Acquire),
        );
        if decision == SupervisorDecision::FeedWatchdog {
            watchdog.pet();
        }
    }
}

fn now_ms() -> u32 {
    let bytes = Instant::now().as_millis().to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
