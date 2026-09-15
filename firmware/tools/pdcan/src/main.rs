mod cli;
mod error;

use std::{fmt::Write as _, fs, process::ExitCode};

#[cfg(target_os = "linux")]
use std::time::SystemTime;

use clap::Parser as _;
use cli::Cli;
use error::{CliError, CliResult};
use pdcan_artifact::{Bundle, SignatureAlgorithm};
use pdcan_protocol::{ExtendedId, decode_header};
#[cfg(target_os = "linux")]
use pdcan_types::RequestId;
#[cfg(any(target_os = "linux", test))]
use pdcan_types::{NodeId, NodeUid};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.network.json;
    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if json {
                eprintln!("{}", error.json());
            } else {
                eprintln!("pdcan [{}]: {error}", error.code());
            }
            ExitCode::from(error.exit_code())
        }
    }
}

fn decode_id_command(arguments: &[String]) -> CliResult<()> {
    let [raw] = arguments else {
        return Err("decode-id requires one hexadecimal or decimal ID".into());
    };
    let raw = parse_u32(raw).map_err(CliError::InvalidInput)?;
    let id = ExtendedId::new(raw)
        .map_err(|_| CliError::InvalidInput(format!("ID {raw:#x} is not 29-bit")))?;
    let header = decode_header(id)
        .map_err(|error| CliError::Codec(format!("invalid PDCAN ID: {error:?}")))?;
    println!("id={raw:#010x} header={header:?}");
    Ok(())
}

fn inspect_bundle(arguments: &[String]) -> CliResult<()> {
    let [path] = arguments else {
        return Err("firmware inspect requires exactly one bundle path".into());
    };
    let bytes = fs::read(path)
        .map_err(|error| CliError::Artifact(format!("cannot read {path:?}: {error}")))?;
    let bundle = Bundle::parse(&bytes)
        .map_err(|error| CliError::Artifact(format!("invalid bundle: {error:?}")))?;
    let manifest = bundle.manifest;
    println!("format={}", pdcan_artifact::FORMAT_VERSION);
    println!("role={:?}", manifest.target.role);
    println!("carrier_profile={:?}", manifest.target.carrier_profile);
    println!(
        "hardware_revision={}",
        manifest.target.hardware_revision.get()
    );
    println!("partition_layout={}", manifest.target.partition_layout);
    println!(
        "version={}.{}.{}",
        manifest.version.major, manifest.version.minor, manifest.version.patch
    );
    println!("image_bytes={}", manifest.image_size);
    println!("sha256={}", format_digest(manifest.digest.as_bytes()));
    println!(
        "signature={}",
        match bundle.signature.algorithm {
            SignatureAlgorithm::Unsigned => "unsigned",
            SignatureAlgorithm::Ed25519 => "ed25519 (recorded, enforcement disabled)",
        }
    );
    Ok(())
}

fn format_digest(digest: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(any(target_os = "linux", test))]
fn parse_node(value: &str) -> Result<NodeId, String> {
    let raw = value
        .parse::<u8>()
        .map_err(|_| format!("invalid Node ID {value:?}"))?;
    NodeId::new(raw).map_err(|_| format!("Node ID {raw} is outside 1..=254"))
}

#[cfg(any(target_os = "linux", test))]
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

