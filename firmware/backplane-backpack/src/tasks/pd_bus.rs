use backplane_backpack_firmware::BOARD;
use backplane_backpack_firmware::action_executor::{ControllerCompletion, PdBusCommand};
use embassy_futures::select::{Either3, select3};
use embassy_stm32::gpio::Output;
use embassy_stm32::i2c::{Error as I2cError, I2c, Master};
use embassy_stm32::mode::Async;
use embassy_time::{Duration, Instant, Timer};
use pdcan_core::{CompletionOutcome, PdActionKind};
use pdcan_drivers::sw3538::{DEFAULT_ADDRESS, Error as Sw3538Error, FixedPdoPlan, Sw3538};
use pdcan_drivers::tca9548a::Tca9548a;
use pdcan_types::{MAX_PORTS, PortId, PortPolicy};

use crate::channels::{
    CONTROLLER_EVENTS, ControllerEvent, PD_BUS_PROGRESS, PD_EMERGENCY_COMMANDS, PD_POLICY_COMMANDS,
    advance,
};

const PROBE_INTERVAL_MS_PROVISIONAL: u64 = 250;
const MUX_RESET_PULSE_MS: u64 = 1;
const FAILURE_RETRY_DELAY_MS_PROVISIONAL: u64 = 100;

#[embassy_executor::task]
pub async fn pd_bus_task(mut bus: I2c<'static, Async, Master>, mut mux_reset: Output<'static>) {
    let mux = Tca9548a::default_address();
    let mut devices: [Sw3538; MAX_PORTS] =
        core::array::from_fn(|_| Sw3538::assuming_base_bank(DEFAULT_ADDRESS));
    let mut present = [false; MAX_PORTS];
    let mut next_probe = 0_u8;
    let mut probe_deadline = Instant::now() + Duration::from_millis(PROBE_INTERVAL_MS_PROVISIONAL);

    if mux.deselect_all(&mut bus).await.is_err() {
        reset_mux(&mut mux_reset).await;
    }
    advance(&PD_BUS_PROGRESS);

    loop {
        if let Ok(command) = PD_EMERGENCY_COMMANDS.try_receive() {
            process_command(&mut bus, mux, &mut mux_reset, &mut devices, command).await;
            advance(&PD_BUS_PROGRESS);
            continue;
        }
        if Instant::now() >= probe_deadline {
            probe_deadline = next_probe_deadline();
            probe_next_port(
                &mut bus,
                mux,
                &mut mux_reset,
                &mut devices,
                &mut present,
                &mut next_probe,
            )
            .await;
            advance(&PD_BUS_PROGRESS);
            continue;
        }

        match select3(
            PD_EMERGENCY_COMMANDS.receive(),
            PD_POLICY_COMMANDS.receive(),
            Timer::at(probe_deadline),
        )
        .await
        {
            Either3::First(command) | Either3::Second(command) => {
                process_command(&mut bus, mux, &mut mux_reset, &mut devices, command).await;
            }
            Either3::Third(()) => {
                probe_deadline = next_probe_deadline();
                probe_next_port(
                    &mut bus,
                    mux,
                    &mut mux_reset,
                    &mut devices,
                    &mut present,
                    &mut next_probe,
                )
                .await;
            }
        }
        advance(&PD_BUS_PROGRESS);
    }
}

async fn process_command(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Output<'static>,
    devices: &mut [Sw3538; MAX_PORTS],
    command: PdBusCommand,
) {
    let succeeded = execute_command(bus, mux, devices, command).await;
    if !succeeded {
        reset_mux(mux_reset).await;
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

async fn probe_next_port(
    bus: &mut I2c<'static, Async, Master>,
    mux: Tca9548a,
    mux_reset: &mut Output<'static>,
    devices: &mut [Sw3538; MAX_PORTS],
    present: &mut [bool; MAX_PORTS],
    next_probe: &mut u8,
) {
    let port = PortId::new(*next_probe).expect("probe index is always a logical port");
    *next_probe = if *next_probe == PortId::MAX {
        PortId::MIN
    } else {
        *next_probe + 1
    };
    if BOARD.supports(port) {
        probe_port(
            bus,
            mux,
            mux_reset,
            &mut devices[port.index()],
            &mut present[port.index()],
            port,
        )
        .await;
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
    mux_reset: &mut Output<'static>,
    device: &mut Sw3538,
    was_present: &mut bool,
    port: PortId,
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
            // wedged and take the explicit recovery path below.
            Err(Sw3538Error::Bus(I2cError::Nack)) => false,
            Err(Sw3538Error::BankStateUnknown) => {
                // A mux reset cannot reset the downstream SW3538. Preserve the
                // prior presence state rather than guessing which register bank
                // the device is in; the recovery sequence is a HIL blocker.
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

    if detected != *was_present {
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
        reset_mux(mux_reset).await;
    }
}

async fn reset_mux(reset: &mut Output<'static>) {
    reset.set_low();
    Timer::after_millis(MUX_RESET_PULSE_MS).await;
    reset.set_high();
    Timer::after_millis(MUX_RESET_PULSE_MS).await;
}
