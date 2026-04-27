// VM Exit 处理
//
// 定义 VM Exit 原因、I/O 模拟器和 VM Exit 处理器

use std::collections::HashMap;

use crate::error::VirtError;
use crate::vcpu::Vcpu;

/// VM Exit 原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VmExitReason {
    /// 异常或 NMI
    ExceptionOrNmi = 0,
    /// 外部中断
    ExternalInterrupt = 1,
    /// 三重故障
    TripleFault = 2,
    /// INIT 信号
    InitSignal = 3,
    /// Startup IPI
    StartupIpi = 4,
    /// I/O 指令
    IoInstruction = 5,
    /// CPUID
    Cpuid = 6,
    /// HLT
    Hlt = 7,
    /// INVD
    Invd = 8,
    /// INVLPG
    Invlpg = 9,
    /// RDPMC
    Rdpmc = 10,
    /// RDTSC
    Rdtsc = 11,
    /// VMCALL
    Vmcall = 12,
    /// MOV CR
    MovCr = 13,
    /// MOV DR
    MovDr = 14,
    /// I/O 访问
    IoAccess = 15,
    /// MSR 读取
    MsrRead = 16,
    /// MSR 写入
    MsrWrite = 17,
    /// EPT 违规
    EptViolation = 18,
    /// EPT 配置错误
    EptMisconfig = 19,
    /// APIC 访问
    ApicAccess = 20,
    /// Posted Interrupt
    PostedInterrupt = 21,
    /// 未知原因
    Unknown = 255,
}

/// VM Exit 信息
#[derive(Debug, Clone)]
pub struct VmExitInfo {
    /// Exit 原因
    pub reason: VmExitReason,
    /// 限定词
    pub qualification: u64,
    /// 指令长度
    pub instruction_length: u8,
    /// 客户机线性地址
    pub guest_linear_address: u64,
    /// 客户机物理地址
    pub guest_physical_address: u64,
    /// Exit 指令信息
    pub exit_instruction_info: u32,
}

impl VmExitInfo {
    /// 创建 I/O 访问的 VM Exit 信息
    pub fn io_exit(port: u16, is_write: bool, size: u8) -> Self {
        let mut qualification = port as u64;
        if is_write {
            qualification |= 1 << 3;
        }
        qualification |= (size as u64) << 16;
        Self {
            reason: VmExitReason::IoAccess,
            qualification,
            instruction_length: 0,
            guest_linear_address: 0,
            guest_physical_address: 0,
            exit_instruction_info: 0,
        }
    }

    /// 从 qualification 解析 I/O 操作
    pub fn parse_io_operation(&self) -> IoOperation {
        let port = (self.qualification & 0xFFFF) as u16;
        let is_write = (self.qualification >> 3) & 1 == 1;
        let size = ((self.qualification >> 16) & 0x7) as u8;
        IoOperation {
            port,
            size,
            is_write,
            value: 0,
        }
    }
}

/// I/O 操作
#[derive(Debug, Clone)]
pub struct IoOperation {
    /// 端口号
    pub port: u16,
    /// 操作大小（1, 2, 或 4 字节）
    pub size: u8,
    /// 是否为写操作
    pub is_write: bool,
    /// 值
    pub value: u32,
}

/// VM Exit 处理结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitAction {
    /// 继续执行
    Continue,
    /// 跳过当前指令
    AdvanceRip,
    /// 暂停 vCPU
    Halt,
    /// 关闭 VM
    Shutdown,
    /// 重试
    Retry,
    /// 注入异常
    InjectException {
        /// 中断向量
        vector: u8,
        /// 错误码
        error_code: Option<u32>,
    },
}

/// I/O 端口处理 trait
pub trait IoPortHandler: Send + Sync {
    /// 处理端口读取
    fn handle_read(&self, port: u16, size: u8) -> Result<u32, VirtError>;
    /// 处理端口写入
    fn handle_write(&self, port: u16, size: u8, value: u32) -> Result<(), VirtError>;
}

/// I/O 模拟器
pub struct IoEmulator {
    /// 端口处理函数映射
    port_handlers: HashMap<u16, Box<dyn IoPortHandler>>,
}

impl IoEmulator {
    /// 创建新的 I/O 模拟器
    pub fn new() -> Self {
        Self {
            port_handlers: HashMap::new(),
        }
    }

    /// 注册端口处理函数
    pub fn register_port(&mut self, port: u16, handler: Box<dyn IoPortHandler>) {
        self.port_handlers.insert(port, handler);
    }

    /// 处理 I/O 操作
    pub fn handle_io(&self, op: &IoOperation) -> Result<ExitAction, VirtError> {
        if let Some(handler) = self.port_handlers.get(&op.port) {
            if op.is_write {
                handler.handle_write(op.port, op.size, op.value)?;
            } else {
                handler.handle_read(op.port, op.size)?;
            }
            Ok(ExitAction::AdvanceRip)
        } else {
            // 未注册的端口，忽略并继续
            Ok(ExitAction::AdvanceRip)
        }
    }

