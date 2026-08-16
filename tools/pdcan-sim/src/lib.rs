use std::collections::VecDeque;

use pdcan_core::{
    Action, Commissioning, CompletionOutcome, Controller, ControllerError, PersistCompletion,
};
use pdcan_protocol::{
    CommandResponse, CommandResult, CommissioningMessage, ControlCommand, ControlRequest,
    DecodedHeader, ExtendedId, PortState, PortStateFlags, WireFrame, commissioning_opcode,
    decode_commissioning, decode_control_request, decode_header, encode_command_response,
    encode_commissioning, encode_port_state,
};
use pdcan_types::{
    BoardDefinition, CommissioningState, FaultFlags, FirmwareVersion, HardwareRevision, NodeId,
    NodeUid, PersistentSettings, PortBitmap, PortId, RequestId, RequesterId,
};

const REQUEST_CACHE_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachedRequest {
    request: ControlRequest,
    response: CommandResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachedCommissioningRequest {
    requester: RequesterId,
    request: CommissioningMessage,
    response: CommissioningMessage,
}

pub struct SimulatedNode {
    board: BoardDefinition,
    controller: Controller,
    commissioning: Commissioning,
    durable_settings: PersistentSettings,
    request_cache: VecDeque<CachedRequest>,
    commissioning_request_cache: VecDeque<CachedCommissioningRequest>,
    identify_duration_seconds: u16,
    persistence_failures_remaining: u32,
    online_ports: PortBitmap,
    state_sequence: u16,
}

impl SimulatedNode {
    pub fn new(port_count: u8, uid: NodeUid, node_id: Option<NodeId>) -> Result<Self, String> {
        let board = simulator_board(port_count)?;
        let settings = PersistentSettings {
            node_id,
            ..PersistentSettings::FACTORY_DEFAULT
        };
        let mut commissioning = Commissioning::new(uid, node_id);
        if node_id.is_some() {
            commissioning.claim_window_complete();
        }
        let mut node = Self {
            board,
            controller: Controller::new(board, settings),
            commissioning,
            durable_settings: settings,
            request_cache: VecDeque::with_capacity(REQUEST_CACHE_CAPACITY),
            commissioning_request_cache: VecDeque::with_capacity(REQUEST_CACHE_CAPACITY),
            identify_duration_seconds: 0,
            persistence_failures_remaining: 0,
            online_ports: board.supported_ports,
            state_sequence: 0,
        };
        for raw_port in 0..port_count {
            node.controller
                .module_detected(PortId::new(raw_port).expect("validated simulator port"))
                .map_err(|error| format!("cannot add simulated port: {error:?}"))?;
        }
        node.execute_actions();
        Ok(node)
    }

    pub const fn board(&self) -> BoardDefinition {
        self.board
    }

    pub const fn settings(&self) -> PersistentSettings {
        self.controller.settings()
    }

    pub const fn durable_settings(&self) -> PersistentSettings {
        self.durable_settings
    }

    pub const fn commissioning(&self) -> Commissioning {
        self.commissioning
    }

    pub const fn identify_duration_seconds(&self) -> u16 {
        self.identify_duration_seconds
    }

    pub fn fail_next_persistence_attempts(&mut self, count: u32) {
        self.persistence_failures_remaining = count;
    }

    pub fn remove_port(&mut self, port: PortId) -> Result<(), ControllerError> {
        self.controller.module_removed(port).map(|_| {
            self.online_ports.remove(port);
        })
    }

    #[must_use]
    pub fn restart(&self) -> Self {
        let port_count = u8::try_from(self.board.supported_ports.bits().count_ones())
            .expect("an eight-bit port bitmap has at most eight bits");
        Self::new(
            port_count,
            self.commissioning.uid(),
            self.durable_settings.node_id,
        )
        .expect("an existing simulator board definition is valid")
        .with_settings(self.durable_settings)
    }

    pub fn handle_frame(&mut self, id: ExtendedId, payload: &[u8]) -> Vec<WireFrame> {
        match decode_header(id) {
            Ok(DecodedHeader::Operational(_)) => self.handle_control(id, payload),
            Ok(DecodedHeader::Commissioning(_)) => self.handle_commissioning(id, payload),
            Err(_) => Vec::new(),
        }
    }

    fn with_settings(mut self, settings: PersistentSettings) -> Self {
        self.controller = Controller::new(self.board, settings);
        self.durable_settings = settings;
        self.commissioning = Commissioning::new(self.commissioning.uid(), settings.node_id);
        if settings.node_id.is_some() {
            self.commissioning.claim_window_complete();
        }
        for raw_port in PortId::MIN..=PortId::MAX {
            let port = PortId::new(raw_port).expect("MAX_PORTS defines valid IDs");
            if self.board.supports(port) {
                self.controller
                    .module_detected(port)
                    .expect("board-supported simulator port");
            }
        }
        self.execute_actions();
        self
    }

    fn handle_control(&mut self, id: ExtendedId, payload: &[u8]) -> Vec<WireFrame> {
        let Ok(request) = decode_control_request(id, payload) else {
            return Vec::new();
        };
        let is_broadcast_emergency =
            request.node == 0 && matches!(request.command, ControlCommand::EmergencyDisable);
        let addressed_to_self = self.commissioning.node_id().is_some_and(|node| {
            node.get() == request.node && self.commissioning.normal_traffic_allowed()
        });
        if !addressed_to_self && !is_broadcast_emergency {
            return Vec::new();
        }

        if let Some(cached) = self
            .request_cache
            .iter()
            .find(|cached| {
                cached.request.requester == request.requester
                    && cached.request.request_id == request.request_id
            })
            .copied()
        {
            let response = if cached.request == request {
                cached.response
            } else {
                self.make_response(request, CommandResult::InvalidArgument, 1)
            };
            let mut frames = Self::encode_operational_response(response);
            if cached.request == request
                && response.result == CommandResult::Ok
                && let ControlCommand::RequestStatus { port } = request.command
                && let Some(state) = self.port_state_frame(port)
            {
                frames.push(state);
            }
            return frames;
        }

        let result = self.apply_control(request);
        let response = self.make_response(request, result, 0);
        self.remember(request, response);

        // An uncommissioned board still obeys broadcast emergency disable, but
        // cannot safely emit an operational response with a shared Node ID.
        if self.commissioning.node_id().is_none()
            || self.commissioning.state() == CommissioningState::AddressConflict
        {
            Vec::new()
        } else {
            let mut frames = Self::encode_operational_response(response);
            if response.result == CommandResult::Ok
                && let ControlCommand::RequestStatus { port } = request.command
                && let Some(state) = self.port_state_frame(port)
            {
                frames.push(state);
            }
            frames
        }
    }

    fn apply_control(&mut self, request: ControlRequest) -> CommandResult {
        let result = match request.command {
            ControlCommand::EmergencyDisable => {
                self.controller.emergency_disable();
                Ok(CommandResult::Ok)
            }
            ControlCommand::SetPortPolicy { port, policy } => self
                .controller
                .set_port_policy(port, policy)
                .map(|_| CommandResult::Ok),
            ControlCommand::AcknowledgeEmergencyResolved => self
                .controller
                .acknowledge_emergency_resolved()
                .map(|_| CommandResult::Ok),
            ControlCommand::RequestStatus { port } => {
                if self.board.supports(port) {
                    Ok(CommandResult::Ok)
                } else {
                    Err(ControllerError::UnsupportedPort(port))
                }
            }
            ControlCommand::SetFanConfig { config } => self
                .controller
                .set_fan_config(config)
                .map(|_| CommandResult::Ok),
        };
        let result = result.unwrap_or_else(map_controller_error);
        self.execute_actions();
        result
    }

    fn handle_commissioning(&mut self, id: ExtendedId, payload: &[u8]) -> Vec<WireFrame> {
        let Ok((header, message)) = decode_commissioning(id, payload) else {
            return Vec::new();
        };
        if commissioning_request_identity(message).is_some()
            && !commissioning_targets_uid(message, self.commissioning.uid())
        {
            return Vec::new();
        }
        if let Some(replayed) = self.replay_commissioning(header.requester, message) {
            return replayed;
        }
        let response = match message {
            CommissioningMessage::Discover { nonce } => {
                Some(CommissioningMessage::DiscoveryResponse {
                    nonce,
                    info: pdcan_protocol::DiscoveryInfo {
                        uid: self.commissioning.uid(),
                        node_id: self.commissioning.node_id(),
                        state: self.commissioning.state(),
                        protocol_major: pdcan_protocol::PROTOCOL_MAJOR,
                        protocol_minor: pdcan_protocol::PROTOCOL_MINOR,
                        firmware: FirmwareVersion {
                            major: 0,
                            minor: 1,
                            patch: 0,
                        },
                        hardware_revision: self.board.hardware_revision as u8,
                        supported_ports: self.board.supported_ports.bits(),
                    },
                })
            }
            CommissioningMessage::Identify {
                request_id,
                uid,
                duration_seconds,
            } if uid == self.commissioning.uid() => {
                self.identify_duration_seconds = duration_seconds;
                Some(self.commissioning_result(
                    request_id,
                    commissioning_opcode::IDENTIFY,
                    CommandResult::Ok,
                ))
            }
            CommissioningMessage::AssignNode {
                request_id,
                uid,
                node_id,
            } if uid == self.commissioning.uid() => {
                self.controller.set_node_id(Some(node_id));
                self.execute_actions();
                self.commissioning.apply_persisted_node_id(Some(node_id));
                self.commissioning.claim_window_complete();
                Some(self.commissioning_result(
                    request_id,
                    commissioning_opcode::ASSIGN_NODE,
                    CommandResult::Ok,
                ))
            }
            CommissioningMessage::ClearNode { request_id, uid }
                if uid == self.commissioning.uid() =>
            {
                self.controller.set_node_id(None);
                self.execute_actions();
                self.commissioning.apply_persisted_node_id(None);
                Some(self.commissioning_result(
                    request_id,
                    commissioning_opcode::CLEAR_NODE,
                    CommandResult::Ok,
                ))
            }
            CommissioningMessage::NodeClaim {
                uid,
                node_id,
                state: _,
                claim_nonce: _,
            } => {
                self.commissioning.observe_claim(uid, node_id);
                None
            }
            _ => None,
        };
        if let Some(response) = response
            && commissioning_request_identity(message).is_some()
        {
            self.remember_commissioning(header.requester, message, response);
        }
        response
            .and_then(|message| encode_commissioning(header.requester, message).ok())
            .into_iter()
            .collect()
    }

    fn replay_commissioning(
        &self,
        requester: RequesterId,
        request: CommissioningMessage,
    ) -> Option<Vec<WireFrame>> {
        let (request_id, opcode) = commissioning_request_identity(request)?;
        let cached = self.commissioning_request_cache.iter().find(|cached| {
            cached.requester == requester
                && commissioning_request_identity(cached.request)
                    .is_some_and(|(cached_id, _)| cached_id == request_id)
        })?;
        let response = if cached.request == request {
            cached.response
        } else {
            self.commissioning_result(request_id, opcode, CommandResult::InvalidArgument)
        };
        Some(
            encode_commissioning(requester, response)
                .ok()
                .into_iter()
                .collect(),
        )
    }

    fn commissioning_result(
        &self,
        request_id: RequestId,
        request_opcode: u8,
        result: CommandResult,
    ) -> CommissioningMessage {
        CommissioningMessage::Result {
            request_id,
            uid: self.commissioning.uid(),
            request_opcode,
            result,
            node_id: self.commissioning.node_id(),
            state: self.commissioning.state(),
        }
    }

    fn execute_actions(&mut self) {
        while let Some(action) = self.controller.next_action() {
            match action {
                Action::Pd {
                    operation,
                    port,
                    slot_epoch,
                    ..
                } => self.controller.complete_pd_operation(
                    port,
                    operation,
                    slot_epoch,
                    CompletionOutcome::Succeeded,
                ),
                Action::PersistConfig { revision, settings } => {
                    let outcome = if self.persistence_failures_remaining == 0 {
                        self.durable_settings = settings;
                        CompletionOutcome::Succeeded
                    } else {
                        self.persistence_failures_remaining -= 1;
                        CompletionOutcome::Failed
                    };
                    let completion = self.controller.complete_persist(revision, outcome);
                    if outcome == CompletionOutcome::Succeeded {
                        debug_assert_eq!(completion, PersistCompletion::DurableThrough(revision));
                    }
                }
            }
        }
    }

    fn make_response(
        &self,
        request: ControlRequest,
        result: CommandResult,
        detail: u32,
    ) -> CommandResponse {
        CommandResponse {
            node: self.commissioning.node_id().map_or(0, NodeId::get),
            requester: request.requester,
            request_id: request.request_id,
            request_opcode: request.command.opcode(),
            request_target: request.command.target(),
            result,
            detail,
        }
    }

    fn encode_operational_response(response: CommandResponse) -> Vec<WireFrame> {
        encode_command_response(response).ok().into_iter().collect()
    }

    fn remember(&mut self, request: ControlRequest, response: CommandResponse) {
        if self.request_cache.len() == REQUEST_CACHE_CAPACITY {
            self.request_cache.pop_front();
        }
        self.request_cache
            .push_back(CachedRequest { request, response });
    }

    fn remember_commissioning(
        &mut self,
        requester: RequesterId,
        request: CommissioningMessage,
        response: CommissioningMessage,
    ) {
        if self.commissioning_request_cache.len() == REQUEST_CACHE_CAPACITY {
            self.commissioning_request_cache.pop_front();
        }
        self.commissioning_request_cache
            .push_back(CachedCommissioningRequest {
                requester,
                request,
                response,
            });
    }

    fn port_state_frame(&mut self, port: PortId) -> Option<WireFrame> {
        let node = self.commissioning.node_id()?.get();
        let policy = self.controller.settings().port_policy[port.index()];
        let mut flags = 0;
        if policy.enabled {
            flags |= PortStateFlags::ENABLED;
        }
        if self.online_ports.contains(port) {
            flags |= PortStateFlags::MODULE_PRESENT;
        }
        if self.controller.emergency_latched() {
            flags |= PortStateFlags::EMERGENCY_LATCHED;
        }
        self.state_sequence = self.state_sequence.wrapping_add(1);
        encode_port_state(PortState {
            node,
            port,
            sequence: self.state_sequence,
            flags: PortStateFlags::from_bits(flags),
            faults: FaultFlags::from_bits(0),
            uptime_seconds: 0,
            active_profile: u8::MAX,
            slot_generation: self.controller.slot_epoch(port)?.0.to_le_bytes()[0],
        })
        .ok()
    }
}

pub fn simulator_board(port_count: u8) -> Result<BoardDefinition, String> {
    if port_count != 6 && port_count != 8 {
        return Err("port count must be 6 or 8".into());
    }
    let supported_bits = if port_count == 8 { 0xFF } else { 0x3F };
    Ok(BoardDefinition {
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
        default_fan_mode: pdcan_types::FanMode::ThreeWire,
    })
}

fn map_controller_error(error: ControllerError) -> CommandResult {
    match error {
        ControllerError::UnsupportedPort(_) => CommandResult::InvalidTarget,
        ControllerError::InvalidPolicy(_) | ControllerError::InvalidFanDuty(_) => {
            CommandResult::InvalidArgument
        }
        ControllerError::EmergencyLatched => CommandResult::EmergencyLatched,
        ControllerError::EmergencyAlreadyClear => CommandResult::InvalidArgument,
        ControllerError::EmergencyClearInProgress => CommandResult::Busy,
    }
}

const fn commissioning_request_identity(message: CommissioningMessage) -> Option<(RequestId, u8)> {
    match message {
        CommissioningMessage::Identify { request_id, .. } => {
            Some((request_id, commissioning_opcode::IDENTIFY))
        }
        CommissioningMessage::AssignNode { request_id, .. } => {
            Some((request_id, commissioning_opcode::ASSIGN_NODE))
        }
        CommissioningMessage::ClearNode { request_id, .. } => {
            Some((request_id, commissioning_opcode::CLEAR_NODE))
        }
        _ => None,
    }
}

fn commissioning_targets_uid(message: CommissioningMessage, uid: NodeUid) -> bool {
    match message {
        CommissioningMessage::Identify {
            uid: target_uid, ..
        }
        | CommissioningMessage::AssignNode {
            uid: target_uid, ..
        }
        | CommissioningMessage::ClearNode {
            uid: target_uid, ..
        } => target_uid == uid,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_protocol::{
        ControlCommand, ControlRequest, decode_command_response, encode_control_request,
    };
    use pdcan_types::{PortPolicy, RequestId};

    const UID: NodeUid = NodeUid::from_bytes([0x11; NodeUid::LENGTH]);

    fn node(port_count: u8) -> SimulatedNode {
        SimulatedNode::new(port_count, UID, Some(NodeId::new(3).unwrap())).unwrap()
    }

    fn send(node: &mut SimulatedNode, request: ControlRequest) -> CommandResponse {
        let frame = encode_control_request(request).unwrap();
        let responses = node.handle_frame(frame.id(), frame.payload());
        assert_eq!(responses.len(), 1);
        decode_command_response(responses[0].id(), responses[0].payload()).unwrap()
    }

    #[test]
    fn six_and_eight_port_models_have_distinct_capability_masks() {
        assert_eq!(node(6).board().supported_ports.bits(), 0x3f);
        assert_eq!(node(8).board().supported_ports.bits(), 0xff);
    }

    #[test]
    fn unsupported_rev_a_logical_port_is_rejected() {
        let mut node = node(6);
        let response = send(
            &mut node,
            ControlRequest {
                node: 3,
                requester: RequesterId::PDCAN_DEFAULT,
                request_id: RequestId(1),
                command: ControlCommand::RequestStatus {
                    port: PortId::new(6).unwrap(),
                },
            },
        );
        assert_eq!(response.result, CommandResult::InvalidTarget);
    }

    #[test]
    fn emergency_latch_and_clear_survive_simulated_power_cycles() {
        let mut node = node(6);
        let emergency = send(
            &mut node,
            ControlRequest {
                node: 3,
                requester: RequesterId::PDCAN_DEFAULT,
                request_id: RequestId(2),
                command: ControlCommand::EmergencyDisable,
            },
        );
        assert_eq!(emergency.result, CommandResult::Ok);
        let mut restarted = node.restart();
        assert!(restarted.settings().emergency_latched);

        restarted.fail_next_persistence_attempts(2);
        let clear = send(
            &mut restarted,
            ControlRequest {
                node: 3,
                requester: RequesterId::PDCAN_DEFAULT,
                request_id: RequestId(3),
                command: ControlCommand::AcknowledgeEmergencyResolved,
            },
        );
        assert_eq!(clear.result, CommandResult::Ok);
        assert!(!restarted.restart().settings().emergency_latched);
    }

    #[test]
    fn exact_duplicate_replays_response_but_request_id_reuse_is_rejected() {
        let mut node = node(6);
        let request = ControlRequest {
            node: 3,
            requester: RequesterId::new(4).unwrap(),
            request_id: RequestId(99),
            command: ControlCommand::SetPortPolicy {
                port: PortId::new(0).unwrap(),
                policy: PortPolicy::SAFE_DISABLED,
            },
        };
        let first = send(&mut node, request);
        let duplicate = send(&mut node, request);
        assert_eq!(duplicate, first);

        let conflict = send(
            &mut node,
            ControlRequest {
                command: ControlCommand::RequestStatus {
                    port: PortId::new(0).unwrap(),
                },
                ..request
            },
        );
        assert_eq!(conflict.result, CommandResult::InvalidArgument);
        assert_eq!(conflict.detail, 1);
    }

    #[test]
    fn conflicting_claim_preserves_node_id_and_suppresses_commands() {
        let mut node = node(6);
        let claim = encode_commissioning(
            RequesterId::PROTOCOL,
            CommissioningMessage::NodeClaim {
                uid: NodeUid::from_bytes([0x22; NodeUid::LENGTH]),
                node_id: NodeId::new(3).unwrap(),
                state: CommissioningState::Claiming,
                claim_nonce: 0,
            },
        )
        .unwrap();
        assert!(node.handle_frame(claim.id(), claim.payload()).is_empty());
        assert_eq!(
            node.commissioning().state(),
            CommissioningState::AddressConflict
        );
        assert_eq!(node.settings().node_id, Some(NodeId::new(3).unwrap()));

        let request = encode_control_request(ControlRequest {
            node: 3,
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(4),
            command: ControlCommand::RequestStatus {
                port: PortId::new(0).unwrap(),
            },
        })
        .unwrap();
        assert!(
            node.handle_frame(request.id(), request.payload())
                .is_empty()
        );
    }

    #[test]
    fn discovery_and_uid_assignment_work_while_uncommissioned() {
        let mut node = SimulatedNode::new(8, UID, None).unwrap();
        let discover = encode_commissioning(
            RequesterId::PDCAN_DEFAULT,
            CommissioningMessage::Discover { nonce: 123 },
        )
        .unwrap();
        let response = node.handle_frame(discover.id(), discover.payload());
        assert_eq!(response.len(), 1);
        let (_, CommissioningMessage::DiscoveryResponse { info, .. }) =
            decode_commissioning(response[0].id(), response[0].payload()).unwrap()
        else {
            panic!("expected discovery response");
        };
        assert_eq!(info.node_id, None);
        assert_eq!(info.supported_ports, 0xff);

        let assign = encode_commissioning(
            RequesterId::PDCAN_DEFAULT,
            CommissioningMessage::AssignNode {
                request_id: RequestId(5),
                uid: UID,
                node_id: NodeId::new(8).unwrap(),
            },
        )
        .unwrap();
        let response = node.handle_frame(assign.id(), assign.payload());
        assert_eq!(response.len(), 1);
        assert_eq!(node.settings().node_id, Some(NodeId::new(8).unwrap()));
        assert_eq!(
            node.commissioning().state(),
            CommissioningState::Commissioned
        );
    }

    #[test]
    fn commissioning_retries_replay_and_request_id_reuse_is_rejected() {
        let mut node = SimulatedNode::new(8, UID, None).unwrap();
        let requester = RequesterId::new(4).unwrap();
        let request = CommissioningMessage::AssignNode {
            request_id: RequestId(50),
            uid: UID,
            node_id: NodeId::new(8).unwrap(),
        };
        let frame = encode_commissioning(requester, request).unwrap();
        let first = node.handle_frame(frame.id(), frame.payload());
        let replay = node.handle_frame(frame.id(), frame.payload());
        assert_eq!(replay, first);

        let conflicting = encode_commissioning(
            requester,
            CommissioningMessage::ClearNode {
                request_id: RequestId(50),
                uid: UID,
            },
        )
        .unwrap();
        let response = node.handle_frame(conflicting.id(), conflicting.payload());
        assert!(matches!(
            decode_commissioning(response[0].id(), response[0].payload())
                .unwrap()
                .1,
            CommissioningMessage::Result {
                result: CommandResult::InvalidArgument,
                ..
            }
        ));
        assert_eq!(node.settings().node_id, Some(NodeId::new(8).unwrap()));

        let foreign = encode_commissioning(
            requester,
            CommissioningMessage::ClearNode {
                request_id: RequestId(50),
                uid: NodeUid::from_bytes([0x77; NodeUid::LENGTH]),
            },
        )
        .unwrap();
        assert!(
            node.handle_frame(foreign.id(), foreign.payload())
                .is_empty()
        );
    }

    #[test]
    fn simulator_capacity_constant_remains_eight() {
        assert_eq!(pdcan_types::MAX_PORTS, 8);
    }
}
