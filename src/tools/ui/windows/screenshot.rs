//! Windows implementation of the screenshot tool
//!
//! This module provides Windows-specific implementation for the screenshot tool
//! using the xcap crate for cross-platform screen capture.

use crate::llm::{Content, ImageSource};
use crate::tools::ToolResult;
use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, GenericImageView, ImageOutputFormat};
use std::io::Cursor;
use xcap::{Monitor, Window, XCapResult};

/// Execute the Windows screenshot tool
pub async fn execute_windows_screenshot(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Parse screenshot command
    let command = parse_command(args);

    // Log tool invocation
    crate::bprintln!(
        dev: "Screenshot tool executing with args: '{}', command: {:?}",
        args,
        command
    );

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
                            text: format!("Screenshot: {}", i),
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
            let error_message = format!("Failed to capture screenshot: {}", e);

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
                    crate::bprintln!(
                        dev: "Invalid screen index '{}', capturing all screens",
                        parts[1]
                    );
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
            let monitors = Monitor::all();
            if index < monitors.len() {
                ScreenshotCommand::SingleScreen(index)
            } else {
                // Treat as window ID if index is out of bounds
                ScreenshotCommand::Window(args.to_string())
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
            // Capture a specific window
            let result = capture_window(&window_id)?;
            Ok(vec![result])
        }
    }
}

/// Capture all screens as separate images
fn capture_all_screens() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Get all monitors
    let monitors = Monitor::all();

    if monitors.is_empty() {
        return Err("No screens found".into());
    }

    // Capture each monitor separately
    let mut results = Vec::new();

    for (i, monitor) in monitors.iter().enumerate() {
        crate::bprintln!(
            dev: "Capturing screen {} ({}x{})",
            i,
            monitor.width(),
            monitor.height()
        );

        // Capture the screen
        let image = monitor.capture()?;

        // Convert xcap::image::Image to DynamicImage
        let dynamic_image = DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(
                image.width() as u32,
                image.height() as u32,
                image.data().to_vec(),
            )
            .ok_or("Failed to convert image data")?,
        );

        // Process the image
        let base64_image = process_image(dynamic_image)?;
        results.push(base64_image);
    }

    Ok(results)
}

/// Capture a single screen by index
fn capture_single_screen(index: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Get all monitors
    let monitors = Monitor::all();

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
    crate::bprintln!(
        dev: "Capturing screen {} ({}x{})",
        index,
        monitor.width(),
        monitor.height()
    );

    // Capture the screen
    let image = monitor.capture()?;

    // Convert xcap::image::Image to DynamicImage
    let dynamic_image = DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(
            image.width() as u32,
            image.height() as u32,
            image.data().to_vec(),
        )
        .ok_or("Failed to convert image data")?,
    );

    // Process the image
    let base64_image = process_image(dynamic_image)?;

    Ok(vec![base64_image])
}

/// Capture a specific window
fn capture_window(window_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    crate::bprintln!(dev: "Capturing window: {}", window_id);

    // First try using a numeric index for the window
    if let Ok(window_index) = window_id.parse::<usize>() {
        let windows = Window::all()?;
        if window_index < windows.len() {
            let window = &windows[window_index];
            crate::bprintln!(dev: "Found window '{}' by index {}", window.title(), window_index);

            // Capture the window
            let image = window.capture()?;

            // Convert to DynamicImage
            let dynamic_image = DynamicImage::ImageRgba8(
                image::RgbaImage::from_raw(
                    image.width() as u32,
                    image.height() as u32,
                    image.data().to_vec(),
                )
                .ok_or("Failed to convert image data")?,
            );

            return process_image(dynamic_image);
        }
    }

    // Try finding window by title match
    let windows = Window::all()?;
    for window in windows {
        if window.title().contains(window_id) {
            crate::bprintln!(dev: "Found window '{}' by title match", window.title());

            // Capture the window
            let image = window.capture()?;

            // Convert to DynamicImage
            let dynamic_image = DynamicImage::ImageRgba8(
                image::RgbaImage::from_raw(
                    image.width() as u32,
                    image.height() as u32,
                    image.data().to_vec(),
                )
                .ok_or("Failed to convert image data")?,
            );

            return process_image(dynamic_image);
        }
    }

    Err(format!("Window not found: {}", window_id).into())
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

        crate::bprintln!(
            dev: "Resizing image from {}x{} to {}x{}",
            width,
            height,
            new_width,
            new_height
        );
        img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Convert to JPEG format with quality adjustment for reasonable size
    let mut jpeg_data = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_data);
    resized_img.write_to(&mut cursor, ImageOutputFormat::Jpeg(75))?;

    // Encode as base64
    let base64_image = general_purpose::STANDARD.encode(&jpeg_data);

    crate::bprintln!(dev: "Generated base64 image data ({} bytes)", base64_image.len());

    Ok(base64_image)
}