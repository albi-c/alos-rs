use alloc::collections::LinkedList;
use modular_bitfield::prelude::*;
use crate::{debug, logger, volatile_struct};
use crate::lock::Lock;
use crate::memory::address::{PageCount, PhysAddr, VirtAddrPageAligned};

logger!("PCIe");

volatile_struct! { DeviceHeaderDataGeneral;
    bar: [u32; 6],
    _unused1: [u32; 3],
    capabilities: u8,
    _unused2: [u8; 7],
    interrupt_line: u8,
    interrupt_pin: u8,
    min_grant: u8,
    max_latency: u8,
}

impl DeviceHeaderDataGeneral {
    pub fn get_bar64(&self, index: usize) -> u64 {
        assert!(index < 5, "bar index out of bounds");
        let bar = self.bar.load();
        bar[index] as u64 | ((bar[index + 1] as u64) << 32)
    }
}

#[derive(Debug)]
pub enum DeviceHeaderData<'a> {
    General(&'a mut DeviceHeaderDataGeneral),
    BridgePci,
    BridgeCardBus,
}

#[bitfield]
#[derive(Debug, Copy, Clone)]
pub struct DeviceCommand {
    pub io: bool,
    pub memory: bool,
    pub bus_master: bool,
    pub special_cycles: bool,
    pub memory_write_invalidate: bool,
    pub vga_palette_snoop: bool,
    pub parity_error_response: bool,
    _reserved1: bool,
    pub serr_enable: bool,
    pub fast_back_to_back: bool,
    pub interrupt_disable: bool,
    _reserved2: B5,
}

volatile_struct! { DeviceHeader;
    vendor: u16,
    device: u16,
    command: DeviceCommand,
    status: u16,
    revision: u8,
    prog_if: u8,
    subclass: u8,
    class: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
    _data: (),
}

impl DeviceHeader {
    pub fn data(&mut self) -> DeviceHeaderData<'_> {
        let header_type = self.header_type.load();
        match header_type & 0x7f {
            0x0 => DeviceHeaderData::General(unsafe { &mut *self._data.as_mut_ptr().cast() }),
            0x1 => DeviceHeaderData::BridgePci,
            0x2 => DeviceHeaderData::BridgeCardBus,
            _ => panic!("Invalid PCI device header type: {:#x}", header_type),
        }
    }
}

pub struct Device {
    header: *mut DeviceHeader,
    data: DeviceHeaderData<'static>,
}

impl Device {
    pub fn header(&mut self) -> &mut DeviceHeader {
        unsafe { &mut *self.header }
    }
    pub fn data(&mut self) -> &mut DeviceHeaderData<'static> {
        &mut self.data
    }
}

fn device_header(base: VirtAddrPageAligned, bus: u8, device: u8, function: u8) -> &'static mut DeviceHeader {
    assert!(device < 32, "PCI device index out of range");
    assert!(function < 8, "PCI device function out of range");
    let addr = base + PageCount::new(
        ((bus as usize) << 8) + ((device as usize) << 3) + function as usize);
    unsafe { addr.as_mut() }
}

fn visit_function(devices: &mut LinkedList<Device>, header: &'static mut DeviceHeader) {
    debug!("found device - vendor: 0x{:04x}, device: 0x{:04x}, class: {:#x}, subclass: {:#x}, prog_if: {:#x}",
        header.vendor.load(), header.device.load(), header.class.load(), header.subclass.load(), header.prog_if.load());

    devices.push_back(Device {
        header: header as _,
        data: header.data(),
    })
}

fn visit_device(devices: &mut LinkedList<Device>, base: VirtAddrPageAligned, bus: u8, device: u8) {
    let header = device_header(base, bus, device, 0);

    if header.vendor.load() == 0xffff {
        return;
    }
    let header_type = header.header_type.load();
    visit_function(devices, header);

    if header_type & 0x80 != 0 {
        for function in 1..8 {
            let header = device_header(base, bus, device, function);
            if header.vendor.load() == 0xffff {
                continue;
            }
            visit_function(devices, header);
        }
    }
}

fn visit_bus(devices: &mut LinkedList<Device>, base: VirtAddrPageAligned, bus: u8) {
    for device in 0..32 {
        visit_device(devices, base, bus, device);
    }
}

static DEVICES: Lock<LinkedList<Device>> = Lock::new(LinkedList::new());

pub fn init(tables: &crate::acpi::Tables) {
    let regions = tables.get_pci_regions().expect("No PCI regions");

    let mut devices = LinkedList::new();
    for region in &regions.regions {
        if region.bus_number_end < region.bus_number_start {
            continue;
        }
        let virt = PhysAddr::new(region.base_address as usize).as_page_aligned()
            .expect("PCI region base must be page aligned").hhdm_to_virt();
        for bus in 0..=(region.bus_number_end - region.bus_number_start) {
            visit_bus(&mut devices, virt, bus);
        }
    }

    *DEVICES.write() = devices;
}

pub fn use_devices(mut filter: impl FnMut(&DeviceHeader) -> bool) -> LinkedList<Device> {
    let mut lock = DEVICES.write();
    let mut cur =  lock.cursor_front_mut();
    let mut result = LinkedList::new();
    while let Some(dev) = cur.current() {
        if filter(dev.header()) {
            let mut dev = cur.remove_current_as_list().unwrap();
            result.append(&mut dev);
        } else {
            cur.move_next();
        }
    }
    result
}
