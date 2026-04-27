// vCPU 管理
//
// 定义 vCPU 状态、客户机寄存器和 vCPU 结构

/// vCPU 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VcpuState {
    /// 空闲
    Idle = 0,
    /// 运行中
    Running = 1,
    /// 阻塞
    Blocked = 2,
    /// 停止
    Halted = 3,
}

/// 客户机寄存器
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GuestRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
}

impl Default for GuestRegisters {
    fn default() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
            rflags: 0x202, // 默认 IF 标志位
            cr0: 0,
            cr2: 0,
            cr3: 0,
            cr4: 0,
        }
    }
}

impl GuestRegisters {
    /// 获取通用寄存器值
    /// index 0-15 对应 rax, rbx, rcx, rdx, rsi, rdi, rbp, rsp, r8-r15
    pub fn get_reg(&self, index: usize) -> u64 {
        match index {
            0 => self.rax,
            1 => self.rbx,
            2 => self.rcx,
            3 => self.rdx,
            4 => self.rsi,
            5 => self.rdi,
            6 => self.rbp,
            7 => self.rsp,
            8 => self.r8,
            9 => self.r9,
            10 => self.r10,
            11 => self.r11,
            12 => self.r12,
            13 => self.r13,
            14 => self.r14,
            15 => self.r15,
            _ => 0,
        }
    }

    /// 设置通用寄存器值
    /// index 0-15 对应 rax, rbx, rcx, rdx, rsi, rdi, rbp, rsp, r8-r15
    pub fn set_reg(&mut self, index: usize, value: u64) {
        match index {
            0 => self.rax = value,
            1 => self.rbx = value,
            2 => self.rcx = value,
            3 => self.rdx = value,
            4 => self.rsi = value,
            5 => self.rdi = value,
            6 => self.rbp = value,
            7 => self.rsp = value,
            8 => self.r8 = value,
            9 => self.r9 = value,
            10 => self.r10 = value,
            11 => self.r11 = value,
            12 => self.r12 = value,
            13 => self.r13 = value,
            14 => self.r14 = value,
            15 => self.r15 = value,
            _ => {}
        }
    }
}

/// vCPU
pub struct Vcpu {
    /// vCPU ID
    pub id: u32,
    /// vCPU 状态
    pub state: VcpuState,
    /// 客户机寄存器
    pub registers: GuestRegisters,
    /// VM Exit 计数
    pub exit_count: u64,
    /// CPU 时间（纳秒）
    pub cpu_time_ns: u64,
}

impl Vcpu {
    /// 创建新的 vCPU
    pub fn new(id: u32) -> Self {
        Self {
            id,
            state: VcpuState::Idle,
            registers: GuestRegisters::default(),
            exit_count: 0,
            cpu_time_ns: 0,
        }
    }

    /// 保存寄存器状态
    pub fn save_registers(&mut self, regs: &GuestRegisters) {
        self.registers = *regs;
    }

    /// 恢复寄存器状态
    pub fn restore_registers(&self) -> GuestRegisters {
        self.registers
    }

    /// 记录 VM Exit
    pub fn record_exit(&mut self) {
        self.exit_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcpu_new() {
        let vcpu = Vcpu::new(0);
        assert_eq!(vcpu.id, 0);
        assert_eq!(vcpu.state, VcpuState::Idle);
        assert_eq!(vcpu.exit_count, 0);
        assert_eq!(vcpu.cpu_time_ns, 0);
    }

    #[test]
    fn test_vcpu_save_restore_registers() {
        let mut vcpu = Vcpu::new(0);

        let mut regs = GuestRegisters::default();
        regs.rax = 0xDEADBEEF;
        regs.rbx = 0xCAFEBABE;
        regs.rip = 0x1000;

        vcpu.save_registers(&regs);

        let restored = vcpu.restore_registers();
        assert_eq!(restored.rax, 0xDEADBEEF);
        assert_eq!(restored.rbx, 0xCAFEBABE);
        assert_eq!(restored.rip, 0x1000);
    }

    #[test]
    fn test_vcpu_record_exit() {
        let mut vcpu = Vcpu::new(0);
        assert_eq!(vcpu.exit_count, 0);

        vcpu.record_exit();
        assert_eq!(vcpu.exit_count, 1);

        vcpu.record_exit();
        vcpu.record_exit();
        assert_eq!(vcpu.exit_count, 3);
    }

    #[test]
    fn test_guest_registers_default() {
        let regs = GuestRegisters::default();
        assert_eq!(regs.rax, 0);
        assert_eq!(regs.rflags, 0x202);
        assert_eq!(regs.cr0, 0);
    }

    #[test]
    fn test_guest_registers_get_reg() {
        let mut regs = GuestRegisters::default();
        regs.rax = 100;
        regs.rdi = 200;
        regs.r15 = 300;

        assert_eq!(regs.get_reg(0), 100);  // rax
        assert_eq!(regs.get_reg(5), 200);  // rdi
        assert_eq!(regs.get_reg(15), 300); // r15
        assert_eq!(regs.get_reg(1), 0);    // rbx（未设置）
        assert_eq!(regs.get_reg(20), 0);   // 超出范围
    }

    #[test]
    fn test_guest_registers_set_reg() {
        let mut regs = GuestRegisters::default();

        regs.set_reg(0, 42);   // rax
        regs.set_reg(7, 0x7FFF); // rsp
        regs.set_reg(10, 999);  // r10

        assert_eq!(regs.rax, 42);
        assert_eq!(regs.rsp, 0x7FFF);
        assert_eq!(regs.r10, 999);

        // 超出范围的设置不产生效果
        regs.set_reg(20, 12345);
        assert_eq!(regs.get_reg(20), 0);
    }

    #[test]
    fn test_vcpu_state_repr() {
        assert_eq!(VcpuState::Idle as u8, 0);
        assert_eq!(VcpuState::Running as u8, 1);
        assert_eq!(VcpuState::Blocked as u8, 2);
        assert_eq!(VcpuState::Halted as u8, 3);
    }
}
