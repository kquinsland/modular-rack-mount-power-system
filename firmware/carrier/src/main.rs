#![no_std]
#![no_main]

use core::{cell::RefCell, hint::black_box};

use embassy_boot_stm32::{AlignedBuffer, BlockingFirmwareUpdater, FirmwareUpdaterConfig};
use embassy_stm32::flash::Flash;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_sync::blocking_mutex::{Mutex, raw::NoopRawMutex};
use embassy_time::Timer;
use panic_halt as _;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) -> ! {
    let _ = spawner;
    let peripherals = embassy_stm32::init(embassy_stm32::Config::default());

    // The finalized hardware is deliberately fail-safe: PD_ENABLE must remain
    // low until policy restoration, hardware checks, and startup staggering all
    // complete.
    let _pd_enable = Output::new(peripherals.PB3, Level::Low, Speed::Low);
    let _status_led = Output::new(peripherals.PB8, Level::Low, Speed::VeryHigh);

    let regions = Flash::new_blocking(peripherals.FLASH).into_blocking_regions();
    let flash = Mutex::<NoopRawMutex, _>::new(RefCell::new(regions.bank1_region));
    let updater_config = FirmwareUpdaterConfig::from_linkerfile_blocking(&flash, &flash);
    let mut state_buffer = AlignedBuffer([0; 8]);
    let mut firmware_updater = BlockingFirmwareUpdater::new(updater_config, state_buffer.as_mut());
    let boot_state = firmware_updater
        .get_state()
        .expect("Embassy Boot state partition must be readable");
    black_box(boot_state);

    // Do not mark a trial image booted yet. Confirmation belongs after the CAN,
    // watchdog, configuration, and critical peripheral health gates are live.
    loop {
        Timer::after_secs(1).await;
    }
}
