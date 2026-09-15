use std::{env, process::ExitCode};

#[cfg(test)]
use std::fmt::Write as _;

use pdcan_sim::{SimulatedNode, SimulatedProfile};
use pdcan_types::{CarrierProfile, NodeId, NodeUid, UpdateImpact};

const DEFAULT_UID: NodeUid = NodeUid::from_bytes([
    0x50, 0x44, 0x43, 0x41, 0x4e, 0x2d, 0x53, 0x49, 0x4d, 0x2d, 0x30, 0x31,
]);

const HELP: &str = "\
PDCAN Generation-2 deterministic node simulator

Usage:
  pdcan-sim [--role backplane|carrier] [--profile sw3538|basic|accessory|240w]
            [--update-impact live|interrupt] [--uid 24HEX] [--node ID|none]
            [--interface vcan0] [--max-requests N]

The default is the finalized SW3538 carrier profile, whose update impact is
interrupt. --profile applies only to carriers. SocketCAN transport is available
on Linux; without --interface the command prints the selected node model.
";

#[derive(Debug, Eq, PartialEq)]
struct Options {
    profile: SimulatedProfile,
    interface: Option<String>,
    uid: NodeUid,
    node_id: Option<NodeId>,
    max_requests: Option<usize>,
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pdcan-sim: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let arguments: Vec<_> = arguments.collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print!("{HELP}");
        return Ok(());
    }
    let options = parse_options(arguments.into_iter())?;
    let node = SimulatedNode::new(options.profile, options.uid, options.node_id);
    let descriptor = node.descriptor();
    let Some(interface) = options.interface else {
        println!(
            "PDCAN simulator: role={:?} profile={:?} update_impact={:?} capabilities=0x{:016x}",
            descriptor.role,
            descriptor.carrier_profile,
            descriptor.update_impact,
            descriptor.capabilities.bits()
        );
        return Ok(());
    };
    run_transport(&interface, node, options.max_requests)
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let arguments: Vec<_> = arguments.collect();
    let mut role = "carrier";
    let mut carrier_profile = CarrierProfile::Sw3538;
    let mut impact = UpdateImpact::Interrupt;
    let mut interface = None;
    let mut uid = DEFAULT_UID;
    let mut node_id = Some(NodeId::new(3).expect("default node is valid"));
    let mut max_requests = None;
    let mut index = 0;

    while index < arguments.len() {
        let option = arguments[index].as_str();
        index += 1;
        match option {
            "--role" => {
                role = take_value(&arguments, &mut index, option)?;
                if !matches!(role, "backplane" | "carrier") {
                    return Err("--role must be backplane or carrier".into());
                }
            }
            "--profile" => {
                carrier_profile = parse_profile(take_value(&arguments, &mut index, option)?)?;
            }
            "--update-impact" => {
                impact = match take_value(&arguments, &mut index, option)? {
                    "live" => UpdateImpact::Live,
                    "interrupt" => UpdateImpact::Interrupt,
                    _ => return Err("--update-impact must be live or interrupt".into()),
                };
            }
            "--interface" | "-i" => {
                interface = Some(take_value(&arguments, &mut index, option)?.to_owned());
            }
            "--uid" => uid = parse_uid(take_value(&arguments, &mut index, option)?)?,
            "--node" => {
                let value = take_value(&arguments, &mut index, option)?;
                node_id = if value == "none" {
                    None
                } else {
                    Some(parse_node(value)?)
                };
            }
            "--max-requests" => {
                let value = take_value(&arguments, &mut index, option)?;
                let maximum = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid request count {value:?}"))?;
                if maximum == 0 {
                    return Err("--max-requests must be greater than zero".into());
                }
                max_requests = Some(maximum);
            }
            unknown => return Err(format!("unknown option {unknown:?}\n\n{HELP}")),
        }
    }

    let profile = if role == "backplane" {
        SimulatedProfile::Backplane
    } else {
        SimulatedProfile::Carrier {
            profile: carrier_profile,
            update_impact: impact,
        }
    };
    Ok(Options {
        profile,
        interface,
        uid,
        node_id,
        max_requests,
    })
}

