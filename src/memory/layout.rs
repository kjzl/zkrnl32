//! Static memory layout of the kernel image.
//!
//! `zkrnl32` is a *higher-half* kernel: the linker (`boot/linker.ld`) links it
//! to run from high virtual addresses (the `0xC000_0000` half) but GRUB loads
//! it low in physical memory. The script brackets the image with symbols at
//! both ends of that split - a physical pair (`kernel_phys_*`) and a virtual
//! pair (`kernel_virt_*`).

/// Virtual base of the kernel's high half: the fixed offset between a kernel
/// virtual address and its physical load address.
///
/// This must equal `KERNEL_VMA` in `boot/linker.ld`; the two are kept in
/// lockstep by hand, and disagreeing would mismap the whole kernel.
pub const KERNEL_VIRTUAL_BASE: usize = 0xC000_0000;

unsafe extern "C" {
    /// Linker marker at the first physical byte of the loaded image (its
    /// `.boot` section, at the 2 MiB load address).
    static kernel_phys_start: u8;
    /// Linker marker one physical byte past the loaded image, rounded up to a
    /// 4 KiB frame.
    static kernel_phys_end: u8;
    /// Linker marker at the first virtual byte of the kernel's high half.
    static kernel_virt_start: u8;
    /// Linker marker one virtual byte past the kernel's high half, rounded up
    /// to a 4 KiB page.
    static kernel_virt_end: u8;
}

/// Physical address of the first byte of the loaded kernel image.
///
/// This is the 2 MiB load address fixed by the linker script.
pub fn kernel_phys_start_addr() -> usize {
    (&raw const kernel_phys_start) as usize
}

/// Physical address one past the last byte of the loaded kernel image, rounded
/// up to a 4 KiB frame boundary.
pub fn kernel_phys_end_addr() -> usize {
    (&raw const kernel_phys_end) as usize
}

/// Virtual address of the first byte of the kernel's high half.
pub fn kernel_virt_start_addr() -> usize {
    (&raw const kernel_virt_start) as usize
}

/// Virtual address one past the last byte of the kernel's high half, rounded up
/// to a 4 KiB page boundary.
pub fn kernel_virt_end_addr() -> usize {
    (&raw const kernel_virt_end) as usize
}
