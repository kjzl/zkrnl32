//! 32-bit (non-PAE) page tables: the structures the MMU walks once paging is on.
//!
//! Translation is a two-level walk. A [`PageDirectory`] of 1024
//! [`PageDirectoryEntry`]s is indexed by the high 10 bits of a virtual address;
//! each entry either points at a [`PageTable`] of 1024 [`PageTableEntry`]s
//! (4 KiB pages, indexed by the middle 10 bits) or, with the page-size bit set,
//! maps one 4 MiB page directly.
//!
//! Both levels pack the same way: a frame address in the high 20 bits (a 4 MiB
//! page uses only the high 10) and [`PageFlags`] in the low 12. Those low 12
//! bits are free for control bits precisely because every table and frame is
//! 4 KiB-aligned, so the low 12 bits of their addresses are always zero.

use crate::memory::address::FrameNumber;
use crate::memory::address::PhysAddr;
use crate::memory::address::VirtAddr;
use crate::memory::frame::FrameBitmap;
use crate::memory::layout::KERNEL_VIRTUAL_BASE;
use crate::memory::layout::kernel_data_start_addr;
use crate::memory::layout::kernel_phys_start_addr;
use crate::memory::layout::kernel_virt_end_addr;
use crate::memory::layout::kernel_virt_start_addr;
use crate::stack::stack_guard_addr;

/// Bytes a 4 KiB page (or frame) spans.
const PAGE_SIZE: usize = 4096;
/// Entries in a page directory or page table.
const ENTRY_COUNT: usize = 1024;
/// Bytes of virtual address space one page table maps (`1024 * 4 KiB = 4 MiB`),
/// which is also the size and alignment of a single 4 MiB page.
const PAGE_TABLE_SPAN: usize = PAGE_SIZE * ENTRY_COUNT;

/// The low bits of an entry that hold its [`PageFlags`]. Equal to the page
/// offset width, since 4 KiB alignment frees exactly those bits in every
/// frame address.
const FLAGS_MASK: u32 = PAGE_SIZE as u32 - 1;
/// The high bits of an entry that hold its frame address.
const ADDRESS_MASK: u32 = !FLAGS_MASK;

/// Control bits shared by page-directory and page-table entries.
///
/// They occupy the low 12 bits of an entry. [`PRESENT`](Self::PRESENT),
/// [`WRITABLE`](Self::WRITABLE), and [`USER`](Self::USER) are the rights the
/// kernel sets deliberately; [`ACCESSED`](Self::ACCESSED) and
/// [`DIRTY`](Self::DIRTY) are set by the CPU and only read back.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(u32);

impl PageFlags {
    /// Bit 0 - the mapping is present; clearing it makes any access fault.
    pub const PRESENT: Self = Self(1);
    /// Bit 1 - writable; when clear the page is read-only (for ring 0 this is
    /// only enforced while `CR0.WP` is set).
    pub const WRITABLE: Self = Self(1 << 1);
    /// Bit 2 - user-accessible (ring 3); when clear the page is supervisor-only.
    pub const USER: Self = Self(1 << 2);
    /// Bit 3 - write-through rather than write-back caching for this page.
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    /// Bit 4 - disable caching for this page.
    pub const CACHE_DISABLE: Self = Self(1 << 4);
    /// Bit 5 - set by the CPU on the first access through this mapping.
    pub const ACCESSED: Self = Self(1 << 5);
    /// Bit 6 - set by the CPU on the first write through this mapping.
    pub const DIRTY: Self = Self(1 << 6);
    /// Bit 8 - global: the TLB entry is not flushed on a `CR3` reload while
    /// `CR4.PGE` is set. Intended for kernel pages common to every address space.
    pub const GLOBAL: Self = Self(1 << 8);

    /// Present, supervisor, read-only.
    pub const KERNEL_RO: Self = Self::PRESENT;
    /// Present, supervisor, read-write.
    pub const KERNEL_RW: Self = Self::PRESENT.union(Self::WRITABLE);
    /// Present, user, read-only.
    pub const USER_RO: Self = Self::PRESENT.union(Self::USER);
    /// Present, user, read-write.
    pub const USER_RW: Self = Self::PRESENT.union(Self::WRITABLE).union(Self::USER);

