use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand};
use clap_schema::{CliSchema, CommandSchema, schema_handler};

use super::{
    decode_id_command,
    error::{CliError, CliResult},
    inspect_bundle, run_network,
};

/// Control and inspect Generation-2 PDCAN nodes.
#[derive(Debug, Parser, CliSchema)]
#[command(name = "pdcan", version, about)]
pub struct Cli {
    #[command(flatten)]
    pub network: NetworkArgs,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn run(self) -> CliResult<()> {
        self.command.run(&self.network)
    }
}

/// Options shared by commands that talk to the CAN bus.
#[derive(Clone, Debug, Args)]
pub struct NetworkArgs {
    /// Linux `SocketCAN` interface.
    #[arg(long, short = 'i', default_value = "can0", global = true)]
    pub interface: String,

    /// PDCAN requester ID (1 through 15).
    #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u8).range(1..=15), global = true)]
    pub requester: u8,

    /// Explicit 64-bit decimal or 0x-prefixed request ID.
    #[arg(long, global = true)]
    pub request_id: Option<String>,

    /// Per-attempt receive timeout.
    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(1..=60_000), global = true)]
    pub timeout_ms: u64,

    /// Number of retransmissions after the initial attempt.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(0..=10), global = true)]
    pub retries: u8,

    /// Emit stable machine-readable output where supported.
    #[arg(long, global = true)]
    pub json: bool,
}

impl NetworkArgs {
    fn append_to(&self, arguments: &mut Vec<String>) {
        arguments.extend(["--interface".into(), self.interface.clone()]);
        arguments.extend(["--requester".into(), self.requester.to_string()]);
        arguments.extend(["--timeout-ms".into(), self.timeout_ms.to_string()]);
        arguments.extend(["--retries".into(), self.retries.to_string()]);
        if let Some(request_id) = &self.request_id {
            arguments.extend(["--request-id".into(), request_id.clone()]);
        }
        if self.json {
            arguments.push("--json".into());
        }
    }

    fn execute(&self, mut arguments: Vec<String>) -> CliResult<()> {
        self.append_to(&mut arguments);
        run_network(&arguments)
    }
}

#[derive(Debug, Subcommand, CommandSchema)]
pub enum Commands {
    /// Display valid CAN-FD frames until interrupted.
    Monitor(MonitorArgs),
    /// Decode one 29-bit PDCAN identifier without accessing CAN.
    DecodeId(DecodeIdArgs),
    /// Discover commissioned and uncommissioned nodes.
    Scan(ScanArgs),
    /// Discover one commissioned node and print its identity.
    Info(InfoArgs),
    /// Ask a physical node to display its identify LED pattern.
    Identify(IdentifyArgs),
    /// Persist a Node ID for a UID.
    Assign(AssignArgs),
    /// Clear the persisted Node ID for a UID.
    ClearNode(ClearNodeArgs),
    /// Request current state and identity from a node.
    Status(StatusArgs),
    /// Change a carrier's persisted output and optional USB-PD policy.
    SetPolicy(SetPolicyArgs),
    /// Set one backplane fan's persisted duty cycle.
    SetFan(SetFanArgs),
    /// Attach descriptive backplane UID/slot metadata to a carrier.
    Bind(BindArgs),
    /// Clear a carrier's descriptive slot binding.
    ClearBinding(ClearBindingArgs),
    /// Latch a safe emergency response on one node or every node.
    EmergencyDisable(EmergencyArgs),
    /// Clear a node's emergency latch after the cause is resolved.
    AcknowledgeResolved(AcknowledgeArgs),
    /// Inspect, stage, activate, or abort application firmware updates.
    Firmware(FirmwareArgs),
    /// Emit the machine-readable `clap_schema` command contract as JSON.
    Schema(SchemaArgs),
}

