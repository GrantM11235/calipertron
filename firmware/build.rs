use std::f64::consts::PI;
use std::fs::File;
use std::io::Write;

fn generate_sine_cosine_table(
    signal_frequency: f64,
    sampling_frequency: f64,
    num_points: usize,
) -> String {
    let mut output = String::new();
    output.push_str("pub const SINE_COSINE_TABLE: [(f32, f32); ");
    output.push_str(&num_points.to_string());
    output.push_str("] = [\n");

    for i in 0..num_points {
        let angle = 2.0 * PI * signal_frequency * (i as f64 * (1.0 / sampling_frequency));
        let sine = angle.sin();
        let cosine = angle.cos();
        output.push_str(&format!("    ({:.6}, {:.6}),\n", sine, cosine));
    }

    output.push_str("];\n");
    output
}

fn main() {
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

    // TODO: reference these from firmware file so they're not duplicated
    let pdm_frequency = 100_000.0; // 100 kHz
    let pdm_length = 132;
    let signal_frequency = pdm_frequency / pdm_length as f64;

    let adc_frequency = 12_000_000.;
    let adc_sample_cycles = 239.5;
    let adc_sample_overhead_cycles = 12.5; // see reference manual section 11.6

    let sampling_frequency = adc_frequency / (adc_sample_cycles + adc_sample_overhead_cycles);

    let num_points = 1000;
    let table_content =
        generate_sine_cosine_table(signal_frequency, sampling_frequency, num_points);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("sine_cosine_table.rs");
    let mut f = File::create(&dest_path).unwrap();
    f.write_all(table_content.as_bytes()).unwrap();

    // Tell Cargo to rerun this script if the source file changes
    println!("cargo:rerun-if-changed=build.rs");
}
