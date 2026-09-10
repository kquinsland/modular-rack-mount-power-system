#[cfg(not(target_os = "linux"))]
compile_error!("pdcan currently supports Linux/SocketCAN only");

use std::{
    collections::BTreeMap,
    env,
    fmt::Write as _,
    io::ErrorKind,
    process::ExitCode,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use pdcan_protocol::{
    BoardTemperature, CommandResponse, CommandResult, CommissioningHeader, CommissioningMessage,
    ControlCommand, ControlRequest, DecodedHeader, DiscoveryInfo, ExtendedId, Header, MessageClass,
    PROTOCOL_MAJOR, PROTOCOL_MINOR, PortState, WireFrame, commissioning_opcode,
    decode_board_temperature, decode_command_response, decode_commissioning, decode_header,
    decode_port_state, encode_commissioning, encode_control_request, telemetry_opcode,
};
use pdcan_types::{
    FanConfig, FanMode, NodeId, NodeUid, PortId, PortPolicy, RequestId, RequesterId,
};
use socketcan::{CanAnyFrame, CanFdFrame, CanFdSocket, EmbeddedFrame as _, Frame as _, Socket};

const DEFAULT_TIMEOUT_MS: u64 = 250;
const DEFAULT_RETRIES: u8 = 2;
const DEFAULT_SCAN_ROUNDS: u8 = 3;

const HELP: &str = "\
PDCAN host utility (pre-v1 draft protocol)

Usage:
  pdcan monitor [--interface can0] [--requests] [--requester ID] [--json] [--count N]
  pdcan decode-id ID
  pdcan scan [--interface can0] [--requester ID] [--rounds N] [--timeout-ms MS] [--json]
  pdcan info NODE [scan options]
  pdcan identify UID [--seconds N] [request options]
  pdcan assign UID NODE [--force] [request options]
  pdcan clear-node UID [request options]
  pdcan status NODE.PORT [request options]
  pdcan set-policy NODE.PORT (--enabled|--disabled) [--max-voltage-mv MV]
                   [--max-current-ma MA] [--max-power-mw MW] [request options]
  pdcan set-fan NODE --mode 3-wire|4-wire --duty PERCENT [request options]
  pdcan emergency-disable NODE|all [request options]
  pdcan acknowledge-resolved NODE [request options]
  pdcan --version

Request options:
  --interface, -i IFACE       Linux SocketCAN interface (default can0)
  --requester ID              1..15; 15 is the pdcan development default
  --request-id ID             explicit 64-bit decimal or hexadecimal ID
  --timeout-ms MS             per-attempt timeout (default 250)
  --retries N                 retransmissions using the same RequestId (default 2)
  --json                      stable machine-readable output

monitor displays every requester by default. --requests restricts output to
control/commissioning requests; --requester only filters display.
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
            println!("pdcan {} (protocol 0.1 draft)", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("decode-id") => decode_id_command(&arguments[1..]),
        Some("monitor") => monitor_command(&arguments[1..]),
        Some("scan") => scan_command(&arguments[1..]),
        Some("info") => info_command(&arguments[1..]),
        Some("identify") => identify_command(&arguments[1..]),
        Some("assign") => assign_command(&arguments[1..]),
        Some("clear-node") => clear_node_command(&arguments[1..]),
        Some("status") => status_command(&arguments[1..]),
        Some("set-policy") => set_policy_command(&arguments[1..]),
        Some("set-fan") => set_fan_command(&arguments[1..]),
        Some("emergency-disable") => emergency_command(&arguments[1..]),
        Some("acknowledge-resolved") => acknowledge_command(&arguments[1..]),
        Some(command) => Err(format!("unknown command {command:?}\n\n{HELP}")),
    }
}

