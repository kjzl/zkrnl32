//! Physical frame allocation.
//!
//! A bitmap allocator for the 4 KiB physical frames of the full 4 GiB i386
//! address space: one bit per frame, `1` meaning free, `0` occupied. The
//! 128 KiB bitmap lives in kernel `.bss`.
//!
//! Every frame starts occupied. [`FrameBitmap::init`] frees the regions the
//! multiboot memory map reports as usable; only frames published that way
//! can ever be handed out.
//!
//! All mutation goes through `&mut self`, so the allocator itself carries no
//! synchronisation; see [`FRAME_ALLOCATOR`].

use core::num::NonZeroUsize;

use crate::memory::address::FrameNumber;
use crate::memory::address::PhysAddr;
use crate::memory::layout::kernel_end_addr;
use crate::memory::layout::kernel_start_addr;
use crate::multiboot::MemoryMap;
use crate::utils::bits::WORD_BITS;
use crate::utils::bits::clipped_bit_range_mask;

const FRAME_SIZE: usize = 4096;
/// Number of 4 KiB frames in the 4 GiB i386 physical address space.
const FRAME_COUNT: usize = 1 << 20;
/// Number of words in the bitmap.
const WORD_COUNT: usize = FRAME_COUNT / WORD_BITS;

const _: () = assert!(
    FRAME_COUNT.is_multiple_of(WORD_BITS),
    "the bitmap must cover FRAME_COUNT exactly, with no partial trailing word"
);

/// The kernel-wide physical frame allocator.
///
/// ```ignore
/// // SAFETY: single-core, no interrupts, no other borrow live.
/// let allocator = unsafe { &mut *&raw mut FRAME_ALLOCATOR };
/// ```
// TODO: wrap in an interrupt-safe lock once one exists and drop `static mut`.
pub(super) static mut FRAME_ALLOCATOR: FrameBitmap = FrameBitmap::new();

/// Owned, non-empty run of contiguous physical frames issued by the
/// allocator.
///
/// The fields are private, so a run can only be obtained from an allocation;
/// callers cannot fabricate one over frames they do not own. There is no
/// `Drop` impl - a run holds no reference back to the allocator - so dropping
/// one silently leaks its frames.
#[must_use = "dropping a FrameRun leaks its frames; return it via FrameBitmap::free"]
#[derive(Debug)]
pub struct FrameRun {
    start: FrameNumber,
    len: NonZeroUsize,
}

impl FrameRun {
    #[inline(always)]
    fn new(start: usize, len: NonZeroUsize) -> Self {
        debug_assert!(start < FRAME_COUNT);
        debug_assert!(len.get() <= FRAME_COUNT - start);
        Self {
            start: FrameNumber::new(start),
            len,
        }
    }

    /// Returns the first frame of the run.
    #[inline(always)]
    pub const fn start(&self) -> FrameNumber {
        self.start
    }

    /// Returns the number of frames in the run.
    #[inline(always)]
    pub const fn len(&self) -> NonZeroUsize {
        self.len
    }

    /// Returns the exclusive end of the run as a raw frame index.
    #[inline(always)]
    pub const fn end_exclusive(&self) -> usize {
        self.start.index() + self.len.get()
    }
}

/// Bitmap-backed physical frame allocator.
pub(super) struct FrameBitmap {
    /// One bit per frame: `1` free, `0` occupied. Zero-initialised, so every
    /// frame starts occupied until [`Self::init`] publishes usable memory.
    bitmap: [u32; WORD_COUNT],
    /// Word index to start the next free-run search from. Purely a bias:
    /// [`Self::find_free_run`] falls back to a full scan, so a stale hint
    /// costs time, never frames.
    // INVARIANT: search_hint <= WORD_COUNT.
    search_hint: usize,
}

impl FrameBitmap {
    const fn new() -> Self {
        Self {
            bitmap: [0; WORD_COUNT],
            search_hint: 0,
        }
    }

    /// Builds the initial picture of usable physical memory.
    pub(super) fn init(&mut self, mmap: MemoryMap<'_>) {
        for region in mmap {
            let bytes = region.addressable_bytes() as usize;
            if !region.is_available() || bytes == 0 {
                continue;
            }
            let base = region.base() as usize;
            let start = FrameNumber::new(base.div_ceil(FRAME_SIZE));
            let end = (base + bytes) / FRAME_SIZE;
            if let Some(len) = NonZeroUsize::new(end.saturating_sub(start.index())) {
                self.mark_free(start, len);
            };
        }

        self.reserve_bytes(0, FRAME_SIZE); // GDT, etc. ...

        let kstart = kernel_start_addr();
        self.reserve_bytes(kstart, kernel_end_addr() - kstart);
    }

    /// Marks every frame touched by the byte range `[addr, addr + len)` occupied.
    /// Rounds *outward* so a partially-used frame is never left free.
    fn reserve_bytes(&mut self, addr: usize, len: usize) {
        let start = PhysAddr::new(addr as u32).frame_floor();
        let end = PhysAddr::new((addr + len) as u32).frame_ceil();
        self.set_occupied(start, end.index() - start.index());
    }

