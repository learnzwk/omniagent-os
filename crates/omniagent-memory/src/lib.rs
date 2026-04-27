//! # OmniAgent 量化内存服务
//!
//! 为 AI Agent 提供高效的内存管理和量化推理支持。
//! 包含量化类型、张量存储、内存区域和内存池等核心组件。

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

// ============================================================================
// 错误类型
// ============================================================================

/// 内存服务错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    /// 内存不足
    OutOfMemory {
        /// 请求的大小
        requested: usize,
        /// 可用的大小
        available: usize,
    },
    /// 无效的形状
    InvalidShape(String),
    /// 无效的量化类型
    InvalidQuantType(String),
    /// 量化错误
    QuantizationError(String),
    /// 内存区域未找到
    RegionNotFound(u64),
    /// 内存区域已满
    RegionFull {
        /// 区域 ID
        region_id: u64,
        /// 请求的大小
        requested: usize,
        /// 可用的大小
        available: usize,
    },
    /// 张量未找到
    TensorNotFound(String),
    /// 张量已存在
    TensorAlreadyExists(String),
    /// 无效的对齐
    InvalidAlignment(usize),
    /// 释放错误
    DeallocateError(String),
    /// 类型不兼容
    IncompatibleType(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::OutOfMemory { requested, available } => {
                write!(f, "内存不足: 请求 {} 字节, 可用 {} 字节", requested, available)
            }
            MemoryError::InvalidShape(msg) => write!(f, "无效的形状: {}", msg),
            MemoryError::InvalidQuantType(msg) => write!(f, "无效的量化类型: {}", msg),
            MemoryError::QuantizationError(msg) => write!(f, "量化错误: {}", msg),
            MemoryError::RegionNotFound(id) => write!(f, "内存区域未找到: {}", id),
            MemoryError::RegionFull { region_id, requested, available } => {
                write!(
                    f,
                    "内存区域 {} 已满: 请求 {} 字节, 可用 {} 字节",
                    region_id, requested, available
                )
            }
            MemoryError::TensorNotFound(name) => write!(f, "张量未找到: {}", name),
            MemoryError::TensorAlreadyExists(name) => write!(f, "张量已存在: {}", name),
            MemoryError::InvalidAlignment(align) => write!(f, "无效的对齐: {}", align),
            MemoryError::DeallocateError(msg) => write!(f, "释放错误: {}", msg),
            MemoryError::IncompatibleType(msg) => write!(f, "类型不兼容: {}", msg),
        }
    }
}

impl std::error::Error for MemoryError {}

// ============================================================================
// 量化类型
// ============================================================================

/// 量化数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum QuantType {
    /// 4-bit 整数 (0-15)
    Q4 = 0,
    /// 8-bit 整数 (-128 to 127)
    Q8 = 1,
    /// 16-bit 浮点 (半精度)
    F16 = 2,
    /// 布尔量化 (1-bit)
    B1 = 3,
    /// 非量化 (32-bit 浮点)
    F32 = 4,
    /// 8-bit 无符号 (0-255)
    U8 = 5,
    /// 混合精度
    Mixed = 6,
}

impl QuantType {
    /// 每个元素的字节数
    pub fn bytes_per_element(&self) -> usize {
        match self {
            QuantType::Q4 => 1, // 实际 0.5 字节，但存储时按字节对齐
            QuantType::Q8 => 1,
            QuantType::F16 => 2,
            QuantType::B1 => 1, // 实际 0.125 字节，但存储时按字节对齐
            QuantType::F32 => 4,
            QuantType::U8 => 1,
            QuantType::Mixed => 4, // 混合精度默认按最大类型计算
        }
    }

    /// 位数
    pub fn bits(&self) -> usize {
        match self {
            QuantType::Q4 => 4,
            QuantType::Q8 => 8,
            QuantType::F16 => 16,
            QuantType::B1 => 1,
            QuantType::F32 => 32,
            QuantType::U8 => 8,
            QuantType::Mixed => 32, // 混合精度默认按最大类型计算
        }
    }

    /// 是否为量化类型
    pub fn is_quantized(&self) -> bool {
        match self {
            QuantType::Q4 | QuantType::Q8 | QuantType::B1 | QuantType::U8 => true,
            QuantType::F16 | QuantType::F32 | QuantType::Mixed => false,
        }
    }
}

/// 量化参数
#[derive(Debug, Clone)]
pub struct QuantParams {
    /// 量化类型
    pub quant_type: QuantType,
    /// 缩放因子
    pub scale: f32,
    /// 零点
    pub zero_point: i32,
    /// 最小值
    pub min_val: f32,
    /// 最大值
    pub max_val: f32,
}

impl QuantParams {
    /// 从数据范围计算量化参数
    pub fn from_range(quant_type: QuantType, min: f32, max: f32) -> Self {
        // 确保范围有效
        let (min_val, max_val) = if (max - min).abs() < f32::EPSILON {
            (min - 1.0, min + 1.0)
        } else {
            (min, max)
        };

        // 根据量化类型确定量化范围
        let (qmin, qmax): (i32, i32) = match quant_type {
            QuantType::Q4 => (0, 15),
            QuantType::Q8 => (-128, 127),
            QuantType::U8 => (0, 255),
            QuantType::B1 => (0, 1),
            QuantType::F16 | QuantType::F32 | QuantType::Mixed => (0, 0),
        };

        // 计算缩放因子和零点
        let scale = if qmax == qmin {
            1.0
        } else {
            (max_val - min_val) / (qmax - qmin) as f32
        };

        let zero_point = if qmax == qmin {
            0
        } else {
            let zp = (qmin as f32 - min_val / scale).round() as i32;
            zp.clamp(qmin, qmax)
        };

        QuantParams {
            quant_type,
            scale,
            zero_point,
            min_val,
            max_val,
        }
    }

