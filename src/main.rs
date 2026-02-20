#![no_std]
#![no_main]

use panic_halt as _;
use rp2040_hal as hal;
use hal::pac;
use core::fmt::Write;
use hal::fugit::RateExtU32;
use rp2040_hal::clocks::Clock;

/// Boot block required for the ROM bootloader
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

const XTAL_FREQ_HZ: u32 = 12_000_000u32;

#[hal::entry]
fn main() -> ! {
    // 1. Initialize Peripherals
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let sio = hal::Sio::new(pac.SIO);

    // 2. Configure Clocks
    let clocks = hal::clocks::init_clocks_and_plls(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .unwrap();

    // 3. Initialize Delay provider
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    // 4. Configure Pins for UART
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // UART TX on GPIO0, RX on GPIO1
    let uart_pins = (
        pins.gpio0.into_function::<hal::gpio::FunctionUart>(),
        pins.gpio1.into_function::<hal::gpio::FunctionUart>(),
    );

    // 5. Initialize UART Peripheral
    let mut uart = hal::uart::UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(
            hal::uart::UartConfig::new(115200.Hz(), hal::uart::DataBits::Eight, None, hal::uart::StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();

    // 6. Infinite Loop
    loop {
        // Use writeln! for easy string formatting
        writeln!(uart, "RyuOS Heartbeat: System OK\r").unwrap();
        
        // Delay for 2000ms (2 seconds)
        delay.delay_ms(2000);
    }
}