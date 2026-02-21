#![no_std]
#![no_main]

use panic_halt as _;
use rp2040_hal as hal;
use hal::pac;
use usb_device::prelude::*;
use usbd_serial::SerialPort;
use core::str;
use hal::rom_data;

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

    let usb_bus = usb_device::bus::UsbBusAllocator::new(
        hal::usb::UsbBus::new(
            pac.USBCTRL_REGS,
            pac.USBCTRL_DPRAM,
            clocks.usb_clock,
            true,
            &mut pac.RESETS,
        )
    );

    let mut serial = SerialPort::new(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[
            StringDescriptors::default()
                .manufacturer("RyuOS")
                .product("Ryu-Native-Serial")
                .serial_number("001")
        ])
        .unwrap()
        .device_class(2) // Communications Device Class (CDC)
        .build();

    // --- Command Parser State ---
    let mut input_buf = [0u8; 64];
    let mut input_pos = 0usize;

    loop {
        if usb_dev.poll(&mut [&mut serial]) {
            let mut read_buf = [0u8; 16];
            match serial.read(&mut read_buf) {
                Ok(count) if count > 0 => {
                    for i in 0..count {
                        let c = read_buf[i];

                        match c {
                            // 1. Handle Backspace (ASCII 8) or Delete (ASCII 127)
                            8 | 127 => {
                                if input_pos > 0 {
                                    input_pos -= 1;
                                    // Visual erase: Backspace, Space, Backspace
                                    let _ = serial.write(b"\x08 \x08");
                                }
                            }
                            // 2. Handle Newline (Enter)
                            b'\r' | b'\n' => {
                                let _ = serial.write(b"\r\n");
                                if input_pos > 0 {
                                    if let Ok(command_line) = str::from_utf8(&input_buf[..input_pos]) {
                                        handle_command(&mut serial, command_line);
                                    }
                                    input_pos = 0; // Reset buffer
                                }
                                let _ = serial.write(b"> "); // New prompt
                            }
                            // 3. Handle Regular Characters
                            _ => {
                                if input_pos < input_buf.len() {
                                    input_buf[input_pos] = c;
                                    input_pos += 1;
                                    let _ = serial.write(&[c]); // Echo character
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Simple command parser that splits by whitespace
fn handle_command(serial: &mut SerialPort<hal::usb::UsbBus>, line: &str) {
    let mut args = line.split_whitespace();
    
    if let Some(cmd) = args.next() {
        match cmd {
            "ping" => {
                let _ = serial.write(b"pong\r\n");
            }
            "echo" => {
                for arg in args {
                    let _ = serial.write(arg.as_bytes());
                    let _ = serial.write(b" ");
                }
                let _ = serial.write(b"\r\n");
            }
            "about" => {
                let _logo = r#"                                             
                         @-                       
                       **--%                      
                 #******+--#                      
              *@******@.--:            @::--. #   
          .- ********@ @*%#.       .--------..    
            --@*@  %*.---=:--+    - ---#@---      
              -@@@---@--@:#       -:-**---.       
              % .%----=-%         -@** :-         
              @@     -@--*#=%.    ***-#           
                  +#*-----***=.  #**              
                 =***-----****=:***               
                :@***---:-@****=**#               
                -%**@-%--@-@**#=                  
               @#@**%------=*****@                
             .**-*****------**%*----              
             ----=-***--*%---*@-----@             
             -------#--@---+--------              
              *---@---*####@#-----                
                 #=---#*##=-++*@                  
                  +@@     #--@                             

                RyuOS v0.1.0
                Built for RP2040 (Native USB)
                        "#;
                for line in _logo.lines() {
                    let _ = serial.write(line.as_bytes());
                    let _ = serial.write(b"\r\n");
                }
            }
            "reboot" => {
                let _ = serial.write(b"Rebooting...\r\n");
                hal::pac::SCB::sys_reset();
            }
            "bootsel" => {
                let _ = serial.write(b"Jumping to Bootloader...\r\n");
                rom_data::reset_to_usb_boot(0, 0);
            }
            _ => {
                let _ = serial.write(b"Unknown command: ");
                let _ = serial.write(cmd.as_bytes());
                let _ = serial.write(b"\r\n");
            }
        }
    }
}