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
    let threshold = Arc::new(Mutex::new(None));
    let threshold_clone = Arc::clone(&threshold);

    // Start USB reading thread
    thread::spawn(move || {
        usb_reading_thread(queue, samples_clone, threshold_clone);
    });

    let options = eframe::NativeOptions::default();
    let app = ADCApp { samples, threshold };
    eframe::run_native(
        "ADC Visualization",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

fn usb_reading_thread(
    mut queue: Queue<RequestBuffer>,
    samples: Arc<Mutex<VecDeque<u16>>>,
    threshold: Arc<Mutex<Option<u16>>>,
) {
    let mut triggered = false;
    let mut prev_value = 0;

    loop {
        while queue.pending() < 1 {
            queue.submit(nusb::transfer::RequestBuffer::new(TRANSFER_SIZE));
        }

        let completion = futures_lite::future::block_on(queue.next_complete());
        let data = completion.data.as_slice();

        let threshold = *threshold.lock().unwrap();
        let mut samples = samples.lock().unwrap();
        for chunk in data.chunks_exact(2) {
            if let [low, high] = chunk {
                let adc_value = u16::from_le_bytes([*low, *high]);

                match threshold {
                    Some(threshold) => {
                        if triggered {
                            samples.push_back(adc_value);
                            if samples.len() >= MAX_SAMPLES {
                                triggered = false;
                            }
                        } else {
                            if prev_value <= threshold && adc_value > threshold {
                                triggered = true;
                                samples.clear();
                            }
                            prev_value = adc_value;
                        }
                    }
                    None => {
                        samples.push_back(adc_value);
                        if samples.len() >= MAX_SAMPLES {
                            samples.pop_front();
                        }
                    }
                }
            }
        }

        queue.submit(nusb::transfer::RequestBuffer::reuse(
            completion.data,
            TRANSFER_SIZE,
        ));
    }
}

struct ADCApp {
    samples: Arc<Mutex<VecDeque<u16>>>,
    threshold: Arc<Mutex<Option<u16>>>,
}

impl eframe::App for ADCApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("controls").show(ctx, |ui| {
            ui.heading("Controls");
            let mut threshold = self.threshold.lock().unwrap().unwrap_or(0);
            ui.add(egui::Slider::new(&mut threshold, 0..=4000).text("Trigger"));
            *self.threshold.lock().unwrap() = if threshold == 0 {
                None
            } else {
                Some(threshold)
            };
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ADC Values");

            let samples = self.samples.lock().unwrap();
            let threshold = *self.threshold.lock().unwrap(); // Create a copy and immediately drop the lock
            let plot = Plot::new("ADC Plot")
                .include_y(0.0)
                .include_y(10000.0)
                .include_x(MAX_SAMPLES as f64);
            plot.show(ui, |plot_ui| {
                let points: PlotPoints = samples
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| [i as f64, v as f64])
                    .collect();
                let line = Line::new(points);
                plot_ui.line(line);

                // Add horizontal line for non-zero threshold
                if let Some(threshold) = threshold {
                    let threshold_line = Line::new(vec![
                        [0.0, threshold as f64],
                        [MAX_SAMPLES as f64, threshold as f64],
                    ])
                    .color(egui::Color32::BLUE)
                    .name("Trigger Threshold");
                    plot_ui.line(threshold_line);
                }
            });
        });

        ctx.request_repaint();
    }
}