#[cfg(any(target_os = "linux", test))]
fn format_uid(uid: NodeUid) -> String {
    let mut output = String::with_capacity(NodeUid::LENGTH * 2);
    for byte in uid.as_bytes() {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn parse_u32(value: &str) -> Result<u32, String> {
    u32::try_from(parse_u64(value)?).map_err(|_| format!("integer {value:?} exceeds u32"))
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(|| value.parse(), |hex| u64::from_str_radix(hex, 16))
        .map_err(|_| format!("invalid integer {value:?}"))
}

#[cfg(target_os = "linux")]
fn generated_request_id() -> RequestId {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let time = u64::try_from(nanos & u128::from(u64::MAX)).expect("masked timestamp fits u64");
    RequestId(time ^ u64::from(std::process::id()).rotate_left(32))
}

#[cfg(target_os = "linux")]
fn run_network(arguments: &[String]) -> CliResult<()> {
    linux::run(arguments)
}

#[cfg(not(target_os = "linux"))]
fn run_network(_arguments: &[String]) -> CliResult<()> {
    Err(CliError::Transport(
        "CAN commands require Linux SocketCAN; firmware inspect and decode-id are portable".into(),
    ))
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        collections::BTreeMap,
        io::ErrorKind,
        time::{Duration, Instant},
    };

    use pdcan_artifact::Bundle;
    use pdcan_protocol::{
        BROADCAST_NODE, CommissioningMessage, ControlCommand, ControlRequest, DiscoveryInfo,
        ExtendedId, FIRMWARE_CHUNK_BYTES, FIRMWARE_WINDOW_FRAMES, FirmwareAck, FirmwareRequest,
        FirmwareStatus, WireFrame, commissioning_opcode, decode_command_response,
        decode_commissioning, decode_firmware_ack, decode_firmware_status, decode_header,
        decode_node_info, decode_node_state, encode_commissioning, encode_control_request,
        encode_firmware_request,
    };
    use pdcan_types::{
        CarrierBinding, CarrierPolicy, CarrierProfile, FanConfig, FanId, NodeRole, PdLimits,
        RequesterId, SlotIndex, UpdateError, UpdateImpact, UpdateSessionId, UpdateState,
    };
    use socketcan::{CanAnyFrame, CanFdFrame, CanFdSocket, EmbeddedFrame as _, Frame as _, Socket};

    use super::{
        CliError, CliResult, NodeId, NodeUid, RequestId, format_uid, fs, generated_request_id,
        parse_node, parse_u32, parse_u64, parse_uid,
    };

    const DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);
    const DEFAULT_RETRIES: u8 = 3;

    #[derive(Clone, Debug)]
    struct Options {
        interface: String,
        requester: RequesterId,
        request_id: RequestId,
        timeout: Duration,
        retries: u8,
        json: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Self {
                interface: "can0".into(),
                requester: RequesterId::PDCAN_DEFAULT,
                request_id: generated_request_id(),
                timeout: DEFAULT_TIMEOUT,
                retries: DEFAULT_RETRIES,
                json: false,
            }
        }
    }

    pub fn run(arguments: &[String]) -> CliResult<()> {
        let (options, command) = parse_options(arguments)?;
        match command.first().map(String::as_str) {
            Some("monitor") => monitor(&options, &command[1..]),
            Some("scan") => scan(&options, None),
            Some("info") => {
                let [node] = &command[1..] else {
                    return Err("info requires NODE".into());
                };
                scan(&options, Some(parse_node(node)?))
            }
            Some("identify") => commissioning_command(&options, &command[1..], "identify"),
            Some("assign") => commissioning_command(&options, &command[1..], "assign"),
            Some("clear-node") => commissioning_command(&options, &command[1..], "clear-node"),
            Some("status") => status(&options, &command[1..]),
            Some("set-policy") => set_policy(&options, &command[1..]),
            Some("set-fan") => set_fan(&options, &command[1..]),
            Some("bind") => bind(&options, &command[1..]),
            Some("clear-binding") => clear_binding(&options, &command[1..]),
            Some("emergency-disable") => emergency(&options, &command[1..]),
            Some("acknowledge-resolved") => acknowledge(&options, &command[1..]),
            Some("firmware") => firmware(&options, &command[1..]),
            Some(command) => Err(CliError::Internal(format!(
                "dispatch reached unknown command {command:?}"
            ))),
            None => Err(CliError::Internal("dispatch reached no command".into())),
        }
    }

    fn parse_options(arguments: &[String]) -> CliResult<(Options, Vec<String>)> {
        let mut options = Options::default();
        let mut command = Vec::new();
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--interface" | "-i" => {
                    index += 1;
                    options
                        .interface
                        .clone_from(arguments.get(index).ok_or("--interface requires a value")?);
                }
                "--requester" => {
                    index += 1;
                    let raw = arguments
                        .get(index)
                        .ok_or("--requester requires a value")?
                        .parse::<u8>()
                        .map_err(|_| "--requester must be from 1 through 15".to_owned())?;
                    options.requester = RequesterId::new(raw)
                        .map_err(|_| "--requester must be from 1 through 15".to_owned())?;
                    if options.requester.is_protocol_reserved() {
                        return Err("RequesterId 0 is reserved".into());
                    }
                }
                "--request-id" => {
                    index += 1;
                    options.request_id = RequestId(parse_u64(
                        arguments
                            .get(index)
                            .ok_or("--request-id requires a value")?,
                    )?);
                }
                "--timeout-ms" => {
                    index += 1;
                    let timeout = parse_u64(
                        arguments
                            .get(index)
                            .ok_or("--timeout-ms requires a value")?,
                    )?;
                    if timeout == 0 || timeout > 60_000 {
                        return Err("--timeout-ms must be from 1 through 60000".into());
                    }
                    options.timeout = Duration::from_millis(timeout);
                }
                "--retries" => {
                    index += 1;
                    options.retries = arguments
                        .get(index)
                        .ok_or("--retries requires a value")?
                        .parse::<u8>()
                        .map_err(|_| "--retries must be from 0 through 10".to_owned())?;
                    if options.retries > 10 {
                        return Err("--retries must be from 0 through 10".into());
                    }
                }
                "--json" => options.json = true,
                value => command.push(value.to_owned()),
            }
            index += 1;
        }
        Ok((options, command))
    }

    fn open(options: &Options) -> CliResult<CanFdSocket> {
        CanFdSocket::open(&options.interface).map_err(|error| {
            CliError::Transport(format!("cannot open {}: {error}", options.interface))
        })
    }

    fn monitor(options: &Options, arguments: &[String]) -> CliResult<()> {
        let count = match arguments {
            [] => None,
            [flag, value] if flag == "--count" => {
                Some(value.parse::<usize>().map_err(|_| "invalid count")?)
            }
            _ => return Err("monitor accepts only --count N after global options".into()),
        };
        let socket = open(options)?;
        let mut displayed = 0;
        loop {
            let frame = read_fd(&socket, &options.interface)?;
            let Some((id, payload)) = protocol_frame(&frame) else {
                continue;
            };
            if let Ok(header) = decode_header(id) {
                println!(
                    "id={:#010x} header={header:?} payload={payload:02x?}",
                    id.get()
                );
                displayed += 1;
                if count.is_some_and(|maximum| displayed >= maximum) {
                    return Ok(());
                }
            }
        }
    }

    fn scan(options: &Options, filter: Option<NodeId>) -> CliResult<()> {
        let socket = open(options)?;
        let nonce = options.request_id.0;
        let request =
            encode_commissioning(options.requester, CommissioningMessage::Discover { nonce })
                .map_err(codec)?;
        let mut found = BTreeMap::<NodeUid, DiscoveryInfo>::new();
        for _ in 0..=options.retries {
            write_frame(&socket, request, &options.interface)?;
            let deadline = Instant::now() + options.timeout;
            while let Some(frame) = read_until(&socket, deadline, &options.interface)? {
                let Some((id, payload)) = protocol_frame(&frame) else {
                    continue;
                };
                let Ok((_, CommissioningMessage::DiscoveryResponse { nonce: reply, info })) =
                    decode_commissioning(id, payload)
                else {
                    continue;
                };
                if reply == nonce && filter.is_none_or(|node| info.node_id == Some(node)) {
                    found.insert(info.uid, info);
                }
            }
        }
        if found.is_empty() {
            return Err(CliError::Timeout("no matching nodes replied".into()));
        }
        for info in found.values() {
            print_discovery(info, options.json);
        }
        Ok(())
    }

    fn commissioning_command(
        options: &Options,
        arguments: &[String],
        command: &str,
    ) -> CliResult<()> {
        let message = match (command, arguments) {
            ("identify", [uid]) => CommissioningMessage::Identify {
                request_id: options.request_id,
                uid: parse_uid(uid)?,
                duration_seconds: 10,
            },
            ("identify", [uid, flag, seconds]) if flag == "--seconds" => {
                CommissioningMessage::Identify {
                    request_id: options.request_id,
                    uid: parse_uid(uid)?,
                    duration_seconds: seconds
                        .parse::<u16>()
                        .map_err(|_| "--seconds requires a u16 value")?,
                }
            }
            ("assign", [uid, node]) => CommissioningMessage::AssignNode {
                request_id: options.request_id,
                uid: parse_uid(uid)?,
                node_id: parse_node(node)?,
            },
            ("clear-node", [uid]) => CommissioningMessage::ClearNode {
                request_id: options.request_id,
                uid: parse_uid(uid)?,
            },
            _ => return Err(format!("invalid {command} arguments").into()),
        };
        let expected_opcode = match message {
            CommissioningMessage::Identify { .. } => commissioning_opcode::IDENTIFY,
            CommissioningMessage::AssignNode { .. } => commissioning_opcode::ASSIGN_NODE,
            CommissioningMessage::ClearNode { .. } => commissioning_opcode::CLEAR_NODE,
            _ => unreachable!(),
        };
        let frame = encode_commissioning(options.requester, message).map_err(codec)?;
        let socket = open(options)?;
        for _ in 0..=options.retries {
            write_frame(&socket, frame, &options.interface)?;
            let deadline = Instant::now() + options.timeout;
            while let Some(received) = read_until(&socket, deadline, &options.interface)? {
                let Some((id, payload)) = protocol_frame(&received) else {
                    continue;
                };
                let Ok((
                    _,
                    CommissioningMessage::Result {
                        request_id,
                        uid,
                        request_opcode,
                        result,
                        node_id,
                        state,
                    },
                )) = decode_commissioning(id, payload)
                else {
                    continue;
                };
                if request_id == options.request_id && request_opcode == expected_opcode {
                    println!(
                        "uid={} node={:?} state={state:?} result={result:?}",
                        format_uid(uid),
                        node_id.map(NodeId::get)
                    );
                    return if result == pdcan_protocol::CommandResult::Ok {
                        Ok(())
                    } else {
                        Err(CliError::NodeRejected(format!(
                            "commissioning command failed: {result:?}"
                        )))
                    };
                }
            }
        }
        Err(CliError::Timeout(
            "timed out waiting for commissioning result".into(),
        ))
    }

    fn status(options: &Options, arguments: &[String]) -> CliResult<()> {
        let [node] = arguments else {
            return Err("status requires NODE".into());
        };
        let node = parse_node(node)?;
        let command = ControlCommand::RequestStatus;
        let request = encode_control_request(ControlRequest {
            node: node.get(),
            requester: options.requester,
            request_id: options.request_id,
            command,
        })
        .map_err(codec)?;
        let socket = open(options)?;
        for _ in 0..=options.retries {
            write_frame(&socket, request, &options.interface)?;
            let deadline = Instant::now() + options.timeout;
            let mut accepted = false;
            let mut received_state = false;
            let mut received_info = false;
            while let Some(frame) = read_until(&socket, deadline, &options.interface)? {
                let Some((id, payload)) = protocol_frame(&frame) else {
                    continue;
                };
                if let Ok(response) = decode_command_response(id, payload)
                    && response.requester == options.requester
                    && response.request_id == options.request_id
                    && response.request_opcode == command.opcode()
                {
                    if response.result != pdcan_protocol::CommandResult::Ok {
                        return Err(CliError::NodeRejected(format!(
                            "status request failed: {:?}",
                            response.result
                        )));
                    }
                    accepted = true;
                } else if let Ok(state) = decode_node_state(id, payload)
                    && state.node == node.get()
                {
                    println!("state={state:?}");
                    received_state = true;
                } else if let Ok(info) = decode_node_info(id, payload)
                    && info.node == node.get()
                {
                    println!("info={info:?}");
                    received_info = true;
                }
                if accepted && received_state && received_info {
                    return Ok(());
                }
            }
        }
        Err(CliError::Timeout(
            "timed out waiting for complete node status".into(),
        ))
    }

    fn set_policy(options: &Options, arguments: &[String]) -> CliResult<()> {
        let Some(node) = arguments.first() else {
            return Err("set-policy requires NODE".into());
        };
        let enabled = arguments.iter().any(|value| value == "--enabled");
        let disabled = arguments.iter().any(|value| value == "--disabled");
        if enabled == disabled {
            return Err("set-policy requires exactly one of --enabled or --disabled".into());
        }
        let voltage = named_u32(arguments, "--max-voltage-mv")?;
        let current = named_u32(arguments, "--max-current-ma")?;
        let power = named_u32(arguments, "--max-power-mw")?;
        let any_limit = voltage.is_some() || current.is_some() || power.is_some();
        let all_limits = voltage.is_some() && current.is_some() && power.is_some();
        if any_limit != all_limits {
            return Err("PD limits must supply voltage, current, and power together".into());
        }
        let pd_limits = all_limits.then(|| PdLimits {
            max_voltage_mv: voltage.unwrap(),
            max_current_ma: current.unwrap(),
            max_power_mw: power.unwrap(),
        });
        command(
            options,
            parse_node(node)?.get(),
            ControlCommand::SetCarrierPolicy(CarrierPolicy { enabled, pd_limits }),
        )
    }

    fn set_fan(options: &Options, arguments: &[String]) -> CliResult<()> {
        if arguments.len() < 4 || arguments[2] != "--duty" {
            return Err("set-fan requires NODE FAN --duty PERCENT".into());
        }
        let node = parse_node(&arguments[0])?;
        let fan = FanId::new(
            arguments[1]
                .parse()
                .map_err(|_| "FAN must be 0 or 1".to_owned())?,
        )
        .map_err(|_| "FAN must be 0 or 1".to_owned())?;
        let duty_percent = arguments[3]
            .parse::<u8>()
            .map_err(|_| "duty must be from 0 through 100".to_owned())?;
        command(
            options,
            node.get(),
            ControlCommand::SetFan {
                fan,
                config: FanConfig { duty_percent },
            },
        )
    }

    fn bind(options: &Options, arguments: &[String]) -> CliResult<()> {
        let [node, uid, slot] = arguments else {
            return Err("bind requires NODE BACKPLANE_UID SLOT".into());
        };
        command(
            options,
            parse_node(node)?.get(),
            ControlCommand::SetBinding(Some(CarrierBinding {
                backplane_uid: parse_uid(uid)?,
                slot: SlotIndex::new(
                    slot.parse()
                        .map_err(|_| "SLOT must be from 0 through 254".to_owned())?,
                )
                .map_err(|_| "SLOT must be from 0 through 254".to_owned())?,
            })),
        )
    }

    fn clear_binding(options: &Options, arguments: &[String]) -> CliResult<()> {
        let [node] = arguments else {
            return Err("clear-binding requires NODE".into());
        };
        command(
            options,
            parse_node(node)?.get(),
            ControlCommand::SetBinding(None),
        )
    }

    fn emergency(options: &Options, arguments: &[String]) -> CliResult<()> {
        let [node] = arguments else {
            return Err("emergency-disable requires NODE or all".into());
        };
        let node = if node == "all" {
            BROADCAST_NODE
        } else {
            parse_node(node)?.get()
        };
        command(options, node, ControlCommand::EmergencyDisable)
    }

    fn acknowledge(options: &Options, arguments: &[String]) -> CliResult<()> {
        let [node] = arguments else {
            return Err("acknowledge-resolved requires NODE".into());
        };
        command(
            options,
            parse_node(node)?.get(),
            ControlCommand::AcknowledgeEmergency,
        )
    }

    fn command(options: &Options, node: u8, command: ControlCommand) -> CliResult<()> {
        let frame = encode_control_request(ControlRequest {
            node,
            requester: options.requester,
            request_id: options.request_id,
            command,
        })
        .map_err(codec)?;
        let socket = open(options)?;
        for _ in 0..=options.retries {
            write_frame(&socket, frame, &options.interface)?;
            let deadline = Instant::now() + options.timeout;
            while let Some(received) = read_until(&socket, deadline, &options.interface)? {
                let Some((id, payload)) = protocol_frame(&received) else {
                    continue;
                };
                let Ok(response) = decode_command_response(id, payload) else {
                    continue;
                };
                if response.requester == options.requester
                    && response.request_id == options.request_id
                    && response.request_opcode == command.opcode()
                {
                    println!("response={:?} detail={}", response.result, response.detail);
                    return if response.result == pdcan_protocol::CommandResult::Ok {
                        Ok(())
                    } else {
                        Err(CliError::NodeRejected(format!(
                            "command failed: {:?}",
                            response.result
                        )))
                    };
                }
            }
            if node == BROADCAST_NODE {
                return Ok(());
            }
        }
        Err(CliError::Timeout(
            "timed out waiting for command response".into(),
        ))
    }

    fn firmware(options: &Options, arguments: &[String]) -> CliResult<()> {
        match arguments {
            [action, node] if action == "status" => {
                let status = firmware_status(options, parse_node(node)?)?;
                print_firmware_status(status);
                Ok(())
            }
            [action, node, bundle] if action == "stage" => {
                stage(options, parse_node(node)?, bundle).map(|_| ())
            }
            [action, node] if action == "activate" => activate(options, parse_node(node)?, false),
            [action, node, flag] if action == "activate" && flag == "--allow-interruption" => {
                activate(options, parse_node(node)?, true)
            }
            [action, node, bundle] if action == "update" => {
                let node = parse_node(node)?;
                stage(options, node, bundle)?;
                activate(options, node, false)
            }
            [action, node, bundle, flag]
                if action == "update" && flag == "--allow-interruption" =>
            {
                let node = parse_node(node)?;
                stage(options, node, bundle)?;
                activate(options, node, true)
            }
            [action, node] if action == "abort" => abort(options, parse_node(node)?),
            _ => Err("invalid firmware command; see pdcan --help".into()),
        }
    }

    fn stage(options: &Options, node: NodeId, path: &str) -> CliResult<UpdateSessionId> {
        let bytes = fs::read(path)
            .map_err(|error| CliError::Artifact(format!("cannot read {path:?}: {error}")))?;
        let bundle = Bundle::parse(&bytes)
            .map_err(|error| CliError::Artifact(format!("invalid bundle: {error:?}")))?;
        let request_bytes = options.request_id.0.to_le_bytes();
        let session = UpdateSessionId(
            u32::from_le_bytes([
                request_bytes[0],
                request_bytes[1],
                request_bytes[2],
                request_bytes[3],
            ]) ^ u32::from(node.get()),
        );
        let socket = open(options)?;
        let begin = encode_firmware_request(
            node,
            options.requester,
            FirmwareRequest::Begin {
                request_id: options.request_id,
                session,
                manifest: bundle.manifest,
            },
        )
        .map_err(codec)?;
        let ack = send_firmware_request(options, &socket, begin, |ack| {
            ack.request_id == options.request_id && ack.session == session
        })?;
        require_update_ok(ack)?;

        transfer_image(options, &socket, node, session, &bundle, ack.next_offset)?;

        let finish_id = RequestId(options.request_id.0.wrapping_add(1));
        let finish = encode_firmware_request(
            node,
            options.requester,
            FirmwareRequest::Finish {
                request_id: finish_id,
                session,
            },
        )
        .map_err(codec)?;
        let ack = send_firmware_request(options, &socket, finish, |ack| {
            ack.request_id == finish_id && ack.session == session
        })?;
        require_update_ok(ack)?;
        if ack.state != UpdateState::Staged {
            return Err(CliError::NodeRejected(format!(
                "node did not stage image: {:?}",
                ack.state
            )));
        }
        println!("staged node={} session={}", node.get(), session.0);
        Ok(session)
    }

    fn transfer_image(
        options: &Options,
        socket: &CanFdSocket,
        node: NodeId,
        session: UpdateSessionId,
        bundle: &Bundle<'_>,
        mut offset: u32,
    ) -> CliResult<()> {
        while offset < bundle.manifest.image_size {
            let window_start = offset;
            for _ in 0..=options.retries {
                let sent_until = send_data_window(options, socket, node, session, bundle, offset)?;
                if let Some(next_offset) =
                    receive_window_ack(options, socket, node, session, sent_until)?
                    && next_offset > window_start
                {
                    offset = next_offset;
                    break;
                }
            }
            if offset == window_start {
                return Err(CliError::Timeout(format!(
                    "firmware transfer stalled at offset {offset} after {} attempts",
                    u16::from(options.retries) + 1
                )));
            }
        }
        Ok(())
    }

    fn send_data_window(
        options: &Options,
        socket: &CanFdSocket,
        node: NodeId,
        session: UpdateSessionId,
        bundle: &Bundle<'_>,
        offset: u32,
    ) -> CliResult<u32> {
        let mut sent_until = offset;
        for _ in 0..FIRMWARE_WINDOW_FRAMES {
            if sent_until >= bundle.manifest.image_size {
                break;
            }
            let start = usize::try_from(sent_until).map_err(|_| {
                CliError::Internal("firmware image offset does not fit usize".into())
            })?;
            let len = (bundle.image.len() - start).min(FIRMWARE_CHUNK_BYTES);
            let mut chunk = [0; FIRMWARE_CHUNK_BYTES];
            chunk[..len].copy_from_slice(&bundle.image[start..start + len]);
            let frame = encode_firmware_request(
                node,
                options.requester,
                FirmwareRequest::Data {
                    session,
                    offset: sent_until,
                    len: u8::try_from(len).expect("firmware chunk length fits u8"),
                    bytes: chunk,
                },
            )
            .map_err(codec)?;
            write_frame(socket, frame, &options.interface)?;
            sent_until += u32::try_from(len).expect("firmware chunk length fits u32");
        }
        Ok(sent_until)
    }

    fn receive_window_ack(
        options: &Options,
        socket: &CanFdSocket,
        node: NodeId,
        session: UpdateSessionId,
        sent_until: u32,
    ) -> CliResult<Option<u32>> {
        let deadline = Instant::now() + options.timeout;
        let mut cumulative = None;
        while let Some(received) = read_until(socket, deadline, &options.interface)? {
            let Some((id, payload)) = protocol_frame(&received) else {
                continue;
            };
            let Ok(ack) = decode_firmware_ack(id, payload) else {
                continue;
            };
            if ack.node == node.get()
                && ack.requester == options.requester
                && ack.session == session
            {
                require_update_ok(ack)?;
                cumulative = Some(ack.next_offset);
                if ack.next_offset >= sent_until {
                    break;
                }
            }
        }
        Ok(cumulative)
    }

    fn firmware_status(options: &Options, node: NodeId) -> CliResult<FirmwareStatus> {
        let socket = open(options)?;
        let request = encode_firmware_request(
            node,
            options.requester,
            FirmwareRequest::Status {
                request_id: options.request_id,
            },
        )
        .map_err(codec)?;
        for _ in 0..=options.retries {
            write_frame(&socket, request, &options.interface)?;
            let deadline = Instant::now() + options.timeout;
            while let Some(received) = read_until(&socket, deadline, &options.interface)? {
                let Some((id, payload)) = protocol_frame(&received) else {
                    continue;
                };
                let Ok(status) = decode_firmware_status(id, payload) else {
                    continue;
                };
                if status.node == node.get()
                    && status.requester == options.requester
                    && status.request_id == options.request_id
                {
                    return Ok(status);
                }
            }
        }
        Err(CliError::Timeout(
            "timed out waiting for firmware status".into(),
        ))
    }

    fn activate(options: &Options, node: NodeId, allow_interruption: bool) -> CliResult<()> {
        let status = firmware_status(options, node)?;
        if status.state != UpdateState::Staged {
            return Err(CliError::NodeRejected(format!(
                "node has no staged image ({:?})",
                status.state
            )));
        }
        if status.impact == UpdateImpact::Interrupt && !allow_interruption {
            return Err(CliError::InterruptionRequired);
        }
        let socket = open(options)?;
        let request_id = RequestId(options.request_id.0.wrapping_add(2));
        let request = encode_firmware_request(
            node,
            options.requester,
            FirmwareRequest::Activate {
                request_id,
                session: status.session,
                allow_interruption,
            },
        )
        .map_err(codec)?;
        let ack = send_firmware_request(options, &socket, request, |ack| {
            ack.request_id == request_id && ack.session == status.session
        })?;
        require_update_ok(ack)?;
        println!("activation accepted for node={}", node.get());
        Ok(())
    }

    fn abort(options: &Options, node: NodeId) -> CliResult<()> {
        let status = firmware_status(options, node)?;
        let socket = open(options)?;
        let request_id = RequestId(options.request_id.0.wrapping_add(3));
        let request = encode_firmware_request(
            node,
            options.requester,
            FirmwareRequest::Abort {
                request_id,
                session: status.session,
            },
        )
        .map_err(codec)?;
        let ack = send_firmware_request(options, &socket, request, |ack| {
            ack.request_id == request_id
        })?;
        require_update_ok(ack)?;
        println!("aborted firmware staging on node={}", node.get());
        Ok(())
    }

    fn send_firmware_request(
        options: &Options,
        socket: &CanFdSocket,
        frame: WireFrame,
        matches: impl Fn(FirmwareAck) -> bool,
    ) -> CliResult<FirmwareAck> {
        for _ in 0..=options.retries {
            write_frame(socket, frame, &options.interface)?;
            let deadline = Instant::now() + options.timeout;
            while let Some(received) = read_until(socket, deadline, &options.interface)? {
                let Some((id, payload)) = protocol_frame(&received) else {
                    continue;
                };
                if let Ok(ack) = decode_firmware_ack(id, payload)
                    && matches(ack)
                {
                    return Ok(ack);
                }
            }
        }
        Err(CliError::Timeout(
            "timed out waiting for firmware acknowledgement".into(),
        ))
    }

    fn require_update_ok(ack: FirmwareAck) -> CliResult<()> {
        if ack.error == UpdateError::None {
            Ok(())
        } else {
            Err(CliError::NodeRejected(format!(
                "firmware operation failed at offset {}: {:?}",
                ack.next_offset, ack.error
            )))
        }
    }

    fn print_firmware_status(status: FirmwareStatus) {
        println!(
            "node={} state={:?} impact={:?} running={}.{}.{} staged={:?} session={} offset={} error={:?}",
            status.node,
            status.state,
            status.impact,
            status.running.major,
            status.running.minor,
            status.running.patch,
            status.staged,
            status.session.0,
            status.next_offset,
            status.error
        );
    }

    fn named_u32(arguments: &[String], name: &str) -> Result<Option<u32>, String> {
        let Some(index) = arguments.iter().position(|argument| argument == name) else {
            return Ok(None);
        };
        arguments
            .get(index + 1)
            .ok_or_else(|| format!("{name} requires a value"))
            .and_then(|value| parse_u32(value))
            .map(Some)
    }

    fn print_discovery(info: &DiscoveryInfo, json: bool) {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "uid": format_uid(info.uid),
                    "node": info.node_id.map(NodeId::get),
                    "role": match info.node_info.descriptor.role {
                        NodeRole::Backplane => "backplane",
                        NodeRole::Carrier => "carrier",
                    },
                    "profile": info.node_info.descriptor.carrier_profile.map(|profile| match profile {
                        CarrierProfile::Basic => "basic",
                        CarrierProfile::Sw3538 => "sw3538",
                        CarrierProfile::Accessory => "accessory",
                        CarrierProfile::HighPower240W => "high_power_240w",
                    }),
                    "update_impact": match info.node_info.descriptor.update_impact {
                        UpdateImpact::Live => "live",
                        UpdateImpact::Interrupt => "interrupt",
                    },
                    "capabilities": info.node_info.descriptor.capabilities.bits(),
                    "protocol": format!("{}.{}", info.protocol_major, info.protocol_minor),
                })
            );
        } else {
            println!(
                "uid={} node={:?} role={:?} profile={:?} update={:?} capabilities=0x{:016x} protocol={}.{}",
                format_uid(info.uid),
                info.node_id.map(NodeId::get),
                info.node_info.descriptor.role,
                info.node_info.descriptor.carrier_profile,
                info.node_info.descriptor.update_impact,
                info.node_info.descriptor.capabilities.bits(),
                info.protocol_major,
                info.protocol_minor
            );
        }
    }

    fn protocol_frame(frame: &CanFdFrame) -> Option<(ExtendedId, &[u8])> {
        if !frame.is_brs() || !frame.is_extended() {
            return None;
        }
        ExtendedId::new(frame.raw_id())
            .ok()
            .map(|id| (id, frame.data()))
    }

    fn read_fd(socket: &CanFdSocket, interface: &str) -> CliResult<CanFdFrame> {
        loop {
            match socket.read_frame() {
                Ok(CanAnyFrame::Fd(frame)) => return Ok(frame),
                Ok(_) => {}
                Err(error) => {
                    return Err(CliError::Transport(format!(
                        "read from {interface} failed: {error}"
                    )));
                }
            }
        }
    }

    fn read_until(
        socket: &CanFdSocket,
        deadline: Instant,
        interface: &str,
    ) -> CliResult<Option<CanFdFrame>> {
        loop {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(None);
            };
            socket
                .set_read_timeout(remaining.max(Duration::from_millis(1)))
                .map_err(|error| {
                    CliError::Transport(format!(
                        "cannot set receive timeout on {interface}: {error}"
                    ))
                })?;
            match socket.read_frame() {
                Ok(CanAnyFrame::Fd(frame)) => return Ok(Some(frame)),
                Ok(_) => {}
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    return Ok(None);
                }
                Err(error) => {
                    return Err(CliError::Transport(format!(
                        "read from {interface} failed: {error}"
                    )));
                }
            }
        }
    }

    fn write_frame(socket: &CanFdSocket, frame: WireFrame, interface: &str) -> CliResult<()> {
        let can_id = socketcan::ExtendedId::new(frame.id().get())
            .ok_or_else(|| CliError::Internal("protocol emitted an invalid extended ID".into()))?;
        let mut frame = CanFdFrame::new(can_id, frame.payload()).ok_or_else(|| {
            CliError::Internal("protocol emitted an invalid CAN-FD payload".into())
        })?;
        frame.set_brs(true);
        socket
            .write_frame(&frame)
            .map_err(|error| CliError::Transport(format!("write to {interface} failed: {error}")))
    }

    fn codec(error: impl std::fmt::Debug) -> CliError {
        CliError::Codec(format!("cannot encode PDCAN frame: {error:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_artifact::build_unsigned;
    use pdcan_types::{
        CarrierProfile, FirmwareVersion, HardwareRevision, ImageTarget, NodeRole, Sha256Digest,
        UpdateManifest,
    };
    use sha2::{Digest as _, Sha256};

    #[test]
    fn node_and_uid_parsing_are_strict() {
        assert_eq!(parse_node("254").unwrap().get(), 254);
        assert!(parse_node("0").is_err());
        assert!(parse_node("255").is_err());
        let uid = parse_uid("000102030405060708090a0b").unwrap();
        assert_eq!(format_uid(uid), "000102030405060708090a0b");
        assert!(parse_uid("short").is_err());
    }

    #[test]
    fn bundle_inspection_accepts_a_valid_signed_ready_artifact() {
        let image = b"firmware";
        let digest: [u8; 32] = Sha256::digest(image).into();
        let manifest = UpdateManifest {
            target: ImageTarget {
                role: NodeRole::Carrier,
                carrier_profile: Some(CarrierProfile::Sw3538),
                hardware_revision: HardwareRevision::REV_A,
                partition_layout: 1,
            },
            version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            image_size: u32::try_from(image.len()).unwrap(),
            digest: Sha256Digest::from_bytes(digest),
        };
        let artifact = build_unsigned(manifest, image).unwrap();
        let parsed = Bundle::parse(&artifact).unwrap();
        assert_eq!(parsed.manifest, manifest);
    }
}