impl Commands {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        match self {
            Self::Monitor(args) => args.run(network),
            Self::DecodeId(args) => args.run(network),
            Self::Scan(args) => args.run(network),
            Self::Info(args) => args.run(network),
            Self::Identify(args) => args.run(network),
            Self::Assign(args) => args.run(network),
            Self::ClearNode(args) => args.run(network),
            Self::Status(args) => args.run(network),
            Self::SetPolicy(args) => args.run(network),
            Self::SetFan(args) => args.run(network),
            Self::Bind(args) => args.run(network),
            Self::ClearBinding(args) => args.run(network),
            Self::EmergencyDisable(args) => args.run(network),
            Self::AcknowledgeResolved(args) => args.run(network),
            Self::Firmware(args) => args.run(network),
            Self::Schema(args) => args.run(network),
        }
    }
}

#[derive(Debug, Args)]
pub struct MonitorArgs {
    /// Stop after this many valid frames.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    count: Option<u64>,
}

#[schema_handler(run)]
impl MonitorArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        let mut arguments = vec!["monitor".into()];
        if let Some(count) = self.count {
            arguments.extend(["--count".into(), count.to_string()]);
        }
        network.execute(arguments)
    }
}

#[derive(Debug, Args)]
pub struct DecodeIdArgs {
    /// Decimal or 0x-prefixed 29-bit identifier.
    id: String,
}

#[schema_handler(run)]
impl DecodeIdArgs {
    fn run(self, _network: &NetworkArgs) -> CliResult<()> {
        decode_id_command(&[self.id])
    }
}

#[derive(Debug, Args)]
pub struct ScanArgs {}

#[schema_handler(run)]
impl ScanArgs {
    #[allow(clippy::unused_self)]
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["scan".into()])
    }
}

#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Commissioned Node ID.
    node: u8,
}

#[schema_handler(run)]
impl InfoArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["info".into(), self.node.to_string()])
    }
}

#[derive(Debug, Args)]
pub struct IdentifyArgs {
    /// STM32 96-bit UID as exactly 24 hexadecimal digits.
    uid: String,
    /// Identify duration.
    #[arg(long, default_value_t = 10)]
    seconds: u16,
}

#[schema_handler(run)]
impl IdentifyArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec![
            "identify".into(),
            self.uid,
            "--seconds".into(),
            self.seconds.to_string(),
        ])
    }
}

#[derive(Debug, Args)]
pub struct AssignArgs {
    /// STM32 96-bit UID as exactly 24 hexadecimal digits.
    uid: String,
    /// Node ID to persist (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
}

#[schema_handler(run)]
impl AssignArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["assign".into(), self.uid, self.node.to_string()])
    }
}

#[derive(Debug, Args)]
pub struct ClearNodeArgs {
    /// STM32 96-bit UID as exactly 24 hexadecimal digits.
    uid: String,
}

#[schema_handler(run)]
impl ClearNodeArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["clear-node".into(), self.uid])
    }
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Commissioned Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
}

#[schema_handler(run)]
impl StatusArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["status".into(), self.node.to_string()])
    }
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("output_state")
        .required(true)
        .multiple(false)
        .args(["enabled", "disabled"])
))]
pub struct SetPolicyArgs {
    /// Carrier Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
    /// Persist enabled output policy.
    #[arg(long)]
    enabled: bool,
    /// Persist disabled output policy.
    #[arg(long)]
    disabled: bool,
    /// Maximum negotiated voltage in millivolts.
    #[arg(long, requires_all = ["max_current_ma", "max_power_mw"])]
    max_voltage_mv: Option<u32>,
    /// Maximum negotiated current in milliamps.
    #[arg(long, requires_all = ["max_voltage_mv", "max_power_mw"])]
    max_current_ma: Option<u32>,
    /// Maximum negotiated power in milliwatts.
    #[arg(long, requires_all = ["max_voltage_mv", "max_current_ma"])]
    max_power_mw: Option<u32>,
}

#[schema_handler(run)]
impl SetPolicyArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        let mut arguments = vec![
            "set-policy".into(),
            self.node.to_string(),
            if self.enabled {
                "--enabled".into()
            } else {
                "--disabled".into()
            },
        ];
        for (name, value) in [
            ("--max-voltage-mv", self.max_voltage_mv),
            ("--max-current-ma", self.max_current_ma),
            ("--max-power-mw", self.max_power_mw),
        ] {
            if let Some(value) = value {
                arguments.extend([name.into(), value.to_string()]);
            }
        }
        network.execute(arguments)
    }
}

