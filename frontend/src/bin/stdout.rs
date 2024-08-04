fn main() {
    let di = nusb::list_devices()
        .unwrap()
        .find(|d| d.vendor_id() == 0xc0de && d.product_id() == 0xcafe)
        .expect("device should be connected");

    eprintln!("Device info: {di:?}");

    let device = di.open().unwrap();

    let interface = device.claim_interface(0).unwrap();

    // Read and print ADC values
    let endpoint_addr = 1;
    let mut queue = interface.bulk_in_queue(0x80 + endpoint_addr);
    let transfer_size = 64;

    loop {
        while queue.pending() < 1 {
            queue.submit(nusb::transfer::RequestBuffer::new(transfer_size));
        }

        let completion = futures_lite::future::block_on(queue.next_complete());

        let data = completion.data.as_slice();
        for chunk in data.chunks_exact(2) {
            if let [low, high] = chunk {
                let adc_value = u16::from_le_bytes([*low, *high]);
                //println!("ADC value: {} mV", adc_value);
                println!("{}", adc_value);
            }
        }
        queue.submit(nusb::transfer::RequestBuffer::reuse(
            completion.data,
            transfer_size,
        ));
    }
}
