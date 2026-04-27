//! 颜色类型（ARGB 格式）

/// 颜色（ARGB 格式）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Color {
    /// 透明度通道 (0-255)
    pub a: u8,
    /// 红色通道 (0-255)
    pub r: u8,
    /// 绿色通道 (0-255)
    pub g: u8,
    /// 蓝色通道 (0-255)
    pub b: u8,
}

impl Color {
    /// 完全透明
    pub const TRANSPARENT: Color = Color { a: 0, r: 0, g: 0, b: 0 };
    /// 白色（完全不透明）
    pub const WHITE: Color = Color { a: 255, r: 255, g: 255, b: 255 };
    /// 黑色（完全不透明）
    pub const BLACK: Color = Color { a: 255, r: 0, g: 0, b: 0 };

    /// 从 RGBA 分量创建颜色
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { a, r, g, b }
    }

    /// 从 RGB 分量创建颜色（完全不透明）
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { a: 255, r, g, b }
    }

    /// 从 u32（ARGB 格式）创建颜色
    /// 高位字节为 alpha，依次为 R、G、B
    pub fn from_u32(argb: u32) -> Self {
        Color {
            a: ((argb >> 24) & 0xFF) as u8,
            r: ((argb >> 16) & 0xFF) as u8,
            g: ((argb >> 8) & 0xFF) as u8,
            b: (argb & 0xFF) as u8,
        }
    }

    /// 将颜色转换为 u32（ARGB 格式）
    pub fn to_u32(&self) -> u32 {
        ((self.a as u32) << 24)
            | ((self.r as u32) << 16)
            | ((self.g as u32) << 8)
            | (self.b as u32)
    }

    /// 线性插值两个颜色
    /// t 为 0.0 时返回 self，t 为 1.0 时返回 other
    pub fn lerp(&self, other: &Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            a: (self.a as f32 + (other.a as f32 - self.a as f32) * t) as u8,
            r: (self.r as f32 + (other.r as f32 - self.r as f32) * t) as u8,
            g: (self.g as f32 + (other.g as f32 - self.g as f32) * t) as u8,
            b: (self.b as f32 + (other.b as f32 - self.b as f32) * t) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba() {
        let c = Color::rgba(10, 20, 30, 200);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
        assert_eq!(c.a, 200);
    }

    #[test]
    fn test_rgb() {
        let c = Color::rgb(100, 150, 200);
        assert_eq!(c.r, 100);
        assert_eq!(c.g, 150);
        assert_eq!(c.b, 200);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn test_from_u32() {
        // ARGB: 0xFF804020 -> A=0xFF, R=0x80, G=0x40, B=0x20
        let c = Color::from_u32(0xFF804020);
        assert_eq!(c.a, 0xFF);
        assert_eq!(c.r, 0x80);
        assert_eq!(c.g, 0x40);
        assert_eq!(c.b, 0x20);
    }

    #[test]
    fn test_to_u32() {
        let c = Color::rgba(0x80, 0x40, 0x20, 0xFF);
        assert_eq!(c.to_u32(), 0xFF804020);
    }

    #[test]
    fn test_from_u32_to_u32_roundtrip() {
        let original = 0xAB123456;
        let c = Color::from_u32(original);
        assert_eq!(c.to_u32(), original);
    }

    #[test]
    fn test_lerp_t0() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 200, 255);
        let result = a.lerp(&b, 0.0);
        assert_eq!(result, a);
    }

    #[test]
    fn test_lerp_t1() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 200, 255);
        let result = a.lerp(&b, 1.0);
        assert_eq!(result, b);
    }

    #[test]
    fn test_lerp_mid() {
        let a = Color::rgba(0, 0, 0, 0);
        let b = Color::rgba(100, 200, 255, 200);
        let result = a.lerp(&b, 0.5);
        assert_eq!(result.a, 100);
        assert_eq!(result.r, 50);
        assert_eq!(result.g, 100);
        assert_eq!(result.b, 127); // 255/2 = 127.5 截断为 127
    }

    #[test]
    fn test_lerp_clamp() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 100, 100);
        // t 超出范围应被钳制
        let result = a.lerp(&b, 2.0);
        assert_eq!(result, b);
        let result = a.lerp(&b, -1.0);
        assert_eq!(result, a);
    }

    #[test]
    fn test_constants() {
        assert_eq!(Color::TRANSPARENT, Color::rgba(0, 0, 0, 0));
        assert_eq!(Color::WHITE, Color::rgba(255, 255, 255, 255));
        assert_eq!(Color::BLACK, Color::rgba(0, 0, 0, 255));
    }
}
