//! PS/2 键盘驱动

#[cfg(not(test))]
use alloc::format;

use crate::arch::x86_64::port_io::{inb, outb};
use crate::drivers::serial::SERIAL;

/// 键盘数据端口
const KEYBOARD_DATA_PORT: u16 = 0x60;
/// 键盘状态/命令端口
const KEYBOARD_STATUS_PORT: u16 = 0x64;

/// 键盘命令
const KEYBOARD_ENABLE_SCANNING: u8 = 0xF4;

/// 键盘事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Pressed(u8),
    Released(u8),
}

impl KeyEvent {
    pub fn is_pressed(&self) -> bool {
        matches!(self, KeyEvent::Pressed(_))
    }
    pub fn scancode(&self) -> u8 {
        match self {
            KeyEvent::Pressed(code) | KeyEvent::Released(code) => *code,
        }
    }
}

/// 简单的扫描码到 ASCII 映射 (US QWERTY, Set 1, 仅字母和数字)
fn scancode_to_ascii(scancode: u8) -> Option<char> {
    // Set 1 scancodes for US QWERTY keyboard (lowercase)
    // Index 0x00 = no key, 0x01 = Esc, 0x02-0x0B = 1-0, 0x0C = -, 0x0D = =
    // 0x0E = Backspace, 0x0F = Tab, 0x10-0x19 = q-p, 0x1A = [, 0x1B = ]
    // 0x1C = Enter, 0x1D = (ctrl), 0x1E-0x26 = a-l, 0x27 = ;, 0x28 = '
    // 0x29 = `, 0x2A = (shift), 0x2B = \, 0x2C-0x32 = z-m, 0x33-0x35 = ,./
    // 0x36 = (shift), 0x37 = *, 0x38 = (alt), 0x39 = space
    const SCANCODE_MAP: &[u8] = b"\
\x00\x1B1234567890-=\x08\
\tqwertyuiop[]\n\
\x00asdfghjkl;'`\
\x00\\zxcvbnm,./\
\x00\x00 \
\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\
\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\
7894561230";
    if (scancode as usize) < SCANCODE_MAP.len() {
        let ch = SCANCODE_MAP[scancode as usize];
        if ch != 0 { Some(ch as char) } else { None }
    } else {
        None
    }
}

/// 键盘事件环形缓冲区
const KEYBOARD_BUFFER_SIZE: usize = 128;

pub struct KeyboardBuffer {
    buffer: [Option<KeyEvent>; KEYBOARD_BUFFER_SIZE],
    read_pos: usize,
    write_pos: usize,
}

impl KeyboardBuffer {
    pub const fn new() -> Self {
        Self {
            buffer: [None; KEYBOARD_BUFFER_SIZE],
            read_pos: 0,
            write_pos: 0,
        }
    }

    pub fn push(&mut self, event: KeyEvent) -> bool {
        let next = (self.write_pos + 1) % KEYBOARD_BUFFER_SIZE;
        if next == self.read_pos { return false; } // Full
        self.buffer[self.write_pos] = Some(event);
        self.write_pos = next;
        true
    }

    pub fn pop(&mut self) -> Option<KeyEvent> {
        if self.read_pos == self.write_pos { return None; }
        let event = self.buffer[self.read_pos];
        self.read_pos = (self.read_pos + 1) % KEYBOARD_BUFFER_SIZE;
        event
    }

    pub fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }

    pub fn len(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            KEYBOARD_BUFFER_SIZE - self.read_pos + self.write_pos
        }
    }
}

/// 全局键盘缓冲区
pub static KEYBOARD_BUFFER: spin::Mutex<KeyboardBuffer> = spin::Mutex::new(KeyboardBuffer::new());

/// 初始化键盘
pub unsafe fn init_keyboard() {
    // 等待键盘控制器就绪
    while inb(KEYBOARD_STATUS_PORT) & 0x02 != 0 {}
    // 启用扫描
    outb(KEYBOARD_DATA_PORT, KEYBOARD_ENABLE_SCANNING);
    SERIAL.lock().write_str("[KBD] PS/2 keyboard initialized\n");
}

/// 键盘中断处理函数 (IRQ 1)
pub extern "C" fn keyboard_interrupt_handler() {
    let scancode: u8;
    unsafe { scancode = inb(KEYBOARD_DATA_PORT); }

    let event = if scancode & 0x80 != 0 {
        KeyEvent::Released(scancode & 0x7F)
    } else {
        KeyEvent::Pressed(scancode)
    };

    KEYBOARD_BUFFER.lock().push(event);

    if let Some(ch) = scancode_to_ascii(event.scancode()) {
        if event.is_pressed() {
            SERIAL.lock().write_str(&format!("[KBD] Key: '{}'\n", ch));
        }
    }

    unsafe { crate::arch::x86_64::apic::send_eoi(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_event_pressed() {
        let event = KeyEvent::Pressed(0x1E);
        assert!(event.is_pressed());
        assert_eq!(event.scancode(), 0x1E);
    }

    #[test]
    fn test_key_event_released() {
        let event = KeyEvent::Released(0x1E);
        assert!(!event.is_pressed());
    }

    #[test]
    fn test_scancode_to_ascii_a() {
        // 'A' key scancode is 0x1E
        assert_eq!(scancode_to_ascii(0x1E), Some('a'));
    }

    #[test]
    fn test_scancode_to_ascii_enter() {
        // Enter scancode is 0x1C
        assert_eq!(scancode_to_ascii(0x1C), Some('\n'));
    }

    #[test]
    fn test_scancode_to_ascii_invalid() {
        assert_eq!(scancode_to_ascii(0xFF), None);
    }

    #[test]
    fn test_keyboard_buffer_push_pop() {
        let mut buf = KeyboardBuffer::new();
        assert!(buf.is_empty());
        assert!(buf.push(KeyEvent::Pressed(0x01)));
        assert!(!buf.is_empty());
        let event = buf.pop().unwrap();
        assert_eq!(event, KeyEvent::Pressed(0x01));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_keyboard_buffer_overflow() {
        let mut buf = KeyboardBuffer::new();
        // Ring buffer can hold KEYBOARD_BUFFER_SIZE - 1 items
        for i in 0..KEYBOARD_BUFFER_SIZE - 1 {
            assert!(buf.push(KeyEvent::Pressed(i as u8)));
        }
        assert!(!buf.push(KeyEvent::Pressed(0xFF))); // Full
        assert_eq!(buf.len(), KEYBOARD_BUFFER_SIZE - 1);
    }

    #[test]
    fn test_keyboard_buffer_fifo() {
        let mut buf = KeyboardBuffer::new();
        buf.push(KeyEvent::Pressed(1));
        buf.push(KeyEvent::Pressed(2));
        buf.push(KeyEvent::Pressed(3));
        assert_eq!(buf.pop().unwrap().scancode(), 1);
        assert_eq!(buf.pop().unwrap().scancode(), 2);
        assert_eq!(buf.pop().unwrap().scancode(), 3);
    }

    #[test]
    fn test_keyboard_ports() {
        assert_eq!(KEYBOARD_DATA_PORT, 0x60);
        assert_eq!(KEYBOARD_STATUS_PORT, 0x64);
    }
}
