use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use nusb::transfer::{Queue, RequestBuffer};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

const MAX_SAMPLES: usize = 1000;
const TRANSFER_SIZE: usize = 64;

fn main() -> Result<(), eframe::Error> {
    let di = nusb::list_devices()
        .unwrap()
        .find(|d| d.vendor_id() == 0xc0de && d.product_id() == 0xcafe)
        .expect("device should be connected");

    eprintln!("Device info: {di:?}");

    let device = di.open().unwrap();
    let interface = device.claim_interface(0).unwrap();

    let endpoint_addr = 1;
    let queue = interface.bulk_in_queue(0x80 + endpoint_addr);

    let samples = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_SAMPLES)));
    let samples_clone = Arc::clone(&samples);

    // Start USB reading thread
    thread::spawn(move || {
        usb_reading_thread(queue, samples_clone);
    });

    let options = eframe::NativeOptions::default();
    let app = ADCApp { samples };
    eframe::run_native(
        "ADC Visualization",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

fn usb_reading_thread(mut queue: Queue<RequestBuffer>, samples: Arc<Mutex<VecDeque<u16>>>) {
    loop {
        while queue.pending() < 1 {
            queue.submit(nusb::transfer::RequestBuffer::new(TRANSFER_SIZE));
        }

        let completion = futures_lite::future::block_on(queue.next_complete());
        let data = completion.data.as_slice();
        
        let mut samples = samples.lock().unwrap();
        for chunk in data.chunks_exact(2) {
            if let [low, high] = chunk {
                let adc_value = u16::from_le_bytes([*low, *high]);
                //println!("{adc_value}");
                if samples.len() >= MAX_SAMPLES {
                    samples.pop_front();
                }
                samples.push_back(adc_value);
            }
        }
        drop(samples);

        queue.submit(nusb::transfer::RequestBuffer::reuse(
            completion.data,
            TRANSFER_SIZE,
        ));
    }
}

struct ADCApp {
    samples: Arc<Mutex<VecDeque<u16>>>,
}

impl eframe::App for ADCApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ADC Values");

            let samples = self.samples.lock().unwrap();
            let plot = Plot::new("ADC Plot");
            plot.show(ui, |plot_ui| {
                let points: PlotPoints = samples
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [i as f64, v as f64])
                    .collect();
                let line = Line::new(points);
                plot_ui.line(line);
            });
        });

        ctx.request_repaint();
    }
}