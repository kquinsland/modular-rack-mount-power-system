use backplane_backpack_firmware::status::{
    IDENTIFY_COLOR, Rgb, STATUS_WORDS, StatusCommand, StatusState, encode_ws2812_pwm, render,
};
use embassy_futures::select::{Either, select};
use embassy_stm32::peripherals;
use embassy_stm32::timer::ringbuffered::RingBufferedPwmChannel;
use embassy_time::{Duration, Instant, Timer};

use crate::channels::STATUS_REQUEST;

const RESET_HOLD_US: u64 = 80;
const IDENTIFY_BLINK_MS: u64 = 250;

#[embassy_executor::task]
pub async fn status_task(mut output: RingBufferedPwmChannel<'static, peripherals::TIM15, u16>) {
    let mut base = StatusState::Booting;
    let mut identify_until = None;
    let mut identify_on = false;
    show(&mut output, render(base)).await;

    loop {
        match select(
            STATUS_REQUEST.wait(),
            Timer::after_millis(IDENTIFY_BLINK_MS),
        )
        .await
        {
            Either::First(StatusCommand::SetBase(state)) => base = state,
            Either::First(StatusCommand::IdentifySeconds(0)) => identify_until = None,
            Either::First(StatusCommand::IdentifySeconds(seconds)) => {
                identify_until = Some(Instant::now() + Duration::from_secs(u64::from(seconds)));
                identify_on = true;
            }
            Either::Second(()) => identify_on = !identify_on,
        }

        if identify_until.is_some_and(|deadline| Instant::now() >= deadline) {
            identify_until = None;
        }
        let color = if identify_until.is_some() {
            if identify_on {
                IDENTIFY_COLOR
            } else {
                Rgb::OFF
            }
        } else {
            render(base)
        };
        show(&mut output, color).await;
    }
}

async fn show(output: &mut RingBufferedPwmChannel<'static, peripherals::TIM15, u16>, color: Rgb) {
    let mut words = [0_u16; STATUS_WORDS];
    encode_ws2812_pwm(color, output.max_duty_cycle(), &mut words);

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
