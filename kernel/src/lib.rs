#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(not(test))]
use core::panic::PanicInfo;

pub mod boot;
pub mod vga;
pub mod drivers;
pub mod arch;
pub mod interrupts;
pub mod memory;
pub mod time;
pub mod syscall;
pub mod agent;
pub mod scheduler;
pub mod fs;
pub mod net;
pub mod security;
pub mod softbus;
pub mod ipc;
pub mod service;
pub mod capability;
pub mod svc_manager;
pub mod device_manager;
pub mod bitmap_scheduler;
pub mod logger;
pub mod config;
pub mod package;
pub mod shell;

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub const KERNEL_VERSION: &str = "0.2.0";
pub const KERNEL_NAME: &str = "OmniAgent OS";

pub fn version() -> &'static str {
    KERNEL_VERSION
}

pub fn name() -> &'static str {
    KERNEL_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_version_is_set() {
        assert!(!version().is_empty());
    }

    #[test]
    fn test_kernel_name_is_set() {
        assert_eq!(name(), "OmniAgent OS");
    }

    #[test]
    fn test_kernel_version_format() {
        let v = version();
        let parts: Vec<&str> = v.split('.').collect();
        assert_eq!(parts.len(), 3);
        for p in &parts {
            assert!(p.parse::<u32>().is_ok(), "version part '{}' is not a number", p);
        }
    }
}
