use elf::ElfBytes;
use elf::endian::NativeEndian;
use crate::memory::{address, MemoryFlags, MemorySpace};
use crate::println;

pub enum ElfError {
    Lib(elf::ParseError),
    Err(&'static str),
}

pub fn load_elf(data: &[u8]) -> Result<usize, ElfError> {
    let file = ElfBytes::<NativeEndian>::minimal_parse(data).expect("invalid elf");

    for program in file.segments().ok_or(ElfError::Err("no program headers"))? {
        if program.p_type != 1 {
            continue;
        }
        if program.p_memsz == 0 {
            continue;
        }
        let addr = MemorySpace::with(|mem| {
            let prog_virt_addr = program.p_vaddr as usize;
            let virt_addr = address::page_align_down(prog_virt_addr);
            let size_diff = prog_virt_addr - virt_addr;
            let pages = address::page_count_up(program.p_memsz as usize + size_diff);
            let phys_addr = mem.user_phys_alloc(pages).ok_or(ElfError::Err("out of memory"))?;
            mem.user_virt_alloc_at(virt_addr, pages).ok_or(ElfError::Err("unable to allocate virtual memory"))?;
            let flags = if program.p_flags & 0x1 != 0 {
                MemoryFlags::WRITE | MemoryFlags::USER
            } else {
                MemoryFlags::WRITE | MemoryFlags::USER | MemoryFlags::NO_EXEC
            };
            mem.map(phys_addr, virt_addr, pages, flags);
            Ok(prog_virt_addr)
        })?;
        let data = addr as *mut u8;
        if program.p_filesz > 0 {
            let prog_data = file.segment_data(&program).map_err(ElfError::Lib)?;
            if prog_data.len() < program.p_filesz as usize {
                return Err(ElfError::Err("program data too short"));
            }
            unsafe {
                core::ptr::copy(prog_data.as_ptr(), data, program.p_filesz as usize);
            }
        }
        if program.p_memsz > program.p_filesz {
            unsafe {
                core::ptr::write_bytes(data.byte_add(program.p_filesz as usize), 0,
                                       (program.p_memsz - program.p_filesz) as usize);
            }
        }
        println!("{:x?} @{:#x}", program, addr);
    }

    Ok(file.ehdr.e_entry as usize)
}
