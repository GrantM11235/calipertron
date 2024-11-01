#![no_std]
#![no_main]
use schema::*;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::adc::Adc;
use embassy_stm32::dma::*;
use embassy_stm32::gpio::{Flex, Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::{adc, bind_interrupts, peripherals, usb, Config};
use embassy_time::Timer;
use embassy_usb::driver::{Endpoint, EndpointIn, EndpointOut};
use embassy_usb::Builder;
use {defmt_rtt as _, panic_probe as _};

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

bind_interrupts!(struct Irqs {
    USB_LP_CAN1_RX0 => usb::InterruptHandler<peripherals::USB>;
});

const MAX_PACKET_SIZE: u8 = 64;
const SAMPLES_PER_PACKET: usize = (MAX_PACKET_SIZE as usize) / 2; // 2 bytes per sample
const NUM_SAMPLES: usize = SAMPLES_PER_PACKET * 128;

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

    let tim = embassy_stm32::timer::low_level::Timer::new(p.TIM2);
    let timer_registers = tim.regs_gp16();
    timer_registers
        .cr2()
        .modify(|w| w.set_ccds(embassy_stm32::pac::timer::vals::Ccds::ONUPDATE));
    timer_registers.dier().modify(|w| {
        // Enable update DMA request
        w.set_ude(true);
        // Enable update interrupt request
        w.set_uie(true);
    });

    tim.set_frequency(Hertz(100_000));

    let start_pdm = || unsafe {
        let mut opts = TransferOptions::default();
        opts.circular = true;

        let dma_ch = embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH2);
        let request = embassy_stm32::timer::UpDma::request(&dma_ch);

        tim.reset();

        let t = Transfer::new_write(
            dma_ch,
            request,
            &PDM_SIGNAL,
            embassy_stm32::pac::GPIOA.bsrr().as_ptr() as *mut u32,
            opts,
        );

        tim.start();
        t
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

    let start_adc = |sample_buf| unsafe {
        let dma_ch = embassy_stm32::Peripheral::clone_unchecked(&p.DMA1_CH1);
        let request = embassy_stm32::adc::RxDma::request(&dma_ch);
        let opts = TransferOptions::default();

        let t = Transfer::new_read(
            dma_ch,
            request,
            embassy_stm32::pac::ADC1.dr().as_ptr() as *mut u16,
            sample_buf,
            opts,
        );

        // Start ADC conversions
        embassy_stm32::pac::ADC1.cr2().modify(|w| w.set_adon(true));
        t
    };

    let mut adc = Adc::new(p.ADC1);

    let vrefint_sample = {
        let mut vrefint = adc.enable_vref();

        // give vref some time to warm up
        Timer::after_millis(100).await;

        adc.read(&mut vrefint).await as u32
    };
    info!("VREFINT: {}", vrefint_sample);

    //let convert_to_millivolts = |sample| (sample as u32 * adc::VREF_INT / vrefint_sample) as u16;

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

    //////////////////////////
    // handle commands from host
    let fut_commands = async {
        // Wait for USB to connect
        read_ep.wait_enabled().await;

        // Wait for USB to connect
        write_ep.wait_enabled().await;

        info!("Ready");

        loop {
            let mut command_buf = [0u8; MAX_PACKET_SIZE as usize];

            match read_ep.read(&mut command_buf).await {
                Ok(size) => {
                    if let Some(command) = Command::deserialize(&command_buf[..size]) {
                        info!("Received command: {:?}", command);
                        use Command::*;
                        match command {
                            SetFrequency {
                                frequency_kHz,
                                adc_sampling_period,
                            } => {
                                tim.set_frequency(Hertz((frequency_kHz * 1000.) as u32));

                                adc.smpr2().modify(|w| {
                                    w.set_smp(
                                        PIN_CHANNEL as usize,
                                        match adc_sampling_period {
                                            AdcSamplingPeriod::CYCLES1_5 => {
                                                adc::SampleTime::CYCLES1_5
                                            }
                                            AdcSamplingPeriod::CYCLES7_5 => {
                                                adc::SampleTime::CYCLES7_5
                                            }
                                            AdcSamplingPeriod::CYCLES13_5 => {
                                                adc::SampleTime::CYCLES13_5
                                            }
                                            AdcSamplingPeriod::CYCLES28_5 => {
                                                adc::SampleTime::CYCLES28_5
                                            }
                                            AdcSamplingPeriod::CYCLES41_5 => {
                                                adc::SampleTime::CYCLES41_5
                                            }
                                            AdcSamplingPeriod::CYCLES55_5 => {
                                                adc::SampleTime::CYCLES55_5
                                            }
                                            AdcSamplingPeriod::CYCLES71_5 => {
                                                adc::SampleTime::CYCLES71_5
                                            }
                                            AdcSamplingPeriod::CYCLES239_5 => {
                                                adc::SampleTime::CYCLES239_5
                                            }
                                        },
                                    )
                                })
                            }

                            // would be nice to extract this, but async closures aren't stable yet and no way in hell I'm going to write out the types.
                            Record => {
                                // TODO: I'd rather this be local, but Transfer requires the buffer have the same lifetime as the DMA channel for some reason.
                                static mut ADC_BUF: [u16; NUM_SAMPLES] = [0u16; NUM_SAMPLES];

                                let buf = unsafe { &mut ADC_BUF[..] };

                                // start ADC
                                let adc_transfer = start_adc(buf);

                                // start PDM
                                let mut pdm_transfer = start_pdm();

                                // wait for all of the samples to be taken
                                adc_transfer.await;
                                // TODO: why am I getting errors about multiple mutable borrows --- shouldn't awaiting the adc_transfer above end the borrow?
                                let buf = unsafe { &mut ADC_BUF[..] };

                                pdm_transfer.request_stop();

                                // now we can send the collected results back to the host

                                // for x in buf.iter_mut() {
                                //     *x = convert_to_millivolts(*x);
                                // }
                                for c in buf.chunks(SAMPLES_PER_PACKET) {
                                    let r = write_ep.write(bytemuck::cast_slice(c)).await;
                                    if r.is_err() {
                                        error!("USB Error: {:?}", r);
                                        break;
                                    }
                                }

                                // make sure everything is reset before we continue
                                pdm_transfer.await;
                            }
                        }
                    } else {
                        error!("Failed to deserialize command");
                    }
                }
                Err(e) => error!("Failed to read USB packet: {:?}", e),
            }
        }
    };

    // Pinning and using join_array saves 1kB of flash compared to join3. (Presumably reduced code size.)
    // embassy_futures::join::join3(fut_commands, fut_usb, fut_stream_adc).await;

    let fut_commands = core::pin::pin!(fut_commands);
    let fut_usb = core::pin::pin!(fut_usb);

    let futures: [core::pin::Pin<&mut dyn core::future::Future<Output = _>>; 2] =
        [fut_usb, fut_commands];
    embassy_futures::join::join_array(futures).await;
}
