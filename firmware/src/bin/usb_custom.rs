#![no_std]
#![no_main]

use schema::*;

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::adc::Adc;
use embassy_stm32::gpio::{Flex, Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::{adc, bind_interrupts, peripherals, timer, usb, Config};
use embassy_time::Timer;
use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};
use embassy_usb::Builder;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USB_LP_CAN1_RX0 => usb::InterruptHandler<peripherals::USB>;
});

const MAX_PACKET_SIZE: u8 = 64;
const SAMPLES_PER_PACKET: usize = (MAX_PACKET_SIZE as usize) / 2; // 2 bytes per sample
pub const USB_CLASS_CUSTOM: u8 = 0xFF;
const USB_SUBCLASS_CUSTOM: u8 = 0x00;
const USB_PROTOCOL_CUSTOM: u8 = 0x00;

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

    ////////////////////////
    // Signal emission setup

    let _pins = [
        Output::new(p.PA0, Level::Low, Speed::Low),
        Output::new(p.PA1, Level::Low, Speed::Low),
        Output::new(p.PA2, Level::Low, Speed::Low),
        Output::new(p.PA3, Level::Low, Speed::Low),
        Output::new(p.PA4, Level::Low, Speed::Low),
        Output::new(p.PA5, Level::Low, Speed::Low),
        Output::new(p.PA6, Level::Low, Speed::Low),
        Output::new(p.PA7, Level::Low, Speed::Low),
    ];

    let tim = embassy_stm32::timer::low_level::Timer::new(p.TIM1);
    let timer_registers = tim.regs_advanced();
    timer_registers
        .cr2()
        .modify(|w| w.set_ccds(embassy_stm32::pac::timer::vals::Ccds::ONUPDATE));
    timer_registers.dier().modify(|w| w.set_ude(true)); // Enable update DMA request

    tim.set_frequency(Hertz(1_000));

    tim.start();

    use embassy_stm32::dma::*;
    let gpioa = embassy_stm32::pac::GPIOA;

    let mut opts = TransferOptions::default();
    opts.circular = true;

    let request = embassy_stm32::timer::UpDma::request(&p.DMA1_CH5);
    let _transfer = unsafe {
        Transfer::new_write(
            p.DMA1_CH5,
            request,
            &SIGNAL,
            gpioa.bsrr().as_ptr() as *mut u32,
            opts,
        )
    };

    ////////////////////////
    // USB Setup

    let driver = embassy_stm32::usb::Driver::new(p.USB, Irqs, p.PA12, p.PA11);
    let (vid, pid) = (0xc0de, 0xcafe);
    let mut config = embassy_usb::Config::new(vid, pid);
    config.max_packet_size_0 = MAX_PACKET_SIZE;
    config.product = Some("Calipertron");

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

    let mut func = builder.function(USB_CLASS_CUSTOM, USB_SUBCLASS_CUSTOM, USB_PROTOCOL_CUSTOM);
    let mut iface = func.interface();

    let mut iface_alt = iface.alt_setting(
        USB_CLASS_CUSTOM,
        USB_SUBCLASS_CUSTOM,
        USB_PROTOCOL_CUSTOM,
        None,
    );
    let mut read_ep = iface_alt.endpoint_bulk_out(MAX_PACKET_SIZE as u16);
    let mut write_ep = iface_alt.endpoint_bulk_in(MAX_PACKET_SIZE as u16);
    drop(func);

    let mut usb = builder.build();

    let fut_usb = usb.run();

    ////////////////////////
    // ADC + DMA setup

    let mut adc_buffer = [0; 2 * SAMPLES_PER_PACKET];
    let request = embassy_stm32::adc::RxDma::request(&p.DMA1_CH1);
    let mut opts = TransferOptions::default();
    opts.half_transfer_ir = true;
    let mut adc_rb = unsafe {
        ReadableRingBuffer::new(
            p.DMA1_CH1,
            request,
            embassy_stm32::pac::ADC1.dr().as_ptr() as *mut u16,
            &mut adc_buffer,
            opts,
        )
    };

    let mut adc = Adc::new(p.ADC1);

    let vrefint_sample = {
        let mut vrefint = adc.enable_vref();

        // give vref some time to warm up
        Timer::after_millis(100).await;

        adc.read(&mut vrefint).await as u32
    };
    info!("VREFINT: {}", vrefint_sample);

    let convert_to_millivolts = |sample| (sample as u32 * adc::VREF_INT / vrefint_sample) as u16;

    // Configure ADC for continuous conversion with DMA
    let adc = embassy_stm32::pac::ADC1;

    adc.cr1().modify(|w| {
        w.set_scan(true);
        w.set_eocie(true);
    });

    adc.cr2().modify(|w| {
        w.set_dma(true);
        w.set_cont(true);
    });

    // Configure channel and sampling time
    adc.sqr1().modify(|w| w.set_l(0)); // one conversion.

    // TODO: this may not be necessary
    let mut pb1 = Flex::new(p.PB1);
    pb1.set_as_analog();

    const PIN_CHANNEL: u8 = 9; // PB1 is on channel 9 for STM32F103
    adc.sqr3().modify(|w| w.set_sq(0, PIN_CHANNEL));
    adc.smpr2()
        .modify(|w| w.set_smp(PIN_CHANNEL as usize, adc::SampleTime::CYCLES239_5));

    // Start ADC conversions
    adc.cr2().modify(|w| w.set_adon(true));

    ////////////////////////
    // Stream ADC data to host

    let fut_stream_adc = async {
        // Wait for USB to connect
        write_ep.wait_enabled().await;

        // Start handling DMA requests from ADC
        adc_rb.start();
        let mut buf = [0; SAMPLES_PER_PACKET];
        loop {
            loop {
                let r = adc_rb.read_exact(&mut buf).await;

                if r.is_err() {
                    error!("ADC_RB error: {:?}", r);
                    break;
                }

                for x in buf.iter_mut() {
                    *x = convert_to_millivolts(*x);
                }

                let r = write_ep.write(bytemuck::cast_slice(&buf)).await;
                if r.is_err() {
                    error!("USB Error: {:?}", r);
                    break;
                }
            }

            adc_rb.clear();
        }
    };

    //////////////////////////
    // handle commands from host
    let fut_commands = async {
        // Wait for USB to connect
        read_ep.wait_enabled().await;

        loop {
            let mut command_buf = [0u8; MAX_PACKET_SIZE as usize];

            match read_ep.read(&mut command_buf).await {
                Ok(size) => {
                    if let Some(command) = Command::deserialize(&command_buf[..size]) {
                        info!("Received command: {:?}", command);
                        match command {
                            Command::SetFrequency { frequency_kHz } => {
                                tim.set_frequency(Hertz((frequency_kHz * 1000.) as u32));
                            }
                        }
                    } else {
                        error!("Failed to deserialize command");
                    }
                }
                Err(e) => {
                    error!("Failed to read USB packet: {:?}", e);
                }
            };
        }
    };

    join(fut_commands, join(fut_usb, fut_stream_adc)).await;
}