#[derive(Debug, Args)]
pub struct SetFanArgs {
    /// Backplane Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
    /// Zero-based fan index (0 or 1).
    #[arg(value_parser = clap::value_parser!(u8).range(0..=1))]
    fan: u8,
    /// PWM duty percent for the 3-wire fan supply.
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
    duty: u8,
}

#[schema_handler(run)]
impl SetFanArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec![
            "set-fan".into(),
            self.node.to_string(),
            self.fan.to_string(),
            "--duty".into(),
            self.duty.to_string(),
        ])
    }
}

#[derive(Debug, Args)]
pub struct BindArgs {
    /// Carrier Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
    /// Backplane STM32 UID as exactly 24 hexadecimal digits.
    backplane_uid: String,
    /// Descriptive zero-based slot index (0 through 254).
    slot: u8,
}

#[schema_handler(run)]
impl BindArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec![
            "bind".into(),
            self.node.to_string(),
            self.backplane_uid,
            self.slot.to_string(),
        ])
    }
}

#[derive(Debug, Args)]
pub struct ClearBindingArgs {
    /// Carrier Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
}

#[schema_handler(run)]
impl ClearBindingArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["clear-binding".into(), self.node.to_string()])
    }
}

#[derive(Debug, Args)]
pub struct EmergencyArgs {
    /// Node ID (1 through 254), or "all" for the broadcast emergency command.
    node: String,
}

#[schema_handler(run)]
impl EmergencyArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["emergency-disable".into(), self.node])
    }
}

#[derive(Debug, Args)]
pub struct AcknowledgeArgs {
    /// Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
}

#[schema_handler(run)]
impl AcknowledgeArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec!["acknowledge-resolved".into(), self.node.to_string()])
    }
}

#[derive(Debug, Args, CommandSchema)]
pub struct FirmwareArgs {
    #[command(subcommand)]
    command: FirmwareCommands,
}

impl FirmwareArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        self.command.run(network)
    }
}

#[derive(Debug, Subcommand, CommandSchema)]
enum FirmwareCommands {
    /// Validate and display a local PDCAN firmware bundle.
    Inspect(FirmwareInspectArgs),
    /// Read update state, versions, digest, and update impact from one node.
    Status(FirmwareStatusArgs),
    /// Transfer and verify an image without resetting the node.
    Stage(FirmwareStageArgs),
    /// Activate a previously staged image.
    Activate(FirmwareActivateArgs),
    /// Stage and then activate an image.
    Update(FirmwareUpdateArgs),
    /// Discard the current receiving or staged update.
    Abort(FirmwareAbortArgs),
}

impl FirmwareCommands {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        match self {
            Self::Inspect(args) => args.run(network),
            Self::Status(args) => args.run(network),
            Self::Stage(args) => args.run(network),
            Self::Activate(args) => args.run(network),
            Self::Update(args) => args.run(network),
            Self::Abort(args) => args.run(network),
        }
    }
}

#[derive(Debug, Args)]
struct FirmwareInspectArgs {
    /// Versioned PDCAN firmware bundle.
    bundle: PathBuf,
}

#[schema_handler(run)]
impl FirmwareInspectArgs {
    fn run(self, _network: &NetworkArgs) -> CliResult<()> {
        inspect_bundle(&[self.bundle.to_string_lossy().into_owned()])
    }
}

#[derive(Debug, Args)]
struct FirmwareStatusArgs {
    /// Target Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
}

#[schema_handler(run)]
impl FirmwareStatusArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec![
            "firmware".into(),
            "status".into(),
            self.node.to_string(),
        ])
    }
}

#[derive(Debug, Args)]
struct FirmwareStageArgs {
    /// Target Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
    /// Versioned PDCAN firmware bundle.
    bundle: PathBuf,
}

