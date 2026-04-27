//! 矩形区域类型

/// 矩形区域
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// 左上角 x 坐标
    pub x: i32,
    /// 左上角 y 坐标
    pub y: i32,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
}

impl Rect {
    /// 创建新的矩形区域
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// 判断点 (x, y) 是否在矩形内
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    /// 判断两个矩形是否相交
    pub fn intersects(&self, other: &Rect) -> bool {
        let self_right = self.x + self.width as i32;
        let self_bottom = self.y + self.height as i32;
        let other_right = other.x + other.width as i32;
        let other_bottom = other.y + other.height as i32;

        self.x < other_right && self_right > other.x && self.y < other_bottom && self_bottom > other.y
    }

    /// 计算两个矩形的并集（最小包围矩形）
    pub fn union(&self, other: &Rect) -> Rect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let self_right = self.x + self.width as i32;
        let self_bottom = self.y + self.height as i32;
        let other_right = other.x + other.width as i32;
        let other_bottom = other.y + other.height as i32;
        let x2 = self_right.max(other_right);
        let y2 = self_bottom.max(other_bottom);

        Rect {
            x: x1,
            y: y1,
            width: (x2 - x1) as u32,
            height: (y2 - y1) as u32,
        }
    }

    /// 判断矩形是否为空（宽度或高度为 0）
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// 计算矩形面积
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let r = Rect::new(10, 20, 100, 200);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 200);
    }

    #[test]
    fn test_contains_inside() {
        let r = Rect::new(10, 10, 100, 100);
        assert!(r.contains(10, 10));
        assert!(r.contains(50, 50));
        assert!(r.contains(109, 109));
    }

    #[test]
    fn test_contains_outside() {
        let r = Rect::new(10, 10, 100, 100);
        assert!(!r.contains(9, 10));
        assert!(!r.contains(10, 9));
        assert!(!r.contains(110, 10));
        assert!(!r.contains(10, 110));
        assert!(!r.contains(0, 0));
    }

    #[test]
    fn test_contains_edge() {
        let r = Rect::new(0, 0, 10, 10);
        // 右边界和下边界不包含
        assert!(!r.contains(10, 5));
        assert!(!r.contains(5, 10));
    }

    #[test]
    fn test_intersects_overlap() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        assert!(a.intersects(&b));
    }

    #[test]
    fn test_intersects_no_overlap() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(100, 0, 100, 100);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn test_intersects_contained() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(10, 10, 10, 10);
        assert!(a.intersects(&b));
    }

    #[test]
    fn test_union() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        let u = a.union(&b);
        assert_eq!(u.x, 0);
        assert_eq!(u.y, 0);
        assert_eq!(u.width, 150);
        assert_eq!(u.height, 150);
    }

    #[test]
    fn test_union_negative_coords() {
        let a = Rect::new(-10, -10, 20, 20);
        let b = Rect::new(0, 0, 20, 20);
        let u = a.union(&b);
        assert_eq!(u.x, -10);
        assert_eq!(u.y, -10);
        assert_eq!(u.width, 30);
        assert_eq!(u.height, 30);
    }

    #[test]
    fn test_is_empty() {
        assert!(Rect::new(0, 0, 0, 100).is_empty());
        assert!(Rect::new(0, 0, 100, 0).is_empty());
        assert!(!Rect::new(0, 0, 1, 1).is_empty());
    }

    #[test]
    fn test_area() {
        let r = Rect::new(0, 0, 10, 20);
        assert_eq!(r.area(), 200);
    }

    #[test]
    fn test_area_empty() {
        let r = Rect::new(0, 0, 0, 100);
        assert_eq!(r.area(), 0);
    }
}