    /// 量化一个 f32 值
    pub fn quantize(&self, value: f32) -> i32 {
        let clamped = value.clamp(self.min_val, self.max_val);
        let q = (clamped / self.scale).round() as i32 + self.zero_point;

        // 根据量化类型限制范围
        match self.quant_type {
            QuantType::Q4 => q.clamp(0, 15),
            QuantType::Q8 => q.clamp(-128, 127),
            QuantType::U8 => q.clamp(0, 255),
            QuantType::B1 => if clamped >= (self.min_val + self.max_val) / 2.0 { 1 } else { 0 },
            QuantType::F16 | QuantType::F32 | QuantType::Mixed => {
                // 非量化类型，直接返回位模式
                clamped.to_bits() as i32
            }
        }
    }

    /// 反量化一个整数值
    pub fn dequantize(&self, value: i32) -> f32 {
        match self.quant_type {
            QuantType::Q4 | QuantType::Q8 | QuantType::U8 => {
                (value - self.zero_point) as f32 * self.scale
            }
            QuantType::B1 => {
                if value == 1 {
                    self.max_val
                } else {
                    self.min_val
                }
            }
            QuantType::F16 | QuantType::F32 | QuantType::Mixed => {
                f32::from_bits(value as u32)
            }
        }
    }

    /// 量化一组 f32 值
    pub fn quantize_batch(&self, values: &[f32]) -> Vec<i32> {
        values.iter().map(|&v| self.quantize(v)).collect()
    }

    /// 反量化一组整数值
    pub fn dequantize_batch(&self, values: &[i32]) -> Vec<f32> {
        values.iter().map(|&v| self.dequantize(v)).collect()
    }
}

// ============================================================================
// 张量形状
// ============================================================================

/// 张量形状
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorShape {
    /// 各维度大小
    pub dims: Vec<usize>,
}

impl TensorShape {
    /// 创建新的张量形状
    pub fn new(dims: Vec<usize>) -> Self {
        TensorShape { dims }
    }

    /// 形状的秩 (维度数)
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// 元素总数
    pub fn num_elements(&self) -> usize {
        self.dims.iter().product()
    }

    /// 是否为标量 (0 维)
    pub fn is_scalar(&self) -> bool {
        self.dims.is_empty()
    }

    /// 是否为向量 (1 维)
    pub fn is_vector(&self) -> bool {
        self.dims.len() == 1
    }

    /// 是否为矩阵 (2 维)
    pub fn is_matrix(&self) -> bool {
        self.dims.len() == 2
    }
}

// ============================================================================
// 张量
// ============================================================================

/// 张量 - 支持量化和非量化数据存储
#[derive(Clone)]
pub struct Tensor {
    /// 数据 (原始字节)
    data: Vec<u8>,
    /// 形状
    shape: TensorShape,
    /// 数据类型
    dtype: QuantType,
    /// 量化参数
    quant_params: Option<QuantParams>,
    /// 张量名称
    name: Option<String>,
}

impl Tensor {
    /// 创建新的 f32 张量
    pub fn new_f32(shape: TensorShape, data: Vec<f32>) -> Self {
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for val in &data {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        Tensor {
            data: bytes,
            shape,
            dtype: QuantType::F32,
            quant_params: None,
            name: None,
        }
    }

    /// 创建量化张量
    pub fn new_quantized(
        shape: TensorShape,
        dtype: QuantType,
        data: Vec<u8>,
        params: QuantParams,
    ) -> Self {
        Tensor {
            data,
            shape,
            dtype,
            quant_params: Some(params),
            name: None,
        }
    }

    /// 量化 f32 张量
    pub fn quantize(&self, target_type: QuantType) -> Result<Tensor, MemoryError> {
        if self.dtype != QuantType::F32 {
            return Err(MemoryError::IncompatibleType(format!(
                "只能量化 F32 张量, 当前类型: {:?}",
                self.dtype
            )));
        }

        // 获取 f32 数据
        let f32_data = self.as_f32()?;

        // 计算数据范围
        let min_val = f32_data
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let max_val = f32_data
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);

        // 创建量化参数
        let params = QuantParams::from_range(target_type, min_val, max_val);

        // 执行量化
        let quantized = params.quantize_batch(&f32_data);

        // 将量化值转换为字节
        let mut bytes = Vec::with_capacity(quantized.len() * target_type.bytes_per_element());
        for &val in &quantized {
            match target_type {
                QuantType::Q4 => {
                    // Q4: 每个 i32 值存储为单字节
                    bytes.push(val.clamp(0, 15) as u8);
                }
                QuantType::Q8 => {
                    bytes.push(val.clamp(-128, 127) as u8);
                }
                QuantType::U8 => {
                    bytes.push(val.clamp(0, 255) as u8);
                }
                QuantType::B1 => {
                    bytes.push(if val != 0 { 1 } else { 0 });
                }
                QuantType::F16 => {
                    // F16: 将 f32 转为半精度 (简化实现，截断为 2 字节)
                    let half = f32_to_f16(val as f32);
                    bytes.extend_from_slice(&half.to_le_bytes());
                }
                QuantType::F32 => {
                    bytes.extend_from_slice(&(val as f32).to_le_bytes());
                }
                QuantType::Mixed => {
                    bytes.extend_from_slice(&(val as f32).to_le_bytes());
                }
            }
        }

        Ok(Tensor::new_quantized(
            self.shape.clone(),
            target_type,
            bytes,
            params,
        ))
    }

    /// 反量化为 f32
    pub fn dequantize(&self) -> Result<Tensor, MemoryError> {
        if self.dtype == QuantType::F32 {
            return Ok(self.clone());
        }

        let params = self.quant_params.as_ref().ok_or_else(|| {
            MemoryError::QuantizationError("缺少量化参数, 无法反量化".to_string())
        })?;

        // 获取量化值
        let quant_values = self.read_quant_values()?;

        // 反量化
        let f32_data: Vec<f32> = quant_values.iter().map(|&v| params.dequantize(v)).collect();

        Ok(Tensor::new_f32(self.shape.clone(), f32_data))
    }