#[schema_handler(run)]
impl FirmwareStageArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec![
            "firmware".into(),
            "stage".into(),
            self.node.to_string(),
            self.bundle.to_string_lossy().into_owned(),
        ])
    }
}

#[derive(Debug, Args)]
struct FirmwareActivateArgs {
    /// Target Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
    /// Acknowledge that activation may interrupt the node's declared service.
    #[arg(long)]
    allow_interruption: bool,
}

#[schema_handler(run)]
impl FirmwareActivateArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        let mut arguments = vec!["firmware".into(), "activate".into(), self.node.to_string()];
        if self.allow_interruption {
            arguments.push("--allow-interruption".into());
        }
        network.execute(arguments)
    }
}

#[derive(Debug, Args)]
struct FirmwareUpdateArgs {
    /// Target Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
    /// Versioned PDCAN firmware bundle.
    bundle: PathBuf,
    /// Acknowledge that activation may interrupt the node's declared service.
    #[arg(long)]
    allow_interruption: bool,
}

#[schema_handler(run)]
impl FirmwareUpdateArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        let mut arguments = vec![
            "firmware".into(),
            "update".into(),
            self.node.to_string(),
            self.bundle.to_string_lossy().into_owned(),
        ];
        if self.allow_interruption {
            arguments.push("--allow-interruption".into());
        }
        network.execute(arguments)
    }
}

#[derive(Debug, Args)]
struct FirmwareAbortArgs {
    /// Target Node ID (1 through 254).
    #[arg(value_parser = clap::value_parser!(u8).range(1..=254))]
    node: u8,
}

#[schema_handler(run)]
impl FirmwareAbortArgs {
    fn run(self, network: &NetworkArgs) -> CliResult<()> {
        network.execute(vec![
            "firmware".into(),
            "abort".into(),
            self.node.to_string(),
        ])
    }
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    /// Optional command path, for example: firmware activate.
    path: Vec<String>,
    /// Recursively include the full schema of every descendant command.
    #[arg(long)]
    full: bool,
}

#[schema_handler(run)]
impl SchemaArgs {
    fn run(self, _network: &NetworkArgs) -> CliResult<()> {
        let contract = Cli::schema()
            .map_err(|error| CliError::Internal(format!("cannot build CLI schema: {error}")))?;
        let request = clap_schema::SchemaRequest::new(self.path).with_full(self.full);
        let document = contract.schema(&request).map_err(|error| {
            CliError::InvalidInput(format!("cannot resolve CLI schema: {error}"))
        })?;
        println!(
            "{}",
            serde_json::to_string_pretty(&document).map_err(|error| {
                CliError::Internal(format!("cannot serialize CLI schema: {error}"))
            })?
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn clap_enforces_node_fan_and_policy_constraints() {
        assert!(Cli::try_parse_from(["pdcan", "set-fan", "3", "2", "--duty", "50"]).is_err());
        assert!(Cli::try_parse_from(["pdcan", "set-policy", "3"]).is_err());
        assert!(
            Cli::try_parse_from([
                "pdcan",
                "set-policy",
                "3",
                "--enabled",
                "--max-voltage-mv",
                "20000"
            ])
            .is_err()
        );
    }

    #[test]
    fn clap_schema_exposes_interrupt_acknowledgement_and_global_transport() {
        let contract = Cli::schema().unwrap();
        let activate = contract
            .command_for::<FirmwareActivateArgs>()
            .expect("firmware activate is registered");
        assert!(
            activate
                .options
                .iter()
                .any(|option| option.name == "--allow-interruption")
        );
        assert!(
            activate
                .ancestors
                .iter()
                .flat_map(|context| &context.options)
                .any(|option| option.name == "--interface")
        );
    }

    #[test]
    fn clap_tree_contains_no_legacy_port_commands() {
        let mut command = Cli::command();
        command.build();
        let help = command.render_long_help().to_string();
        assert!(help.contains("set-fan"));
        assert!(help.contains("firmware"));
        assert!(!help.contains("NODE.PORT"));
        assert!(!help.contains("backpack"));
    }
}
