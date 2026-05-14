//! Volatile operations utilities

/// Copies `count` elements `T` from `src` to `dst` using only volatile read/write.
///
/// Overlapping regions are handled with `memmove` semantics.
///
/// # Safety
///
/// `src` must be valid for reads of `count` elements, and `dst` must be valid
/// for writes of `count` elements. Both regions must be suitable for volatile
/// access. If the regions overlap, they must still be part of the same allocated
/// object or memory-mapped region so that comparing and offsetting the pointers
/// is meaningful for the target.
#[inline(always)]
pub unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize) {
    // SAFETY: the safety contract for `copy` must be upheld by the caller.
    unsafe {
        let src_addr = src.addr();
        let dst_addr = dst.addr();
        let byte_count = count * core::mem::size_of::<T>();

        if dst_addr > src_addr && dst_addr < src_addr + byte_count {
            for i in (0..count).rev() {
                let tmp = src.add(i).read_volatile();
                dst.add(i).write_volatile(tmp);
            }
        } else {
            for i in 0..count {
                let tmp = src.add(i).read_volatile();
                dst.add(i).write_volatile(tmp);
            }
        }
    }
}
