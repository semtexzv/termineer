//! macOS implementation of the screenshot tool
//!
//! This module provides macOS-specific implementation for the screenshot tool
//! using the xcap crate for cross-platform screen capture.

use crate::llm::Content;
use crate::llm::ImageSource;
use crate::tools::ui::screendump;
use crate::tools::ToolResult;
use base64::{engine::general_purpose, Engine as _};
use image::ImageFormat;
use image::{DynamicImage, GenericImageView};
use std::io::Cursor;
use xcap::{Monitor, Window};

/// Execute the macOS screenshot tool
pub async fn execute_macos_screenshot(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Parse screenshot command
    let command = parse_command(args);

    // Log tool invocation
    crate::bprintln!(dev: "Screenshot tool executing with args: '{}', command: {:?}", args, command);

    if !silent_mode {
        match &command {
            ScreenshotCommand::FullScreen => {
                crate::bprintln!("📷 Capturing all screens separately...");
            }
            ScreenshotCommand::SingleScreen(index) => {
                crate::bprintln!("📷 Capturing screen {}...", index);
            }
            ScreenshotCommand::Window(id) => {
                crate::bprintln!("📷 Capturing screenshot of window '{}'...", id);
            }
        }
    }

    // Attempt to capture screenshots
    match capture_screenshots(command) {
        Ok(images) => {
            // Create image content objects for each captured image
            let content: Vec<Content> = images
                .into_iter()
                .enumerate()
                .flat_map(|(i, base64_image)| {
                    vec![
                        Content::Text {
                            text: format!("Screenshot: {i}"),
                        },
                        Content::Image {
                            source: ImageSource::Base64 {
                                media_type: "image/jpeg".to_string(),
                                data: base64_image,
                            },
                        },
                    ]
                })
                .collect();

            if !silent_mode {
                crate::bprintln!("✅ Screenshot(s) captured successfully");
            }

            ToolResult::success_with_content(content)
        }
        Err(e) => {
            let error_message = format!("Failed to capture screenshot: {e}");

            crate::bprintln!(dev: "ERROR: Screenshot capture failed: {}", e);

            if !silent_mode {
                crate::bprintln!(error: "{}", error_message);
            }

            ToolResult::error(error_message)
        }
    }
}

/// Commands supported by the screenshot tool
#[derive(Debug, Clone)]
enum ScreenshotCommand {
    /// Capture all displays as separate images
    FullScreen,
    /// Capture a specific screen by index
    SingleScreen(usize),
    /// Capture a specific window
    Window(String),
}

/// Parse the command arguments
fn parse_command(args: &str) -> ScreenshotCommand {
    let args = args.trim();

    if args.is_empty() {
        // Default behavior is full screen capture
        return ScreenshotCommand::FullScreen;
    }

    let parts: Vec<&str> = args.split_whitespace().collect();

    match parts[0].to_lowercase().as_str() {
        "window" | "win" => {
            if parts.len() > 1 {
                // Everything after "window" is treated as the identifier
                let window_id = parts[1..].join(" ");
                ScreenshotCommand::Window(window_id)
            } else {
                // "window" without an ID defaults to full screen
                ScreenshotCommand::FullScreen
            }
        }
        "screen" => {
            if parts.len() > 1 {
                // Get the screen index
                if let Ok(index) = parts[1].parse::<usize>() {
                    ScreenshotCommand::SingleScreen(index)
                } else {
                    // Invalid screen index, default to all screens
                    crate::bprintln!(dev: "Invalid screen index '{}', capturing all screens", parts[1]);
                    ScreenshotCommand::FullScreen
                }
            } else {
                // "screen" without an index defaults to all screens
                ScreenshotCommand::FullScreen
            }
        }
        // If first word is a number, first check if it's a valid screen index
        id if id.parse::<usize>().is_ok() => {
            let index = id.parse::<usize>().unwrap();
            // Check if monitors exist before deciding
            match Monitor::all() {
                Ok(monitors) if index < monitors.len() => ScreenshotCommand::SingleScreen(index),
                _ => {
                    // Treat as window ID if index is out of bounds or error
                    ScreenshotCommand::Window(args.to_string())
                }
            }
        }
        // Otherwise, if there's text, assume it's a window identifier
        _ => {
            // Treat the entire args as a window name/identifier
            ScreenshotCommand::Window(args.to_string())
        }
    }
}

/// Capture screenshots and convert to base64-encoded JPEGs
fn capture_screenshots(
    command: ScreenshotCommand,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    crate::bprintln!(dev: "Capturing screenshot with command: {:?}", command);

    match command {
        ScreenshotCommand::FullScreen => {
            // Capture all displays as separate images
            capture_all_screens()
        }
        ScreenshotCommand::SingleScreen(index) => {
            // Capture a specific screen
            capture_single_screen(index)
        }
        ScreenshotCommand::Window(window_id) => {
            // Use our window rect function to capture a specific window
            let result = capture_window(&window_id)?;
            Ok(vec![result])
        }
    }
}

/// Capture all screens as separate images
fn capture_all_screens() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Get all monitors
    let monitors = Monitor::all()?;

    if monitors.is_empty() {
        return Err("No screens found".into());
    }

    // Capture each monitor separately
    let mut results = Vec::new();

    for (i, monitor) in monitors.iter().enumerate() {
        // Get dimensions - properly handle Result
        let width = monitor.width()?;
        let height = monitor.height()?;

        crate::bprintln!(
            dev: "Capturing screen {} ({}x{})",
            i,
            width,
            height
        );

        // Capture the screen
        let image = monitor.capture_image()?;

        // Convert xcap::image::Image to DynamicImage
        // The image from xcap is already an RgbaImage from the image crate
        let dynamic_image = DynamicImage::ImageRgba8(image);

        // Process the image
        let base64_image = process_image(dynamic_image)?;
        results.push(base64_image);
    }

    Ok(results)
}

