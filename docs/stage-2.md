# Stage 2 - GDT + stack

Maps to `kfs2` of the kfs subject series from the 42 advanced curriculum.

## Mandatory goals

A kernel that:

1. Creates its own Global Descriptor Table (GDT).
2. Places the GDT at physical address `0x00000800`.
3. Defines descriptors for all required segments:
   - kernel code
   - kernel data
   - kernel stack
   - user code
   - user data
   - user stack
4. Installs the GDT with `lgdt`.
5. Reloads the segment registers after installing the GDT.
6. Reloads `cs` with a far jump or equivalent control transfer.
7. Keeps booting into Rust after the GDT switch.
8. Keeps the existing screen/`printk!` debug path working.
9. Implements a tool that prints the kernel stack in a human-friendly format.

## Interpretation notes

- The subject says to "declare the GDT to the BIOS". In this kernel context,
  the practical requirement is to make the CPU use our GDT by loading `gdtr`
  with `lgdt`; this is not a BIOS service call.
- The required user segments are defined now because the subject asks for them,
  but actual userspace execution is out of scope until later stages.
- The stack dump tool should be bounded. It must not blindly walk arbitrary
  memory and fault once paging and stricter memory handling arrive.

## Bonuses included in scope

These are useful enough to include if they do not obscure the required GDT work:

- **Typed descriptors and selectors** - use Rust newtypes for descriptor
  entries, descriptor tables, privilege levels, and segment selectors.
- **Named selectors** - expose clear constants for the required kernel and user
  selectors.
- **Stack dump formatting** - print stack addresses and word values in a stable
  format that can be compared across QEMU runs.

## Bonuses deferred

- **Minimal debugging shell** - depends on a real input path. Without keyboard
  interrupts, a shell would either be fake or polling-based code we are likely
  to throw away.
- **Reboot and halt commands** - useful later, but not required to prove the GDT
  and stack work.
- **Spinlock-protected console** - belongs after interrupt handling exists. The
  current single-core, interrupts-off bootstrap path can keep using the existing
  console model.

## Out of scope (will surface in later stages)

Paging, dynamic memory allocation, interrupts, keyboard input, task switching,
syscalls, and actual ring 3 execution.

## Done when

- `make run` boots the kernel in QEMU.
- The kernel switches from GRUB's initial descriptor setup to our own GDT.
- The GDT lives at `0x00000800`.
- All six required segment descriptors are present.
- Segment registers and `cs` are reloaded without crashing.
- The screen still prints through `printk!` after the GDT switch.
- A bounded stack dump can be printed in a human-readable address/value format.
