use core::{mem, ptr};

use core::intrinsics::{volatile_load, volatile_store};

use crate::memory::Frame;
use crate::paging::{KernelMapper, PhysicalAddress, PageFlags};

use super::sdt::Sdt;
use super::{ACPI_TABLE, find_sdt};

#[repr(packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GenericAddressStructure {
    _address_space: u8,
    _bit_width: u8,
    _bit_offset: u8,
    _access_size: u8,
    pub address: u64,
}

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct Hpet {
    pub header: Sdt,

    pub hw_rev_id: u8,
    pub comparator_descriptor: u8,
    pub pci_vendor_id: u16,

    pub base_address: GenericAddressStructure,

    pub hpet_number: u8,
    pub min_periodic_clk_tick: u16,
    pub oem_attribute: u8
}

impl Hpet {
    pub fn init() {
        let hpet_sdt = find_sdt("HPET");
        let hpet = if hpet_sdt.len() == 1 {
            Hpet::new(hpet_sdt[0])
        } else {
            println!("Unable to find HPET");
            return;
        };

        if let Some(hpet) = hpet {
            println!("  HPET: {:X}", hpet.hpet_number);

            let mut hpet_t = ACPI_TABLE.hpet.write();
            *hpet_t = Some(hpet);
        }
    }

    pub fn new(sdt: &'static Sdt) -> Option<Hpet> {
        if &sdt.signature == b"HPET" && sdt.length as usize >= mem::size_of::<Hpet>() {
            let s = unsafe { ptr::read((sdt as *const Sdt) as *const Hpet) };
            unsafe { s.base_address.init(&mut KernelMapper::lock()) };
            Some(s)
        } else {
            None
        }
    }
}

impl GenericAddressStructure {
    pub unsafe fn init(&self, mapper: &mut KernelMapper) {
        let frame = Frame::containing_address(PhysicalAddress::new(self.address as usize));
        let (_, result) = mapper
            .get_mut()
            .expect("KernelMapper locked re-entrant while mapping memory for GenericAddressStructure")
            .map_linearly(frame.start_address(), PageFlags::new().write(true))
            .expect("failed to map memory for GenericAddressStructure");
        result.flush();
    }

    pub unsafe fn read_u64(&self, offset: usize) -> u64{
        volatile_load((self.address as usize + offset + crate::PHYS_OFFSET) as *const u64)
    }

    pub unsafe fn write_u64(&mut self, offset: usize, value: u64) {
        volatile_store((self.address as usize + offset + crate::PHYS_OFFSET) as *mut u64, value);
    }
}