    /// 检查端口是否已注册
    pub fn has_port(&self, port: u16) -> bool {
        self.port_handlers.contains_key(&port)
    }
}

impl Default for IoEmulator {
    fn default() -> Self {
        Self::new()
    }
}

/// VM Exit 处理器
pub struct VmExitHandler {
    /// I/O 模拟器
    io_emulator: IoEmulator,
    /// 总 Exit 计数
    exit_count: u64,
    /// 按原因分类的 Exit 计数
    exit_counts_by_reason: HashMap<VmExitReason, u64>,
}

impl VmExitHandler {
    /// 创建新的 VM Exit 处理器
    pub fn new() -> Self {
        Self {
            io_emulator: IoEmulator::new(),
            exit_count: 0,
            exit_counts_by_reason: HashMap::new(),
        }
    }

    /// 处理 VM Exit
    pub fn handle_exit(&mut self, info: &VmExitInfo, vcpu: &mut Vcpu) -> Result<ExitAction, VirtError> {
        // 更新统计
        self.exit_count += 1;
        *self.exit_counts_by_reason.entry(info.reason).or_insert(0) += 1;
        vcpu.record_exit();

        match info.reason {
            VmExitReason::IoAccess => {
                let mut op = info.parse_io_operation();
                // 对于写操作，从 qualification 中提取值
                if op.is_write {
                    op.value = (info.qualification >> 32) as u32;
                }
                self.io_emulator.handle_io(&op)
            }
            VmExitReason::Hlt => Ok(ExitAction::Halt),
            VmExitReason::Cpuid => Ok(ExitAction::AdvanceRip),
            VmExitReason::Rdtsc => Ok(ExitAction::AdvanceRip),
            VmExitReason::MsrRead => Ok(ExitAction::AdvanceRip),
            VmExitReason::MsrWrite => Ok(ExitAction::AdvanceRip),
            VmExitReason::TripleFault => Ok(ExitAction::Shutdown),
            VmExitReason::ExceptionOrNmi => Ok(ExitAction::Continue),
            VmExitReason::ExternalInterrupt => Ok(ExitAction::Continue),
            VmExitReason::EptViolation => Ok(ExitAction::Continue),
            VmExitReason::EptMisconfig => Ok(ExitAction::Shutdown),
            _ => Ok(ExitAction::AdvanceRip),
        }
    }

    /// 注册 I/O 端口处理函数
    pub fn register_io_handler(&mut self, port: u16, handler: Box<dyn IoPortHandler>) {
        self.io_emulator.register_port(port, handler);
    }

    /// 获取 Exit 统计信息
    pub fn exit_stats(&self) -> &HashMap<VmExitReason, u64> {
        &self.exit_counts_by_reason
    }

    /// 获取总 Exit 数
    pub fn total_exits(&self) -> u64 {
        self.exit_count
    }
}

