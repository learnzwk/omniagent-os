# 设备驱动开发指南

> 本指南介绍 OmniAgent OS 的设备驱动开发框架，包括用户态驱动模型、驱动注册、中断处理以及 DMA 支持。

## 驱动框架概览

OmniAgent OS 采用用户态驱动模型，所有设备驱动作为独立 Agent 运行在用户空间。内核仅负责中断路由和基础硬件访问控制。

```
┌─────────────────────────────────────────────┐
│                  用户空间                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Serial   │  │ VirtIO   │  │ Network  │  │
│  │ Driver   │  │ BlkDrv   │  │ Driver   │  │
│  │ (Agent)  │  │ (Agent)  │  │ (Agent)  │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  │
│  ┌────┴──────────────┴──────────────┴────┐  │
│  │         omniagent-driver 库           │  │
│  └────────────────┬─────────────────────┘  │
├───────────────────┼────────────────────────┤
│  ┌────────────────┴─────────────────────┐  │
│  │         微内核（中断路由、I/O 控制）   │  │
│  └──────────────────────────────────────┘  │
├─────────────────────────────────────────────┤
│  UART 16550  │  VirtIO Block  │  NIC       │
└─────────────────────────────────────────────┘
```

### DeviceDriver Trait

所有驱动必须实现此 trait：

```rust
use omniagent_ipc::Message;
use omniagent_syscall::PhysicalAddress;

pub struct DriverCapability {
    pub io_ports: Vec<(u16, u16)>,
    pub mmio_regions: Vec<(PhysicalAddress, usize)>,
    pub irqs: Vec<u8>,
}

pub trait DeviceDriver: Send + Sync {
    fn name(&self) -> &str;
    fn device_type(&self) -> DeviceType;
    fn capabilities(&self) -> DriverCapability;
    fn probe(&mut self, bus: &dyn Bus) -> Result<bool, DriverError>;
    fn initialize(&mut self) -> Result<(), DriverError>;
    fn handle_interrupt(&mut self, irq: u8) -> Result<(), DriverError>;
    fn handle_message(&mut self, msg: &Message) -> Result<Message, DriverError>;
    fn shutdown(&mut self) -> Result<(), DriverError>;
}

#[derive(Debug, Clone, Copy)]
pub enum DeviceType { Serial, Block, Network, Display, Input, Custom(u32) }

#[derive(Debug)]
pub enum DriverError {
    DeviceNotFound, ResourceBusy, IoError(String),
    Timeout, UnsupportedDevice, PermissionDenied,
}
```

---

## 用户态驱动模型

### 驱动生命周期

```
注册 -> 探测 -> 初始化 -> 运行 -> 关闭
         │                  │
         └── 失败 -> 退出 ──┘
```

### 驱动 Agent 主循环

```rust
fn main() {
    let mut driver = MyDriver::new();
    let mut runtime = DriverRuntime::new(&driver).expect("Runtime creation failed");
    runtime.register().expect("Driver registration failed");
    runtime.run(&mut driver).expect("Driver error");
}
```

### DriverRuntime

```rust
pub struct DriverRuntime {
    port: Port,       // 客户端请求端口
    irq_port: Port,   // 中断通知端口
    driver_pid: u32,
}

impl DriverRuntime {
    pub fn new(driver: &dyn DeviceDriver) -> Result<Self, DriverError> {
        let port = Port::create(&format!("driver.{}", driver.name()))?;
        let irq_port = Port::create(&format!("driver.{}.irq", driver.name()))?;
        Ok(Self { port, irq_port, driver_pid: omniagent_syscall::getpid() })
    }

    pub fn run(&mut self, driver: &mut dyn DeviceDriver) -> Result<(), DriverError> {
        loop {
            match omniagent_ipc::select(&[&self.port, &self.irq_port]) {
                Ok(event) => match event.source() {
                    &self.port => {
                        let (msg, reply) = self.port.receive()?;
                        reply.send(&driver.handle_message(&msg)?)?;
                    }
                    &self.irq_port => {
                        let (msg, _) = self.irq_port.receive()?;
                        driver.handle_interrupt(msg.data::<u8>()?)?;
                    }
                    _ => unreachable!(),
                },
                Err(e) => return Err(DriverError::IoError(e.to_string())),
            }
        }
    }
}
```

