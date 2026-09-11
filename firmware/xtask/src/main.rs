use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use pdcan_protocol::{
    CommissioningHeader, Header, MESSAGE_DEFINITIONS, MessageClass, MessageSender, NODE_TARGET,
    TargetKind,
};
use pdcan_types::{
    CarrierProfile, FirmwareVersion, HardwareRevision, ImageTarget, NodeRole, RequesterId,
    Sha256Digest, UpdateManifest,
};
use sha2::{Digest as _, Sha256};

const EMBEDDED_TARGET: &str = "thumbv6m-none-eabi";
const APPLICATION_PARTITION_BYTES: u64 = 110 * 1024;
const APPLICATION_BUDGET_BYTES: u64 = 100 * 1024;
const BOOTLOADER_PARTITION_BYTES: u64 = 16 * 1024;
const SRAM_BYTES: u64 = 30 * 1024;
const RAM_BUDGET_BYTES: u64 = SRAM_BYTES * 9 / 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FirmwareTarget {
    Backplane,
    Carrier,
    Bootloader,
}

impl FirmwareTarget {
    const ALL: [Self; 3] = [Self::Backplane, Self::Carrier, Self::Bootloader];

    const fn name(self) -> &'static str {
        match self {
            Self::Backplane => "backplane",
            Self::Carrier => "carrier",
            Self::Bootloader => "bootloader",
        }
    }

    const fn package(self) -> &'static str {
        match self {
            Self::Backplane => "backplane-firmware",
            Self::Carrier => "carrier-firmware",
            Self::Bootloader => "pdcan-bootloader",
        }
    }

    const fn binary(self) -> &'static str {
        match self {
            Self::Backplane => "backplane",
            Self::Carrier => "carrier",
            Self::Bootloader => "pdcan-bootloader",
        }
    }

    const fn features(self) -> Option<&'static str> {
        match self {
            Self::Backplane | Self::Carrier => Some("firmware-bin"),
            Self::Bootloader => None,
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "backplane" => Ok(Self::Backplane),
            "carrier" => Ok(Self::Carrier),
            "bootloader" => Ok(Self::Bootloader),
            _ => Err(format!("unsupported firmware target {value:?}")),
        }
    }
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::from(1)
        }
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let arguments: Vec<_> = arguments.collect();
    match arguments.as_slice() {
        [] => {
            print_help();
            Ok(())
        }
        [arg] if arg == "--help" || arg == "-h" => {
            print_help();
            Ok(())
        }
        [command] if command == "dbc" => write_or_check_dbc(false),
        [command, option] if command == "dbc" && option == "--check" => write_or_check_dbc(true),
        [command] if command == "ci" => run_ci(),
        [firmware, build, all] if firmware == "firmware" && build == "build" && all == "all" => {
            build_all_firmware(false)
        }
        [firmware, build, all, release]
            if firmware == "firmware"
                && build == "build"
                && all == "all"
                && release == "--release" =>
        {
            build_all_firmware(true)
        }
        [firmware, build, target] if firmware == "firmware" && build == "build" => {
            build_firmware(FirmwareTarget::parse(target)?, false)
        }
        [firmware, build, target, release]
            if firmware == "firmware" && build == "build" && release == "--release" =>
        {
            build_firmware(FirmwareTarget::parse(target)?, true)
        }
        [firmware, bundle, target, version, image, output]
            if firmware == "firmware" && bundle == "bundle" =>
        {
            build_bundle(
                FirmwareTarget::parse(target)?,
                parse_version(version)?,
                Path::new(image),
                Path::new(output),
            )
        }
        _ => Err("unsupported arguments; run `cargo xtask --help`".into()),
    }
}

fn print_help() {
    println!(
        "\
Firmware workspace automation (run from firmware/)

Usage:
  cargo xtask ci
  cargo xtask dbc [--check]
  cargo xtask firmware build <backplane|carrier|bootloader|all> [--release]
  cargo xtask firmware bundle <backplane|carrier> <MAJOR.MINOR.PATCH> <IMAGE> <OUTPUT>
"
    );
}

fn parse_version(value: &str) -> Result<FirmwareVersion, String> {
    let mut components = value.split('.');
    let parse_component = |component: Option<&str>| {
        component
            .ok_or_else(|| "version must be MAJOR.MINOR.PATCH".to_owned())?
            .parse::<u16>()
            .map_err(|_| "version components must be unsigned 16-bit integers".to_owned())
    };
    let version = FirmwareVersion {
        major: parse_component(components.next())?,
        minor: parse_component(components.next())?,
        patch: parse_component(components.next())?,
    };
    if components.next().is_some() {
        return Err("version must be MAJOR.MINOR.PATCH".into());
    }
    Ok(version)
}

