//! 动画系统模块
//!
//! 包含缓动函数、动画属性、动画定义和动画管理器。

use std::collections::HashMap;

// ============================================================================
// EasingFunction - 缓动函数
// ============================================================================

/// 缓动函数类型
///
/// 用于定义动画值随时间变化的速率曲线。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    /// 线性插值
    Linear,
    /// 二次缓入
    EaseInQuad,
    /// 二次缓出
    EaseOutQuad,
    /// 二次缓入缓出
    EaseInOutQuad,
    /// 三次缓入
    EaseInCubic,
    /// 三次缓出
    EaseOutCubic,
    /// 三次缓入缓出
    EaseInOutCubic,
    /// 回弹缓入
    EaseInBack,
    /// 回弹缓出
    EaseOutBack,
    /// 弹性缓出
    EaseOutElastic,
    /// 弹簧物理动画
    Spring {
        /// 弹簧刚度
        stiffness: f32,
        /// 弹簧阻尼
        damping: f32,
    },
}

impl EasingFunction {
    /// 计算缓动值
    ///
    /// 将线性进度 `t`（0.0 ~ 1.0）映射为缓动后的值（通常 0.0 ~ 1.0，部分缓动函数可能超出此范围）。
    ///
    /// # 参数
    /// - `t`: 线性进度值，范围 [0.0, 1.0]
    ///
    /// # 返回
    /// 缓动后的值
    pub fn evaluate(&self, t: f32) -> f32 {
        // 将 t 限制在 [0.0, 1.0] 范围内
        let t = t.clamp(0.0, 1.0);

        match self {
            Self::Linear => t,

            Self::EaseInQuad => t * t,

            Self::EaseOutQuad => t * (2.0 - t),

            Self::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }

            Self::EaseInCubic => t * t * t,

            Self::EaseOutCubic => {
                let t1 = t - 1.0;
                t1 * t1 * t1 + 1.0
            }

            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let t1 = 2.0 * t - 2.0;
                    0.5 * t1 * t1 * t1 + 1.0
                }
            }

            Self::EaseInBack => {
                let s = 1.70158;
                t * t * ((s + 1.0) * t - s)
            }

            Self::EaseOutBack => {
                let s = 1.70158;
                let t1 = t - 1.0;
                t1 * t1 * ((s + 1.0) * t1 + s) + 1.0
            }

            Self::EaseOutElastic => {
                if t == 0.0 {
                    return 0.0;
                }
                if t == 1.0 {
                    return 1.0;
                }
                let p = 0.3;
                let s = p / 4.0;
                let t1 = t - 1.0;
                (2.0_f32.powf(-10.0 * t1) * ((t1 - s) * (2.0 * std::f32::consts::PI) / p).sin()) + 1.0
            }

            Self::Spring { stiffness, damping } => {
                // 简化的弹簧模型
                // 使用指数衰减 + 正弦振荡
                let omega = stiffness.sqrt();
                let zeta = damping / (2.0 * omega);
                let t_end = 1.0; // 归一化时间

                if zeta < 1.0 {
                    // 欠阻尼：有振荡
                    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
                    let exp_term = (-zeta * omega * t * t_end).exp();
                    let cos_term = (omega_d * t * t_end).cos();
                    let sin_term = (omega_d * t * t_end).sin();
                    1.0 - exp_term * (cos_term + (zeta * omega / omega_d) * sin_term)
                } else {
                    // 过阻尼或临界阻尼：无振荡
                    let exp_term = (-omega * t * t_end).exp();
                    1.0 - exp_term
                }
            }
        }
    }
}

// ============================================================================
// AnimatableProperty - 动画属性
// ============================================================================

/// 可动画化的属性类型
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatableProperty {
    /// 不透明度 (0.0 ~ 1.0)
    Opacity(f32),
    /// X 坐标位置
    PositionX(f32),
    /// Y 坐标位置
    PositionY(f32),
    /// 宽度
    Width(f32),
    /// 高度
    Height(f32),
    /// 缩放比例
    Scale(f32),
    /// 旋转角度（弧度）
    Rotation(f32),
    /// 颜色 (RGBA)
    Color([f32; 4]),
    /// 圆角半径
    BorderRadius(f32),
}