fn decode_id_command(arguments: &[String]) -> Result<(), String> {
    let [raw] = arguments else {
        return Err("decode-id requires one hexadecimal or decimal ID".into());
    };
    let raw = parse_u32(raw)?;
    let id = ExtendedId::new(raw).map_err(|_| format!("ID {raw:#x} is not 29-bit"))?;
    let header = decode_header(id).map_err(|error| format!("invalid PDCAN ID: {error:?}"))?;
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
        let Ok(header) = decode_header(id) else {
            continue;
        };
        if options.requests_only && !is_request(header) {
            continue;
        }
        if options
            .requester
            .is_some_and(|filter| filter != header_requester(header))
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
                options.requester = Some(parse_requester(
                    arguments
                        .get(index)
                        .ok_or("--requester requires an ID from 0 through 15")?,
                    true,
                )?);
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct RequestOptions {
    interface: String,
    requester: RequesterId,
    request_id: RequestId,
    timeout: Duration,
    retries: u8,
    json: bool,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            interface: "can0".into(),
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: generated_request_id(),
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            retries: DEFAULT_RETRIES,
            json: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScanOptions {
    interface: String,
    requester: RequesterId,
    rounds: u8,
    timeout: Duration,
    json: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            interface: "can0".into(),
            requester: RequesterId::PDCAN_DEFAULT,
            rounds: DEFAULT_SCAN_ROUNDS,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            json: false,
        }
    }
}

fn scan_command(arguments: &[String]) -> Result<(), String> {
    let options = parse_scan_options(arguments)?;
    let found = discover(&options)?;
    print_discovery(&found, options.json);
    Ok(())
}

fn info_command(arguments: &[String]) -> Result<(), String> {
    let Some(raw_node) = arguments.first() else {
        return Err("info requires a Node ID".into());
    };
    let node = parse_node_id(raw_node)?;
    let options = parse_scan_options(&arguments[1..])?;
    let matching: BTreeMap<_, _> = discover(&options)?
        .into_iter()
        .filter(|(_, info)| info.node_id == Some(node))
        .collect();
    if matching.is_empty() {
        return Err(format!(
            "Node ID {} did not respond to discovery",
            node.get()
        ));
    }
    print_discovery(&matching, options.json);
    Ok(())
}

fn parse_scan_options(arguments: &[String]) -> Result<ScanOptions, String> {
    let mut options = ScanOptions::default();
    let mut index = 0;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        index += 1;
        match option {
            "--interface" | "-i" => {
                take_value(arguments, &mut index, option)?.clone_into(&mut options.interface);
            }
            "--requester" => {
                options.requester =
                    parse_requester(take_value(arguments, &mut index, option)?, false)?;
            }
            "--rounds" => {
                options.rounds = take_value(arguments, &mut index, option)?
                    .parse::<u8>()
                    .map_err(|_| "--rounds requires an integer from 1 through 16".to_owned())?;
                if options.rounds == 0 || options.rounds > 16 {
                    return Err("--rounds must be from 1 through 16".into());
                }
            }
            "--timeout-ms" => {
                options.timeout = parse_timeout(take_value(arguments, &mut index, option)?)?;
            }
            "--json" => options.json = true,
            unknown => return Err(format!("unknown scan option {unknown:?}")),
        }
    }
    Ok(options)
}

fn discover(options: &ScanOptions) -> Result<BTreeMap<NodeUid, DiscoveryInfo>, String> {
    let socket = CanFdSocket::open(&options.interface)
        .map_err(|error| format!("cannot open {}: {error}", options.interface))?;
    let base_nonce = generated_request_id().0;
    let mut found = BTreeMap::new();
    for round in 0..options.rounds {
        let nonce = base_nonce.wrapping_add(u64::from(round));
        let request =
            encode_commissioning(options.requester, CommissioningMessage::Discover { nonce })
                .map_err(|error| format!("cannot encode discovery request: {error:?}"))?;
        write_wire_frame(&socket, request, &options.interface)?;
        let deadline = Instant::now() + options.timeout;
        while let Some(frame) = read_until(&socket, deadline, &options.interface)? {
            if !frame.is_extended() {
                continue;
            }
            let Ok(id) = ExtendedId::new(frame.raw_id()) else {
                continue;
            };
            let Ok((
                header,
                CommissioningMessage::DiscoveryResponse {
                    nonce: echoed,
                    info,
                },
            )) = decode_commissioning(id, frame.data())
            else {
                continue;
            };
            if header.requester == options.requester && echoed == nonce {
                found.insert(info.uid, info);
            }
        }
    }
    Ok(found)
}