fn build_bundle(
    target: FirmwareTarget,
    version: FirmwareVersion,
    image_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let (role, carrier_profile) = match target {
        FirmwareTarget::Backplane => (NodeRole::Backplane, None),
        FirmwareTarget::Carrier => (NodeRole::Carrier, Some(CarrierProfile::Sw3538)),
        FirmwareTarget::Bootloader => {
            return Err(
                "bootloaders are SWD-only and cannot be placed in CAN update bundles".into(),
            );
        }
    };
    let image = fs::read(image_path)
        .map_err(|error| format!("cannot read {}: {error}", image_path.display()))?;
    if image.len() > usize::try_from(APPLICATION_PARTITION_BYTES).expect("limit fits usize") {
        return Err(format!(
            "application image exceeds the {APPLICATION_PARTITION_BYTES} byte ACTIVE partition"
        ));
    }
    let digest: [u8; 32] = Sha256::digest(&image).into();
    let manifest = UpdateManifest {
        target: ImageTarget {
            role,
            carrier_profile,
            hardware_revision: HardwareRevision::REV_A,
            partition_layout: 1,
        },
        version,
        image_size: u32::try_from(image.len()).map_err(|_| "application image is too large")?,
        digest: Sha256Digest::from_bytes(digest),
    };
    let bundle = pdcan_artifact::build_unsigned(manifest, &image)
        .map_err(|error| format!("cannot build bundle: {error:?}"))?;
    fs::write(output_path, bundle)
        .map_err(|error| format!("cannot write {}: {error}", output_path.display()))?;
    println!("generated {}", output_path.display());
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is directly below the workspace root")
        .to_owned()
}

fn run_ci() -> Result<(), String> {
    run_command("cargo", &["fmt", "--all", "--check"])?;
    run_command(
        "cargo",
        &["test", "--workspace", "--exclude", "pdcan-bootloader"],
    )?;
    run_command(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--exclude",
            "pdcan-bootloader",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    for target in FirmwareTarget::ALL {
        clippy_firmware(target)?;
    }
    write_or_check_dbc(true)?;
    for target in FirmwareTarget::ALL {
        build_firmware(target, true)?;
    }
    Ok(())
}

fn clippy_firmware(target: FirmwareTarget) -> Result<(), String> {
    let mut arguments = vec![
        "clippy",
        "--package",
        target.package(),
        "--bin",
        target.binary(),
        "--target",
        EMBEDDED_TARGET,
    ];
    if let Some(features) = target.features() {
        arguments.extend(["--features", features]);
    }
    arguments.extend(["--", "-D", "warnings"]);
    run_command("cargo", &arguments)
}

fn build_firmware(target: FirmwareTarget, release: bool) -> Result<(), String> {
    let mut arguments = vec![
        "build",
        "--package",
        target.package(),
        "--bin",
        target.binary(),
        "--target",
        EMBEDDED_TARGET,
    ];
    if let Some(features) = target.features() {
        arguments.extend(["--features", features]);
    }
    if release {
        arguments.push("--release");
    }
    run_command("cargo", &arguments)?;
    if release {
        report_firmware_size(target)?;
    }
    Ok(())
}

fn build_all_firmware(release: bool) -> Result<(), String> {
    for target in FirmwareTarget::ALL {
        build_firmware(target, release)?;
    }
    Ok(())
}

fn report_firmware_size(target: FirmwareTarget) -> Result<(), String> {
    let sysroot = command_stdout("rustc", &["--print", "sysroot"])?;
    let version = command_stdout("rustc", &["-vV"])?;
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc version data did not include a host triple")?;
    let llvm_size = Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin/llvm-size");
    if !llvm_size.is_file() {
        return Err(format!(
            "llvm-tools-preview is required; {} is missing",
            llvm_size.display()
        ));
    }
    let artifact = workspace_root()
        .join("target")
        .join(EMBEDDED_TARGET)
        .join("release")
        .join(target.binary());
    let output = Command::new(&llvm_size)
        .arg(&artifact)
        .output()
        .map_err(|error| format!("cannot run llvm-size: {error}"))?;
    if !output.status.success() {
        return Err(format!("llvm-size failed for {}", artifact.display()));
    }
    let report = String::from_utf8(output.stdout)
        .map_err(|error| format!("llvm-size returned non-UTF-8 output: {error}"))?;
    let (text, data, bss) = parse_llvm_size(&report)?;
    let flash = text.saturating_add(data);
    let ram = data.saturating_add(bss);
    let (hard_flash_limit, budget) = if target == FirmwareTarget::Bootloader {
        (BOOTLOADER_PARTITION_BYTES, BOOTLOADER_PARTITION_BYTES)
    } else {
        (APPLICATION_PARTITION_BYTES, APPLICATION_BUDGET_BYTES)
    };
    println!(
        "firmware {} size: flash={flash}/{hard_flash_limit} bytes (budget {budget}), ram={ram}/{SRAM_BYTES} bytes (budget {RAM_BUDGET_BYTES})",
        target.name()
    );
    if flash > hard_flash_limit {
        return Err(format!(
            "{} flash partition exceeded: {flash} > {hard_flash_limit}",
            target.name()
        ));
    }
    if flash > budget {
        return Err(format!(
            "{} flash growth budget exceeded: {flash} > {budget}",
            target.name()
        ));
    }
    if ram > RAM_BUDGET_BYTES {
        return Err(format!(
            "{} RAM budget exceeded: {ram} > {RAM_BUDGET_BYTES}",
            target.name()
        ));
    }
    Ok(())
}

fn command_stdout(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("{program} output was not UTF-8: {error}"))
}

