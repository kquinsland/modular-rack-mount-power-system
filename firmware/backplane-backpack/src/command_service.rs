use pdcan_core::{
    Commissioning, Controller, ControllerError, Mutation, PersistCompletion, revision_covers,
};
use pdcan_protocol::{
    CommandResponse, CommandResult, CommissioningMessage, ControlCommand, ControlRequest,
    DiscoveryInfo, Heartbeat, PortState, PortStateFlags, ResetFlags, WireFrame,
    commissioning_opcode, encode_command_response, encode_commissioning, encode_heartbeat,
    encode_port_state,
};
use pdcan_types::{
    BoardDefinition, CommissioningState, ConfigRevision, FaultFlags, FirmwareVersion, NodeId,
    NodeUid, RequestId, RequesterId,
};

pub const MAX_PENDING_RESPONSES: usize = 8;
pub const RESPONSE_BATCH_CAPACITY: usize = MAX_PENDING_RESPONSES + 2;
const COMPLETED_CACHE_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingKind {
    Control {
        request: ControlRequest,
        result: CommandResult,
    },
    AssignNode {
        requester: RequesterId,
        request_id: RequestId,
        node_id: NodeId,
    },
    ClearNode {
        requester: RequesterId,
        request_id: RequestId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingResponse {
    required_revision: ConfigRevision,
    kind: PendingKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletedControl {
    request: ControlRequest,
    response: CommandResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletedCommissioning {
    requester: RequesterId,
    request: CommissioningMessage,
    response: CommissioningMessage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceOutput {
    pub frames: [Option<WireFrame>; RESPONSE_BATCH_CAPACITY],
    pub identify_seconds: Option<u16>,
}

impl ServiceOutput {
    pub const EMPTY: Self = Self {
        frames: [None; RESPONSE_BATCH_CAPACITY],
        identify_seconds: None,
    };

    fn push(&mut self, frame: WireFrame) {
        if let Some(slot) = self.frames.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(frame);
        }
    }

    pub fn merge(&mut self, other: Self) {
        for frame in other.frames.into_iter().flatten() {
            self.push(frame);
        }
        if other.identify_seconds.is_some() {
            self.identify_seconds = other.identify_seconds;
        }
    }
}

pub struct CommandService {
    board: BoardDefinition,
    firmware: FirmwareVersion,
    commissioning: Commissioning,
    pending: [Option<PendingResponse>; MAX_PENDING_RESPONSES],
    completed: [Option<CompletedControl>; COMPLETED_CACHE_CAPACITY],
    completed_cursor: usize,
    completed_commissioning: [Option<CompletedCommissioning>; COMPLETED_CACHE_CAPACITY],
    completed_commissioning_cursor: usize,
    state_sequence: u16,
    uptime_seconds: u32,
    reset_flags: ResetFlags,
}

impl CommandService {
    pub const fn new(
        board: BoardDefinition,
        firmware: FirmwareVersion,
        uid: NodeUid,
        node_id: Option<NodeId>,
    ) -> Self {
        Self {
            board,
            firmware,
            commissioning: Commissioning::new(uid, node_id),
            pending: [None; MAX_PENDING_RESPONSES],
            completed: [None; COMPLETED_CACHE_CAPACITY],
            completed_cursor: 0,
            completed_commissioning: [None; COMPLETED_CACHE_CAPACITY],
            completed_commissioning_cursor: 0,
            state_sequence: 0,
            uptime_seconds: 0,
            reset_flags: ResetFlags::from_bits(0),
        }
    }

    pub const fn commissioning(&self) -> Commissioning {
        self.commissioning
    }

    pub fn set_uptime_seconds(&mut self, uptime_seconds: u32) {
        self.uptime_seconds = uptime_seconds;
    }

    pub fn set_reset_flags(&mut self, reset_flags: ResetFlags) {
        self.reset_flags = reset_flags;
    }

    pub fn startup_claim(&self) -> ServiceOutput {
        self.claim_round(0)
    }

    pub fn claim_round(&self, claim_nonce: u16) -> ServiceOutput {
        let mut output = ServiceOutput::EMPTY;
        if let Some(frame) = self.claim_frame(claim_nonce) {
            output.push(frame);
        }
        output
    }

    pub fn claim_window_complete(&mut self) {
        self.commissioning.claim_window_complete();
    }

    pub fn handle_control(
        &mut self,
        controller: &mut Controller,
        request: ControlRequest,
    ) -> ServiceOutput {
        let mut output = ServiceOutput::EMPTY;
        let broadcast_emergency =
            request.node == 0 && matches!(request.command, ControlCommand::EmergencyDisable);
        let addressed = self.commissioning.normal_traffic_allowed()
            && self
                .commissioning
                .node_id()
                .is_some_and(|node| node.get() == request.node);
        if !addressed && !broadcast_emergency {
            return output;
        }

        if let Some(completed) = self
            .completed
            .iter()
            .flatten()
            .find(|entry| {
                entry.request.requester == request.requester
                    && entry.request.request_id == request.request_id
            })
            .copied()
        {
            let exact_retry = completed.request == request;
            let response = if exact_retry {
                completed.response
            } else {
                self.control_response(request, CommandResult::InvalidArgument, 1)
            };
            self.push_operational(&mut output, response);
            if exact_retry
                && response.result == CommandResult::Ok
                && let ControlCommand::RequestStatus { port } = request.command
                && let Some(frame) = self.port_state_frame(controller, port)
            {
                output.push(frame);
            }
            return output;
        }
        if let Some(pending) = self.pending.iter().flatten().find(|entry| {
            matches!(
                entry.kind,
                PendingKind::Control { request: pending, .. }
                    if pending.requester == request.requester
                        && pending.request_id == request.request_id
            )
        }) {
            if !matches!(pending.kind, PendingKind::Control { request: pending, .. } if pending == request)
            {
                let response = self.control_response(request, CommandResult::InvalidArgument, 1);
                self.push_operational(&mut output, response);
            }
            return output;
        }

        let serialized_persistent = matches!(
            request.command,
            ControlCommand::SetPortPolicy { .. }
                | ControlCommand::AcknowledgeEmergencyResolved
                | ControlCommand::SetFanConfig { .. }
        );
        if serialized_persistent && self.pending.iter().any(Option::is_some) {
            let response = self.control_response(request, CommandResult::Busy, 0);
            self.push_operational(&mut output, response);
            return output;
        }

        self.apply_new_control(controller, request)
    }

    fn apply_new_control(
        &mut self,
        controller: &mut Controller,
        request: ControlRequest,
    ) -> ServiceOutput {
        let mut output = ServiceOutput::EMPTY;
        let (mutation, result) = match request.command {
            ControlCommand::EmergencyDisable => {
                let mutation = controller.emergency_disable();
                self.cancel_pending_emergency_clear(&mut output);
                (Ok(mutation), CommandResult::Ok)
            }
            ControlCommand::SetPortPolicy { port, policy } => (
                controller.set_port_policy(port, policy),
                CommandResult::OkPending,
            ),
            ControlCommand::AcknowledgeEmergencyResolved => (
                controller
                    .acknowledge_emergency_resolved()
                    .map(|revision| Mutation {
                        persist_revision: Some(revision),
                    }),
                CommandResult::Ok,
            ),
            ControlCommand::RequestStatus { port } => {
                let result = if self.board.supports(port) {
                    CommandResult::Ok
                } else {
                    CommandResult::InvalidTarget
                };
                let response = self.control_response(request, result, 0);
                self.complete_control(request, response);
                self.push_operational(&mut output, response);
                if result == CommandResult::Ok
                    && let Some(frame) = self.port_state_frame(controller, port)
                {
                    output.push(frame);
                }
                return output;
            }
            ControlCommand::SetFanConfig { config } => {
                (controller.set_fan_config(config), CommandResult::Ok)
            }
        };

        match mutation {
            Ok(Mutation {
                persist_revision: Some(required_revision),
            }) => {
                self.enqueue(PendingResponse {
                    required_revision,
                    kind: PendingKind::Control { request, result },
                });
            }
            Ok(Mutation {
                persist_revision: None,
            }) => {
                let response = self.control_response(request, result, 0);
                self.complete_control(request, response);
                self.push_operational(&mut output, response);
            }
            Err(error) => {
                let response = self.control_response(request, map_controller_error(error), 0);
                self.complete_control(request, response);
                self.push_operational(&mut output, response);
            }
        }
        output
    }

    pub fn handle_commissioning(
        &mut self,
        controller: &mut Controller,
        requester: RequesterId,
        message: CommissioningMessage,
    ) -> ServiceOutput {
        if commissioning_request_identity(message).is_some()
            && !commissioning_targets_uid(message, self.commissioning.uid())
        {
            return ServiceOutput::EMPTY;
        }
        if let Some(replayed) = self.replay_commissioning(requester, message) {
            return replayed;
        }
        let mut output = ServiceOutput::EMPTY;
        match message {
            CommissioningMessage::Discover { nonce } => {
                let response = CommissioningMessage::DiscoveryResponse {
                    nonce,
                    info: DiscoveryInfo {
                        uid: self.commissioning.uid(),
                        node_id: self.commissioning.node_id(),
                        state: self.commissioning.state(),
                        protocol_major: pdcan_protocol::PROTOCOL_MAJOR,
                        protocol_minor: pdcan_protocol::PROTOCOL_MINOR,
                        firmware: self.firmware,
                        hardware_revision: self.board.hardware_revision as u8,
                        supported_ports: self.board.supported_ports.bits(),
                    },
                };
                Self::push_commissioning(&mut output, requester, response);
            }
            CommissioningMessage::Identify {
                request_id,
                uid,
                duration_seconds,
            } if uid == self.commissioning.uid() => {
                output.identify_seconds = Some(duration_seconds);
                let result = self.commissioning_result(
                    request_id,
                    commissioning_opcode::IDENTIFY,
                    CommandResult::Ok,
                );
                Self::push_commissioning(&mut output, requester, result);
                self.complete_commissioning(requester, message, result);
            }
            CommissioningMessage::AssignNode {
                request_id,
                uid,
                node_id,
            } if uid == self.commissioning.uid() => {
                return self.handle_assign_node(controller, requester, request_id, node_id);
            }
            CommissioningMessage::ClearNode { request_id, uid }
                if uid == self.commissioning.uid() =>
            {
                return self.handle_clear_node(controller, requester, request_id);
            }
            CommissioningMessage::NodeClaim {
                uid,
                node_id,
                state: _,
                claim_nonce,
            } => {
                let should_answer = uid != self.commissioning.uid()
                    && self.commissioning.node_id() == Some(node_id)
                    && self.commissioning.state() != CommissioningState::AddressConflict;
                self.commissioning.observe_claim(uid, node_id);
                if should_answer && let Some(claim) = self.claim_frame(claim_nonce.wrapping_add(1))
                {
                    output.push(claim);
                }
            }
            _ => {}
        }
        output
    }

    fn replay_commissioning(
        &self,
        requester: RequesterId,
        request: CommissioningMessage,
    ) -> Option<ServiceOutput> {
        let (request_id, opcode) = commissioning_request_identity(request)?;
        if let Some(completed) = self.completed_commissioning.iter().flatten().find(|entry| {
            entry.requester == requester
                && commissioning_request_identity(entry.request)
                    .is_some_and(|(completed_id, _)| completed_id == request_id)
        }) {
            let mut output = ServiceOutput::EMPTY;
            let response = if completed.request == request {
                completed.response
            } else {
                self.commissioning_result(request_id, opcode, CommandResult::InvalidArgument)
            };
            Self::push_commissioning(&mut output, requester, response);
            return Some(output);
        }
        if let Some(pending) = self.pending.iter().flatten().find(|entry| {
            pending_commissioning_identity(entry.kind).is_some_and(
                |(pending_requester, pending_id)| {
                    pending_requester == requester && pending_id == request_id
                },
            )
        }) {
            if pending_matches_commissioning(pending.kind, request, self.commissioning.uid()) {
                return Some(ServiceOutput::EMPTY);
            }
            return Some(self.commissioning_error(
                requester,
                request_id,
                opcode,
                CommandResult::InvalidArgument,
            ));
        }
        None
    }

    fn handle_assign_node(
        &mut self,
        controller: &mut Controller,
        requester: RequesterId,
        request_id: RequestId,
        node_id: NodeId,
    ) -> ServiceOutput {
        if self.pending.iter().any(Option::is_some) {
            return self.commissioning_error(
                requester,
                request_id,
                commissioning_opcode::ASSIGN_NODE,
                CommandResult::Busy,
            );
        }
        let mut output = ServiceOutput::EMPTY;
        let mutation = controller.set_node_id(Some(node_id));
        if let Some(required_revision) = mutation.persist_revision {
            self.enqueue(PendingResponse {
                required_revision,
                kind: PendingKind::AssignNode {
                    requester,
                    request_id,
                    node_id,
                },
            });
        } else {
            self.commissioning.apply_persisted_node_id(Some(node_id));
            let result = self.commissioning_result(
                request_id,
                commissioning_opcode::ASSIGN_NODE,
                CommandResult::Ok,
            );
            Self::push_commissioning(&mut output, requester, result);
            self.complete_commissioning(
                requester,
                CommissioningMessage::AssignNode {
                    request_id,
                    uid: self.commissioning.uid(),
                    node_id,
                },
                result,
            );
            if let Some(claim) = self.claim_frame(0) {
                output.push(claim);
            }
        }
        output
    }

    fn handle_clear_node(
        &mut self,
        controller: &mut Controller,
        requester: RequesterId,
        request_id: RequestId,
    ) -> ServiceOutput {
        if self.pending.iter().any(Option::is_some) {
            return self.commissioning_error(
                requester,
                request_id,
                commissioning_opcode::CLEAR_NODE,
                CommandResult::Busy,
            );
        }
        let mut output = ServiceOutput::EMPTY;
        let mutation = controller.set_node_id(None);
        if let Some(required_revision) = mutation.persist_revision {
            self.enqueue(PendingResponse {
                required_revision,
                kind: PendingKind::ClearNode {
                    requester,
                    request_id,
                },
            });
        } else {
            self.commissioning.apply_persisted_node_id(None);
            let result = self.commissioning_result(
                request_id,
                commissioning_opcode::CLEAR_NODE,
                CommandResult::Ok,
            );
            Self::push_commissioning(&mut output, requester, result);
            self.complete_commissioning(
                requester,
                CommissioningMessage::ClearNode {
                    request_id,
                    uid: self.commissioning.uid(),
                },
                result,
            );
        }
        output
    }

    fn commissioning_error(
        &self,
        requester: RequesterId,
        request_id: RequestId,
        opcode: u8,
        result: CommandResult,
    ) -> ServiceOutput {
        let mut output = ServiceOutput::EMPTY;
        let result = self.commissioning_result(request_id, opcode, result);
        Self::push_commissioning(&mut output, requester, result);
        output
    }

    pub fn complete_persist(&mut self, completion: PersistCompletion) -> ServiceOutput {
        let mut output = ServiceOutput::EMPTY;
        let PersistCompletion::DurableThrough(durable) = completion else {
            return output;
        };
        for index in 0..self.pending.len() {
            let Some(pending) = self.pending[index] else {
                continue;
            };
            if !revision_covers(durable, pending.required_revision) {
                continue;
            }
            self.pending[index] = None;
            match pending.kind {
                PendingKind::Control { request, result } => {
                    let response = self.control_response(request, result, 0);
                    self.complete_control(request, response);
                    self.push_operational(&mut output, response);
                }
                PendingKind::AssignNode {
                    requester,
                    request_id,
                    node_id,
                } => {
                    self.commissioning.apply_persisted_node_id(Some(node_id));
                    let result = self.commissioning_result(
                        request_id,
                        commissioning_opcode::ASSIGN_NODE,
                        CommandResult::Ok,
                    );
                    Self::push_commissioning(&mut output, requester, result);
                    self.complete_commissioning(
                        requester,
                        CommissioningMessage::AssignNode {
                            request_id,
                            uid: self.commissioning.uid(),
                            node_id,
                        },
                        result,
                    );
                    if let Some(claim) = self.claim_frame(0) {
                        output.push(claim);
                    }
                }
                PendingKind::ClearNode {
                    requester,
                    request_id,
                } => {
                    self.commissioning.apply_persisted_node_id(None);
                    let result = self.commissioning_result(
                        request_id,
                        commissioning_opcode::CLEAR_NODE,
                        CommandResult::Ok,
                    );
                    Self::push_commissioning(&mut output, requester, result);
                    self.complete_commissioning(
                        requester,
                        CommissioningMessage::ClearNode {
                            request_id,
                            uid: self.commissioning.uid(),
                        },
                        result,
                    );
                }
            }
        }
        output
    }

    pub fn heartbeat(&self, controller: &Controller) -> ServiceOutput {
        let mut output = ServiceOutput::EMPTY;
        let Some(node_id) = self.commissioning.node_id() else {
            return output;
        };
        if !self.commissioning.normal_traffic_allowed() {
            return output;
        }
        let heartbeat = Heartbeat {
            node: node_id.get(),
            protocol_major: pdcan_protocol::PROTOCOL_MAJOR,
            protocol_minor: pdcan_protocol::PROTOCOL_MINOR,
            firmware: self.firmware,
            hardware_revision: self.board.hardware_revision as u8,
            health_flags: 0,
            supported_ports: self.board.supported_ports.bits(),
            online_ports: controller.online_ports().bits(),
            enabled_ports: controller.enabled_ports().bits(),
            faulted_ports: 0,
            emergency_latched: controller.emergency_latched(),
            commissioning_state: self.commissioning.state(),
            reset_flags: self.reset_flags,
            uptime_seconds: self.uptime_seconds,
        };
        if let Ok(frame) = encode_heartbeat(heartbeat) {
            output.push(frame);
        }
        output
    }

    fn enqueue(&mut self, pending: PendingResponse) {
        if let Some(slot) = self.pending.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(pending);
        }
    }

    fn cancel_pending_emergency_clear(&mut self, output: &mut ServiceOutput) {
        for index in 0..self.pending.len() {
            let Some(PendingResponse {
                kind:
                    PendingKind::Control {
                        request:
                            request @ ControlRequest {
                                command: ControlCommand::AcknowledgeEmergencyResolved,
                                ..
                            },
                        ..
                    },
                ..
            }) = self.pending[index]
            else {
                continue;
            };
            self.pending[index] = None;
            let response = self.control_response(request, CommandResult::EmergencyLatched, 0);
            self.complete_control(request, response);
            self.push_operational(output, response);
        }
    }

    fn complete_control(&mut self, request: ControlRequest, response: CommandResponse) {
        self.completed[self.completed_cursor] = Some(CompletedControl { request, response });
        self.completed_cursor = (self.completed_cursor + 1) % COMPLETED_CACHE_CAPACITY;
    }

    fn complete_commissioning(
        &mut self,
        requester: RequesterId,
        request: CommissioningMessage,
        response: CommissioningMessage,
    ) {
        self.completed_commissioning[self.completed_commissioning_cursor] =
            Some(CompletedCommissioning {
                requester,
                request,
                response,
            });
        self.completed_commissioning_cursor =
            (self.completed_commissioning_cursor + 1) % COMPLETED_CACHE_CAPACITY;
    }

    fn control_response(
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

    fn port_state_frame(
        &mut self,
        controller: &Controller,
        port: pdcan_types::PortId,
    ) -> Option<WireFrame> {
        let node = self.commissioning.node_id()?.get();
        let online = controller.port_online(port)?;
        let policy = controller.settings().port_policy[port.index()];
        let mut flags = 0;
        if policy.enabled {
            flags |= PortStateFlags::ENABLED;
        }
        if online {
            flags |= PortStateFlags::MODULE_PRESENT;
        }
        if controller.port_policy_pending(port)? {
            flags |= PortStateFlags::POLICY_PENDING;
        }
        if controller.emergency_latched() {
            flags |= PortStateFlags::EMERGENCY_LATCHED;
        }
        if controller.port_powered(port)? {
            flags |= PortStateFlags::INPUT_POWERED;
        }
        self.state_sequence = self.state_sequence.wrapping_add(1);
        let generation = controller.slot_epoch(port)?.0.to_le_bytes()[0];
        encode_port_state(PortState {
            node,
            port,
            sequence: self.state_sequence,
            flags: PortStateFlags::from_bits(flags),
            faults: FaultFlags::from_bits(0),
            uptime_seconds: self.uptime_seconds,
            active_profile: u8::MAX,
            slot_generation: generation,
        })
        .ok()
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

    fn claim_frame(&self, claim_nonce: u16) -> Option<WireFrame> {
        let node_id = self.commissioning.node_id()?;
        encode_commissioning(
            RequesterId::PROTOCOL,
            CommissioningMessage::NodeClaim {
                uid: self.commissioning.uid(),
                node_id,
                state: self.commissioning.state(),
                claim_nonce,
            },
        )
        .ok()
    }

    fn push_operational(&self, output: &mut ServiceOutput, response: CommandResponse) {
        if self.commissioning.normal_traffic_allowed()
            && let Ok(frame) = encode_command_response(response)
        {
            output.push(frame);
        }
    }

    fn push_commissioning(
        output: &mut ServiceOutput,
        requester: RequesterId,
        message: CommissioningMessage,
    ) {
        if let Ok(frame) = encode_commissioning(requester, message) {
            output.push(frame);
        }
    }
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

const fn pending_commissioning_identity(kind: PendingKind) -> Option<(RequesterId, RequestId)> {
    match kind {
        PendingKind::AssignNode {
            requester,
            request_id,
            ..
        }
        | PendingKind::ClearNode {
            requester,
            request_id,
        } => Some((requester, request_id)),
        PendingKind::Control { .. } => None,
    }
}

fn pending_matches_commissioning(
    kind: PendingKind,
    request: CommissioningMessage,
    uid: NodeUid,
) -> bool {
    match (kind, request) {
        (
            PendingKind::AssignNode {
                requester: _,
                request_id: pending_id,
                node_id: pending_node,
            },
            CommissioningMessage::AssignNode {
                request_id,
                uid: request_uid,
                node_id,
            },
        ) => pending_id == request_id && pending_node == node_id && request_uid == uid,
        (
            PendingKind::ClearNode {
                requester: _,
                request_id: pending_id,
            },
            CommissioningMessage::ClearNode {
                request_id,
                uid: request_uid,
            },
        ) => pending_id == request_id && request_uid == uid,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_core::{Action, CompletionOutcome};
    use pdcan_protocol::{decode_command_response, decode_commissioning, decode_heartbeat};
    use pdcan_types::{
        FanMode, HardwareRevision, PersistentSettings, PortBitmap, PortId, PortPolicy,
    };

    const BOARD: BoardDefinition = BoardDefinition {
        hardware_revision: HardwareRevision::RevA,
        supported_ports: PortBitmap::from_bits(0x3f),
        mux_channel_by_port: [
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            None,
            None,
        ],
        power_gate_bit_by_port: [None; pdcan_types::MAX_PORTS],
        default_fan_mode: FanMode::ThreeWire,
    };
    const UID: NodeUid = NodeUid::from_bytes([0x44; NodeUid::LENGTH]);
    const VERSION: FirmwareVersion = FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    };

    fn setup(node_id: Option<NodeId>) -> (Controller, CommandService) {
        let settings = PersistentSettings {
            node_id,
            ..PersistentSettings::FACTORY_DEFAULT
        };
        (
            Controller::new(BOARD, settings),
            CommandService::new(BOARD, VERSION, UID, node_id),
        )
    }

    fn complete_next_persist(
        controller: &mut Controller,
        service: &mut CommandService,
    ) -> ServiceOutput {
        loop {
            match controller.next_action().expect("pending controller action") {
                Action::Pd {
                    operation,
                    port,
                    slot_epoch,
                    ..
                } => controller.complete_pd_operation(
                    port,
                    operation,
                    slot_epoch,
                    CompletionOutcome::Succeeded,
                ),
                Action::PersistConfig { revision, .. } => {
                    let completion =
                        controller.complete_persist(revision, CompletionOutcome::Succeeded);
                    return service.complete_persist(completion);
                }
                Action::PowerGate {
                    operation,
                    port,
                    slot_epoch,
                    enabled,
                    ..
                } => controller.complete_power_gate_operation(
                    port,
                    operation,
                    slot_epoch,
                    enabled,
                    CompletionOutcome::Succeeded,
                ),
                Action::PowerOutputs { operation, kind } => controller
                    .complete_power_outputs_operation(
                        operation,
                        kind,
                        CompletionOutcome::Succeeded,
                    ),
            }
        }
    }

    #[test]
    fn persistent_command_response_waits_for_durable_completion() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        let request = ControlRequest {
            node: node.get(),
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(1),
            command: ControlCommand::SetPortPolicy {
                port: PortId::new(0).unwrap(),
                policy: PortPolicy {
                    enabled: true,
                    ..PortPolicy::SAFE_DISABLED
                },
            },
        };
        assert_eq!(
            service
                .handle_control(&mut controller, request)
                .frames
                .into_iter()
                .flatten()
                .count(),
            0
        );

        let output = complete_next_persist(&mut controller, &mut service);
        let frame = output.frames.into_iter().flatten().next().unwrap();
        let response = decode_command_response(frame.id(), frame.payload()).unwrap();
        assert_eq!(response.request_id, RequestId(1));
        assert_eq!(response.result, CommandResult::OkPending);
    }

    #[test]
    fn exact_retry_replays_completed_response_without_mutating_again() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        let request = ControlRequest {
            node: node.get(),
            requester: RequesterId::new(4).unwrap(),
            request_id: RequestId(2),
            command: ControlCommand::SetFanConfig {
                config: pdcan_types::FanConfig {
                    mode: FanMode::FourWire,
                    duty_percent: 60,
                },
            },
        };
        assert!(
            service
                .handle_control(&mut controller, request)
                .frames
                .iter()
                .all(Option::is_none)
        );
        let completed = complete_next_persist(&mut controller, &mut service);
        let replay = service.handle_control(&mut controller, request);
        assert_eq!(replay.frames, completed.frames);
        assert!(controller.next_action().is_none());
    }

    #[test]
    fn ordinary_durable_mutations_are_serialized() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        let first = ControlRequest {
            node: node.get(),
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(20),
            command: ControlCommand::SetFanConfig {
                config: pdcan_types::FanConfig {
                    mode: FanMode::FourWire,
                    duty_percent: 60,
                },
            },
        };
        assert!(
            service
                .handle_control(&mut controller, first)
                .frames
                .iter()
                .all(Option::is_none)
        );
        let overlapping = ControlRequest {
            request_id: RequestId(21),
            command: ControlCommand::SetPortPolicy {
                port: PortId::new(0).unwrap(),
                policy: PortPolicy::SAFE_DISABLED,
            },
            ..first
        };
        let busy = service.handle_control(&mut controller, overlapping);
        let response = decode_command_response(
            busy.frames[0].unwrap().id(),
            busy.frames[0].unwrap().payload(),
        )
        .unwrap();
        assert_eq!(response.result, CommandResult::Busy);

        let _ = complete_next_persist(&mut controller, &mut service);
        let accepted = service.handle_control(&mut controller, overlapping);
        assert!(matches!(
            accepted.frames[0].map(|frame| decode_command_response(frame.id(), frame.payload())
                .unwrap()
                .result),
            Some(CommandResult::OkPending)
        ));
    }

    #[test]
    fn a_new_emergency_cancels_an_in_flight_clear_response() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        let latch = ControlRequest {
            node: node.get(),
            requester: RequesterId::PDCAN_DEFAULT,
            request_id: RequestId(22),
            command: ControlCommand::EmergencyDisable,
        };
        assert!(
            service
                .handle_control(&mut controller, latch)
                .frames
                .iter()
                .all(Option::is_none)
        );
        let _ = complete_next_persist(&mut controller, &mut service);

        let clear = ControlRequest {
            request_id: RequestId(23),
            command: ControlCommand::AcknowledgeEmergencyResolved,
            ..latch
        };
        assert!(
            service
                .handle_control(&mut controller, clear)
                .frames
                .iter()
                .all(Option::is_none)
        );
        let Action::PersistConfig {
            revision: clear_revision,
            settings,
        } = controller.next_action().unwrap()
        else {
            panic!("expected the clear write to be in flight")
        };
        assert!(!settings.emergency_latched);

        let relatch = ControlRequest {
            request_id: RequestId(24),
            command: ControlCommand::EmergencyDisable,
            ..latch
        };
        let cancellation = service.handle_control(&mut controller, relatch);
        let cancelled = decode_command_response(
            cancellation.frames[0].unwrap().id(),
            cancellation.frames[0].unwrap().payload(),
        )
        .unwrap();
        assert_eq!(cancelled.request_id, clear.request_id);
        assert_eq!(cancelled.result, CommandResult::EmergencyLatched);

        let stale_clear = service.complete_persist(
            controller.complete_persist(clear_revision, CompletionOutcome::Succeeded),
        );
        assert!(stale_clear.frames.iter().all(Option::is_none));
        assert!(controller.emergency_latched());

        let relatch_completed = complete_next_persist(&mut controller, &mut service);
        let response = decode_command_response(
            relatch_completed.frames[0].unwrap().id(),
            relatch_completed.frames[0].unwrap().payload(),
        )
        .unwrap();
        assert_eq!(response.request_id, relatch.request_id);
        assert_eq!(response.result, CommandResult::Ok);
    }

    #[test]
    fn assignment_result_and_claim_are_emitted_only_after_persistence() {
        let (mut controller, mut service) = setup(None);
        let output = service.handle_commissioning(
            &mut controller,
            RequesterId::PDCAN_DEFAULT,
            CommissioningMessage::AssignNode {
                request_id: RequestId(3),
                uid: UID,
                node_id: NodeId::new(8).unwrap(),
            },
        );
        assert!(output.frames.iter().all(Option::is_none));

        let output = complete_next_persist(&mut controller, &mut service);
        let messages: [Option<CommissioningMessage>; 2] = core::array::from_fn(|index| {
            output.frames[index]
                .map(|frame| decode_commissioning(frame.id(), frame.payload()).unwrap().1)
        });
        assert!(matches!(
            messages[0],
            Some(CommissioningMessage::Result {
                result: CommandResult::Ok,
                ..
            })
        ));
        assert!(matches!(
            messages[1],
            Some(CommissioningMessage::NodeClaim { .. })
        ));
        assert_eq!(
            service.commissioning().state(),
            CommissioningState::Claiming
        );
    }

    #[test]
    fn assignment_retries_wait_for_persistence_and_request_id_reuse_is_rejected() {
        let (mut controller, mut service) = setup(None);
        let requester = RequesterId::new(4).unwrap();
        let request = CommissioningMessage::AssignNode {
            request_id: RequestId(30),
            uid: UID,
            node_id: NodeId::new(8).unwrap(),
        };
        assert!(
            service
                .handle_commissioning(&mut controller, requester, request)
                .frames
                .iter()
                .all(Option::is_none)
        );
        assert!(
            service
                .handle_commissioning(&mut controller, requester, request)
                .frames
                .iter()
                .all(Option::is_none),
            "an exact retry must wait for the original durable completion"
        );

        let conflicting = service.handle_commissioning(
            &mut controller,
            requester,
            CommissioningMessage::AssignNode {
                request_id: RequestId(30),
                uid: UID,
                node_id: NodeId::new(9).unwrap(),
            },
        );
        assert!(matches!(
            conflicting.frames[0]
                .map(|frame| decode_commissioning(frame.id(), frame.payload()).unwrap().1),
            Some(CommissioningMessage::Result {
                result: CommandResult::InvalidArgument,
                ..
            })
        ));

        let completed = complete_next_persist(&mut controller, &mut service);
        let replay = service.handle_commissioning(&mut controller, requester, request);
        assert_eq!(replay.frames[0], completed.frames[0]);
        assert!(controller.next_action().is_none());

        let foreign = service.handle_commissioning(
            &mut controller,
            requester,
            CommissioningMessage::ClearNode {
                request_id: RequestId(30),
                uid: NodeUid::from_bytes([0x99; NodeUid::LENGTH]),
            },
        );
        assert!(foreign.frames.iter().all(Option::is_none));
    }

    #[test]
    fn status_reports_supplied_monotonic_uptime() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        service.set_uptime_seconds(42);
        controller.module_detected(PortId::new(0).unwrap()).unwrap();
        let output = service.handle_control(
            &mut controller,
            ControlRequest {
                node: node.get(),
                requester: RequesterId::PDCAN_DEFAULT,
                request_id: RequestId(31),
                command: ControlCommand::RequestStatus {
                    port: PortId::new(0).unwrap(),
                },
            },
        );
        let state = pdcan_protocol::decode_port_state(
            output.frames[1].unwrap().id(),
            output.frames[1].unwrap().payload(),
        )
        .unwrap();
        assert_eq!(state.uptime_seconds, 42);
        assert_ne!(state.flags.bits() & PortStateFlags::MODULE_PRESENT, 0);
        assert_ne!(state.flags.bits() & PortStateFlags::POLICY_PENDING, 0);
        assert_ne!(state.flags.bits() & PortStateFlags::INPUT_POWERED, 0);
    }

    #[test]
    fn heartbeat_reports_reset_cause_uptime_and_capability_bitmaps() {
        let node = NodeId::new(3).unwrap();
        let (controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        service.set_uptime_seconds(123);
        let reset_flags =
            ResetFlags::from_bits(ResetFlags::POWER | ResetFlags::INDEPENDENT_WATCHDOG);
        service.set_reset_flags(reset_flags);

        let output = service.heartbeat(&controller);
        let frame = output.frames[0].unwrap();
        let heartbeat = decode_heartbeat(frame.id(), frame.payload()).unwrap();
        assert_eq!(heartbeat.node, node.get());
        assert_eq!(heartbeat.supported_ports, 0x3f);
        assert_eq!(heartbeat.reset_flags, reset_flags);
        assert_eq!(heartbeat.uptime_seconds, 123);
    }

    #[test]
    fn reassigning_the_same_persisted_id_recovers_from_conflict() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        let _ = service.handle_commissioning(
            &mut controller,
            RequesterId::PROTOCOL,
            CommissioningMessage::NodeClaim {
                uid: NodeUid::from_bytes([0x55; NodeUid::LENGTH]),
                node_id: node,
                state: CommissioningState::Commissioned,
                claim_nonce: 0,
            },
        );
        assert_eq!(
            service.commissioning().state(),
            CommissioningState::AddressConflict
        );

        let output = service.handle_commissioning(
            &mut controller,
            RequesterId::PDCAN_DEFAULT,
            CommissioningMessage::AssignNode {
                request_id: RequestId(32),
                uid: UID,
                node_id: node,
            },
        );
        assert_eq!(
            service.commissioning().state(),
            CommissioningState::Claiming
        );
        assert!(matches!(
            output.frames[1]
                .map(|frame| decode_commissioning(frame.id(), frame.payload()).unwrap().1),
            Some(CommissioningMessage::NodeClaim { .. })
        ));
        assert!(controller.next_action().is_none());
    }

    #[test]
    fn conflict_suppresses_operational_responses_but_discovery_still_works() {
        let node = NodeId::new(3).unwrap();
        let (mut controller, mut service) = setup(Some(node));
        service.claim_window_complete();
        let conflict_reply = service.handle_commissioning(
            &mut controller,
            RequesterId::PROTOCOL,
            CommissioningMessage::NodeClaim {
                uid: NodeUid::from_bytes([0x55; NodeUid::LENGTH]),
                node_id: node,
                state: CommissioningState::Claiming,
                claim_nonce: 0,
            },
        );
        assert!(matches!(
            conflict_reply.frames[0]
                .map(|frame| { decode_commissioning(frame.id(), frame.payload()).unwrap().1 }),
            Some(CommissioningMessage::NodeClaim {
                state: CommissioningState::AddressConflict,
                ..
            })
        ));
        let operational = service.handle_control(
            &mut controller,
            ControlRequest {
                node: node.get(),
                requester: RequesterId::PDCAN_DEFAULT,
                request_id: RequestId(4),
                command: ControlCommand::RequestStatus {
                    port: PortId::new(0).unwrap(),
                },
            },
        );
        assert!(operational.frames.iter().all(Option::is_none));

        let discovery = service.handle_commissioning(
            &mut controller,
            RequesterId::PDCAN_DEFAULT,
            CommissioningMessage::Discover { nonce: 9 },
        );
        assert_eq!(
            discovery.frames.into_iter().flatten().count(),
            1,
            "UID discovery remains available in conflict"
        );
    }
}
