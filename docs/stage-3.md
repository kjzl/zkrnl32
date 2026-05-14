# Stage 3 - memory

Maps to `kfs3` of the kfs subject series from the 42 advanced curriculum.

## Mandatory goals

A kernel that:

1. Enables 32-bit paging.
2. Creates page directory and page table structures for `i386`.
3. Loads `cr3` with the active page directory.
4. Enables paging through `cr0`.
5. Keeps the kernel alive across the paging switch.
6. Defines kernel-space and user-space virtual address ranges.
7. Represents page rights, at minimum:
   - present
   - writable/read-only
   - supervisor/user
8. Implements functions to create, get, map, and unmap memory pages.
9. Implements physical-memory allocation, free, and size retrieval for
   variable-sized allocations.
10. Implements virtual-memory allocation, free, and size retrieval for
    variable-sized allocations.
11. Provides the subject-required physical memory helpers:
    - `kmalloc`
    - `kfree`
    - `ksize`
    - `kbrk`
12. Provides the subject-required virtual memory helpers:
    - `vmalloc`
    - `vfree`
    - `vsize`
    - `vbrk`
13. Handles kernel panics with print-and-stop behavior.
14. Distinguishes fatal and non-fatal panic/error situations.

## Allocation contract

The subject requires allocation, free, and size retrieval for a variable in both
physical and virtual memory. That means the public allocator behavior must be
variable-sized, even if the backing implementation reserves whole frames or
whole pages internally.

For each successful allocation, the kernel must record allocation metadata so
that:

- the allocation can be freed by its returned pointer/address;
- the allocation's size can be retrieved later;
- freeing an unknown or already freed allocation is detected as an error;
- the allocator can distinguish the requested variable size from any internal
  rounded backing size.

For this project, `ksize` and `vsize` should return the requested variable size,
not the rounded frame/page span. The rounded backing size may be exposed through
separate debugging output later if it becomes useful.

## Paging model

Stage 3 uses classic non-PAE 32-bit paging:

- one page directory contains 1024 entries;
- one page table contains 1024 entries;
- a normal page is 4 KiB;
- a page table maps 4 MiB;
- a page directory can describe 4 GiB of virtual address space.

The initial mapping should be conservative:

- identity-map the low memory needed to survive the paging switch;
- map the kernel image and statically reserved kernel data;
- reserve a kernel virtual allocation area;
- reserve user-space ranges without executing userspace yet.

## Bonuses included in scope

These are useful if they remain small and directly support the mandatory memory
work:

- **Typed addresses** - use newtypes for physical addresses, virtual addresses,
  frames, pages, and allocation sizes.
- **Boot diagnostics** - print a compact memory self-check after paging and
  allocation are online.
- **Allocator invariants** - validate alignment, double-free detection, and
  page-right expectations in debug output.

## Bonuses deferred

- **Small-object heap sophistication** - slabs, bins, coalescing heaps, and
  sub-page reuse can come later. Stage 3 may back variable-sized allocations
  with whole pages or frames as long as it records and reports variable sizes
  correctly.
- **Swapping/disk-backed memory** - mentioned as motivation in the subject text,
  but not required for this stage.
- **Real process isolation** - define user-space ranges and page rights now;
  actual userspace execution belongs to later stages.
- **Copy-on-write and demand paging** - not required for the first paging and
  allocation implementation.

## Out of scope (will surface in later stages)

Interrupt-driven page fault handling, multitasking, ELF loading, syscalls,
userspace process startup, filesystem-backed memory, and advanced heap
allocation policies.

## Done when

- `make run` boots the kernel in QEMU.
- Paging is enabled and the kernel continues executing.
- Kernel and reserved user virtual ranges are defined.
- Pages can be created, retrieved, mapped, and unmapped with explicit rights.
- A physical allocation can be created for an arbitrary requested byte size,
  freed, and queried for its requested size.
- A virtual allocation can be created for an arbitrary requested byte size,
  accessed, freed, and queried for its requested size.
- Unknown or double frees are reported as non-fatal errors where possible.
- Fatal memory failures panic with useful output and halt.
- Boot-time diagnostics demonstrate both physical and virtual variable
  allocation paths.
