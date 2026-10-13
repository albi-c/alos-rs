use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;
use modular_bitfield::prelude::*;
use crate::{debug, driver, error, logger, memory, pcie, println, volatile_struct};
use crate::memory::address::{PageCount, PhysAddrPageAligned};
use crate::memory::{hhdm, MemorySpace, MemoryFlags};

logger!("NVMe");

#[bitfield]
#[derive(Debug, Copy, Clone)]
struct ControllerCapabilities {
    max_queue_entries: B16,
    contiguous_queues: bool,
    arbitration_mechanism: B2,
    _reserved1: B5,
    timeout: B8,
    doorbell_stride: B4,
    subsystem_reset: bool,
    command_sets: B8,
    boot_partition: bool,
    _reserved2: B2,
    page_size_min: B4,
    page_size_max: B4,
    _reserved3:  B8
}

impl ControllerCapabilities {
    fn get_max_queue_entries(&self) -> usize {
        self.max_queue_entries() as usize + 1
    }
}

#[bitfield]
#[derive(Debug, Copy, Clone)]
struct ControllerConfig {
    enable: bool,
    _reserved1: B3,
    command_set: B3,
    memory_page_size: B4,
    arbitration_mechanism: B3,
    shutdown_notification: B2,
    sq_entry_size: B4,
    cq_entry_size: B4,
    _reserved2: B8,
}

#[bitfield]
#[derive(Debug, Copy, Clone)]
struct ControllerStatus {
    ready: bool,
    fatal: bool,
    shutdown: B2,
    subsystem_reset: bool,
    paused: bool,
    _reserved1: B26,
}

#[bitfield]
#[derive(Debug, Copy, Clone)]
struct AdminQueueAttributes {
    sq_size: B12,
    _reserved1: B4,
    cq_size: B12,
    _reserved2: B4,
}