fn identify_command(arguments: &[String]) -> Result<(), String> {
    let uid = parse_required_uid(arguments, "identify")?;
    let mut duration_seconds = 30u16;
    let (options, extras) = parse_request_options(&arguments[1..], &["--seconds"])?;
    let mut index = 0;
    while index < extras.len() {
        if extras[index].0 == "--seconds" {
            duration_seconds = extras[index]
                .1
                .parse::<u16>()
                .map_err(|_| "--seconds requires an integer from 0 through 65535".to_owned())?;
        }
        index += 1;
    }
    execute_commissioning(
        &options,
        uid,
        commissioning_opcode::IDENTIFY,
        CommissioningMessage::Identify {
            request_id: options.request_id,
            uid,
            duration_seconds,
        },
    )
}

fn assign_command(arguments: &[String]) -> Result<(), String> {
    if arguments.len() < 2 {
        return Err("assign requires UID and Node ID".into());
    }
    let uid = parse_uid(&arguments[0])?;
    let node_id = parse_node_id(&arguments[1])?;
    let (options, extras) = parse_request_options(&arguments[2..], &["--force"])?;
    let force = extras.iter().any(|(name, _)| name == "--force");
    if !force {
        let scan_options = ScanOptions {
            interface: options.interface.clone(),
            requester: options.requester,
            rounds: DEFAULT_SCAN_ROUNDS,
            timeout: options.timeout,
            json: false,
        };
        if let Some(occupant) = discover(&scan_options)?
            .values()
            .find(|info| info.node_id == Some(node_id) && info.uid != uid)
        {
            return Err(format!(
                "Node ID {} is already reported by {}; pass --force only after resolving why",
                node_id.get(),
                format_uid(occupant.uid)
            ));
        }
    }
    execute_commissioning(
        &options,
        uid,
        commissioning_opcode::ASSIGN_NODE,
        CommissioningMessage::AssignNode {
            request_id: options.request_id,
            uid,
            node_id,
        },
    )
}

fn clear_node_command(arguments: &[String]) -> Result<(), String> {
    let uid = parse_required_uid(arguments, "clear-node")?;
    let (options, _) = parse_request_options(&arguments[1..], &[])?;
    execute_commissioning(
        &options,
        uid,
        commissioning_opcode::CLEAR_NODE,
        CommissioningMessage::ClearNode {
            request_id: options.request_id,
            uid,
        },
    )
}

fn execute_commissioning(
    options: &RequestOptions,
    uid: NodeUid,
    request_opcode: u8,
    message: CommissioningMessage,
) -> Result<(), String> {
    let frame = encode_commissioning(options.requester, message)
        .map_err(|error| format!("cannot encode commissioning request: {error:?}"))?;
    let socket = CanFdSocket::open(&options.interface)
        .map_err(|error| format!("cannot open {}: {error}", options.interface))?;
    for _ in 0..=options.retries {
        write_wire_frame(&socket, frame, &options.interface)?;
        let deadline = Instant::now() + options.timeout;
        while let Some(received) = read_until(&socket, deadline, &options.interface)? {
            if !received.is_extended() {
                continue;
            }
            let Ok(id) = ExtendedId::new(received.raw_id()) else {
                continue;
            };
            let Ok((
                header,
                CommissioningMessage::Result {
                    request_id,
                    uid: response_uid,
                    request_opcode: response_opcode,
                    result,
                    node_id,
                    state,
                },
            )) = decode_commissioning(id, received.data())
            else {
                continue;
            };
            if header.requester == options.requester
                && request_id == options.request_id
                && response_uid == uid
                && response_opcode == request_opcode
            {
                print_commissioning_result(
                    uid,
                    node_id,
                    state,
                    result,
                    options.request_id,
                    options.json,
                );
                return require_success(result);
            }
        }
    }
    Err(format!(
        "timed out waiting for UID {} after {} attempts",
        format_uid(uid),
        u16::from(options.retries) + 1
    ))
}

fn status_command(arguments: &[String]) -> Result<(), String> {
    let target = parse_required_node_port(arguments, "status")?;
    let (options, _) = parse_request_options(&arguments[1..], &[])?;
    execute_status(&options, target)
}

