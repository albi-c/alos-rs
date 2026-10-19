use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use modular_bitfield::prelude::*;
use uuid::Uuid;
use crate::{debug, driver, error, interrupts, logger, pcie, volatile_struct};
use crate::device::{uuid_from_device_name, BlockDevice, Device, DeviceError, device_seed_from_pci, BLOCK_DEVICE_REGISTRY, DeviceResult, DeviceTransfer, DeviceOperation, DeviceFuture, DeviceRef};
use crate::lock::{Lock, LockGuardWrite};
use crate::memory::address::{PageCount, PhysAddr, PhysAddrPageAligned};
use crate::memory::{hhdm, MemorySpace};
use crate::memory::mmio::{VolatileAccess, MMIO};
use crate::memory::slab::SLABS;
use crate::print::ByteSize64;
use crate::task::sched_yield;
use crate::volatile::Volatile;

logger!("virtio-blk");

const SECTOR_SHIFT: u32 = 9;
const SECTOR_SIZE: u64 = 1 << SECTOR_SHIFT;

#[bitfield]
#[derive(Debug, Copy, Clone)]
struct Status {
    acknowledged: bool,
    driver: bool,
    ready: bool,
    features_ok: bool,
    _reserved: B2,
    error: bool,
    failed: bool,
}

volatile_struct! { Registers;
    device_feature_select: u32,
    device_feature: u32,
    driver_feature_select: u32,
    driver_feature: u32,
    msix_config: u16,
    num_queues: u16,
    device_status: Status,
    config_generation: u8,

    queue_select: u16,
    queue_size: u16,
    queue_msix_vector: u16,
    queue_enable: u16,
    queue_notify_off: u16,
    queue_desc: u64,
    queue_avail: u64,
    queue_used: u64,
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct StorageRegisters {
    sector_count: u64,
    max_segment_size: u32,
    max_segment_count: u32,
    geometry_cylinders: u16,
    geometry_heads: u8,
    geometry_sectors: u8,
    block_size: u32,
}

#[derive(Debug)]
#[repr(C)]
struct CapabilityData {
    bar: u8,
    _padding: [u8; 3],
    offset: u32,
    length: u32,
}

bitflags! {
    #[derive(Debug, Copy, Clone, Pod, Zeroable)]
    #[repr(transparent)]
    struct QueueBufferFlags : u16 {
        const HAS_NEXT = 0x1;
        const WRITE = 0x2;
        const HAS_ADDRESSES = 0x4;
    }
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct QueueBuffer {
    addr: PhysAddr,
    size: u32,
    flags: QueueBufferFlags,
    next: u16,
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct QueueUsed {
    index: u32,
    length: u32,
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
struct RequestType(u32);

impl RequestType {
    const READ: Self = Self(0);
    const WRITE: Self = Self(1);
}

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct RequestHeader {
    type_: RequestType,
    _reserved1: u32,
    sector: u64,
}

#[derive(Debug)]
struct Queue {
    size_shift: u32,
    owned_phys: bool,
    buffers: MMIO,
    available: MMIO,
    used: MMIO,
    mmio_lock: Lock<()>,
    phys: [PhysAddrPageAligned; 3],
    buffer_slots: Box<[AtomicBool]>,
}

fn queue_data_sizes(size: u16) -> (u32, [PageCount; 3]) {
    assert!(size <= 32768, "virtio-blk: queue size out of bounds");
    assert!(size.is_power_of_two(), "virtio-blk: queue size is not a power of two");
    let size_shift = size.checked_ilog2().expect("virtio-blk: queue size is zero");
    (
        size_shift,
        [
            PageCount::pages_up(16 << size_shift),
            PageCount::pages_up(6 + (2 << size_shift)),
            PageCount::pages_up(6 + (8 << size_shift)),
        ],
    )
}

impl Queue {
    fn new_at(size: u16, owned_phys: bool, buffers: PhysAddrPageAligned,
              available: PhysAddrPageAligned, used: PhysAddrPageAligned) -> Self {
        let (size_shift, data_sizes) = queue_data_sizes(size);
        Self {
            size_shift,
            owned_phys,
            buffers: unsafe { MMIO::new(buffers, data_sizes[0], true, false) },
            available: unsafe { MMIO::new(available, data_sizes[1], true, false) },
            used: unsafe { MMIO::new(used, data_sizes[2], true, false) },
            mmio_lock: Lock::new(()),
            phys: [buffers, available, used],
            buffer_slots: Vec::from_fn(size as usize / 3, |_| AtomicBool::new(false)).into_boxed_slice(),
        }
    }

    fn new(size: u16) -> Self {
        let (_, data_sizes) = queue_data_sizes(size);
        let [buffers, available, used] = MemorySpace::with(|space| {
            data_sizes.map(|size| {
                let phys = space.phys_alloc(size)
                    .expect("virtio-blk: failed to allocate physical memory for queue");
                unsafe { core::ptr::write_bytes(hhdm::as_ptr::<u8>(phys.addr()), 0, size.size()) };
                phys
            })
        });
        Self::new_at(size, true, buffers, available, used)
    }

    fn from_regs(regs: &Registers) -> Self {
        Self::new_at(
            regs.queue_size.load(),
            false,
            PhysAddrPageAligned::new(regs.queue_desc.load() as usize)
                .expect("virtio-blk: queue buffer pointer not page aligned"),
            PhysAddrPageAligned::new(regs.queue_avail.load() as usize)
                .expect("virtio-blk: queue available pointer not page aligned"),
            PhysAddrPageAligned::new(regs.queue_used.load() as usize)
                .expect("virtio-blk: queue used pointer not page aligned"),
        )
    }

    fn to_regs(&self, regs: &mut Registers) {
        regs.queue_size.store(1 << self.size_shift);
        regs.queue_desc.store(self.phys[0].addr() as u64);
        regs.queue_avail.store(self.phys[1].addr() as u64);
        regs.queue_used.store(self.phys[2].addr() as u64);
    }

    fn buffer(&self, index: u16) -> &mut Volatile<QueueBuffer> {
        self.buffers.get_volatile(index as usize * size_of::<QueueBuffer>())
    }

    fn mask_index(&self, index: u16) -> u16 {
        index & ((1 << self.size_shift) - 1)
    }

    fn used_index(&self) -> &mut Volatile<u16> {
        self.used.get_volatile(2)
    }
    fn used_ring(&self, index: u16) -> &mut Volatile<QueueUsed> {
        self.used.get_volatile(4 + self.mask_index(index) as usize * size_of::<QueueUsed>())
    }

    fn allocate_buffer_group(&self) -> Option<BufferGroup> {
        for (i, group) in self.buffer_slots.iter().enumerate() {
            if !group.swap(true, Ordering::Acquire) {
                return Some(BufferGroup(i.try_into().unwrap()));
            }
        }
        None
    }
    fn free_buffer_group(&self, group: BufferGroup) {
        self.buffer_slots[group.0 as usize].store(false, Ordering::Release);
        core::mem::forget(group);
    }

    fn lock(&self) -> LockedQueue<'_> {
        let lock = self.mmio_lock.write();
        LockedQueue {
            queue: self,
            lock,
        }
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        if self.owned_phys {
            let (_, data_sizes) = queue_data_sizes(1 << self.size_shift);
            MemorySpace::with(|space| {
                self.phys.into_iter().zip(data_sizes).for_each(|(phys, size)| {
                    space.phys_dealloc(phys, size);
                });
            });
        }
    }
}

#[derive(Debug)]
struct LockedQueue<'a> {
    queue: &'a Queue,
    lock: LockGuardWrite<'a, ()>,
}

impl<'a> LockedQueue<'a> {
    fn available_index(&self) -> &mut Volatile<u16> {
        self.queue.available.get_volatile(2)
    }
    fn available_ring(&self, index: u16) -> &mut Volatile<u16> {
        self.queue.available.get_volatile(4 + self.queue.mask_index(index) as usize * size_of::<u16>())
    }
}

#[derive(Debug)]
struct BufferGroup(u16);

impl BufferGroup {
    fn index(&self) -> u16 {
        self.0
    }

    fn header(&self) -> u16 {
        self.0 * 3 + 0
    }
    fn data(&self) -> u16 {
        self.0 * 3 + 1
    }
    fn status(&self) -> u16 {
        self.0 * 3 + 2
    }
}

fn map_bar(data: &pcie::DeviceHeaderDataGeneral,
           (regs_bar, regs_offset, length): (u8, u32, u32), writable: bool) -> MMIO {
    data.map_bar(regs_bar as usize,
                 PageCount::try_pages(regs_offset as usize)
                     .expect("virtio-blk: BAR offset is not page aligned"),
                 PageCount::pages_up(length.min(0x10000) as usize), writable)
}

fn init() {
    let devices = pcie::use_devices(|device| {
        device.vendor.load() == 0x1af4 && device.device.load() == 0x1001
    });
    let mut id = 0;
    for mut dev in devices {
        if id >= 100 {
            error!("Device limit reached");
            break
        }

        let header = dev.header();
        debug!("Found device - vendor 0x{:04x}, device 0x{:04x}", header.vendor.load(), header.device.load());

        let mut bar_offsets: [Option<(u8, u32, u32)>; 5] = [None; 5];
        let mut notify_multiplier = None;

        for cap in header.capabilities() {
            match cap {
                pcie::DeviceCapability::VendorSpecific(cap) => {
                    let data = unsafe { cap.data_ptr().cast::<CapabilityData>().read() };
                    let ext = cap.extension();
                    let offset = (ext >> 8) - 1;
                    bar_offsets[offset as usize] = Some((data.bar, data.offset, data.length));
                    if offset == 1 {
                        notify_multiplier = Some(unsafe {
                            cap.data_ptr().cast::<u32>().byte_add(size_of::<CapabilityData>()).read() });
                    }
                },
                _ => {},
            }
        }

        let Some(notify_multiplier) = notify_multiplier else {
            error!("Missing notify multiplier");
            continue
        };

        let pcie::DeviceHeaderData::General(data) = header.data() else {
            error!("Invalid header data: {:x?}", header.data());
            continue
        };

        debug!("BAR: {:x?}, BAR offsets: {:x?}", data.bar.load(), bar_offsets);

        let Some(bar_offsets_0) = bar_offsets[0] else {
            error!("Missing bar offset [0]");
            continue
        };
        let Some(bar_offsets_1) = bar_offsets[1] else {
            error!("Missing bar offset [1]");
            continue
        };
        let Some(bar_offsets_2) = bar_offsets[2] else {
            error!("Missing bar offset [2]");
            continue
        };
        let Some(bar_offsets_3) = bar_offsets[3] else {
            error!("Missing bar offset [3]");
            continue
        };
        let mmio_regs = map_bar(data, bar_offsets_0, true);
        let mmio_storage_regs = map_bar(data, bar_offsets_3, false);
        let mmio_notification = map_bar(data, bar_offsets_1, true);
        let mmio_isr = map_bar(data, bar_offsets_2, false);

        let regs = mmio_regs.get_mut::<Registers>(0);

        if regs.num_queues.load() != 1 {
            error!("Invalid number of queues: {}", regs.num_queues.load());
            continue;
        }

        debug!("[#{id}] Acquired device id");

        // TODO: does not process requests when using new queue
        // let queue = Queue::new(0x100);
        // queue.to_regs(regs);

        let queue = Queue::from_regs(regs);

        if (1 << queue.size_shift) < 0x40 {
            error!("[#{id}] Default queue is too small: {}", 1 << queue.size_shift);
            continue;
        }

        let device = BlkDevice::new(
            id, queue, mmio_regs, mmio_storage_regs,
            mmio_notification, mmio_isr, notify_multiplier,
            data.interrupt_line.load());
        device.init_and_register();

        id += 1;
    }
}

#[derive(Debug)]
struct FutureSharedData {
    status: AtomicU32,
    status_ref: &'static Volatile<u8>,
    phys_status: PhysAddr,
    phys_header: PhysAddr,
}

#[derive(Debug)]
struct BlkDevice {
    id: u32,
    queue: Queue,
    mmio_regs: MMIO,
    mmio_storage_regs: MMIO,
    mmio_notification: MMIO,
    mmio_isr: MMIO,
    notify_multiplier: u32,
    name: [u8; 12],
    uuid: Uuid,
    interrupt_line: u8,
    futures: NonNull<[Option<Arc<FutureSharedData>>]>,
    used_index: AtomicU16,
    used_lock: Lock<()>,
}

impl BlkDevice {
    fn new(id: u32, queue: Queue, mmio_regs: MMIO, mmio_storage_regs: MMIO,
           mmio_notification: MMIO, mmio_isr: MMIO, notify_multiplier: u32,
           interrupt_line: u8) -> &'static mut Self {
        assert!(id < 100);
        let mut name = *b"virtio_blk00";
        name[10] = (id / 10) as u8 + b'0';
        name[11] = (id % 10) as u8 + b'0';
        Box::leak(Box::new(Self {
            futures: NonNull::from(Box::leak(Vec::from_fn(queue.buffer_slots.len(), |_| None).into_boxed_slice())),
            used_index: AtomicU16::new(queue.used_index().load()),
            id,
            queue,
            mmio_regs,
            mmio_storage_regs,
            mmio_notification,
            mmio_isr,
            notify_multiplier,
            name,
            uuid: uuid_from_device_name(device_seed_from_pci::<0x1af4, 0x1001>(id), &name),
            interrupt_line,
            used_lock: Lock::new(()),
        }))
    }

    fn init_and_register(self: DeviceRef<Self>) {
        let regs = self.regs();

        regs.queue_select.store(0);
        self.queue.to_regs(regs);

        regs.device_status.modify(|s| s.with_acknowledged(true));
        regs.device_status.modify(|s| s.with_driver(true));
        regs.device_status.modify(|s| s.with_features_ok(true));
        regs.device_status.modify(|s| s.with_ready(true));

        self.queue.available.store(0, 0u16);
        self.queue.used.store(0, 0u16);

        debug!("[#{}] Initialized device", self.id);
        let sector_count = self.storage_regs().sector_count;
        debug!("[#{}] Sector count: {}, Storage size: {}", self.id, sector_count, ByteSize64(sector_count * SECTOR_SIZE));

        interrupts::attach_irq(0x20 + self.interrupt_line, move |_ctx| {
            let status = self.mmio_isr.load::<u8>(0);
            if status & 0x1 != 0 {
                self.handle_responses()
            }
            if status & 0x2 != 0 {
                error!("#[{}] Configuration changed", self.id);
            }
        });

        BLOCK_DEVICE_REGISTRY.add(self);
    }

    fn futures(self: DeviceRef<Self>) -> &'static mut [Option<Arc<FutureSharedData>>] {
        unsafe { &mut *self.futures.as_ptr() }
    }

    fn regs(self: DeviceRef<Self>) -> &'static mut Registers {
        self.mmio_regs.get_mut::<Registers>(0)
    }
    fn storage_regs(self: DeviceRef<Self>) -> &'static StorageRegisters {
        self.mmio_storage_regs.get_ref::<StorageRegisters>(0)
    }

    fn notify_available(self: DeviceRef<Self>) {
        self.mmio_notification.store::<u16>(
            (self.regs().queue_notify_off.load() as u32 * self.notify_multiplier) as usize, 0u16);
    }

    fn check_read_bounds(self: DeviceRef<Self>, block: usize, size: usize) -> Result<u32, DeviceError> {
        let Ok(size): Result<u32, _> = size.try_into() else {
            return Err(DeviceError::DataSizeTooLarge);
        };
        let count = size >> SECTOR_SHIFT;
        if count << SECTOR_SHIFT != size {
            return Err(DeviceError::DataSizeUnaligned);
        }
        if block + (count as usize) > self.block_count() {
            return Err(DeviceError::OutOfBounds);
        }
        Ok(size)
    }

    fn submit_rw_request(self: DeviceRef<Self>, read: bool, block: usize, data: DeviceTransfer) -> DeviceOperation {
        let size = match self.check_read_bounds(block, data.size()) {
            Ok(size) => size,
            Err(err) => return Future::failed(data, err),
        };

        let (header, phys_header) = SLABS.allocate_one::<RequestHeader>();
        let (status, phys_status) = SLABS.allocate_one::<u8>();

        Volatile::from_ref(header).store(RequestHeader {
            type_: if read { RequestType::READ } else { RequestType::WRITE },
            _reserved1: 0,
            sector: block as u64,
        });

        let buf_group = loop {
            let group = self.queue.allocate_buffer_group();
            if let Some(group) = group {
                break group;
            }
            sched_yield();
        };

        let shared = Arc::new(FutureSharedData {
            status: AtomicU32::new(u32::MAX),
            status_ref: Volatile::from_ref(status),
            phys_status,
            phys_header,
        });
        assert!(self.futures()[buf_group.index() as usize].replace(shared.clone()).is_none(),
                "virtio-blk: Queue::allocate_buffer_group returned used index");

        self.queue.buffer(buf_group.header()).store(QueueBuffer {
            addr: phys_header,
            size: size_of::<RequestHeader>() as u32,
            flags: QueueBufferFlags::HAS_NEXT,
            next: 1,
        });
        self.queue.buffer(buf_group.data()).store(QueueBuffer {
            addr: data.phys(),
            size,
            flags: if read { QueueBufferFlags::WRITE } else { QueueBufferFlags::empty() } | QueueBufferFlags::HAS_NEXT,
            next: 2,
        });
        self.queue.buffer(buf_group.status()).store(QueueBuffer {
            addr: phys_status,
            size: size_of::<u8>() as u32,
            flags: QueueBufferFlags::WRITE,
            next: 0,
        });

        {
            let queue = self.queue.lock();
            let index = queue.available_index().load();
            queue.available_ring(index).store(buf_group.header());
            queue.available_index().store(index.wrapping_add(1));
        }

        self.notify_available();

        Future::ok(FutureOk {
            device: self,
            data,
            shared,
        })
    }

    fn handle_responses(self: DeviceRef<Self>) {
        let (mut start, end) = {
            let lock = self.used_lock.write();
            let start = self.used_index.load(Ordering::SeqCst);
            let end = self.queue.used_index().load();
            self.used_index.store(end, Ordering::SeqCst);
            drop(lock);
            (start, end)
        };

        while start != end {
            let used = self.queue.used_ring(start).load();
            self.handle_used(used);
            start = start.wrapping_add(1);
        }
    }

    fn handle_used(self: DeviceRef<Self>, used: QueueUsed) {
        let buf_group = BufferGroup((used.index / 3).try_into()
            .expect("virtio-blk: queue used buffer index too large"));

        let fut = &mut self.futures()[buf_group.index() as usize];
        match fut {
            Some(_) => {
                let shared = fut.take()
                    .expect("virtio-blk: shared data was taken");

                shared.status.store(shared.status_ref.load() as u32, Ordering::SeqCst);

                self.queue.free_buffer_group(buf_group);
            },
            None => {
                panic!("virtio-blk: missing status Arc for group with index {:#x}, base {:#x}",
                       buf_group.index(), buf_group.header());
            },
        }
    }
}

