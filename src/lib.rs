//! zkrnl32 - A 32-bit x86 kernel
//!
//! This is a from-scratch 32-bit x86 kernel written in Rust. It serves as a
//! learning project to internalise the fundamentals of kernel booting, memory
//! management, and interrupts.
//!
//! For architectural details and project goals, refer to the `docs/` directory.

#![no_std]
#![deny(missing_docs)]
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
pub mod prelude;
pub mod vga;
pub mod volatile;

use core::panic::PanicInfo;

use crate::vga::Color;

/// Kernel entry point called by the assembly boot stub.
#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    #[expect(static_mut_refs)]
    let display = unsafe { &mut vga::VGA_WRITER };

    display.clear();
    display.write(b"Hello World!\n");
    printk_color!(Color::Black.on(Color::White), "42\n");

    loop {
        core::hint::spin_loop();
    }
}

/// The central panic handler for the kernel.
///
/// This function is called whenever the kernel panics. It currently loops indefinitely.
///
/// # Safety
///
/// This function never returns.
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
