#![no_main]
#![no_std]

mod channels;
mod tasks;

use backplane_backpack_firmware::board::{CAN_DATA_BITRATE, CAN_NOMINAL_BITRATE};
use backplane_backpack_firmware::config_store::ConfigStore;
use backplane_backpack_firmware::config_store::stm32::Stm32ConfigFlash;
use backplane_backpack_firmware::fan::{FanDrive, THREE_WIRE_FREQUENCY_HZ_PROVISIONAL};
use backplane_backpack_firmware::status::STATUS_WORDS;
use backplane_backpack_firmware::supervisor::IWDG_TIMEOUT_US_PROVISIONAL;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::can::config::{GlobalFilter, NonMatchingFilter};
use embassy_stm32::can::{self, CanConfigurator};
use embassy_stm32::dma;
use embassy_stm32::gpio::{Level, Output, OutputType, Pull, Speed};
use embassy_stm32::i2c::{self, I2c};
use embassy_stm32::peripherals;
use embassy_stm32::rcc::{Hsi, HsiKerDiv, HsiSysDiv};
use embassy_stm32::time::Hertz;
use embassy_stm32::timer;
use embassy_stm32::timer::input_capture::{CapturePin, InputCapture};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::wdg::IndependentWatchdog;
use embassy_time::{Duration, Timer};
use panic_halt as _;
use pdcan_protocol::ResetFlags;
use pdcan_types::{NodeUid, PersistentSettings};
use static_cell::StaticCell;

const I2C_FREQUENCY_HZ: u32 = 100_000;
const I2C_TIMEOUT_MS_PROVISIONAL: u64 = 100;
const TACH_TIMER_HZ_PROVISIONAL: u32 = 1_000_000;
const WS2812_TIMER_HZ: u32 = 800_000;
const STATUS_DMA_CAPACITY: usize = STATUS_WORDS + 8;

static STATUS_DMA_BUFFER: StaticCell<[u16; STATUS_DMA_CAPACITY]> = StaticCell::new();