fn execute_status(options: &RequestOptions, target: NodePort) -> Result<(), String> {
    let command = ControlCommand::RequestStatus { port: target.port };
    let frame = encode_control_request(ControlRequest {
        node: target.node.get(),
        requester: options.requester,
        request_id: options.request_id,
        command,
    })
    .map_err(|error| format!("cannot encode status request: {error:?}"))?;
    let socket = CanFdSocket::open(&options.interface)
        .map_err(|error| format!("cannot open {}: {error}", options.interface))?;
    let mut response_received = false;
    let mut state = None;
    for _ in 0..=options.retries {
        write_wire_frame(&socket, frame, &options.interface)?;
        let deadline = Instant::now() + options.timeout;
        while let Some(received) = read_until(&socket, deadline, &options.interface)? {
            if !received.is_extended() {
                continue;
            }
            let Ok(id) = ExtendedId::new(received.raw_id()) else {
                continue;
            };
            if let Ok(candidate) = decode_command_response(id, received.data())
                && candidate.node == target.node.get()
                && candidate.requester == options.requester
                && candidate.request_id == options.request_id
                && candidate.request_opcode == command.opcode()
                && candidate.request_target == command.target()
            {
                print_command_response(candidate, options.json);
                require_success(candidate.result)?;
                response_received = true;
            }
            if let Ok(candidate) = decode_port_state(id, received.data())
                && candidate.node == target.node.get()
                && candidate.port == target.port
            {
                state = Some(candidate);
            }
            if response_received && let Some(state) = state {
                print_port_state(state, options.json);
                return Ok(());
            }
        }
    }
    Err(format!(
        "timed out waiting for complete status of {}.{} after {} attempts",
        target.node.get(),
        target.port.get(),
        u16::from(options.retries) + 1
    ))
}

fn set_policy_command(arguments: &[String]) -> Result<(), String> {
    let target = parse_required_node_port(arguments, "set-policy")?;
    let (options, extras) = parse_request_options(
        &arguments[1..],
        &[
            "--enabled",
            "--disabled",
            "--max-voltage-mv",
            "--max-current-ma",
            "--max-power-mw",
        ],
    )?;
    let mut enabled = None;
    let mut policy = PortPolicy::SAFE_DISABLED;
    for (name, value) in extras {
        match name.as_str() {
            "--enabled" | "--disabled" => {
                if enabled.is_some() {
                    return Err("set-policy requires exactly one of --enabled or --disabled".into());
                }
                enabled = Some(name == "--enabled");
            }
            "--max-voltage-mv" => {
                policy.max_voltage_mv = value
                    .parse::<u16>()
                    .map_err(|_| format!("invalid voltage {value:?}"))?;
            }
            "--max-current-ma" => {
                policy.max_current_ma = value
                    .parse::<u16>()
                    .map_err(|_| format!("invalid current {value:?}"))?;
            }
            "--max-power-mw" => {
                policy.max_power_mw = value
                    .parse::<u32>()
                    .map_err(|_| format!("invalid power {value:?}"))?;
            }
            _ => unreachable!("extra options were validated"),
        }
    }
    policy.enabled = enabled.ok_or("set-policy requires exactly one of --enabled or --disabled")?;
    policy
        .validate()
        .map_err(|error| format!("invalid port policy: {error:?}"))?;
    execute_control(
        &options,
        target.node.get(),
        ControlCommand::SetPortPolicy {
            port: target.port,
            policy,
        },
    )
}

fn set_fan_command(arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() {
        return Err("set-fan requires a Node ID".into());
    }
    let node = parse_node_id(&arguments[0])?;
    let (options, extras) = parse_request_options(&arguments[1..], &["--mode", "--duty"])?;
    let mut mode = None;
    let mut duty_percent = None;
    for (name, value) in extras {
        match name.as_str() {
            "--mode" => {
                mode = Some(match value.as_str() {
                    "3-wire" => FanMode::ThreeWire,
                    "4-wire" => FanMode::FourWire,
                    _ => return Err("--mode must be 3-wire or 4-wire".into()),
                });
            }
            "--duty" => {
                duty_percent = Some(
                    value
                        .parse::<u8>()
                        .map_err(|_| format!("invalid fan duty {value:?}"))?,
                );
            }
            _ => unreachable!("extra options were validated"),
        }
    }
    let config = FanConfig {
        mode: mode.ok_or("set-fan requires --mode")?,
        duty_percent: duty_percent.ok_or("set-fan requires --duty")?,
    };
    config
        .validate()
        .map_err(|_| "fan duty must be from 0 through 100".to_owned())?;
    execute_control(
        &options,
        node.get(),
        ControlCommand::SetFanConfig { config },
    )
}

