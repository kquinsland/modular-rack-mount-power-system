use backplane_backpack_firmware::action_executor::{
    ControllerCompletion, PdBusCommand, PersistCommand,
};
use backplane_backpack_firmware::status::StatusCommand;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use pdcan_protocol::{CommissioningMessage, ControlRequest, WireFrame};
use pdcan_types::RequesterId;
use pdcan_types::{FanConfig, PortId};
use portable_atomic::{AtomicBool, AtomicU32, Ordering};

pub const PD_COMMAND_CAPACITY: usize = 8;
pub const PERSIST_COMMAND_CAPACITY: usize = 2;
pub const CONTROLLER_EVENT_CAPACITY: usize = 16;
pub const CAN_TX_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerEvent {
    Completion(ControllerCompletion),
    ModuleDetected(PortId),
    ModuleRemoved(PortId),
    CanControl(ControlRequest),
    CanCommissioning {
        requester: RequesterId,
        message: CommissioningMessage,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanRequest {
    pub config: FanConfig,
    pub force_full_speed: bool,
}

pub static PD_EMERGENCY_COMMANDS: Channel<
    CriticalSectionRawMutex,
    PdBusCommand,
    PD_COMMAND_CAPACITY,
> = Channel::new();
pub static PD_POLICY_COMMANDS: Channel<CriticalSectionRawMutex, PdBusCommand, PD_COMMAND_CAPACITY> =
    Channel::new();
pub static PERSIST_COMMANDS: Channel<
    CriticalSectionRawMutex,
    PersistCommand,
    PERSIST_COMMAND_CAPACITY,
> = Channel::new();
pub static CONTROLLER_EVENTS: Channel<
    CriticalSectionRawMutex,
    ControllerEvent,
    CONTROLLER_EVENT_CAPACITY,
> = Channel::new();
pub static CAN_TX: Channel<CriticalSectionRawMutex, WireFrame, CAN_TX_CAPACITY> = Channel::new();
pub static FAN_REQUEST: Signal<CriticalSectionRawMutex, FanRequest> = Signal::new();
pub static STATUS_REQUEST: Signal<CriticalSectionRawMutex, StatusCommand> = Signal::new();

pub static CONTROLLER_PROGRESS: AtomicU32 = AtomicU32::new(0);
pub static PD_BUS_PROGRESS: AtomicU32 = AtomicU32::new(0);
pub static CAN_PROGRESS: AtomicU32 = AtomicU32::new(0);
pub static FLASH_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn advance(counter: &AtomicU32) {
    counter.fetch_add(1, Ordering::Relaxed);
}

pub fn load(counter: &AtomicU32) -> u32 {
    counter.load(Ordering::Relaxed)
}