---

## 驱动注册

```rust
pub fn register_driver(driver: &mut dyn DeviceDriver) -> Result<(), DriverError> {
    let caps = driver.capabilities();
    for (start, end) in &caps.io_ports { omniagent_syscall::request_io_port(*start, *end - *start)?; }
    for (phys_addr, size) in &caps.mmio_regions { omniagent_syscall::map_device_memory(*phys_addr, *size)?; }
    for irq in &caps.irqs { omniagent_syscall::register_irq(*irq)?; }
    let reg = DriverRegistration {
        name: driver.name().to_string(), device_type: driver.device_type(),
        pid: omniagent_syscall::getpid(), port_name: format!("driver.{}", driver.name()),
    };
    Port::connect("driver-manager")?.send(&Message::with_data("register", &reg))?;
    Ok(())
}
```

---

## ioctl 接口

```rust
pub mod ioctl {
    use omniagent_driver::IoctlCommand;
    pub const SERIAL_SET_BAUD: IoctlCommand = IoctlCommand::new(0x01);
    pub const BLK_GET_SIZE: IoctlCommand = IoctlCommand::new(0x10);
    pub const BLK_READ: IoctlCommand = IoctlCommand::new(0x12);
}

// 在 handle_message 中处理
fn handle_message(&mut self, msg: &Message) -> Result<Message, DriverError> {
    match msg.command() {
        "ioctl" => {
            let cmd: IoctlCommand = msg.data()?;
            match cmd {
                x if x == ioctl::SERIAL_SET_BAUD => {
                    self.set_baud_rate(serde::decode(msg.payload())?)?;
                    Ok(Message::ok())
                }
                _ => Err(DriverError::UnsupportedDevice),
            }
        }
        _ => Err(DriverError::IoError("Unknown command".into())),
    }
}
```

---

## 示例：串口驱动

```rust
use omniagent_driver::{DeviceDriver, DeviceType, DriverCapability, DriverError, IoPort};

pub struct SerialDriver { base_port: u16, baud_rate: u32 }

impl SerialDriver {
    pub fn new() -> Self { Self { base_port: 0x3F8, baud_rate: 115200 } }

    fn init_uart(&self) -> Result<(), DriverError> {
        let p = self.base_port;
        unsafe {
            IoPort::write_u8(p + 1, 0x00);       // 禁用中断
            IoPort::write_u8(p + 3, 0x80);       // 启用 DLAB
            let div = 115200 / self.baud_rate;
            IoPort::write_u8(p + 0, (div & 0xFF) as u8);
            IoPort::write_u8(p + 1, ((div >> 8) & 0xFF) as u8);
            IoPort::write_u8(p + 3, 0x03);       // 8N1
            IoPort::write_u8(p + 2, 0xC7);       // FIFO
            IoPort::write_u8(p + 4, 0x0B);       // IRQs + RTS/DSR
        }
        Ok(())
    }

    fn write_byte(&self, byte: u8) {
        unsafe { while IoPort::read_u8(self.base_port + 5) & 0x20 == 0 {} }
        unsafe { IoPort::write_u8(self.base_port, byte); }
    }

    fn read_byte(&self) -> Option<u8> {
        unsafe {
            if IoPort::read_u8(self.base_port + 5) & 0x01 != 0 {
                Some(IoPort::read_u8(self.base_port))
            } else { None }
        }
    }
}

impl DeviceDriver for SerialDriver {
    fn name(&self) -> &str { "serial-com1" }
    fn device_type(&self) -> DeviceType { DeviceType::Serial }
    fn capabilities(&self) -> DriverCapability {
        DriverCapability { io_ports: vec![(0x3F8, 0x400)], mmio_regions: vec![], irqs: vec![4] }
    }
    fn probe(&mut self, _: &dyn omniagent_driver::Bus) -> Result<bool, DriverError> {
        unsafe {
            IoPort::write_u8(self.base_port + 4, 0x10);
            IoPort::write_u8(self.base_port, 0xAE);
            let ok = IoPort::read_u8(self.base_port) == 0xAE;
            IoPort::write_u8(self.base_port + 4, 0x1F);
            Ok(ok)
        }
    }
    fn initialize(&mut self) -> Result<(), DriverError> { self.init_uart() }
    fn handle_interrupt(&mut self, _: u8) -> Result<(), DriverError> {
        while let Some(byte) = self.read_byte() { /* 处理接收数据 */ }
        Ok(())
    }
    fn handle_message(&mut self, msg: &Message) -> Result<Message, DriverError> {
        match msg.command() {
            "write" => { for &b in &msg.payload() { self.write_byte(b); } Ok(Message::ok()) }
            "read" => self.read_byte().map_or(
                Ok(Message::with_data("empty", &())),
                |b| Ok(Message::with_payload(vec![b])),
            ),
            _ => Err(DriverError::IoError("Unknown command".into())),
        }
    }
    fn shutdown(&mut self) -> Result<(), DriverError> {
        unsafe { IoPort::write_u8(self.base_port + 1, 0x00); } Ok(())
    }
}
```

