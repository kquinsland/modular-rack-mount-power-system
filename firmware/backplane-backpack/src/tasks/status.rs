use backplane_backpack_firmware::status::{STATUS_WORDS, StatusState, encode_ws2812_pwm, render};
use embassy_stm32::peripherals;
use embassy_stm32::timer::ringbuffered::RingBufferedPwmChannel;
use embassy_time::Timer;

use crate::channels::STATUS_REQUEST;

const RESET_HOLD_US: u64 = 80;

#[embassy_executor::task]
pub async fn status_task(mut output: RingBufferedPwmChannel<'static, peripherals::TIM15, u16>) {
    show(&mut output, StatusState::Booting).await;
    loop {
        show(&mut output, STATUS_REQUEST.wait().await).await;
    }
}

async fn show(
    output: &mut RingBufferedPwmChannel<'static, peripherals::TIM15, u16>,
    state: StatusState,
) {
    let mut words = [0_u16; STATUS_WORDS];
    encode_ws2812_pwm(render(state), output.max_duty_cycle(), &mut words);

    output.clear();
    if output.write_immediate(&words).is_ok() {
        output.enable();
        output.start();
        output.stop().await;
    } else {
        output.request_reset();
    }
    output.disable();
    Timer::after_micros(RESET_HOLD_US).await;
}
