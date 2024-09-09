#![allow(non_snake_case)]

use schema::Command;

fn main() {
    // Parse command-line argument for frequency
    let frequency_kHz = parse_frequency_arg();

    let di = nusb::list_devices()
        .unwrap()
        .find(|d| d.vendor_id() == 0xc0de && d.product_id() == 0xcafe)
        .expect("device should be connected");

    eprintln!("Device info: {di:?}");

    let device = di.open().unwrap();

    let interface = device.claim_interface(0).unwrap();

    // Send frequency command to firmware
    let endpoint_addr = 1;
    let mut out_queue = interface.bulk_out_queue(endpoint_addr);

    send_command(&mut out_queue, Command::SetFrequency { frequency_kHz });

    send_command(&mut out_queue, Command::Record);

    // Read and print ADC values
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
                println!("{}", adc_value);
            }
        }
        queue.submit(nusb::transfer::RequestBuffer::reuse(
            completion.data,
            transfer_size,
        ));
    }
}

fn parse_frequency_arg() -> f64 {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <frequency_kHz>", args[0]);
        std::process::exit(1);
    }

    match args[1].parse::<f64>() {
        Ok(freq) if freq >= 0.0 && freq <= 100.0 => freq,
        _ => {
            eprintln!("Error: Frequency must be a number between 0 and 100 kHz");
            std::process::exit(1);
        }
    }
}

fn send_command(out_queue: &mut nusb::transfer::Queue<Vec<u8>>, command: Command) {
    let mut buf = [0u8; 64]; // Assuming MAX_PACKET_SIZE is 64
    if let Ok(serialized) = command.serialize(&mut buf) {
        out_queue.submit(serialized.into());
    } else {
        eprintln!("Error: Failed to serialize command");
        std::process::exit(1);
    }
}