    /// 从原始字节读取量化整数值
    fn read_quant_values(&self) -> Result<Vec<i32>, MemoryError> {
        let n = self.shape.num_elements();
        let mut values = Vec::with_capacity(n);

        match self.dtype {
            QuantType::Q4 => {
                // Q4: 每个字节存储一个 4-bit 值
                for i in 0..n {
                    if i < self.data.len() {
                        values.push(self.data[i] as i32);
                    }
                }
            }
            QuantType::Q8 => {
                // Q8: 每个字节存储一个有符号 8-bit 值
                for i in 0..n {
                    if i < self.data.len() {
                        values.push(self.data[i] as i8 as i32);
                    }
                }
            }
            QuantType::U8 => {
                // U8: 每个字节存储一个无符号 8-bit 值
                for i in 0..n {
                    if i < self.data.len() {
                        values.push(self.data[i] as i32);
                    }
                }
            }
            QuantType::B1 => {
                // B1: 每个字节存储一个布尔值
                for i in 0..n {
                    if i < self.data.len() {
                        values.push(self.data[i] as i32);
                    }
                }
            }
            QuantType::F16 => {
                // F16: 每 2 字节存储一个半精度浮点
                for i in 0..n {
                    let offset = i * 2;
                    if offset + 2 <= self.data.len() {
                        let bytes = [self.data[offset], self.data[offset + 1]];
                        let half = u16::from_le_bytes(bytes);
                        let f32_val = f16_to_f32(half);
                        values.push(f32_val.to_bits() as i32);
                    }
                }
            }
            QuantType::F32 => {
                // F32: 每 4 字节存储一个浮点
                for i in 0..n {
                    let offset = i * 4;
                    if offset + 4 <= self.data.len() {
                        let bytes = [
                            self.data[offset],
                            self.data[offset + 1],
                            self.data[offset + 2],
                            self.data[offset + 3],
                        ];
                        let val = f32::from_le_bytes(bytes);
                        values.push(val.to_bits() as i32);
                    }
                }
            }
            QuantType::Mixed => {
                // Mixed: 按 F32 处理
                for i in 0..n {
                    let offset = i * 4;
                    if offset + 4 <= self.data.len() {
                        let bytes = [
                            self.data[offset],
                            self.data[offset + 1],
                            self.data[offset + 2],
                            self.data[offset + 3],
                        ];
                        let val = f32::from_le_bytes(bytes);
                        values.push(val.to_bits() as i32);
                    }
                }
            }
        }

        Ok(values)
    }

    /// 获取形状
    pub fn shape(&self) -> &TensorShape {
        &self.shape
    }

    /// 获取数据类型
    pub fn dtype(&self) -> QuantType {
        self.dtype
    }

    /// 元素数量
    pub fn num_elements(&self) -> usize {
        self.shape.num_elements()
    }

    /// 数据大小 (字节)
    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    /// 获取 f32 数据 (反量化后)
    pub fn as_f32(&self) -> Result<Vec<f32>, MemoryError> {
        if self.dtype == QuantType::F32 {
            let n = self.num_elements();
            let mut result = Vec::with_capacity(n);
            for i in 0..n {
                let offset = i * 4;
                if offset + 4 <= self.data.len() {
                    let bytes = [
                        self.data[offset],
                        self.data[offset + 1],
                        self.data[offset + 2],
                        self.data[offset + 3],
                    ];
                    result.push(f32::from_le_bytes(bytes));
                }
            }
            Ok(result)
        } else {
            let dequantized = self.dequantize()?;
            dequantized.as_f32()
        }
    }

    /// 获取原始数据
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// 重塑形状
    pub fn reshape(&self, new_shape: TensorShape) -> Result<Tensor, MemoryError> {
        if new_shape.num_elements() != self.shape.num_elements() {
            return Err(MemoryError::InvalidShape(format!(
                "重塑形状元素数不匹配: 原始 {} 个元素, 目标 {} 个元素",
                self.shape.num_elements(),
                new_shape.num_elements()
            )));
        }

        Ok(Tensor {
            data: self.data.clone(),
            shape: new_shape,
            dtype: self.dtype,
            quant_params: self.quant_params.clone(),
            name: self.name.clone(),
        })
    }

    /// 转置 (仅支持 2D 矩阵)
    pub fn transpose(&self) -> Result<Tensor, MemoryError> {
        if !self.shape.is_matrix() {
            return Err(MemoryError::InvalidShape(format!(
                "转置仅支持矩阵 (2D), 当前秩: {}",
                self.shape.rank()
            )));
        }

        let rows = self.shape.dims[0];
        let cols = self.shape.dims[1];
        let f32_data = self.as_f32()?;

        let mut transposed = vec![0.0f32; rows * cols];
        for i in 0..rows {
            for j in 0..cols {
                transposed[j * rows + i] = f32_data[i * cols + j];
            }
        }

        let new_shape = TensorShape::new(vec![cols, rows]);
        Ok(Tensor::new_f32(new_shape, transposed))
    }

    /// 切片
    pub fn slice(&self, ranges: &[Range<usize>]) -> Result<Tensor, MemoryError> {
        if ranges.len() != self.shape.rank() {
            return Err(MemoryError::InvalidShape(format!(
                "切片维度数 {} 与张量秩 {} 不匹配",
                ranges.len(),
                self.shape.rank()
            )));
        }

        // 验证范围
        for (i, range) in ranges.iter().enumerate() {
            if range.end > self.shape.dims[i] {
                return Err(MemoryError::InvalidShape(format!(
                    "切片范围超出维度 {} 的大小: [{}, {}) > {}",
                    i, range.start, range.end, self.shape.dims[i]
                )));
            }
        }

        let f32_data = self.as_f32()?;

        // 计算新形状
        let new_dims: Vec<usize> = ranges.iter().map(|r| r.end - r.start).collect();
        let new_shape = TensorShape::new(new_dims);

        // 提取切片数据
        let mut sliced = Vec::new();
        extract_slice(&f32_data, &self.shape.dims, ranges, 0, 0, &mut sliced);

        Ok(Tensor::new_f32(new_shape, sliced))
    }

