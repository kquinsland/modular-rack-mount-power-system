use backplane_backpack_firmware::fan::{FanDrive, THREE_WIRE_FREQUENCY_HZ_PROVISIONAL};
use embassy_futures::select::{Either, select};
use embassy_stm32::peripherals;
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::Channel;
use embassy_stm32::timer::input_capture::InputCapture;
use embassy_stm32::timer::simple_pwm::SimplePwm;

use crate::channels::FAN_REQUEST;

#[embassy_executor::task]
pub async fn fan_task(
    mut pwm: SimplePwm<'static, peripherals::TIM2>,
    mut tach: InputCapture<'static, peripherals::TIM17>,
) {
    apply_drive(&mut pwm, FanDrive::BOOT_SAFE);

    loop {
        match select(FAN_REQUEST.wait(), tach.wait_for_rising_edge(Channel::Ch1)).await {
            Either::First(request) => {
                let drive = FanDrive::from_config(request.config, request.force_full_speed)
                    .unwrap_or(FanDrive::BOOT_SAFE);
                apply_drive(&mut pwm, drive);
            }
            // Captures prove the tach input is live. RPM calculation and fan-stall
            // thresholds remain provisional until the actual fan is characterized.
            Either::Second(_capture) => {}
        }
    }
}

fn apply_drive(pwm: &mut SimplePwm<'static, peripherals::TIM2>, drive: FanDrive) {
    let frequency = if drive.frequency_hz == 0 {
        THREE_WIRE_FREQUENCY_HZ_PROVISIONAL
    } else {
        drive.frequency_hz
    };
    pwm.set_frequency(Hertz::hz(frequency));
    let mut channel = pwm.ch1();
    channel.set_duty_cycle_percent(drive.pa0_high_percent);
    channel.enable();
}
