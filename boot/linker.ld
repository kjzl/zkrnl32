/* zkrnl32 is a higher-half kernel: it is linked to run from high virtual
   addresses (the 0xC0000000 half) but GRUB loads it low in physical memory.
   The split is undone at runtime by the paging transition, which maps the high
   virtual range onto the low physical range before jumping into it. */

/* The bootloader starts execution at the entry-point symbol. _start still lives
   in the low .boot section (see boot.asm), so GRUB enters it at a low physical
   address while paging is still off. */
ENTRY(_start)

/* Virtual base of the kernel's high half, and the physical address GRUB loads
   the image at. KERNEL_VMA must stay in lockstep with the virtual base the
   paging code uses; it is the single most important constant to keep in sync. */
KERNEL_VMA = 0xC0000000;
KERNEL_LMA = 2M;

SECTIONS
{
	/* 2M (rather than the traditional 1M) is a safer load address on modern
	   firmware. The boot code runs with paging OFF, so it is linked at its
	   physical address: here VMA == LMA, and absolute references resolve
	   correctly before any page table exists. */
	. = KERNEL_LMA;
	kernel_phys_start = .;

	.boot ALIGN(4K) :
	{
		/* The multiboot header must sit very early in the file or GRUB won't
		   recognise the image, so the low boot section leads. */
		KEEP(*(.multiboot))
		*(.boot)
		*(.boot.*)
	}

	/* From here the location counter jumps into the high half: sections are
	   linked at high virtual addresses (ADDR) but loaded at the matching low
	   physical address (AT), keeping the image contiguous in physical memory
	   while the kernel runs from the high half. */
	. += KERNEL_VMA;
	. = ALIGN(4K);
	kernel_virt_start = .;

	.text ALIGN(4K) : AT(ADDR(.text) - KERNEL_VMA)
	{
		*(.text)
		*(.text.*)
	}

	/* Read-only data. */
	.rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VMA)
	{
		*(.rodata)
		*(.rodata.*)
	}

	/* Read-write data (initialized) through to the end of .bss. The paging code
	   maps everything below this page-aligned boundary read-only (.text and
	   .rodata) and everything at or above it read-write. */
	. = ALIGN(4K);
	kernel_data_start = .;
	.data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VMA)
	{
		*(.data)
		*(.data.*)
	}

	/* Read-write data (uninitialized) and stack. */
	.bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VMA)
	{
		*(COMMON)
		*(.bss)
		*(.bss.*)
	}

	. = ALIGN(4K);
	kernel_virt_end = .;
	/* The physical extent is the virtual extent shifted back down by the
	   offset; the frame allocator reserves the kernel by this physical range. */
	kernel_phys_end = kernel_virt_end - KERNEL_VMA;

	/DISCARD/ :
	{
		*(.eh_frame)
		*(.debug_gdb_scripts)
		*(.comment)
		*(.note*)
	}

	/* The compiler may produce other sections, by default it will put them in
	   a segment with the same name. Simply add stuff here as needed. */
}