    /// 拼接多个张量
    pub fn concat(tensors: &[&Tensor], axis: usize) -> Result<Tensor, MemoryError> {
        if tensors.is_empty() {
            return Err(MemoryError::InvalidShape("至少需要一个张量进行拼接".to_string()));
        }

        let first = &tensors[0];
        let rank = first.shape.rank();

        if axis >= rank {
            return Err(MemoryError::InvalidShape(format!(
                "拼接轴 {} 超出张量秩 {}",
                axis, rank
            )));
        }

        // 验证所有张量的形状兼容性
        for tensor in &tensors[1..] {
            if tensor.shape.rank() != rank {
                return Err(MemoryError::IncompatibleType(format!(
                    "张量秩不匹配: {} vs {}",
                    rank,
                    tensor.shape.rank()
                )));
            }
            for (i, (&d1, &d2)) in first
                .shape
                .dims
                .iter()
                .zip(tensor.shape.dims.iter())
                .enumerate()
            {
                if i != axis && d1 != d2 {
                    return Err(MemoryError::InvalidShape(format!(
                        "维度 {} 大小不匹配: {} vs {}",
                        i, d1, d2
                    )));
                }
            }
        }

        // 获取所有张量的 f32 数据
        let all_data: Result<Vec<Vec<f32>>, MemoryError> =
            tensors.iter().map(|t| t.as_f32()).collect();
        let all_data = all_data?;

        // 计算新形状
        let mut new_dims = first.shape.dims.clone();
        new_dims[axis] = tensors.iter().map(|t| t.shape.dims[axis]).sum();

        let new_shape = TensorShape::new(new_dims);

        // 拼接数据
        let mut result = Vec::new();
        for data in &all_data {
            result.extend_from_slice(data);
        }

        Ok(Tensor::new_f32(new_shape, result))
    }

    /// 获取张量名称
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// 设置张量名称
    pub fn set_name(&mut self, name: &str) {
        self.name = Some(name.to_string());
    }
}

/// 从多维数组中提取切片数据
fn extract_slice(
    data: &[f32],
    dims: &[usize],
    ranges: &[Range<usize>],
    dim_idx: usize,
    base_offset: usize,
    result: &mut Vec<f32>,
) {
    if dim_idx == dims.len() {
        if base_offset < data.len() {
            result.push(data[base_offset]);
        }
        return;
    }

    let stride: usize = dims[dim_idx + 1..].iter().product();
    for i in ranges[dim_idx].clone() {
        let offset = base_offset + i * stride;
        extract_slice(data, dims, ranges, dim_idx + 1, offset, result);
    }
}

/// 简化的 f32 转 f16 (半精度浮点)
fn f32_to_f16(val: f32) -> u16 {
    // 简化实现: 使用截断方式
    let bits = val.to_bits();
    let sign = (bits >> 31) & 1;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let frac = bits & 0x007F_FFFF;

    // F16 指数偏移为 15, F32 为 127
    let new_exp = exp - 127 + 15;

    if new_exp <= 0 {
        // 零或下溢
        0
    } else if new_exp >= 31 {
        // 无穷大或上溢
        (sign as u16) << 15 | 0x7C00
    } else {
        // 正常值
        let new_frac = frac >> 13;
        ((sign as u16) << 15) | ((new_exp as u16) << 10) | (new_frac as u16)
    }
}

/// 简化的 f16 转 f32 (半精度浮点)
fn f16_to_f32(half: u16) -> f32 {
    let sign = (half >> 15) & 1;
    let exp = ((half >> 10) & 0x1F) as i32;
    let frac = half & 0x03FF;

    if exp == 0 {
        // 零或次正规数
        if frac == 0 {
            0.0
        } else {
            // 次正规数
            let sign_f = if sign == 1 { -1.0 } else { 1.0 };
            sign_f * (frac as f32) / 1024.0 * (2.0f32).powi(-14)
        }
    } else if exp == 31 {
        // 无穷大或 NaN
        if frac == 0 {
            if sign == 1 { f32::NEG_INFINITY } else { f32::INFINITY }
        } else {
            f32::NAN
        }
    } else {
        // 正常值
        let sign_f = if sign == 1 { -1.0 } else { 1.0 };
        let f = 1.0 + (frac as f32) / 1024.0;
        sign_f * f * (2.0f32).powi(exp - 15)
    }
}

// ============================================================================
// 内存区域类型
// ============================================================================

/// 内存区域类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemoryRegionType {
    /// 模型权重 (只读)
    Weights = 0,
    /// 激活值 (读写)
    Activations = 1,
    /// 梯度 (读写)
    Gradients = 2,
    /// KV 缓存 (读写)
    KVCache = 3,
    /// 输入/输出缓冲区
    Buffer = 4,
    /// 临时工作区
    Scratch = 5,
}

/// 已分配的内存块
#[derive(Debug, Clone)]
struct AllocatedBlock {
    /// 起始地址 (相对于区域基址)
    offset: usize,
    /// 大小
    size: usize,
}

/// 内存区域
pub struct MemoryRegion {
    /// 区域 ID
    pub id: u64,
    /// 区域名称
    pub name: String,
    /// 区域类型
    pub region_type: MemoryRegionType,
    /// 起始地址 (虚拟)
    pub base_addr: u64,
    /// 大小 (字节)
    pub size: usize,
    /// 已使用大小
    pub used: usize,
    /// 对齐要求
    pub alignment: usize,
    /// 是否只读
    pub read_only: bool,
    /// 已分配的块
    allocated_blocks: Vec<AllocatedBlock>,
}

impl MemoryRegion {
    /// 创建新的内存区域
    pub fn new(id: u64, name: &str, region_type: MemoryRegionType, size: usize) -> Self {
        let read_only = matches!(region_type, MemoryRegionType::Weights);
        MemoryRegion {
            id,
            name: name.to_string(),
            region_type,
            base_addr: 0,
            size,
            used: 0,
            alignment: 16, // 默认 16 字节对齐
            read_only,
            allocated_blocks: Vec::new(),
        }
    }

