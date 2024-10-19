#![allow(non_snake_case)]

// This binary sweeps the frequency of the PDM signal and records the ADC values to a file.
// Use with "Recorder" firmware.

use schema::Command;
use std::io::{BufWriter, Write};

fn main() {
    let di = nusb::list_devices()
        .unwrap()
        .find(|d| d.vendor_id() == 0xc0de && d.product_id() == 0xcafe)
        .expect("device should be connected");

    eprintln!("Device info: {di:?}");

    let device = di.open().unwrap();

    let interface = device.claim_interface(0).unwrap();

    let file = std::fs::File::create("frequency_sweep.csv").unwrap();
    let mut csv_writer = BufWriter::new(file);
    writeln!(csv_writer, "frequency, adc").unwrap();

    // Send frequency command to firmware
    let endpoint_addr = 1;
    let mut out_queue = interface.bulk_out_queue(endpoint_addr);

    // Read and print ADC values
    let mut queue = interface.bulk_in_queue(0x80 + endpoint_addr);
    let transfer_size = 64;

    for frequency_kHz in (100..1000).step_by(20) {
        send_command(
            &mut out_queue,
            Command::SetFrequency {
                frequency_kHz: frequency_kHz as f64,
            },
        );

        send_command(&mut out_queue, Command::Record);
        let num_recorded_packets = 64;
        let mut samples = Vec::new();
        for _packet_idx in 0..num_recorded_packets {
            while queue.pending() < 1 {
                queue.submit(nusb::transfer::RequestBuffer::new(transfer_size));
            }

            let completion = futures_lite::future::block_on(queue.next_complete());

            let data = completion.data.as_slice();
            for chunk in data.chunks_exact(2) {
                if let [low, high] = chunk {
                    let adc_value = u16::from_le_bytes([*low, *high]);
                    samples.push(adc_value);
                }
            }
            queue.submit(nusb::transfer::RequestBuffer::reuse(
                completion.data,
                transfer_size,
            ));
        }

        println!(
            "Recorded {} samples at {} kHz",
            samples.len(),
            frequency_kHz
        );

        for sample in samples {
            writeln!(csv_writer, "{},{}", frequency_kHz * 1000, sample).unwrap();
        }
    }

    csv_writer.flush().unwrap();
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