impl Default for VmExitHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用 I/O 端口处理函数
    struct TestPortHandler {
        value: u32,
    }

    impl TestPortHandler {
        fn new(value: u32) -> Self {
            Self { value }
        }
    }

    impl IoPortHandler for TestPortHandler {
        fn handle_read(&self, _port: u16, _size: u8) -> Result<u32, VirtError> {
            Ok(self.value)
        }

        fn handle_write(&self, _port: u16, _size: u8, _value: u32) -> Result<(), VirtError> {
            Ok(())
        }
    }

    #[test]
    fn test_vm_exit_reason_repr() {
        assert_eq!(VmExitReason::ExceptionOrNmi as u8, 0);
        assert_eq!(VmExitReason::IoAccess as u8, 15);
        assert_eq!(VmExitReason::EptViolation as u8, 18);
        assert_eq!(VmExitReason::Unknown as u8, 255);
    }

    #[test]
    fn test_io_emulator_register_port() {
        let mut emulator = IoEmulator::new();
        assert!(!emulator.has_port(0x60));

        emulator.register_port(0x60, Box::new(TestPortHandler::new(0)));
        assert!(emulator.has_port(0x60));
    }

    #[test]
    fn test_io_emulator_handle_io_read() {
        let mut emulator = IoEmulator::new();
        emulator.register_port(0x60, Box::new(TestPortHandler::new(0xAB)));

        let op = IoOperation {
            port: 0x60,
            size: 1,
            is_write: false,
            value: 0,
        };

        let result = emulator.handle_io(&op).unwrap();
        assert_eq!(result, ExitAction::AdvanceRip);
    }

    #[test]
    fn test_io_emulator_handle_io_write() {
        let mut emulator = IoEmulator::new();
        emulator.register_port(0x60, Box::new(TestPortHandler::new(0)));

        let op = IoOperation {
            port: 0x60,
            size: 1,
            is_write: true,
            value: 0xFF,
        };

        let result = emulator.handle_io(&op).unwrap();
        assert_eq!(result, ExitAction::AdvanceRip);
    }

    #[test]
    fn test_io_emulator_handle_unregistered_port() {
        let emulator = IoEmulator::new();

        let op = IoOperation {
            port: 0x9999,
            size: 4,
            is_write: true,
            value: 0,
        };

        // 未注册的端口应该返回 AdvanceRip（忽略）
        let result = emulator.handle_io(&op).unwrap();
        assert_eq!(result, ExitAction::AdvanceRip);
    }

    #[test]
    fn test_vm_exit_handler_handle_hlt() {
        let mut handler = VmExitHandler::new();
        let mut vcpu = Vcpu::new(0);

        let info = VmExitInfo {
            reason: VmExitReason::Hlt,
            qualification: 0,
            instruction_length: 1,
            guest_linear_address: 0,
            guest_physical_address: 0,
            exit_instruction_info: 0,
        };

        let action = handler.handle_exit(&info, &mut vcpu).unwrap();
        assert_eq!(action, ExitAction::Halt);
        assert_eq!(handler.total_exits(), 1);
        assert_eq!(vcpu.exit_count, 1);
    }

    #[test]
    fn test_vm_exit_handler_handle_io_exit() {
        let mut handler = VmExitHandler::new();
        handler.register_io_handler(0x60, Box::new(TestPortHandler::new(0)));
        let mut vcpu = Vcpu::new(0);

        let info = VmExitInfo::io_exit(0x60, true, 1);
        let action = handler.handle_exit(&info, &mut vcpu).unwrap();
        assert_eq!(action, ExitAction::AdvanceRip);
        assert_eq!(handler.total_exits(), 1);
    }

    #[test]
    fn test_vm_exit_handler_exit_stats() {
        let mut handler = VmExitHandler::new();
        let mut vcpu = Vcpu::new(0);

        // 处理多个 HLT exit
        for _ in 0..3 {
            let info = VmExitInfo {
                reason: VmExitReason::Hlt,
                qualification: 0,
                instruction_length: 1,
                guest_linear_address: 0,
                guest_physical_address: 0,
                exit_instruction_info: 0,
            };
            handler.handle_exit(&info, &mut vcpu).unwrap();
        }

        // 处理 CPUID exit
        let info = VmExitInfo {
            reason: VmExitReason::Cpuid,
            qualification: 0,
            instruction_length: 2,
            guest_linear_address: 0,
            guest_physical_address: 0,
            exit_instruction_info: 0,
        };
        handler.handle_exit(&info, &mut vcpu).unwrap();

        assert_eq!(handler.total_exits(), 4);
        assert_eq!(*handler.exit_stats().get(&VmExitReason::Hlt).unwrap(), 3);
        assert_eq!(*handler.exit_stats().get(&VmExitReason::Cpuid).unwrap(), 1);
    }

    #[test]
    fn test_vm_exit_handler_triple_fault() {
        let mut handler = VmExitHandler::new();
        let mut vcpu = Vcpu::new(0);

        let info = VmExitInfo {
            reason: VmExitReason::TripleFault,
            qualification: 0,
            instruction_length: 0,
            guest_linear_address: 0,
            guest_physical_address: 0,
            exit_instruction_info: 0,
        };

        let action = handler.handle_exit(&info, &mut vcpu).unwrap();
        assert_eq!(action, ExitAction::Shutdown);
    }

    #[test]
    fn test_vm_exit_info_parse_io_operation() {
        // 构造一个写端口 0x3F8、大小 1 字节的 I/O exit
        let info = VmExitInfo::io_exit(0x3F8, true, 1);
        let op = info.parse_io_operation();
        assert_eq!(op.port, 0x3F8);
        assert!(op.is_write);
        assert_eq!(op.size, 1);

        // 构造一个读端口 0x60、大小 4 字节的 I/O exit
        let info = VmExitInfo::io_exit(0x60, false, 4);
        let op = info.parse_io_operation();
        assert_eq!(op.port, 0x60);
        assert!(!op.is_write);
        assert_eq!(op.size, 4);
    }

    #[test]
    fn test_exit_action_equality() {
        assert_eq!(ExitAction::Continue, ExitAction::Continue);
        assert_eq!(ExitAction::Halt, ExitAction::Halt);
        assert_eq!(ExitAction::Shutdown, ExitAction::Shutdown);
        assert_ne!(ExitAction::Continue, ExitAction::Halt);

        assert_eq!(
            ExitAction::InjectException { vector: 13, error_code: Some(0) },
            ExitAction::InjectException { vector: 13, error_code: Some(0) }
        );
        assert_ne!(
            ExitAction::InjectException { vector: 13, error_code: None },
            ExitAction::InjectException { vector: 14, error_code: None }
        );
    }
}
