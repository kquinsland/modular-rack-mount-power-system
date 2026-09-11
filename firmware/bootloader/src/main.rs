#![no_std]
#![no_main]

use core::cell::RefCell;

use cortex_m_rt::entry;
use embassy_boot_stm32::{BootLoader, BootLoaderConfig};
use embassy_stm32::flash::Flash;
use embassy_sync::blocking_mutex::{Mutex, raw::NoopRawMutex};
use panic_halt as _;

#[entry]
fn main() -> ! {
    let peripherals = embassy_stm32::init(embassy_stm32::Config::default());
    let regions = Flash::new_blocking(peripherals.FLASH).into_blocking_regions();
    let flash = Mutex::<NoopRawMutex, _>::new(RefCell::new(regions.bank1_region));
    let config = BootLoaderConfig::from_linkerfile_blocking(&flash, &flash, &flash);
    let bootloader = BootLoader::prepare::<_, _, _, 2048>(config);

    // The linker symbols and constant must agree; keeping the jump address
    // explicit makes the boot boundary reviewable alongside memory.x.
    unsafe { bootloader.load(0x0800_6000) }
}