fn emergency_command(arguments: &[String]) -> Result<(), String> {
    let Some(target) = arguments.first() else {
        return Err("emergency-disable requires a Node ID or all".into());
    };
    let node = if target == "all" {
        0
    } else {
        parse_node_id(target)?.get()
    };
    let (options, _) = parse_request_options(&arguments[1..], &[])?;
    execute_control(&options, node, ControlCommand::EmergencyDisable)
}

fn acknowledge_command(arguments: &[String]) -> Result<(), String> {
    let Some(target) = arguments.first() else {
        return Err("acknowledge-resolved requires a Node ID".into());
    };
    let node = parse_node_id(target)?;
    let (options, _) = parse_request_options(&arguments[1..], &[])?;
    execute_control(
        &options,
        node.get(),
        ControlCommand::AcknowledgeEmergencyResolved,
    )
}

fn execute_control(
    options: &RequestOptions,
    node: u8,
    command: ControlCommand,
) -> Result<(), String> {
    let request = ControlRequest {
        node,
        requester: options.requester,
        request_id: options.request_id,
        command,
    };
    let frame = encode_control_request(request)
        .map_err(|error| format!("cannot encode request: {error:?}"))?;
    let socket = CanFdSocket::open(&options.interface)
        .map_err(|error| format!("cannot open {}: {error}", options.interface))?;
    for _ in 0..=options.retries {
        write_wire_frame(&socket, frame, &options.interface)?;
        let deadline = Instant::now() + options.timeout;
        while let Some(received) = read_until(&socket, deadline, &options.interface)? {
            if !received.is_extended() {
                continue;
            }
            let Ok(id) = ExtendedId::new(received.raw_id()) else {
                continue;
            };
            let Ok(response) = decode_command_response(id, received.data()) else {
                continue;
            };
            if response.requester == options.requester
                && (node == 0 || response.node == node)
                && response.request_id == options.request_id
                && response.request_opcode == command.opcode()
                && response.request_target == command.target()
            {
                print_command_response(response, options.json);
                return require_success(response.result);
            }
        }
    }
    Err(format!(
        "timed out waiting for requester {} request {} after {} attempts",
        options.requester.get(),
        options.request_id.0,
        u16::from(options.retries) + 1
    ))
}

fn parse_request_options(
    arguments: &[String],
    extra_names: &[&str],
) -> Result<(RequestOptions, Vec<(String, String)>), String> {
    let mut options = RequestOptions::default();
    let mut extras = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        index += 1;
        match option {
            "--interface" | "-i" => {
                take_value(arguments, &mut index, option)?.clone_into(&mut options.interface);
            }
            "--requester" => {
                options.requester =
                    parse_requester(take_value(arguments, &mut index, option)?, false)?;
            }
            "--request-id" => {
                options.request_id =
                    RequestId(parse_u64(take_value(arguments, &mut index, option)?)?);
            }
            "--timeout-ms" => {
                options.timeout = parse_timeout(take_value(arguments, &mut index, option)?)?;
            }
            "--retries" => {
                options.retries = take_value(arguments, &mut index, option)?
                    .parse::<u8>()
                    .map_err(|_| "--retries requires an integer from 0 through 10".to_owned())?;
                if options.retries > 10 {
                    return Err("--retries must be from 0 through 10".into());
                }
            }
            "--json" => options.json = true,
            extra if extra_names.contains(&extra) => {
                let takes_value = !matches!(extra, "--enabled" | "--disabled" | "--force");
                let value = if takes_value {
                    take_value(arguments, &mut index, extra)?.to_owned()
                } else {
                    String::new()
                };
                extras.push((extra.to_owned(), value));
            }
            unknown => return Err(format!("unknown option {unknown:?}")),
        }
    }
    Ok((options, extras))
}

fn read_until(
    socket: &CanFdSocket,
    deadline: Instant,
    interface: &str,
) -> Result<Option<CanFdFrame>, String> {
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Ok(None);
        };
        socket
            .set_read_timeout(remaining.max(Duration::from_millis(1)))
            .map_err(|error| format!("cannot set receive timeout on {interface}: {error}"))?;
        match socket.read_frame() {
            Ok(CanAnyFrame::Fd(frame)) if frame.is_brs() => return Ok(Some(frame)),
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Ok(None);
            }
            Err(error) => return Err(format!("read from {interface} failed: {error}")),
        }
    }
}

