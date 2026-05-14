//! Write into the Vga buffer

use core::fmt::{self, Write};

const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
/// Shared VGA device
pub static mut VGA_WRITER: VgaWriter = VgaWriter::new(); // FIXME: Enable safe shared access via a Spinlock

/// Colors the VGA device can print
#[allow(missing_docs)]
#[repr(u8)]
pub enum Color {
    Black = 0x0,
    Blue = 0x1,
    Green = 0x2,
    Cyan = 0x3,
    Red = 0x4,
    Magenta = 0x5,
    Brown = 0x6,
    LightGray = 0x7,
    DarkGray = 0x8,
    LightBlue = 0x9,
    LightGreen = 0xa,
    LightCyan = 0xb,
    LightRed = 0xc,
    Pink = 0xd,
    Yellow = 0xe,
    White = 0xf,
}

impl Color {
    /// self on a colored background
    pub fn on(self, bg: Self) -> u8 {
        (bg as u8) << 4 | (self as u8)
    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}

/// Writes formatted text via `VGA_WRITER`
pub fn _print(args: fmt::Arguments<'_>) {
    #[expect(static_mut_refs)]
    let writer = unsafe { &mut VGA_WRITER };
    writer.write_fmt(args).ok();
}

/// Temporarily changes the color `VGA_WRITER` uses and writes formatted text
pub fn _print_color(color: u8, args: fmt::Arguments<'_>) {
    #[expect(static_mut_refs)]
    let writer = unsafe { &mut VGA_WRITER };

    let prev_color = writer.color;
    writer.color = color;
    writer.write_fmt(args).ok();
    writer.color = prev_color;
}

/// VGA Bookkeeping
#[derive(Debug)]
pub struct VgaWriter {
    col: usize,
    row: usize,
    color: u8,
}

impl VgaWriter {
    const fn new() -> Self {
        Self {
            col: 0,
            row: 0,
            color: 0x0F, // white on black
        }
    }

    /// Safely write a slice of bytes into the VGA Buffer
    ///
    /// Handles '\n' and automatic line wrapping.
    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_byte(*byte);
        }
    }

    /// Safely write a byte into the VGA Buffer
    ///
    /// Handles '\n' and automatic line wrapping.
    #[inline]
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            byte => {
                if self.col >= VGA_WIDTH {
                    self.newline();
                }
                let offset = (self.row * VGA_WIDTH + self.col) * 2;
                unsafe {
                    VGA_BUFFER.add(offset).write_volatile(byte);
                    VGA_BUFFER.add(offset + 1).write_volatile(self.color);
                }
                self.col += 1;
            }
        }
    }

    /// Set the cursor to a new line
    ///
    /// Scrolls if necessary.
    #[inline]
    pub fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= VGA_HEIGHT {
            self.scroll();
        }
    }

    /// Reset the cursor and clear the VGA Buffer
    pub fn reset(&mut self) {
        self.col = 0;
        self.row = 0;
        self.clear();
    }

    /// Clear the VGA Buffer
    pub fn clear(&mut self) {
        unsafe {
            for cell in 0..(VGA_WIDTH * VGA_HEIGHT) {
                VGA_BUFFER.add(cell * 2).write_volatile(b' ');
                VGA_BUFFER.add(cell * 2 + 1).write_volatile(self.color);
            }
        }
    }

    unsafe fn clear_row(&mut self, row: usize) {
        unsafe {
            let row_buf = VGA_BUFFER.add(row * VGA_WIDTH * 2);
            for col in 0..VGA_WIDTH {
                row_buf.add(col * 2).write_volatile(b' ');
                row_buf.add(col * 2 + 1).write_volatile(self.color);
            }
        }
    }

    fn scroll(&mut self) {
        // copy each byte one row up
        unsafe {
            let dst = VGA_BUFFER;
            let src = VGA_BUFFER.add(VGA_WIDTH * 2);
            let count = (VGA_HEIGHT - 1) * VGA_WIDTH * 2;
            crate::volatile::copy(src, dst, count);
        }
        // clear the last row
        self.row = VGA_HEIGHT - 1;
        unsafe {
            self.clear_row(self.row);
        }
    }
}