impl Device for BlkDevice {
    fn name(self: DeviceRef<Self>) -> &'static str {
        unsafe { core::str::from_utf8_unchecked(&self.name) }
    }
    fn uuid(self: DeviceRef<Self>) -> Uuid {
        self.uuid
    }
}

impl BlockDevice for BlkDevice {
    fn block_size_shift(self: DeviceRef<Self>) -> u32 {
        SECTOR_SHIFT
    }

    fn block_count(self: DeviceRef<Self>) -> usize {
        self.storage_regs().sector_count as usize
    }

    fn max_data_size(self: DeviceRef<Self>) -> usize {
        u32::MAX as usize
    }

    fn read(self: DeviceRef<Self>, block: usize, data: DeviceTransfer) -> DeviceOperation {
        self.submit_rw_request(true, block, data)
    }

    fn write(self: DeviceRef<Self>, _block: usize, data: DeviceTransfer) -> DeviceOperation {
        Future::failed(data, DeviceError::Unsupported)
    }
}

#[derive(Debug)]
struct FutureOk {
    device: DeviceRef<BlkDevice>,
    data: DeviceTransfer,
    shared: Arc<FutureSharedData>,
}

impl FutureOk {
    fn is_finished(&self) -> bool {
        self.shared.status.load(Ordering::SeqCst) != u32::MAX
    }

