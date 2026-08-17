use backplane_backpack_firmware::BOARD;
use backplane_backpack_firmware::action_executor::{ControllerCompletion, PdBusCommand};
#[cfg(feature = "board-rev-b")]
use backplane_backpack_firmware::action_executor::{
    PowerCommand, PowerGateCommand, PowerOutputsCommand,
};
#[cfg(feature = "board-rev-b")]
use backplane_backpack_firmware::board::{
    BOARD_TEMPERATURE_SENSOR_ADDRESS, POWER_EXPANDER_ADDRESS, POWER_GATE_SETTLE_MS_PROVISIONAL,
};
#[cfg(feature = "board-rev-b")]
use embassy_futures::select::{Either, select};
use embassy_futures::select::{Either3, select3};
use embassy_stm32::gpio::Output;
use embassy_stm32::i2c::{Error as I2cError, I2c, Master};
use embassy_stm32::mode::Async;
use embassy_time::{Duration, Instant, Timer};
#[cfg(feature = "board-rev-b")]
use pdcan_core::PowerOutputsActionKind;
use pdcan_core::{CompletionOutcome, PdActionKind};
#[cfg(feature = "board-rev-b")]
use pdcan_drivers::pca9554::Pca9554;
use pdcan_drivers::sw3538::{DEFAULT_ADDRESS, Error as Sw3538Error, FixedPdoPlan, Sw3538};
use pdcan_drivers::tca9548a::Tca9548a;
use pdcan_drivers::tmp102::Tmp102;
use pdcan_types::{MAX_PORTS, PortId, PortPolicy};
use portable_atomic::Ordering;

use crate::channels::{
    BOARD_TEMPERATURE, BoardTemperatureSample, CONTROLLER_EVENTS, ControllerEvent, PD_BUS_PROGRESS,
    PD_EMERGENCY_COMMANDS, PD_POLICY_COMMANDS, POWERED_PORTS, advance,
};
#[cfg(feature = "board-rev-b")]
use crate::channels::{POWER_COMMANDS, POWER_EMERGENCY_COMMANDS, POWER_OFF_COMMANDS};

const PROBE_INTERVAL_MS_PROVISIONAL: u64 = 250;
const MUX_RESET_PULSE_MS: u64 = 1;
const FAILURE_RETRY_DELAY_MS_PROVISIONAL: u64 = 100;
const BOARD_TEMPERATURE_INTERVAL_MS: u64 = 1_000;