/// Capture a single screen by index
fn capture_single_screen(index: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Get all monitors
    let monitors = Monitor::all()?;

    if monitors.is_empty() {
        return Err("No screens found".into());
    }

    if index >= monitors.len() {
        return Err(format!(
            "Screen index {} out of bounds (0-{})",
            index,
            monitors.len() - 1
        )
        .into());
    }

    // Capture the specified monitor
    let monitor = &monitors[index];

    // Get dimensions - properly handle Result
    let width = monitor.width()?;
    let height = monitor.height()?;

    crate::bprintln!(
        dev: "Capturing screen {} ({}x{})",
        index,
        width,
        height
    );

    // Capture the screen
    let image = monitor.capture_image()?;

    // Convert to DynamicImage
    // The image from xcap is already an RgbaImage from the image crate
    let dynamic_image = DynamicImage::ImageRgba8(image);

    // Process the image
    let base64_image = process_image(dynamic_image)?;

    Ok(vec![base64_image])
}

/// Capture a specific window
fn capture_window(window_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    crate::bprintln!(dev: "Capturing window: {}", window_id);

    // First try using xcap's Window functions if the window_id is a numeric ID
    if let Ok(window_index) = window_id.parse::<usize>() {
        let windows = Window::all()?;
        if window_index < windows.len() {
            let window = &windows[window_index];
            let title = window.title()?;
            crate::bprintln!(dev: "Found window '{}' by index {}", title, window_index);

            // Capture the window
            let image = window.capture_image()?;

            // Convert to DynamicImage - image is already an RgbaImage
            let dynamic_image = DynamicImage::ImageRgba8(image);

            return process_image(dynamic_image);
        }
    }

    // Try finding window by title match
    let windows = Window::all()?;
    for window in windows {
        if let Ok(title) = window.title() {
            if title.contains(window_id) {
                crate::bprintln!(dev: "Found window '{}' by title match", title);

                // Capture the window
                let image = window.capture_image()?;

                // Convert to DynamicImage - image is already an RgbaImage
                let dynamic_image = DynamicImage::ImageRgba8(image);

                return process_image(dynamic_image);
            }
        }
    }

    // If not found by xcap, fall back to our custom window finder and region capture
    // Get the window rectangle using the screendump function
    let window_rect = screendump::get_window_rect(window_id)
        .map_err(|e| format!("Failed to find window: {e}"))?;

    let (app_name, window_title, x, y, width, height) = window_rect;

    crate::bprintln!(dev: "Found window '{}' of app '{}' at {}x{} size {}x{}",
        window_title, app_name, x, y, width, height);

    // Capture the region using monitor.capture_area
    let monitors = Monitor::all()?;

    // Find which monitor contains this window
    let mut found_monitor = None;
    for monitor in &monitors {
        let monitor_x = monitor.x()?;
        let monitor_y = monitor.y()?;
        let monitor_width = monitor.width()? as i32;
        let monitor_height = monitor.height()? as i32;

        // Check if the window is at least partially on this monitor
        if x < monitor_x + monitor_width
            && x + width > monitor_x
            && y < monitor_y + monitor_height
            && y + height > monitor_y
        {
            found_monitor = Some(monitor);
            break;
        }
    }

    let monitor = found_monitor.ok_or_else(|| "Window not on any screen".to_string())?;

    // Calculate coordinates relative to the monitor
    let rel_x = (x - monitor.x()?).max(0);
    let rel_y = (y - monitor.y()?).max(0);

    // Ensure width and height stay within monitor bounds
    let width_pixels = monitor.width()? as i32;
    let height_pixels = monitor.height()? as i32;

    let cap_width = (width as u32).min((width_pixels - rel_x) as u32);
    let cap_height = (height as u32).min((height_pixels - rel_y) as u32);

    // Capture the entire monitor and crop the region
    let full_image = monitor.capture_image()?;

    // Crop the image to the desired area
    let cropped_image = image::imageops::crop_imm(
        &full_image,
        rel_x as u32,
        rel_y as u32,
        cap_width,
        cap_height,
    )
    .to_image();

    // Convert to DynamicImage
    let dynamic_image = DynamicImage::ImageRgba8(cropped_image);

    // Process the image
    process_image(dynamic_image)
}

/// Process the image (resize if needed, convert to JPEG)
fn process_image(img: DynamicImage) -> Result<String, Box<dyn std::error::Error>> {
    // Check if image dimensions are too large (Claude has limits)
    let (width, height) = img.dimensions();
    let resized_img = if width > 1600 || height > 1200 {
        // Scale down to reasonable dimensions while preserving aspect ratio
        let scale_factor = f32::min(1600.0 / width as f32, 1200.0 / height as f32);
        let new_width = (width as f32 * scale_factor) as u32;
        let new_height = (height as f32 * scale_factor) as u32;

        crate::bprintln!(dev: "Resizing image from {}x{} to {}x{}", width, height, new_width, new_height);
        img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Convert to JPEG format with quality adjustment for reasonable size
    let mut jpeg_data = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_data);
    resized_img.write_to(&mut cursor, ImageFormat::Jpeg)?;

    // Encode as base64
    let base64_image = general_purpose::STANDARD.encode(&jpeg_data);

    crate::bprintln!(dev: "Generated base64 image data ({} bytes)", base64_image.len());

    Ok(base64_image)
}