// ============================================================================
// AnimationDirection - 动画方向
// ============================================================================

/// 动画播放方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationDirection {
    /// 正常方向（从起始值到目标值）
    Normal = 0,
    /// 反向（从目标值到起始值）
    Reverse = 1,
    /// 交替（正反交替播放）
    Alternate = 2,
}

// ============================================================================
// Animation - 动画
// ============================================================================

/// 动画定义
///
/// 描述一个属性从起始值到目标值的过渡动画。
#[derive(Debug, Clone)]
pub struct Animation {
    /// 动画属性
    pub property: AnimatableProperty,
    /// 起始值
    pub from: f32,
    /// 目标值
    pub to: f32,
    /// 动画持续时间（毫秒）
    pub duration_ms: u32,
    /// 延迟时间（毫秒）
    pub delay_ms: u32,
    /// 缓动函数
    pub easing: EasingFunction,
    /// 迭代次数（0 = 无限循环）
    pub iteration_count: u32,
    /// 动画方向
    pub direction: AnimationDirection,
}

impl Animation {
    /// 创建新的动画
    ///
    /// # 参数
    /// - `property`: 要动画化的属性
    /// - `to`: 目标值
    /// - `duration_ms`: 动画持续时间（毫秒）
    pub fn new(property: AnimatableProperty, to: f32, duration_ms: u32) -> Self {
        // 从属性中提取起始值
        let from = match &property {
            AnimatableProperty::Opacity(v) => *v,
            AnimatableProperty::PositionX(v) => *v,
            AnimatableProperty::PositionY(v) => *v,
            AnimatableProperty::Width(v) => *v,
            AnimatableProperty::Height(v) => *v,
            AnimatableProperty::Scale(v) => *v,
            AnimatableProperty::Rotation(v) => *v,
            AnimatableProperty::Color(v) => v[0], // 颜色取第一个分量作为起始
            AnimatableProperty::BorderRadius(v) => *v,
        };

        Self {
            property,
            from,
            to,
            duration_ms,
            delay_ms: 0,
            easing: EasingFunction::Linear,
            iteration_count: 1,
            direction: AnimationDirection::Normal,
        }
    }