fn write_wire_frame(socket: &CanFdSocket, frame: WireFrame, interface: &str) -> Result<(), String> {
    let can_id = socketcan::ExtendedId::new(frame.id().get())
        .ok_or_else(|| "protocol emitted an invalid extended ID".to_owned())?;
    let mut frame = CanFdFrame::new(can_id, frame.payload())
        .ok_or_else(|| "protocol emitted an invalid CAN-FD payload".to_owned())?;
    frame.set_brs(true);
    socket
        .write_frame(&frame)
        .map_err(|error| format!("write to {interface} failed: {error}"))
}

fn print_discovery(found: &BTreeMap<NodeUid, DiscoveryInfo>, json: bool) {
    for info in found.values() {
        let compatible =
            info.protocol_major == PROTOCOL_MAJOR && info.protocol_minor == PROTOCOL_MINOR;
        let node = info
            .node_id
            .map_or_else(|| "null".to_owned(), |node| node.get().to_string());
        if json {
            println!(
                "{{\"kind\":\"discovery\",\"uid\":\"{}\",\"node_id\":{},\"state\":\"{:?}\",\"protocol\":\"{}.{}\",\"firmware\":\"{}.{}.{}\",\"hardware_revision\":{},\"supported_ports\":{},\"compatible\":{}}}",
                format_uid(info.uid),
                node,
                info.state,
                info.protocol_major,
                info.protocol_minor,
                info.firmware.major,
                info.firmware.minor,
                info.firmware.patch,
                info.hardware_revision,
                info.supported_ports,
                compatible,
            );
        } else {
            println!(
                "uid={} node={} state={:?} protocol={}.{} firmware={}.{}.{} hw={} ports=0x{:02x} compatible={}",
                format_uid(info.uid),
                info.node_id
                    .map_or_else(|| "none".to_owned(), |node| node.get().to_string()),
                info.state,
                info.protocol_major,
                info.protocol_minor,
                info.firmware.major,
                info.firmware.minor,
                info.firmware.patch,
                info.hardware_revision,
                info.supported_ports,
                compatible,
            );
        }
    }
}

fn print_command_response(response: CommandResponse, json: bool) {
    if json {
        println!(
            "{{\"kind\":\"command_response\",\"node\":{},\"requester\":{},\"request_id\":\"{}\",\"opcode\":{},\"target\":{},\"result\":\"{:?}\",\"detail\":{}}}",
            response.node,
            response.requester.get(),
            response.request_id.0,
            response.request_opcode,
            response.request_target,
            response.result,
            response.detail,
        );
    } else {
        println!(
            "node={} requester={} request={} result={:?} detail={}",
            response.node,
            response.requester.get(),
            response.request_id.0,
            response.result,
            response.detail,
        );
    }
}

fn print_port_state(state: PortState, json: bool) {
    if json {
        println!(
            "{{\"kind\":\"port_state\",\"node\":{},\"port\":{},\"sequence\":{},\"flags\":{},\"faults\":{},\"uptime_seconds\":{},\"active_profile\":{},\"slot_generation\":{}}}",
            state.node,
            state.port.get(),
            state.sequence,
            state.flags.bits(),
            state.faults.bits(),
            state.uptime_seconds,
            state.active_profile,
            state.slot_generation,
        );
    } else {
        println!(
            "node={}.{} sequence={} flags=0x{:04x} faults=0x{:08x} uptime={}s profile={} generation={}",
            state.node,
            state.port.get(),
            state.sequence,
            state.flags.bits(),
            state.faults.bits(),
            state.uptime_seconds,
            state.active_profile,
            state.slot_generation,
        );
    }
}

fn print_commissioning_result(
    uid: NodeUid,
    node_id: Option<NodeId>,
    state: pdcan_types::CommissioningState,
    result: CommandResult,
    request_id: RequestId,
    json: bool,
) {
    if json {
        let node = node_id.map_or_else(|| "null".to_owned(), |node| node.get().to_string());
        println!(
            "{{\"kind\":\"commissioning_result\",\"uid\":\"{}\",\"node_id\":{},\"state\":\"{:?}\",\"request_id\":\"{}\",\"result\":\"{:?}\"}}",
            format_uid(uid),
            node,
            state,
            request_id.0,
            result,
        );
    } else {
        println!(
            "uid={} node={} state={:?} request={} result={:?}",
            format_uid(uid),
            node_id.map_or_else(|| "none".to_owned(), |node| node.get().to_string()),
            state,
            request_id.0,
            result,
        );
    }
}