    fn to_result(self) -> DeviceResult {
        let status = self.shared.status.load(Ordering::SeqCst);
        assert_ne!(status, u32::MAX,
                   "virtio-blk: FutureOk::to_result called, but operation is not finished");

        SLABS.deallocate_physical_one::<RequestHeader>(self.shared.phys_header);
        SLABS.deallocate_physical_one::<u8>(self.shared.phys_status);

        let data_size = self.data.size();
        match status {
            0x0 => DeviceResult::ok(self.data, data_size),
            0x1 => DeviceResult::err(self.data, DeviceError::DeviceFault),
            0x2 => DeviceResult::err(self.data, DeviceError::Unsupported),
            _ => {
                error!("[#{}] unknown operation result status: {:#x}", self.device.id, status);
                DeviceResult::err(self.data, DeviceError::DeviceFault)
            },
        }
    }
}

#[derive(Debug)]
enum Future {
    Failed(DeviceTransfer, DeviceError),
    Waiting(FutureOk),
    Finished,
}

impl Future {
    fn failed(data: DeviceTransfer, err: DeviceError) -> DeviceOperation {
        Box::new(Self::Failed(data, err))
    }

    fn ok(ok: FutureOk) -> DeviceOperation {
        Box::new(Self::Waiting(ok))
    }
}

impl DeviceFuture for Future {
    fn poll(&mut self) -> Option<DeviceResult> {
        match self {
            Self::Failed(_, _) => {
                let Future::Failed(data, err) = core::mem::replace(self, Self::Finished) else {
                    unreachable!();
                };
                Some(DeviceResult::err(data, err))
            },
            Self::Waiting(fut) => {
                if !fut.is_finished() {
                    return None;
                }
                let Future::Waiting(fut) = core::mem::replace(self, Self::Finished) else {
                    unreachable!();
                };
                Some(fut.to_result())
            },
            Self::Finished => panic!("virtio-blk: Future::poll called after result returned"),
        }
    }
}

driver!("virtio-blk", init);
