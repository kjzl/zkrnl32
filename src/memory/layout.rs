//! Static memory layout of the kernel image.
//!
//! The linker script (`boot/linker.ld`) brackets the loaded kernel with two
//! symbols, `kernel_start` and `kernel_end`. They are not real variables - only
//! their *addresses* mean anything: together they bound the half-open physical
//! range `[kernel_start, kernel_end)` the kernel image occupies once GRUB has
//! loaded it at the 2 MiB mark.

unsafe extern "C" {
    /// Linker marker at the first byte of the loaded kernel image.
    static kernel_start: u8;
    /// Linker marker one byte past the end of the loaded kernel image.
    static kernel_end: u8;
}

/// Physical address of the first byte of the loaded kernel image.
///
/// This is the 2 MiB load address fixed by the linker script.
pub fn kernel_start_addr() -> usize {
    (&raw const kernel_start) as usize
}

/// Physical address one past the last byte of the loaded kernel image, rounded
/// up to a 4 KiB frame boundary.
pub fn kernel_end_addr() -> usize {
    (&raw const kernel_end) as usize
}
