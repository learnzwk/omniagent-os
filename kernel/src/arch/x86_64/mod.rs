pub mod gdt;
pub mod idt;
pub mod pic;
pub mod apic;
pub mod port_io;

pub const ARCH_NAME: &str = "x86_64";
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_MASK: usize = !(PAGE_SIZE - 1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_name() {
        assert_eq!(ARCH_NAME, "x86_64");
    }

    #[test]
    fn test_page_size_is_power_of_two() {
        assert!(PAGE_SIZE.is_power_of_two());
    }

    #[test]
    fn test_page_size_is_4k() {
        assert_eq!(PAGE_SIZE, 4096);
    }

    #[test]
    fn test_page_mask() {
        assert_eq!(PAGE_SIZE_MASK & 0x1FFF, 0);
        assert_eq!(PAGE_SIZE_MASK & 0x2000, 0x2000);
    }
}
