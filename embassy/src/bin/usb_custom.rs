#![no_std]
#![no_main]

use defmt::{panic, *};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::adc::Adc;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::ADC1;
use embassy_stm32::time::Hertz;
use embassy_stm32::{adc, bind_interrupts, peripherals, usb, Config};
use embassy_time::Timer;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::Builder;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USB_LP_CAN1_RX0 => usb::InterruptHandler<peripherals::USB>;
});

bind_interrupts!(struct AdcIrqs {
    ADC1_2 => adc::InterruptHandler<ADC1>;
});

const MAX_PACKET_SIZE: u8 = 64;

pub const USB_CLASS_CUSTOM: u8 = 0xFF;
const USB_SUBCLASS_CUSTOM: u8 = 0x00;
const USB_PROTOCOL_CUSTOM: u8 = 0x00;

pub struct CustomClass<'d, D: Driver<'d>> {
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
}

impl<'d, D: Driver<'d>> CustomClass<'d, D> {
    pub fn new(builder: &mut Builder<'d, D>) -> Self {
        let mut func = builder.function(USB_CLASS_CUSTOM, USB_SUBCLASS_CUSTOM, USB_PROTOCOL_CUSTOM);
        let mut iface = func.interface();
        let mut iface_alt = iface.alt_setting(
            USB_CLASS_CUSTOM,
            USB_SUBCLASS_CUSTOM,
            USB_PROTOCOL_CUSTOM,
            None,
        );
        let read_ep = iface_alt.endpoint_bulk_out(MAX_PACKET_SIZE as u16);
        let write_ep = iface_alt.endpoint_bulk_in(MAX_PACKET_SIZE as u16);

        CustomClass { read_ep, write_ep }
    }

    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.write_ep.write(data).await
    }

    pub async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    pub async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL9,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }
    let mut p = embassy_stm32::init(config);

    info!("Hello World!");

    {
        // Board has a pull-up resistor on the D+ line; pull it down to send a RESET condition to the USB bus.
        // This forced reset is needed only for development, without it host will not reset your device when you upload new firmware.
        let _dp = Output::new(&mut p.PA12, Level::Low, Speed::Low);
        Timer::after_millis(10).await;
    }

    let driver = embassy_stm32::usb::Driver::new(p.USB, Irqs, p.PA12, p.PA11);
    let (vid, pid) = (0xc0de, 0xcafe);
    let mut config = embassy_usb::Config::new(vid, pid);
    config.max_packet_size_0 = MAX_PACKET_SIZE;

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no msos descriptors
        &mut control_buf,
    );

    let mut custom = CustomClass::new(&mut builder);
    let mut usb = builder.build();
    let usb_fut = usb.run();

    let mut adc = Adc::new(p.ADC1);
    let mut pin = p.PB1;

    let fut = async {
        loop {
            custom.wait_connection().await;
            info!("Connected");
            let _ = stream_adc(&mut custom, &mut adc, &mut pin).await;
            info!("Disconnected");
        }
    };

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, fut).await;
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn stream_adc<'d, D: Driver<'d>>(
    custom: &mut CustomClass<'d, D>,
    adc: &mut Adc<'d, ADC1>,
    pin: &mut impl adc::AdcChannel<ADC1>,
) -> Result<(), Disconnected> {
    let mut vrefint = adc.enable_vref();
    let vrefint_sample = adc.read(&mut vrefint).await;
    let convert_to_millivolts =
        |sample| (u32::from(sample) * adc::VREF_INT / u32::from(vrefint_sample)) as u16;

    let mut buf = [0u8; MAX_PACKET_SIZE as usize];
    let samples_per_packet = (MAX_PACKET_SIZE as usize) / 2; // 2 bytes per sample

    loop {
        for i in 0..samples_per_packet {
            let v = adc.read(pin).await;
            let mv = convert_to_millivolts(v);
            buf[(i * 2)..(i * 2 + 2)].copy_from_slice(&mv.to_be_bytes());
        }
        custom.write_packet(&buf).await?;
    }
}
