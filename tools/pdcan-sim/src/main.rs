#[cfg(not(target_os = "linux"))]
compile_error!("pdcan-sim currently supports Linux/SocketCAN only");

use std::{env, process::ExitCode};

use pdcan_protocol::ExtendedId;
use pdcan_sim::SimulatedNode;
use pdcan_types::{MAX_PORTS, NodeId, NodeUid};
use socketcan::{CanAnyFrame, CanFdFrame, CanFdSocket, EmbeddedFrame as _, Frame as _, Socket};

const DEFAULT_UID: NodeUid = NodeUid::from_bytes([
    0x50, 0x44, 0x43, 0x41, 0x4e, 0x2d, 0x53, 0x49, 0x4d, 0x2d, 0x30, 0x31,
]);

const HELP: &str = "\
PDCAN deterministic node simulator

Usage:
  pdcan-sim [--ports 6|8]
  pdcan-sim --interface vcan0 [--ports 6|8] [--uid 24HEX] [--node ID|none]
            [--max-requests N]

Without --interface, prints the selected capability model and exits. With an
interface, runs a CAN-FD peer until terminated or --max-requests is reached.
";

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ports: u8,
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
    let mut node = SimulatedNode::new(options.ports, options.uid, options.node_id)?;
    let supported = node.board().supported_ports.bits();
    let Some(interface) = options.interface else {
        println!("PDCAN simulator: logical_capacity={MAX_PORTS} supported_mask=0x{supported:02x}");
        return Ok(());
    };

    let socket = CanFdSocket::open(&interface)
        .map_err(|error| format!("cannot open {interface}: {error}"))?;
    let mut received = 0usize;
    loop {
        let frame = socket
            .read_frame()
            .map_err(|error| format!("read from {interface} failed: {error}"))?;
        let CanAnyFrame::Fd(frame) = frame else {
            continue;
        };
        if !frame.is_brs() {
            continue;
        }
        if !frame.is_extended() {
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
        if options
            .max_requests
            .is_some_and(|maximum| received >= maximum)
        {
            return Ok(());
        }
    }
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let arguments: Vec<_> = arguments.collect();
    let mut options = Options {
        ports: 8,
        interface: None,
        uid: DEFAULT_UID,
        node_id: Some(NodeId::new(3).expect("default node is valid")),
        max_requests: None,
    };
    let mut index = 0;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        index += 1;
        match option {
            "--ports" => {
                options.ports = take_value(&arguments, &mut index, option)?
                    .parse::<u8>()
                    .map_err(|_| "--ports must be 6 or 8".to_owned())?;
                if options.ports != 6 && options.ports != 8 {
                    return Err("--ports must be 6 or 8".into());
                }
            }
            "--interface" | "-i" => {
                options.interface = Some(take_value(&arguments, &mut index, option)?.to_owned());
            }
            "--uid" => {
                options.uid = parse_uid(take_value(&arguments, &mut index, option)?)?;
            }
            "--node" => {
                let value = take_value(&arguments, &mut index, option)?;
                options.node_id = if value == "none" {
                    None
                } else {
                    let raw = value
                        .parse::<u8>()
                        .map_err(|_| format!("invalid Node ID {value:?}"))?;
                    Some(
                        NodeId::new(raw)
                            .map_err(|_| format!("Node ID {raw} is outside 1..=254"))?,
                    )
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
                options.max_requests = Some(maximum);
            }
            unknown => return Err(format!("unknown option {unknown:?}\n\n{HELP}")),
        }
    }
    Ok(options)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn strings<'a>(values: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        values.iter().map(|value| (*value).to_owned())
    }

    #[test]
    fn server_options_accept_uncommissioned_six_port_node() {
        let options = parse_options(strings(&[
            "--interface",
            "vcan0",
            "--ports",
            "6",
            "--uid",
            "000102030405060708090a0b",
            "--node",
            "none",
            "--max-requests",
            "4",
        ]))
        .unwrap();
        assert_eq!(options.ports, 6);
        assert_eq!(options.interface.as_deref(), Some("vcan0"));
        assert_eq!(options.node_id, None);
        assert_eq!(
            options.uid.to_bytes(),
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
        );
        assert_eq!(options.max_requests, Some(4));
    }

    #[test]
    fn invalid_uids_and_node_ids_are_rejected() {
        assert!(parse_uid("1234").is_err());
        assert!(parse_options(strings(&["--node", "0"])).is_err());
        assert!(parse_options(strings(&["--ports", "7"])).is_err());
    }
}