    /// No bits set: an absent entry with no rights.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// The raw bit pattern, ready to OR into an entry alongside a frame address.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Recovers the flags from a raw entry, discarding its address field.
    const fn from_bits(bits: u32) -> Self {
        Self(bits & FLAGS_MASK)
    }

    /// The union of two flag sets. A `const`-callable form of `|`.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every bit set in `other` is also set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for PageFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

/// The top-level table the MMU consults first, indexed by the high 10 bits of
/// a virtual address.
#[repr(C, align(4096))]
pub(super) struct PageDirectory([PageDirectoryEntry; ENTRY_COUNT]);

/// The second-level table, indexed by the middle 10 bits of a virtual address;
/// each entry maps one 4 KiB page.
#[repr(C, align(4096))]
pub(super) struct PageTable([PageTableEntry; ENTRY_COUNT]);

/// One page-table entry: maps a single 4 KiB page.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(super) struct PageTableEntry(u32);

/// One page-directory entry: either points at a [`PageTable`] or, with the
/// page-size bit set, maps a single 4 MiB page.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(super) struct PageDirectoryEntry(u32);

impl PageTableEntry {
    /// An absent entry: no frame, no rights.
    pub(super) const EMPTY: Self = Self(0);

    /// Maps `frame` with `flags`. Pass [`PageFlags::PRESENT`] in `flags` for the
    /// mapping to be live.
    pub(super) const fn new(frame: FrameNumber, flags: PageFlags) -> Self {
        Self(frame.base_addr().raw() | flags.bits())
    }

    /// The flag bits of this entry.
    pub(super) const fn flags(self) -> PageFlags {
        PageFlags::from_bits(self.0)
    }

    /// The frame this entry maps. Meaningful only when [`is_present`](Self::is_present).
    pub(super) const fn frame(self) -> FrameNumber {
        PhysAddr::new(self.0 & ADDRESS_MASK).frame_floor()
    }

    /// Whether the present bit is set.
    pub(super) const fn is_present(self) -> bool {
        self.flags().contains(PageFlags::PRESENT)
    }
}

impl PageDirectoryEntry {
    /// Bit 7, the page-size bit: set means the entry maps a 4 MiB page directly
    /// instead of pointing at a page table. Honoured only while `CR4.PSE` is set.
    const PAGE_SIZE_BIT: u32 = 1 << 7;

    /// An absent entry.
    pub(super) const EMPTY: Self = Self(0);

    /// Points the entry at the page table stored in `frame`, with `flags`.
    pub(super) const fn for_table(frame: FrameNumber, flags: PageFlags) -> Self {
        Self(frame.base_addr().raw() | flags.bits())
    }

    /// Maps a single 4 MiB page at `base` with `flags`; the page-size bit is set
    /// for you. `base` must be 4 MiB-aligned, so its low 22 bits are the page's
    /// reserved/flag space rather than address.
    pub(super) const fn for_huge_page(base: PhysAddr, flags: PageFlags) -> Self {
        debug_assert!(
            (base.raw() as usize).is_multiple_of(PAGE_TABLE_SPAN),
            "a 4 MiB page must be aligned to a 4 MiB boundary"
        );
        Self(base.raw() | Self::PAGE_SIZE_BIT | flags.bits())
    }

    /// The flag bits of this entry.
    pub(super) const fn flags(self) -> PageFlags {
        PageFlags::from_bits(self.0)
    }

    /// Whether this entry maps a 4 MiB page rather than pointing at a page table.
    pub(super) const fn is_huge(self) -> bool {
        self.0 & Self::PAGE_SIZE_BIT != 0
    }

    /// Whether the present bit is set.
    pub(super) const fn is_present(self) -> bool {
        self.flags().contains(PageFlags::PRESENT)
    }

