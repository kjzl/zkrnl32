use crate::prelude::*;

#[macro_export]
macro_rules! current_esp {
    () => {{
        let esp: usize;
        unsafe {
            core::arch::asm!(
                "mov {}, esp",
                out(reg) esp,
                options(nomem, nostack, preserves_flags),
            );
        }
        esp
    }};
}

#[macro_export]
macro_rules! dump_stack {
    ($dword_count:expr) => {{ $crate::stack::dump_stack_from($crate::current_esp!(), $dword_count) }};
}

pub fn dump_stack_from(addr: usize, dword_count: usize) {
    printk!("stack dump: ");
    let misalignment = addr % 4;
    let addr = addr - misalignment;
    let sb = stack_bottom_addr();
    let st = stack_top_addr();
    if addr < sb || addr >= st {
        printk!("{addr:#010x} not within stack bounds {sb:#010x}-{st:#010x}\n");
        return;
    }
    let available_dwords = (st - addr) / 4;
    let dword_count = dword_count.min(available_dwords);
    printk!("{dword_count}/{available_dwords}");
    if misalignment != 0 {
        printk!(" misaligned start address {:#010x}", addr + misalignment);
    }
    printk!("\n");
    for i in 0..dword_count {
        let addr = addr + i * 4;
        let value = unsafe { (addr as *const u32).read_volatile() };
        printk!("{:#010x}: {:#010x}\n", addr, value);
    }
}

unsafe extern "C" {
    static stack_guard: u8;
    static stack_bottom: u8;
    static stack_top: u8;
}

/// Address of the stack guard page: one page below `stack_bottom` that the real
/// page directory leaves unmapped, so a stack overflow faults here instead of
/// corrupting the `.bss` below the stack.
pub fn stack_guard_addr() -> usize {
    (&raw const stack_guard) as usize
}

pub fn stack_bottom_addr() -> usize {
    (&raw const stack_bottom) as usize
}

pub fn stack_top_addr() -> usize {
    (&raw const stack_top) as usize
}