    /// 分配内存
    pub fn allocate(&mut self, size: usize, alignment: usize) -> Result<u64, MemoryError> {
        // 验证对齐
        if !alignment.is_power_of_two() || alignment == 0 {
            return Err(MemoryError::InvalidAlignment(alignment));
        }

        let effective_alignment = alignment.max(self.alignment);

        // 查找合适的空闲位置 (首次适配)
        let mut current_offset = 0u64;

        // 按偏移排序已分配块
        self.allocated_blocks.sort_by_key(|b| b.offset);

        for block in &self.allocated_blocks {
            let block_end = block.offset as u64 + block.size as u64;
            // 计算当前偏移到下一个块之间的可用空间
            let aligned_offset = align_up(current_offset, effective_alignment as u64);
            let available = block.offset as u64 - aligned_offset;

            if available >= size as u64 {
                // 找到合适的空间
                let addr = self.base_addr + aligned_offset;
                self.allocated_blocks.push(AllocatedBlock {
                    offset: aligned_offset as usize,
                    size,
                });
                self.used += size;
                return Ok(addr);
            }

            current_offset = block_end;
        }

        // 检查最后一个块之后的空间
        let aligned_offset = align_up(current_offset, effective_alignment as u64);
        let available = self.size as u64 - aligned_offset;

        if available >= size as u64 {
            let addr = self.base_addr + aligned_offset;
            self.allocated_blocks.push(AllocatedBlock {
                offset: aligned_offset as usize,
                size,
            });
            self.used += size;
            return Ok(addr);
        }

        Err(MemoryError::RegionFull {
            region_id: self.id,
            requested: size,
            available: self.available(),
        })
    }

    /// 释放内存
    pub fn deallocate(&mut self, addr: u64, size: usize) -> Result<(), MemoryError> {
        let offset = (addr - self.base_addr) as usize;

        // 查找对应的块
        let block_idx = self
            .allocated_blocks
            .iter()
            .position(|b| b.offset == offset && b.size == size);

        match block_idx {
            Some(idx) => {
                self.allocated_blocks.remove(idx);
                self.used = self.used.saturating_sub(size);
                Ok(())
            }
            None => Err(MemoryError::DeallocateError(format!(
                "未找到地址 {} 处的已分配块",
                addr
            ))),
        }
    }

    /// 可用空间
    pub fn available(&self) -> usize {
        self.size.saturating_sub(self.used)
    }

    /// 使用率 (0.0 ~ 1.0)
    pub fn utilization(&self) -> f32 {
        if self.size == 0 {
            return 0.0;
        }
        self.used as f32 / self.size as f32
    }

    /// 碎片化率 (0.0 ~ 1.0)
    /// 基于已分配块之间的间隔计算
    pub fn fragmentation(&self) -> f32 {
        if self.allocated_blocks.is_empty() || self.size == 0 {
            return 0.0;
        }

        let mut sorted_blocks = self.allocated_blocks.clone();
        sorted_blocks.sort_by_key(|b| b.offset);

        // 计算总间隔
        let mut total_gap: usize = 0;
        let mut prev_end: usize = 0;

        for block in &sorted_blocks {
            if block.offset > prev_end {
                total_gap += block.offset - prev_end;
            }
            prev_end = block.offset + block.size;
        }

        // 末尾的空闲空间不算碎片
        let available = self.available();
        let internal_fragmentation = if available > total_gap {
            total_gap
        } else {
            total_gap
        };

        if self.size == 0 {
            0.0
        } else {
            internal_fragmentation as f32 / self.size as f32
        }
    }
}

/// 向上对齐到指定对齐值
fn align_up(addr: u64, alignment: u64) -> u64 {
    (addr + alignment - 1) & !(alignment - 1)
}

// ============================================================================
// 内存统计
// ============================================================================

/// 内存统计信息
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// 总容量
    pub total_capacity: usize,
    /// 总已使用
    pub total_used: usize,
    /// 总可用
    pub total_available: usize,
    /// 区域数量
    pub region_count: usize,
    /// 张量数量
    pub tensor_count: usize,
    /// 使用率百分比
    pub utilization_percent: f32,
}

// ============================================================================
// 内存池
// ============================================================================

/// 内存池 - 管理所有内存区域和张量
pub struct MemoryPool {
    /// 所有内存区域
    regions: HashMap<u64, MemoryRegion>,
    /// 张量注册表
    tensors: HashMap<String, Tensor>,
    /// 总容量
    total_capacity: usize,
    /// 总已使用
    total_used: usize,
    /// 下一个区域 ID
    next_region_id: u64,
}

impl MemoryPool {
    /// 创建新的内存池
    pub fn new(total_capacity: usize) -> Self {
        MemoryPool {
            regions: HashMap::new(),
            tensors: HashMap::new(),
            total_capacity,
            total_used: 0,
            next_region_id: 1,
        }
    }

    /// 创建内存区域
    pub fn create_region(
        &mut self,
        name: &str,
        region_type: MemoryRegionType,
        size: usize,
    ) -> Result<u64, MemoryError> {
        // 检查总容量
        let current_used: usize = self.regions.values().map(|r| r.size).sum();
        if current_used + size > self.total_capacity {
            return Err(MemoryError::OutOfMemory {
                requested: size,
                available: self.total_capacity - current_used,
            });
        }

        let id = self.next_region_id;
        self.next_region_id += 1;

        let region = MemoryRegion::new(id, name, region_type, size);
        self.regions.insert(id, region);

        Ok(id)
    }

    /// 删除内存区域
    pub fn remove_region(&mut self, region_id: u64) -> Result<(), MemoryError> {
        if self.regions.remove(&region_id).is_some() {
            Ok(())
        } else {
            Err(MemoryError::RegionNotFound(region_id))
        }
    }

    /// 注册张量
    pub fn register_tensor(
        &mut self,
        name: &str,
        tensor: Tensor,
        region_id: u64,
    ) -> Result<(), MemoryError> {
        // 检查区域是否存在
        if !self.regions.contains_key(&region_id) {
            return Err(MemoryError::RegionNotFound(region_id));
        }

        // 检查张量是否已存在
        if self.tensors.contains_key(name) {
            return Err(MemoryError::TensorAlreadyExists(name.to_string()));
        }

        let tensor_size = tensor.data_size();

        // 在区域中分配空间
        let region = self
            .regions
            .get_mut(&region_id)
            .ok_or(MemoryError::RegionNotFound(region_id))?;

        region.allocate(tensor_size, 16)?;

        self.total_used += tensor_size;

        let mut tensor = tensor;
        tensor.set_name(name);
        self.tensors.insert(name.to_string(), tensor);

        Ok(())
    }

