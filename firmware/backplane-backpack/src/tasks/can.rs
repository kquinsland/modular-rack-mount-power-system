use embassy_futures::select::{Either3, select3};
use embassy_stm32::can::Can;
use embassy_stm32::can::frame::{FdFrame, Header as CanHeader};
use embassy_stm32::gpio::Output;
use embassy_time::Timer;
use embedded_can::Id;
use pdcan_protocol::{ExtendedId, WireFrame, decode_commissioning, decode_control_request};

use crate::channels::{CAN_PROGRESS, CAN_TX, CONTROLLER_EVENTS, ControllerEvent, advance};

const HEALTH_TICK_MS: u64 = 250;

/// Owns FDCAN and is the only firmware task which converts between Embassy
/// frames and transport-neutral protocol frames.
#[embassy_executor::task]
pub async fn can_task(can: Can<'static>, mut transceiver_standby: Output<'static>) {
    transceiver_standby.set_low();
    let (mut tx, mut rx, _properties) = can.split();
    loop {
        match select3(
            rx.read_fd(),
            CAN_TX.receive(),
            Timer::after_millis(HEALTH_TICK_MS),
        )
        .await
        {
            Either3::First(Ok(envelope)) => {
                let frame = envelope.frame;
                if frame.header().fdcan()
                    && frame.header().bit_rate_switching()
                    && let Id::Extended(id) = *frame.id()
                    && let Ok(id) = ExtendedId::new(id.as_raw())
                {
                    dispatch_received(id, frame.data()).await;
                }
            }
            Either3::First(Err(_bus_error)) => {}
            Either3::Second(frame) => transmit(&mut tx, frame).await,
            Either3::Third(()) => {}
        }
        advance(&CAN_PROGRESS);
    }
}

async fn dispatch_received(id: ExtendedId, payload: &[u8]) {
    if let Ok(request) = decode_control_request(id, payload) {
        CONTROLLER_EVENTS
            .send(ControllerEvent::CanControl(request))
            .await;
    } else if let Ok((header, message)) = decode_commissioning(id, payload) {
        CONTROLLER_EVENTS
            .send(ControllerEvent::CanCommissioning {
                requester: header.requester,
                message,
            })
            .await;
    }
}

async fn transmit(tx: &mut embassy_stm32::can::CanTx<'static>, frame: WireFrame) {
    let Some(id) = embedded_can::ExtendedId::new(frame.id().get()) else {
        return;
    };
    let Ok(length) = u8::try_from(frame.payload().len()) else {
        return;
    };
    let header = CanHeader::new_fd(id.into(), length, false, true);
    let Ok(mut pending) = FdFrame::new(header, frame.payload()) else {
        return;
    };
    while let Some(displaced) = tx.write_fd(&pending).await {
        pending = displaced;
    }
}
