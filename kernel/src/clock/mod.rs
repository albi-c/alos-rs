mod hpet;
pub mod tsc;

pub fn init_with_acpi(tables: &crate::acpi::Tables) {
    let hpet = hpet::init(tables);
    tsc::init(&hpet);
}