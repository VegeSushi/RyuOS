#![no_std]
#![no_main]

extern crate alloc;

use embedded_alloc::TlsfHeap as Heap;
use panic_halt as _;
use rp2040_hal as hal;
use hal::pac;
use usb_device::prelude::*;
use usbd_serial::SerialPort;
use core::str;
use hal::rom_data;
use alloc::string::String;

#[global_allocator]
static ALLOCATOR: Heap = Heap::empty();

static mut SCRIPT_BUFFER: Option<String> = None;
static mut IN_EDITOR: bool = false;
static mut CURSOR_POS: usize = 0;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

#[hal::entry]
fn main() -> ! {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 32 * 1024;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        
        // We use addr_of_mut! to get a raw pointer without creating an intermediate reference.
        // This satisfies the new safety requirements for mutable statics.
        unsafe {
            let ptr = core::ptr::addr_of_mut!(HEAP_MEM) as usize;
            ALLOCATOR.init(ptr, HEAP_SIZE);
        }
    }

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
    let mut line_buf = [0u8; 128];
    let mut line_pos = 0usize;
    
    // State machine for ANSI Escape Sequences
    let mut esc_state = 0u8; 

    loop {
        if usb_dev.poll(&mut [&mut serial]) {
            let mut read_buf = [0u8; 16];
            if let Ok(count) = serial.read(&mut read_buf) {
                for i in 0..count {
                    let c = read_buf[i];
                    unsafe {
                        if IN_EDITOR {
                            handle_editor_input(&mut serial, c, &mut esc_state);
                        } else {
                            match c {
                                b'\r' | b'\n' => {
                                    let _ = serial.write(b"\r\n");
                                    if line_pos > 0 {
                                        if let Ok(line) = str::from_utf8(&line_buf[..line_pos]) {
                                            handle_command(&mut serial, line);
                                        }
                                        line_pos = 0;
                                    }
                                    if !IN_EDITOR {
                                        let _ = serial.write(b"> ");
                                    }
                                }
                                8 | 127 => {
                                    if line_pos > 0 {
                                        line_pos -= 1;
                                        let _ = serial.write(b"\x08 \x08");
                                    }
                                }
                                _ => {
                                    if line_pos < line_buf.len() {
                                        line_buf[line_pos] = c;
                                        line_pos += 1;
                                        let _ = serial.write(&[c]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

unsafe fn handle_editor_input(serial: &mut SerialPort<hal::usb::UsbBus>, c: u8, esc_state: &mut u8) {
    let script_ptr = core::ptr::addr_of_mut!(SCRIPT_BUFFER);
    if let Some(buf) = (*script_ptr).as_mut() {
        match *esc_state {
            0 => { // Normal Mode
                match c {
                    27 => *esc_state = 1,
                    24 => { // CTRL+X
                        IN_EDITOR = false;
                        let _ = serial.write(b"\r\n[ Saved ]\r\n> ");
                    }
                    b'\t' => { // Tab Support: Insert 4 spaces
                        for _ in 0..4 {
                            buf.insert(CURSOR_POS, ' ');
                            CURSOR_POS += 1;
                        }
                        refresh_screen(serial, buf);
                    }
                    b'\r' | b'\n' => {
                        // 1. Insert the newline first
                        buf.insert(CURSOR_POS, '\n');
                        CURSOR_POS += 1;

                        // 2. Auto-Indent: Calculate indentation from the previous line
                        let bytes = buf.as_bytes();
                        let mut line_start = CURSOR_POS - 1; 

                        // Move back to the start of the line we just finished
                        // We check bytes[line_start - 1] to find the previous newline
                        while line_start > 0 && bytes[line_start - 1] != b'\n' {
                            line_start -= 1;
                        }

                        // Count leading spaces on that previous line
                        let mut space_count = 0;
                        while line_start + space_count < CURSOR_POS - 1 {
                            if bytes[line_start + space_count] == b' ' {
                                space_count += 1;
                            } else {
                                break;
                            }
                        }

                        // 3. Apply the indentation to the new line
                        for _ in 0..space_count {
                            buf.insert(CURSOR_POS, ' ');
                            CURSOR_POS += 1;
                        }

                        refresh_screen(serial, buf);
                    }
                    8 | 127 => { // Backspace
                        if CURSOR_POS > 0 {
                            CURSOR_POS -= 1;
                            buf.remove(CURSOR_POS);
                            refresh_screen(serial, buf);
                        }
                    }
                    _ => {
                        if c >= 32 && c <= 126 {
                            buf.insert(CURSOR_POS, c as char);
                            CURSOR_POS += 1;
                            refresh_screen(serial, buf);
                        }
                    }
                }
            }
            1 => { if c == b'[' { *esc_state = 2; } else { *esc_state = 0; } }
            2 => { // ANSI Sequence Handler
                match c {
                    b'A' => CURSOR_POS = find_vertical_pos(buf, CURSOR_POS, true),
                    b'B' => CURSOR_POS = find_vertical_pos(buf, CURSOR_POS, false),
                    b'C' => if CURSOR_POS < buf.len() { CURSOR_POS += 1; },
                    b'D' => if CURSOR_POS > 0 { CURSOR_POS -= 1; },
                    _ => {}
                }
                *esc_state = 0;
                refresh_screen(serial, buf);
            }
            _ => *esc_state = 0,
        }
    }
}

fn find_vertical_pos(buf: &str, current_pos: usize, up: bool) -> usize {
    // 1. Find the start of the current line
    let line_start = buf[..current_pos].rfind('\n').map(|n| n + 1).unwrap_or(0);
    let column = current_pos - line_start;

    if up {
        if line_start == 0 { return current_pos; } // Already on top line
        // 2. Find the start of the previous line
        let prev_line_end = line_start - 1;
        let prev_line_start = buf[..prev_line_end].rfind('\n').map(|n| n + 1).unwrap_or(0);
        let prev_line_len = prev_line_end - prev_line_start;
        
        // 3. Aim for the same column, but clamp to line length
        prev_line_start + core::cmp::min(column, prev_line_len)
    } else {
        // 2. Find the start of the next line
        if let Some(next_line_start) = buf[current_pos..].find('\n').map(|n| n + current_pos + 1) {
            let remainder = &buf[next_line_start..];
            let next_line_end = remainder.find('\n').map(|n| n + next_line_start).unwrap_or(buf.len());
            let next_line_len = next_line_end - next_line_start;
            
            // 3. Aim for same column, clamp to next line length
            next_line_start + core::cmp::min(column, next_line_len)
        } else {
            current_pos // Already on bottom line
        }
    }
}

fn refresh_screen(serial: &mut SerialPort<hal::usb::UsbBus>, buf: &str) {
    // 1. Clear Screen and Home Cursor
    let _ = serial.write(b"\x1b[2J\x1b[H"); 
    let _ = serial.write(b"--- RYU EDITOR (CTRL+X to Exit) ---\r\n");
    
    let cursor_idx = unsafe { CURSOR_POS };
    
    // 2. Iterate through buffer and print char-by-char to handle cursor injection
    // We use a small buffer to avoid excessive USB overhead
    for (i, c) in buf.chars().enumerate() {
        // If this is the cursor position, print a visual marker
        if i == cursor_idx {
            let _ = serial.write(b"\x1b[7m \x1b[0m"); // Inverted space (block cursor)
        }

        if c == '\n' {
            let _ = serial.write(b"\r\n"); // The fix for the "broken carriage"
        } else {
            let mut b = [0u8; 4];
            let s = c.encode_utf8(&mut b);
            let _ = serial.write(s.as_bytes());
        }
    }

    // 3. Handle case where cursor is at the very end of the buffer
    if cursor_idx == buf.len() {
        let _ = serial.write(b"\x1b[7m \x1b[0m");
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
            "edit" => unsafe {
                IN_EDITOR = true;
                // Get the raw pointer to the static Option
                let script_ptr = core::ptr::addr_of_mut!(SCRIPT_BUFFER);
    
                // Dereference the pointer to check/modify the Option
                if (*script_ptr).is_none() {
                    *script_ptr = Some(String::new());
                }
    
                // Now safely access the inner String
                if let Some(ref buf) = *script_ptr {
                    refresh_screen(serial, buf);
                }
            },
            "list" => unsafe {
                let script_ptr = core::ptr::addr_of!(SCRIPT_BUFFER);
                if let Some(ref buf) = *script_ptr {
                    let _ = serial.write(b"\r\n--- BUFFER CONTENTS ---\r\n");
                    // We split by lines to ensure we can inject \r for the terminal
                    for line in buf.lines() {
                        let _ = serial.write(line.as_bytes());
                        let _ = serial.write(b"\r\n");
                    }
                    let _ = serial.write(b"-----------------------\r\n");
                } else {
                    let _ = serial.write(b"Buffer is empty.\r\n");
                }
            }
            "run" => unsafe {
                let script_ptr = core::ptr::addr_of!(SCRIPT_BUFFER);
                if let Some(ref code) = *script_ptr {
                    
                } else {
                    let _ = serial.write(b"Buffer empty. Use 'edit' first.\r\n");
                }
            }
            _ => {
                let _ = serial.write(b"Unknown command: ");
                let _ = serial.write(cmd.as_bytes());
                let _ = serial.write(b"\r\n");
            }
        }
    }
}

