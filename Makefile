TARGET := targets/i386-zkrnl32.json
TARGET_NAME := i386-zkrnl32
PROFILE := debug
RUST_TOOLCHAIN ?= nightly-2026-04-24
BUILD_DIR := build
KERNEL := $(BUILD_DIR)/kernel.elf
ISO := $(BUILD_DIR)/zkrnl32.iso
ISO_ROOT := $(BUILD_DIR)/iso
BOOT_OBJ := $(BUILD_DIR)/boot.o
RUST_LIB := target/$(TARGET_NAME)/$(PROFILE)/libzkrnl32.a
RUST_SYSROOT = $(shell rustc +$(RUST_TOOLCHAIN) --print sysroot)
RUST_LLD = $(RUST_SYSROOT)/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld
QEMU := qemu-system-i386
QEMU_FLAGS := -cdrom $(ISO)
QEMU_HEADLESS_FLAGS := -cdrom $(ISO) -display none
# Halt-and-log instead of silently rebooting on a fault: until an IDT exists,
# an unhandled fault triple-faults the CPU, and -no-reboot leaves a register
# dump in the log rather than resetting. -d logs exceptions, CPU resets (with
# registers), and invalid-state errors to build/qemu.log.
QEMU_DEBUG_FLAGS := -cdrom $(ISO) -no-reboot -no-shutdown -d int,cpu_reset,guest_errors -D $(BUILD_DIR)/qemu.log
# As above, plus a GDB stub on localhost:1234 (-s), frozen at power-on (-S) so a
# debugger can attach before the first instruction. Connect with
# `target remote :1234`.
QEMU_GDB_FLAGS := $(QEMU_DEBUG_FLAGS) -s -S
GRUB_BIOS_DIR := /usr/lib/grub/i386-pc

.PHONY: all build iso run run-headless run-debug run-gdb check check-rust-toolchain check-iso-tools check-run-tools print-rust-toolchain clean

# Default target
all: build

print-rust-toolchain:
	@printf '%s\n' '$(RUST_TOOLCHAIN)'

check-rust-toolchain:
	@rustc +$(RUST_TOOLCHAIN) --version >/dev/null

# Builds the bootable kernel ELF.
build: $(KERNEL)

# Builds a GRUB-bootable ISO image.
iso: $(ISO)

# Boots the ISO in QEMU.
run: $(ISO) | check-run-tools
	$(QEMU) $(QEMU_FLAGS)

# Boots the ISO in QEMU without opening a display window.
run-headless: $(ISO) | check-run-tools
	$(QEMU) $(QEMU_HEADLESS_FLAGS)

# Boots with fault logging to build/qemu.log and no reboot on fault.
run-debug: $(ISO) | check-run-tools
	$(QEMU) $(QEMU_DEBUG_FLAGS)

# Like run-debug, but freezes at power-on and exposes a GDB stub on :1234.
run-gdb: $(ISO) | check-run-tools
	$(QEMU) $(QEMU_GDB_FLAGS)

$(BUILD_DIR)/.dir:
	mkdir -p $(BUILD_DIR)
	touch $(BUILD_DIR)/.dir

$(BOOT_OBJ): boot/boot.asm | $(BUILD_DIR)/.dir
	nasm -f elf32 boot/boot.asm -o $(BOOT_OBJ)

RUST_SRCS := $(shell find src -name '*.rs')

$(RUST_LIB): Cargo.toml Cargo.lock $(RUST_SRCS) $(TARGET) | check-rust-toolchain
	cargo +$(RUST_TOOLCHAIN) build --target $(TARGET) -Zjson-target-spec -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem

$(KERNEL): $(BOOT_OBJ) $(RUST_LIB) boot/linker.ld | check-rust-toolchain
	$(RUST_LLD) -flavor gnu -m elf_i386 -T boot/linker.ld -o $(KERNEL) $(BOOT_OBJ) $(RUST_LIB)

check-iso-tools:
	@command -v grub-mkrescue >/dev/null 2>&1 || { echo "error: missing required tool: grub-mkrescue" >&2; exit 1; }
	@command -v xorriso >/dev/null 2>&1 || { echo "error: missing required tool: xorriso" >&2; exit 1; }
	@command -v mformat >/dev/null 2>&1 || { echo "error: missing required tool: mformat (install mtools)" >&2; exit 1; }
	@test -d $(GRUB_BIOS_DIR) || { echo "error: missing GRUB BIOS modules: $(GRUB_BIOS_DIR) (install grub-pc-bin)" >&2; exit 1; }

check-run-tools:
	@command -v $(QEMU) >/dev/null 2>&1 || { echo "error: missing required tool: $(QEMU)" >&2; exit 1; }

$(ISO): $(KERNEL) boot/grub.cfg Makefile | check-iso-tools
	mkdir -p $(ISO_ROOT)/boot/grub
	cp $(KERNEL) $(ISO_ROOT)/boot/kernel.elf
	cp boot/grub.cfg $(ISO_ROOT)/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) $(ISO_ROOT)

# Runs all format, lint, and validation checks using prek
check:
	prek run --all-files

# Cleans build artifacts
clean:
	cargo clean
	rm -rf $(BUILD_DIR)
