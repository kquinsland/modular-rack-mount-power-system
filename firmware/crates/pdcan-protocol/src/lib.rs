#![no_std]

#[cfg(test)]
extern crate std;

use pdcan_types::{
    CapabilityFlags, CarrierBinding, CarrierPolicy, CarrierProfile, CommissioningState, FanConfig,
    FanId, FaultFlags, FirmwareVersion, HardwareRevision, ImageTarget, NodeDescriptor, NodeId,
    NodeRole, NodeUid, PdLimits, PowerReading, PowerSource, RequestId, RequesterId, SampleStatus,
    ServiceState, Sha256Digest, SlotIndex, TemperatureReading, TemperatureSensorId,
    TemperatureSource, UpdateError, UpdateImpact, UpdateManifest, UpdateSessionId, UpdateState,
};

pub const EXTENDED_ID_MASK: u32 = 0x1FFF_FFFF;
pub const BROADCAST_NODE: u8 = 0;
pub const INVALID_NODE: u8 = 0xFF;
pub const NODE_TARGET: u8 = 0x0F;
pub const PROTOCOL_MAJOR: u8 = 1;
pub const PROTOCOL_MINOR: u8 = 0;
pub const FIRMWARE_CHUNK_BYTES: usize = 48;
pub const FIRMWARE_WINDOW_FRAMES: u8 = 8;

pub mod control_opcode {
    pub const EMERGENCY_DISABLE: u8 = 0;
    pub const SET_CARRIER_POLICY: u8 = 1;
    pub const ACKNOWLEDGE_EMERGENCY: u8 = 2;
    pub const REQUEST_STATUS: u8 = 3;
    pub const SET_FAN: u8 = 4;
    pub const SET_BINDING: u8 = 5;
}

pub mod response_opcode {
    pub const COMMAND: u8 = 0;
}

pub mod state_opcode {
    pub const NODE: u8 = 0;
    pub const FAN: u8 = 1;
}

pub mod telemetry_opcode {
    pub const POWER: u8 = 0;
    pub const TEMPERATURE: u8 = 1;
}

pub mod management_opcode {
    pub const HEARTBEAT: u8 = 0;
    pub const NODE_INFO: u8 = 1;
    pub const BINDING: u8 = 2;
}