    /// The frame holding the page table this entry points at. Meaningful only
    /// when the entry is present and not [`huge`](Self::is_huge).
    pub(super) const fn table_frame(self) -> FrameNumber {
        PhysAddr::new(self.0 & ADDRESS_MASK).frame_floor()
    }
}

impl PageDirectory {
    /// Builds the bootstrap directory the boot stub loads into `CR3` to bring
    /// paging up, before any page tables exist.
    ///
    /// Two 4 MiB pages suffice because the whole kernel image fits in the first
    /// 4 MiB of physical memory: entry 0 identity-maps `[0, 4 MiB)` so the low
    /// boot code survives the `CR0.PG` flip, and the entry for the high half
    /// ([`KERNEL_VIRTUAL_BASE`]) maps onto the same physical `[0, 4 MiB)`.
    const fn bootstrap() -> Self {
        let mut entries = [PageDirectoryEntry::EMPTY; ENTRY_COUNT];
        entries[0] = PageDirectoryEntry::for_huge_page(PhysAddr::new(0), PageFlags::KERNEL_RW);
        entries[KERNEL_VIRTUAL_BASE / PAGE_TABLE_SPAN] =
            PageDirectoryEntry::for_huge_page(PhysAddr::new(0), PageFlags::KERNEL_RW);
        Self(entries)
    }
}

/// The bootstrap page directory the boot stub installs before turning paging
/// on; see [`PageDirectory::bootstrap`].
///
/// It lives in the identity-linked `.boot.data` section, so its link address is
/// also its physical address - exactly what `CR3` wants - and the boot stub
/// loads it by the `BOOT_PAGE_DIRECTORY` symbol. `PageDirectory`'s 4 KiB `repr`
/// alignment satisfies `CR3`'s alignment requirement. It is `mut` because the
/// CPU sets the accessed and dirty bits in the live entries; no Rust code reads
/// it, so no reference is ever taken.
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".boot.data")]
static mut BOOT_PAGE_DIRECTORY: PageDirectory = PageDirectory::bootstrap();

/// The directory's last slot points at the directory itself, so every page
/// table is reachable through the top 4 MiB of virtual space once paging is on.
const RECURSIVE_INDEX: usize = ENTRY_COUNT - 1;
/// One past the highest physical address the bootstrap map identity-maps. Every
/// frame backing the new directory must lie below it to be writable here.
const BOOTSTRAP_IDENTITY_LIMIT: usize = PAGE_TABLE_SPAN;
/// `CR0.WP`: makes ring 0 honour the read-only bit, so the kernel cannot scribble
/// over its own `.text`/`.rodata`.
const CR0_WP: u32 = 1 << 16;

/// Builds the real 4 KiB-granular page directory and switches to it, retiring
/// the bootstrap huge-page map.
///
/// The directory is assembled while the bootstrap map is still active, so every
/// paging frame is reached through its low identity address and must lie in the
/// low 4 MiB (asserted in [`bootstrap_frame_ptr`]). It maps the kernel high half
/// with per-section rights, keeps the physical memory below the kernel identity-
/// mapped for the GDT and VGA, and points its last slot at itself. The closing
/// `CR3` load hands over atomically - the new directory already maps the running
/// code, the stack, and VGA - and `CR0.WP` then enforces the read-only pages.
pub(super) fn init(allocator: &mut FrameBitmap) {
    let pd_frame = alloc_frame(allocator);
    zero_frame(pd_frame);
    // SAFETY: pd_frame is freshly allocated, identity-mapped, 4 KiB-aligned and
    // referenced by nothing else, so this borrow over the zeroed frame is sound.
    let pd = unsafe { &mut *bootstrap_frame_ptr::<PageDirectory>(pd_frame) };

    pd.0[RECURSIVE_INDEX] = PageDirectoryEntry::for_table(pd_frame, PageFlags::KERNEL_RW);

    let writable_start = kernel_data_start_addr();
    let guard = stack_guard_addr();
    for virt in (kernel_virt_start_addr()..kernel_virt_end_addr()).step_by(PAGE_SIZE) {
        // Leave the stack guard page absent so a stack overflow faults here
        // rather than corrupting the .bss below the stack.
        if virt == guard {
            continue;
        }
        let flags = if virt < writable_start {
            PageFlags::KERNEL_RO
        } else {
            PageFlags::KERNEL_RW
        };
        let phys = PhysAddr::new((virt - KERNEL_VIRTUAL_BASE) as u32);
        map_bootstrap(pd, allocator, VirtAddr::new(virt as u32), phys, flags);
    }

    // Physical memory below the kernel stays identity-mapped so the GDT (0x800)
    // and the VGA buffer (0xB8000) survive the switch; capped below the kernel
    // so it never aliases the image's read-only mapping.
    for phys in (0..kernel_phys_start_addr()).step_by(PAGE_SIZE) {
        let addr = VirtAddr::new(phys as u32);
        map_bootstrap(
            pd,
            allocator,
            addr,
            PhysAddr::new(phys as u32),
            PageFlags::KERNEL_RW,
        );
    }

    // SAFETY: the new directory maps the running high-half code, the high-half
    // stack, low VGA/GDT, and itself, so execution survives the CR3 load (which
    // also flushes the TLB); setting CR0.WP then makes the read-only kernel
    // pages stick.
    unsafe {
        core::arch::asm!(
            "mov cr3, {pd:e}",
            "mov {tmp:e}, cr0",
            "or {tmp:e}, {wp}",
            "mov cr0, {tmp:e}",
            pd = in(reg) pd_frame.base_addr().raw(),
            tmp = out(reg) _,
            wp = const CR0_WP,
            options(nostack),
        );
    }
}

