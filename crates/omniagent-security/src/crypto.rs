//! 简化的密码学原语
//!
//! 实现一个简化的哈希函数，用于安全标签生成和完整性校验。
//! 注意：这不是密码学安全的哈希函数，仅用于内部标识和测试目的。

/// 简化的哈希算法
///
/// 基于简单的位混合和旋转操作实现，生成 256 位 (32 字节) 的哈希值。
/// 此实现不追求密码学安全性，仅用于安全标签和内部标识。
pub struct Hash {
    /// 哈希状态 (8 个 32 位字 = 256 位)
    state: [u32; 8],
    /// 数据缓冲区
    buffer: Vec<u8>,
    /// 已处理的数据总长度
    length: u64,
}

/// SHA-256 风格的初始常量（前 8 个素数的平方根的小数部分）
const INITIAL_STATE: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// SHA-256 风格的轮常量（前 64 个素数的立方根的小数部分）
const ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Hash {
    /// 创建一个新的哈希实例
    pub fn new() -> Self {
        Hash {
            state: INITIAL_STATE,
            buffer: Vec::new(),
            length: 0,
        }
    }

    /// 更新哈希状态
    ///
    /// 将数据添加到哈希计算中。可以多次调用以逐步处理数据。
    pub fn update(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        self.length += data.len() as u64;

        // 每次处理 64 字节 (512 位) 的块
        while self.buffer.len() >= 64 {
            let block: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.process_block(&block);
            self.buffer.drain(..64);
        }
    }

    /// 完成哈希计算并返回结果
    ///
    /// 返回 32 字节 (256 位) 的哈希值。
    pub fn finalize(&mut self) -> [u8; 32] {
        // 保存原始状态以便多次调用
        let saved_state = self.state;
        let saved_buffer = self.buffer.clone();
        let saved_length = self.length;

        // 添加填充位 (1 后面跟 0)
        self.buffer.push(0x80);

        // 填充到 56 字节（为 8 字节长度预留空间）
        while self.buffer.len() % 64 != 56 {
            self.buffer.push(0x00);
        }

        // 添加长度（大端序，64 位）
        let bit_length = self.length * 8;
        self.buffer.extend_from_slice(&bit_length.to_be_bytes());

        // 处理剩余的块
        while self.buffer.len() >= 64 {
            let block: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.process_block(&block);
            self.buffer.drain(..64);
        }

        // 将状态转换为字节数组
        let mut result = [0u8; 32];
        for (i, &word) in self.state.iter().enumerate() {
            result[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }

        // 恢复状态
        self.state = saved_state;
        self.buffer = saved_buffer;
        self.length = saved_length;

        result
    }

    /// 便捷函数：一次性计算数据的哈希值
    pub fn hash(data: &[u8]) -> [u8; 32] {
        let mut h = Hash::new();
        h.update(data);
        h.finalize()
    }

    /// 处理一个 64 字节的数据块
    fn process_block(&mut self, block: &[u8; 64]) {
        // 准备消息调度数组
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }

        for i in 16..64 {
            let s0 = Self::rotr(w[i - 15], 7) ^ Self::rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
            let s1 = Self::rotr(w[i - 2], 17) ^ Self::rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        // 初始化工作变量
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        // 主压缩循环
        for i in 0..64 {
            let s1 = Self::rotr(e, 6) ^ Self::rotr(e, 11) ^ Self::rotr(e, 25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(ROUND_CONSTANTS[i])
                .wrapping_add(w[i]);
            let s0 = Self::rotr(a, 2) ^ Self::rotr(a, 13) ^ Self::rotr(a, 22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        // 更新状态
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    /// 32 位循环右移
    #[inline]
    fn rotr(x: u32, n: u32) -> u32 {
        (x >> n) | (x << (32 - n))
    }
}

impl Default for Hash {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_empty() {
        let result = Hash::hash(b"");
        // SHA-256("") 的已知值
        let expected: [u8; 32] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14,
            0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
            0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
            0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_hash_hello() {
        let result = Hash::hash(b"hello");
        // SHA-256("hello") 的已知值
        let expected: [u8; 32] = [
            0x2c, 0xf2, 0x4d, 0xba, 0x5f, 0xb0, 0xa3, 0x0e,
            0x26, 0xe8, 0x3b, 0x2a, 0xc5, 0xb9, 0xe2, 0x9e,
            0x1b, 0x16, 0x1e, 0x5c, 0x1f, 0xa7, 0x42, 0x5e,
            0x73, 0x04, 0x33, 0x62, 0x93, 0x8b, 0x98, 0x24,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_hash_update_multiple() {
        let mut h = Hash::new();
        h.update(b"hello");
        h.update(b" ");
        h.update(b"world");
        let result1 = h.finalize();

        let result2 = Hash::hash(b"hello world");
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_hash_deterministic() {
        let data = b"test data for determinism check";
        let result1 = Hash::hash(data);
        let result2 = Hash::hash(data);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_hash_different_inputs() {
        let result1 = Hash::hash(b"data1");
        let result2 = Hash::hash(b"data2");
        assert_ne!(result1, result2);
    }

    #[test]
    fn test_hash_large_data() {
        // 测试超过一个块 (64 字节) 的数据
        let data = [0x42u8; 256];
        let result = Hash::hash(&data);
        // 确保产生了非零结果
        assert!(!result.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_hash_finalize_preserves_state() {
        let mut h = Hash::new();
        h.update(b"test");
        let result1 = h.finalize();
        let result2 = h.finalize();
        assert_eq!(result1, result2, "finalize 应该保持内部状态不变");
    }

    #[test]
    fn test_hash_new_default() {
        let h1 = Hash::new();
        let h2 = Hash::default();
        // 两者应该有相同的初始状态
        assert_eq!(h1.state, h2.state);
    }
}