    /// 设置缓动函数（构建器模式）
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// 设置延迟时间（构建器模式）
    pub fn with_delay(mut self, delay_ms: u32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// 设置迭代次数（构建器模式）
    pub fn with_iteration_count(mut self, count: u32) -> Self {
        self.iteration_count = count;
        self
    }

    /// 设置动画方向（构建器模式）
    pub fn with_direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    /// 计算当前动画值
    ///
    /// # 参数
    /// - `elapsed_ms`: 从动画开始到当前的已过时间（毫秒）
    ///
    /// # 返回
    /// 当前动画值，如果动画尚未开始则返回 None
    pub fn evaluate(&self, elapsed_ms: u32) -> Option<f32> {
        // 检查是否在延迟期内
        if elapsed_ms < self.delay_ms {
            return Some(self.from);
        }

        let active_time = elapsed_ms - self.delay_ms;

        // 计算当前迭代
        let total_duration = self.duration_ms as u64;
        if total_duration == 0 {
            return Some(self.to);
        }

        let current_iteration = (active_time as u64) / total_duration;
        let iteration_time = (active_time as u64) % total_duration;

        // 检查是否已完成所有迭代
        if self.iteration_count > 0 && current_iteration >= self.iteration_count as u64 {
            // 返回最终值
            return Some(match self.direction {
                AnimationDirection::Normal => self.to,
                AnimationDirection::Reverse => self.from,
                AnimationDirection::Alternate => {
                    if self.iteration_count % 2 == 0 {
                        self.from
                    } else {
                        self.to
                    }
                }
            });
        }

        // 计算线性进度
        let t = iteration_time as f32 / self.duration_ms as f32;

        // 应用缓动
        let eased_t = self.easing.evaluate(t);

        // 根据方向计算实际值
        let value = match self.direction {
            AnimationDirection::Normal => self.from + (self.to - self.from) * eased_t,
            AnimationDirection::Reverse => self.to + (self.from - self.to) * eased_t,
            AnimationDirection::Alternate => {
                if current_iteration % 2 == 0 {
                    self.from + (self.to - self.from) * eased_t
                } else {
                    self.to + (self.from - self.to) * eased_t
                }
            }
        };

        Some(value)
    }

    /// 动画是否已完成
    ///
    /// # 参数
    /// - `elapsed_ms`: 从动画开始到当前的已过时间（毫秒）
    pub fn is_complete(&self, elapsed_ms: u32) -> bool {
        if self.iteration_count == 0 {
            // 无限循环，永不完成
            return false;
        }

        if elapsed_ms < self.delay_ms {
            return false;
        }

        let active_time = elapsed_ms - self.delay_ms;
        let total_duration = self.duration_ms as u64;
        if total_duration == 0 {
            return true;
        }

        let current_iteration = (active_time as u64) / total_duration;
        current_iteration >= self.iteration_count as u64
    }
}

// ============================================================================
// AnimationManager - 动画管理器
// ============================================================================

/// 动画管理器
///
/// 管理多个动画的生命周期，包括添加、移除和求值。
pub struct AnimationManager {
    /// 动画集合，键为动画 ID
    animations: HashMap<String, Animation>,
    /// 各动画的开始时间
    start_times: HashMap<String, u64>,
}

impl AnimationManager {
    /// 创建新的动画管理器
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            start_times: HashMap::new(),
        }
    }

    /// 添加动画
    ///
    /// # 参数
    /// - `id`: 动画唯一标识
    /// - `animation`: 动画定义
    /// - `start_time`: 动画开始时间（毫秒）
    pub fn add(&mut self, id: &str, animation: Animation, start_time: u64) {
        self.animations.insert(id.to_string(), animation);
        self.start_times.insert(id.to_string(), start_time);
    }

    /// 移除动画
    ///
    /// # 参数
    /// - `id`: 要移除的动画 ID
    pub fn remove(&mut self, id: &str) {
        self.animations.remove(id);
        self.start_times.remove(id);
    }

    /// 评估动画当前值
    ///
    /// # 参数
    /// - `id`: 动画 ID
    /// - `current_time`: 当前时间（毫秒）
    ///
    /// # 返回
    /// 动画当前值，如果动画不存在或尚未开始则返回 None
    pub fn evaluate(&self, id: &str, current_time: u64) -> Option<f32> {
        let animation = self.animations.get(id)?;
        let start_time = *self.start_times.get(id)?;

        if current_time < start_time {
            return None;
        }

        let elapsed = (current_time - start_time) as u32;
        animation.evaluate(elapsed)
    }

    /// 检查动画是否已完成
    ///
    /// # 参数
    /// - `id`: 动画 ID
    /// - `current_time`: 当前时间（毫秒）
    pub fn is_complete(&self, id: &str, current_time: u64) -> bool {
        if let (Some(animation), Some(&start_time)) =
            (self.animations.get(id), self.start_times.get(id))
        {
            if current_time < start_time {
                return false;
            }
            let elapsed = (current_time - start_time) as u32;
            animation.is_complete(elapsed)
        } else {
            // 动画不存在视为已完成
            true
        }
    }

    /// 获取当前活跃的动画数量
    pub fn active_count(&self) -> usize {
        self.animations.len()
    }

    /// 检查是否包含指定 ID 的动画
    pub fn contains(&self, id: &str) -> bool {
        self.animations.contains_key(id)
    }
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- EasingFunction 测试 ----

    #[test]
    fn test_easing_linear() {
        let e = EasingFunction::Linear;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_in_quad() {
        let e = EasingFunction::EaseInQuad;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        // t=0.5 -> 0.25
        assert!((e.evaluate(0.5) - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_out_quad() {
        let e = EasingFunction::EaseOutQuad;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        // t=0.5 -> 0.75
        assert!((e.evaluate(0.5) - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_in_out_quad() {
        let e = EasingFunction::EaseInOutQuad;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((e.evaluate(0.5) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_in_cubic() {
        let e = EasingFunction::EaseInCubic;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        // t=0.5 -> 0.125
        assert!((e.evaluate(0.5) - 0.125).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_out_cubic() {
        let e = EasingFunction::EaseOutCubic;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_in_out_cubic() {
        let e = EasingFunction::EaseInOutCubic;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((e.evaluate(0.5) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_ease_in_back() {
        let e = EasingFunction::EaseInBack;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        // EaseInBack 在开始时会低于 0
        assert!(e.evaluate(0.3) < 0.0);
    }

    #[test]
    fn test_easing_ease_out_back() {
        let e = EasingFunction::EaseOutBack;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
        // EaseOutBack 在结束时会超过 1
        assert!(e.evaluate(0.8) > 1.0);
    }

    #[test]
    fn test_easing_ease_out_elastic() {
        let e = EasingFunction::EaseOutElastic;
        assert!((e.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_easing_spring() {
        let e = EasingFunction::Spring {
            stiffness: 100.0,
            damping: 10.0,
        };
        assert!((e.evaluate(0.0) - 0.0).abs() < 0.01);
        // 弹簧在 t=1.0 时应接近 1.0
        let val = e.evaluate(1.0);
        assert!((val - 1.0).abs() < 0.1, "弹簧在 t=1.0 时应接近 1.0，实际: {}", val);
    }

    #[test]
    fn test_easing_clamp() {
        let e = EasingFunction::Linear;
        // 超出范围的值应被限制
        assert!((e.evaluate(-0.5) - 0.0).abs() < f32::EPSILON);
        assert!((e.evaluate(1.5) - 1.0).abs() < f32::EPSILON);
    }

    // ---- Animation 测试 ----

    #[test]
    fn test_animation_new() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        assert_eq!(anim.from, 0.0);
        assert_eq!(anim.to, 1.0);
        assert_eq!(anim.duration_ms, 1000);
        assert_eq!(anim.delay_ms, 0);
        assert_eq!(anim.easing, EasingFunction::Linear);
        assert_eq!(anim.iteration_count, 1);
        assert_eq!(anim.direction, AnimationDirection::Normal);
    }

    #[test]
    fn test_animation_with_easing() {
        let anim = Animation::new(AnimatableProperty::Scale(1.0), 2.0, 500)
            .with_easing(EasingFunction::EaseOutCubic);
        assert_eq!(anim.easing, EasingFunction::EaseOutCubic);
    }

    #[test]
    fn test_animation_with_delay() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000)
            .with_delay(200);
        assert_eq!(anim.delay_ms, 200);
    }

    #[test]
    fn test_animation_evaluate_start() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        // 动画刚开始
        let val = anim.evaluate(0).unwrap();
        assert!((val - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_evaluate_end() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        // 动画结束
        let val = anim.evaluate(1000).unwrap();
        assert!((val - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_evaluate_mid() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        // 动画中间（线性）
        let val = anim.evaluate(500).unwrap();
        assert!((val - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_evaluate_during_delay() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000)
            .with_delay(500);
        // 在延迟期内，应返回起始值
        let val = anim.evaluate(200).unwrap();
        assert!((val - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_evaluate_after_delay() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000)
            .with_delay(500);
        // 延迟结束后，动画进行到一半
        let val = anim.evaluate(1000).unwrap();
        assert!((val - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_is_complete_not_started() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        assert!(!anim.is_complete(0));
    }

    #[test]
    fn test_animation_is_complete_in_progress() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        assert!(!anim.is_complete(500));
    }

    #[test]
    fn test_animation_is_complete_done() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        assert!(anim.is_complete(1000));
        assert!(anim.is_complete(2000));
    }

    #[test]
    fn test_animation_infinite_loop() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000)
            .with_iteration_count(0);
        assert!(!anim.is_complete(10000));
    }

    #[test]
    fn test_animation_reverse() {
        let anim = Animation::new(AnimatableProperty::Opacity(1.0), 0.0, 1000)
            .with_direction(AnimationDirection::Reverse);
        let val_start = anim.evaluate(0).unwrap();
        let val_end = anim.evaluate(1000).unwrap();
        assert!((val_start - 0.0).abs() < f32::EPSILON);
        assert!((val_end - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_alternate() {
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000)
            .with_direction(AnimationDirection::Alternate)
            .with_iteration_count(2);
        // 第一次迭代：0 -> 1
        let val_mid1 = anim.evaluate(500).unwrap();
        assert!((val_mid1 - 0.5).abs() < f32::EPSILON);
        // 第二次迭代：1 -> 0
        let val_mid2 = anim.evaluate(1500).unwrap();
        assert!((val_mid2 - 0.5).abs() < f32::EPSILON);
    }

    // ---- AnimationManager 测试 ----

    #[test]
    fn test_animation_manager_new() {
        let mgr = AnimationManager::new();
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_animation_manager_add() {
        let mut mgr = AnimationManager::new();
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        mgr.add("fade_in", anim, 0);
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.contains("fade_in"));
    }

    #[test]
    fn test_animation_manager_remove() {
        let mut mgr = AnimationManager::new();
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        mgr.add("fade_in", anim, 0);
        assert_eq!(mgr.active_count(), 1);
        mgr.remove("fade_in");
        assert_eq!(mgr.active_count(), 0);
        assert!(!mgr.contains("fade_in"));
    }

    #[test]
    fn test_animation_manager_evaluate() {
        let mut mgr = AnimationManager::new();
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        mgr.add("fade_in", anim, 100);

        // 动画尚未开始
        assert!(mgr.evaluate("fade_in", 50).is_none());

        // 动画刚开始
        let val = mgr.evaluate("fade_in", 100).unwrap();
        assert!((val - 0.0).abs() < f32::EPSILON);

        // 动画中间
        let val = mgr.evaluate("fade_in", 600).unwrap();
        assert!((val - 0.5).abs() < f32::EPSILON);

        // 动画结束
        let val = mgr.evaluate("fade_in", 1100).unwrap();
        assert!((val - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_manager_is_complete() {
        let mut mgr = AnimationManager::new();
        let anim = Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000);
        mgr.add("fade_in", anim, 0);

        assert!(!mgr.is_complete("fade_in", 500));
        assert!(mgr.is_complete("fade_in", 1000));
        assert!(mgr.is_complete("fade_in", 2000));
    }

    #[test]
    fn test_animation_manager_nonexistent() {
        let mgr = AnimationManager::new();
        assert!(mgr.evaluate("nonexistent", 1000).is_none());
        assert!(mgr.is_complete("nonexistent", 1000));
    }

    #[test]
    fn test_animation_manager_multiple() {
        let mut mgr = AnimationManager::new();
        mgr.add(
            "anim1",
            Animation::new(AnimatableProperty::Opacity(0.0), 1.0, 1000),
            0,
        );
        mgr.add(
            "anim2",
            Animation::new(AnimatableProperty::Scale(1.0), 2.0, 500),
            200,
        );
        assert_eq!(mgr.active_count(), 2);

        // anim1 在 t=500 时进行到一半
        let val1 = mgr.evaluate("anim1", 500).unwrap();
        assert!((val1 - 0.5).abs() < f32::EPSILON);

        // anim2 在 t=200 时刚开始
        let val2 = mgr.evaluate("anim2", 200).unwrap();
        assert!((val2 - 1.0).abs() < f32::EPSILON);

        // anim2 在 t=450 时进行到一半 (200 + 250 = 450, 250/500 = 0.5)
        let val2 = mgr.evaluate("anim2", 450).unwrap();
        assert!((val2 - 1.5).abs() < f32::EPSILON);
    }

    // ---- AnimatableProperty 测试 ----

    #[test]
    fn test_animatable_property_equality() {
        assert_eq!(
            AnimatableProperty::Opacity(0.5),
            AnimatableProperty::Opacity(0.5)
        );
        assert_ne!(
            AnimatableProperty::Opacity(0.5),
            AnimatableProperty::Opacity(0.8)
        );
        assert_eq!(
            AnimatableProperty::Color([1.0, 0.0, 0.0, 1.0]),
            AnimatableProperty::Color([1.0, 0.0, 0.0, 1.0])
        );
    }
}
