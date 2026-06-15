//! Kernel memory management.
//!
//! Houses the typed [addresses](address), the physical [frame allocator](frame),
//! the static kernel-image [layout](layout), and the [`init`] entry point the
//! boot path calls to bring the subsystem online. Paging and the kernel heap
//! will join here soon.

use crate::multiboot::MultibootInfo;

#[expect(dead_code)]
pub mod address;
#[expect(dead_code)]
pub mod frame;
pub mod layout;

/// Brings the memory subsystem online from the Multiboot hand-off.
///
/// Builds the physical frame allocator's picture of usable RAM from the BIOS
/// memory map: every fully-available frame is freed, then the frames the kernel
/// must keep are reserved.
///
/// # Panics
///
/// Panics if GRUB supplied no memory map. Without it the kernel has no record
/// of which physical frames exist, so there is nothing safe to allocate -
/// unrecoverable this early in boot.
pub fn init(info: &MultibootInfo) {
    let map = info
        .memory_map()
        .expect("multiboot supplied no memory map; cannot build the frame allocator");

    // SAFETY: boot runs on a single core with interrupts still disabled, and
    // this is the first and only code to touch FRAME_ALLOCATOR, so no other
    // reference to it is live.
    #[expect(static_mut_refs)]
    let allocator = unsafe { &mut frame::FRAME_ALLOCATOR };
    allocator.init(map);
}