/// Allocates one frame for paging structures, leaking the run: the directory
/// and its tables are permanent.
fn alloc_frame(allocator: &mut FrameBitmap) -> FrameNumber {
    allocator
        .allocate_one()
        .expect("out of physical frames while building the page directory")
        .start()
}

/// A writable pointer to a paging frame, addressed through the bootstrap
/// identity map of the low 4 MiB. Valid only before the `CR3` switch.
///
/// # Panics
///
/// Panics if the frame lies above the bootstrap identity map, where it could
/// not be initialised without first mapping it.
fn bootstrap_frame_ptr<T>(frame: FrameNumber) -> *mut T {
    let phys = frame.base_addr().raw() as usize;
    assert!(
        phys < BOOTSTRAP_IDENTITY_LIMIT,
        "paging frame at {phys:#x} lies above the bootstrap identity map"
    );
    phys as *mut T
}

/// Zeroes a freshly allocated paging frame in place, leaving every entry
/// absent. A byte fill avoids materialising a 4 KiB table value on the stack,
/// which the boot stack cannot spare.
fn zero_frame(frame: FrameNumber) {
    let ptr = bootstrap_frame_ptr::<u8>(frame);
    // SAFETY: identity-mapped, frame-aligned, unaliased; fills exactly the frame.
    unsafe { ptr.write_bytes(0, PAGE_SIZE) };
}

/// Maps `virt` to `phys` with `flags` in the directory under construction,
/// allocating the page table for its directory slot on first use.
fn map_bootstrap(
    pd: &mut PageDirectory,
    allocator: &mut FrameBitmap,
    virt: VirtAddr,
    phys: PhysAddr,
    flags: PageFlags,
) {
    let index = virt.pd_index();
    let table_frame = if pd.0[index].is_present() {
        pd.0[index].table_frame()
    } else {
        let frame = alloc_frame(allocator);
        zero_frame(frame);
        pd.0[index] = PageDirectoryEntry::for_table(frame, PageFlags::KERNEL_RW);
        frame
    };
    let table = bootstrap_frame_ptr::<PageTable>(table_frame);
    // SAFETY: identity-mapped and aligned; this page table is reached only here,
    // not through the `&mut pd` borrow, so the write does not alias.
    unsafe { (*table).0[virt.pt_index()] = PageTableEntry::new(phys.frame_floor(), flags) };
}

impl core::fmt::Debug for PageFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        const NAMED: [(PageFlags, &str); 8] = [
            (PageFlags::PRESENT, "PRESENT"),
            (PageFlags::WRITABLE, "WRITABLE"),
            (PageFlags::USER, "USER"),
            (PageFlags::WRITE_THROUGH, "WRITE_THROUGH"),
            (PageFlags::CACHE_DISABLE, "CACHE_DISABLE"),
            (PageFlags::ACCESSED, "ACCESSED"),
            (PageFlags::DIRTY, "DIRTY"),
            (PageFlags::GLOBAL, "GLOBAL"),
        ];

        write!(f, "PageFlags(")?;
        let mut first = true;
        for (flag, name) in NAMED {
            if self.contains(flag) {
                write!(f, "{}{name}", if first { "" } else { " | " })?;
                first = false;
            }
        }
        if first {
            write!(f, "empty")?;
        }
        write!(f, ")")
    }
}

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageTableEntry")
            .field("frame", &self.frame())
            .field("flags", &self.flags())
            .finish()
    }
}

impl core::fmt::Debug for PageDirectoryEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_huge() {
            return write!(f, "PageDirectoryEntry(huge 4 MiB, {:?})", self.flags());
        }
        f.debug_struct("PageDirectoryEntry")
            .field("table_frame", &self.table_frame())
            .field("flags", &self.flags())
            .finish()
    }
}
