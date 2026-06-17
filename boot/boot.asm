bits 32

; Declare constants for the multiboot header.
MBALIGN  equ  1 << 0            ; align loaded modules on page boundaries
MEMINFO  equ  1 << 1            ; provide memory map
MBFLAGS  equ  MBALIGN | MEMINFO ; this is the Multiboot 'flag' field
MAGIC    equ  0x1BADB002        ; 'magic number' lets bootloader find the header
CHECKSUM equ -(MAGIC + MBFLAGS) ; checksum of above, to prove we are multiboot
                                ; CHECKSUM + MAGIC + MBFLAGS should be Zero (0)

; Declare a multiboot header that marks the program as a kernel. These are magic
; values that are documented in the multiboot standard. The bootloader will
; search for this signature in the first 8 KiB of the kernel file, aligned at a
; 32-bit boundary. The signature is in its own section so the header can be
; forced to be within the first 8 KiB of the kernel file.
section .multiboot
align 4
	dd MAGIC
	dd MBFLAGS
	dd CHECKSUM

; The directory is built in Rust (memory::paging) and placed in the
; identity-linked .boot.data section, so its link address is also its physical
; address - exactly what CR3 wants. The boot stub only needs its symbol.
extern BOOT_PAGE_DIRECTORY

; 16 KiB reserved in .bss, which the linker places in the high half. Unusable
; until paging is on, so the boot code below sets ESP only after the jump. The
; SysV i386 ABI wants 16-byte alignment, which the compiler relies on.
section .bss
align 16
global stack_bottom
global stack_top
stack_bottom:
	resb 16384
stack_top:

; This section is identity-linked (VMA == LMA, low), because it executes with
; paging off, where only physical addresses are valid. GRUB enters at _start.
section .boot.text progbits alloc exec nowrite align=16
global _start:function (_start.end - _start)
_start:
	; GRUB hands us EAX = multiboot magic, EBX = info pointer, with paging and
	; interrupts off. EAX is about to be clobbered by the control-register dance,
	; so stash magic in ESI; EBX is left untouched all the way to kernel_main.
	mov esi, eax

	; Point CR3 at the bootstrap directory. It lives in identity-linked
	; .boot.data, so the symbol's address already is its physical address.
	mov eax, BOOT_PAGE_DIRECTORY
	mov cr3, eax

	; Enable 4 MiB pages (CR4.PSE). Without it the PAGE_SIZE bit in our PDEs is
	; reserved and the entries would fault.
	mov eax, cr4
	or eax, 1 << 4
	mov cr4, eax

	; Turn paging on (CR0.PG). From the next instruction fetch on, every address
	; is translated; we keep running only because PDE[0] identity-maps the low
	; memory this code sits in. Writing CR0 is serializing, so the new mapping is
	; in effect immediately.
	mov eax, cr0
	or eax, 1 << 31
	mov cr0, eax

	; Leave the low half. `higher_half` is linked at its high virtual address,
	; now mapped by PDE[768], so an indirect jump lands us there. A near jump is
	; enough: the GDT segments are flat, so CS already spans the high half.
	mov ecx, higher_half
	jmp ecx
.end:

; Linked and now executing in the high half. The low identity map is still in
; place, so the GDT, VGA, and GRUB's low multiboot structures stay reachable.
section .text
higher_half:
	; A usable stack at last.
	mov esp, stack_top

	; Rebuild kernel_main(magic, info)'s cdecl frame (pushed right-to-left) and
	; keep ESP 16-byte aligned at both calls below. install_gdt takes no
	; arguments and returns ESP unchanged, so the frame survives for kernel_main.
	sub esp, 8       ; alignment padding for the 2-dword argument frame
	push ebx         ; arg 2: multiboot info pointer
	push esi         ; arg 1: multiboot magic

	; Install the kernel's own GDT and reload segments before entering Rust that
	; assumes our protected-mode layout.
	extern install_gdt
	call install_gdt

	; Enter the high-level kernel. The argument frame is already in place and ESP
	; is still 16-byte aligned, so we call straight through.
	extern kernel_main
	call kernel_main

	; kernel_main does not return; park the CPU if it ever does. Interrupts are
	; already off, but be explicit.
	cli
.hang:
	hlt
	jmp .hang
