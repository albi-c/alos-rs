use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::fmt::Debug;
use core::ptr::NonNull;
use uuid::Uuid;
use crate::lock::Lock;
use crate::{debug, logger};
use crate::memory::address::PhysAddr;
use crate::memory::slab::SLABS;

logger!("Device registry");

pub fn device_seed_from_pci<const VENDOR: u16, const DEVICE: u16>(offset: u32) -> u64 {
    const PCI: [u8; 4] = [b'p', b'c', b'i', b' '];
    ((((u32::from_le_bytes(PCI) ^ offset) as u64) << 32)
        | ((VENDOR as u64) << 16) | (DEVICE as u64)) ^ (offset as u64)
}

pub fn uuid_from_device_name(seed: u64, name: &[u8]) -> Uuid {
    Uuid::from_u128(twox_hash::XxHash3_128::oneshot_with_seed(seed, name))
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeviceError {
    Unsupported,
    OutOfBounds,
    DataSizeUnaligned,
    DataSizeTooLarge,
    DeviceFault,
}

pub struct DeviceRegistry<T: Device + ?Sized> {
    name: &'static str,
    devices: Lock<BTreeMap<Uuid, Arc<T>>>,
}

impl<T: Device + ?Sized> DeviceRegistry<T> {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            devices: Lock::new(BTreeMap::new()),
        }
    }

    pub fn add(&self, device: Arc<T>) {
        match self.devices.write().entry(device.uuid()) {
            Entry::Vacant(entry) => {
                let device = entry.insert(device);
                debug!("[{}] added device '{}' with uuid {}", self.name, device.name(), device.uuid());
            },
            Entry::Occupied(entry) => {
                panic!("DeviceRegistry::add: device with uuid {} already present - old name '{}', new name '{}'",
                       device.uuid(), entry.get().name(), device.name());
            },
        }
    }

    pub fn get(&self, uuid: Uuid) -> Option<Arc<T>> {
        self.devices.read().get(&uuid).cloned()
    }

    pub fn with_iter<R>(&self, mut func: impl FnMut(Uuid, &Arc<T>) -> Option<R>) -> Option<R> {
        for (&uuid, dev) in self.devices.read().iter() {
            if let Some(result) = func(uuid, dev) {
                return Some(result);
            }
        }
        None
    }

    pub fn find(&self, mut func: impl FnMut(&T) -> bool) -> Option<Arc<T>> {
        self.with_iter(|_, dev| func(dev).then(|| dev.clone()))
    }
}

pub static BLOCK_DEVICE_REGISTRY: DeviceRegistry<dyn BlockDevice> = DeviceRegistry::new("block");
pub static CHARACTER_DEVICE_REGISTRY: DeviceRegistry<dyn CharacterDevice> = DeviceRegistry::new("character");

#[derive(Debug)]
pub struct DeviceTransfer {
    phys: PhysAddr,
    data: NonNull<u8>,
    size: usize,
}

impl DeviceTransfer {
    unsafe fn new_uninitialized(size: usize) -> Self {
        let (data, phys) = SLABS.allocate(size);
        Self {
            phys,
            data,
            size,
        }
    }

    pub fn new(size: usize) -> Self {
        let mut transfer = unsafe { Self::new_uninitialized(size) };
        transfer.data_mut().into_iter().for_each(|x| *x = 0u8);
        transfer
    }

    pub fn new_with_data(data: &[u8]) -> Self {
        let mut transfer = unsafe { Self::new_uninitialized(data.len()) };
        transfer.data_mut().copy_from_slice(data);
        transfer
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn data(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.data.as_ptr(), self.size) }
    }
    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.data.as_ptr(), self.size) }
    }

    pub fn phys(&self) -> PhysAddr {
        self.phys
    }
}

impl Drop for DeviceTransfer {
    fn drop(&mut self) {
        SLABS.deallocate_physical(self.size, self.phys);
    }
}

#[derive(Debug)]
pub struct DeviceResult {
    pub data: DeviceTransfer,
    pub result: Result<usize, DeviceError>,
}

impl DeviceResult {
    pub fn ok(data: DeviceTransfer, size: usize) -> Self {
        Self {
            data,
            result: Ok(size),
        }
    }

    pub fn err(data: DeviceTransfer, error: DeviceError) -> Self {
        Self {
            data,
            result: Err(error),
        }
    }

    pub fn expect(self, msg: &str) -> (DeviceTransfer, usize) {
        let DeviceResult { data, result } = self;
        (data, result.expect(msg))
    }
}

#[derive(Debug)]
pub struct DeviceOperation(DeviceResult);

impl DeviceOperation {
    pub fn new(result: DeviceResult) -> Self {
        Self(result)
    }

    pub fn check(self) -> Result<DeviceResult, Self> {
        Ok(self.0)
    }

    pub fn wait(self) -> DeviceResult {
        self.0
    }
}

pub trait Device : Debug {
    fn name(&self) -> &str;
    fn uuid(&self) -> Uuid;
}

pub trait BlockDevice : Device {
    // all methods must be thread safe

    fn block_size_shift(&self) -> u32;
    fn block_size(&self) -> usize {
        1 << self.block_size_shift()
    }

    fn block_count(&self) -> usize;

    fn max_data_size(&self) -> usize;

    fn read(&self, block: usize, data: DeviceTransfer) -> DeviceOperation;
    fn write(&self, block: usize, data: DeviceTransfer) -> DeviceOperation;
}

pub trait CharacterDevice : Device {
    // all methods must be thread safe

    fn read(&self, data: DeviceTransfer) -> DeviceOperation;
    fn write(&self, data: DeviceTransfer) -> DeviceOperation;
}