fn parse_profile(value: &str) -> Result<CarrierProfile, String> {
    match value {
        "sw3538" => Ok(CarrierProfile::Sw3538),
        "basic" => Ok(CarrierProfile::Basic),
        "accessory" => Ok(CarrierProfile::Accessory),
        "240w" => Ok(CarrierProfile::HighPower240W),
        _ => Err("--profile must be sw3538, basic, accessory, or 240w".into()),
    }
}

fn parse_node(value: &str) -> Result<NodeId, String> {
    let raw = value
        .parse::<u8>()
        .map_err(|_| format!("invalid Node ID {value:?}"))?;
    NodeId::new(raw).map_err(|_| format!("Node ID {raw} is outside 1..=254"))
}

fn take_value<'a>(
    arguments: &'a [String],
    index: &mut usize,
    option: &str,
) -> Result<&'a str, String> {
    let value = arguments
        .get(*index)
        .ok_or_else(|| format!("{option} requires a value"))?;
    *index += 1;
    Ok(value)
}

fn parse_uid(value: &str) -> Result<NodeUid, String> {
    if value.len() != NodeUid::LENGTH * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("UID must be exactly 24 hexadecimal digits".into());
    }
    let mut bytes = [0; NodeUid::LENGTH];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "UID must be exactly 24 hexadecimal digits".to_owned())?;
    }
    Ok(NodeUid::from_bytes(bytes))
}

#[cfg(target_os = "linux")]
fn run_transport(
    interface: &str,
    mut node: SimulatedNode,
    max_requests: Option<usize>,
) -> Result<(), String> {
    use pdcan_protocol::ExtendedId;
    use socketcan::{CanAnyFrame, CanFdFrame, CanFdSocket, EmbeddedFrame as _, Frame as _, Socket};

    let socket = CanFdSocket::open(interface)
        .map_err(|error| format!("cannot open {interface}: {error}"))?;
    let mut received = 0usize;
    loop {
        let frame = socket
            .read_frame()
            .map_err(|error| format!("read from {interface} failed: {error}"))?;
        let CanAnyFrame::Fd(frame) = frame else {
            continue;
        };
        if !frame.is_brs() || !frame.is_extended() {
            continue;
        }
        let Ok(id) = ExtendedId::new(frame.raw_id()) else {
            continue;
        };
        for response in node.handle_frame(id, frame.data()) {
            let can_id = socketcan::ExtendedId::new(response.id().get())
                .ok_or_else(|| "protocol emitted an invalid extended ID".to_owned())?;
            let mut response_frame = CanFdFrame::new(can_id, response.payload())
                .ok_or_else(|| "protocol emitted an invalid CAN-FD payload".to_owned())?;
            response_frame.set_brs(true);
            socket
                .write_frame(&response_frame)
                .map_err(|error| format!("write to {interface} failed: {error}"))?;
        }
        received += 1;
        if max_requests.is_some_and(|maximum| received >= maximum) {
            return Ok(());
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn run_transport(
    interface: &str,
    _node: SimulatedNode,
    _max_requests: Option<usize>,
) -> Result<(), String> {
    Err(format!(
        "SocketCAN interface {interface:?} is only available on Linux"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> std::vec::IntoIter<String> {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn default_is_the_current_interrupt_carrier() {
        let options = parse_options(std::iter::empty()).unwrap();
        assert_eq!(
            options.profile,
            SimulatedProfile::Carrier {
                profile: CarrierProfile::Sw3538,
                update_impact: UpdateImpact::Interrupt,
            }
        );
    }

    #[test]
    fn mixed_future_profiles_and_live_capability_are_explicit() {
        let options = parse_options(strings(&[
            "--profile",
            "accessory",
            "--update-impact",
            "live",
            "--node",
            "9",
        ]))
        .unwrap();
        assert_eq!(
            options.profile,
            SimulatedProfile::Carrier {
                profile: CarrierProfile::Accessory,
                update_impact: UpdateImpact::Live,
            }
        );
        assert_eq!(options.node_id, Some(NodeId::new(9).unwrap()));
    }

    #[test]
    fn uid_formatting_stays_stable() {
        let uid = parse_uid("504443414e2d53494d2d3031").unwrap();
        let mut formatted = String::new();
        for byte in uid.as_bytes() {
            write!(&mut formatted, "{byte:02x}").unwrap();
        }
        assert_eq!(formatted, "504443414e2d53494d2d3031");
    }
}