struct ProbeState {
    present: [bool; MAX_PORTS],
    observed_powered: [bool; MAX_PORTS],
    probed_since_powered: [bool; MAX_PORTS],
    next_port: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusEvent {
    Pd(PdBusCommand),
    #[cfg(feature = "board-rev-b")]
    Power(PowerCommand),
    Probe,
}

impl ProbeState {
    const INITIAL: Self = Self {
        present: [false; MAX_PORTS],
        observed_powered: [false; MAX_PORTS],
        probed_since_powered: [false; MAX_PORTS],
        next_port: PortId::MIN,
    };
}

#[embassy_executor::task]
pub async fn pd_bus_task(
    mut bus: I2c<'static, Async, Master>,
    mut mux_reset: Option<Output<'static>>,
) {
    let mux = Tca9548a::default_address();
    #[cfg(feature = "board-rev-b")]
    let mut power = Pca9554::new(POWER_EXPANDER_ADDRESS);
    let temperature_sensor = board_temperature_sensor();
    let mut temperature_sequence = 0_u16;
    let mut devices: [Sw3538; MAX_PORTS] =
        core::array::from_fn(|_| Sw3538::assuming_base_bank(DEFAULT_ADDRESS));
    let mut probe_state = ProbeState::INITIAL;
    let mut probe_deadline = Instant::now() + Duration::from_millis(PROBE_INTERVAL_MS_PROVISIONAL);
    let mut temperature_deadline =
        Instant::now() + Duration::from_millis(BOARD_TEMPERATURE_INTERVAL_MS);

    #[cfg(feature = "board-rev-b")]
    initialize_shared_bus(&mut bus, mux, &mut mux_reset, &mut power).await;
    #[cfg(feature = "board-rev-a")]
    initialize_shared_bus(&mut bus, mux, &mut mux_reset).await;
    advance(&PD_BUS_PROGRESS);

    loop {
        #[cfg(feature = "board-rev-b")]
        if let Ok(command) = POWER_EMERGENCY_COMMANDS.try_receive() {
            process_power_command(&mut bus, &mut power, command).await;
            advance(&PD_BUS_PROGRESS);
            continue;
        }
        #[cfg(feature = "board-rev-b")]
        if let Ok(command) = POWER_OFF_COMMANDS.try_receive() {
            process_power_command(&mut bus, &mut power, command).await;
            advance(&PD_BUS_PROGRESS);
            continue;
        }
        if let Ok(command) = PD_EMERGENCY_COMMANDS.try_receive() {
            process_command(&mut bus, mux, &mut mux_reset, &mut devices, command).await;
            advance(&PD_BUS_PROGRESS);
            continue;
        }
        if read_board_temperature_if_due(
            &mut bus,
            mux,
            &mut mux_reset,
            temperature_sensor,
            &mut temperature_deadline,
            &mut temperature_sequence,
        )
        .await
        {
            advance(&PD_BUS_PROGRESS);
            continue;
        }
        if probe_if_due(
            &mut bus,
            mux,
            &mut mux_reset,
            &mut devices,
            &mut probe_state,
            &mut probe_deadline,
        )
        .await
        {
            advance(&PD_BUS_PROGRESS);
            continue;
        }

        match next_bus_event(
            probe_deadline,
            temperature_sensor.map(|_| temperature_deadline),
        )
        .await
        {
            BusEvent::Pd(command) => {
                process_command(&mut bus, mux, &mut mux_reset, &mut devices, command).await;
            }
            #[cfg(feature = "board-rev-b")]
            BusEvent::Power(command) => {
                process_power_command(&mut bus, &mut power, command).await;
            }
            BusEvent::Probe => {
                if !read_board_temperature_if_due(
                    &mut bus,
                    mux,
                    &mut mux_reset,
                    temperature_sensor,
                    &mut temperature_deadline,
                    &mut temperature_sequence,
                )
                .await
                {
                    let _ = probe_if_due(
                        &mut bus,
                        mux,
                        &mut mux_reset,
                        &mut devices,
                        &mut probe_state,
                        &mut probe_deadline,
                    )
                    .await;
                }
            }
        }
        advance(&PD_BUS_PROGRESS);
    }
}

#[cfg(feature = "board-rev-a")]
const fn board_temperature_sensor() -> Option<Tmp102> {
    None
}

#[cfg(feature = "board-rev-b")]
#[allow(clippy::unnecessary_wraps)]
const fn board_temperature_sensor() -> Option<Tmp102> {
    Some(Tmp102::new(BOARD_TEMPERATURE_SENSOR_ADDRESS))
}

#[cfg(feature = "board-rev-a")]
async fn initialize_shared_bus(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
) {
    POWERED_PORTS.store(0, Ordering::Release);
    if mux.deselect_all(bus).await.is_err() {
        reset_mux_if_available(mux_reset).await;
    }
}

#[cfg(feature = "board-rev-b")]
async fn initialize_shared_bus(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    power: &mut Pca9554,
) {
    POWERED_PORTS.store(0, Ordering::Release);
    let _ = power
        .initialize_outputs_low(bus, BOARD.power_gate_mask())
        .await;
    if mux.deselect_all(bus).await.is_err() {
        reset_mux_if_available(mux_reset).await;
    }
}

#[cfg(feature = "board-rev-a")]
async fn next_bus_event(
    probe_deadline: Instant,
    _temperature_deadline: Option<Instant>,
) -> BusEvent {
    match select3(
        PD_EMERGENCY_COMMANDS.receive(),
        PD_POLICY_COMMANDS.receive(),
        Timer::at(probe_deadline),
    )
    .await
    {
        Either3::First(command) | Either3::Second(command) => BusEvent::Pd(command),
        Either3::Third(()) => BusEvent::Probe,
    }
}

#[cfg(feature = "board-rev-b")]
async fn next_bus_event(
    probe_deadline: Instant,
    temperature_deadline: Option<Instant>,
) -> BusEvent {
    let periodic_deadline = temperature_deadline.map_or(probe_deadline, |temperature_deadline| {
        if temperature_deadline < probe_deadline {
            temperature_deadline
        } else {
            probe_deadline
        }
    });
    match select(
        select3(
            POWER_EMERGENCY_COMMANDS.receive(),
            POWER_OFF_COMMANDS.receive(),
            PD_EMERGENCY_COMMANDS.receive(),
        ),
        select3(
            POWER_COMMANDS.receive(),
            PD_POLICY_COMMANDS.receive(),
            Timer::at(periodic_deadline),
        ),
    )
    .await
    {
        Either::First(Either3::First(command) | Either3::Second(command)) => {
            BusEvent::Power(command)
        }
        Either::Second(Either3::First(command)) => BusEvent::Power(command),
        Either::First(Either3::Third(command)) | Either::Second(Either3::Second(command)) => {
            BusEvent::Pd(command)
        }
        Either::Second(Either3::Third(())) => BusEvent::Probe,
    }
}

#[cfg(feature = "board-rev-b")]
async fn process_power_command(
    bus: &mut I2c<'static, Async, Master>,
    power: &mut Pca9554,
    command: PowerCommand,
) {
    match command {
        PowerCommand::Gate(command) => process_power_gate(bus, power, command).await,
        PowerCommand::Outputs(command) => process_power_outputs(bus, power, command).await,
    }
}

#[cfg(feature = "board-rev-b")]
async fn process_power_gate(
    bus: &mut I2c<'static, Async, Master>,
    power: &mut Pca9554,
    command: PowerGateCommand,
) {
    let valid_mapping = BOARD.power_gate_bit(command.port) == Some(command.output_bit);
    let mut succeeded = valid_mapping
        && power
            .set_output(bus, command.output_bit, command.enabled)
            .await
            .is_ok();

    if succeeded && command.enabled {
        // Do not expose the port to probing until its input rail has settled.
        // An emergency command may preempt this delay, but it cannot preempt an
        // I2C transaction already in progress on the shared upstream bus.
        match select(
            POWER_EMERGENCY_COMMANDS.receive(),
            Timer::after_millis(POWER_GATE_SETTLE_MS_PROVISIONAL),
        )
        .await
        {
            Either::First(PowerCommand::Outputs(emergency)) => {
                process_power_outputs(bus, power, emergency).await;
                succeeded = false;
            }
            Either::First(PowerCommand::Gate(_)) => {
                // Only all-output commands belong in the emergency queue.
                // Fail closed if that channel invariant is ever violated.
                let _ = power.all_off(bus).await;
                POWERED_PORTS.store(0, Ordering::Release);
                succeeded = false;
            }
            Either::Second(()) => {}
        }
    } else if succeeded {
        POWERED_PORTS.fetch_and(!(1 << command.port.get()), Ordering::AcqRel);
    } else {
        // The PCA9554 has no asynchronous output-enable pin. This second write
        // is a best-effort fail-closed request; a wedged I2C bus can leave the
        // physical output state unknown until hardware or bus recovery.
        let _ = power.all_off(bus).await;
        POWERED_PORTS.store(0, Ordering::Release);
    }

    CONTROLLER_EVENTS
        .send(ControllerEvent::Completion(
            ControllerCompletion::PowerGate {
                operation: command.operation,
                port: command.port,
                slot_epoch: command.slot_epoch,
                enabled: command.enabled,
                outcome: completion(succeeded),
            },
        ))
        .await;
    if succeeded && command.enabled {
        // Publish only after the core has received the completed transition.
        POWERED_PORTS.fetch_or(1 << command.port.get(), Ordering::AcqRel);
    }
}

#[cfg(feature = "board-rev-b")]
async fn process_power_outputs(
    bus: &mut I2c<'static, Async, Master>,
    power: &mut Pca9554,
    command: PowerOutputsCommand,
) {
    let succeeded = match command.kind {
        PowerOutputsActionKind::EmergencyDisableAll => power.all_off(bus).await.is_ok(),
        PowerOutputsActionKind::ArmAllOff => power
            .initialize_outputs_low(bus, BOARD.power_gate_mask())
            .await
            .is_ok(),
    };
    POWERED_PORTS.store(0, Ordering::Release);

    CONTROLLER_EVENTS
        .send(ControllerEvent::Completion(
            ControllerCompletion::PowerOutputs {
                operation: command.operation,
                kind: command.kind,
                outcome: completion(succeeded),
            },
        ))
        .await;
}

#[cfg(feature = "board-rev-b")]
const fn completion(succeeded: bool) -> CompletionOutcome {
    if succeeded {
        CompletionOutcome::Succeeded
    } else {
        CompletionOutcome::Failed
    }
}

async fn process_command(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    devices: &mut [Sw3538; MAX_PORTS],
    command: PdBusCommand,
) {
    let succeeded = execute_command(bus, mux, devices, command).await;
    if !succeeded {
        reset_mux_if_available(mux_reset).await;
    }
    CONTROLLER_EVENTS
        .send(ControllerEvent::Completion(ControllerCompletion::Pd {
            operation: command.operation,
            port: command.port,
            slot_epoch: command.slot_epoch,
            outcome: if succeeded {
                CompletionOutcome::Succeeded
            } else {
                CompletionOutcome::Failed
            },
        }))
        .await;

    if !succeeded && PD_EMERGENCY_COMMANDS.is_empty() && PD_POLICY_COMMANDS.is_empty() {
        Timer::after_millis(FAILURE_RETRY_DELAY_MS_PROVISIONAL).await;
    }
}

fn next_probe_deadline() -> Instant {
    Instant::now() + Duration::from_millis(PROBE_INTERVAL_MS_PROVISIONAL)
}

fn next_temperature_deadline() -> Instant {
    Instant::now() + Duration::from_millis(BOARD_TEMPERATURE_INTERVAL_MS)
}

async fn read_board_temperature(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    sensor: Tmp102,
    sequence: &mut u16,
) {
    // The sensor sits on the direct/upstream bus. Deselect every downstream
    // channel first so a module with the same address cannot contend with it.
    if mux.deselect_all(bus).await.is_err() {
        reset_mux_if_available(mux_reset).await;
        return;
    }
    match sensor.read_temperature_centi_c(bus).await {
        Ok(temperature_centi_c) => {
            BOARD_TEMPERATURE.signal(BoardTemperatureSample {
                sequence: *sequence,
                temperature_centi_c,
            });
            *sequence = (*sequence).wrapping_add(1);
        }
        Err(_) => reset_mux_if_available(mux_reset).await,
    }
}

async fn read_board_temperature_if_due(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    sensor: Option<Tmp102>,
    deadline: &mut Instant,
    sequence: &mut u16,
) -> bool {
    let Some(sensor) = sensor else {
        return false;
    };
    if Instant::now() < *deadline {
        return false;
    }
    *deadline = next_temperature_deadline();
    read_board_temperature(bus, mux, mux_reset, sensor, sequence).await;
    true
}

async fn probe_if_due(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    devices: &mut [Sw3538; MAX_PORTS],
    state: &mut ProbeState,
    deadline: &mut Instant,
) -> bool {
    if Instant::now() < *deadline {
        return false;
    }
    *deadline = next_probe_deadline();
    probe_next_port(bus, mux, mux_reset, devices, state).await;
    true
}

async fn probe_next_port(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    devices: &mut [Sw3538; MAX_PORTS],
    state: &mut ProbeState,
) {
    let port = PortId::new(state.next_port).expect("probe index is always a logical port");
    state.next_port = if state.next_port == PortId::MAX {
        PortId::MIN
    } else {
        state.next_port + 1
    };
    if BOARD.supports(port) {
        if BOARD.power_gate_bit(port).is_some() {
            let powered = POWERED_PORTS.load(Ordering::Acquire) & (1 << port.get()) != 0;
            if !powered {
                state.observed_powered[port.index()] = false;
                state.probed_since_powered[port.index()] = false;
                state.present[port.index()] = false;
                return;
            }
            if !state.observed_powered[port.index()] {
                state.observed_powered[port.index()] = true;
                state.probed_since_powered[port.index()] = false;
                state.present[port.index()] = false;
                devices[port.index()].assume_base_bank();
            }
        }
        let report_initial_result =
            BOARD.power_gate_bit(port).is_some() && !state.probed_since_powered[port.index()];
        probe_port(
            bus,
            mux,
            mux_reset,
            &mut devices[port.index()],
            &mut state.present[port.index()],
            port,
            report_initial_result,
        )
        .await;
        state.probed_since_powered[port.index()] = true;
    }
}

async fn execute_command(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    devices: &mut [Sw3538; MAX_PORTS],
    command: PdBusCommand,
) -> bool {
    if mux.select_port(bus, BOARD, command.port).await.is_err() {
        return false;
    }

    let device = &mut devices[command.port.index()];
    let operation_succeeded = match command.kind {
        PdActionKind::EmergencyDisable => {
            if device.set_cc_undriven(bus, true).await.is_err() {
                false
            } else {
                let disabled = FixedPdoPlan::from_policy(PortPolicy::SAFE_DISABLED)
                    .expect("the safe disabled policy is valid");
                device.apply_fixed_pdo_plan(bus, disabled).await.is_ok()
            }
        }
        PdActionKind::ApplyPolicy(policy) => apply_policy(device, bus, policy).await,
    };
    let deselected = mux.deselect_all(bus).await.is_ok();
    operation_succeeded && deselected
}

async fn apply_policy(
    device: &mut Sw3538,
    bus: &mut I2c<'static, Async, Master>,
    policy: PortPolicy,
) -> bool {
    let Ok(plan) = FixedPdoPlan::from_policy(policy) else {
        return false;
    };

    if !policy.enabled && device.set_cc_undriven(bus, true).await.is_err() {
        return false;
    }
    if device.apply_fixed_pdo_plan(bus, plan).await.is_err() {
        return false;
    }
    if policy.enabled {
        if device.set_cc_undriven(bus, false).await.is_err() {
            return false;
        }
        if device.send_source_capabilities(bus).await.is_err() {
            return false;
        }
    }
    true
}

async fn probe_port(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Option<Output<'static>>,
    device: &mut Sw3538,
    was_present: &mut bool,
    port: PortId,
    report_initial_result: bool,
) {
    if !*was_present {
        // An absent-to-present transition should be a newly powered device. A
        // false absence or MCU-only reset makes this assumption a HIL concern.
        device.assume_base_bank();
    }
    let mut recover_mux = false;
    let mut preserve_presence = false;
    let detected = if let Ok(()) = mux.select_port(bus, BOARD, port).await {
        let detected = match device.read_identity(bus).await {
            Ok(_) => true,
            // An unpopulated slot is an expected operating state, not a bus
            // fault. Other failures may have left the mux or I2C peripheral
            // wedged and take the board-dependent recovery path below.
            Err(Sw3538Error::Bus(I2cError::Nack)) => false,
            Err(Sw3538Error::BankStateUnknown) => {
                // Even legacy boards with a mux-reset GPIO cannot reset the
                // downstream SW3538. Preserve the prior presence state rather
                // than guessing which register bank the device is in.
                recover_mux = true;
                preserve_presence = true;
                *was_present
            }
            Err(_) => {
                recover_mux = true;
                false
            }
        };
        if mux.deselect_all(bus).await.is_err() {
            recover_mux = true;
            if preserve_presence {
                *was_present
            } else {
                false
            }
        } else {
            detected
        }
    } else {
        recover_mux = true;
        false
    };

    if detected != *was_present || (report_initial_result && !detected) {
        *was_present = detected;
        CONTROLLER_EVENTS
            .send(if detected {
                ControllerEvent::ModuleDetected(port)
            } else {
                ControllerEvent::ModuleRemoved(port)
            })
            .await;
    }
    if recover_mux {
        reset_mux_if_available(mux_reset).await;
    }
}

async fn reset_mux_if_available(reset: &mut Option<Output<'static>>) {
    // Rev B intentionally passes None: reset is backplane-local and absent
    // from the backpack boundary. Legacy Rev A retains its PA3 pulse path.
    if let Some(reset) = reset {
        reset.set_low();
        Timer::after_millis(MUX_RESET_PULSE_MS).await;
        reset.set_high();
        Timer::after_millis(MUX_RESET_PULSE_MS).await;
    }
}
