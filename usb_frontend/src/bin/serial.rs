use std::io;
use std::time::Duration;
use serialport::SerialPort;

fn main() -> io::Result<()> {
    let port_name = "/dev/tty.usbmodem12201";
    let baud_rate = 115200; // Adjust this to match your device's baud rate

    let mut port = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("Failed to open port");

    println!("Serial port opened successfully");

    let mut buffer = [0u8; 64];

    loop {
        match port.read_exact(&mut buffer) {
            Ok(_) => {
                for chunk in buffer.chunks_exact(2) {
                    let adc_value = u16::from_be_bytes([chunk[0], chunk[1]]);
                    println!("ADC value: {} mV", adc_value);
                }
            },
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                println!("Timeout");
                continue;
            },
            Err(e) => return Err(e),
        }
    }
}