fn parse_llvm_size(report: &str) -> Result<(u64, u64, u64), String> {
    let values: Vec<_> = report
        .lines()
        .nth(1)
        .ok_or_else(|| format!("unexpected llvm-size output: {report:?}"))?
        .split_whitespace()
        .collect();
    if values.len() < 3 {
        return Err(format!("unexpected llvm-size row: {values:?}"));
    }
    let parse = |index: usize, name: &str| {
        values[index]
            .parse::<u64>()
            .map_err(|error| format!("invalid {name} size {:?}: {error}", values[index]))
    };
    Ok((parse(0, "text")?, parse(1, "data")?, parse(2, "bss")?))
}

fn write_or_check_dbc(check: bool) -> Result<(), String> {
    let path = workspace_root().join("docs/pdcan/can/pdcan.dbc");
    let generated = render_dbc()?;
    if check {
        let existing = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read generated {}: {error}", path.display()))?;
        if existing != generated {
            return Err(format!(
                "{} is stale; run `cargo xtask dbc`",
                path.display()
            ));
        }
    } else {
        let parent = path.parent().expect("DBC path has a parent");
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        fs::write(&path, generated)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        println!("generated {}", path.display());
    }
    Ok(())
}

fn render_dbc() -> Result<String, String> {
    let mut output = String::from(
        "VERSION \"PDCAN Gen-2; generated by cargo xtask dbc\"\n\nNS_ :\n\nBS_:\n\nBU_: Node Host\n\n",
    );
    output.push_str(
        "CM_ \"Generated from pdcan_protocol::MESSAGE_DEFINITIONS. Operational IDs use node 1, target 15 (or fan 0), and requester 15; commissioning IDs use representative token 1.\";\n\n",
    );
    for definition in MESSAGE_DEFINITIONS {
        let target = match definition.target {
            TargetKind::Fan => 0,
            TargetKind::Node | TargetKind::Commissioning => NODE_TARGET,
        };
        let requester = if definition.requester_scoped {
            RequesterId::PDCAN_DEFAULT
        } else {
            RequesterId::PROTOCOL
        };
        let id = if definition.class == MessageClass::Commissioning {
            CommissioningHeader::new(definition.priority, 1, requester, definition.opcode)
                .map_err(|error| format!("invalid definition {}: {error:?}", definition.name))?
                .encode()
        } else {
            Header::new(
                definition.priority,
                definition.class,
                1,
                target,
                requester,
                definition.opcode,
            )
            .map_err(|error| format!("invalid definition {}: {error:?}", definition.name))?
            .encode()
        };
        let transmitter = match definition.sender {
            MessageSender::Host => "Host",
            MessageSender::Node => "Node",
        };
        writeln!(
            &mut output,
            "BO_ {} PDCAN_{}: {} {transmitter}",
            id.get() | 0x8000_0000,
            definition.name,
            definition.payload_len
        )
        .map_err(|error| format!("cannot render DBC: {error}"))?;
        if definition.has_request_id {
            output.push_str(
                " SG_ RequestId : 0|64@1+ (1,0) [0|1.8446744073709552e+19] \"\" Vector__XXX\n",
            );
        }
        output.push('\n');
    }
    Ok(output)
}

fn run_command(program: &str, arguments: &[&str]) -> Result<(), String> {
    println!("+ {program} {}", arguments.join(" "));
    let status = Command::new(program)
        .args(arguments)
        .current_dir(workspace_root())
        .status()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_llvm_size_berkeley_output() {
        let report = "text data bss dec hex filename\n1024 12 44 1080 438 firmware\n";
        assert_eq!(parse_llvm_size(report), Ok((1024, 12, 44)));
    }

    #[test]
    fn generated_dbc_contains_every_gen2_definition_and_no_backpack_node() {
        let dbc = render_dbc().unwrap();
        for definition in MESSAGE_DEFINITIONS {
            assert!(dbc.contains(&format!("PDCAN_{}:", definition.name)));
        }
        assert!(dbc.contains("BU_: Node Host"));
        assert!(!dbc.contains("Backpack"));
    }

    #[test]
    fn all_three_finalized_firmware_targets_are_buildable_names() {
        assert_eq!(FirmwareTarget::ALL.len(), 3);
        assert_eq!(
            FirmwareTarget::parse("backplane"),
            Ok(FirmwareTarget::Backplane)
        );
        assert_eq!(
            FirmwareTarget::parse("carrier"),
            Ok(FirmwareTarget::Carrier)
        );
        assert_eq!(
            FirmwareTarget::parse("bootloader"),
            Ok(FirmwareTarget::Bootloader)
        );
    }
}
