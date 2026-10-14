use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};
use modular_bitfield::prelude::*;
use crate::{debug, driver, error, logger, pcie, volatile_struct};
use crate::memory::address::{PageCount, PhysAddr, PhysAddrPageAligned};
use crate::memory::{hhdm, MemorySpace};
use crate::memory::mmio::{VolatileAccess, MMIO};
use crate::print::ByteSize64;

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

// TODO: Drop implementation for owned queues
#[derive(Debug)]
struct Queue {
    size_shift: u32,
    owned: bool,
    buffers: MMIO,
    available: MMIO,
    used: MMIO,
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
    fn new_at(size: u16, owned: bool, buffers: PhysAddrPageAligned,
              available: PhysAddrPageAligned, used: PhysAddrPageAligned) -> Self {
        let (size_shift, data_sizes) = queue_data_sizes(size);
        Self {
            size_shift,
            owned,
            buffers: unsafe { MMIO::new(buffers, data_sizes[0], true, false) },
            available: unsafe { MMIO::new(available, data_sizes[1], true, false) },
            used: unsafe { MMIO::new(used, data_sizes[2], false, false) },
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
        regs.queue_desc.store(self.buffers.ptr().as_ptr() as u64);
        regs.queue_avail.store(self.available.ptr().as_ptr() as u64);
        regs.queue_used.store(self.used.ptr().as_ptr() as u64);
    }
}

fn map_bar(data: &pcie::DeviceHeaderDataGeneral, (regs_bar, regs_offset): (u8, u32), writable: bool) -> MMIO {
    data.map_bar(regs_bar as usize, PageCount::try_pages(regs_offset as usize)
        .expect("virtio-blk: BAR offset is not page aligned"), 1.into(), writable)
}

fn init() {
    let devices = pcie::use_devices(|device| {
        device.vendor.load() == 0x1af4 && device.device.load() == 0x1001
    });
    let mut id = 0;
    for mut dev in devices {
        let header = dev.header();
        debug!("Found device - vendor 0x{:04x}, device 0x{:04x}", header.vendor.load(), header.device.load());

        let mut bar_offsets: [Option<(u8, u32)>; 5] = [None; 5];

        for cap in header.capabilities() {
            match cap {
                pcie::DeviceCapability::VendorSpecific(cap) => {
                    let data = unsafe { cap.data_ptr().cast::<CapabilityData>().read() };
                    let ext = cap.extension();
                    bar_offsets[((ext >> 8) - 1) as usize] = Some((data.bar, data.offset));
                },
                _ => {},
            }
        }

        let pcie::DeviceHeaderData::General(data) = header.data() else {
            error!("Invalid header data: {:x?}", header.data());
            continue
        };

        let Some(bar_offsets_0) = bar_offsets[0] else {
            error!("Missing bar offset [0]");
            continue
        };
        let Some(bar_offsets_3) = bar_offsets[3] else {
            error!("Missing bar offset [3]");
            continue
        };
        let mmio_regs = map_bar(data, bar_offsets_0, true);
        let mmio_storage_regs = map_bar(data, bar_offsets_3, false);

        let regs = mmio_regs.get_mut::<Registers>(0);

        if regs.num_queues.load() != 1 {
            error!("Invalid number of queues: {}", regs.num_queues.load());
            continue;
        }

        debug!("[#{id}] Acquired device id");

        // TODO: crashes when queue size is over 0x400
        let queue = Queue::new(0x400);

        let device = Device::new(id, queue, mmio_regs, mmio_storage_regs);
        device.init();

        id += 1;
    }
}

#[derive(Debug)]
struct Device {
    id: u32,
    queue: Queue,
    mmio_regs: MMIO,
    mmio_storage_regs: MMIO,
}

impl Device {
    fn new(id: u32, queue: Queue, mmio_regs: MMIO, mmio_storage_regs: MMIO) -> Self {
        Self {
            id,
            queue,
            mmio_regs,
            mmio_storage_regs,
        }
    }

    fn init(&self) {
        let regs = self.regs();

        regs.queue_select.store(0);
        self.queue.to_regs(regs);

        regs.device_status.store(regs.device_status.load()
            .with_acknowledged(true));
        regs.device_status.store(regs.device_status.load()
            .with_driver(true));
        regs.device_status.store(regs.device_status.load()
            .with_features_ok(true));
        regs.device_status.store(regs.device_status.load()
            .with_ready(true));

        debug!("[#{}] Initialized device", self.id);
        let sector_count = self.storage_regs().sector_count;
        debug!("[#{}] Sector count: {}, Storage size: {}", self.id, sector_count, ByteSize64(sector_count * SECTOR_SIZE));
    }

    fn regs(&self) -> &mut Registers {
        self.mmio_regs.get_mut::<Registers>(0)
    }
    fn storage_regs(&self) -> &StorageRegisters {
        self.mmio_storage_regs.get_ref::<StorageRegisters>(0)
    }
}

driver!("virtio-blk", init);
