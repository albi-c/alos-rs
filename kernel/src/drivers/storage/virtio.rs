use modular_bitfield::prelude::*;
use crate::{debug, driver, logger, pcie, volatile_struct};
use crate::memory::address::PhysAddrPageAligned;
use crate::memory::{MemoryFlags, MemorySpace};

logger!("virtio-blk");

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

volatile_struct! { StorageRegisters;
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

fn map_bar<T>(data: &pcie::DeviceHeaderDataGeneral, (regs_bar, regs_offset): (u8, u32)) -> &'static mut T {
    let bar = data.get_bar64(regs_bar as usize) as usize;
    let bar_addr = (bar & !0xf) + regs_offset as usize;
    MemorySpace::with(|space| {
        let virt = space.virt_alloc(1.into());
        space.map(PhysAddrPageAligned::new(bar_addr)
                      .expect("virtio-blk: physical address is not page aligned"),
                  virt, 1.into(), MemoryFlags::WRITE | MemoryFlags::NO_CACHE | MemoryFlags::NO_EXEC);
        unsafe { virt.as_mut::<T>() }
    })
}

fn init() {
    let devices = pcie::use_devices(|device| {
        device.vendor.load() == 0x1af4 && device.device.load() == 0x1001
    });
    for mut dev in devices {
        let header = dev.header();
        debug!("found device - vendor 0x{:04x}, device 0x{:04x}", header.vendor.load(), header.device.load());

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

        debug!("BAR Offsets: {:x?}", bar_offsets);

        let pcie::DeviceHeaderData::General(data) = header.data() else { continue };
        debug!("BAR: {:x?}", data.bar.load());

        let regs = map_bar::<Registers>(
            data, bar_offsets[0].expect("nvme: no bar offset [0]"));
        let storage_regs = map_bar::<StorageRegisters>(
            data, bar_offsets[3].expect("nvme: no bar offset [3]"));

        regs.device_status.store(regs.device_status.load()
            .with_acknowledged(true));
        regs.device_status.store(regs.device_status.load()
            .with_driver(true));
        regs.device_status.store(regs.device_status.load()
            .with_features_ok(true));
        regs.device_status.store(regs.device_status.load()
            .with_ready(true));

        debug!("Registers: {:#x?}", regs);
        debug!("Storage Registers: {:#x?}", storage_regs);
    }
}

driver!("virtio-blk", init);
