#[cfg(not(target_os = "linux"))]
compile_error!("pdcan currently supports Linux/SocketCAN only");

use std::{env, fmt::Write as _, process::ExitCode};

use pdcan_protocol::{ExtendedId, Header, MessageClass};
use pdcan_types::RequesterId;
use socketcan::{CanFdSocket, EmbeddedFrame as _, Frame, Socket};

const HELP: &str = "\
PDCAN host utility (draft protocol)

Usage:
  pdcan monitor [--interface can0] [--requests] [--requester ID] [--json] [--count N]
  pdcan decode-id ID
  pdcan --version

monitor displays every PDCAN requester by default. --requests restricts output to
control requests; --requester filters by the 4-bit RequesterId.
";

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pdcan: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let arguments: Vec<_> = arguments.collect();
    match arguments.first().map(String::as_str) {
        None | Some("--help" | "-h") => {
            print!("{HELP}");
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("pdcan {} (protocol draft)", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("decode-id") => decode_id_command(&arguments[1..]),
        Some("monitor") => monitor_command(&arguments[1..]),
        Some(command) => Err(format!("unknown command {command:?}\n\n{HELP}")),
    }
}

fn decode_id_command(arguments: &[String]) -> Result<(), String> {
    let [raw] = arguments else {
        return Err("decode-id requires one hexadecimal or decimal ID".into());
    };
    let raw = parse_u32(raw)?;
    let id = ExtendedId::new(raw).map_err(|_| format!("ID {raw:#x} is not 29-bit"))?;
    let header = Header::decode(id).map_err(|error| format!("invalid PDCAN ID: {error:?}"))?;
    print_header(id, header, &[], false);
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct MonitorOptions {
    interface: String,
    requests_only: bool,
    requester: Option<RequesterId>,
    json: bool,
    count: Option<usize>,
}

fn monitor_command(arguments: &[String]) -> Result<(), String> {
    let options = parse_monitor_options(arguments)?;
    let socket = CanFdSocket::open(&options.interface)
        .map_err(|error| format!("cannot open {}: {error}", options.interface))?;

    let mut displayed = 0usize;
    loop {
        let frame = socket
            .read_frame()
            .map_err(|error| format!("read from {} failed: {error}", options.interface))?;
        if !frame.is_extended() {
            continue;
        }
        let Ok(id) = ExtendedId::new(frame.raw_id()) else {
            continue;
        };
        let Ok(header) = Header::decode(id) else {
            continue;
        };
        if options.requests_only && header.class != MessageClass::Control {
            continue;
        }
        if options
            .requester
            .is_some_and(|filter| filter != header.requester)
        {
            continue;
        }
        print_header(id, header, frame.data(), options.json);
        displayed += 1;
        if options.count.is_some_and(|count| displayed >= count) {
            return Ok(());
        }
    }
}

fn parse_monitor_options(arguments: &[String]) -> Result<MonitorOptions, String> {
    let mut options = MonitorOptions {
        interface: "can0".into(),
        requests_only: false,
        requester: None,
        json: false,
        count: None,
    };

    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--interface" | "-i" => {
                index += 1;
                options.interface.clone_from(
                    arguments
                        .get(index)
                        .ok_or("--interface requires a Linux network-interface name")?,
                );
            }
            "--requester" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or("--requester requires an ID from 0 through 15")?;
                let value = value
                    .parse::<u8>()
                    .map_err(|_| format!("invalid RequesterId {value:?}"))?;
                options.requester = Some(
                    RequesterId::new(value)
                        .map_err(|_| format!("RequesterId {value} is outside 0..=15"))?,
                );
            }
            "--requests" => options.requests_only = true,
            "--json" => options.json = true,
            "--count" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or("--count requires a positive frame count")?;
                let count = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid frame count {value:?}"))?;
                if count == 0 {
                    return Err("--count must be greater than zero".into());
                }
                options.count = Some(count);
            }
            argument => return Err(format!("unknown monitor option {argument:?}")),
        }
        index += 1;
    }
    Ok(options)
}

fn parse_u32(value: &str) -> Result<u32, String> {
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(|| value.parse::<u32>(), |hex| u32::from_str_radix(hex, 16));
    parsed.map_err(|_| format!("invalid integer {value:?}"))
}

fn print_header(id: ExtendedId, header: Header, data: &[u8], json: bool) {
    let mut payload = String::with_capacity(data.len() * 2);
    for byte in data {
        write!(&mut payload, "{byte:02x}").expect("writing to a String cannot fail");
    }

    if json {
        println!(
            "{{\"id\":\"0x{:08x}\",\"priority\":{},\"class\":\"{:?}\",\"node\":{},\"target\":{},\"requester\":{},\"opcode\":{},\"data\":\"{}\"}}",
            id.get(),
            header.priority,
            header.class,
            header.node,
            header.target,
            header.requester.get(),
            header.opcode,
            payload
        );
    } else {
        println!(
            "0x{:08x} priority={} class={:?} node={} target={} requester={} opcode={} data={}",
            id.get(),
            header.priority,
            header.class,
            header.node,
            header.target,
            header.requester.get(),
            header.opcode,
            payload
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn monitor_defaults_to_all_requesters_on_can0() {
        let options = parse_monitor_options(&[]).unwrap();
        assert_eq!(
            options,
            MonitorOptions {
                interface: "can0".into(),
                requests_only: false,
                requester: None,
                json: false,
                count: None,
            }
        );
    }

    #[test]
    fn monitor_can_filter_without_changing_default_visibility() {
        let options = parse_monitor_options(&strings(&[
            "--interface",
            "vcan0",
            "--requests",
            "--requester",
            "3",
            "--json",
            "--count",
            "1",
        ]))
        .unwrap();
        assert_eq!(options.interface, "vcan0");
        assert!(options.requests_only);
        assert_eq!(options.requester, Some(RequesterId::new(3).unwrap()));
        assert!(options.json);
        assert_eq!(options.count, Some(1));
    }
}