---

## 示例：virtio-blk 驱动

```rust
pub struct VirtioBlkDriver { mmio_base: usize, capacity: u64, block_size: u32 }

impl VirtioBlkDriver {
    pub fn new(mmio_base: usize) -> Self { Self { mmio_base, capacity: 0, block_size: 512 } }
    unsafe fn read_reg(&self, offset: usize) -> u32 {
        core::ptr::read_volatile((self.mmio_base + offset) as *const u32)
    }
    unsafe fn write_reg(&self, offset: usize, val: u32) {
        core::ptr::write_volatile((self.mmio_base + offset) as *mut u32, val);
    }
}

impl DeviceDriver for VirtioBlkDriver {
    fn name(&self) -> &str { "virtio-blk" }
    fn device_type(&self) -> DeviceType { DeviceType::Block }
    fn capabilities(&self) -> DriverCapability {
        DriverCapability { io_ports: vec![], mmio_regions: vec![(self.mmio_base as u64, 0x1000)], irqs: vec![1] }
    }
    fn probe(&mut self, _: &dyn omniagent_driver::Bus) -> Result<bool, DriverError> {
        unsafe { Ok(self.read_reg(0x00) == 0x74726976 && self.read_reg(0x08) == 2) }
    }
    fn initialize(&mut self) -> Result<(), DriverError> {
        unsafe {
            self.write_reg(0x70, 0); self.write_reg(0x70, 1); self.write_reg(0x70, 3);
            self.write_reg(0x14, self.read_reg(0x10) & 0x01);
            self.write_reg(0x70, 7); self.write_reg(0x70, 15);
            let cfg = &*((self.mmio_base + 0x100) as *const VirtioBlkConfig);
            self.capacity = cfg.capacity * 512; self.block_size = cfg.blk_size;
        }
        Ok(())
    }
    fn handle_interrupt(&mut self, _: u8) -> Result<(), DriverError> { Ok(()) }
    fn handle_message(&mut self, msg: &Message) -> Result<Message, DriverError> {
        match msg.command() {
            "get_info" => Ok(Message::with_data("ok", &json!({
                "capacity": self.capacity, "block_size": self.block_size }))),
            _ => Err(DriverError::IoError("Unknown command".into())),
        }
    }
    fn shutdown(&mut self) -> Result<(), DriverError> { unsafe { self.write_reg(0x70, 0); } Ok(()) }
}
```

---

## 中断处理与 DMA

### 中断注册与处理

