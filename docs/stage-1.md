# Stage 1 — boot + screen

Maps to `kfs1` of the kfs subject series from the 42 advanced curriculum.

## Mandatory goals

A kernel that:

1. Boots via GRUB on `i386`.
2. Has an ASM boot stub with a Multiboot 1 header that GRUB can find and validate.
3. Calls into Rust (`kernel_main`) once the CPU is in protected mode with a usable stack.
4. Compiles without host runtime or library dependencies (no `std`, no host libc).
5. Links with a custom linker script (no host system linker scripts).
6. Implements a kernel/screen interface that can place characters into VGA text memory.
7. Displays the literal string `42` on screen.

## Bonuses included in scope

The screen driver is the only debug surface available until later stages, so it's worth investing in:

- **Colour support** — VGA text mode supports a 16-colour foreground/background pair per cell; expose it.
- **Cursor tracking** — keep an `(x, y)` cursor that advances per character and wraps on line end.
- **Scrolling** — when the cursor passes the bottom of the screen, shift the buffer up and clear the new bottom row.
- **`printk!` macro** — formatted printing into the screen buffer. Pays itself back the moment we start debugging a GDT in stage 2.

## Bonuses deferred

- **Keyboard input** — requires an IDT and an ISR for the PS/2 controller; belongs in stage 4. A polling-based stand-in would be code we throw away.
- **Multiple virtual screens with shortcut switching** — depends on keyboard input.

## Out of scope (will surface in later stages)

GDT setup beyond what GRUB leaves us with, paging, interrupts, dynamic memory, multitasking, userspace.

## Done when

- `make run` boots the kernel in QEMU.
- `make iso` produces a GRUB-bootable ISO.
- The screen shows `42` after boot.
- `printk!` works for ASCII strings, with colour, cursor advance, and scrolling.
- The kernel-helpers module is reachable and usable from `kernel_main`.
