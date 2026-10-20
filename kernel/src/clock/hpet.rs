use bytemuck::{Pod, Zeroable};
use modular_bitfield::prelude::*;
use crate::{debug, logger};
use crate::memory::address::PhysAddrPageAligned;
use crate::memory::mmio::{VolatileAccess, MMIO};

logger!("HPET");

const REG_CAPABILITIES: usize = 0x00;
const REG_CONFIGURATION: usize = 0x10;
const REG_MAIN_COUNTER: usize = 0xf0;

#[bitfield]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
struct Capabilities {
    revision_id: B8,
    timer_count: B5,
    timer_64bit: bool,
    reserved1: bool,
    legacy_replacement_mapping: bool,
    vendor_id: B16,
    counter_clock_period: B32,
}

#[bitfield]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
struct Configuration {
    enabled: bool,
    legacy_replacement_mapping: bool,
    reserved1: B62,
}

#[derive(Debug)]
pub struct HPET {
    regs: MMIO,
    clock_period: u32,
}

impl HPET {
    #[inline(always)]
    pub fn per_tick_femtoseconds(&self) -> u32 {
        self.clock_period
    }

    #[inline(always)]
    pub fn get_ticks(&self) -> u64 {
        self.regs.load(REG_MAIN_COUNTER)
    }
}

pub fn init(tables: &crate::acpi::Tables) -> HPET {
    let table = tables.get_hpet().expect("hpet: not present");
    let regs = unsafe { MMIO::new(
        PhysAddrPageAligned::new(table.base_address)
            .expect("hpet: physical address of registers is not page aligned"),
        1.into(), true, false) };

    let capabilities = regs
        .get_volatile::<Capabilities>(REG_CAPABILITIES).load();
    let configuration = regs.get_volatile::<Configuration>(REG_CONFIGURATION);

    let clock_period = capabilities.counter_clock_period();
    assert_ne!(clock_period, 0, "hpet: counter clock period is zero");
    debug!("main counter period: {} fs = {} ns", clock_period, clock_period / 1000000);

    if !capabilities.timer_64bit() {
        panic!("hpet: 64 bit timer not available");
    }

    configuration.modify(|config| config.with_enabled(true));

    HPET {
        regs,
        clock_period,
    }
}