```rust
impl DriverRuntime {
    pub fn register_irq(&self, irq: u8) -> Result<(), DriverError> {
        let reg = IrqRegistration { irq, pid: self.driver_pid,
            port_name: format!("driver.{}.irq", self.port.name()) };
        Port::connect("irq-manager")?.send(&Message::with_data("register_irq", &reg))?;
        Ok(())
    }
}

// 最佳实践：ACK -> 读取状态 -> 处理 -> 重新启用
fn handle_interrupt(&mut self, irq: u8) -> Result<(), DriverError> {
    self.acknowledge_interrupt(irq)?;
    let status = self.read_status()?;
    if status & STATUS_RX_READY != 0 { self.handle_rx_ready()?; }
    if status & STATUS_ERROR != 0 { self.handle_error(status)?; }
    self.enable_interrupts(irq)?;
    Ok(())
}
```

### DMA 与共享内存

```rust
pub fn allocate_dma_buffer(size: usize) -> Result<DmaBuffer, DriverError> {
    let shm = shared_alloc(size, omniagent_syscall::SHM_DMA)?;
    Ok(DmaBuffer { phys_addr: shm.physical_address(), virt_addr: shm.as_ptr(), size, _shm: shm })
}

pub struct DmaBuffer { phys_addr: u64, virt_addr: *mut u8, size: usize, _shm: SharedMemory }

impl DmaBuffer {
    pub fn physical_address(&self) -> u64 { self.phys_addr }
    pub fn as_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.virt_addr, self.size) }
    }
}

// DMA 传输示例
pub fn dma_read(&self, sector: u64, count: u32) -> Result<Vec<u8>, DriverError> {
    let mut buf = allocate_dma_buffer((count as usize) * 512)?;
    unsafe {
        self.write_reg(REG_DMA_ADDRESS, buf.physical_address() as u32);
        self.write_reg(REG_DMA_SECTOR, sector as u32);
        self.write_reg(REG_DMA_CONTROL, DMA_START);
    }
    self.wait_dma_complete()?;
    Ok(buf.as_slice().to_vec())
}
```

---

## 驱动测试策略

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_driver_creation() {
        let driver = SerialDriver::new();
        assert_eq!(driver.name(), "serial-com1");
        assert_eq!(driver.device_type(), DeviceType::Serial);
    }
    #[test]
    fn test_baud_rate() { assert_eq!(115200 / SerialDriver::new().baud_rate, 1); }
}
```

## 热插拔与移植

```rust
#[derive(Serialize, Deserialize)]
pub enum HotplugEvent {
    DeviceAdded { bus_type: String, device_id: String, location: String },
    DeviceRemoved { device_id: String },
}

fn handle_message(&mut self, msg: &Message) -> Result<Message, DriverError> {
    match msg.command() {
        "hotplug" => {
            let event: HotplugEvent = msg.data()?;
            match event {
                HotplugEvent::DeviceAdded { device_id, .. } => klog!("Added: {}", device_id),
                HotplugEvent::DeviceRemoved { device_id } => klog!("Removed: {}", device_id),
            }
            Ok(Message::ok())
        }
        _ => self.handle_device_message(msg),
    }
}
```

### 移植 Linux 驱动

| 方面 | Linux 内核驱动 | OmniAgent OS 驱动 |
|------|---------------|-------------------|
| 运行空间 | 内核空间 | 用户空间 |
| 中断处理 | 注册处理函数 | IPC 接收通知 |
| 并发控制 | 自旋锁、互斥锁 | IPC 消息序列化 |
| DMA | dma_alloc_coherent | shared_alloc(SHM_DMA) |
**移植步骤**：1) 分析 Linux 驱动初始化流程 2) 创建 Agent 项目实现 `DeviceDriver` trait 3) 替换内核 API 4) 适配中断模型 5) 编写测试 6) 性能优化
**注意事项**：用户态驱动无法直接访问内核数据结构；中断延迟可能更高；需正确声明能力（最小权限原则）；驱动崩溃不会导致内核崩溃，但需自动恢复
