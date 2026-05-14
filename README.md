# zkrnl32

A from-scratch 32-bit x86 kernel in Rust — no external crates, booted by GRUB.

## What this is

A hand-written `i386` kernel built without leaning on the Rust kernel-dev ecosystem (`bootloader`, `x86_64`, `uefi`, etc.). Everything that runs after GRUB hands control to the kernel is written here: the boot stub, the screen driver, the kernel runtime, the lot.

It exists as a learning vehicle. The goal isn't to ship an OS; it's to internalise how an x86 machine actually starts, what the privilege rings really do, how paging is laid out by hand, how interrupts are dispatched, and how a small kernel grows into something that can run a userspace process.

## Status

Early.

- Stage 1 (`kfs1`): boot + screen - done.
- Stage 2 (`kfs2`): GDT + stack - goals defined.
- Stage 3 (`kfs3`): memory - goals defined.

## Building

```sh
make build
make iso
make run
```

Use `make run-headless` when no graphical QEMU display is available.

### Requirements

- nasm
- rust nightly
- rust-src for the nightly toolchain
- grub-mkrescue
- xorriso
- mtools
- qemu-system-i386

## Documentation

- [`docs/stage-1.md`](docs/stage-1.md) — stage 1 goals
- [`docs/stage-2.md`](docs/stage-2.md) — stage 2 goals
- [`docs/stage-3.md`](docs/stage-3.md) — stage 3 goals

## License

MIT — see [`LICENSE`](LICENSE).
