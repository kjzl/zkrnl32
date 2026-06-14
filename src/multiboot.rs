//! Parsing of the Multiboot 1 information GRUB hands the kernel.
//!
//! GRUB enters the kernel with `eax` holding [`BOOTLOADER_MAGIC`] and `ebx`
//! pointing at a [`MultibootInfo`] structure. This module turns that raw,
//! firmware-owned structure into typed, bounds-checked views - chiefly the
//! BIOS [memory map](MultibootInfo::memory_map), which the frame allocator
//! needs before it can hand out a single physical frame.

use crate::prelude::*;
use crate::vga::Color;

/// The value GRUB leaves in `eax` to prove a Multiboot-compliant boot.
pub const BOOTLOADER_MAGIC: u32 = 0x2BAD_B002;

const _: () = assert!(core::mem::offset_of!(MultibootInfo, mmap_addr) == 48);
const _: () = assert!(core::mem::offset_of!(MultibootInfo, mmap_length) == 44);

/// The Multiboot information structure GRUB hands the kernel in `ebx`.
///
/// Only the fields this kernel reads are named; the run of fields between
/// `flags` and the memory map is kept as opaque padding so the memory-map
/// fields land at their spec offsets.
#[repr(C)]
pub struct MultibootInfo {
    /// Bitset recording which of the later fields GRUB actually filled in.
    flags: u32,
    reserved: [u32; 10],
    /// Size in bytes of the memory-map buffer at `mmap_addr`.
    mmap_length: u32,
    /// Physical address of the first memory-map entry.
    mmap_addr: u32,
}

impl MultibootInfo {
    /// Bit 6 of `flags`: the memory map (`mmap_addr`/`mmap_length`) is valid.
    const FLAG_MMAP: u32 = 1 << 6;

    /// Returns an iterator over the BIOS memory map, or `None` if GRUB did not
    /// provide one.
    ///
    /// The flag check is what makes reading `mmap_addr`/`mmap_length` sound:
    /// without it, those fields hold undefined values.
    pub fn memory_map(&self) -> Option<MemoryMap<'_>> {
        if self.flags & Self::FLAG_MMAP == 0 {
            return None;
        }
        // SAFETY: with the mmap flag set, GRUB guarantees `mmap_length` bytes of
        // valid memory-map data at `mmap_addr`, mapped and unmodified for as long
        // as this borrow of the boot structures lives.
        let buf = unsafe {
            core::slice::from_raw_parts(self.mmap_addr as *const u8, self.mmap_length as usize)
        };
        Some(MemoryMap { buf, offset: 0 })
    }
}

/// One raw entry of the Multiboot memory map.
///
/// `#[repr(C, packed)]` pins the exact on-the-wire layout on every target.
/// Read its fields by value only.
#[repr(C, packed)]
struct MmapEntry {
    /// Bytes in this entry *after* this field; the stride to the next entry is
    /// `size + 4`.
    size: u32,
    /// Region start, as a 64-bit physical address.
    base_addr: u64,
    /// Region length in bytes, 64-bit.
    length: u64,
    /// Region type code; `1` is available RAM. See [`RegionKind`].
    kind: u32,
}

/// An iterator over the entries of the Multiboot memory map.
///
/// Walks the buffer entry by entry, advancing by each entry's
/// self-described `size + 4` rather than by [`MmapEntry`]'s own size.
pub struct MemoryMap<'a> {
    /// The memory-map buffer.
    buf: &'a [u8],
    /// Byte offset of the next entry within `buf`.
    offset: usize,
}

impl Iterator for MemoryMap<'_> {
    type Item = MemoryRegion;

    fn next(&mut self) -> Option<Self::Item> {
        // A whole entry's worth of bytes from the cursor, or `None` once a
        // truncated tail (or a stride that overran on bad data) leaves too few.
        let bytes = self.buf.get(self.offset..)?.get(..size_of::<MmapEntry>())?;

        // SAFETY: `bytes` is exactly `size_of::<MmapEntry>()` bytes borrowed from
        // the map buffer; `read_unaligned` needs no alignment, which the packed
        // layout requires.
        let entry = unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const MmapEntry) };

        // Stride by the entry's own size (+4 for the `size` field it excludes);
        // the `+ 4` keeps the step positive, so the walk always terminates.
        self.offset = self
            .offset
            .saturating_add(entry.size as usize + size_of::<u32>());

        Some(MemoryRegion {
            base: entry.base_addr,
            length: entry.length,
            kind: RegionKind::from_raw(entry.kind),
        })
    }
}

/// The kind of a memory-map region, as classified by the firmware.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegionKind {
    /// Type 1: available RAM, free for the kernel to allocate.
    Available,
    /// Type 3: holds ACPI tables; reclaimable once the kernel has read them.
    AcpiReclaimable,
    /// Type 4: firmware memory to preserve across sleep states (ACPI NVS).
    AcpiNvs,
    /// Type 5: RAM the firmware flagged as defective.
    Defective,
    /// Type 2, or any other code: reserved; must not be allocated.
    Reserved(u32),
}

impl RegionKind {
    fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Available,
            3 => Self::AcpiReclaimable,
            4 => Self::AcpiNvs,
            5 => Self::Defective,
            other => Self::Reserved(other),
        }
    }

    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// A single region of the physical address space.
#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    base: u64,
    length: u64,
    kind: RegionKind,
}

impl MemoryRegion {
    /// The physical start address, as reported (may be at or above 4 GiB).
    pub fn base(&self) -> u64 {
        self.base
    }

    /// The length in bytes, as reported.
    pub fn length(&self) -> u64 {
        self.length
    }

    /// The region's type classification.
    pub fn kind(&self) -> RegionKind {
        self.kind
    }

    /// Whether this is available RAM the kernel may allocate.
    pub fn is_available(&self) -> bool {
        self.kind.is_available()
    }

    /// The number of bytes of this region the 32-bit kernel can address: the
    /// region clamped to the low 4 GiB. Zero if it lies wholly at or above the
    /// 4 GiB limit.
    pub fn addressable_bytes(&self) -> u64 {
        /// One past the last byte a 32-bit kernel can address.
        const LIMIT: u64 = 1 << 32;

        if self.base >= LIMIT {
            return 0;
        }
        self.base.saturating_add(self.length).min(LIMIT) - self.base
    }
}

/// Prints the firmware memory map and the total usable RAM to the console.
///
/// A boot self-check: it makes the Multiboot hand-off and the map walk visible
/// before the frame allocator begins to trust either.
pub fn print_memory_map(info: &MultibootInfo) {
    let Some(map) = info.memory_map() else {
        printk_color!(Color::Yellow, "multiboot: no memory map provided\n");
        return;
    };

    printk!("multiboot memory map:\n");
    let mut usable_bytes: u64 = 0;
    for region in map {
        let base = region.base();
        let end = base.saturating_add(region.length());
        let kib = region.length() / 1024;
        printk!(
            "  [{base:#012x}, {end:#012x}) {kib:>9} KiB  {:?}\n",
            region.kind()
        );

        if region.is_available() {
            usable_bytes += region.addressable_bytes();
        }
    }
    printk!("usable RAM: {} KiB\n", usable_bytes / 1024);
}
