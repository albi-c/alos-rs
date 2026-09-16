use elf::ElfBytes;
use elf::endian::NativeEndian;
use crate::memory::{MemoryFlags, MemorySpace};
use crate::memory::address::{align_page_range, VirtAddr};
use crate::println;

pub enum ElfError {
    Lib(elf::ParseError),
    Err(&'static str),
}

impl From<elf::ParseError> for ElfError {
    fn from(err: elf::ParseError) -> ElfError {
        ElfError::Lib(err)
    }
}

pub fn load_elf(data: &[u8]) -> Result<usize, ElfError> {
    let file = ElfBytes::<NativeEndian>::minimal_parse(data)?;

    for program in file.segments().ok_or(ElfError::Err("no program headers"))? {
        if program.p_type != 1 {
            continue;
        }
        if program.p_memsz == 0 {
            continue;
        }
        let (addr, protect) = {
            let mem = MemorySpace::get();
            let prog_virt_addr = VirtAddr::new(program.p_vaddr as usize);
            let (virt_addr, pages) = align_page_range(
                prog_virt_addr.addr(), program.p_memsz as usize);
            let phys_addr = mem.user_phys_alloc(pages).ok_or(ElfError::Err("out of memory"))?;
            mem.user_virt_alloc_at(virt_addr, pages).ok_or(ElfError::Err("unable to allocate virtual memory"))?;
            const DEFAULT_FLAGS: MemoryFlags = MemoryFlags::DEFAULT_URW;
            mem.map(phys_addr, virt_addr, pages, DEFAULT_FLAGS);
            let mut flags = MemoryFlags::USER;
            if program.p_flags & 0x1 == 0 {
                flags |= MemoryFlags::NO_EXEC;
            }
            if program.p_flags & 0x2 != 0 {
                flags |= MemoryFlags::WRITE;
            }
            if flags != DEFAULT_FLAGS {
                (prog_virt_addr, Some((virt_addr, pages, flags)))
            } else {
                (prog_virt_addr, None)
            }
        };
        let data = addr.as_mut_ptr::<u8>();
        if program.p_filesz > 0 {
            let prog_data = file.segment_data(&program)?;
            if prog_data.len() < program.p_filesz as usize {
                return Err(ElfError::Err("program data too short"));
            }
            unsafe {
                core::ptr::copy(prog_data.as_ptr(), data, program.p_filesz.min(program.p_memsz) as usize);
            }
        }
        if program.p_memsz > program.p_filesz {
            unsafe {
                core::ptr::write_bytes(data.byte_add(program.p_filesz as usize), 0,
                                       (program.p_memsz - program.p_filesz) as usize);
            }
        }
        if let Some((virt_addr, pages, flags)) = protect {
            MemorySpace::get().protect(virt_addr, pages, flags);
        }
        println!("{:x?} @{:x?}", program, addr);
    }

    Ok(file.ehdr.e_entry as usize)
}
