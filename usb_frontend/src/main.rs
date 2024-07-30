use nusb::transfer::{Control, ControlType, Recipient};
use std::time::Duration;

fn main() {
    let di = nusb::list_devices()
        .unwrap()
        .find(|d| d.vendor_id() == 0xc0de && d.product_id() == 0xcafe)
        .expect("device should be connected");

    println!("Device info: {di:?}");

    let device = di.open().unwrap();

    let interface = device.claim_interface(0).unwrap();

    let result = interface.control_out_blocking(
        Control {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: 0x81,
            value: 0x9999,
            index: 0x9999,
        },
        &[1, 2, 3, 4, 5],
        Duration::from_secs(1),
    );
    println!("{result:?}");

    let mut buf = [0; 64];

    let len = interface
        .control_in_blocking(
            Control {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: 0x81,
                value: 0x9999,
                index: 0x9999,
            },
            &mut buf,
            Duration::from_secs(1),
        )
        .unwrap();
    println!("{data:?}", data = &buf[..len]);
}
