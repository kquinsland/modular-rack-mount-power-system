use std::{
    env,
    io::ErrorKind,
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use pdcan_protocol::ExtendedId;
use pdcan_sim::SimulatedNode;
use pdcan_types::NodeUid;
use socketcan::{CanAnyFrame, CanFdFrame, CanFdSocket, EmbeddedFrame as _, Frame as _, Socket};

const UID: &str = "000102030405060708090a0b";

struct PeerGuard {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for PeerGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("join simulated node");
        }
    }
}

#[test]
fn cli_commissions_and_controls_a_six_port_simulated_node() {
    let Ok(interface) = env::var("PDCAN_VCAN_INTERFACE") else {
        eprintln!("PDCAN_VCAN_INTERFACE is not set; skipping SocketCAN integration test");
        return;
    };

    let simulator = start_peer(&interface);
    assert_scan(&interface);
    commission(&interface);
    control_node(&interface);
    assert_unsupported_port(&interface);
    clear_node(&interface);
    drop(simulator);
}

fn assert_scan(interface: &str) {
    let scan = run_pdcan(&[
        "scan",
        "--interface",
        interface,
        "--requester",
        "9",
        "--rounds",
        "1",
        "--timeout-ms",
        "100",
        "--json",
    ]);
    assert_success(&scan);
    let scan_stdout = String::from_utf8_lossy(&scan.stdout);
    assert!(scan_stdout.contains("\"kind\":\"discovery\""));
    assert!(scan_stdout.contains(&format!("\"uid\":\"{UID}\"")));
    assert!(scan_stdout.contains("\"node_id\":null"));
    assert!(scan_stdout.contains("\"supported_ports\":63"));
    assert!(scan_stdout.contains("\"compatible\":true"));
}

fn commission(interface: &str) {
    assert_success(&run_pdcan(&[
        "identify",
        UID,
        "--seconds",
        "5",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "1",
    ]));
    assert_success(&run_pdcan(&[
        "assign",
        UID,
        "73",
        "--force",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "2",
    ]));
    let info = run_pdcan(&[
        "info",
        "73",
        "--interface",
        interface,
        "--requester",
        "9",
        "--rounds",
        "1",
        "--timeout-ms",
        "100",
        "--json",
    ]);
    assert_success(&info);
    let info_stdout = String::from_utf8_lossy(&info.stdout);
    assert!(info_stdout.contains(&format!("\"uid\":\"{UID}\"")));
    assert!(info_stdout.contains("\"node_id\":73"));
}

fn control_node(interface: &str) {
    assert_success(&run_pdcan(&[
        "set-policy",
        "73.0",
        "--enabled",
        "--max-voltage-mv",
        "20000",
        "--max-current-ma",
        "5000",
        "--max-power-mw",
        "100000",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "3",
    ]));
    assert_success(&run_pdcan(&[
        "set-fan",
        "73",
        "--mode",
        "4-wire",
        "--duty",
        "65",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "4",
    ]));
    let status = run_pdcan(&[
        "status",
        "73.0",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "40",
        "--json",
    ]);
    assert_success(&status);
    assert!(String::from_utf8_lossy(&status.stdout).contains("\"kind\":\"port_state\""));
    assert!(String::from_utf8_lossy(&status.stdout).contains("\"flags\":3"));
    assert_success(&run_pdcan(&[
        "emergency-disable",
        "all",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "5",
    ]));
    assert_success(&run_pdcan(&[
        "acknowledge-resolved",
        "73",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "6",
    ]));
}

fn assert_unsupported_port(interface: &str) {
    let unsupported = run_pdcan(&[
        "status",
        "73.6",
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "7",
    ]);
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("InvalidTarget"));
}

fn clear_node(interface: &str) {
    assert_success(&run_pdcan(&[
        "clear-node",
        UID,
        "--interface",
        interface,
        "--requester",
        "9",
        "--request-id",
        "8",
    ]));
}

fn run_pdcan(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdcan"))
        .args(arguments)
        .output()
        .expect("run pdcan")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "pdcan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn start_peer(interface: &str) -> PeerGuard {
    let interface = interface.to_owned();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread = thread::spawn(move || {
        let uid = NodeUid::from_bytes([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        let mut node = SimulatedNode::new(6, uid, None).expect("create simulated node");
        let socket = CanFdSocket::open(&interface).expect("open simulator vcan socket");
        socket
            .set_read_timeout(Duration::from_millis(20))
            .expect("set simulator receive timeout");
        ready_tx.send(()).expect("signal simulator readiness");
        while !thread_stop.load(Ordering::Relaxed) {
            let frame = match socket.read_frame() {
                Ok(frame) => frame,
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    continue;
                }
                Err(error) => panic!("simulator read failed: {error}"),
            };
            let CanAnyFrame::Fd(frame) = frame else {
                continue;
            };
            if !frame.is_brs() {
                continue;
            }
            if !frame.is_extended() {
                continue;
            }
            let Ok(id) = ExtendedId::new(frame.raw_id()) else {
                continue;
            };
            for response in node.handle_frame(id, frame.data()) {
                let can_id = socketcan::ExtendedId::new(response.id().get())
                    .expect("simulator response has valid ID");
                let mut response = CanFdFrame::new(can_id, response.payload())
                    .expect("simulator response has valid payload");
                response.set_brs(true);
                socket
                    .write_frame(&response)
                    .expect("write simulator response");
            }
        }
    });
    ready_rx.recv().expect("wait for simulated node");
    PeerGuard {
        stop,
        thread: Some(thread),
    }
}
