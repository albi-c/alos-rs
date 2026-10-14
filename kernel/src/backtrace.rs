use alloc::sync::Arc;
use crate::lock::Lock;

static EXECUTABLE: Lock<Option<Executable>> = Lock::new(None);

struct Executable {
    data: &'static [u8],
    dwarf: Arc<gimli::Dwarf<gimli::EndianSlice<'static, gimli::NativeEndian>>>,
}

impl Executable {
    fn new(data: &'static [u8]) -> Self {
        let elf = elf::ElfBytes::<elf::endian::NativeEndian>::minimal_parse(data)
            .expect("backtrace: failed to parse elf");
        let dwarf = gimli::Dwarf::load::<_, elf::ParseError>(|section_id| {
            let Some(section_header) = elf.section_header_by_name(section_id.name())? else {
                return Ok(gimli::read::EndianSlice::new(&[], gimli::NativeEndian));
            };
            let (section_data, compression) = elf.section_data(&section_header)?;
            assert!(compression.is_none(), "backtrace: elf compression is not supported");
            Ok(gimli::read::EndianSlice::new(section_data, gimli::NativeEndian))
        }).expect("backtrace: failed to parse elf");

        Self {
            data,
            dwarf: Arc::new(dwarf),
        }
    }
}

fn handle_frame(frame: &backtracer_core::Frame, func: &mut impl FnMut(usize)) -> bool {
    let ip = frame.ip();
    if (ip as u64) < 0xffffffff00000000 {
        false
    } else {
        func(ip as usize);
        true
    }
}

pub fn print(mut func: impl FnMut(usize)) {
    backtracer_core::trace(move |frame| handle_frame(frame, &mut func));
}

pub fn print_from(mut func: impl FnMut(usize), bp: u64, sp: u64, ip: u64) {
    backtracer_core::trace_from(
        backtracer_core::EntryPoint::new(bp, sp, ip + 1),
        move |frame| handle_frame(frame, &mut func));
}

pub fn set_executable(executable: &'static [u8]) {
    assert!(EXECUTABLE.write().replace(Executable::new(executable)).is_none(),
            "backtrace: set_executable called twice");
}
