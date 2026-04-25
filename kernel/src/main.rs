#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
use core::panic::PanicInfo;

mod vga;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    crate::println!("OmniAgent OS v0.1.0");
    crate::println!("Kernel initialized successfully");
    crate::println!("CPU: x86_64");
    crate::println!("VGA: 80x25 text mode");
    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
