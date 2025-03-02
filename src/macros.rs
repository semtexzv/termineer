//! Macros for simplified buffer printing
//!
//! These macros provide a simpler way to interact with the buffer system
//! without explicitly using the CURRENT_BUFFER task-local storage.

/// Print to the current buffer with no line ending
#[macro_export]
macro_rules! bprint {
    ($($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!($($arg)*);
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(message)
        });
    }};
}

/// Print to the current buffer with a line ending
#[macro_export]
macro_rules! bprintln {
    () => {
        $crate::bprint!("\n")
    };
    ($($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!($($arg)*);
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(format!("{}\n", message))
        });
    }};
}

/// Print a tool message to the current buffer with a line ending
#[macro_export]
macro_rules! btool_println {
    ($tool:expr, $($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let tool_name = $tool;
        let message = format!($($arg)*);
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.tool(tool_name, message)
        });
    }};
}

/// Print an error message to the current buffer
#[macro_export]
macro_rules! berror_println {
    ($($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!("{}❌ Error:{} {}",
                              $crate::constants::FORMAT_BOLD,
                              $crate::constants::FORMAT_RESET,
                              format!($($arg)*));
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stderr(message)
        });
    }};
}

/// Print a warning message to the current buffer
#[macro_export]
macro_rules! bwarning_println {
    ($($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!("{}⚠️ Warning:{} {}",
                              $crate::constants::FORMAT_BOLD,
                              $crate::constants::FORMAT_RESET,
                              format!($($arg)*));
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(message)
        });
    }};
}

/// Print an info message to the current buffer
#[macro_export]
macro_rules! binfo_println {
    ($($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!("{}ℹ️ Info:{} {}",
                              $crate::constants::FORMAT_BOLD,
                              $crate::constants::FORMAT_RESET,
                              format!($($arg)*));
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(message)
        });
    }};
}
