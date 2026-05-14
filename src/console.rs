//! Interact with the console

/// Prints formatted text to the console
#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {
        $crate::vga::_print(core::format_args!($($arg)*))
    };
}

/// Prints formatted, colored text to the console.
///
/// A foreground color without an explicit background is interpreted as that
/// color on black:
///
/// ```rust,ignore
/// use zkrnl32::vga::Color;
///
/// printk_color!(Color::Yellow, "warning: {}\n", "low memory");
/// ```
///
/// Passing the equivalent full VGA color byte works the same way:
///
/// ```rust,ignore
/// use zkrnl32::vga::Color;
///
/// printk_color!(Color::Yellow.on(Color::Black), "warning: {}\n", "low memory");
/// ```
#[macro_export]
macro_rules! printk_color {
    ($color:expr, $($arg:tt)*) => {
        $crate::vga::_print_color(($color) as u8, core::format_args!($($arg)*))
    };
}
