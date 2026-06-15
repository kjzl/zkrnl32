//! Typed physical and virtual addresses, and the 4 KiB frame/page coordinates
//! they decompose into.

/// A byte address in the physical address space.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(u32);

/// A byte address in the virtual (linear) address space.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(u32);

/// Index of a 4 KiB frame in physical memory.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameNumber(usize);

/// Index of a 4 KiB page in virtual memory.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageNumber(usize);

impl PhysAddr {
    pub(super) const fn new(addr: u32) -> Self {
        Self(addr)
    }

    pub(super) const fn raw(self) -> u32 {
        self.0
    }

    /// The frame this address falls in (rounds down).
    pub(super) const fn frame_floor(self) -> FrameNumber {
        FrameNumber((self.0 / 4096) as usize)
    }

    /// First whole frame at or after this address, as an exclusive end index
    /// (rounds up). Stays in index space, so it can't overflow the way rounding
    /// a byte address up to the 4 GiB boundary would.
    pub(super) const fn frame_ceil(self) -> FrameNumber {
        FrameNumber(self.0.div_ceil(4096) as usize)
    }

    pub(super) const fn is_frame_aligned(self) -> bool {
        self.0.is_multiple_of(4096)
    }
}

impl VirtAddr {
    pub(super) const fn new(addr: u32) -> Self {
        Self(addr)
    }

    pub(super) const fn raw(self) -> u32 {
        self.0
    }

    pub(super) fn as_ptr(&self) -> *const u8 {
        self.0 as *const u8
    }

    pub(super) fn as_mut_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }

    /// The page this address falls in (rounds down).
    pub(super) const fn page_floor(self) -> PageNumber {
        PageNumber((self.0 / 4096) as usize)
    }

    /// First whole page at or after this address, as an exclusive end index
    /// (rounds up); overflow-safe, unlike rounding the byte address.
    pub(super) const fn page_ceil(self) -> PageNumber {
        PageNumber(self.0.div_ceil(4096) as usize)
    }

    /// Page-directory index: the top 10 bits.
    pub(super) const fn pd_index(self) -> usize {
        (self.0 >> 22) as usize
    }

    /// Page-table index: the middle 10 bits.
    pub(super) const fn pt_index(self) -> usize {
        ((self.0 >> 12) & 0x3FF) as usize
    }

    /// Byte offset within the page: the low 12 bits.
    pub(super) const fn offset(self) -> usize {
        (self.0 & 0xFFF) as usize
    }

    pub(super) const fn from_indices(pd_index: usize, pt_index: usize, offset: usize) -> Self {
        Self(((pd_index << 22) | (pt_index << 12) | offset) as u32)
    }

    pub(super) const fn is_page_aligned(self) -> bool {
        self.0.is_multiple_of(4096)
    }
}

impl FrameNumber {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }

    pub(super) const fn base_addr(self) -> PhysAddr {
        PhysAddr((self.0 * 4096) as u32)
    }
}

impl PageNumber {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }

    pub(super) const fn base_addr(self) -> VirtAddr {
        VirtAddr((self.0 * 4096) as u32)
    }
}

impl core::fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysAddr({:#010x})", self.0)
    }
}

impl core::fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtAddr({:#010x})", self.0)
    }
}
