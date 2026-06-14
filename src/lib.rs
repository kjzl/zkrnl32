//! zkrnl32 - A 32-bit x86 kernel
//!
//! This is a from-scratch 32-bit x86 kernel written in Rust. It serves as a
//! learning project to internalise the fundamentals of kernel booting, memory
//! management, and interrupts.
//!
//! For architectural details and project goals, refer to the `docs/` directory.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::all)]
#![warn(rust_2018_idioms)]
// --- Nightly Features ---
//
// Document all required nightly feature flags here, including why they are needed.
// If a feature becomes stabilized, leave a commented-out record of it here for historical context.
//
// Active features:
// #![feature(naked_functions)] // Required for x86 interrupt handlers without prologue/epilogue.
// #![feature(abi_x86_interrupt)] // Provides the "x86-interrupt" calling convention.

// --- Testing Strategy (TBD) ---
//
// Currently `test = false` and `bench = false` are set in Cargo.toml to keep Rust
// Analyzer happy since standard tests require `std`.
//
// In the future, we will likely adopt a two-pronged testing strategy:
// 1. Host-side Unit Tests: For pure hardware-agnostic logic (e.g. data structures),
//    we will use `#[cfg(test)]` to temporarily re-enable `std` and run `cargo test`
//    on the host machine.
// 2. Custom Test Frameworks: For hardware-specific kernel logic, we will use
//    `#![feature(custom_test_frameworks)]` to compile tests directly into the kernel
//    binary, execute them inside a headless QEMU instance, and parse the output via
//    a virtual serial port. This will be implemented around Stage 3/4.

pub mod console;
pub mod gdt;
pub mod memory;
pub mod multiboot;
pub mod prelude;
pub mod stack;
pub mod utils;
pub mod vga;
pub mod volatile;

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::multiboot::{BOOTLOADER_MAGIC, MultibootInfo};
use crate::vga::Color;

/// Kernel entry point called by the assembly boot stub.
///
/// Receives the Multiboot hand-off the boot stub pushed: `magic` (from `eax`)
/// proves a Multiboot-compliant loader, and `info` (from `ebx`) points at the
/// structure GRUB built. The magic is checked before `info` is dereferenced.
///
/// # Safety
///
/// Must be called exactly once, by the boot stub, with the register state a
/// Multiboot loader provides. When `magic` equals [`BOOTLOADER_MAGIC`], `info`
/// must point at the valid Multiboot information structure GRUB built.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_main(magic: u32, info: *const MultibootInfo) -> ! {
    if magic != BOOTLOADER_MAGIC {
        panic!("not booted by a multiboot loader: magic={magic:#x}");
    }
    // SAFETY: a matching boot magic means GRUB placed a valid information
    // structure at `info`; it stays mapped and unmodified this early in boot,
    // so a shared borrow for the duration of this call is sound.
    let info = unsafe { &*info };

    {
        // SAFETY: single-core with interrupts disabled, and no other borrow of
        // VGA_WRITER is live across this block.
        #[expect(static_mut_refs)]
        let display = unsafe { &mut vga::VGA_WRITER };
        display.clear();
    }

    printk!("zkrnl32 booting\n");
    multiboot::print_memory_map(info);

    loop {
        core::hint::spin_loop();
    }
}

/// The kernel's fatal-error path.
///
/// Prints the panic message and source location in a loud colour, then halts
/// the processor for good. There is no unwinding (`panic = "abort"`), so every
/// panic ends execution here. A re-entrancy guard keeps a fault *inside* this
/// handler from recursing: on re-entry it skips printing and halts directly.
#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // A panic while printing, e.g. a fault touching the VGA buffer, would
    // re-enter this handler. `swap` lets only the first entrant print; later
    // ones fall straight through to the halt, so a secondary fault can neither
    // recurse forever nor scramble the screen. Relaxed suffices: the kernel is
    // single-core and takes no interrupts.
    static PANICKING: AtomicBool = AtomicBool::new(false);
    if !PANICKING.swap(true, Ordering::Relaxed) {
        printk_color!(Color::White.on(Color::Red), "\nKERNEL PANIC\n{info}\n");
        // Dump a short window of the stack from this frame upward through the
        // frames that led here. `dump_stack!` is bounds-checked, so a corrupt or
        // overflowed stack degrades to a diagnostic line instead of a wild read.
        // Kept short so the message above stays on the 25-row VGA screen; a
        // serial mirror later lifts that limit.
        crate::dump_stack!(16);
    }

    // Park the core: `hlt` stops it until an interrupt, and the loop re-halts if
    // a non-maskable one wakes it. `cli` masks the maskable interrupts (none are
    // enabled yet, but be explicit). Idles the CPU instead of the old hot spin.
    loop {
        // SAFETY: `cli`/`hlt` touch only CPU interrupt state; sound to run here.
        unsafe { core::arch::asm!("cli", "hlt", options(nomem, nostack)) };
    }
}
