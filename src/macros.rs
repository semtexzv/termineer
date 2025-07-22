//! Macros for simplified buffer printing
//!
//! These macros provide a simpler way to interact with the buffer system
//! without explicitly using the CURRENT_BUFFER task-local storage.
//!
//! The `bdebug_println!` macro is only active in debug builds, allowing for
//! selective logging of sensitive implementation details that shouldn't be
//! visible in release builds.

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
///
/// This unified macro supports multiple message types:
/// - bprintln!("message")                    - Regular message
/// - bprintln!(info: "message")              - Info message
/// - bprintln!(warn: "message")              - Warning message
/// - bprintln!(error: "message")             - Error message
/// - bprintln!(debug: "message")             - Debug message (debug builds only)
/// - bprintln!(dev: "message")               - Dev message (debug builds only)
/// - bprintln!(tool: "name", "message")      - Tool-specific message
#[macro_export]
macro_rules! bprintln {
    // Empty case
    () => {
        $crate::bprint!("\n")
    };

    // Info message: info: format
    (info: $($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!("{}ℹ️ Info:{} {}",
                              $crate::constants::FORMAT_BOLD,
                              $crate::constants::FORMAT_RESET,
                              format!($($arg)*));
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(format!("{message}\n"))
        });
    }};

    // Warning message: warn: format
    (warn: $($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!("{}⚠️ Warning:{} {}",
                              $crate::constants::FORMAT_BOLD,
                              $crate::constants::FORMAT_RESET,
                              format!($($arg)*));
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(format!("{message}\n"))
        });
    }};

    // Error message: error: format
    (error: $($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!("{}❌ Error:{} {}",
                              $crate::constants::FORMAT_BOLD,
                              $crate::constants::FORMAT_RESET,
                              format!($($arg)*));
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stderr(format!("{message}\n"))
        });
    }};

    // Debug message: debug: format (only active in debug builds)
    (debug: $($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            use $crate::output::CURRENT_BUFFER;
            let message = format!("{}🔍 Debug:{} {}",
                                $crate::constants::FORMAT_CYAN,
                                $crate::constants::FORMAT_RESET,
                                format!($($arg)*));
            let _ = CURRENT_BUFFER.with(|buffer| {
                buffer.stdout(format!("{message}\n"))
            });
        }
        // In release builds, this is a no-op
        #[cfg(not(debug_assertions))]
        {
            // Do nothing
        }
    }};

    // Dev message: dev: format (only active in debug builds but with different styling)
    (dev: $($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            use $crate::output::CURRENT_BUFFER;
            let message = format!("{}🛠️ Dev:{} {}",
                                $crate::constants::FORMAT_MAGENTA,
                                $crate::constants::FORMAT_RESET,
                                format!($($arg)*));
            let _ = CURRENT_BUFFER.with(|buffer| {
                buffer.stdout(format!("{message}\n"))
            });
        }
        // In release builds, this is a no-op
        #[cfg(not(debug_assertions))]
        {
            // Do nothing
        }
    }};

    // Tool message: tool: NAME, format
    (tool: $tool:expr, $($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let tool_name = $tool;
        let message = format!($($arg)*);
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.tool(tool_name, message)
        });
    }};

    // Default case (regular message)
    ($($arg:tt)*) => {{
        use $crate::output::CURRENT_BUFFER;
        let message = format!($($arg)*);
        let _ = CURRENT_BUFFER.with(|buffer| {
            buffer.stdout(format!("{message}\n"))
        });
    }};
}
