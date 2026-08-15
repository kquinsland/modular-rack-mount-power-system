use std::{env, process::ExitCode};

use pdcan_core::{Action, Controller};
use pdcan_types::{
    BoardDefinition, FanMode, HardwareRevision, MAX_PORTS, PersistentSettings, PortBitmap, PortId,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pdcan-sim: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let port_count = match (arguments.next().as_deref(), arguments.next()) {
        (None, None) => 8,
        (Some("--ports"), Some(value)) => value
            .parse::<u8>()
            .map_err(|_| format!("invalid port count {value:?}"))?,
        _ => return Err("usage: pdcan-sim [--ports 6|8]".into()),
    };
    if port_count != 6 && port_count != 8 {
        return Err("port count must be 6 or 8".into());
    }

    let supported_bits = if port_count == 8 { 0xFF } else { 0x3F };
    let board = BoardDefinition {
        hardware_revision: HardwareRevision::Simulator,
        supported_ports: PortBitmap::from_bits(supported_bits),
        mux_channel_by_port: [
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            (port_count == 8).then_some(6),
            (port_count == 8).then_some(7),
        ],
        default_fan_mode: FanMode::ThreeWire,
    };
    let mut controller = Controller::new(board, PersistentSettings::FACTORY_DEFAULT);
    for raw_port in 0..port_count {
        controller
            .module_detected(PortId::new(raw_port).expect("validated simulator port"))
            .map_err(|error| format!("cannot add simulated port: {error:?}"))?;
    }

    let mut scheduled = 0;
    while let Some(Action::Pd { .. }) = controller.next_action() {
        scheduled += 1;
    }
    println!(
        "PDCAN simulator: logical_capacity={MAX_PORTS} supported_mask=0x{supported_bits:02x} initial_actions={scheduled}"
    );
    Ok(())
}
