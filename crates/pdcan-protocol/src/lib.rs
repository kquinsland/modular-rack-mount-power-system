#![no_std]

#[cfg(test)]
extern crate std;

use pdcan_types::{
    CommissioningState, FanConfig, FanMode, FaultFlags, FirmwareVersion, NodeId, NodeUid,
    PolicyValidationError, PortId, PortPolicy, RequestId, RequesterId,
};

pub const EXTENDED_ID_MASK: u32 = 0x1FFF_FFFF;
pub const BROADCAST_NODE: u8 = 0;
pub const INVALID_NODE: u8 = 0xFF;
pub const ALL_PORTS_TARGET: u8 = 0x0E;
pub const BOARD_TARGET: u8 = 0x0F;
pub const PROTOCOL_MAJOR: u8 = 0;
pub const PROTOCOL_MINOR: u8 = 1;

pub mod control_opcode {
    pub const EMERGENCY_DISABLE: u8 = 0;
    pub const SET_PORT_POLICY: u8 = 1;
    pub const ACKNOWLEDGE_EMERGENCY_RESOLVED: u8 = 2;
    pub const REQUEST_STATUS: u8 = 3;
    pub const SET_FAN_CONFIG: u8 = 4;
}

pub mod response_opcode {
    pub const COMMAND_RESPONSE: u8 = 0;
}

pub mod state_opcode {
    pub const PORT_STATE: u8 = 0;
}

pub mod telemetry_opcode {
    pub const PORT_POWER: u8 = 0;
}

pub mod management_opcode {
    pub const HEARTBEAT: u8 = 0;
    pub const BOARD_INFO: u8 = 1;
}

pub mod commissioning_opcode {
    pub const NODE_CLAIM: u8 = 0;
    pub const DISCOVER: u8 = 1;
    pub const DISCOVERY_RESPONSE: u8 = 2;
    pub const IDENTIFY: u8 = 3;
    pub const ASSIGN_NODE: u8 = 4;
    pub const CLEAR_NODE: u8 = 5;
    pub const RESULT: u8 = 6;
}

const PRIORITY_SHIFT: u32 = 26;
const CLASS_SHIFT: u32 = 22;
const NODE_SHIFT: u32 = 14;
const TARGET_SHIFT: u32 = 10;
const REQUESTER_SHIFT: u32 = 6;

const MAX_PRIORITY: u8 = 0x07;
const MAX_TARGET: u8 = 0x0F;
const MAX_OPCODE: u8 = 0x3F;

const PRIORITY_MASK: u32 = 0x07;
const CLASS_MASK: u32 = 0x0F;
const NODE_MASK: u32 = 0xFF;
const TARGET_MASK: u32 = 0x0F;
const REQUESTER_MASK: u32 = 0x0F;
const OPCODE_MASK: u32 = 0x3F;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MessageClass {
    Control = 0x0,
    Response = 0x1,
    State = 0x2,
    Telemetry = 0x3,
    Management = 0x4,
    Configuration = 0x5,
    Commissioning = 0xE,
    Debug = 0xF,
}

