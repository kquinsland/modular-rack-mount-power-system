use embassy_futures::select::{Either, select};
use embassy_stm32::can::Can;
use embassy_stm32::gpio::Output;
use embassy_time::Timer;

use crate::channels::{CAN_PROGRESS, advance};

const HEALTH_TICK_MS: u64 = 250;

/// Owns FDCAN for the lifetime of the firmware.
///
/// This first hardware-independent slice deliberately stops at frame reception.
/// Dispatch and response encoding stay behind the protocol-freeze hardware spike;
/// the task establishes compile-checked interrupt, pin, clock, FD/BRS, and
/// ownership integration for later physical validation.
#[embassy_executor::task]
pub async fn can_task(mut can: Can<'static>, mut transceiver_standby: Output<'static>) {
    transceiver_standby.set_low();
    loop {
        match select(can.read_fd(), Timer::after_millis(HEALTH_TICK_MS)).await {
            Either::First(_received) => {}
            Either::Second(()) => {}
        }
        advance(&CAN_PROGRESS);
    }
}