volatile_struct! { ControllerRegisters;
    capabilities: ControllerCapabilities,
    version: u32,
    int_mask_set: u32,
    int_mask_clear: u32,
    config: ControllerConfig,
    status: ControllerStatus,
    admin_queue_attributes: AdminQueueAttributes,
    admin_sq: u64,
    admin_cq: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct SubmissionEntry {
    pub opcode: u8,
    // lower 2 bits select fused operation, upper 2 bits select prp/sgl
    pub fused_prp: u8,
    pub command_id: u16,
    pub namespace: u32,
    pub _reserved1: [u32; 2],
    pub metadata: u64,
    pub data: [u64; 2],
    pub command_data: [u32; 6],
}

impl SubmissionEntry {
    const FLAG_CONTIGUOUS: u32 = 0x1;

    fn cmd_create_sq(command_id: u16, queue: &Queue<SubmissionEntry>, queue_id: u16, cq_id: u16) -> Self {
        let size = queue.size;
        assert!(size > 0, "queue cannot be empty");
        let size: u16 = (size - 1).try_into().expect("queue size must fit into 16 bits");
        Self {
            opcode: 0x01,
            command_id,
            data: [queue.addr.addr() as u64, 0],
            command_data: [
                queue_id as u32 | ((size as u32) << 16),
                Self::FLAG_CONTIGUOUS | ((cq_id as u32) << 16),
                0, 0, 0, 0,
            ],
            ..Default::default()
        }
    }

    fn cmd_create_cq(command_id: u16, queue: &Queue<CompletionEntry>, queue_id: u16, vector: u16) -> Self {
        let size = queue.size;
        assert!(size > 0, "queue cannot be empty");
        let size: u16 = (size - 1).try_into().expect("queue size must fit into 16 bits");
        Self {
            opcode: 0x05,
            command_id,
            data: [queue.addr.addr() as u64, 0],
            command_data: [
                queue_id as u32 | ((size as u32) << 16),
                Self::FLAG_CONTIGUOUS | ((vector as u32) << 16),
                0, 0, 0, 0,
            ],
            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CompletionEntry {
    pub command_data: u32,
    pub _reserved1: u32,
    pub sq_head: u16,
    pub sq_id: u16,
    pub command_id: u16,
    // low bit is phase, the rest is status
    pub phase_status: u16,
}

fn init() {
    let devices = pcie::use_devices(|dev| {
        dev.class.load() == 0x1 && dev.subclass.load() == 0x8 && dev.prog_if.load() == 0x2
    });
    for mut dev in devices {
        let header = dev.header();
        debug!("found device - vendor 0x{:04x}, device 0x{:04x}", header.vendor.load(), header.device.load());
        let device = Device::new(dev);
        if let Some(_device) = device {
            //
        }
    }
}

struct Device {
    dev: pcie::Device,
    admin: QueuePair,
    registers: &'static mut ControllerRegisters,
    doorbells: NonNull<u32>,
}

impl Device {
    fn new(mut dev: pcie::Device) -> Option<Self> {
        let header = dev.header();

        header.command.store(header.command.load()
            .with_memory(true)
            .with_bus_master(true)
            .with_interrupt_disable(false));

        let header_data = match header.data() {
            pcie::DeviceHeaderData::General(data) => data,
            _ => {
                error!("invalid header data - {:#?}", header.data());
                return None;
            },
        };

        let bar = header_data.get_bar64(0);
        let bar_addr = bar & !0xf;
        let mapped = MemorySpace::with(|space| {
            let count = PageCount::new(16);
            let virt = space.virt_alloc(count);
            space.map(PhysAddrPageAligned::new(bar_addr as usize)
                          .expect("nvme: pcie bar is not page aligned"),
                      virt, count, MemoryFlags::WRITE | MemoryFlags::NO_EXEC | MemoryFlags::NO_CACHE);
            NonNull::new(virt.addr() as *mut u8).unwrap()
        });
        let registers = unsafe { mapped.cast::<ControllerRegisters>().as_mut() };

        println!("registers: {:#x?}", registers);

        registers.config.store(registers.config.load().with_enable(false));

        let admin = QueuePair::new_admin(registers);
        let doorbells = unsafe { mapped.byte_add(0x1000).cast() };

        registers.config.store(registers.config.load().with_enable(true));

        Some(Self {
            dev,
            admin,
            registers,
            doorbells,
        })
    }
}

struct QueuePair {
    submission: Queue<SubmissionEntry>,
    completion: Queue<CompletionEntry>,
}

impl QueuePair {
    fn new_admin(registers: &mut ControllerRegisters) -> Self {
        let sq = Queue::new(64);
        let cq = Queue::new(64);

        println!("registers: {:#x?}", registers);

        println!("sq {:#x?}, cq: {:#x?}", sq.addr.addr(), cq.addr.addr());

        registers.admin_sq.store(sq.addr.addr() as u64);
        registers.admin_cq.store(cq.addr.addr() as u64);
        registers.admin_queue_attributes.store(AdminQueueAttributes::new()
            .with_sq_size(64)
            .with_cq_size(64));

        println!("registers: {:#x?}", registers);

        Self {
            submission: sq,
            completion: cq,
        }
    }
}

struct Queue<T: Copy + 'static> {
    addr: PhysAddrPageAligned,
    size: u32,
    doorbell: u32,
    _pd: PhantomData<T>,
}

impl<T: Copy + 'static> Queue<T> {
    fn new(size: usize) -> Self {
        assert!(size > 0, "queue cannot be empty");
        let addr = memory::alloc_pages_zeroed(PageCount::pages_up(size * size_of::<T>()))
            .expect("nvme: failed to allocate memory for admin queue");
        Self {
            addr,
            size: size.try_into().expect("queue size must fit into 32 bits"),
            doorbell: 0,
            _pd: PhantomData,
        }
    }
}

impl<T: Copy + 'static> Index<usize> for Queue<T> {
    type Output = SubmissionEntry;

    fn index(&self, index: usize) -> &SubmissionEntry {
        unsafe { hhdm::as_slice(self.addr.addr(), self.size as usize) }.get(index).unwrap()
    }
}
impl<T: Copy + 'static> IndexMut<usize> for Queue<T> {
    fn index_mut(&mut self, index: usize) -> &mut SubmissionEntry {
        unsafe { hhdm::as_mut_slice(self.addr.addr(), self.size as usize) }.get_mut(index).unwrap()
    }
}

driver!("nvme", init);
