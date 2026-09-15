use pdcan_core::{ChunkDisposition, UpdateController};
use pdcan_protocol::{
    BindingInfo, CommandResponse, CommandResult, CommissioningMessage, ControlCommand,
    DecodedHeader, DiscoveryInfo, FirmwareAck, FirmwareRequest, FirmwareStatus, NodeInfo,
    NodeState, PROTOCOL_MAJOR, PROTOCOL_MINOR, WireFrame, commissioning_opcode,
    decode_commissioning, decode_control_request, decode_firmware_request, decode_header,
    encode_binding, encode_command_response, encode_commissioning, encode_firmware_ack,
    encode_firmware_status, encode_node_info, encode_node_state,
};
use pdcan_types::{
    CapabilityFlags, CarrierBinding, CarrierPolicy, CarrierProfile, CommissioningState, FanConfig,
    FirmwareVersion, HardwareRevision, ImageTarget, NodeDescriptor, NodeId, NodeRole, NodeUid,
    RequestId, RequesterId, ServiceState, Sha256Digest, UpdateError, UpdateImpact, UpdateState,
};
use sha2::{Digest as _, Sha256};

pub const PARTITION_LAYOUT_VERSION: u8 = 1;
pub const SIMULATED_FIRMWARE: FirmwareVersion = FirmwareVersion {
    major: 0,
    minor: 1,
    patch: 0,
};
pub const SIMULATED_BOOTLOADER: FirmwareVersion = FirmwareVersion {
    major: 0,
    minor: 1,
    patch: 0,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatedProfile {
    Backplane,
    Carrier {
        profile: CarrierProfile,
        update_impact: UpdateImpact,
    },
}

impl SimulatedProfile {
    pub const fn descriptor(self) -> NodeDescriptor {
        match self {
            Self::Backplane => NodeDescriptor {
                role: NodeRole::Backplane,
                carrier_profile: None,
                hardware_revision: HardwareRevision::SIMULATOR,
                capabilities: CapabilityFlags::FIRMWARE_UPDATE
                    .union(CapabilityFlags::FAN_CONTROL)
                    .union(CapabilityFlags::FAN_TACHOMETER)
                    .union(CapabilityFlags::INTERNAL_TEMPERATURE)
                    .union(CapabilityFlags::EXTERNAL_TEMPERATURE)
                    .union(CapabilityFlags::AGGREGATE_POWER),
                update_impact: UpdateImpact::Interrupt,
                output_count: 0,
                fan_count: 2,
                external_temperature_capacity: 0xFF,
            },
            Self::Carrier {
                profile,
                update_impact,
            } => NodeDescriptor {
                role: NodeRole::Carrier,
                carrier_profile: Some(profile),
                hardware_revision: HardwareRevision::SIMULATOR,
                capabilities: carrier_capabilities(profile),
                update_impact,
                output_count: if matches!(profile, CarrierProfile::Accessory) {
                    0
                } else {
                    1
                },
                fan_count: 0,
                external_temperature_capacity: 0,
            },
        }
    }
}

const fn carrier_capabilities(profile: CarrierProfile) -> CapabilityFlags {
    let base = CapabilityFlags::FIRMWARE_UPDATE.union(CapabilityFlags::INTERNAL_TEMPERATURE);
    match profile {
        CarrierProfile::Sw3538 => base
            .union(CapabilityFlags::SWITCHABLE_OUTPUT)
            .union(CapabilityFlags::LOAD_MONITORING)
            .union(CapabilityFlags::USB_PD_CONTROL)
            .union(CapabilityFlags::USB_PD_STATUS)
            .union(CapabilityFlags::USER_BUTTON),
        CarrierProfile::Basic => base
            .union(CapabilityFlags::SWITCHABLE_OUTPUT)
            .union(CapabilityFlags::LOAD_MONITORING),
        CarrierProfile::Accessory => base.union(CapabilityFlags::AUXILIARY_IO),
        CarrierProfile::HighPower240W => base
            .union(CapabilityFlags::SWITCHABLE_OUTPUT)
            .union(CapabilityFlags::LOAD_MONITORING)
            .union(CapabilityFlags::USB_PD_CONTROL)
            .union(CapabilityFlags::USB_PD_STATUS),
    }
}

#[derive(Debug)]
pub struct SimulatedNode {
    profile: SimulatedProfile,
    uid: NodeUid,
    node_id: Option<NodeId>,
    state: CommissioningState,
    binding: Option<CarrierBinding>,
    policy: CarrierPolicy,
    fans: [FanConfig; 2],
    emergency_latched: bool,
    firmware: FirmwareVersion,
    update: UpdateController,
    image: Vec<u8>,
}

impl SimulatedNode {
    pub fn new(profile: SimulatedProfile, uid: NodeUid, node_id: Option<NodeId>) -> Self {
        let descriptor = profile.descriptor();
        Self {
            profile,
            uid,
            node_id,
            state: if node_id.is_some() {
                CommissioningState::Commissioned
            } else {
                CommissioningState::Uncommissioned
            },
            binding: None,
            policy: if descriptor
                .capabilities
                .contains(CapabilityFlags::USB_PD_CONTROL)
            {
                CarrierPolicy::SW3538_SAFE_DISABLED
            } else {
                CarrierPolicy {
                    enabled: false,
                    pd_limits: None,
                }
            },
            fans: [FanConfig::SAFE_DEFAULT; 2],
            emergency_latched: false,
            firmware: SIMULATED_FIRMWARE,
            update: UpdateController::new(
                ImageTarget {
                    role: descriptor.role,
                    carrier_profile: descriptor.carrier_profile,
                    hardware_revision: descriptor.hardware_revision,
                    partition_layout: PARTITION_LAYOUT_VERSION,
                },
                descriptor.update_impact,
            ),
            image: Vec::new(),
        }
    }

    pub const fn descriptor(&self) -> NodeDescriptor {
        self.profile.descriptor()
    }

    pub const fn node_id(&self) -> Option<NodeId> {
        self.node_id
    }

    pub fn handle_frame(
        &mut self,
        id: pdcan_protocol::ExtendedId,
        payload: &[u8],
    ) -> Vec<WireFrame> {
        match decode_header(id) {
            Ok(DecodedHeader::Commissioning(_)) => self.handle_commissioning(id, payload),
            Ok(DecodedHeader::Operational(header))
                if header.class == pdcan_protocol::MessageClass::Control =>
            {
                self.handle_control(id, payload)
            }
            Ok(DecodedHeader::Operational(header))
                if header.class == pdcan_protocol::MessageClass::Firmware =>
            {
                self.handle_firmware(id, payload)
            }
            _ => Vec::new(),
        }
    }

    fn handle_commissioning(
        &mut self,
        id: pdcan_protocol::ExtendedId,
        payload: &[u8],
    ) -> Vec<WireFrame> {
        let Ok((header, request)) = decode_commissioning(id, payload) else {
            return Vec::new();
        };
        let response = match request {
            CommissioningMessage::Discover { nonce } => CommissioningMessage::DiscoveryResponse {
                nonce,
                info: DiscoveryInfo {
                    uid: self.uid,
                    node_id: self.node_id,
                    state: self.state,
                    protocol_major: PROTOCOL_MAJOR,
                    protocol_minor: PROTOCOL_MINOR,
                    node_info: self.node_info(),
                },
            },
            CommissioningMessage::Identify {
                request_id, uid, ..
            } if uid == self.uid => self.commissioning_result(
                request_id,
                commissioning_opcode::IDENTIFY,
                CommandResult::Ok,
            ),
            CommissioningMessage::AssignNode {
                request_id,
                uid,
                node_id,
            } if uid == self.uid => {
                self.node_id = Some(node_id);
                self.state = CommissioningState::Commissioned;
                self.commissioning_result(
                    request_id,
                    commissioning_opcode::ASSIGN_NODE,
                    CommandResult::Ok,
                )
            }
            CommissioningMessage::ClearNode { request_id, uid } if uid == self.uid => {
                self.node_id = None;
                self.state = CommissioningState::Uncommissioned;
                self.commissioning_result(
                    request_id,
                    commissioning_opcode::CLEAR_NODE,
                    CommandResult::Ok,
                )
            }
            _ => return Vec::new(),
        };
        encode_commissioning(header.requester, response)
            .map(|frame| vec![frame])
            .unwrap_or_default()
    }

    fn commissioning_result(
        &self,
        request_id: RequestId,
        request_opcode: u8,
        result: CommandResult,
    ) -> CommissioningMessage {
        CommissioningMessage::Result {
            request_id,
            uid: self.uid,
            request_opcode,
            result,
            node_id: self.node_id,
            state: self.state,
        }
    }

    fn handle_control(&mut self, id: pdcan_protocol::ExtendedId, payload: &[u8]) -> Vec<WireFrame> {
        let Ok(request) = decode_control_request(id, payload) else {
            return Vec::new();
        };
        let is_broadcast_emergency = request.node == pdcan_protocol::BROADCAST_NODE
            && request.command == ControlCommand::EmergencyDisable;
        if !is_broadcast_emergency && Some(request.node) != self.node_id.map(NodeId::get) {
            return Vec::new();
        }

        let result = match request.command {
            ControlCommand::EmergencyDisable => {
                self.emergency_latched = true;
                self.policy.enabled = false;
                self.fans = [FanConfig::SAFE_DEFAULT; 2];
                CommandResult::Ok
            }
            ControlCommand::AcknowledgeEmergency => {
                self.emergency_latched = false;
                CommandResult::Ok
            }
            ControlCommand::RequestStatus => CommandResult::Ok,
            ControlCommand::SetCarrierPolicy(policy)
                if self.descriptor().role == NodeRole::Carrier =>
            {
                self.policy = policy;
                CommandResult::Ok
            }
            ControlCommand::SetFan { fan, config }
                if self.descriptor().role == NodeRole::Backplane =>
            {
                self.fans[fan.index()] = config;
                CommandResult::Ok
            }
            ControlCommand::SetBinding(binding) if self.descriptor().role == NodeRole::Carrier => {
                self.binding = binding;
                CommandResult::Ok
            }
            _ => CommandResult::Unsupported,
        };

        let Some(node) = self.node_id else {
            return Vec::new();
        };
        let mut frames = vec![
            encode_command_response(CommandResponse {
                node: node.get(),
                requester: request.requester,
                request_id: request.request_id,
                request_opcode: request.command.opcode(),
                request_target: match request.command {
                    ControlCommand::SetFan { fan, .. } => fan.get(),
                    _ => pdcan_protocol::NODE_TARGET,
                },
                result,
                detail: 0,
            })
            .expect("simulator response is valid"),
        ];
        if request.command == ControlCommand::RequestStatus {
            frames.push(encode_node_state(self.node_state()).expect("simulator state is valid"));
            frames.push(encode_node_info(self.node_info()).expect("simulator info is valid"));
            if self.descriptor().role == NodeRole::Carrier {
                frames.push(
                    encode_binding(BindingInfo {
                        node: node.get(),
                        binding: self.binding,
                    })
                    .expect("simulator binding is valid"),
                );
            }
        }
        frames
    }

    fn handle_firmware(
        &mut self,
        id: pdcan_protocol::ExtendedId,
        payload: &[u8],
    ) -> Vec<WireFrame> {
        let Ok((node, requester, request)) = decode_firmware_request(id, payload) else {
            return Vec::new();
        };
        if Some(node) != self.node_id {
            return Vec::new();
        }
        match request {
            FirmwareRequest::Status { request_id } => {
                vec![self.firmware_status(requester, request_id)]
            }
            FirmwareRequest::Begin {
                request_id,
                session,
                manifest,
            } => {
                let result = self.update.begin(session, manifest);
                if result.is_ok() {
                    self.image = vec![0; manifest.image_size as usize];
                }
                vec![self.firmware_ack(requester, request_id, result.err())]
            }
            FirmwareRequest::Data {
                session,
                offset,
                len,
                bytes,
            } => {
                let mut error = None;
                match self.update.accept_chunk(session, offset, len) {
                    Ok(ChunkDisposition::Write) => {
                        let start = offset as usize;
                        let end = start + usize::from(len);
                        self.image[start..end].copy_from_slice(&bytes[..usize::from(len)]);
                    }
                    Ok(ChunkDisposition::Duplicate) => {
                        let start = offset as usize;
                        let end = start + usize::from(len);
                        if self.image.get(start..end) != Some(&bytes[..usize::from(len)]) {
                            error = Some(UpdateError::ConflictingData);
                        }
                    }
                    Err(update_error) => error = Some(update_error),
                }
                vec![self.firmware_ack(requester, RequestId(0), error)]
            }
            FirmwareRequest::Finish {
                request_id,
                session,
            } => {
                let error = match self.update.begin_verification(session) {
                    Ok(manifest) => {
                        let digest: [u8; 32] = Sha256::digest(&self.image).into();
                        self.update
                            .verification_complete(digest == *manifest.digest.as_bytes())
                            .err()
                    }
                    Err(update_error) => Some(update_error),
                };
                vec![self.firmware_ack(requester, request_id, error)]
            }
            FirmwareRequest::Activate {
                request_id,
                session,
                allow_interruption,
            } => {
                let result = self.update.activate(session, allow_interruption);
                if result.is_ok() {
                    if let Some(manifest) = self.update.manifest() {
                        self.firmware = manifest.version;
                    }
                    self.policy.enabled = false;
                }
                vec![self.firmware_ack(requester, request_id, result.err())]
            }
            FirmwareRequest::Abort {
                request_id,
                session,
            } => {
                let result = self.update.abort(session);
                if result.is_ok() {
                    self.image.clear();
                }
                vec![self.firmware_ack(requester, request_id, result.err())]
            }
        }
    }

    fn node_info(&self) -> NodeInfo {
        NodeInfo {
            node: self.node_id.map_or(0, NodeId::get),
            descriptor: self.descriptor(),
            firmware: self.firmware,
            bootloader: SIMULATED_BOOTLOADER,
            partition_layout: PARTITION_LAYOUT_VERSION,
        }
    }

    fn node_state(&self) -> NodeState {
        NodeState {
            node: self.node_id.expect("operational state needs a node").get(),
            sequence: 1,
            service: if self.emergency_latched {
                ServiceState::Faulted
            } else if self.policy.enabled {
                ServiceState::Enabled
            } else {
                ServiceState::Disabled
            },
            faults: if self.emergency_latched {
                pdcan_types::FaultFlags::EMERGENCY_LATCHED
            } else {
                pdcan_types::FaultFlags::NONE
            },
            uptime_seconds: 1,
            output_enabled: self.policy.enabled,
            power_good: self.policy.enabled,
            emergency_latched: self.emergency_latched,
        }
    }

    fn firmware_ack(
        &self,
        requester: RequesterId,
        request_id: RequestId,
        error: Option<UpdateError>,
    ) -> WireFrame {
        encode_firmware_ack(FirmwareAck {
            node: self.node_id.expect("firmware response needs a node").get(),
            requester,
            request_id,
            session: self.update.session(),
            next_offset: self.update.next_offset(),
            state: self.update.state(),
            error: error.unwrap_or(UpdateError::None),
            window_frames: pdcan_protocol::FIRMWARE_WINDOW_FRAMES,
            detail: 0,
        })
        .expect("simulator firmware acknowledgement is valid")
    }

    fn firmware_status(&self, requester: RequesterId, request_id: RequestId) -> WireFrame {
        let manifest = self.update.manifest();
        encode_firmware_status(FirmwareStatus {
            node: self.node_id.expect("firmware response needs a node").get(),
            requester,
            request_id,
            state: self.update.state(),
            error: self.update.error(),
            impact: self.descriptor().update_impact,
            running: self.firmware,
            staged: (self.update.state() == UpdateState::Staged)
                .then(|| manifest.map(|value| value.version))
                .flatten(),
            session: self.update.session(),
            next_offset: self.update.next_offset(),
            digest: manifest.map_or(Sha256Digest::from_bytes([0; 32]), |value| value.digest),
        })
        .expect("simulator firmware status is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_protocol::{
        FIRMWARE_CHUNK_BYTES, decode_firmware_ack, decode_firmware_status, encode_firmware_request,
    };
    use pdcan_types::{UpdateManifest, UpdateSessionId};

    const UID: NodeUid = NodeUid::from_bytes([0x42; 12]);
    const NODE: NodeId = match NodeId::new(7) {
        Ok(node) => node,
        Err(_) => panic!("valid test node"),
    };
    const REQUESTER: RequesterId = RequesterId::PDCAN_DEFAULT;

    #[test]
    fn profile_controls_role_capabilities_and_update_impact() {
        let current = SimulatedProfile::Carrier {
            profile: CarrierProfile::Sw3538,
            update_impact: UpdateImpact::Interrupt,
        }
        .descriptor();
        assert_eq!(current.role, NodeRole::Carrier);
        assert_eq!(current.update_impact, UpdateImpact::Interrupt);
        assert!(
            current
                .capabilities
                .contains(CapabilityFlags::USB_PD_CONTROL)
        );

        let future_live = SimulatedProfile::Carrier {
            profile: CarrierProfile::Accessory,
            update_impact: UpdateImpact::Live,
        }
        .descriptor();
        assert_eq!(future_live.update_impact, UpdateImpact::Live);
        assert!(
            future_live
                .capabilities
                .contains(CapabilityFlags::AUXILIARY_IO)
        );
    }

    #[test]
    fn discovery_reports_complete_gen2_identity() {
        let mut node = SimulatedNode::new(SimulatedProfile::Backplane, UID, Some(NODE));
        let discover = pdcan_protocol::encode_commissioning(
            REQUESTER,
            CommissioningMessage::Discover { nonce: 99 },
        )
        .unwrap();
        let responses = node.handle_frame(discover.id(), discover.payload());
        let (_, CommissioningMessage::DiscoveryResponse { info, .. }) =
            decode_commissioning(responses[0].id(), responses[0].payload()).unwrap()
        else {
            panic!("expected discovery response");
        };
        assert_eq!(info.node_info.descriptor.role, NodeRole::Backplane);
        assert_eq!(info.node_info.descriptor.fan_count, 2);
        assert_eq!(info.protocol_major, PROTOCOL_MAJOR);
    }

    #[test]
    fn update_retries_are_idempotent_and_interrupt_activation_is_guarded() {
        let profile = SimulatedProfile::Carrier {
            profile: CarrierProfile::Sw3538,
            update_impact: UpdateImpact::Interrupt,
        };
        let mut node = SimulatedNode::new(profile, UID, Some(NODE));
        let image = [0xA5; FIRMWARE_CHUNK_BYTES];
        let digest: [u8; 32] = Sha256::digest(image).into();
        let session = UpdateSessionId(12);
        let manifest = UpdateManifest {
            target: ImageTarget {
                role: NodeRole::Carrier,
                carrier_profile: Some(CarrierProfile::Sw3538),
                hardware_revision: HardwareRevision::SIMULATOR,
                partition_layout: PARTITION_LAYOUT_VERSION,
            },
            version: FirmwareVersion {
                major: 1,
                minor: 2,
                patch: 3,
            },
            image_size: u32::try_from(image.len()).unwrap(),
            digest: Sha256Digest::from_bytes(digest),
        };

        let begin = encode_firmware_request(
            NODE,
            REQUESTER,
            FirmwareRequest::Begin {
                request_id: RequestId(1),
                session,
                manifest,
            },
        )
        .unwrap();
        node.handle_frame(begin.id(), begin.payload());

        let mut bytes = [0; FIRMWARE_CHUNK_BYTES];
        bytes.copy_from_slice(&image);
        let data = encode_firmware_request(
            NODE,
            REQUESTER,
            FirmwareRequest::Data {
                session,
                offset: 0,
                len: u8::try_from(FIRMWARE_CHUNK_BYTES).unwrap(),
                bytes,
            },
        )
        .unwrap();
        node.handle_frame(data.id(), data.payload());
        let retry = node.handle_frame(data.id(), data.payload());
        let retry_ack = decode_firmware_ack(retry[0].id(), retry[0].payload()).unwrap();
        assert_eq!(retry_ack.error, UpdateError::None);
        assert_eq!(
            retry_ack.next_offset,
            u32::try_from(FIRMWARE_CHUNK_BYTES).unwrap()
        );

        let finish = encode_firmware_request(
            NODE,
            REQUESTER,
            FirmwareRequest::Finish {
                request_id: RequestId(2),
                session,
            },
        )
        .unwrap();
        node.handle_frame(finish.id(), finish.payload());

        let denied = encode_firmware_request(
            NODE,
            REQUESTER,
            FirmwareRequest::Activate {
                request_id: RequestId(3),
                session,
                allow_interruption: false,
            },
        )
        .unwrap();
        let denied_response = node.handle_frame(denied.id(), denied.payload());
        assert_eq!(
            decode_firmware_ack(denied_response[0].id(), denied_response[0].payload())
                .unwrap()
                .error,
            UpdateError::InterruptionNotAuthorized
        );

        let status = encode_firmware_request(
            NODE,
            REQUESTER,
            FirmwareRequest::Status {
                request_id: RequestId(4),
            },
        )
        .unwrap();
        let status_response = node.handle_frame(status.id(), status.payload());
        assert_eq!(
            decode_firmware_status(status_response[0].id(), status_response[0].payload())
                .unwrap()
                .state,
            UpdateState::Staged
        );
    }
}
