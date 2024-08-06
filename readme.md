# Kevin's work-in-progress caliper firmware/software

Please keep code/repo private for now. I'll tidy and open-source later.

Firmware running against [BluePillCaliper](https://github.com/MitkoDyakov/BluePillCaliper/).

## install

First [install Rust](https://www.rust-lang.org/tools/install), then:

    rustup target add thumbv7m-none-eabi
    cargo install probe-rs-tools

For Python analysis stuff, [install Rye](https://rye.astral.sh/).

## firmware/
Rust firmware for v1.1 PCB based on the Embassy framework.

Build and flash via STLink:

    cargo run --release --bin usb_custom

Attach to running firmware:

    probe-rs attach --chip STM32F103C8 target/thumbv7m-none-eabi/release/usb_custom

## frontend/
Custom USB "oscilliscope" and control UI:

    cargo run --release --bin scope



## analysis/

Python analysis

Install:

    rye sync

## Log
### Aug 6

Had a chat with Mitko.
Look into "pulse density modulation" for signal generation. Also read color university paper in repo. Mitko will add patent too.

---

Looking more into minimal reproducible example for the hang I've been running into.

It seems like the issue may be resolved by doing a full power reset when flashing new firmware.
So maybe something about the USB bus is getting corrupted in such a way that causes the later hang?

I do need to trigger interrupt when USB disconnects --- right now the USB tasks just hang in wfe.
(discovered this via unplugging and then Ctrl-C probe-run, which listed /Users/dev/.cargo/git/checkouts/embassy-9312dcb0ed774b29/46aa206/embassy-executor/src/arch/cortex_m.rs:106:21)

Managed to catch and debug registers via STM32Cube.
ICSR
  ISRPending 1
  VectPending 0x024


This vect points to USB stuff, but I didn't see any obvious error flags in the USB registers.

Had another freeze and captured the pending interrupt as 0x01B, which is DMA1_CHANNEL5

// TODO: possible I need to bind 
    // DMA1_CHANNEL5 => timer::UpdateInterruptHandler<peripherals::TIM1>;
    
    
When the app is running fine, though, and I read this register, it fluctuates between:

0x01B
0x01F

and also (when USB is being read)

0x024

I'm not sure how Claude knew 0x024 was USB --- I'd like to see this in a datasheet

the enum values in https://docs.embassy.dev/embassy-stm32/git/stm32f103c8/interrupt/enum.Interrupt.html don't match


---

- if code is mapped into RAM, it's possible stack could clobber it if it's too big.
debuggers
  - uses jetbrains integrated debugger in RustRover.
  - espressif has onboard debugger over USB
  - set breakpoints, watchpoints etc.
  - break on interrupt
  
  
---

crash still occurs when debugger data cables disconnected


tried adding my own hardfault handler

use cortex_m_rt::{exception, ExceptionFrame};
#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    loop {}
}

but didn't end up there on the freeze. (all registers point to 0x2100_0000
  
port install gdb +multiarch

break on exception.



### Aug 5 - debugging


https://matrix.to/#/!YoLPkieCYHGzdjUhOK:matrix.org/$dOadSX4X9q9CnQEiKhKyZJr5toI8R8q08fk8qE3VloE?via=matrix.org&via=tchncs.de&via=mozilla.org



14:49:40 $ /Applications/STMicroelectronics/STM32Cube/STM32CubeProgrammer/STM32CubeProgrammer.app/Contents/MacOs/bin/STM32_Programmer_CLI -c port=SWD
      -------------------------------------------------------------------
                        STM32CubeProgrammer v2.17.0                  
      -------------------------------------------------------------------

ST-LINK SN  : 53FF6C064884534937360587
ST-LINK FW  : V2J37S7
Board       : --
Voltage     : 3.24V
SWD freq    : 4000 KHz
Connect mode: Normal
Reset mode  : Software reset
Device ID   : 0x410
Revision ID : Rev X
Device name : STM32F101/F102/F103 Medium-density
Flash size  : 64 KBytes
Device type : MCU
Device CPU  : Cortex-M3
BL Version  : --


I didn't see anything in errata that seemed sus: https://www.st.com/resource/en/errata_sheet/es0340-stm32f101xcde-stm32f103xcde-device-errata-stmicroelectronics.pdf


cargo install cargo-binutils
rustup component add llvm-tools-preview

I can print out assembly of my program now via

    cargo objdump --bin usb_custom --release -- -d --no-show-raw-insn --print-imm-hex

that doesn't help me yet, since I don't have any fault handlers pointing to what went wrong.


### Aug 3 - ADC pickup
DMA ring buffer is working.
the duplicates I was getting earlier is because stop disables the circularity on the channel, but start doesn't restore it.
so if you call stop once, you get screwed forever.

(I should just reset the device if USB ever disconnects)

based on defmt timing, each loop takes about 600--700us.
awaiting USB causes us to overrun DMA, so I may need to have a larger buffer and do that in a separate task.


Ugh. I have no idea what's fucked here.
Even when not trying to send over USB, eventually the ringbuffer freezes somehow and the await loop stops.


ugh, maybe it's bad wiring?
just reflashing and fucking about, no real changes and now it's streaming alnog fine.

    // use embassy_stm32::interrupt;
    // #[interrupt]
    // fn DMA1_CHANNEL1() {
    //     info!("interrupt");
    // }

yeah, very intermittent.
Everything seems to be working fine now. this is infuriating.



### Aug 2 - signal emission and ADC pickup
Claude helped me make a little USB scope visualization.

For the life of me I can't seem to affect the waveform, though.
The average value seems to change on every reset.

There's no pulldown, so I thought it might be a floating voltage.
But trying to clear it with

    {
        let _adc_pin = Output::new(p.PB1, Level::Low, Speed::VeryHigh);
        Timer::after_millis(1000).await;
        // dropping returns to high impedence
    }

doesn't affect things.
average value can be anywhere from 800 to 1400.
Though I guess looking at the schematic the floating is occucring before the MCP6S21 amplifier.


Let's rule out it's not some USB delay thing by generating a cycling counter.

Yeah, USB and egui rendering look superfast, no problems there.

Maybe the ADC isn't actually sampling properly and it's repeating itself?

Yeah, I think that's what's happening. scrolling through the output file, the readings are super identical.


Okay, it's something in embassy ring buffer.
If I have DMA write to a static mut and I just YOLO read from it, the values are definitely change

from looking at https://github.com/embassy-rs/embassy/blob/a2ea2630f648a9a46dc4f9276d9f5eaf256ae893/embassy-stm32/src/adc/ringbuffered_v2.rs#L122 it seems like I'm holding the ring buffer correctly. hmmm.



### July 31 - Rust USB ADC data streaming.
Got this working first by porting Embassy USB CDC example, but on MacOS there's some kind of internal buffer that waits to fill up before any results are printed out of the file.
It takes 10+ seconds so is probably a few MB.

I used Cursor to generate, then had it do a custom USB class protocol thing so I can bypass the CDC_ACM virtual serial port stuff.

It took a bit of coaxing, but I managed to get something working with a custom reader too.

Embassy seems to have nice DMA stuff, but it doesn't seem to support F1-series.
https://github.com/embassy-rs/embassy/pull/3116

I should just give up on this for now and stream naively.





### July 29 - Arudino test

installed arduino-ide_2.3.2_macOS_arm64


followed https://community.st.com/t5/stm32-mcus/how-to-program-and-debug-the-stm32-using-the-arduino-ide/ta-p/608514

add board manager URL in preferences: https://github.com/stm32duino/BoardManagerFiles/raw/main/package_stmicroelectronics_index.json

installed STM32 MCU based boards version 2.8.1

Trying to upload:

    Sketch uses 14896 bytes (45%) of program storage space. Maximum is 32768 bytes.
    Global variables use 1632 bytes (15%) of dynamic memory, leaving 8608 bytes for local variables. Maximum is 10240 bytes.
    STM32CubeProgrammer not found (STM32_Programmer_CLI).
      Please install it or add '<STM32CubeProgrammer path>/bin' to your PATH environment:
      https://www.st.com/en/development-tools/stm32cubeprog.html

Aight, had to install cube stuff but then the arduino upload worked:


    Sketch uses 14896 bytes (45%) of program storage space. Maximum is 32768 bytes.
    Global variables use 1632 bytes (15%) of dynamic memory, leaving 8608 bytes for local variables. Maximum is 10240 bytes.
    Warning: long options not supported due to getopt from FreeBSD usage.
    Selected interface: swd
    -------------------------------------------------------------------
    STM32CubeProgrammer v2.17.0                  
    -------------------------------------------------------------------

    ST-LINK SN  : 53FF6C064884534937360587
    ST-LINK FW  : V2J37S7
    Board       : --
    Voltage     : 3.25V
    SWD freq    : 4000 KHz
    Connect mode: Under Reset
    Reset mode  : Hardware reset
    Device ID   : 0x410
    Revision ID : Rev X
    Device name : STM32F101/F102/F103 Medium-density
    Flash size  : 64 KBytes
    Device type : MCU
    Device CPU  : Cortex-M3
    BL Version  : --



    Memory Programming ...
    Opening and parsing file: Base.ino.bin
    File          : Base.ino.bin
    Size          : 14.84 KB 
    Address       : 0x08000000 


    Erasing memory corresponding to segment 0:
    Erasing internal memory sectors [0 14]
    Download in Progress:


    File download complete
    Time elapsed during download operation: 00:00:01.048

    RUNNING Program ... 
    Address:      : 0x8000000
    Application is running, Please Hold on...
    Start operation achieved successfully

    
Was also able to get serial monitor working by setting USB Support Generic Serial CDC.
Man, Arduino is nicer than Rust lol.


The Arduino serial monitor only showed 100 points at a time (ugh!) but I found https://github.com/hacknus/serial-monitor-rust which worked great.

uggh, serial monitor isn't actually live.
Seems like it must have some buffer or otherwise be dropping stuff on the floor.
Recording a few seconds and then saving a CSV only gives a 1000-ish data.

but using minicom

minicom -D /dev/tty.usbmodem4995277E384B1 -b 115200 -C foo.csv

gives 10x the data.



### 2024 July 29 - hardware connection test
Connected via stlink and jtag pins as per https://github.com/MitkoDyakov/BluePillCaliper/blob/main/Hardware/Schematics%20V1.1.pdf

curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh


$ probe-rs info
Probing target via JTAG

 WARN probe_rs::probe::stlink: send_jtag_command 242 failed: JtagGetIdcodeError
Error identifying target using protocol JTAG: An error with the usage of the probe occurred

Probing target via SWD

 WARN probe_rs::probe::stlink: send_jtag_command 242 failed: JtagGetIdcodeError
Error identifying target using protocol SWD: An error with the usage of the probe occurred


ah, I was reading the schematic incorrectly. Managed to connect:

$ probe-rs info
Probing target via JTAG

ARM Chip with debug port Default:
Debug Port: DPv1, DP Designer: ARM Ltd
└── 0 MemoryAP
    └── ROM Table (Class 1), Designer: STMicroelectronics
        ├── Cortex-M3 SCS   (Generic IP component)
        │   └── CPUID
        │       ├── IMPLEMENTER: ARM Ltd
        │       ├── VARIANT: 1
        │       ├── PARTNO: Cortex-M3
        │       └── REVISION: 1
        ├── Cortex-M3 DWT   (Generic IP component)
        ├── Cortex-M3 FBP   (Generic IP component)
        ├── Cortex-M3 ITM   (Generic IP component)
        └── Cortex-M3 TPIU  (Coresight Component)
