use core::fmt::Debug;
use core::marker::PhantomData;
use core::ptr::NonNull;
use bytemuck::Pod;
use crate::memory::address::{PageCount, PhysAddrPageAligned, VirtAddrPageAligned};
use crate::memory::{MemoryFlags, MemorySpace};
use crate::volatile::Volatile;

#[derive(Debug)]
pub struct MMIO {
    base: NonNull<u8>,
    size: usize,
}

impl MMIO {
    pub unsafe fn new(phys: PhysAddrPageAligned, size: PageCount, writable: bool, write_through: bool) -> Self {
        assert!(writable || !write_through, "mmio: memory cannot be both read-only and write-through");
        let virt = MemorySpace::with(|space| {
            let virt = space.virt_alloc(size);
            let mut flags = MemoryFlags::NO_CACHE | MemoryFlags::NO_EXEC;
            if writable {
                flags |= MemoryFlags::WRITE;
            }
            if write_through {
                flags |= MemoryFlags::WRITE_THROUGH;
            }
            space.map(phys, virt, size, flags);
            virt
        });
        Self {
            base: virt.as_non_null().expect("mmio: virt_alloc returned null"),
            size: size.size(),
        }
    }
}

unsafe impl VolatileAccess for MMIO {
    fn ptr(&self) -> NonNull<u8> {
        self.base
    }
    fn size(&self) -> usize {
        self.size
    }
}

#[derive(Debug)]
pub struct MMIOPartition<'a> {
    base: NonNull<u8>,
    size: usize,
    pd: PhantomData<&'a mut ()>,
}

unsafe impl<'a> VolatileAccess for MMIOPartition<'a> {
    fn ptr(&self) -> NonNull<u8> {
        self.base
    }
    fn size(&self) -> usize {
        self.size
    }
}

pub unsafe trait VolatileAccess {
    fn ptr(&self) -> NonNull<u8>;
    fn size(&self) -> usize;

    fn get_ptr_at(&self, offset: usize, len: usize) -> NonNull<u8> {
        let ptr = self.ptr();
        let size = self.size();

        let end = offset.strict_add(len);
        assert!(end < size,
                "mmio: out of bounds access - offset {offset:#x}..{end:#x}, mmio base {:#x}, size {:#x}",
                ptr.as_ptr() as usize, size);
        unsafe { ptr.byte_add(offset) }
    }

    fn get_ptr<T: Pod>(&self, offset: usize) -> NonNull<T> {
        self.get_ptr_at(offset, size_of::<T>()).cast()
    }

    fn load<T: Pod>(&self, offset: usize) -> T {
        unsafe { self.get_ptr::<T>(offset).read_volatile() }
    }
    fn store<T: Pod>(&self, offset: usize, data: T) {
        unsafe { self.get_ptr::<T>(offset).write_volatile(data) }
    }

    fn get_ref<T: Pod>(&self, offset: usize) -> &T {
        unsafe { self.get_ptr(offset).as_ref() }
    }
    fn get_mut<T: Pod>(&self, offset: usize) -> &mut T {
        unsafe { self.get_ptr(offset).as_mut() }
    }

    fn get_slice<T: Pod>(&self, offset: usize, len: usize) -> &[T] {
        unsafe { core::slice::from_raw_parts(
            self.get_ptr_at(offset, len * size_of::<T>()).cast().as_ptr(), len) }
    }
    fn get_slice_mut<T: Pod>(&self, offset: usize, len: usize) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(
            self.get_ptr_at(offset, len * size_of::<T>()).cast().as_ptr(), len) }
    }

    fn get_volatile<T: Pod + Debug>(&self, offset: usize) -> &mut Volatile<T> {
        self.get_mut(offset)
    }

    fn partition(&self, offset: usize, size: usize) -> MMIOPartition<'_> {
        MMIOPartition {
            base: self.get_ptr_at(offset, size),
            size,
            pd: PhantomData,
        }
    }
}

impl Drop for MMIO {
    fn drop(&mut self) {
        let virt = VirtAddrPageAligned::new(self.base.addr().into())
            .expect("mmio: base is not page aligned");
        let size = PageCount::try_pages(self.size)
            .expect("mmio: size is not page aligned");
        MemorySpace::get().virt_dealloc(virt, size, |_, _| {});
    }
}
