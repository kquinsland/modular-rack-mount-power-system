#![no_main]
#![no_std]

use backplane_backpack_firmware::BOARD;
use cortex_m_rt::entry;
use panic_halt as _;

#[entry]
fn main() -> ! {
    let _board = BOARD;

    // Phase-1 linker/startup proof. Clock, FDCAN, I2C, watchdog, and Embassy task
    // bring-up are intentionally the next hardware-dependent vertical slice.
    loop {
        cortex_m::asm::wfi();
    }
}