fn require_success(result: CommandResult) -> Result<(), String> {
    if matches!(result, CommandResult::Ok | CommandResult::OkPending) {
        Ok(())
    } else {
        Err(format!("node rejected request with {result:?}"))
    }
}

fn is_request(header: DecodedHeader) -> bool {
    match header {
        DecodedHeader::Operational(header) => header.class == MessageClass::Control,
        DecodedHeader::Commissioning(header) => matches!(
            header.opcode,
            commissioning_opcode::DISCOVER
                | commissioning_opcode::IDENTIFY
                | commissioning_opcode::ASSIGN_NODE
                | commissioning_opcode::CLEAR_NODE
        ),
    }
}

const fn header_requester(header: DecodedHeader) -> RequesterId {
    match header {
        DecodedHeader::Operational(header) => header.requester,
        DecodedHeader::Commissioning(header) => header.requester,
    }
}

fn print_header(id: ExtendedId, header: DecodedHeader, data: &[u8], json: bool) {
    if let DecodedHeader::Operational(operational) = header
        && operational.class == MessageClass::Telemetry
        && operational.opcode == telemetry_opcode::BOARD_TEMPERATURE
        && let Ok(temperature) = decode_board_temperature(id, data)
    {
        print_board_temperature(temperature, json);
        return;
    }

    let mut payload = String::with_capacity(data.len() * 2);
    for byte in data {
        write!(&mut payload, "{byte:02x}").expect("writing to a String cannot fail");
    }

    match header {
        DecodedHeader::Operational(header) => print_operational_header(id, header, &payload, json),
        DecodedHeader::Commissioning(header) => {
            print_commissioning_header(id, header, &payload, json);
        }
    }
}

fn print_board_temperature(temperature: BoardTemperature, json: bool) {
    if json {
        println!(
            "{{\"kind\":\"board_temperature\",\"node\":{},\"sequence\":{},\"temperature_centi_c\":{}}}",
            temperature.node, temperature.sequence, temperature.temperature_centi_c,
        );
    } else {
        println!(
            "node={} board_temperature={:.2} C sequence={}",
            temperature.node,
            f64::from(temperature.temperature_centi_c) / 100.0,
            temperature.sequence,
        );
    }
}

