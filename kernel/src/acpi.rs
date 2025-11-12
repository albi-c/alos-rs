use core::cmp::max;
use acpi::{AcpiTables, Handle, Handler, PciAddress, PhysicalMapping};
use acpi::aml::AmlError;
use acpi::platform::PciConfigRegions;
use limine::request::RsdpRequest;
use crate::memory::hhdm;
use crate::ports::Port;

#[used]
#[unsafe(link_section = ".requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

pub fn init() {
    let response = RSDP_REQUEST.get_response().expect("No RSDP address");
    let tables = unsafe {
        AcpiTables::from_rsdp(MemoryHandler, response.address()) }.expect("Invalid ACPI tables");
    let pci_conf = PciConfigRegions::new(&tables).expect("No PCI config regions");
    for region in pci_conf.regions {
    }
}

#[derive(Debug, Copy, Clone)]
struct MemoryHandler;

impl Handler for MemoryHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        let length = max(size_of::<T>(), size);
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: hhdm::as_non_null(physical_address),
            region_length: length,
            mapped_length: length,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}

    fn read_u8(&self, address: usize) -> u8 {
        unsafe { *hhdm::as_ptr(address) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        unsafe { *hhdm::as_ptr(address) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        unsafe { *hhdm::as_ptr(address) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        unsafe { *hhdm::as_ptr(address) }
    }

    fn write_u8(&self, address: usize, value: u8) {
        unsafe { *hhdm::as_ptr(address) = value };
    }

    fn write_u16(&self, address: usize, value: u16) {
        unsafe { *hhdm::as_ptr(address) = value };
    }

    fn write_u32(&self, address: usize, value: u32) {
        unsafe { *hhdm::as_ptr(address) = value };
    }

    fn write_u64(&self, address: usize, value: u64) {
        unsafe { *hhdm::as_ptr(address) = value };
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { Port::new(port, 1) }.in_b(0)
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { Port::new(port, 2) }.in_w(0)
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { Port::new(port, 4) }.in_d(0)
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { Port::new(port, 1) }.out_b(0, value)
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { Port::new(port, 2) }.out_w(0, value)
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { Port::new(port, 4) }.out_d(0, value)
    }

    fn read_pci_u8(&self, _address: PciAddress, _offset: u16) -> u8 {
        unimplemented!("MemoryHandler: read_pci_u8")
    }

    fn read_pci_u16(&self, _address: PciAddress, _offset: u16) -> u16 {
        unimplemented!("MemoryHandler: read_pci_u16")
    }

    fn read_pci_u32(&self, _address: PciAddress, _offset: u16) -> u32 {
        unimplemented!("MemoryHandler: read_pci_u32")
    }

    fn write_pci_u8(&self, _address: PciAddress, _offset: u16, _value: u8) {
        unimplemented!("MemoryHandler: write_pci_u8")
    }

    fn write_pci_u16(&self, _address: PciAddress, _offset: u16, _value: u16) {
        unimplemented!("MemoryHandler: write_pci_u16")
    }

    fn write_pci_u32(&self, _address: PciAddress, _offset: u16, _value: u32) {
        unimplemented!("MemoryHandler: write_pci_u32")
    }

    fn nanos_since_boot(&self) -> u64 {
        unimplemented!("MemoryHandler: nanos_since_boot")
    }

    fn stall(&self, _microseconds: u64) {
        unimplemented!("MemoryHandler: stall")
    }

    fn sleep(&self, _milliseconds: u64) {
        unimplemented!("MemoryHandler: sleep")
    }

    fn create_mutex(&self) -> Handle {
        Handle(0)
    }

    fn acquire(&self, _mutex: Handle, _timeout: u16) -> Result<(), AmlError> {
        Ok(())
    }

    fn release(&self, _mutex: Handle) {}
}