impl TryFrom<u8> for MessageClass {
    type Error = InvalidClass;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Self::Control),
            0x1 => Ok(Self::Response),
            0x2 => Ok(Self::State),
            0x3 => Ok(Self::Telemetry),
            0x4 => Ok(Self::Management),
            0x5 => Ok(Self::Configuration),
            0xE => Ok(Self::Commissioning),
            0xF => Ok(Self::Debug),
            _ => Err(InvalidClass(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidClass(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub priority: u8,
    pub class: MessageClass,
    pub node: u8,
    pub target: u8,
    pub requester: RequesterId,
    pub opcode: u8,
}

impl Header {
    pub const fn new(
        priority: u8,
        class: MessageClass,
        node: u8,
        target: u8,
        requester: RequesterId,
        opcode: u8,
    ) -> Result<Self, InvalidHeader> {
        if priority > MAX_PRIORITY {
            return Err(InvalidHeader::Priority(priority));
        }
        if node == INVALID_NODE {
            return Err(InvalidHeader::Node(node));
        }
        if target > MAX_TARGET {
            return Err(InvalidHeader::Target(target));
        }
        if opcode > MAX_OPCODE {
            return Err(InvalidHeader::Opcode(opcode));
        }

        Ok(Self {
            priority,
            class,
            node,
            target,
            requester,
            opcode,
        })
    }

    pub const fn encode(self) -> ExtendedId {
        ExtendedId(
            (self.priority as u32) << PRIORITY_SHIFT
                | (self.class as u32) << CLASS_SHIFT
                | (self.node as u32) << NODE_SHIFT
                | (self.target as u32) << TARGET_SHIFT
                | (self.requester.get() as u32) << REQUESTER_SHIFT
                | self.opcode as u32,
        )
    }

    pub fn decode(id: ExtendedId) -> Result<Self, InvalidHeader> {
        let raw = id.get();
        let priority = ((raw >> PRIORITY_SHIFT) & PRIORITY_MASK) as u8;
        let class_raw = ((raw >> CLASS_SHIFT) & CLASS_MASK) as u8;
        let node = ((raw >> NODE_SHIFT) & NODE_MASK) as u8;
        let target = ((raw >> TARGET_SHIFT) & TARGET_MASK) as u8;
        let requester_raw = ((raw >> REQUESTER_SHIFT) & REQUESTER_MASK) as u8;
        let opcode = (raw & OPCODE_MASK) as u8;

        let class = MessageClass::try_from(class_raw)
            .map_err(|InvalidClass(value)| InvalidHeader::Class(value))?;
        let requester =
            RequesterId::new(requester_raw).map_err(|error| InvalidHeader::Requester(error.0))?;
        Self::new(priority, class, node, target, requester, opcode)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidHeader {
    Priority(u8),
    Class(u8),
    Node(u8),
    Target(u8),
    Requester(u8),
    Opcode(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtendedId(u32);

impl ExtendedId {
    pub const fn new(raw: u32) -> Result<Self, InvalidExtendedId> {
        if raw <= EXTENDED_ID_MASK {
            Ok(Self(raw))
        } else {
            Err(InvalidExtendedId(raw))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidExtendedId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetKind {
    Port,
    AllPorts,
    Board,
    Commissioning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageSender {
    Host,
    Backpack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageDefinition {
    pub name: &'static str,
    pub class: MessageClass,
    pub opcode: u8,
    pub priority: u8,
    pub target: TargetKind,
    pub payload_len: u8,
    pub requester_scoped: bool,
    pub has_request_id: bool,
    pub sender: MessageSender,
}

pub const MESSAGE_DEFINITIONS: &[MessageDefinition] = &[
    MessageDefinition {
        name: "EMERGENCY_DISABLE",
        class: MessageClass::Control,
        opcode: control_opcode::EMERGENCY_DISABLE,
        priority: 0,
        target: TargetKind::AllPorts,
        payload_len: 8,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "SET_PORT_POLICY",
        class: MessageClass::Control,
        opcode: control_opcode::SET_PORT_POLICY,
        priority: 1,
        target: TargetKind::Port,
        payload_len: 20,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "ACKNOWLEDGE_EMERGENCY_RESOLVED",
        class: MessageClass::Control,
        opcode: control_opcode::ACKNOWLEDGE_EMERGENCY_RESOLVED,
        priority: 1,
        target: TargetKind::Board,
        payload_len: 8,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "REQUEST_STATUS",
        class: MessageClass::Control,
        opcode: control_opcode::REQUEST_STATUS,
        priority: 1,
        target: TargetKind::Port,
        payload_len: 8,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "SET_FAN_CONFIG",
        class: MessageClass::Control,
        opcode: control_opcode::SET_FAN_CONFIG,
        priority: 1,
        target: TargetKind::Board,
        payload_len: 12,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "COMMAND_RESPONSE",
        class: MessageClass::Response,
        opcode: response_opcode::COMMAND_RESPONSE,
        priority: 2,
        target: TargetKind::Board,
        payload_len: 16,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "PORT_STATE",
        class: MessageClass::State,
        opcode: state_opcode::PORT_STATE,
        priority: 3,
        target: TargetKind::Port,
        payload_len: 16,
        requester_scoped: false,
        has_request_id: false,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "PORT_POWER",
        class: MessageClass::Telemetry,
        opcode: telemetry_opcode::PORT_POWER,
        priority: 4,
        target: TargetKind::Port,
        payload_len: 16,
        requester_scoped: false,
        has_request_id: false,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "HEARTBEAT",
        class: MessageClass::Management,
        opcode: management_opcode::HEARTBEAT,
        priority: 5,
        target: TargetKind::Board,
        payload_len: 24,
        requester_scoped: false,
        has_request_id: false,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "BOARD_INFO",
        class: MessageClass::Management,
        opcode: management_opcode::BOARD_INFO,
        priority: 5,
        target: TargetKind::Board,
        payload_len: 24,
        requester_scoped: false,
        has_request_id: false,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "NODE_CLAIM",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::NODE_CLAIM,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 16,
        requester_scoped: false,
        has_request_id: false,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "DISCOVER",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::DISCOVER,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 8,
        requester_scoped: true,
        has_request_id: false,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "DISCOVERY_RESPONSE",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::DISCOVERY_RESPONSE,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 32,
        requester_scoped: true,
        has_request_id: false,
        sender: MessageSender::Backpack,
    },
    MessageDefinition {
        name: "IDENTIFY",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::IDENTIFY,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 24,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "ASSIGN_NODE",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::ASSIGN_NODE,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 24,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "CLEAR_NODE",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::CLEAR_NODE,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 20,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Host,
    },
    MessageDefinition {
        name: "COMMISSIONING_RESULT",
        class: MessageClass::Commissioning,
        opcode: commissioning_opcode::RESULT,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 24,
        requester_scoped: true,
        has_request_id: true,
        sender: MessageSender::Backpack,
    },
];

pub fn encode_request_id(request_id: RequestId, output: &mut [u8]) -> Result<(), PayloadError> {
    let destination = output.get_mut(..8).ok_or(PayloadError::TooShort)?;
    destination.copy_from_slice(&request_id.0.to_le_bytes());
    Ok(())
}

pub fn decode_request_id(payload: &[u8]) -> Result<RequestId, PayloadError> {
    let source: [u8; 8] = payload
        .get(..8)
        .ok_or(PayloadError::TooShort)?
        .try_into()
        .map_err(|_| PayloadError::TooShort)?;
    Ok(RequestId(u64::from_le_bytes(source)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadError {
    TooShort,
    WrongLength { expected: u8, actual: u8 },
    InvalidBoolean { offset: u8, value: u8 },
    InvalidValue { offset: u8, value: u8 },
    ReservedNonZero { offset: u8 },
    InvalidNodeId(u8),
    InvalidPortId(u8),
    InvalidPolicy(PolicyValidationError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireFrame {
    id: ExtendedId,
    len: u8,
    data: [u8; 64],
}

impl WireFrame {
    pub fn new(id: ExtendedId, payload: &[u8]) -> Result<Self, FrameError> {
        let len = u8::try_from(payload.len()).map_err(|_| FrameError::PayloadTooLong)?;
        if len > 64 {
            return Err(FrameError::PayloadTooLong);
        }
        if !matches!(len, 0..=8 | 12 | 16 | 20 | 24 | 32 | 48 | 64) {
            return Err(FrameError::InvalidCanFdLength(len));
        }
        let mut data = [0; 64];
        data[..payload.len()].copy_from_slice(payload);
        Ok(Self { id, len, data })
    }

    pub const fn id(&self) -> ExtendedId {
        self.id
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    PayloadTooLong,
    InvalidCanFdLength(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    InvalidHeader(InvalidHeader),
    UnexpectedClass(u8),
    UnknownOpcode(u8),
    WrongTarget(u8),
    InvalidCommissioningToken(u16),
    Payload(PayloadError),
    Frame(FrameError),
}

impl From<InvalidHeader> for CodecError {
    fn from(error: InvalidHeader) -> Self {
        Self::InvalidHeader(error)
    }
}

impl From<PayloadError> for CodecError {
    fn from(error: PayloadError) -> Self {
        Self::Payload(error)
    }
}

impl From<FrameError> for CodecError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCommand {
    EmergencyDisable,
    SetPortPolicy { port: PortId, policy: PortPolicy },
    AcknowledgeEmergencyResolved,
    RequestStatus { port: PortId },
    SetFanConfig { config: FanConfig },
}

impl ControlCommand {
    pub const fn opcode(self) -> u8 {
        match self {
            Self::EmergencyDisable => control_opcode::EMERGENCY_DISABLE,
            Self::SetPortPolicy { .. } => control_opcode::SET_PORT_POLICY,
            Self::AcknowledgeEmergencyResolved => control_opcode::ACKNOWLEDGE_EMERGENCY_RESOLVED,
            Self::RequestStatus { .. } => control_opcode::REQUEST_STATUS,
            Self::SetFanConfig { .. } => control_opcode::SET_FAN_CONFIG,
        }
    }

    pub const fn target(self) -> u8 {
        match self {
            Self::EmergencyDisable => ALL_PORTS_TARGET,
            Self::SetPortPolicy { port, .. } | Self::RequestStatus { port } => port.get(),
            Self::AcknowledgeEmergencyResolved | Self::SetFanConfig { .. } => BOARD_TARGET,
        }
    }

    pub const fn priority(self) -> u8 {
        match self {
            Self::EmergencyDisable => 0,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlRequest {
    pub node: u8,
    pub requester: RequesterId,
    pub request_id: RequestId,
    pub command: ControlCommand,
}

pub fn encode_control_request(request: ControlRequest) -> Result<WireFrame, CodecError> {
    let command = request.command;
    require_host_requester(request.requester)?;
    if !matches!(command, ControlCommand::EmergencyDisable) {
        require_operational_node(request.node)?;
    }
    let header = Header::new(
        command.priority(),
        MessageClass::Control,
        request.node,
        command.target(),
        request.requester,
        command.opcode(),
    )?;
    let mut payload = [0u8; 20];
    encode_request_id(request.request_id, &mut payload)?;
    let len = match command {
        ControlCommand::EmergencyDisable
        | ControlCommand::AcknowledgeEmergencyResolved
        | ControlCommand::RequestStatus { .. } => 8,
        ControlCommand::SetPortPolicy { policy, .. } => {
            payload[8] = u8::from(policy.enabled);
            payload[10..12].copy_from_slice(&policy.max_voltage_mv.to_le_bytes());
            payload[12..14].copy_from_slice(&policy.max_current_ma.to_le_bytes());
            payload[16..20].copy_from_slice(&policy.max_power_mw.to_le_bytes());
            20
        }
        ControlCommand::SetFanConfig { config } => {
            payload[8] = config.mode as u8;
            payload[9] = config.duty_percent;
            12
        }
    };
    WireFrame::new(header.encode(), &payload[..len]).map_err(Into::into)
}

pub fn decode_control_request(
    id: ExtendedId,
    payload: &[u8],
) -> Result<ControlRequest, CodecError> {
    let header = Header::decode(id)?;
    if header.class != MessageClass::Control {
        return Err(CodecError::UnexpectedClass(header.class as u8));
    }
    require_host_requester(header.requester)?;

    let request_id = decode_request_id(payload)?;
    let command = match header.opcode {
        control_opcode::EMERGENCY_DISABLE => {
            require_target(header.target, ALL_PORTS_TARGET)?;
            require_len(payload, 8)?;
            ControlCommand::EmergencyDisable
        }
        control_opcode::SET_PORT_POLICY => {
            require_len(payload, 20)?;
            let port =
                PortId::new(header.target).map_err(|error| PayloadError::InvalidPortId(error.0))?;
            require_zero(payload, 9)?;
            require_zero_range(payload, 14, 16)?;
            if payload[8] & !1 != 0 {
                return Err(PayloadError::InvalidValue {
                    offset: 8,
                    value: payload[8],
                }
                .into());
            }
            let policy = PortPolicy {
                enabled: payload[8] != 0,
                max_voltage_mv: read_u16(payload, 10),
                max_current_ma: read_u16(payload, 12),
                max_power_mw: read_u32(payload, 16),
            };
            policy.validate().map_err(PayloadError::InvalidPolicy)?;
            ControlCommand::SetPortPolicy { port, policy }
        }
        control_opcode::ACKNOWLEDGE_EMERGENCY_RESOLVED => {
            require_target(header.target, BOARD_TARGET)?;
            require_len(payload, 8)?;
            ControlCommand::AcknowledgeEmergencyResolved
        }
        control_opcode::REQUEST_STATUS => {
            require_len(payload, 8)?;
            let port =
                PortId::new(header.target).map_err(|error| PayloadError::InvalidPortId(error.0))?;
            ControlCommand::RequestStatus { port }
        }
        control_opcode::SET_FAN_CONFIG => {
            require_target(header.target, BOARD_TARGET)?;
            require_len(payload, 12)?;
            require_zero_range(payload, 10, 12)?;
            let mode = match payload[8] {
                0 => FanMode::ThreeWire,
                1 => FanMode::FourWire,
                value => return Err(PayloadError::InvalidValue { offset: 8, value }.into()),
            };
            let config = FanConfig {
                mode,
                duty_percent: payload[9],
            };
            config
                .validate()
                .map_err(|error| PayloadError::InvalidValue {
                    offset: 9,
                    value: error.0,
                })?;
            ControlCommand::SetFanConfig { config }
        }
        opcode => return Err(CodecError::UnknownOpcode(opcode)),
    };
    if header.priority != command.priority() {
        return Err(CodecError::InvalidHeader(InvalidHeader::Priority(
            header.priority,
        )));
    }
    if !matches!(command, ControlCommand::EmergencyDisable) {
        require_operational_node(header.node)?;
    }
    Ok(ControlRequest {
        node: header.node,
        requester: header.requester,
        request_id,
        command,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CommandResult {
    Ok = 0,
    OkPending = 1,
    InvalidArgument = 2,
    InvalidTarget = 3,
    Unsupported = 4,
    Busy = 5,
    NoModule = 6,
    NotReady = 7,
    I2cError = 8,
    PdControllerError = 9,
    Timeout = 10,
    VerifyFailed = 11,
    AddressConflict = 12,
    PersistFailed = 13,
    EmergencyLatched = 14,
    InternalError = 15,
}

impl TryFrom<u8> for CommandResult {
    type Error = PayloadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::OkPending),
            2 => Ok(Self::InvalidArgument),
            3 => Ok(Self::InvalidTarget),
            4 => Ok(Self::Unsupported),
            5 => Ok(Self::Busy),
            6 => Ok(Self::NoModule),
            7 => Ok(Self::NotReady),
            8 => Ok(Self::I2cError),
            9 => Ok(Self::PdControllerError),
            10 => Ok(Self::Timeout),
            11 => Ok(Self::VerifyFailed),
            12 => Ok(Self::AddressConflict),
            13 => Ok(Self::PersistFailed),
            14 => Ok(Self::EmergencyLatched),
            15 => Ok(Self::InternalError),
            _ => Err(PayloadError::InvalidValue { offset: 10, value }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandResponse {
    pub node: u8,
    pub requester: RequesterId,
    pub request_id: RequestId,
    pub request_opcode: u8,
    pub request_target: u8,
    pub result: CommandResult,
    pub detail: u32,
}

pub fn encode_command_response(response: CommandResponse) -> Result<WireFrame, CodecError> {
    require_operational_node(response.node)?;
    require_host_requester(response.requester)?;
    let header = Header::new(
        2,
        MessageClass::Response,
        response.node,
        BOARD_TARGET,
        response.requester,
        response_opcode::COMMAND_RESPONSE,
    )?;
    let mut payload = [0u8; 16];
    encode_request_id(response.request_id, &mut payload)?;
    payload[8] = response.request_opcode;
    payload[9] = response.request_target;
    payload[10] = response.result as u8;
    payload[12..16].copy_from_slice(&response.detail.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_command_response(
    id: ExtendedId,
    payload: &[u8],
) -> Result<CommandResponse, CodecError> {
    let header = Header::decode(id)?;
    if header.class != MessageClass::Response {
        return Err(CodecError::UnexpectedClass(header.class as u8));
    }
    if header.opcode != response_opcode::COMMAND_RESPONSE {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_operational_node(header.node)?;
    require_host_requester(header.requester)?;
    require_priority(header.priority, 2)?;
    require_target(header.target, BOARD_TARGET)?;
    require_len(payload, 16)?;
    require_zero(payload, 11)?;
    Ok(CommandResponse {
        node: header.node,
        requester: header.requester,
        request_id: decode_request_id(payload)?,
        request_opcode: payload[8],
        request_target: payload[9],
        result: CommandResult::try_from(payload[10])?,
        detail: read_u32(payload, 12),
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PortStateFlags(u16);

impl PortStateFlags {
    pub const ENABLED: u16 = 1 << 0;
    pub const MODULE_PRESENT: u16 = 1 << 1;
    pub const POLICY_PENDING: u16 = 1 << 2;
    pub const EMERGENCY_LATCHED: u16 = 1 << 3;
    pub const USB_CONNECTED: u16 = 1 << 4;
    pub const CONTRACT_VALID: u16 = 1 << 5;
    /// Firmware has successfully commanded the module input on. This is not
    /// physical rail readback. On boards without a controllable input switch it
    /// is true for every supported port.
    pub const INPUT_POWERED: u16 = 1 << 6;
    const KNOWN_MASK: u16 = Self::ENABLED
        | Self::MODULE_PRESENT
        | Self::POLICY_PENDING
        | Self::EMERGENCY_LATCHED
        | Self::USB_CONNECTED
        | Self::CONTRACT_VALID
        | Self::INPUT_POWERED;

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortState {
    pub node: u8,
    pub port: PortId,
    pub sequence: u16,
    pub flags: PortStateFlags,
    pub faults: FaultFlags,
    pub uptime_seconds: u32,
    /// `0xff` means no active PDO/profile is known.
    pub active_profile: u8,
    pub slot_generation: u8,
}

pub fn encode_port_state(state: PortState) -> Result<WireFrame, CodecError> {
    require_operational_node(state.node)?;
    require_port_state_flags(state.flags.bits())?;
    let header = Header::new(
        3,
        MessageClass::State,
        state.node,
        state.port.get(),
        RequesterId::PROTOCOL,
        state_opcode::PORT_STATE,
    )?;
    let mut payload = [0u8; 16];
    payload[..2].copy_from_slice(&state.sequence.to_le_bytes());
    payload[2..4].copy_from_slice(&state.flags.bits().to_le_bytes());
    payload[4..8].copy_from_slice(&state.faults.bits().to_le_bytes());
    payload[8..12].copy_from_slice(&state.uptime_seconds.to_le_bytes());
    payload[12] = state.active_profile;
    payload[13] = state.slot_generation;
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_port_state(id: ExtendedId, payload: &[u8]) -> Result<PortState, CodecError> {
    let header = Header::decode(id)?;
    if header.class != MessageClass::State {
        return Err(CodecError::UnexpectedClass(header.class as u8));
    }
    if header.opcode != state_opcode::PORT_STATE {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_priority(header.priority, 3)?;
    require_operational_node(header.node)?;
    if header.requester != RequesterId::PROTOCOL {
        return Err(CodecError::InvalidHeader(InvalidHeader::Requester(
            header.requester.get(),
        )));
    }
    require_len(payload, 16)?;
    require_zero_range(payload, 14, 16)?;
    let flags = read_u16(payload, 2);
    require_port_state_flags(flags)?;
    let port = PortId::new(header.target).map_err(|error| PayloadError::InvalidPortId(error.0))?;
    Ok(PortState {
        node: header.node,
        port,
        sequence: read_u16(payload, 0),
        flags: PortStateFlags::from_bits(flags),
        faults: FaultFlags::from_bits(read_u32(payload, 4)),
        uptime_seconds: read_u32(payload, 8),
        active_profile: payload[12],
        slot_generation: payload[13],
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortPower {
    pub node: u8,
    pub port: PortId,
    pub sequence: u16,
    pub voltage_mv: u16,
    pub current_ma: u16,
    pub power_mw: u32,
    pub temperature_centi_c: i16,
    pub contract_voltage_mv: u16,
    pub contract_current_ma: u16,
}

pub fn encode_port_power(power: PortPower) -> Result<WireFrame, CodecError> {
    require_operational_node(power.node)?;
    let header = Header::new(
        4,
        MessageClass::Telemetry,
        power.node,
        power.port.get(),
        RequesterId::PROTOCOL,
        telemetry_opcode::PORT_POWER,
    )?;
    let mut payload = [0u8; 16];
    payload[..2].copy_from_slice(&power.sequence.to_le_bytes());
    payload[2..4].copy_from_slice(&power.voltage_mv.to_le_bytes());
    payload[4..6].copy_from_slice(&power.current_ma.to_le_bytes());
    payload[6..10].copy_from_slice(&power.power_mw.to_le_bytes());
    payload[10..12].copy_from_slice(&power.temperature_centi_c.to_le_bytes());
    payload[12..14].copy_from_slice(&power.contract_voltage_mv.to_le_bytes());
    payload[14..16].copy_from_slice(&power.contract_current_ma.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_port_power(id: ExtendedId, payload: &[u8]) -> Result<PortPower, CodecError> {
    let header = Header::decode(id)?;
    if header.class != MessageClass::Telemetry {
        return Err(CodecError::UnexpectedClass(header.class as u8));
    }
    if header.opcode != telemetry_opcode::PORT_POWER {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_priority(header.priority, 4)?;
    require_operational_node(header.node)?;
    if header.requester != RequesterId::PROTOCOL {
        return Err(CodecError::InvalidHeader(InvalidHeader::Requester(
            header.requester.get(),
        )));
    }
    require_len(payload, 16)?;
    let port = PortId::new(header.target).map_err(|error| PayloadError::InvalidPortId(error.0))?;
    Ok(PortPower {
        node: header.node,
        port,
        sequence: read_u16(payload, 0),
        voltage_mv: read_u16(payload, 2),
        current_ma: read_u16(payload, 4),
        power_mw: read_u32(payload, 6),
        temperature_centi_c: read_i16(payload, 10),
        contract_voltage_mv: read_u16(payload, 12),
        contract_current_ma: read_u16(payload, 14),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Heartbeat {
    pub node: u8,
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub firmware: FirmwareVersion,
    pub hardware_revision: u8,
    pub health_flags: u8,
    pub supported_ports: u8,
    pub online_ports: u8,
    pub enabled_ports: u8,
    pub faulted_ports: u8,
    pub emergency_latched: bool,
    pub commissioning_state: CommissioningState,
    pub reset_flags: ResetFlags,
    pub uptime_seconds: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResetFlags(u8);

impl ResetFlags {
    pub const OPTION_BYTE: u8 = 1 << 0;
    pub const PIN: u8 = 1 << 1;
    pub const POWER: u8 = 1 << 2;
    pub const SOFTWARE: u8 = 1 << 3;
    pub const INDEPENDENT_WATCHDOG: u8 = 1 << 4;
    pub const WINDOW_WATCHDOG: u8 = 1 << 5;
    pub const LOW_POWER: u8 = 1 << 6;
    const KNOWN_MASK: u8 = 0x7f;

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }
}

pub fn encode_heartbeat(heartbeat: Heartbeat) -> Result<WireFrame, CodecError> {
    require_operational_node(heartbeat.node)?;
    let header = Header::new(
        5,
        MessageClass::Management,
        heartbeat.node,
        BOARD_TARGET,
        RequesterId::PROTOCOL,
        management_opcode::HEARTBEAT,
    )?;
    let mut payload = [0u8; 24];
    payload[0] = heartbeat.protocol_major;
    payload[1] = heartbeat.protocol_minor;
    payload[2..4].copy_from_slice(&heartbeat.firmware.major.to_le_bytes());
    payload[4..6].copy_from_slice(&heartbeat.firmware.minor.to_le_bytes());
    payload[6..8].copy_from_slice(&heartbeat.firmware.patch.to_le_bytes());
    payload[8] = heartbeat.hardware_revision;
    payload[9] = heartbeat.health_flags;
    payload[10] = heartbeat.supported_ports;
    payload[11] = heartbeat.online_ports;
    payload[12] = heartbeat.enabled_ports;
    payload[13] = heartbeat.faulted_ports;
    payload[14] = u8::from(heartbeat.emergency_latched);
    payload[15] = heartbeat.commissioning_state as u8;
    payload[16] = heartbeat.reset_flags.bits();
    payload[20..24].copy_from_slice(&heartbeat.uptime_seconds.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_heartbeat(id: ExtendedId, payload: &[u8]) -> Result<Heartbeat, CodecError> {
    let header = Header::decode(id)?;
    if header.class != MessageClass::Management {
        return Err(CodecError::UnexpectedClass(header.class as u8));
    }
    if header.opcode != management_opcode::HEARTBEAT {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_priority(header.priority, 5)?;
    require_operational_node(header.node)?;
    require_protocol_requester(header.requester)?;
    require_target(header.target, BOARD_TARGET)?;
    require_len(payload, 24)?;
    require_zero_range(payload, 17, 20)?;
    let emergency_latched = match payload[14] {
        0 => false,
        1 => true,
        value => return Err(PayloadError::InvalidBoolean { offset: 14, value }.into()),
    };
    if payload[16] & !ResetFlags::KNOWN_MASK != 0 {
        return Err(PayloadError::InvalidValue {
            offset: 16,
            value: payload[16],
        }
        .into());
    }
    Ok(Heartbeat {
        node: header.node,
        protocol_major: payload[0],
        protocol_minor: payload[1],
        firmware: FirmwareVersion {
            major: read_u16(payload, 2),
            minor: read_u16(payload, 4),
            patch: read_u16(payload, 6),
        },
        hardware_revision: payload[8],
        health_flags: payload[9],
        supported_ports: payload[10],
        online_ports: payload[11],
        enabled_ports: payload[12],
        faulted_ports: payload[13],
        emergency_latched,
        commissioning_state: decode_commissioning_state(payload[15], 15)?,
        reset_flags: ResetFlags::from_bits(payload[16]),
        uptime_seconds: read_u32(payload, 20),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardInfo {
    pub node: u8,
    pub uid: NodeUid,
    pub commissioning_state: CommissioningState,
    pub hardware_revision: u8,
    pub supported_ports: u8,
    pub firmware: FirmwareVersion,
    pub protocol_major: u8,
    pub protocol_minor: u8,
}

pub fn encode_board_info(info: BoardInfo) -> Result<WireFrame, CodecError> {
    require_operational_node(info.node)?;
    let header = Header::new(
        5,
        MessageClass::Management,
        info.node,
        BOARD_TARGET,
        RequesterId::PROTOCOL,
        management_opcode::BOARD_INFO,
    )?;
    let mut payload = [0u8; 24];
    payload[..12].copy_from_slice(info.uid.as_bytes());
    payload[12] = info.node;
    payload[13] = info.commissioning_state as u8;
    payload[14] = info.hardware_revision;
    payload[15] = info.supported_ports;
    payload[16..18].copy_from_slice(&info.firmware.major.to_le_bytes());
    payload[18..20].copy_from_slice(&info.firmware.minor.to_le_bytes());
    payload[20..22].copy_from_slice(&info.firmware.patch.to_le_bytes());
    payload[22] = info.protocol_major;
    payload[23] = info.protocol_minor;
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_board_info(id: ExtendedId, payload: &[u8]) -> Result<BoardInfo, CodecError> {
    let header = Header::decode(id)?;
    if header.class != MessageClass::Management {
        return Err(CodecError::UnexpectedClass(header.class as u8));
    }
    if header.opcode != management_opcode::BOARD_INFO {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_priority(header.priority, 5)?;
    require_operational_node(header.node)?;
    require_protocol_requester(header.requester)?;
    require_target(header.target, BOARD_TARGET)?;
    require_len(payload, 24)?;
    if payload[12] != header.node {
        return Err(PayloadError::InvalidNodeId(payload[12]).into());
    }
    Ok(BoardInfo {
        node: header.node,
        uid: read_uid(payload, 0),
        commissioning_state: decode_commissioning_state(payload[13], 13)?,
        hardware_revision: payload[14],
        supported_ports: payload[15],
        firmware: FirmwareVersion {
            major: read_u16(payload, 16),
            minor: read_u16(payload, 18),
            patch: read_u16(payload, 20),
        },
        protocol_major: payload[22],
        protocol_minor: payload[23],
    })
}

/// Commissioning reuses the operational `node | target` bits as a 12-bit
/// UID-derived arbitration token. The full UID in the payload is always the
/// authority; the token only prevents ordinary small-node response collisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommissioningHeader {
    pub priority: u8,
    pub token: u16,
    pub requester: RequesterId,
    pub opcode: u8,
}

impl CommissioningHeader {
    pub const MAX_TOKEN: u16 = 0x0FFF;

    pub const fn new(
        priority: u8,
        token: u16,
        requester: RequesterId,
        opcode: u8,
    ) -> Result<Self, CodecError> {
        if priority > MAX_PRIORITY {
            return Err(CodecError::InvalidHeader(InvalidHeader::Priority(priority)));
        }
        if token > Self::MAX_TOKEN {
            return Err(CodecError::InvalidCommissioningToken(token));
        }
        if opcode > MAX_OPCODE {
            return Err(CodecError::InvalidHeader(InvalidHeader::Opcode(opcode)));
        }
        Ok(Self {
            priority,
            token,
            requester,
            opcode,
        })
    }

    pub const fn encode(self) -> ExtendedId {
        ExtendedId(
            (self.priority as u32) << PRIORITY_SHIFT
                | (MessageClass::Commissioning as u32) << CLASS_SHIFT
                | (self.token as u32) << TARGET_SHIFT
                | (self.requester.get() as u32) << REQUESTER_SHIFT
                | self.opcode as u32,
        )
    }

    pub fn decode(id: ExtendedId) -> Result<Self, CodecError> {
        let raw = id.get();
        let class = ((raw >> CLASS_SHIFT) & CLASS_MASK) as u8;
        if class != MessageClass::Commissioning as u8 {
            return Err(CodecError::UnexpectedClass(class));
        }
        let priority = ((raw >> PRIORITY_SHIFT) & PRIORITY_MASK) as u8;
        let token = ((raw >> TARGET_SHIFT) & 0x0FFF) as u16;
        let requester_raw = ((raw >> REQUESTER_SHIFT) & REQUESTER_MASK) as u8;
        let requester =
            RequesterId::new(requester_raw).map_err(|error| InvalidHeader::Requester(error.0))?;
        let opcode = (raw & OPCODE_MASK) as u8;
        Self::new(priority, token, requester, opcode)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedHeader {
    Operational(Header),
    Commissioning(CommissioningHeader),
}

pub fn decode_header(id: ExtendedId) -> Result<DecodedHeader, CodecError> {
    let class = ((id.get() >> CLASS_SHIFT) & CLASS_MASK) as u8;
    if class == MessageClass::Commissioning as u8 {
        CommissioningHeader::decode(id).map(DecodedHeader::Commissioning)
    } else {
        Header::decode(id)
            .map(DecodedHeader::Operational)
            .map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscoveryInfo {
    pub uid: NodeUid,
    pub node_id: Option<NodeId>,
    pub state: CommissioningState,
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub firmware: FirmwareVersion,
    /// Raw revision is retained so an older CLI can still report unknown
    /// hardware instead of rejecting the discovery response.
    pub hardware_revision: u8,
    pub supported_ports: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommissioningMessage {
    NodeClaim {
        uid: NodeUid,
        node_id: NodeId,
        state: CommissioningState,
        claim_nonce: u16,
    },
    Discover {
        nonce: u64,
    },
    DiscoveryResponse {
        nonce: u64,
        info: DiscoveryInfo,
    },
    Identify {
        request_id: RequestId,
        uid: NodeUid,
        duration_seconds: u16,
    },
    AssignNode {
        request_id: RequestId,
        uid: NodeUid,
        node_id: NodeId,
    },
    ClearNode {
        request_id: RequestId,
        uid: NodeUid,
    },
    Result {
        request_id: RequestId,
        uid: NodeUid,
        request_opcode: u8,
        result: CommandResult,
        node_id: Option<NodeId>,
        state: CommissioningState,
    },
}

pub fn encode_commissioning(
    requester: RequesterId,
    message: CommissioningMessage,
) -> Result<WireFrame, CodecError> {
    require_commissioning_requester(requester, message)?;
    let mut payload = [0u8; 32];
    let (opcode, token, len) = encode_commissioning_payload(message, &mut payload)?;
    let header = CommissioningHeader::new(6, token, requester, opcode)?;
    WireFrame::new(header.encode(), &payload[..len]).map_err(Into::into)
}

fn encode_commissioning_payload(
    message: CommissioningMessage,
    payload: &mut [u8; 32],
) -> Result<(u8, u16, usize), CodecError> {
    let fields = match message {
        CommissioningMessage::NodeClaim {
            uid,
            node_id,
            state,
            claim_nonce,
        } => {
            payload[..12].copy_from_slice(uid.as_bytes());
            payload[12] = node_id.get();
            payload[13] = state as u8;
            payload[14..16].copy_from_slice(&claim_nonce.to_le_bytes());
            (
                commissioning_opcode::NODE_CLAIM,
                node_claim_token(uid, node_id, claim_nonce),
                16,
            )
        }
        CommissioningMessage::Discover { nonce } => {
            payload[..8].copy_from_slice(&nonce.to_le_bytes());
            (commissioning_opcode::DISCOVER, 0, 8)
        }
        CommissioningMessage::DiscoveryResponse { nonce, info } => {
            payload[..12].copy_from_slice(info.uid.as_bytes());
            payload[12] = info.node_id.map_or(0, NodeId::get);
            payload[13] = info.state as u8;
            payload[14] = info.protocol_major;
            payload[15] = info.protocol_minor;
            payload[16..18].copy_from_slice(&info.firmware.major.to_le_bytes());
            payload[18..20].copy_from_slice(&info.firmware.minor.to_le_bytes());
            payload[20..22].copy_from_slice(&info.firmware.patch.to_le_bytes());
            payload[22] = info.hardware_revision;
            payload[23] = info.supported_ports;
            payload[24..32].copy_from_slice(&nonce.to_le_bytes());
            (
                commissioning_opcode::DISCOVERY_RESPONSE,
                commissioning_token(info.uid, nonce),
                32,
            )
        }
        CommissioningMessage::Identify {
            request_id,
            uid,
            duration_seconds,
        } => {
            encode_request_id(request_id, payload)?;
            payload[8..20].copy_from_slice(uid.as_bytes());
            payload[20..22].copy_from_slice(&duration_seconds.to_le_bytes());
            (
                commissioning_opcode::IDENTIFY,
                commissioning_token(uid, request_id.0),
                24,
            )
        }
        CommissioningMessage::AssignNode {
            request_id,
            uid,
            node_id,
        } => {
            encode_request_id(request_id, payload)?;
            payload[8..20].copy_from_slice(uid.as_bytes());
            payload[20] = node_id.get();
            (
                commissioning_opcode::ASSIGN_NODE,
                commissioning_token(uid, request_id.0),
                24,
            )
        }
        CommissioningMessage::ClearNode { request_id, uid } => {
            encode_request_id(request_id, payload)?;
            payload[8..20].copy_from_slice(uid.as_bytes());
            (
                commissioning_opcode::CLEAR_NODE,
                commissioning_token(uid, request_id.0),
                20,
            )
        }
        CommissioningMessage::Result {
            request_id,
            uid,
            request_opcode,
            result,
            node_id,
            state,
        } => {
            encode_request_id(request_id, payload)?;
            payload[8..20].copy_from_slice(uid.as_bytes());
            payload[20] = request_opcode;
            payload[21] = result as u8;
            payload[22] = node_id.map_or(0, NodeId::get);
            payload[23] = state as u8;
            (
                commissioning_opcode::RESULT,
                commissioning_token(uid, request_id.0),
                24,
            )
        }
    };
    Ok(fields)
}

pub fn decode_commissioning(
    id: ExtendedId,
    payload: &[u8],
) -> Result<(CommissioningHeader, CommissioningMessage), CodecError> {
    let header = CommissioningHeader::decode(id)?;
    if header.priority != 6 {
        return Err(CodecError::InvalidHeader(InvalidHeader::Priority(
            header.priority,
        )));
    }
    let message = decode_commissioning_message(header, payload)?;
    Ok((header, message))
}

fn decode_commissioning_message(
    header: CommissioningHeader,
    payload: &[u8],
) -> Result<CommissioningMessage, CodecError> {
    if header.opcode == commissioning_opcode::NODE_CLAIM {
        require_protocol_requester(header.requester)?;
    } else {
        require_host_requester(header.requester)?;
    }
    let message = match header.opcode {
        commissioning_opcode::NODE_CLAIM => {
            require_len(payload, 16)?;
            let uid = read_uid(payload, 0);
            let node_id = decode_node(payload[12])?.ok_or(PayloadError::InvalidNodeId(0))?;
            let state = decode_commissioning_state(payload[13], 13)?;
            let claim_nonce = read_u16(payload, 14);
            let expected = node_claim_token(uid, node_id, claim_nonce);
            require_token(header.token, expected)?;
            CommissioningMessage::NodeClaim {
                uid,
                node_id,
                state,
                claim_nonce,
            }
        }
        commissioning_opcode::DISCOVER => {
            require_len(payload, 8)?;
            require_token(header.token, 0)?;
            CommissioningMessage::Discover {
                nonce: read_u64(payload, 0),
            }
        }
        commissioning_opcode::DISCOVERY_RESPONSE => {
            require_len(payload, 32)?;
            let uid = read_uid(payload, 0);
            let nonce = read_u64(payload, 24);
            require_token(header.token, commissioning_token(uid, nonce))?;
            CommissioningMessage::DiscoveryResponse {
                nonce,
                info: DiscoveryInfo {
                    uid,
                    node_id: decode_node(payload[12])?,
                    state: decode_commissioning_state(payload[13], 13)?,
                    protocol_major: payload[14],
                    protocol_minor: payload[15],
                    firmware: FirmwareVersion {
                        major: read_u16(payload, 16),
                        minor: read_u16(payload, 18),
                        patch: read_u16(payload, 20),
                    },
                    hardware_revision: payload[22],
                    supported_ports: payload[23],
                },
            }
        }
        commissioning_opcode::IDENTIFY => {
            require_len(payload, 24)?;
            require_zero_range(payload, 22, 24)?;
            let request_id = decode_request_id(payload)?;
            let uid = read_uid(payload, 8);
            require_token(header.token, commissioning_token(uid, request_id.0))?;
            CommissioningMessage::Identify {
                request_id,
                uid,
                duration_seconds: read_u16(payload, 20),
            }
        }
        commissioning_opcode::ASSIGN_NODE => {
            require_len(payload, 24)?;
            require_zero_range(payload, 21, 24)?;
            let request_id = decode_request_id(payload)?;
            let uid = read_uid(payload, 8);
            let node_id = decode_node(payload[20])?.ok_or(PayloadError::InvalidNodeId(0))?;
            require_token(header.token, commissioning_token(uid, request_id.0))?;
            CommissioningMessage::AssignNode {
                request_id,
                uid,
                node_id,
            }
        }
        commissioning_opcode::CLEAR_NODE => {
            require_len(payload, 20)?;
            let request_id = decode_request_id(payload)?;
            let uid = read_uid(payload, 8);
            require_token(header.token, commissioning_token(uid, request_id.0))?;
            CommissioningMessage::ClearNode { request_id, uid }
        }
        commissioning_opcode::RESULT => {
            require_len(payload, 24)?;
            let request_id = decode_request_id(payload)?;
            let uid = read_uid(payload, 8);
            require_token(header.token, commissioning_token(uid, request_id.0))?;
            CommissioningMessage::Result {
                request_id,
                uid,
                request_opcode: payload[20],
                result: CommandResult::try_from(payload[21])?,
                node_id: decode_node(payload[22])?,
                state: decode_commissioning_state(payload[23], 23)?,
            }
        }
        opcode => return Err(CodecError::UnknownOpcode(opcode)),
    };
    Ok(message)
}

pub fn commissioning_token(uid: NodeUid, nonce: u64) -> u16 {
    let nonce = nonce.to_le_bytes();
    let nonce_low = u32::from_le_bytes([nonce[0], nonce[1], nonce[2], nonce[3]]);
    let nonce_high = u32::from_le_bytes([nonce[4], nonce[5], nonce[6], nonce[7]]);
    let crc = crc32_chunks(uid.as_bytes(), &[]);
    let mixed = avalanche32(crc ^ nonce_low ^ nonce_high.rotate_left(13));
    u16::try_from(mixed & u32::from(CommissioningHeader::MAX_TOKEN))
        .expect("the commissioning mask fits u16")
}

pub fn node_claim_token(uid: NodeUid, node_id: NodeId, claim_nonce: u16) -> u16 {
    const CLAIM_DOMAIN: u64 = 0x5044_4341_4e43_4c4d;
    let nonce = CLAIM_DOMAIN ^ u64::from(node_id.get()) << 16 ^ u64::from(claim_nonce);
    commissioning_token(uid, nonce)
}

fn avalanche32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x85eb_ca6b);
    value ^= value >> 13;
    value = value.wrapping_mul(0xc2b2_ae35);
    value ^ (value >> 16)
}

fn require_len(payload: &[u8], expected: u8) -> Result<(), PayloadError> {
    let actual = u8::try_from(payload.len()).unwrap_or(u8::MAX);
    if actual == expected {
        Ok(())
    } else {
        Err(PayloadError::WrongLength { expected, actual })
    }
}

fn require_target(actual: u8, expected: u8) -> Result<(), CodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CodecError::WrongTarget(actual))
    }
}

fn require_priority(actual: u8, expected: u8) -> Result<(), CodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CodecError::InvalidHeader(InvalidHeader::Priority(actual)))
    }
}

fn require_protocol_requester(requester: RequesterId) -> Result<(), CodecError> {
    if requester == RequesterId::PROTOCOL {
        Ok(())
    } else {
        Err(CodecError::InvalidHeader(InvalidHeader::Requester(
            requester.get(),
        )))
    }
}

fn require_commissioning_requester(
    requester: RequesterId,
    message: CommissioningMessage,
) -> Result<(), CodecError> {
    if matches!(message, CommissioningMessage::NodeClaim { .. }) {
        require_protocol_requester(requester)
    } else {
        require_host_requester(requester)
    }
}

fn require_host_requester(requester: RequesterId) -> Result<(), CodecError> {
    if requester.is_protocol_reserved() {
        Err(CodecError::InvalidHeader(InvalidHeader::Requester(
            requester.get(),
        )))
    } else {
        Ok(())
    }
}

fn require_operational_node(node: u8) -> Result<(), CodecError> {
    NodeId::new(node)
        .map(|_| ())
        .map_err(|error| CodecError::InvalidHeader(InvalidHeader::Node(error.0)))
}

fn require_port_state_flags(flags: u16) -> Result<(), CodecError> {
    let unknown = flags & !PortStateFlags::KNOWN_MASK;
    if unknown == 0 {
        return Ok(());
    }
    let bytes = unknown.to_le_bytes();
    let (offset, value) = if bytes[0] == 0 {
        (3, bytes[1])
    } else {
        (2, bytes[0])
    };
    Err(PayloadError::InvalidValue { offset, value }.into())
}

fn require_token(actual: u16, expected: u16) -> Result<(), CodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CodecError::InvalidCommissioningToken(actual))
    }
}

fn require_zero(payload: &[u8], offset: usize) -> Result<(), PayloadError> {
    if payload[offset] == 0 {
        Ok(())
    } else {
        Err(PayloadError::ReservedNonZero {
            offset: u8::try_from(offset).unwrap_or(u8::MAX),
        })
    }
}

fn require_zero_range(payload: &[u8], start: usize, end: usize) -> Result<(), PayloadError> {
    for offset in start..end {
        require_zero(payload, offset)?;
    }
    Ok(())
}

fn decode_node(raw: u8) -> Result<Option<NodeId>, PayloadError> {
    if raw == 0 {
        Ok(None)
    } else {
        NodeId::new(raw)
            .map(Some)
            .map_err(|error| PayloadError::InvalidNodeId(error.0))
    }
}

fn decode_commissioning_state(raw: u8, offset: u8) -> Result<CommissioningState, PayloadError> {
    match raw {
        0 => Ok(CommissioningState::Uncommissioned),
        1 => Ok(CommissioningState::Claiming),
        2 => Ok(CommissioningState::Commissioned),
        3 => Ok(CommissioningState::AddressConflict),
        value => Err(PayloadError::InvalidValue { offset, value }),
    }
}

fn read_uid(bytes: &[u8], offset: usize) -> NodeUid {
    let mut uid = [0; NodeUid::LENGTH];
    uid.copy_from_slice(&bytes[offset..offset + NodeUid::LENGTH]);
    NodeUid::from_bytes(uid)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_i16(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn crc32_chunks(first: &[u8], second: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in first.iter().chain(second) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_header_vector_includes_requester_id() {
        let header = Header::new(
            1,
            MessageClass::Control,
            3,
            4,
            RequesterId::PDCAN_DEFAULT,
            2,
        )
        .unwrap();
        let encoded = header.encode();
        assert_eq!(encoded.get(), 0x0400_D3C2);
        assert_eq!(Header::decode(encoded), Ok(header));
    }

    #[test]
    fn invalid_node_and_opcode_are_rejected() {
        assert!(matches!(
            Header::new(
                1,
                MessageClass::Control,
                INVALID_NODE,
                0,
                RequesterId::PDCAN_DEFAULT,
                0,
            ),
            Err(InvalidHeader::Node(INVALID_NODE))
        ));
        assert!(matches!(
            Header::new(
                1,
                MessageClass::Control,
                1,
                0,
                RequesterId::PDCAN_DEFAULT,
                64,
            ),
            Err(InvalidHeader::Opcode(64))
        ));
    }

    #[test]
    fn request_id_is_explicit_little_endian() {
        let mut payload = [0u8; 16];
        let request_id = RequestId(0x0102_0304_0506_0708);
        encode_request_id(request_id, &mut payload).unwrap();
        assert_eq!(&payload[..8], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(decode_request_id(&payload), Ok(request_id));
    }

    #[test]
    fn every_allocated_message_fits_header_and_can_fd_payload() {
        for definition in MESSAGE_DEFINITIONS {
            assert!(definition.opcode <= MAX_OPCODE);
            assert!(definition.priority <= MAX_PRIORITY);
            assert!(matches!(
                definition.payload_len,
                0..=8 | 12 | 16 | 20 | 24 | 32 | 48 | 64
            ));
        }
        assert_eq!(
            WireFrame::new(ExtendedId::new(1).unwrap(), &[0; 9]),
            Err(FrameError::InvalidCanFdLength(9))
        );
    }

    #[test]
    fn set_policy_golden_vector_is_strict_little_endian() {
        let policy = PortPolicy {
            enabled: true,
            max_voltage_mv: 20_000,
            max_current_ma: 5_000,
            max_power_mw: 100_000,
        };
        let request = ControlRequest {
            node: 3,
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(0x0102_0304_0506_0708),
            command: ControlCommand::SetPortPolicy {
                port: PortId::new(2).unwrap(),
                policy,
            },
        };
        let frame = encode_control_request(request).unwrap();
        assert_eq!(frame.id().get(), 0x0400_CBC1);
        assert_eq!(
            frame.payload(),
            &[
                0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x01, 0x00, 0x20, 0x4e, 0x88, 0x13,
                0x00, 0x00, 0xa0, 0x86, 0x01, 0x00,
            ]
        );
        assert_eq!(
            decode_control_request(frame.id(), frame.payload()),
            Ok(request)
        );
    }

    #[test]
    fn policy_decoder_rejects_reserved_bits_and_non_exact_lengths() {
        let request = ControlRequest {
            node: 1,
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(1),
            command: ControlCommand::SetPortPolicy {
                port: PortId::new(0).unwrap(),
                policy: PortPolicy::SAFE_DISABLED,
            },
        };
        let frame = encode_control_request(request).unwrap();
        let mut payload = [0; 20];
        payload.copy_from_slice(frame.payload());
        payload[14] = 1;
        assert_eq!(
            decode_control_request(frame.id(), &payload),
            Err(CodecError::Payload(PayloadError::ReservedNonZero {
                offset: 14
            }))
        );
        assert_eq!(
            decode_control_request(frame.id(), &payload[..19]),
            Err(CodecError::Payload(PayloadError::WrongLength {
                expected: 20,
                actual: 19
            }))
        );
    }

    #[test]
    fn command_response_round_trip_preserves_requester_and_request_id() {
        let response = CommandResponse {
            node: 9,
            requester: RequesterId::new(4).unwrap(),
            request_id: RequestId(u64::MAX - 4),
            request_opcode: control_opcode::SET_PORT_POLICY,
            request_target: 7,
            result: CommandResult::OkPending,
            detail: 0xAABB_CCDD,
        };
        let frame = encode_command_response(response).unwrap();
        assert_eq!(
            decode_command_response(frame.id(), frame.payload()),
            Ok(response)
        );
    }

    #[test]
    fn commissioning_token_has_a_golden_crc_vector() {
        let uid = NodeUid::from_bytes([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(commissioning_token(uid, 0x0102_0304_0506_0708), 0x387);
        assert_eq!(node_claim_token(uid, NodeId::new(7).unwrap(), 0), 0xe2b);
    }

    #[test]
    fn discovery_response_golden_id_and_round_trip() {
        let uid = NodeUid::from_bytes([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        let message = CommissioningMessage::DiscoveryResponse {
            nonce: 0x0102_0304_0506_0708,
            info: DiscoveryInfo {
                uid,
                node_id: Some(NodeId::new(3).unwrap()),
                state: CommissioningState::Commissioned,
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                firmware: FirmwareVersion {
                    major: 1,
                    minor: 2,
                    patch: 513,
                },
                hardware_revision: 1,
                supported_ports: 0x3f,
            },
        };
        let frame = encode_commissioning(RequesterId::PDCAN_DEFAULT, message).unwrap();
        assert_eq!(frame.id().get(), 0x1B8E_1FC2);
        assert_eq!(
            decode_commissioning(frame.id(), frame.payload()),
            Ok((
                CommissioningHeader::new(
                    6,
                    0x387,
                    RequesterId::PDCAN_DEFAULT,
                    commissioning_opcode::DISCOVERY_RESPONSE,
                )
                .unwrap(),
                message,
            ))
        );
    }

    #[test]
    fn every_commissioning_message_round_trips() {
        let uid = NodeUid::from_bytes([0x5a; NodeUid::LENGTH]);
        let node = NodeId::new(17).unwrap();
        let request_id = RequestId(0x1234_5678_9abc_def0);
        let messages = [
            CommissioningMessage::NodeClaim {
                uid,
                node_id: node,
                state: CommissioningState::Claiming,
                claim_nonce: 0,
            },
            CommissioningMessage::Discover { nonce: 77 },
            CommissioningMessage::DiscoveryResponse {
                nonce: 77,
                info: DiscoveryInfo {
                    uid,
                    node_id: Some(node),
                    state: CommissioningState::Commissioned,
                    protocol_major: PROTOCOL_MAJOR,
                    protocol_minor: PROTOCOL_MINOR,
                    firmware: FirmwareVersion {
                        major: 0,
                        minor: 1,
                        patch: 0,
                    },
                    hardware_revision: 1,
                    supported_ports: 0xff,
                },
            },
            CommissioningMessage::Identify {
                request_id,
                uid,
                duration_seconds: 30,
            },
            CommissioningMessage::AssignNode {
                request_id,
                uid,
                node_id: node,
            },
            CommissioningMessage::ClearNode { request_id, uid },
            CommissioningMessage::Result {
                request_id,
                uid,
                request_opcode: commissioning_opcode::ASSIGN_NODE,
                result: CommandResult::Ok,
                node_id: Some(node),
                state: CommissioningState::Claiming,
            },
        ];
        for message in messages {
            let requester = if matches!(message, CommissioningMessage::NodeClaim { .. }) {
                RequesterId::PROTOCOL
            } else {
                RequesterId::new(5).unwrap()
            };
            let frame = encode_commissioning(requester, message).unwrap();
            let (header, decoded) = decode_commissioning(frame.id(), frame.payload()).unwrap();
            assert_eq!(header.requester, requester);
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn reserved_requester_and_broadcast_scope_are_strict() {
        let request = ControlRequest {
            node: 3,
            requester: RequesterId::PROTOCOL,
            request_id: RequestId(1),
            command: ControlCommand::RequestStatus {
                port: PortId::new(0).unwrap(),
            },
        };
        assert_eq!(
            encode_control_request(request),
            Err(CodecError::InvalidHeader(InvalidHeader::Requester(0)))
        );
        assert!(matches!(
            encode_control_request(ControlRequest {
                node: BROADCAST_NODE,
                requester: RequesterId::PDCAN_DEFAULT,
                ..request
            }),
            Err(CodecError::InvalidHeader(InvalidHeader::Node(
                BROADCAST_NODE
            )))
        ));
        assert!(
            encode_control_request(ControlRequest {
                node: BROADCAST_NODE,
                requester: RequesterId::PDCAN_DEFAULT,
                command: ControlCommand::EmergencyDisable,
                ..request
            })
            .is_ok()
        );

        let uid = NodeUid::from_bytes([1; NodeUid::LENGTH]);
        assert_eq!(
            encode_commissioning(
                RequesterId::PROTOCOL,
                CommissioningMessage::Discover { nonce: 1 }
            ),
            Err(CodecError::InvalidHeader(InvalidHeader::Requester(0)))
        );
        assert_eq!(
            encode_commissioning(
                RequesterId::PDCAN_DEFAULT,
                CommissioningMessage::NodeClaim {
                    uid,
                    node_id: NodeId::new(1).unwrap(),
                    state: CommissioningState::Claiming,
                    claim_nonce: 0,
                }
            ),
            Err(CodecError::InvalidHeader(InvalidHeader::Requester(15)))
        );
    }

    #[test]
    fn corrupted_commissioning_token_is_rejected() {
        let message = CommissioningMessage::DiscoveryResponse {
            nonce: 1,
            info: DiscoveryInfo {
                uid: NodeUid::from_bytes([9; NodeUid::LENGTH]),
                node_id: None,
                state: CommissioningState::Uncommissioned,
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                firmware: FirmwareVersion {
                    major: 0,
                    minor: 1,
                    patch: 0,
                },
                hardware_revision: 1,
                supported_ports: 0x3f,
            },
        };
        let frame = encode_commissioning(RequesterId::PDCAN_DEFAULT, message).unwrap();
        let corrupted_id = ExtendedId::new(frame.id().get() ^ (1 << TARGET_SHIFT)).unwrap();
        assert!(matches!(
            decode_commissioning(corrupted_id, frame.payload()),
            Err(CodecError::InvalidCommissioningToken(_))
        ));
    }

    #[test]
    fn state_telemetry_and_management_payloads_round_trip() {
        let port = PortId::new(5).unwrap();
        let state = PortState {
            node: 3,
            port,
            sequence: 0x1234,
            flags: PortStateFlags::from_bits(
                PortStateFlags::ENABLED | PortStateFlags::MODULE_PRESENT,
            ),
            faults: FaultFlags::from_bits(0xa5a5_0001),
            uptime_seconds: 77,
            active_profile: 4,
            slot_generation: 9,
        };
        let state_frame = encode_port_state(state).unwrap();
        assert_eq!(
            decode_port_state(state_frame.id(), state_frame.payload()),
            Ok(state)
        );

        let power = PortPower {
            node: 3,
            port,
            sequence: 8,
            voltage_mv: 19_987,
            current_ma: 4_999,
            power_mw: 99_123,
            temperature_centi_c: -1_250,
            contract_voltage_mv: 20_000,
            contract_current_ma: 5_000,
        };
        let power_frame = encode_port_power(power).unwrap();
        assert_eq!(
            decode_port_power(power_frame.id(), power_frame.payload()),
            Ok(power)
        );

        let heartbeat = Heartbeat {
            node: 3,
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            firmware: FirmwareVersion {
                major: 1,
                minor: 2,
                patch: 3,
            },
            hardware_revision: 1,
            health_flags: 2,
            supported_ports: 0x3f,
            online_ports: 0x21,
            enabled_ports: 1,
            faulted_ports: 0x20,
            emergency_latched: true,
            commissioning_state: CommissioningState::Commissioned,
            reset_flags: ResetFlags::from_bits(
                ResetFlags::POWER | ResetFlags::INDEPENDENT_WATCHDOG,
            ),
            uptime_seconds: 0x0102_0304,
        };
        let heartbeat_frame = encode_heartbeat(heartbeat).unwrap();
        assert_eq!(
            decode_heartbeat(heartbeat_frame.id(), heartbeat_frame.payload()),
            Ok(heartbeat)
        );
    }

    #[test]
    fn board_info_retains_versions_and_full_uid() {
        let info = BoardInfo {
            node: 17,
            uid: NodeUid::from_bytes([0x77; NodeUid::LENGTH]),
            commissioning_state: CommissioningState::Commissioned,
            hardware_revision: 1,
            supported_ports: 0x3f,
            firmware: FirmwareVersion {
                major: 300,
                minor: 2,
                patch: 999,
            },
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
        };
        let frame = encode_board_info(info).unwrap();
        assert_eq!(decode_board_info(frame.id(), frame.payload()), Ok(info));
    }

    #[test]
    fn rotating_discovery_nonce_separates_a_real_twelve_bit_collision() {
        let mut by_token = std::vec![None; 4096];
        let mut collision = None;
        for suffix in 0..=u16::MAX {
            let suffix_bytes = suffix.to_le_bytes();
            let uid = NodeUid::from_bytes([
                0,
                1,
                2,
                3,
                4,
                5,
                6,
                7,
                8,
                9,
                suffix_bytes[0],
                suffix_bytes[1],
            ]);
            let token = usize::from(commissioning_token(uid, 1));
            if let Some(other) = by_token[token]
                && commissioning_token(other, 2) != commissioning_token(uid, 2)
            {
                collision = Some((other, uid));
                break;
            }
            by_token[token] = Some(uid);
        }
        let (first, second) =
            collision.expect("pigeonhole search finds a nonce-rotatable collision");
        assert_eq!(
            commissioning_token(first, 1),
            commissioning_token(second, 1)
        );
        assert_ne!(
            commissioning_token(first, 2),
            commissioning_token(second, 2)
        );
    }
}