    /// 获取张量
    pub fn get_tensor(&self, name: &str) -> Result<&Tensor, MemoryError> {
        self.tensors
            .get(name)
            .ok_or_else(|| MemoryError::TensorNotFound(name.to_string()))
    }

    /// 获取可变张量
    pub fn get_tensor_mut(&mut self, name: &str) -> Result<&mut Tensor, MemoryError> {
        self.tensors
            .get_mut(name)
            .ok_or_else(|| MemoryError::TensorNotFound(name.to_string()))
    }

    /// 释放张量
    pub fn release_tensor(&mut self, name: &str) -> Result<(), MemoryError> {
        if let Some(tensor) = self.tensors.remove(name) {
            self.total_used = self.total_used.saturating_sub(tensor.data_size());
            Ok(())
        } else {
            Err(MemoryError::TensorNotFound(name.to_string()))
        }
    }

    /// 列出所有张量名称
    pub fn list_tensors(&self) -> Vec<&str> {
        self.tensors.keys().map(|s| s.as_str()).collect()
    }

    /// 获取内存统计
    pub fn stats(&self) -> MemoryStats {
        let total_available = self.total_capacity.saturating_sub(self.total_used);
        let utilization_percent = if self.total_capacity > 0 {
            (self.total_used as f32 / self.total_capacity as f32) * 100.0
        } else {
            0.0
        };

        MemoryStats {
            total_capacity: self.total_capacity,
            total_used: self.total_used,
            total_available,
            region_count: self.regions.len(),
            tensor_count: self.tensors.len(),
            utilization_percent,
        }
    }

