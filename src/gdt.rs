//! Global Descriptor Table and associated Data Structures

use core::arch::naked_asm;

pub const GDT_BASE: u32 = 0x0000_0800;
pub const GDT_ENTRY_COUNT: usize = 7;
// bits 0-1 represent the privilege level
// if bit 2 is set, the LDT will be used instead of the GDT
// bits 3-15 represent the index into the gdt
pub const KERNEL_CODE_SELECTOR: u16 = 0b_01000;
pub const KERNEL_DATA_SELECTOR: u16 = 0b_10000;
pub const KERNEL_STACK_SELECTOR: u16 = 0b11000;

/// Indexes into the GDT Table
pub mod index {
    pub const NULL: usize = 0;
    pub const KERNEL_CODE: usize = 1;
    pub const KERNEL_DATA: usize = 2;
    pub const KERNEL_STACK: usize = 3;
    pub const USER_CODE: usize = 4;
    pub const USER_DATA: usize = 5;
    pub const USER_STACK: usize = 6;
}

#[repr(C, packed)]
pub struct Gdtr {
    limit: u16,
    base: u32,
}

/// Writes the bootstrap GDT to `dst_addr` and writes a matching GDTR.
///
/// # Safety
///
/// `dst_addr..dst_addr + size_of::<Gdt>()` must be valid writable memory.
/// `dst_addr` must be a valid linear address.
/// `out_gdtr` must be valid for writing one `Gdtr`.
#[unsafe(no_mangle)]
unsafe extern "C" fn write_gdt(dst_addr: u32, out_gdtr: *mut Gdtr) {
    let dst = dst_addr as *mut Gdt;

    unsafe {
        dst.write(Gdt::bootstrap());
        out_gdtr.write(Gdtr {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: dst_addr,
        });
    }
}

/// Installs the bootstrap GDT and reloads segment registers.
///
/// # Safety
///
/// Must be called in 32-bit protected mode with a valid stack.
/// Paging must be disabled, or `GDT_BASE` must be mapped.
/// `write_gdt` must write a valid GDT/GDTR pair.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn install_gdt() {
    naked_asm!(
        "sub esp, 6",
        "mov eax, esp",

        "push eax",
        "push {gdt_base}",
        "call {write_gdt}",
        "add esp, 8",

        "lgdt [esp]", // load gdt

        // selectors can only be set via another register, not raw by value
        "mov ax, {data_sel}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",

        "mov ax, {stack_sel}",
        "mov ss, ax",

        // `cs` can't be set directly, only through a control transfer.
        // When doing a control transfer via `retf` (far return),
        // a new code selector and `EIP` Instruction Pointer are loaded from the stack.
        "push {code_sel}",
        "lea eax, [2f]", // load effective address of next label 2 (in forward direction)
        "push eax",
        // far return using the pushed code selector and the instruction pointer,
        // pointing to the label 2 in the line below
        "retf",
        "2:",

        "add esp, 6",
        "ret",

        gdt_base = const GDT_BASE,
        code_sel = const KERNEL_CODE_SELECTOR,
        data_sel = const KERNEL_DATA_SELECTOR,
        stack_sel = const KERNEL_STACK_SELECTOR,
        write_gdt = sym write_gdt,
    );
}

/// The GDT Table structure
#[repr(C, align(8))]
pub struct Gdt {
    entries: [RawDescriptor; GDT_ENTRY_COUNT],
}

impl Gdt {
    const fn bootstrap() -> Self {
        Self {
            entries: [
                RawDescriptor::null(),
                SegmentDescriptor::flat_code(PrivilegeLevel::Ring0).raw(),
                SegmentDescriptor::flat_data(PrivilegeLevel::Ring0).raw(),
                SegmentDescriptor::flat_data(PrivilegeLevel::Ring0).raw(),
                SegmentDescriptor::flat_code(PrivilegeLevel::Ring3).raw(),
                SegmentDescriptor::flat_data(PrivilegeLevel::Ring3).raw(),
                SegmentDescriptor::flat_data(PrivilegeLevel::Ring3).raw(),
            ],
        }
    }
}

/// Privilege level
#[allow(missing_docs)]
#[repr(u8)]
pub enum PrivilegeLevel {
    /// Kernel mode
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    /// User mode
    Ring3 = 3,
}

/// A Raw Descriptor for usage in a Global Descriptor Table
///
/// Obtainable via .raw() on respective descriptors
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct RawDescriptor(u64);

impl RawDescriptor {
    const fn null() -> Self {
        Self(0)
    }
}

/// A GDT Table Segment Descriptor Entry
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    const fn raw(self) -> RawDescriptor {
        RawDescriptor(self.0)
    }

    const fn flat_code(privilege: PrivilegeLevel) -> Self {
        // 1 << 7 presence bit
        // 0b11 << 5 privilege
        // 1 << 4 segment
        // 1 << 3 executable
        // 1 << 2 conforming bit
        // 1 << 1 readable bit
        // 1 cpu access bit
        let mut access = 0b1001_1010;
        access |= (privilege as u8) << 5;
        Self::flat(access)
    }

    const fn flat_data(privilege: PrivilegeLevel) -> Self {
        // 1 << 7 presence bit
        // 0b11 << 5 privilege
        // 1 << 4 segment
        // 1 << 3 executable
        // 1 << 2 direction bit
        // 1 << 1 writable bit
        // 1 cpu access bit
        let mut access = 0b1001_0010;
        access |= (privilege as u8) << 5;
        Self::flat(access)
    }

    const fn flat(access: u8) -> Self {
        // flags &  1 << 3 ? 4KiB : byte (Limit granularity)
        //          1 << 2 ? 32bit : 16bit
        //          1 << 1 ? long mode : protected mode
        let flags: u64 = 1 << 2 | 1 << 3;
        let limit: u64 = 0xFFFFF;
        let mut raw = limit & 0xFFFF; // limit bits 0-15
        // base address bits 0-23
        raw |= (access as u64) << 40; // access bits 0-7
        raw |= (limit >> 16 & 0xF) << 48; // limit bits 16-23
        raw |= (flags & 0xF) << 52; // flag bits 0-3
        // base address bits 24-31
        Self(raw)
    }
}
