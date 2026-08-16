use std::{
    env,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use pdcan_protocol::{Header, MessageClass};
use pdcan_types::RequesterId;
use socketcan::{CanFdFrame, CanFdSocket, EmbeddedFrame as _, Socket};

#[test]
fn monitor_observes_requests_from_another_requester() {
    let Ok(interface) = env::var("PDCAN_VCAN_INTERFACE") else {
        eprintln!("PDCAN_VCAN_INTERFACE is not set; skipping SocketCAN integration test");
        return;
    };

    let mut monitor = Command::new(env!("CARGO_BIN_EXE_pdcan"))
        .args([
            "monitor",
            "--interface",
            &interface,
            "--requests",
            "--requester",
            "3",
            "--json",
            "--count",
            "1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start pdcan monitor");

    let header = Header::new(
        1,
        MessageClass::Control,
        3,
        2,
        RequesterId::new(3).unwrap(),
        3,
    )
    .unwrap();
    let socket = CanFdSocket::open(&interface).expect("open vcan interface for test sender");
    let can_id = socketcan::ExtendedId::new(header.encode().get()).unwrap();
    let frame_without_brs = CanFdFrame::new(can_id, &[8, 7, 6, 5, 4, 3, 2, 1]).unwrap();
    socket
        .write_frame(&frame_without_brs)
        .expect("write non-BRS test frame");
    thread::sleep(Duration::from_millis(50));
    assert!(
        monitor
            .try_wait()
            .expect("poll monitor after non-BRS frame")
            .is_none(),
        "monitor must ignore CAN-FD traffic without BRS"
    );

    let mut frame = frame_without_brs;
    frame.set_brs(true);

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        socket.write_frame(&frame).expect("write test PDCAN frame");
        thread::sleep(Duration::from_millis(20));
        if monitor.try_wait().expect("poll monitor process").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            monitor.kill().expect("terminate timed-out monitor");
            panic!("pdcan monitor did not receive the vcan frame");
        }
    }

    let output = monitor.wait_with_output().expect("collect monitor output");
    assert!(
        output.status.success(),
        "monitor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("monitor output is UTF-8");
    assert!(stdout.contains("\"kind\":\"operational_frame\""));
    assert!(stdout.contains("\"class\":\"Control\""));
    assert!(stdout.contains("\"requester\":3"));
    assert!(stdout.contains("\"data\":\"0807060504030201\""));
}
