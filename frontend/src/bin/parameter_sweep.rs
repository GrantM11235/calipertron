#![allow(non_snake_case)]

// This binary sweeps the frequency of the PDM signal and records the ADC values to a file.
// Use with "Recorder" firmware.

use schema::*;
use std::io::{BufWriter, Write};
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create("parameter_sweep.csv")?;
    let mut csv_writer = BufWriter::new(file);
    writeln!(csv_writer, "pdm_frequency,adc_period,n,sample")?;

    for frequency_kHz in (25..150).step_by(5) {
        use AdcSamplingPeriod::*;
        for adc_sampling_period in &[
            CYCLES1_5,
            CYCLES7_5,
            CYCLES13_5,
            CYCLES28_5,
            CYCLES41_5,
            CYCLES55_5,
            CYCLES71_5,
            CYCLES239_5,
        ] {
            'connection: loop {
                let Some(di) = nusb::list_devices()?
                    .find(|d| d.vendor_id() == 0xc0de && d.product_id() == 0xcafe)
                else {
                    continue 'connection;
                };

                let Ok(device) = di.open() else {
                    continue 'connection;
                };

                let interface = device.claim_interface(0)?;

                let endpoint_addr = 1;
                let mut out_queue = interface.bulk_out_queue(endpoint_addr);

                let mut queue = interface.bulk_in_queue(0x80 + endpoint_addr);
                let transfer_size = 64;

                send_command(
                    &mut out_queue,
                    Command::SetFrequency {
                        frequency_kHz: frequency_kHz as f64,
                        adc_sampling_period: adc_sampling_period.clone(),
                    },
                );

                send_command(&mut out_queue, Command::Record);

                let num_recorded_packets = 128;
                let mut samples = Vec::new();
                for _packet_idx in 0..num_recorded_packets {
                    if queue.pending() == 0 {
                        queue.submit(nusb::transfer::RequestBuffer::new(transfer_size));
                    }

                    let completion =
                        match timeout(std::time::Duration::from_secs(1), queue.next_complete())
                            .await
                        {
                            Ok(completion) => completion,
                            Err(_) => {
                                println!("Device frozen, please reset");
                                continue 'connection;
                            }
                        };

                    let data = completion.data.as_slice();

                    // Not sure what's going on here, but after device freezes and we reset it and reconnect, we end up getting some 0 length packets.
                    // in that case, just start over
                    if data.len() == 0 {
                        continue 'connection;
                    }

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

                for (idx, sample) in samples.iter().enumerate() {
                    writeln!(
                        csv_writer,
                        "{},{},{},{}",
                        frequency_kHz * 1000,
                        adc_sampling_period.to_seconds(),
                        idx,
                        sample
                    )?;
                }
                break 'connection;
            }
        }
    }

    csv_writer.flush()?;
    Ok(())
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
