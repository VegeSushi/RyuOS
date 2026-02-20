#![no_std]
#![no_main]

use panic_halt as _;
use rp2040_hal as hal;
use hal::pac;

// USB Serial Trait Imports
use usb_device::prelude::*;
use usbd_serial::SerialPort;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

#[hal::entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    let clocks = hal::clocks::init_clocks_and_plls(
        12_000_000u32,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .unwrap();

    // 1. Setup the USB Driver
    // Note the change from pac.USB to USBCTRL_REGS and USBCTRL_DPRAM
    let usb_bus = usb_device::bus::UsbBusAllocator::new(
        hal::usb::UsbBus::new(
            pac.USBCTRL_REGS,
            pac.USBCTRL_DPRAM,
            clocks.usb_clock,
            true,
            &mut pac.RESETS,
        )
    );

    // 2. Setup the Serial Port (CDC)
    let mut serial = SerialPort::new(&usb_bus);

    // 3. Setup the USB Device
    // In usb-device 0.3.x, manufacturer and product are set via strings in the builder
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::default()
            .manufacturer("RyuOS")
            .product("Ryu-Native-Serial")
            .serial_number("001")])
        .unwrap()
        .device_class(2) 
        .build();

    let mut timer = 0u32;

    loop {
        // 4. Poll the USB device
        if usb_dev.poll(&mut [&mut serial]) {
            let mut buf = [0u8; 64];
            let _ = serial.read(&mut buf);
        }

        // 5. Non-blocking delay
        timer += 1;
        if timer >= 1_000_000 { 
            let _ = serial.write(b"RyuOS Heartbeat over Native USB\r\n");
            timer = 0;
        }
    }
}