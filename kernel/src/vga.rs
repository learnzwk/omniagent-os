//! VGA 文本模式驱动
//!
//! 80x25 文本模式，16 色，支持 print!/println! 宏

use core::fmt;
use spin::{lazy::Lazy, Mutex};

/// VGA 文本缓冲区尺寸
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

/// VGA 颜色
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VgaColor {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// VGA 字符单元 (2 字节: 前景+背景色 | ASCII 字符)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct VgaChar {
    pub character: u8,
    pub color_code: u8,
}

/// VGA 文本缓冲区写入器
pub struct VgaWriter {
    column_position: usize,
    color_code: u8,
    buffer: &'static mut [[VgaChar; VGA_WIDTH]; VGA_HEIGHT],
}

impl VgaWriter {
    /// 创建新的 VGA 写入器
    pub fn new() -> Self {
        Self {
            column_position: 0,
            color_code: (VgaColor::LightGray as u8) | ((VgaColor::Black as u8) << 4),
            buffer: unsafe { &mut *(0xb8000 as *mut [[VgaChar; VGA_WIDTH]; VGA_HEIGHT]) },
        }
    }

    /// 设置颜色
    pub fn set_color(&mut self, foreground: VgaColor, background: VgaColor) {
        self.color_code = (foreground as u8) | ((background as u8) << 4);
    }

    /// 写入单个字节
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\r' => self.column_position = 0,
            byte => {
                if self.column_position >= VGA_WIDTH {
                    self.new_line();
                }
                let row = VGA_HEIGHT - 1;
                let col = self.column_position;
                let color_code = self.color_code;
                self.buffer[row][col] = VgaChar {
                    character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    /// 写入字符串
    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    /// 换行
    fn new_line(&mut self) {
        // 简化实现：只移动光标到底部行
        self.column_position = 0;
    }

    /// 清屏
    pub fn clear(&mut self) {
        let blank = VgaChar {
            character: b' ',
            color_code: self.color_code,
        };
        for row in &mut self.buffer.iter_mut() {
            for col in row.iter_mut() {
                *col = blank;
            }
        }
        self.column_position = 0;
    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

/// 全局 VGA 写入器
pub static VGA_WRITER: Lazy<Mutex<VgaWriter>> = Lazy::new(|| Mutex::new(VgaWriter::new()));

/// 打印宏 (类似 std::print!)
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::vga::_print(format_args!($($arg)*));
    });
}

/// 打印换行宏 (类似 std::println!)
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// 内部打印函数
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    VGA_WRITER.lock().write_fmt(args).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vga_color_values() {
        assert_eq!(VgaColor::Black as u8, 0);
        assert_eq!(VgaColor::White as u8, 15);
    }

    #[test]
    fn test_vga_char_size() {
        assert_eq!(core::mem::size_of::<VgaChar>(), 2);
    }

    #[test]
    fn test_vga_dimensions() {
        assert_eq!(VGA_WIDTH, 80);
        assert_eq!(VGA_HEIGHT, 25);
    }

    #[test]
    fn test_vga_writer_color() {
        let mut writer = VgaWriter::new();
        writer.set_color(VgaColor::White, VgaColor::Blue);
        assert_eq!(writer.color_code, (15) | (1 << 4));
    }

    #[test]
    fn test_vga_char_creation() {
        let ch = VgaChar {
            character: b'A',
            color_code: 0x0F,
        };
        assert_eq!(ch.character, b'A');
        assert_eq!(ch.color_code, 0x0F);
    }
}