    /// 压缩/碎片整理
    pub fn compact(&mut self) -> Result<(), MemoryError> {
        for region in self.regions.values_mut() {
            // 简单的碎片整理: 清空并重新排列所有块
            let total_block_size: usize = region.allocated_blocks.iter().map(|b| b.size).sum();
            region.allocated_blocks.clear();
            region.used = total_block_size;

            // 创建一个合并的大块
            if total_block_size > 0 {
                region.allocated_blocks.push(AllocatedBlock {
                    offset: 0,
                    size: total_block_size,
                });
            }
        }
        Ok(())
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // QuantType 测试
    // ========================================================================

    #[test]
    fn test_quant_type_bytes_per_element() {
        assert_eq!(QuantType::Q4.bytes_per_element(), 1);
        assert_eq!(QuantType::Q8.bytes_per_element(), 1);
        assert_eq!(QuantType::F16.bytes_per_element(), 2);
        assert_eq!(QuantType::B1.bytes_per_element(), 1);
        assert_eq!(QuantType::F32.bytes_per_element(), 4);
        assert_eq!(QuantType::U8.bytes_per_element(), 1);
        assert_eq!(QuantType::Mixed.bytes_per_element(), 4);
    }

    #[test]
    fn test_quant_type_bits() {
        assert_eq!(QuantType::Q4.bits(), 4);
        assert_eq!(QuantType::Q8.bits(), 8);
        assert_eq!(QuantType::F16.bits(), 16);
        assert_eq!(QuantType::B1.bits(), 1);
        assert_eq!(QuantType::F32.bits(), 32);
        assert_eq!(QuantType::U8.bits(), 8);
        assert_eq!(QuantType::Mixed.bits(), 32);
    }

    #[test]
    fn test_quant_type_is_quantized() {
        assert!(QuantType::Q4.is_quantized());
        assert!(QuantType::Q8.is_quantized());
        assert!(QuantType::B1.is_quantized());
        assert!(QuantType::U8.is_quantized());
        assert!(!QuantType::F16.is_quantized());
        assert!(!QuantType::F32.is_quantized());
        assert!(!QuantType::Mixed.is_quantized());
    }

    // ========================================================================
    // QuantParams 测试
    // ========================================================================

    #[test]
    fn test_quant_params_from_range() {
        let params = QuantParams::from_range(QuantType::Q8, -1.0, 1.0);
        assert_eq!(params.quant_type, QuantType::Q8);
        assert_eq!(params.min_val, -1.0);
        assert_eq!(params.max_val, 1.0);
        // 缩放因子应约为 2.0 / 256
        assert!((params.scale - 2.0 / 256.0).abs() < 0.001);
    }

    #[test]
    fn test_quantize_dequantize() {
        let params = QuantParams::from_range(QuantType::Q8, -10.0, 10.0);

        // 测试量化
        let quantized = params.quantize(5.0);
        assert!(quantized >= -128 && quantized <= 127);

        // 测试反量化
        let dequantized = params.dequantize(quantized);
        // 允许一定的量化误差
        assert!((dequantized - 5.0).abs() < 0.1);

        // 测试边界值
        let q_min = params.quantize(-10.0);
        assert!(q_min >= -128);
        let q_max = params.quantize(10.0);
        assert!(q_max <= 127);
    }

    #[test]
    fn test_quantize_batch() {
        let params = QuantParams::from_range(QuantType::Q8, 0.0, 100.0);
        let values = vec![0.0, 25.0, 50.0, 75.0, 100.0];
        let quantized = params.quantize_batch(&values);

        assert_eq!(quantized.len(), 5);
        for &q in &quantized {
            assert!(q >= -128 && q <= 127);
        }
    }

    #[test]
    fn test_dequantize_batch() {
        let params = QuantParams::from_range(QuantType::Q8, 0.0, 100.0);
        let values = vec![0, 32, 64, 96, 127];
        let dequantized = params.dequantize_batch(&values);

        assert_eq!(dequantized.len(), 5);
        // 验证反量化值在合理范围内
        for &val in &dequantized {
            assert!(val >= 0.0 && val <= 100.0);
        }
    }

    // ========================================================================
    // Tensor 测试
    // ========================================================================

    #[test]
    fn test_tensor_new_f32() {
        let shape = TensorShape::new(vec![2, 3]);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tensor = Tensor::new_f32(shape, data.clone());

        assert_eq!(tensor.dtype(), QuantType::F32);
        assert_eq!(tensor.num_elements(), 6);
        assert_eq!(tensor.data_size(), 24); // 6 * 4 字节
        assert_eq!(tensor.shape().rank(), 2);

        let f32_data = tensor.as_f32().unwrap();
        assert_eq!(f32_data, data);
    }

    #[test]
    fn test_tensor_shape() {
        let scalar_shape = TensorShape::new(vec![]);
        assert!(scalar_shape.is_scalar());
        assert!(!scalar_shape.is_vector());
        assert!(!scalar_shape.is_matrix());
        assert_eq!(scalar_shape.rank(), 0);
        assert_eq!(scalar_shape.num_elements(), 1);

        let vector_shape = TensorShape::new(vec![5]);
        assert!(!vector_shape.is_scalar());
        assert!(vector_shape.is_vector());
        assert!(!vector_shape.is_matrix());
        assert_eq!(vector_shape.rank(), 1);
        assert_eq!(vector_shape.num_elements(), 5);

        let matrix_shape = TensorShape::new(vec![3, 4]);
        assert!(!matrix_shape.is_scalar());
        assert!(!matrix_shape.is_vector());
        assert!(matrix_shape.is_matrix());
        assert_eq!(matrix_shape.rank(), 2);
        assert_eq!(matrix_shape.num_elements(), 12);
    }

    #[test]
    fn test_tensor_quantize() {
        let shape = TensorShape::new(vec![4]);
        let data = vec![0.0, 0.5, 1.0, 1.5];
        let tensor = Tensor::new_f32(shape, data);

        // 量化为 Q8
        let quantized = tensor.quantize(QuantType::Q8).unwrap();
        assert_eq!(quantized.dtype(), QuantType::Q8);
        assert_eq!(quantized.num_elements(), 4);

        // 反量化并验证
        let dequantized = quantized.dequantize().unwrap();
        let f32_data = dequantized.as_f32().unwrap();

        // 允许一定的量化误差
        assert!((f32_data[0] - 0.0).abs() < 0.1);
        assert!((f32_data[1] - 0.5).abs() < 0.1);
        assert!((f32_data[2] - 1.0).abs() < 0.1);
        assert!((f32_data[3] - 1.5).abs() < 0.1);
    }

    #[test]
    fn test_tensor_dequantize() {
        let shape = TensorShape::new(vec![3]);
        let data = vec![-1.0, 0.0, 1.0];
        let tensor = Tensor::new_f32(shape, data.clone());

        // F32 张量反量化应返回自身
        let dequantized = tensor.dequantize().unwrap();
        assert_eq!(dequantized.dtype(), QuantType::F32);

        let f32_data = dequantized.as_f32().unwrap();
        for i in 0..3 {
            assert!((f32_data[i] - data[i]).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_tensor_reshape() {
        let shape = TensorShape::new(vec![2, 3]);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tensor = Tensor::new_f32(shape, data);

        // 重塑为 (3, 2)
        let new_shape = TensorShape::new(vec![3, 2]);
        let reshaped = tensor.reshape(new_shape).unwrap();
        assert_eq!(reshaped.shape().dims, vec![3, 2]);
        assert_eq!(reshaped.num_elements(), 6);

        // 元素数不匹配应返回错误
        let bad_shape = TensorShape::new(vec![2, 2]);
        assert!(tensor.reshape(bad_shape).is_err());
    }

    #[test]
    fn test_tensor_transpose() {
        let shape = TensorShape::new(vec![2, 3]);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tensor = Tensor::new_f32(shape, data);

        let transposed = tensor.transpose().unwrap();
        assert_eq!(transposed.shape().dims, vec![3, 2]);

        let t_data = transposed.as_f32().unwrap();
        // 原始矩阵:
        // [1, 2, 3]
        // [4, 5, 6]
        // 转置后:
        // [1, 4]
        // [2, 5]
        // [3, 6]
        assert_eq!(t_data, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);

        // 非矩阵转置应返回错误
        let vec_tensor = Tensor::new_f32(TensorShape::new(vec![4]), vec![1.0, 2.0, 3.0, 4.0]);
        assert!(vec_tensor.transpose().is_err());
    }

    #[test]
    fn test_tensor_slice() {
        let shape = TensorShape::new(vec![3, 4]);
        let data: Vec<f32> = (0..12).map(|i| i as f32).collect();
        let tensor = Tensor::new_f32(shape, data);

        // 切片: 取第 1 行, 第 1~3 列
        let sliced = tensor.slice(&[1..2, 1..3]).unwrap();
        assert_eq!(sliced.shape().dims, vec![1, 2]);

        let s_data = sliced.as_f32().unwrap();
        // 原始矩阵:
        // [0,  1,  2,  3]
        // [4,  5,  6,  7]
        // [8,  9, 10, 11]
        // 切片 [1..2, 1..3] = [5, 6]
        assert_eq!(s_data, vec![5.0, 6.0]);
    }

    #[test]
    fn test_tensor_concat() {
        // 沿轴 0 拼接两个矩阵
        let shape1 = TensorShape::new(vec![2, 3]);
        let data1 = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tensor1 = Tensor::new_f32(shape1, data1);

        let shape2 = TensorShape::new(vec![1, 3]);
        let data2 = vec![7.0, 8.0, 9.0];
        let tensor2 = Tensor::new_f32(shape2, data2);

        let result = Tensor::concat(&[&tensor1, &tensor2], 0).unwrap();
        assert_eq!(result.shape().dims, vec![3, 3]);

        let r_data = result.as_f32().unwrap();
        assert_eq!(
            r_data,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        );

        // 空张量列表应返回错误
        assert!(Tensor::concat(&[], 0).is_err());
    }

    // ========================================================================
    // MemoryRegion 测试
    // ========================================================================

    #[test]
    fn test_region_allocate() {
        let mut region = MemoryRegion::new(1, "test", MemoryRegionType::Buffer, 1024);

        let addr1 = region.allocate(100, 16).unwrap();
        assert_eq!(addr1, region.base_addr); // 第一个分配从基址开始

        let addr2 = region.allocate(200, 16).unwrap();
        assert!(addr2 > addr1);

        assert_eq!(region.used, 300);
        assert_eq!(region.available(), 724);
    }

    #[test]
    fn test_region_deallocate() {
        let mut region = MemoryRegion::new(1, "test", MemoryRegionType::Buffer, 1024);

        let addr = region.allocate(100, 16).unwrap();
        assert_eq!(region.used, 100);

        region.deallocate(addr, 100).unwrap();
        assert_eq!(region.used, 0);

        // 释放不存在的地址应返回错误
        assert!(region.deallocate(9999, 100).is_err());
    }

    #[test]
    fn test_region_utilization() {
        let mut region = MemoryRegion::new(1, "test", MemoryRegionType::Buffer, 1024);

        assert_eq!(region.utilization(), 0.0);

        region.allocate(512, 16).unwrap();
        assert!((region.utilization() - 0.5).abs() < 0.01);

        region.allocate(512, 16).unwrap();
        assert!((region.utilization() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_region_full() {
        let mut region = MemoryRegion::new(1, "test", MemoryRegionType::Buffer, 100);

        region.allocate(80, 16).unwrap();

        // 尝试分配超过可用空间的内存
        let result = region.allocate(50, 16);
        assert!(result.is_err());
        match result.unwrap_err() {
            MemoryError::RegionFull {
                region_id,
                requested,
                available,
            } => {
                assert_eq!(region_id, 1);
                assert_eq!(requested, 50);
                assert!(available < 50);
            }
            other => panic!("期望 RegionFull 错误, 得到: {:?}", other),
        }
    }

    // ========================================================================
    // MemoryPool 测试
    // ========================================================================

    #[test]
    fn test_pool_create_region() {
        let mut pool = MemoryPool::new(10240);

        let region_id = pool
            .create_region("weights", MemoryRegionType::Weights, 4096)
            .unwrap();
        assert!(region_id > 0);

        let stats = pool.stats();
        assert_eq!(stats.region_count, 1);
    }

    #[test]
    fn test_pool_register_tensor() {
        let mut pool = MemoryPool::new(10240);

        let region_id = pool
            .create_region("activations", MemoryRegionType::Activations, 4096)
            .unwrap();

        let shape = TensorShape::new(vec![2, 3]);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let tensor = Tensor::new_f32(shape, data);

        pool.register_tensor("test_tensor", tensor, region_id).unwrap();

        let retrieved = pool.get_tensor("test_tensor").unwrap();
        assert_eq!(retrieved.num_elements(), 6);

        // 重复注册应返回错误
        let tensor2 = Tensor::new_f32(TensorShape::new(vec![1]), vec![1.0]);
        assert!(pool
            .register_tensor("test_tensor", tensor2, region_id)
            .is_err());
    }

    #[test]
    fn test_pool_get_tensor() {
        let mut pool = MemoryPool::new(10240);

        let region_id = pool
            .create_region("buffer", MemoryRegionType::Buffer, 4096)
            .unwrap();

        let tensor = Tensor::new_f32(TensorShape::new(vec![3]), vec![1.0, 2.0, 3.0]);
        pool.register_tensor("my_tensor", tensor, region_id).unwrap();

        // 获取存在的张量
        let retrieved = pool.get_tensor("my_tensor").unwrap();
        assert_eq!(retrieved.num_elements(), 3);

        // 获取不存在的张量
        assert!(pool.get_tensor("nonexistent").is_err());

        // 获取可变张量
        let mut_retrieved = pool.get_tensor_mut("my_tensor").unwrap();
        assert_eq!(mut_retrieved.num_elements(), 3);
    }

    #[test]
    fn test_pool_release_tensor() {
        let mut pool = MemoryPool::new(10240);

        let region_id = pool
            .create_region("buffer", MemoryRegionType::Buffer, 4096)
            .unwrap();

        let tensor = Tensor::new_f32(TensorShape::new(vec![4]), vec![1.0, 2.0, 3.0, 4.0]);
        pool.register_tensor("temp", tensor, region_id).unwrap();

        let stats_before = pool.stats();
        assert_eq!(stats_before.tensor_count, 1);
        assert!(stats_before.total_used > 0);

        pool.release_tensor("temp").unwrap();

        let stats_after = pool.stats();
        assert_eq!(stats_after.tensor_count, 0);
        assert_eq!(stats_after.total_used, 0);

        // 释放不存在的张量应返回错误
        assert!(pool.release_tensor("nonexistent").is_err());
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = MemoryPool::new(10240);

        let region_id = pool
            .create_region("buffer", MemoryRegionType::Buffer, 4096)
            .unwrap();

        let tensor = Tensor::new_f32(TensorShape::new(vec![4]), vec![1.0, 2.0, 3.0, 4.0]);
        pool.register_tensor("data", tensor, region_id).unwrap();

        let stats = pool.stats();
        assert_eq!(stats.total_capacity, 10240);
        assert!(stats.total_used > 0);
        assert_eq!(stats.region_count, 1);
        assert_eq!(stats.tensor_count, 1);
        assert!(stats.utilization_percent > 0.0);
        assert!(stats.utilization_percent < 100.0);
    }

    #[test]
    fn test_pool_out_of_memory() {
        let mut pool = MemoryPool::new(100);

        // 创建一个接近总容量的区域
        pool.create_region("big", MemoryRegionType::Buffer, 80).unwrap();

        // 尝试创建超出容量的区域
        let result = pool.create_region("too_big", MemoryRegionType::Buffer, 50);
        assert!(result.is_err());
        match result.unwrap_err() {
            MemoryError::OutOfMemory { requested, available } => {
                assert_eq!(requested, 50);
                assert!(available < 50);
            }
            other => panic!("期望 OutOfMemory 错误, 得到: {:?}", other),
        }
    }
}