bind_interrupts!(struct Irqs {
    FDCAN1_IT0 => can::IT0InterruptHandler<peripherals::FDCAN1>;
    FDCAN1_IT1 => can::IT1InterruptHandler<peripherals::FDCAN1>;
    I2C2 => i2c::EventInterruptHandler<peripherals::I2C2>, i2c::ErrorInterruptHandler<peripherals::I2C2>;
    DMA1_CHANNEL1 => dma::InterruptHandler<peripherals::DMA1_CH1>;
    DMA1_CHANNEL2_3 => dma::InterruptHandler<peripherals::DMA1_CH2>, dma::InterruptHandler<peripherals::DMA1_CH3>;
    TIM17 => timer::CaptureCompareInterruptHandler<peripherals::TIM17>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let mut config = embassy_stm32::Config::default();
    config.rcc.hsi = Some(Hsi {
        sys_div: HsiSysDiv::DIV1,
        ker_div: HsiKerDiv::DIV1,
    });
    let p = embassy_stm32::init(config);
    let reset_flags = capture_reset_flags();

    // Establish safety-oriented pin states before loading flash or starting tasks.
    let mut mux_reset = Output::new(p.PA3, Level::Low, Speed::Low);
    let can_standby = Output::new(p.PA4, Level::High, Speed::Low);
    Timer::after_millis(1).await;
    mux_reset.set_high();

    let mut fan_pwm = SimplePwm::new(
        p.TIM2,
        Some(PwmPin::new(p.PA0, OutputType::PushPull)),
        None,
        None,
        None,
        Hertz::hz(THREE_WIRE_FREQUENCY_HZ_PROVISIONAL),
        CountingMode::EdgeAlignedUp,
    );
    {
        let mut channel = fan_pwm.ch1();
        channel.set_duty_cycle_percent(FanDrive::BOOT_SAFE.pa0_high_percent);
        channel.enable();
    }
    let fan_tach = InputCapture::new(
        p.TIM17,
        Some(CapturePin::new(p.PA1, Pull::None)),
        None,
        None,
        None,
        Irqs,
        Hertz::hz(TACH_TIMER_HZ_PROVISIONAL),
        CountingMode::EdgeAlignedUp,
    );

    let status_pwm = SimplePwm::new(
        p.TIM15,
        Some(PwmPin::new(p.PA2, OutputType::PushPull)),
        None,
        None,
        None,
        Hertz::hz(WS2812_TIMER_HZ),
        CountingMode::EdgeAlignedUp,
    );
    let status_buffer = STATUS_DMA_BUFFER.init([0; STATUS_DMA_CAPACITY]);
    let status_output =
        status_pwm
            .split()
            .ch1
            .into_ring_buffered_channel(p.DMA1_CH3, status_buffer, Irqs);

    let mut i2c_config = i2c::Config::default();
    i2c_config.frequency = Hertz::hz(I2C_FREQUENCY_HZ);
    i2c_config.timeout = Duration::from_millis(I2C_TIMEOUT_MS_PROVISIONAL);
    let pd_bus = I2c::new(
        p.I2C2, p.PA7, p.PA6, p.DMA1_CH1, p.DMA1_CH2, Irqs, i2c_config,
    );

    let mut can_configurator = CanConfigurator::new(p.FDCAN1, p.PA11, p.PA12, Irqs);
    can_configurator.set_bitrate(CAN_NOMINAL_BITRATE);
    can_configurator.set_fd_data_bitrate(CAN_DATA_BITRATE, false);
    let can_config = can_configurator.config().set_global_filter(
        GlobalFilter::reject_all().set_handle_extended_frames(NonMatchingFilter::IntoRxFifo0),
    );
    can_configurator.set_config(can_config);
    let can = can_configurator.into_normal_mode();

    let mut config_store = ConfigStore::new(Stm32ConfigFlash::new(p.FLASH));
    let settings = config_store
        .load()
        .ok()
        .flatten()
        .map_or(PersistentSettings::FACTORY_DEFAULT, |record| {
            record.settings
        });

    let watchdog = IndependentWatchdog::new(p.IWDG, IWDG_TIMEOUT_US_PROVISIONAL);

    spawner.spawn(tasks::status::status_task(status_output).unwrap());
    spawner.spawn(tasks::fan::fan_task(fan_pwm, fan_tach).unwrap());
    spawner.spawn(tasks::config::config_task(config_store).unwrap());
    let uid = NodeUid::from_bytes(embassy_stm32::uid::uid());
    spawner.spawn(tasks::controller::controller_task(settings, uid, reset_flags).unwrap());
    spawner.spawn(tasks::pd_bus::pd_bus_task(pd_bus, mux_reset).unwrap());
    spawner.spawn(tasks::can::can_task(can, can_standby).unwrap());
    spawner.spawn(tasks::supervisor::supervisor_task(watchdog).unwrap());

    loop {
        Timer::after_secs(60).await;
    }
}

fn capture_reset_flags() -> ResetFlags {
    let register = stm32_metapac::RCC.csr2().read();
    let mut flags = 0;
    if register.oblrstf() {
        flags |= ResetFlags::OPTION_BYTE;
    }
    if register.pinrstf() {
        flags |= ResetFlags::PIN;
    }
    if register.pwrrstf() {
        flags |= ResetFlags::POWER;
    }
    if register.sftrstf() {
        flags |= ResetFlags::SOFTWARE;
    }
    if register.iwdgrstf() {
        flags |= ResetFlags::INDEPENDENT_WATCHDOG;
    }
    if register.wwdgrstf() {
        flags |= ResetFlags::WINDOW_WATCHDOG;
    }
    if register.lpwrrstf() {
        flags |= ResetFlags::LOW_POWER;
    }
    stm32_metapac::RCC
        .csr2()
        .modify(|register| register.set_rmvf(true));
    ResetFlags::from_bits(flags)
}