fn print_operational_header(id: ExtendedId, header: Header, payload: &str, json: bool) {
    if json {
        println!(
            "{{\"kind\":\"operational_frame\",\"id\":\"0x{:08x}\",\"priority\":{},\"class\":\"{:?}\",\"node\":{},\"target\":{},\"requester\":{},\"opcode\":{},\"data\":\"{}\"}}",
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

fn print_commissioning_header(
    id: ExtendedId,
    header: CommissioningHeader,
    payload: &str,
    json: bool,
) {
    if json {
        println!(
            "{{\"kind\":\"commissioning_frame\",\"id\":\"0x{:08x}\",\"priority\":{},\"class\":\"Commissioning\",\"token\":{},\"requester\":{},\"opcode\":{},\"data\":\"{}\"}}",
            id.get(),
            header.priority,
            header.token,
            header.requester.get(),
            header.opcode,
            payload
        );
    } else {
        println!(
            "0x{:08x} priority={} class=Commissioning token={} requester={} opcode={} data={}",
            id.get(),
            header.priority,
            header.token,
            header.requester.get(),
            header.opcode,
            payload
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodePort {
    node: NodeId,
    port: PortId,
}

fn parse_required_node_port(arguments: &[String], command: &str) -> Result<NodePort, String> {
    arguments
        .first()
        .ok_or_else(|| format!("{command} requires NODE.PORT"))
        .and_then(|value| parse_node_port(value))
}

fn parse_node_port(value: &str) -> Result<NodePort, String> {
    let (node, port) = value
        .split_once('.')
        .ok_or_else(|| format!("target {value:?} must use NODE.PORT syntax"))?;
    let node = parse_node_id(node)?;
    let raw_port = port
        .parse::<u8>()
        .map_err(|_| format!("invalid port {port:?}"))?;
    let port = PortId::new(raw_port).map_err(|_| format!("port {raw_port} is outside 0..=7"))?;
    Ok(NodePort { node, port })
}

fn parse_required_uid(arguments: &[String], command: &str) -> Result<NodeUid, String> {
    arguments
        .first()
        .ok_or_else(|| format!("{command} requires a 24-hex-digit UID"))
        .and_then(|value| parse_uid(value))
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

fn format_uid(uid: NodeUid) -> String {
    let mut output = String::with_capacity(NodeUid::LENGTH * 2);
    for byte in uid.as_bytes() {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn parse_node_id(value: &str) -> Result<NodeId, String> {
    let raw = value
        .parse::<u8>()
        .map_err(|_| format!("invalid Node ID {value:?}"))?;
    NodeId::new(raw).map_err(|_| format!("Node ID {raw} is outside 1..=254"))
}

fn parse_requester(value: &str, allow_reserved: bool) -> Result<RequesterId, String> {
    let raw = value
        .parse::<u8>()
        .map_err(|_| format!("invalid RequesterId {value:?}"))?;
    let requester =
        RequesterId::new(raw).map_err(|_| format!("RequesterId {raw} is outside 0..=15"))?;
    if requester.is_protocol_reserved() && !allow_reserved {
        return Err("RequesterId 0 is reserved for protocol-generated traffic".into());
    }
    Ok(requester)
}

fn parse_timeout(value: &str) -> Result<Duration, String> {
    let milliseconds = value
        .parse::<u64>()
        .map_err(|_| format!("invalid timeout {value:?}"))?;
    if milliseconds == 0 || milliseconds > 60_000 {
        return Err("--timeout-ms must be from 1 through 60000".into());
    }
    Ok(Duration::from_millis(milliseconds))
}

fn parse_u32(value: &str) -> Result<u32, String> {
    parse_integer(value).and_then(|parsed| {
        u32::try_from(parsed).map_err(|_| format!("integer {value:?} exceeds u32"))
    })
}

fn parse_u64(value: &str) -> Result<u64, String> {
    parse_integer(value)
}

fn parse_integer(value: &str) -> Result<u64, String> {
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(|| value.parse::<u64>(), |hex| u64::from_str_radix(hex, 16));
    parsed.map_err(|_| format!("invalid integer {value:?}"))
}

fn generated_request_id() -> RequestId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let time = u64::try_from(nanos & u128::from(u64::MAX)).expect("masked timestamp fits u64");
    RequestId(time ^ u64::from(std::process::id()).rotate_left(32))
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

    #[test]
    fn request_options_reject_reserved_requester_and_preserve_explicit_id() {
        assert!(parse_request_options(&strings(&["--requester", "0"]), &[]).is_err());
        let (options, _) = parse_request_options(
            &strings(&[
                "--interface",
                "vcan0",
                "--requester",
                "4",
                "--request-id",
                "0x0102030405060708",
                "--timeout-ms",
                "50",
                "--retries",
                "1",
                "--json",
            ]),
            &[],
        )
        .unwrap();
        assert_eq!(options.interface, "vcan0");
        assert_eq!(options.requester, RequesterId::new(4).unwrap());
        assert_eq!(options.request_id, RequestId(0x0102_0304_0506_0708));
        assert_eq!(options.timeout, Duration::from_millis(50));
        assert_eq!(options.retries, 1);
        assert!(options.json);
    }

    #[test]
    fn node_port_and_uid_parsing_are_strict() {
        assert_eq!(
            parse_node_port("3.7"),
            Ok(NodePort {
                node: NodeId::new(3).unwrap(),
                port: PortId::new(7).unwrap(),
            })
        );
        assert!(parse_node_port("3.8").is_err());
        assert!(parse_node_port("0.1").is_err());
        assert_eq!(
            format_uid(parse_uid("000102030405060708090a0b").unwrap()),
            "000102030405060708090a0b"
        );
        assert!(parse_uid("abcd").is_err());
    }

    #[test]
    fn policy_options_require_explicit_desired_state() {
        let arguments = strings(&["3.0", "--max-power-mw", "90000"]);
        assert!(set_policy_command(&arguments).is_err());
    }

    #[test]
    fn monitor_recognizes_uid_addressed_commands_as_requests() {
        let frame = encode_commissioning(
            RequesterId::PDCAN_DEFAULT,
            CommissioningMessage::AssignNode {
                request_id: RequestId(1),
                uid: NodeUid::from_bytes([1; NodeUid::LENGTH]),
                node_id: NodeId::new(8).unwrap(),
            },
        )
        .unwrap();
        assert!(is_request(decode_header(frame.id()).unwrap()));
    }
}
