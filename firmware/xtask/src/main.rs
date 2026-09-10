use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use pdcan_protocol::{
    BOARD_TARGET, CommissioningHeader, Header, MESSAGE_DEFINITIONS, MessageClass, MessageSender,
    TargetKind,
};
use pdcan_types::RequesterId;

const FIRMWARE_PACKAGE: &str = "backplane-backpack-firmware";
const FIRMWARE_BINARY: &str = "backplane-backpack";
const FIRMWARE_TARGET: &str = "thumbv6m-none-eabi";
const REV_A_FEATURES: &str = "board-rev-a,firmware-bin";
const REV_B_FEATURES: &str = "board-rev-b,firmware-bin";
const APPLICATION_FLASH_BYTES: u64 = 248 * 1024;
const SRAM_BYTES: u64 = 30 * 1024;
const MAX_FLASH_BYTES: u64 = APPLICATION_FLASH_BYTES * 9 / 10;
const MAX_RAM_BYTES: u64 = SRAM_BYTES * 9 / 10;

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
        [firmware, build, board_flag, board]
            if firmware == "firmware"
                && build == "build"
                && board_flag == "--board"
                && matches!(board.as_str(), "rev-a" | "rev-b") =>
        {
            build_firmware(board, false)
        }
        [firmware, build, board_flag, board, release]
            if firmware == "firmware"
                && build == "build"
                && board_flag == "--board"
                && matches!(board.as_str(), "rev-a" | "rev-b")
                && release == "--release" =>
        {
            build_firmware(board, true)
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
  cargo xtask firmware build --board <rev-a|rev-b> [--release]
"
    );
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
        &["test", "--workspace", "--exclude", FIRMWARE_PACKAGE],
    )?;
    test_firmware_lib("board-rev-a")?;
    test_firmware_lib("board-rev-b")?;
    run_command(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--exclude",
            FIRMWARE_PACKAGE,
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    clippy_firmware_lib("board-rev-a")?;
    clippy_firmware_lib("board-rev-b")?;
    clippy_firmware_bin(REV_A_FEATURES)?;
    clippy_firmware_bin(REV_B_FEATURES)?;
    write_or_check_dbc(true)?;
    build_firmware("rev-a", true)?;
    build_firmware("rev-b", true)
}

fn test_firmware_lib(board_feature: &str) -> Result<(), String> {
    run_command(
        "cargo",
        &[
            "test",
            "--package",
            FIRMWARE_PACKAGE,
            "--lib",
            "--no-default-features",
            "--features",
            board_feature,
        ],
    )
}

fn clippy_firmware_lib(board_feature: &str) -> Result<(), String> {
    run_command(
        "cargo",
        &[
            "clippy",
            "--package",
            FIRMWARE_PACKAGE,
            "--lib",
            "--no-default-features",
            "--features",
            board_feature,
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn clippy_firmware_bin(features: &str) -> Result<(), String> {
    run_command(
        "cargo",
        &[
            "clippy",
            "--package",
            FIRMWARE_PACKAGE,
            "--bin",
            FIRMWARE_BINARY,
            "--target",
            FIRMWARE_TARGET,
            "--no-default-features",
            "--features",
            features,
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn build_firmware(board: &str, release: bool) -> Result<(), String> {
    let features = match board {
        "rev-a" => REV_A_FEATURES,
        "rev-b" => REV_B_FEATURES,
        _ => return Err(format!("unsupported firmware board {board:?}")),
    };
    let mut arguments = vec![
        "build",
        "--package",
        FIRMWARE_PACKAGE,
        "--bin",
        FIRMWARE_BINARY,
        "--target",
        FIRMWARE_TARGET,
        "--no-default-features",
        "--features",
        features,
    ];
    if release {
        arguments.push("--release");
    }
    run_command("cargo", &arguments)?;
    if release {
        report_firmware_size(board)?;
    }
    Ok(())
}

fn report_firmware_size(board: &str) -> Result<(), String> {
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .map_err(|error| format!("cannot query the Rust sysroot: {error}"))?;
    if !sysroot.status.success() {
        return Err("rustc could not report its sysroot".into());
    }
    let sysroot = String::from_utf8(sysroot.stdout)
        .map_err(|error| format!("rustc returned a non-UTF-8 sysroot: {error}"))?;
    let host = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|error| format!("cannot query the Rust host: {error}"))?;
    let host = String::from_utf8(host.stdout)
        .map_err(|error| format!("rustc returned non-UTF-8 version data: {error}"))?;
    let host = host
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
        .join(FIRMWARE_TARGET)
        .join("release")
        .join(FIRMWARE_BINARY);
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
    println!(
        "firmware {board} size: flash={flash}/{APPLICATION_FLASH_BYTES} bytes (budget {MAX_FLASH_BYTES}), ram={ram}/{SRAM_BYTES} bytes (budget {MAX_RAM_BYTES})"
    );
    if flash > MAX_FLASH_BYTES {
        return Err(format!(
            "firmware flash budget exceeded: {flash} > {MAX_FLASH_BYTES}"
        ));
    }
    if ram > MAX_RAM_BYTES {
        return Err(format!(
            "firmware RAM budget exceeded: {ram} > {MAX_RAM_BYTES}"
        ));
    }
    Ok(())
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
        "VERSION \"PDCAN draft; generated by cargo xtask dbc\"\n\nNS_ :\n\nBS_:\n\nBU_: Backpack Host\n\n",
    );
    output.push_str(
        "CM_ \"Generated from pdcan_protocol::MESSAGE_DEFINITIONS. Operational IDs use node 1, port 0, and requester 15; commissioning IDs use representative token 1. Dynamic fields vary on the bus.\";\n\n",
    );

    for definition in MESSAGE_DEFINITIONS {
        let target = match definition.target {
            TargetKind::Port => 0,
            TargetKind::AllPorts => pdcan_protocol::ALL_PORTS_TARGET,
            TargetKind::Board | TargetKind::Commissioning => BOARD_TARGET,
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
        let dbc_id = id.get() | 0x8000_0000;
        let transmitter = match definition.sender {
            MessageSender::Host => "Host",
            MessageSender::Backpack => "Backpack",
        };
        writeln!(
            &mut output,
            "BO_ {dbc_id} PDCAN_{}: {} {transmitter}",
            definition.name, definition.payload_len
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
    fn generated_dbc_contains_every_protocol_definition() {
        let dbc = render_dbc().unwrap();
        for definition in MESSAGE_DEFINITIONS {
            assert!(dbc.contains(&format!("PDCAN_{}:", definition.name)));
        }
    }
}
