#![no_std]

use pdcan_types::{RequestId, RequesterId};

pub const EXTENDED_ID_MASK: u32 = 0x1FFF_FFFF;
pub const BROADCAST_NODE: u8 = 0;
pub const INVALID_NODE: u8 = 0xFF;
pub const ALL_PORTS_TARGET: u8 = 0x0E;
pub const BOARD_TARGET: u8 = 0x0F;

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
pub struct MessageDefinition {
    pub name: &'static str,
    pub class: MessageClass,
    pub opcode: u8,
    pub priority: u8,
    pub target: TargetKind,
    pub payload_len: u8,
    pub requester_scoped: bool,
}

pub const MESSAGE_DEFINITIONS: &[MessageDefinition] = &[
    MessageDefinition {
        name: "EMERGENCY_DISABLE",
        class: MessageClass::Control,
        opcode: 0,
        priority: 0,
        target: TargetKind::AllPorts,
        payload_len: 8,
        requester_scoped: true,
    },
    MessageDefinition {
        name: "SET_PORT_POLICY",
        class: MessageClass::Control,
        opcode: 1,
        priority: 1,
        target: TargetKind::Port,
        payload_len: 20,
        requester_scoped: true,
    },
    MessageDefinition {
        name: "ACKNOWLEDGE_EMERGENCY_RESOLVED",
        class: MessageClass::Control,
        opcode: 2,
        priority: 1,
        target: TargetKind::Board,
        payload_len: 8,
        requester_scoped: true,
    },
    MessageDefinition {
        name: "REQUEST_STATUS",
        class: MessageClass::Control,
        opcode: 3,
        priority: 1,
        target: TargetKind::Port,
        payload_len: 8,
        requester_scoped: true,
    },
    MessageDefinition {
        name: "COMMAND_RESPONSE",
        class: MessageClass::Response,
        opcode: 0,
        priority: 2,
        target: TargetKind::Board,
        payload_len: 16,
        requester_scoped: true,
    },
    MessageDefinition {
        name: "PORT_STATE",
        class: MessageClass::State,
        opcode: 0,
        priority: 3,
        target: TargetKind::Port,
        payload_len: 16,
        requester_scoped: false,
    },
    MessageDefinition {
        name: "PORT_POWER",
        class: MessageClass::Telemetry,
        opcode: 0,
        priority: 4,
        target: TargetKind::Port,
        payload_len: 16,
        requester_scoped: false,
    },
    MessageDefinition {
        name: "HEARTBEAT",
        class: MessageClass::Management,
        opcode: 0,
        priority: 5,
        target: TargetKind::Board,
        payload_len: 16,
        requester_scoped: false,
    },
    MessageDefinition {
        name: "BOARD_INFO",
        class: MessageClass::Management,
        opcode: 1,
        priority: 5,
        target: TargetKind::Board,
        payload_len: 24,
        requester_scoped: false,
    },
    MessageDefinition {
        name: "NODE_CLAIM",
        class: MessageClass::Commissioning,
        opcode: 0,
        priority: 6,
        target: TargetKind::Commissioning,
        payload_len: 16,
        requester_scoped: false,
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
            assert!(definition.payload_len <= 64);
        }
    }
}