pub mod firmware_opcode {
    pub const BEGIN: u8 = 0;
    pub const DATA: u8 = 1;
    pub const FINISH: u8 = 2;
    pub const ACTIVATE: u8 = 3;
    pub const ABORT: u8 = 4;
    pub const STATUS: u8 = 5;
    pub const ACK: u8 = 6;
    pub const STATUS_RESPONSE: u8 = 7;
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
    Firmware = 0x6,
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
            0x6 => Ok(Self::Firmware),
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
    Node,
    Fan,
    Commissioning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageSender {
    Host,
    Node,
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

#[allow(clippy::too_many_arguments)]
const fn message(
    name: &'static str,
    class: MessageClass,
    opcode: u8,
    priority: u8,
    target: TargetKind,
    payload_len: u8,
    requester_scoped: bool,
    has_request_id: bool,
    sender: MessageSender,
) -> MessageDefinition {
    MessageDefinition {
        name,
        class,
        opcode,
        priority,
        target,
        payload_len,
        requester_scoped,
        has_request_id,
        sender,
    }
}

pub const MESSAGE_DEFINITIONS: &[MessageDefinition] = &[
    message(
        "EMERGENCY_DISABLE",
        MessageClass::Control,
        control_opcode::EMERGENCY_DISABLE,
        0,
        TargetKind::Node,
        8,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "SET_CARRIER_POLICY",
        MessageClass::Control,
        control_opcode::SET_CARRIER_POLICY,
        1,
        TargetKind::Node,
        24,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "ACKNOWLEDGE_EMERGENCY",
        MessageClass::Control,
        control_opcode::ACKNOWLEDGE_EMERGENCY,
        1,
        TargetKind::Node,
        8,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "REQUEST_STATUS",
        MessageClass::Control,
        control_opcode::REQUEST_STATUS,
        1,
        TargetKind::Node,
        8,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "SET_FAN",
        MessageClass::Control,
        control_opcode::SET_FAN,
        1,
        TargetKind::Fan,
        12,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "SET_BINDING",
        MessageClass::Control,
        control_opcode::SET_BINDING,
        1,
        TargetKind::Node,
        24,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "COMMAND_RESPONSE",
        MessageClass::Response,
        response_opcode::COMMAND,
        2,
        TargetKind::Node,
        16,
        true,
        true,
        MessageSender::Node,
    ),
    message(
        "NODE_STATE",
        MessageClass::State,
        state_opcode::NODE,
        3,
        TargetKind::Node,
        16,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "FAN_STATE",
        MessageClass::State,
        state_opcode::FAN,
        3,
        TargetKind::Fan,
        12,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "POWER",
        MessageClass::Telemetry,
        telemetry_opcode::POWER,
        4,
        TargetKind::Node,
        16,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "TEMPERATURE",
        MessageClass::Telemetry,
        telemetry_opcode::TEMPERATURE,
        4,
        TargetKind::Node,
        16,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "HEARTBEAT",
        MessageClass::Management,
        management_opcode::HEARTBEAT,
        5,
        TargetKind::Node,
        16,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "NODE_INFO",
        MessageClass::Management,
        management_opcode::NODE_INFO,
        5,
        TargetKind::Node,
        32,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "BINDING",
        MessageClass::Management,
        management_opcode::BINDING,
        5,
        TargetKind::Node,
        16,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "FW_BEGIN",
        MessageClass::Firmware,
        firmware_opcode::BEGIN,
        5,
        TargetKind::Node,
        64,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "FW_DATA",
        MessageClass::Firmware,
        firmware_opcode::DATA,
        5,
        TargetKind::Node,
        64,
        true,
        false,
        MessageSender::Host,
    ),
    message(
        "FW_FINISH",
        MessageClass::Firmware,
        firmware_opcode::FINISH,
        5,
        TargetKind::Node,
        16,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "FW_ACTIVATE",
        MessageClass::Firmware,
        firmware_opcode::ACTIVATE,
        5,
        TargetKind::Node,
        16,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "FW_ABORT",
        MessageClass::Firmware,
        firmware_opcode::ABORT,
        5,
        TargetKind::Node,
        16,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "FW_STATUS",
        MessageClass::Firmware,
        firmware_opcode::STATUS,
        5,
        TargetKind::Node,
        8,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "FW_ACK",
        MessageClass::Firmware,
        firmware_opcode::ACK,
        5,
        TargetKind::Node,
        24,
        true,
        true,
        MessageSender::Node,
    ),
    message(
        "FW_STATUS_RESPONSE",
        MessageClass::Firmware,
        firmware_opcode::STATUS_RESPONSE,
        5,
        TargetKind::Node,
        64,
        true,
        true,
        MessageSender::Node,
    ),
    message(
        "NODE_CLAIM",
        MessageClass::Commissioning,
        commissioning_opcode::NODE_CLAIM,
        6,
        TargetKind::Commissioning,
        16,
        false,
        false,
        MessageSender::Node,
    ),
    message(
        "DISCOVER",
        MessageClass::Commissioning,
        commissioning_opcode::DISCOVER,
        6,
        TargetKind::Commissioning,
        8,
        true,
        false,
        MessageSender::Host,
    ),
    message(
        "DISCOVERY_RESPONSE",
        MessageClass::Commissioning,
        commissioning_opcode::DISCOVERY_RESPONSE,
        6,
        TargetKind::Commissioning,
        64,
        true,
        false,
        MessageSender::Node,
    ),
    message(
        "IDENTIFY",
        MessageClass::Commissioning,
        commissioning_opcode::IDENTIFY,
        6,
        TargetKind::Commissioning,
        24,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "ASSIGN_NODE",
        MessageClass::Commissioning,
        commissioning_opcode::ASSIGN_NODE,
        6,
        TargetKind::Commissioning,
        24,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "CLEAR_NODE",
        MessageClass::Commissioning,
        commissioning_opcode::CLEAR_NODE,
        6,
        TargetKind::Commissioning,
        20,
        true,
        true,
        MessageSender::Host,
    ),
    message(
        "COMMISSIONING_RESULT",
        MessageClass::Commissioning,
        commissioning_opcode::RESULT,
        6,
        TargetKind::Commissioning,
        24,
        true,
        true,
        MessageSender::Node,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireFrame {
    id: ExtendedId,
    payload: [u8; 64],
    len: u8,
}

impl WireFrame {
    pub fn new(id: ExtendedId, payload: &[u8]) -> Result<Self, FrameError> {
        let len = u8::try_from(payload.len()).map_err(|_| FrameError::PayloadTooLong)?;
        if !is_can_fd_length(len) {
            return Err(FrameError::InvalidCanFdLength(len));
        }
        let mut storage = [0; 64];
        storage[..payload.len()].copy_from_slice(payload);
        Ok(Self {
            id,
            payload: storage,
            len,
        })
    }

    pub const fn id(&self) -> ExtendedId {
        self.id
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..usize::from(self.len)]
    }
}

const fn is_can_fd_length(length: u8) -> bool {
    matches!(length, 0..=8 | 12 | 16 | 20 | 24 | 32 | 48 | 64)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    PayloadTooLong,
    InvalidCanFdLength(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadError {
    WrongLength { expected: u8, actual: u8 },
    ReservedNonZero { offset: u8 },
    InvalidValue { offset: u8, value: u8 },
    InvalidNodeId(u8),
    InvalidFanId(u8),
    InvalidDescriptor,
    InvalidPolicy,
    InvalidTemperature,
    InvalidImageTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    InvalidHeader(InvalidHeader),
    InvalidFrame(FrameError),
    Payload(PayloadError),
    UnexpectedClass(u8),
    UnknownOpcode(u8),
    WrongTarget(u8),
    InvalidCommissioningToken(u16),
}

impl From<InvalidHeader> for CodecError {
    fn from(value: InvalidHeader) -> Self {
        Self::InvalidHeader(value)
    }
}

impl From<FrameError> for CodecError {
    fn from(value: FrameError) -> Self {
        Self::InvalidFrame(value)
    }
}

impl From<PayloadError> for CodecError {
    fn from(value: PayloadError) -> Self {
        Self::Payload(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCommand {
    EmergencyDisable,
    SetCarrierPolicy(CarrierPolicy),
    AcknowledgeEmergency,
    RequestStatus,
    SetFan { fan: FanId, config: FanConfig },
    SetBinding(Option<CarrierBinding>),
}

impl ControlCommand {
    pub const fn opcode(self) -> u8 {
        match self {
            Self::EmergencyDisable => control_opcode::EMERGENCY_DISABLE,
            Self::SetCarrierPolicy(_) => control_opcode::SET_CARRIER_POLICY,
            Self::AcknowledgeEmergency => control_opcode::ACKNOWLEDGE_EMERGENCY,
            Self::RequestStatus => control_opcode::REQUEST_STATUS,
            Self::SetFan { .. } => control_opcode::SET_FAN,
            Self::SetBinding(_) => control_opcode::SET_BINDING,
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
    require_host_requester(request.requester)?;
    if matches!(request.command, ControlCommand::EmergencyDisable) {
        if request.node != BROADCAST_NODE {
            require_operational_node(request.node)?;
        }
    } else {
        require_operational_node(request.node)?;
    }
    let target = match request.command {
        ControlCommand::SetFan { fan, .. } => fan.get(),
        _ => NODE_TARGET,
    };
    let header = Header::new(
        u8::from(!matches!(request.command, ControlCommand::EmergencyDisable)),
        MessageClass::Control,
        request.node,
        target,
        request.requester,
        request.command.opcode(),
    )?;
    let mut payload = [0; 24];
    write_request_id(request.request_id, &mut payload);
    let len = match request.command {
        ControlCommand::EmergencyDisable
        | ControlCommand::AcknowledgeEmergency
        | ControlCommand::RequestStatus => 8,
        ControlCommand::SetCarrierPolicy(policy) => {
            payload[8] = u8::from(policy.enabled);
            if let Some(limits) = policy.pd_limits {
                payload[9] = 1;
                payload[12..16].copy_from_slice(&limits.max_voltage_mv.to_le_bytes());
                payload[16..20].copy_from_slice(&limits.max_current_ma.to_le_bytes());
                payload[20..24].copy_from_slice(&limits.max_power_mw.to_le_bytes());
            }
            24
        }
        ControlCommand::SetFan { config, .. } => {
            config.validate().map_err(|_| PayloadError::InvalidValue {
                offset: 8,
                value: config.duty_percent,
            })?;
            payload[8] = config.duty_percent;
            12
        }
        ControlCommand::SetBinding(binding) => {
            match binding {
                Some(binding) => {
                    payload[8] = 1;
                    payload[9] = binding.slot.get();
                    payload[12..24].copy_from_slice(binding.backplane_uid.as_bytes());
                }
                None => payload[9] = 0xFF,
            }
            24
        }
    };
    WireFrame::new(header.encode(), &payload[..len]).map_err(Into::into)
}

pub fn decode_control_request(
    id: ExtendedId,
    payload: &[u8],
) -> Result<ControlRequest, CodecError> {
    let header = Header::decode(id)?;
    require_class(header, MessageClass::Control)?;
    require_host_requester(header.requester)?;
    let request_id = read_request_id(payload)?;
    let command = match header.opcode {
        control_opcode::EMERGENCY_DISABLE => {
            require_priority(header.priority, 0)?;
            require_target(header.target, NODE_TARGET)?;
            require_len(payload, 8)?;
            if header.node != BROADCAST_NODE {
                require_operational_node(header.node)?;
            }
            ControlCommand::EmergencyDisable
        }
        control_opcode::SET_CARRIER_POLICY => {
            require_standard_control_header(header)?;
            require_len(payload, 24)?;
            require_bool(payload[8], 8)?;
            require_bool(payload[9], 9)?;
            require_zero_range(payload, 10, 12)?;
            let pd_limits = if payload[9] == 0 {
                require_zero_range(payload, 12, 24)?;
                None
            } else {
                Some(PdLimits {
                    max_voltage_mv: read_u32(payload, 12),
                    max_current_ma: read_u32(payload, 16),
                    max_power_mw: read_u32(payload, 20),
                })
            };
            ControlCommand::SetCarrierPolicy(CarrierPolicy {
                enabled: payload[8] != 0,
                pd_limits,
            })
        }
        control_opcode::ACKNOWLEDGE_EMERGENCY => {
            require_standard_control_header(header)?;
            require_len(payload, 8)?;
            ControlCommand::AcknowledgeEmergency
        }
        control_opcode::REQUEST_STATUS => {
            require_standard_control_header(header)?;
            require_len(payload, 8)?;
            ControlCommand::RequestStatus
        }
        control_opcode::SET_FAN => {
            require_priority(header.priority, 1)?;
            require_operational_node(header.node)?;
            require_len(payload, 12)?;
            require_zero_range(payload, 9, 12)?;
            let fan =
                FanId::new(header.target).map_err(|error| PayloadError::InvalidFanId(error.0))?;
            let config = FanConfig {
                duty_percent: payload[8],
            };
            config.validate().map_err(|_| PayloadError::InvalidValue {
                offset: 8,
                value: payload[8],
            })?;
            ControlCommand::SetFan { fan, config }
        }
        control_opcode::SET_BINDING => {
            require_standard_control_header(header)?;
            require_len(payload, 24)?;
            require_bool(payload[8], 8)?;
            require_zero_range(payload, 10, 12)?;
            let binding = if payload[8] == 0 {
                if payload[9] != 0xFF {
                    return Err(PayloadError::InvalidValue {
                        offset: 9,
                        value: payload[9],
                    }
                    .into());
                }
                require_zero_range(payload, 12, 24)?;
                None
            } else {
                Some(CarrierBinding {
                    backplane_uid: read_uid(payload, 12),
                    slot: SlotIndex::new(payload[9]).map_err(|error| {
                        PayloadError::InvalidValue {
                            offset: 9,
                            value: error.0,
                        }
                    })?,
                })
            };
            ControlCommand::SetBinding(binding)
        }
        opcode => return Err(CodecError::UnknownOpcode(opcode)),
    };
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
    Pending = 1,
    InvalidArgument = 2,
    InvalidTarget = 3,
    Unsupported = 4,
    Busy = 5,
    NotReady = 6,
    HardwareFault = 7,
    Storage = 8,
    Timeout = 9,
    VerifyFailed = 10,
    AddressConflict = 11,
    EmergencyLatched = 12,
    Internal = 13,
}

impl TryFrom<u8> for CommandResult {
    type Error = PayloadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::Pending),
            2 => Ok(Self::InvalidArgument),
            3 => Ok(Self::InvalidTarget),
            4 => Ok(Self::Unsupported),
            5 => Ok(Self::Busy),
            6 => Ok(Self::NotReady),
            7 => Ok(Self::HardwareFault),
            8 => Ok(Self::Storage),
            9 => Ok(Self::Timeout),
            10 => Ok(Self::VerifyFailed),
            11 => Ok(Self::AddressConflict),
            12 => Ok(Self::EmergencyLatched),
            13 => Ok(Self::Internal),
            value => Err(PayloadError::InvalidValue { offset: 10, value }),
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
        NODE_TARGET,
        response.requester,
        response_opcode::COMMAND,
    )?;
    let mut payload = [0; 16];
    write_request_id(response.request_id, &mut payload);
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
    require_class(header, MessageClass::Response)?;
    require_priority(header.priority, 2)?;
    require_operational_node(header.node)?;
    require_target(header.target, NODE_TARGET)?;
    require_host_requester(header.requester)?;
    if header.opcode != response_opcode::COMMAND {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_len(payload, 16)?;
    require_zero(payload, 11)?;
    Ok(CommandResponse {
        node: header.node,
        requester: header.requester,
        request_id: read_request_id(payload)?,
        request_opcode: payload[8],
        request_target: payload[9],
        result: CommandResult::try_from(payload[10])?,
        detail: read_u32(payload, 12),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeState {
    pub node: u8,
    pub sequence: u16,
    pub service: ServiceState,
    pub faults: FaultFlags,
    pub uptime_seconds: u32,
    pub output_enabled: bool,
    pub power_good: bool,
    pub emergency_latched: bool,
}

pub fn encode_node_state(state: NodeState) -> Result<WireFrame, CodecError> {
    require_operational_node(state.node)?;
    let header = protocol_header(
        3,
        MessageClass::State,
        state.node,
        NODE_TARGET,
        state_opcode::NODE,
    )?;
    let mut payload = [0; 16];
    payload[..2].copy_from_slice(&state.sequence.to_le_bytes());
    payload[2] = state.service as u8;
    payload[3] = u8::from(state.output_enabled)
        | u8::from(state.power_good) << 1
        | u8::from(state.emergency_latched) << 2;
    payload[4..8].copy_from_slice(&state.faults.bits().to_le_bytes());
    payload[8..12].copy_from_slice(&state.uptime_seconds.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_node_state(id: ExtendedId, payload: &[u8]) -> Result<NodeState, CodecError> {
    let header = require_node_message(id, payload, MessageClass::State, state_opcode::NODE, 3, 16)?;
    require_zero_range(payload, 12, 16)?;
    if payload[3] & !0x07 != 0 {
        return Err(PayloadError::InvalidValue {
            offset: 3,
            value: payload[3],
        }
        .into());
    }
    Ok(NodeState {
        node: header.node,
        sequence: read_u16(payload, 0),
        service: ServiceState::try_from(payload[2]).map_err(|error| {
            PayloadError::InvalidValue {
                offset: 2,
                value: error.0,
            }
        })?,
        output_enabled: payload[3] & 1 != 0,
        power_good: payload[3] & 2 != 0,
        emergency_latched: payload[3] & 4 != 0,
        faults: FaultFlags::from_bits_retain(read_u32(payload, 4)),
        uptime_seconds: read_u32(payload, 8),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanState {
    pub node: u8,
    pub fan: FanId,
    pub sequence: u16,
    pub duty_percent: u8,
    pub rpm: u16,
    pub stalled: bool,
}

pub fn encode_fan_state(state: FanState) -> Result<WireFrame, CodecError> {
    require_operational_node(state.node)?;
    FanConfig {
        duty_percent: state.duty_percent,
    }
    .validate()
    .map_err(|_| PayloadError::InvalidValue {
        offset: 2,
        value: state.duty_percent,
    })?;
    let header = protocol_header(
        3,
        MessageClass::State,
        state.node,
        state.fan.get(),
        state_opcode::FAN,
    )?;
    let mut payload = [0; 12];
    payload[..2].copy_from_slice(&state.sequence.to_le_bytes());
    payload[2] = state.duty_percent;
    payload[3] = u8::from(state.stalled);
    payload[4..6].copy_from_slice(&state.rpm.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_fan_state(id: ExtendedId, payload: &[u8]) -> Result<FanState, CodecError> {
    let header = Header::decode(id)?;
    require_protocol_header(header, MessageClass::State, state_opcode::FAN, 3)?;
    require_operational_node(header.node)?;
    require_len(payload, 12)?;
    require_bool(payload[3], 3)?;
    require_zero_range(payload, 6, 12)?;
    let fan = FanId::new(header.target).map_err(|error| PayloadError::InvalidFanId(error.0))?;
    FanConfig {
        duty_percent: payload[2],
    }
    .validate()
    .map_err(|_| PayloadError::InvalidValue {
        offset: 2,
        value: payload[2],
    })?;
    Ok(FanState {
        node: header.node,
        fan,
        sequence: read_u16(payload, 0),
        duty_percent: payload[2],
        rpm: read_u16(payload, 4),
        stalled: payload[3] != 0,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PowerTelemetry {
    pub node: u8,
    pub reading: PowerReading,
}

pub fn encode_power(telemetry: PowerTelemetry) -> Result<WireFrame, CodecError> {
    require_operational_node(telemetry.node)?;
    let header = protocol_header(
        4,
        MessageClass::Telemetry,
        telemetry.node,
        NODE_TARGET,
        telemetry_opcode::POWER,
    )?;
    let mut payload = [0; 16];
    payload[..2].copy_from_slice(&telemetry.reading.sequence.to_le_bytes());
    payload[2] = telemetry.reading.source as u8;
    payload[4..8].copy_from_slice(&telemetry.reading.voltage_mv.to_le_bytes());
    payload[8..12].copy_from_slice(&telemetry.reading.current_ma.to_le_bytes());
    payload[12..16].copy_from_slice(&telemetry.reading.power_mw.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_power(id: ExtendedId, payload: &[u8]) -> Result<PowerTelemetry, CodecError> {
    let header = require_node_message(
        id,
        payload,
        MessageClass::Telemetry,
        telemetry_opcode::POWER,
        4,
        16,
    )?;
    require_zero(payload, 3)?;
    Ok(PowerTelemetry {
        node: header.node,
        reading: PowerReading {
            source: PowerSource::try_from(payload[2]).map_err(|error| {
                PayloadError::InvalidValue {
                    offset: 2,
                    value: error.0,
                }
            })?,
            sequence: read_u16(payload, 0),
            voltage_mv: read_u32(payload, 4),
            current_ma: read_i32(payload, 8),
            power_mw: read_i32(payload, 12),
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemperatureTelemetry {
    pub node: u8,
    pub reading: TemperatureReading,
}

pub fn encode_temperature(telemetry: TemperatureTelemetry) -> Result<WireFrame, CodecError> {
    require_operational_node(telemetry.node)?;
    telemetry
        .reading
        .validate()
        .map_err(|_| PayloadError::InvalidTemperature)?;
    let header = protocol_header(
        4,
        MessageClass::Telemetry,
        telemetry.node,
        NODE_TARGET,
        telemetry_opcode::TEMPERATURE,
    )?;
    let mut payload = [0; 16];
    payload[..2].copy_from_slice(&telemetry.reading.sequence.to_le_bytes());
    payload[2] = telemetry.reading.sensor.source as u8;
    payload[3] = telemetry.reading.status as u8;
    if let Some(value) = telemetry.reading.temperature_centi_c {
        payload[4..6].copy_from_slice(&value.to_le_bytes());
    }
    payload[8..16].copy_from_slice(&telemetry.reading.sensor.identifier.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_temperature(
    id: ExtendedId,
    payload: &[u8],
) -> Result<TemperatureTelemetry, CodecError> {
    let header = require_node_message(
        id,
        payload,
        MessageClass::Telemetry,
        telemetry_opcode::TEMPERATURE,
        4,
        16,
    )?;
    require_zero_range(payload, 6, 8)?;
    let status =
        SampleStatus::try_from(payload[3]).map_err(|error| PayloadError::InvalidValue {
            offset: 3,
            value: error.0,
        })?;
    let reading = TemperatureReading {
        sensor: TemperatureSensorId {
            source: TemperatureSource::try_from(payload[2]).map_err(|error| {
                PayloadError::InvalidValue {
                    offset: 2,
                    value: error.0,
                }
            })?,
            identifier: read_u64(payload, 8),
        },
        sequence: read_u16(payload, 0),
        status,
        temperature_centi_c: match status {
            SampleStatus::Valid | SampleStatus::Stale => Some(read_i16(payload, 4)),
            SampleStatus::Unavailable | SampleStatus::Invalid => {
                require_zero_range(payload, 4, 6)?;
                None
            }
        },
    };
    reading
        .validate()
        .map_err(|_| PayloadError::InvalidTemperature)?;
    Ok(TemperatureTelemetry {
        node: header.node,
        reading,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeInfo {
    pub node: u8,
    pub descriptor: NodeDescriptor,
    pub firmware: FirmwareVersion,
    pub bootloader: FirmwareVersion,
    pub partition_layout: u8,
}

pub fn encode_node_info(info: NodeInfo) -> Result<WireFrame, CodecError> {
    require_operational_node(info.node)?;
    let header = protocol_header(
        5,
        MessageClass::Management,
        info.node,
        NODE_TARGET,
        management_opcode::NODE_INFO,
    )?;
    let mut payload = [0; 32];
    encode_node_info_payload(info, &mut payload)?;
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_node_info(id: ExtendedId, payload: &[u8]) -> Result<NodeInfo, CodecError> {
    let header = require_node_message(
        id,
        payload,
        MessageClass::Management,
        management_opcode::NODE_INFO,
        5,
        32,
    )?;
    decode_node_info_payload(header.node, payload)
}

fn encode_node_info_payload(info: NodeInfo, payload: &mut [u8]) -> Result<(), CodecError> {
    info.descriptor
        .validate()
        .map_err(|_| PayloadError::InvalidDescriptor)?;
    payload[0] = info.descriptor.role as u8;
    payload[1] = info
        .descriptor
        .carrier_profile
        .map_or(0, |profile| profile as u8);
    payload[2] = info.descriptor.hardware_revision.get();
    payload[3] = info.descriptor.update_impact as u8;
    payload[4..12].copy_from_slice(&info.descriptor.capabilities.bits().to_le_bytes());
    payload[12] = info.descriptor.output_count;
    payload[13] = info.descriptor.fan_count;
    payload[14] = info.descriptor.external_temperature_capacity;
    payload[15] = info.partition_layout;
    write_version(info.firmware, payload, 16);
    write_version(info.bootloader, payload, 22);
    Ok(())
}

fn decode_node_info_payload(node: u8, payload: &[u8]) -> Result<NodeInfo, CodecError> {
    require_len(payload, 32)?;
    require_zero_range(payload, 28, 32)?;
    let role = NodeRole::try_from(payload[0]).map_err(|error| PayloadError::InvalidValue {
        offset: 0,
        value: error.0,
    })?;
    let carrier_profile = if payload[1] == 0 {
        None
    } else {
        Some(
            CarrierProfile::try_from(payload[1]).map_err(|error| PayloadError::InvalidValue {
                offset: 1,
                value: error.0,
            })?,
        )
    };
    let descriptor = NodeDescriptor {
        role,
        carrier_profile,
        hardware_revision: HardwareRevision::new(payload[2]).map_err(|error| {
            PayloadError::InvalidValue {
                offset: 2,
                value: error.0,
            }
        })?,
        update_impact: UpdateImpact::try_from(payload[3]).map_err(|error| {
            PayloadError::InvalidValue {
                offset: 3,
                value: error.0,
            }
        })?,
        capabilities: CapabilityFlags::from_bits_retain(read_u64(payload, 4)),
        output_count: payload[12],
        fan_count: payload[13],
        external_temperature_capacity: payload[14],
    };
    descriptor
        .validate()
        .map_err(|_| PayloadError::InvalidDescriptor)?;
    Ok(NodeInfo {
        node,
        descriptor,
        partition_layout: payload[15],
        firmware: read_version(payload, 16),
        bootloader: read_version(payload, 22),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Heartbeat {
    pub node: u8,
    pub sequence: u16,
    pub service: ServiceState,
    pub update: UpdateState,
    pub faults: FaultFlags,
    pub uptime_seconds: u32,
}

pub fn encode_heartbeat(heartbeat: Heartbeat) -> Result<WireFrame, CodecError> {
    require_operational_node(heartbeat.node)?;
    let header = protocol_header(
        5,
        MessageClass::Management,
        heartbeat.node,
        NODE_TARGET,
        management_opcode::HEARTBEAT,
    )?;
    let mut payload = [0; 16];
    payload[..2].copy_from_slice(&heartbeat.sequence.to_le_bytes());
    payload[2] = heartbeat.service as u8;
    payload[3] = heartbeat.update as u8;
    payload[4..8].copy_from_slice(&heartbeat.faults.bits().to_le_bytes());
    payload[8..12].copy_from_slice(&heartbeat.uptime_seconds.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_heartbeat(id: ExtendedId, payload: &[u8]) -> Result<Heartbeat, CodecError> {
    let header = require_node_message(
        id,
        payload,
        MessageClass::Management,
        management_opcode::HEARTBEAT,
        5,
        16,
    )?;
    require_zero_range(payload, 12, 16)?;
    Ok(Heartbeat {
        node: header.node,
        sequence: read_u16(payload, 0),
        service: ServiceState::try_from(payload[2]).map_err(|error| {
            PayloadError::InvalidValue {
                offset: 2,
                value: error.0,
            }
        })?,
        update: UpdateState::try_from(payload[3]).map_err(|error| PayloadError::InvalidValue {
            offset: 3,
            value: error.0,
        })?,
        faults: FaultFlags::from_bits_retain(read_u32(payload, 4)),
        uptime_seconds: read_u32(payload, 8),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingInfo {
    pub node: u8,
    pub binding: Option<CarrierBinding>,
}

pub fn encode_binding(info: BindingInfo) -> Result<WireFrame, CodecError> {
    require_operational_node(info.node)?;
    let header = protocol_header(
        5,
        MessageClass::Management,
        info.node,
        NODE_TARGET,
        management_opcode::BINDING,
    )?;
    let mut payload = [0; 16];
    match info.binding {
        Some(binding) => {
            payload[0] = 1;
            payload[1] = binding.slot.get();
            payload[4..16].copy_from_slice(binding.backplane_uid.as_bytes());
        }
        None => payload[1] = 0xFF,
    }
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_binding(id: ExtendedId, payload: &[u8]) -> Result<BindingInfo, CodecError> {
    let header = require_node_message(
        id,
        payload,
        MessageClass::Management,
        management_opcode::BINDING,
        5,
        16,
    )?;
    require_bool(payload[0], 0)?;
    require_zero_range(payload, 2, 4)?;
    let binding = if payload[0] == 0 {
        if payload[1] != 0xFF {
            return Err(PayloadError::InvalidValue {
                offset: 1,
                value: payload[1],
            }
            .into());
        }
        require_zero_range(payload, 4, 16)?;
        None
    } else {
        Some(CarrierBinding {
            backplane_uid: read_uid(payload, 4),
            slot: SlotIndex::new(payload[1]).map_err(|error| PayloadError::InvalidValue {
                offset: 1,
                value: error.0,
            })?,
        })
    };
    Ok(BindingInfo {
        node: header.node,
        binding,
    })
}

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
        let requester = RequesterId::new(((raw >> REQUESTER_SHIFT) & REQUESTER_MASK) as u8)
            .map_err(|error| InvalidHeader::Requester(error.0))?;
        Self::new(
            ((raw >> PRIORITY_SHIFT) & PRIORITY_MASK) as u8,
            ((raw >> TARGET_SHIFT) & 0x0FFF) as u16,
            requester,
            (raw & OPCODE_MASK) as u8,
        )
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
    pub node_info: NodeInfo,
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

#[allow(clippy::too_many_lines)]
pub fn encode_commissioning(
    requester: RequesterId,
    message: CommissioningMessage,
) -> Result<WireFrame, CodecError> {
    if matches!(message, CommissioningMessage::NodeClaim { .. }) {
        require_protocol_requester(requester)?;
    } else {
        require_host_requester(requester)?;
    }
    let mut payload = [0; 64];
    let (opcode, token, len) = match message {
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
            if info.node_info.node != info.node_id.map_or(BROADCAST_NODE, NodeId::get) {
                return Err(PayloadError::InvalidDescriptor.into());
            }
            payload[..12].copy_from_slice(info.uid.as_bytes());
            payload[12] = info.node_id.map_or(BROADCAST_NODE, NodeId::get);
            payload[13] = info.state as u8;
            payload[14] = info.protocol_major;
            payload[15] = info.protocol_minor;
            encode_node_info_payload(info.node_info, &mut payload[16..48])?;
            payload[48..56].copy_from_slice(&nonce.to_le_bytes());
            (
                commissioning_opcode::DISCOVERY_RESPONSE,
                commissioning_token(info.uid, nonce),
                64,
            )
        }
        CommissioningMessage::Identify {
            request_id,
            uid,
            duration_seconds,
        } => {
            write_request_id(request_id, &mut payload);
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
            write_request_id(request_id, &mut payload);
            payload[8..20].copy_from_slice(uid.as_bytes());
            payload[20] = node_id.get();
            (
                commissioning_opcode::ASSIGN_NODE,
                commissioning_token(uid, request_id.0),
                24,
            )
        }
        CommissioningMessage::ClearNode { request_id, uid } => {
            write_request_id(request_id, &mut payload);
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
            write_request_id(request_id, &mut payload);
            payload[8..20].copy_from_slice(uid.as_bytes());
            payload[20] = request_opcode;
            payload[21] = result as u8;
            payload[22] = node_id.map_or(BROADCAST_NODE, NodeId::get);
            payload[23] = state as u8;
            (
                commissioning_opcode::RESULT,
                commissioning_token(uid, request_id.0),
                24,
            )
        }
    };
    let header = CommissioningHeader::new(6, token, requester, opcode)?;
    WireFrame::new(header.encode(), &payload[..len]).map_err(Into::into)
}

pub fn decode_commissioning(
    id: ExtendedId,
    payload: &[u8],
) -> Result<(CommissioningHeader, CommissioningMessage), CodecError> {
    let header = CommissioningHeader::decode(id)?;
    require_priority(header.priority, 6)?;
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
            require_token(header.token, node_claim_token(uid, node_id, claim_nonce))?;
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
            require_len(payload, 64)?;
            require_zero_range(payload, 56, 64)?;
            let uid = read_uid(payload, 0);
            let node_id = decode_node(payload[12])?;
            let nonce = read_u64(payload, 48);
            require_token(header.token, commissioning_token(uid, nonce))?;
            CommissioningMessage::DiscoveryResponse {
                nonce,
                info: DiscoveryInfo {
                    uid,
                    node_id,
                    state: decode_commissioning_state(payload[13], 13)?,
                    protocol_major: payload[14],
                    protocol_minor: payload[15],
                    node_info: decode_node_info_payload(
                        node_id.map_or(BROADCAST_NODE, NodeId::get),
                        &payload[16..48],
                    )?,
                },
            }
        }
        commissioning_opcode::IDENTIFY => {
            require_len(payload, 24)?;
            require_zero_range(payload, 22, 24)?;
            let request_id = read_request_id(payload)?;
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
            let request_id = read_request_id(payload)?;
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
            let request_id = read_request_id(payload)?;
            let uid = read_uid(payload, 8);
            require_token(header.token, commissioning_token(uid, request_id.0))?;
            CommissioningMessage::ClearNode { request_id, uid }
        }
        commissioning_opcode::RESULT => {
            require_len(payload, 24)?;
            let request_id = read_request_id(payload)?;
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
    Ok((header, message))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareRequest {
    Begin {
        request_id: RequestId,
        session: UpdateSessionId,
        manifest: UpdateManifest,
    },
    Data {
        session: UpdateSessionId,
        offset: u32,
        len: u8,
        bytes: [u8; FIRMWARE_CHUNK_BYTES],
    },
    Finish {
        request_id: RequestId,
        session: UpdateSessionId,
    },
    Activate {
        request_id: RequestId,
        session: UpdateSessionId,
        allow_interruption: bool,
    },
    Abort {
        request_id: RequestId,
        session: UpdateSessionId,
    },
    Status {
        request_id: RequestId,
    },
}

pub fn encode_firmware_request(
    node: NodeId,
    requester: RequesterId,
    request: FirmwareRequest,
) -> Result<WireFrame, CodecError> {
    require_host_requester(requester)?;
    let mut payload = [0; 64];
    let (opcode, len) = match request {
        FirmwareRequest::Begin {
            request_id,
            session,
            manifest,
        } => {
            manifest
                .target
                .validate()
                .map_err(|_| PayloadError::InvalidImageTarget)?;
            write_request_id(request_id, &mut payload);
            payload[8..12].copy_from_slice(&session.0.to_le_bytes());
            payload[12..16].copy_from_slice(&manifest.image_size.to_le_bytes());
            payload[16] = manifest.target.role as u8;
            payload[17] = manifest
                .target
                .carrier_profile
                .map_or(0, |profile| profile as u8);
            payload[18] = manifest.target.hardware_revision.get();
            payload[19] = manifest.target.partition_layout;
            write_version(manifest.version, &mut payload, 20);
            payload[32..64].copy_from_slice(manifest.digest.as_bytes());
            (firmware_opcode::BEGIN, 64)
        }
        FirmwareRequest::Data {
            session,
            offset,
            len,
            bytes,
        } => {
            if len == 0 || usize::from(len) > FIRMWARE_CHUNK_BYTES {
                return Err(PayloadError::InvalidValue {
                    offset: 8,
                    value: len,
                }
                .into());
            }
            if bytes[usize::from(len)..].iter().any(|byte| *byte != 0) {
                return Err(PayloadError::ReservedNonZero { offset: len + 16 }.into());
            }
            payload[..4].copy_from_slice(&session.0.to_le_bytes());
            payload[4..8].copy_from_slice(&offset.to_le_bytes());
            payload[8] = len;
            payload[16..64].copy_from_slice(&bytes);
            (firmware_opcode::DATA, 64)
        }
        FirmwareRequest::Finish {
            request_id,
            session,
        } => {
            write_request_id(request_id, &mut payload);
            payload[8..12].copy_from_slice(&session.0.to_le_bytes());
            (firmware_opcode::FINISH, 16)
        }
        FirmwareRequest::Activate {
            request_id,
            session,
            allow_interruption,
        } => {
            write_request_id(request_id, &mut payload);
            payload[8..12].copy_from_slice(&session.0.to_le_bytes());
            payload[12] = u8::from(allow_interruption);
            (firmware_opcode::ACTIVATE, 16)
        }
        FirmwareRequest::Abort {
            request_id,
            session,
        } => {
            write_request_id(request_id, &mut payload);
            payload[8..12].copy_from_slice(&session.0.to_le_bytes());
            (firmware_opcode::ABORT, 16)
        }
        FirmwareRequest::Status { request_id } => {
            write_request_id(request_id, &mut payload);
            (firmware_opcode::STATUS, 8)
        }
    };
    let header = Header::new(
        5,
        MessageClass::Firmware,
        node.get(),
        NODE_TARGET,
        requester,
        opcode,
    )?;
    WireFrame::new(header.encode(), &payload[..len]).map_err(Into::into)
}

#[allow(clippy::too_many_lines)]
pub fn decode_firmware_request(
    id: ExtendedId,
    payload: &[u8],
) -> Result<(NodeId, RequesterId, FirmwareRequest), CodecError> {
    let header = Header::decode(id)?;
    require_class(header, MessageClass::Firmware)?;
    require_priority(header.priority, 5)?;
    require_target(header.target, NODE_TARGET)?;
    require_host_requester(header.requester)?;
    let node = NodeId::new(header.node).map_err(|error| InvalidHeader::Node(error.0))?;
    let request = match header.opcode {
        firmware_opcode::BEGIN => {
            require_len(payload, 64)?;
            require_zero_range(payload, 26, 32)?;
            let role =
                NodeRole::try_from(payload[16]).map_err(|error| PayloadError::InvalidValue {
                    offset: 16,
                    value: error.0,
                })?;
            let carrier_profile = if payload[17] == 0 {
                None
            } else {
                Some(CarrierProfile::try_from(payload[17]).map_err(|error| {
                    PayloadError::InvalidValue {
                        offset: 17,
                        value: error.0,
                    }
                })?)
            };
            let target = ImageTarget {
                role,
                carrier_profile,
                hardware_revision: HardwareRevision::new(payload[18]).map_err(|error| {
                    PayloadError::InvalidValue {
                        offset: 18,
                        value: error.0,
                    }
                })?,
                partition_layout: payload[19],
            };
            target
                .validate()
                .map_err(|_| PayloadError::InvalidImageTarget)?;
            let mut digest = [0; 32];
            digest.copy_from_slice(&payload[32..64]);
            FirmwareRequest::Begin {
                request_id: read_request_id(payload)?,
                session: UpdateSessionId(read_u32(payload, 8)),
                manifest: UpdateManifest {
                    target,
                    version: read_version(payload, 20),
                    image_size: read_u32(payload, 12),
                    digest: Sha256Digest::from_bytes(digest),
                },
            }
        }
        firmware_opcode::DATA => {
            require_len(payload, 64)?;
            require_zero_range(payload, 9, 16)?;
            let len = payload[8];
            if len == 0 || usize::from(len) > FIRMWARE_CHUNK_BYTES {
                return Err(PayloadError::InvalidValue {
                    offset: 8,
                    value: len,
                }
                .into());
            }
            let mut bytes = [0; FIRMWARE_CHUNK_BYTES];
            bytes.copy_from_slice(&payload[16..64]);
            if bytes[usize::from(len)..].iter().any(|byte| *byte != 0) {
                return Err(PayloadError::ReservedNonZero { offset: len + 16 }.into());
            }
            FirmwareRequest::Data {
                session: UpdateSessionId(read_u32(payload, 0)),
                offset: read_u32(payload, 4),
                len,
                bytes,
            }
        }
        firmware_opcode::FINISH => {
            require_len(payload, 16)?;
            require_zero_range(payload, 12, 16)?;
            FirmwareRequest::Finish {
                request_id: read_request_id(payload)?,
                session: UpdateSessionId(read_u32(payload, 8)),
            }
        }
        firmware_opcode::ACTIVATE => {
            require_len(payload, 16)?;
            require_bool(payload[12], 12)?;
            require_zero_range(payload, 13, 16)?;
            FirmwareRequest::Activate {
                request_id: read_request_id(payload)?,
                session: UpdateSessionId(read_u32(payload, 8)),
                allow_interruption: payload[12] != 0,
            }
        }
        firmware_opcode::ABORT => {
            require_len(payload, 16)?;
            require_zero_range(payload, 12, 16)?;
            FirmwareRequest::Abort {
                request_id: read_request_id(payload)?,
                session: UpdateSessionId(read_u32(payload, 8)),
            }
        }
        firmware_opcode::STATUS => {
            require_len(payload, 8)?;
            FirmwareRequest::Status {
                request_id: read_request_id(payload)?,
            }
        }
        opcode => return Err(CodecError::UnknownOpcode(opcode)),
    };
    Ok((node, header.requester, request))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareAck {
    pub node: u8,
    pub requester: RequesterId,
    /// Zero is used for data-window acknowledgements, whose chunks do not carry
    /// individual request IDs.
    pub request_id: RequestId,
    pub session: UpdateSessionId,
    pub next_offset: u32,
    pub state: UpdateState,
    pub error: UpdateError,
    pub window_frames: u8,
    pub detail: u32,
}

pub fn encode_firmware_ack(ack: FirmwareAck) -> Result<WireFrame, CodecError> {
    require_operational_node(ack.node)?;
    require_host_requester(ack.requester)?;
    let header = Header::new(
        5,
        MessageClass::Firmware,
        ack.node,
        NODE_TARGET,
        ack.requester,
        firmware_opcode::ACK,
    )?;
    let mut payload = [0; 24];
    write_request_id(ack.request_id, &mut payload);
    payload[8..12].copy_from_slice(&ack.session.0.to_le_bytes());
    payload[12..16].copy_from_slice(&ack.next_offset.to_le_bytes());
    payload[16] = ack.state as u8;
    payload[17] = ack.error as u8;
    payload[18] = ack.window_frames;
    payload[20..24].copy_from_slice(&ack.detail.to_le_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_firmware_ack(id: ExtendedId, payload: &[u8]) -> Result<FirmwareAck, CodecError> {
    let header = require_node_message_scoped(
        id,
        payload,
        MessageClass::Firmware,
        firmware_opcode::ACK,
        5,
        24,
    )?;
    require_zero(payload, 19)?;
    Ok(FirmwareAck {
        node: header.node,
        requester: header.requester,
        request_id: read_request_id(payload)?,
        session: UpdateSessionId(read_u32(payload, 8)),
        next_offset: read_u32(payload, 12),
        state: UpdateState::try_from(payload[16]).map_err(|error| PayloadError::InvalidValue {
            offset: 16,
            value: error.0,
        })?,
        error: UpdateError::try_from(payload[17]).map_err(|error| PayloadError::InvalidValue {
            offset: 17,
            value: error.0,
        })?,
        window_frames: payload[18],
        detail: read_u32(payload, 20),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareStatus {
    pub node: u8,
    pub requester: RequesterId,
    pub request_id: RequestId,
    pub state: UpdateState,
    pub error: UpdateError,
    pub impact: UpdateImpact,
    pub running: FirmwareVersion,
    pub staged: Option<FirmwareVersion>,
    pub session: UpdateSessionId,
    pub next_offset: u32,
    pub digest: Sha256Digest,
}

pub fn encode_firmware_status(status: FirmwareStatus) -> Result<WireFrame, CodecError> {
    require_operational_node(status.node)?;
    require_host_requester(status.requester)?;
    let header = Header::new(
        5,
        MessageClass::Firmware,
        status.node,
        NODE_TARGET,
        status.requester,
        firmware_opcode::STATUS_RESPONSE,
    )?;
    let mut payload = [0; 64];
    write_request_id(status.request_id, &mut payload);
    payload[8] = status.state as u8;
    payload[9] = status.error as u8;
    payload[10] = status.impact as u8;
    payload[11] = u8::from(status.staged.is_some());
    write_version(status.running, &mut payload, 12);
    if let Some(staged) = status.staged {
        write_version(staged, &mut payload, 18);
    }
    payload[24..28].copy_from_slice(&status.session.0.to_le_bytes());
    payload[28..32].copy_from_slice(&status.next_offset.to_le_bytes());
    payload[32..64].copy_from_slice(status.digest.as_bytes());
    WireFrame::new(header.encode(), &payload).map_err(Into::into)
}

pub fn decode_firmware_status(
    id: ExtendedId,
    payload: &[u8],
) -> Result<FirmwareStatus, CodecError> {
    let header = require_node_message_scoped(
        id,
        payload,
        MessageClass::Firmware,
        firmware_opcode::STATUS_RESPONSE,
        5,
        64,
    )?;
    require_bool(payload[11], 11)?;
    let staged = if payload[11] == 0 {
        require_zero_range(payload, 18, 24)?;
        None
    } else {
        Some(read_version(payload, 18))
    };
    let mut digest = [0; 32];
    digest.copy_from_slice(&payload[32..64]);
    Ok(FirmwareStatus {
        node: header.node,
        requester: header.requester,
        request_id: read_request_id(payload)?,
        state: UpdateState::try_from(payload[8]).map_err(|error| PayloadError::InvalidValue {
            offset: 8,
            value: error.0,
        })?,
        error: UpdateError::try_from(payload[9]).map_err(|error| PayloadError::InvalidValue {
            offset: 9,
            value: error.0,
        })?,
        impact: UpdateImpact::try_from(payload[10]).map_err(|error| {
            PayloadError::InvalidValue {
                offset: 10,
                value: error.0,
            }
        })?,
        running: read_version(payload, 12),
        staged,
        session: UpdateSessionId(read_u32(payload, 24)),
        next_offset: read_u32(payload, 28),
        digest: Sha256Digest::from_bytes(digest),
    })
}

pub fn commissioning_token(uid: NodeUid, nonce: u64) -> u16 {
    let nonce = nonce.to_le_bytes();
    let nonce_low = u32::from_le_bytes([nonce[0], nonce[1], nonce[2], nonce[3]]);
    let nonce_high = u32::from_le_bytes([nonce[4], nonce[5], nonce[6], nonce[7]]);
    let crc = crc32(uid.as_bytes());
    let mixed = avalanche32(crc ^ nonce_low ^ nonce_high.rotate_left(13));
    u16::try_from(mixed & u32::from(CommissioningHeader::MAX_TOKEN))
        .expect("commissioning token mask fits u16")
}

pub fn node_claim_token(uid: NodeUid, node_id: NodeId, claim_nonce: u16) -> u16 {
    const CLAIM_DOMAIN: u64 = 0x5044_4341_4E43_4C4D;
    commissioning_token(
        uid,
        CLAIM_DOMAIN ^ u64::from(node_id.get()) << 16 ^ u64::from(claim_nonce),
    )
}

fn protocol_header(
    priority: u8,
    class: MessageClass,
    node: u8,
    target: u8,
    opcode: u8,
) -> Result<Header, CodecError> {
    Header::new(priority, class, node, target, RequesterId::PROTOCOL, opcode).map_err(Into::into)
}

fn require_node_message(
    id: ExtendedId,
    payload: &[u8],
    class: MessageClass,
    opcode: u8,
    priority: u8,
    len: u8,
) -> Result<Header, CodecError> {
    let header = Header::decode(id)?;
    require_protocol_header(header, class, opcode, priority)?;
    require_operational_node(header.node)?;
    require_target(header.target, NODE_TARGET)?;
    require_len(payload, len)?;
    Ok(header)
}

fn require_node_message_scoped(
    id: ExtendedId,
    payload: &[u8],
    class: MessageClass,
    opcode: u8,
    priority: u8,
    len: u8,
) -> Result<Header, CodecError> {
    let header = Header::decode(id)?;
    require_class(header, class)?;
    if header.opcode != opcode {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_priority(header.priority, priority)?;
    require_operational_node(header.node)?;
    require_target(header.target, NODE_TARGET)?;
    require_host_requester(header.requester)?;
    require_len(payload, len)?;
    Ok(header)
}

fn require_protocol_header(
    header: Header,
    class: MessageClass,
    opcode: u8,
    priority: u8,
) -> Result<(), CodecError> {
    require_class(header, class)?;
    if header.opcode != opcode {
        return Err(CodecError::UnknownOpcode(header.opcode));
    }
    require_priority(header.priority, priority)?;
    require_protocol_requester(header.requester)
}

fn require_standard_control_header(header: Header) -> Result<(), CodecError> {
    require_priority(header.priority, 1)?;
    require_operational_node(header.node)?;
    require_target(header.target, NODE_TARGET)
}

fn require_class(header: Header, expected: MessageClass) -> Result<(), CodecError> {
    if header.class == expected {
        Ok(())
    } else {
        Err(CodecError::UnexpectedClass(header.class as u8))
    }
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

fn require_bool(value: u8, offset: u8) -> Result<(), PayloadError> {
    if value <= 1 {
        Ok(())
    } else {
        Err(PayloadError::InvalidValue { offset, value })
    }
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
    if raw == BROADCAST_NODE {
        Ok(None)
    } else {
        NodeId::new(raw)
            .map(Some)
            .map_err(|error| PayloadError::InvalidNodeId(error.0))
    }
}

fn decode_commissioning_state(raw: u8, offset: u8) -> Result<CommissioningState, PayloadError> {
    CommissioningState::try_from(raw).map_err(|error| PayloadError::InvalidValue {
        offset,
        value: error.0,
    })
}

fn write_request_id(request_id: RequestId, payload: &mut [u8]) {
    payload[..8].copy_from_slice(&request_id.0.to_le_bytes());
}

fn read_request_id(payload: &[u8]) -> Result<RequestId, PayloadError> {
    if payload.len() < 8 {
        return Err(PayloadError::WrongLength {
            expected: 8,
            actual: u8::try_from(payload.len()).unwrap_or(u8::MAX),
        });
    }
    Ok(RequestId(read_u64(payload, 0)))
}

fn write_version(version: FirmwareVersion, payload: &mut [u8], offset: usize) {
    payload[offset..offset + 2].copy_from_slice(&version.major.to_le_bytes());
    payload[offset + 2..offset + 4].copy_from_slice(&version.minor.to_le_bytes());
    payload[offset + 4..offset + 6].copy_from_slice(&version.patch.to_le_bytes());
}

fn read_version(payload: &[u8], offset: usize) -> FirmwareVersion {
    FirmwareVersion {
        major: read_u16(payload, offset),
        minor: read_u16(payload, offset + 2),
        patch: read_u16(payload, offset + 4),
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

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
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

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn avalanche32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x85EB_CA6B);
    value ^= value >> 13;
    value = value.wrapping_mul(0xC2B2_AE35);
    value ^ (value >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARRIER_CAPABILITIES: CapabilityFlags = CapabilityFlags::FIRMWARE_UPDATE
        .union(CapabilityFlags::SWITCHABLE_OUTPUT)
        .union(CapabilityFlags::LOAD_MONITORING)
        .union(CapabilityFlags::USB_PD_CONTROL)
        .union(CapabilityFlags::USB_PD_STATUS)
        .union(CapabilityFlags::INTERNAL_TEMPERATURE)
        .union(CapabilityFlags::USER_BUTTON);

    fn node_info(node: u8) -> NodeInfo {
        NodeInfo {
            node,
            descriptor: NodeDescriptor {
                role: NodeRole::Carrier,
                carrier_profile: Some(CarrierProfile::Sw3538),
                hardware_revision: HardwareRevision::REV_A,
                capabilities: CARRIER_CAPABILITIES,
                update_impact: UpdateImpact::Interrupt,
                output_count: 1,
                fan_count: 0,
                external_temperature_capacity: 0,
            },
            firmware: FirmwareVersion {
                major: 1,
                minor: 2,
                patch: 3,
            },
            bootloader: FirmwareVersion {
                major: 0,
                minor: 7,
                patch: 0,
            },
            partition_layout: 1,
        }
    }

    #[test]
    fn protocol_major_is_a_clean_gen2_break() {
        assert_eq!(PROTOCOL_MAJOR, 1);
        assert_eq!(PROTOCOL_MINOR, 0);
        assert!(!MESSAGE_DEFINITIONS.iter().any(
            |definition| definition.name.contains("PORT") || definition.name.contains("BOARD")
        ));
    }

    #[test]
    fn every_message_uses_a_legal_can_fd_length() {
        for definition in MESSAGE_DEFINITIONS {
            assert!(is_can_fd_length(definition.payload_len));
            assert!(definition.opcode <= MAX_OPCODE);
            assert!(definition.priority <= MAX_PRIORITY);
        }
    }

    #[test]
    fn header_round_trip_preserves_requester() {
        let header = Header::new(
            1,
            MessageClass::Control,
            3,
            NODE_TARGET,
            RequesterId::PDCAN_DEFAULT,
            control_opcode::REQUEST_STATUS,
        )
        .unwrap();
        assert_eq!(Header::decode(header.encode()), Ok(header));
    }

    #[test]
    fn carrier_policy_and_two_fan_targets_round_trip() {
        let policy = ControlRequest {
            node: 7,
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(42),
            command: ControlCommand::SetCarrierPolicy(CarrierPolicy {
                enabled: true,
                pd_limits: Some(PdLimits::SW3538_SAFE_MAX),
            }),
        };
        let frame = encode_control_request(policy).unwrap();
        assert_eq!(
            decode_control_request(frame.id(), frame.payload()),
            Ok(policy)
        );

        for raw in FanId::MIN..=FanId::MAX {
            let request = ControlRequest {
                command: ControlCommand::SetFan {
                    fan: FanId::new(raw).unwrap(),
                    config: FanConfig { duty_percent: 73 },
                },
                ..policy
            };
            let frame = encode_control_request(request).unwrap();
            assert_eq!(
                decode_control_request(frame.id(), frame.payload()),
                Ok(request)
            );
        }
    }

    #[test]
    fn binding_round_trip_keeps_full_backplane_uid() {
        let info = BindingInfo {
            node: 9,
            binding: Some(CarrierBinding {
                backplane_uid: NodeUid::from_bytes([0xA5; 12]),
                slot: SlotIndex::new(0).unwrap(),
            }),
        };
        let frame = encode_binding(info).unwrap();
        assert_eq!(decode_binding(frame.id(), frame.payload()), Ok(info));
    }

    #[test]
    fn node_info_advertises_profile_and_interrupt_update_impact() {
        let info = node_info(7);
        let frame = encode_node_info(info).unwrap();
        assert_eq!(decode_node_info(frame.id(), frame.payload()), Ok(info));
    }

    #[test]
    fn discovery_includes_the_complete_gen2_identity() {
        let message = CommissioningMessage::DiscoveryResponse {
            nonce: 0x0102_0304_0506_0708,
            info: DiscoveryInfo {
                uid: NodeUid::from_bytes([0x3C; 12]),
                node_id: Some(NodeId::new(7).unwrap()),
                state: CommissioningState::Commissioned,
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                node_info: node_info(7),
            },
        };
        let frame = encode_commissioning(RequesterId::PDCAN_DEFAULT, message).unwrap();
        assert_eq!(frame.payload().len(), 64);
        assert_eq!(
            decode_commissioning(frame.id(), frame.payload()).unwrap().1,
            message
        );
    }

    #[test]
    fn power_and_all_temperature_sources_remain_distinct() {
        let power = PowerTelemetry {
            node: 3,
            reading: PowerReading {
                source: PowerSource::CarrierInput,
                sequence: 1,
                voltage_mv: 24_000,
                current_ma: 4_100,
                power_mw: 98_400,
            },
        };
        let frame = encode_power(power).unwrap();
        assert_eq!(decode_power(frame.id(), frame.payload()), Ok(power));

        for sensor in [
            TemperatureSensorId::STM32_DIE,
            TemperatureSensorId::INA237_DIE,
            TemperatureSensorId::one_wire(0x28AA_0102_0304_056B),
        ] {
            let temperature = TemperatureTelemetry {
                node: 3,
                reading: TemperatureReading {
                    sensor,
                    sequence: 2,
                    status: SampleStatus::Valid,
                    temperature_centi_c: Some(3_125),
                },
            };
            let frame = encode_temperature(temperature).unwrap();
            assert_eq!(
                decode_temperature(frame.id(), frame.payload()),
                Ok(temperature)
            );
        }
    }

    #[test]
    fn firmware_begin_and_48_byte_chunks_round_trip() {
        let begin = FirmwareRequest::Begin {
            request_id: RequestId(99),
            session: UpdateSessionId(0x1234_5678),
            manifest: UpdateManifest {
                target: ImageTarget {
                    role: NodeRole::Carrier,
                    carrier_profile: Some(CarrierProfile::Sw3538),
                    hardware_revision: HardwareRevision::REV_A,
                    partition_layout: 1,
                },
                version: FirmwareVersion {
                    major: 2,
                    minor: 0,
                    patch: 1,
                },
                image_size: 100 * 1024,
                digest: Sha256Digest::from_bytes([0x5A; 32]),
            },
        };
        let node = NodeId::new(3).unwrap();
        let frame = encode_firmware_request(node, RequesterId::PDCAN_DEFAULT, begin).unwrap();
        assert_eq!(
            decode_firmware_request(frame.id(), frame.payload()),
            Ok((node, RequesterId::PDCAN_DEFAULT, begin))
        );

        let data = FirmwareRequest::Data {
            session: UpdateSessionId(0x1234_5678),
            offset: 48,
            len: 48,
            bytes: [0xA5; FIRMWARE_CHUNK_BYTES],
        };
        let frame = encode_firmware_request(node, RequesterId::PDCAN_DEFAULT, data).unwrap();
        assert_eq!(
            decode_firmware_request(frame.id(), frame.payload()),
            Ok((node, RequesterId::PDCAN_DEFAULT, data))
        );
    }

    #[test]
    fn interrupt_activation_is_explicit_on_the_wire() {
        let request = FirmwareRequest::Activate {
            request_id: RequestId(10),
            session: UpdateSessionId(5),
            allow_interruption: true,
        };
        let node = NodeId::new(4).unwrap();
        let frame = encode_firmware_request(node, RequesterId::PDCAN_DEFAULT, request).unwrap();
        assert_eq!(frame.payload()[12], 1);
        assert_eq!(
            decode_firmware_request(frame.id(), frame.payload()),
            Ok((node, RequesterId::PDCAN_DEFAULT, request))
        );
    }

    #[test]
    fn status_carries_running_staged_state_and_full_digest() {
        let status = FirmwareStatus {
            node: 8,
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(77),
            state: UpdateState::Staged,
            error: UpdateError::None,
            impact: UpdateImpact::Interrupt,
            running: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            staged: Some(FirmwareVersion {
                major: 1,
                minor: 1,
                patch: 0,
            }),
            session: UpdateSessionId(22),
            next_offset: 100 * 1024,
            digest: Sha256Digest::from_bytes([0xCC; 32]),
        };
        let frame = encode_firmware_status(status).unwrap();
        assert_eq!(
            decode_firmware_status(frame.id(), frame.payload()),
            Ok(status)
        );
    }
}