    /// Marks `count` frames starting at `start` as free.
    ///
    /// Intended for [`Self::init`] to publish usable memory: unlike
    /// [`Self::free`] it is idempotent and performs no double-free check.
    ///
    /// # Panics
    ///
    /// Panics if the range reaches past the last frame.
    #[inline(always)]
    fn mark_free(&mut self, start: FrameNumber, count: NonZeroUsize) {
        self.set_free(start, count.get());
        self.search_hint = self.search_hint.min(start.index() / WORD_BITS);
    }

    /// Allocates a single frame.
    pub(super) fn allocate_one(&mut self) -> Option<FrameRun> {
        self.allocate(NonZeroUsize::MIN)
    }

    /// Allocates exactly `count` contiguous frames, or `None` if no free gap
    /// that large exists.
    pub(super) fn allocate(&mut self, count: NonZeroUsize) -> Option<FrameRun> {
        if count.get() > FRAME_COUNT {
            return None;
        }

        let start = self.find_free_run(count.get())?;
        self.set_occupied(FrameNumber::new(start), count.get());
        // Resume the next search past this run. Smaller gaps may survive
        // below `start`; the full-scan fallback still reaches them.
        self.search_hint = (start + count.get()) / WORD_BITS;
        Some(FrameRun::new(start, count))
    }

    /// Releases a run back to the allocator.
    ///
    /// # Panics
    ///
    /// Panics if any frame of the run is already free. The check runs before
    /// any bit is flipped, so a double free halts with the bitmap intact
    /// rather than half-updated.
    pub(super) fn free(&mut self, run: FrameRun) {
        let (start, len) = (run.start.index(), run.len.get());

        if self.any_free_in(start, len) {
            panic!("double free of physical frames");
        }
        self.set_free(FrameNumber::new(start), len);

        // Pull the hint back so the next search sees the freed frames; never
        // raise it, so allocations stay packed toward low frames.
        self.search_hint = self.search_hint.min(start / WORD_BITS);
    }

    /// Finds the first run of `count` contiguous free frames.
    ///
    /// Scans from the hint first. A run below the hint - or straddling it -
    /// is invisible to that pass, so a miss falls back to scanning the full
    /// bitmap before giving up.
    #[inline(always)]
    fn find_free_run(&self, count: usize) -> Option<usize> {
        self.scan_for_run(self.search_hint * WORD_BITS, count)
            .or_else(|| self.scan_for_run(0, count))
    }

    /// Scans `[from, FRAME_COUNT)` for `count` contiguous free frames,
    /// returning the index of the first frame of the first such run.
    ///
    /// Frame-by-frame for clarity. If allocation ever shows up on a profile,
    /// working at the word level can skip 32 occupied frames per load.
    #[inline(always)]
    fn scan_for_run(&self, from: usize, count: usize) -> Option<usize> {
        let mut run_start = 0;
        let mut run_len = 0;

        for frame in from..FRAME_COUNT {
            if !self.is_free(frame) {
                run_len = 0;
                continue;
            }
            if run_len == 0 {
                run_start = frame;
            }
            run_len += 1;
            if run_len == count {
                return Some(run_start);
            }
        }
        None
    }

    /// Returns whether `frame` is free.
    #[inline(always)]
    fn is_free(&self, frame: usize) -> bool {
        self.bitmap[frame / WORD_BITS] & (1 << (frame % WORD_BITS)) != 0
    }

    /// Returns whether any frame in `[start, start + len)` is free.
    #[inline(always)]
    fn any_free_in(&self, start: usize, len: usize) -> bool {
        word_masks(start, len).any(|(word_i, mask)| self.bitmap[word_i] & mask != 0)
    }

    /// Sets the bits of `[start, start + len)`, marking the frames free.
    #[inline(always)]
    fn set_free(&mut self, start: FrameNumber, len: usize) {
        for (word_i, mask) in word_masks(start.index(), len) {
            self.bitmap[word_i] |= mask;
        }
    }

    /// Clears the bits of `[start, start + len)`, marking the frames
    /// occupied.
    #[inline(always)]
    fn set_occupied(&mut self, start: FrameNumber, len: usize) {
        for (word_i, mask) in word_masks(start.index(), len) {
            self.bitmap[word_i] &= !mask;
        }
    }
}

/// Yields `(word index, in-word mask)` pairs covering the frame range
/// `[start, start + len)`, splitting it along word boundaries.
#[inline(always)]
fn word_masks(start: usize, len: usize) -> impl Iterator<Item = (usize, u32)> {
    let mut word_i = start / WORD_BITS;
    let mut bit = start % WORD_BITS;
    let mut remaining = len;

    core::iter::from_fn(move || {
        if remaining == 0 {
            return None;
        }
        let (mask, taken) = clipped_bit_range_mask(bit, remaining);
        let item = (word_i, mask);
        remaining -= taken;
        word_i += 1;
        bit = 0;
        Some(item)
    })
}