static SIGNAL: [u32; 132] = [
    0b00000000010101010000000010101010,
    0b00000000010101010000000010101010,
    0b00000000011010100000000010010101,
    0b00000000011010100000000010010101,
    0b00000000010101010000000010101010,
    0b00000000100101010000000001101010,
    0b00000000011010100000000010010101,
    0b00000000011010100000000010010101,
    0b00000000010101010000000010101010,
    0b00000000100101010000000001101010,
    0b00000000011010100000000010010101,
    0b00000000011010100000000010010101,
    0b00000000100101010000000001101010,
    0b00000000100101010000000001101010,
    0b00000000010110100000000010100101,
    0b00000000011010100000000010010101,
    0b00000000100101010000000001101010,
    0b00000000100101010000000001101010,
    0b00000000010110100000000010100101,
    0b00000000010110100000000010100101,
    0b00000000100101010000000001101010,
    0b00000000101001010000000001011010,
    0b00000000010110100000000010100101,
    0b00000000010110100000000010100101,
    0b00000000100101010000000001101010,
    0b00000000101001010000000001011010,
    0b00000000010110100000000010100101,
    0b00000000010110100000000010100101,
    0b00000000101001010000000001011010,
    0b00000000101001010000000001011010,
    0b00000000010101100000000010101001,
    0b00000000010110100000000010100101,
    0b00000000101001010000000001011010,
    0b00000000101001010000000001011010,
    0b00000000010101100000000010101001,
    0b00000000010101100000000010101001,
    0b00000000101001010000000001011010,
    0b00000000101010010000000001010110,
    0b00000000010101100000000010101001,
    0b00000000010101100000000010101001,
    0b00000000101001010000000001011010,
    0b00000000101010010000000001010110,
    0b00000000010101100000000010101001,
    0b00000000010101100000000010101001,
    0b00000000101010010000000001010110,
    0b00000000101010010000000001010110,
    0b00000000010101010000000010101010,
    0b00000000010101100000000010101001,
    0b00000000101010010000000001010110,
    0b00000000101010010000000001010110,
    0b00000000010101010000000010101010,
    0b00000000010101010000000010101010,
    0b00000000101010010000000001010110,
    0b00000000101010100000000001010101,
    0b00000000010101010000000010101010,
    0b00000000010101010000000010101010,
    0b00000000101010010000000001010110,
    0b00000000101010100000000001010101,
    0b00000000010101010000000010101010,
    0b00000000010101010000000010101010,
    0b00000000101010100000000001010101,
    0b00000000101010100000000001010101,
    0b00000000010101010000000010101010,
    0b00000000010101010000000010101010,
    0b00000000101010100000000001010101,
    0b00000000101010100000000001010101,
    0b00000000100101010000000001101010,
    0b00000000010101010000000010101010,
    0b00000000101010100000000001010101,
    0b00000000101010100000000001010101,
    0b00000000100101010000000001101010,
    0b00000000100101010000000001101010,
    0b00000000101010100000000001010101,
    0b00000000011010100000000010010101,
    0b00000000100101010000000001101010,
    0b00000000100101010000000001101010,
    0b00000000101010100000000001010101,
    0b00000000011010100000000010010101,
    0b00000000100101010000000001101010,
    0b00000000100101010000000001101010,
    0b00000000011010100000000010010101,
    0b00000000011010100000000010010101,
    0b00000000101001010000000001011010,
    0b00000000100101010000000001101010,
    0b00000000011010100000000010010101,
    0b00000000011010100000000010010101,
    0b00000000101001010000000001011010,
    0b00000000101001010000000001011010,
    0b00000000011010100000000010010101,
    0b00000000010110100000000010100101,
    0b00000000101001010000000001011010,
    0b00000000101001010000000001011010,
    0b00000000011010100000000010010101,
    0b00000000010110100000000010100101,
    0b00000000101001010000000001011010,
    0b00000000101001010000000001011010,
    0b00000000010110100000000010100101,
    0b00000000010110100000000010100101,
    0b00000000101010010000000001010110,
    0b00000000101001010000000001011010,
    0b00000000010110100000000010100101,
    0b00000000010110100000000010100101,
    0b00000000101010010000000001010110,
    0b00000000101010010000000001010110,
    0b00000000010110100000000010100101,
    0b00000000010101100000000010101001,
    0b00000000101010010000000001010110,
    0b00000000101010010000000001010110,
    0b00000000010110100000000010100101,
    0b00000000010101100000000010101001,
    0b00000000101010010000000001010110,
    0b00000000101010010000000001010110,
    0b00000000010101100000000010101001,
    0b00000000010101100000000010101001,
    0b00000000101010100000000001010101,
    0b00000000101010010000000001010110,
    0b00000000010101100000000010101001,
    0b00000000010101100000000010101001,
    0b00000000101010100000000001010101,
    0b00000000101010100000000001010101,
    0b00000000010101100000000010101001,
    0b00000000010101010000000010101010,
    0b00000000101010100000000001010101,
    0b00000000101010100000000001010101,
    0b00000000010101100000000010101001,
    0b00000000010101010000000010101010,
    0b00000000101010100000000001010101,
    0b00000000101010100000000001010101,
    0b00000000010101010000000010101010,
    0b00000000010101010000000010101010,
    0b00000000011010100000000010010101,
    0b00000000101010100000000001010101,
];
