//! Screen capture tool for AI agents
//!
//! This tool allows agents to capture screenshots and analyze what's on the screen
//! using Claude's computer vision capability.

use crate::tools::ToolResult;
use base64::{engine::general_purpose, Engine as _};
use screenshots::Screen;
use std::io::Cursor;
// Command import removed as it's not needed
use crate::llm::Content;
use crate::llm::ImageSource;
use crate::tools::screendump;
use image::{DynamicImage, GenericImageView, ImageOutputFormat};

/// Execute the screenshot tool - captures the screen and returns it as a base64-encoded image
pub async fn execute_screenshot(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
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
            // Check if screens exist before deciding
            if let Ok(screens) = Screen::all() {
                if index < screens.len() {
                    ScreenshotCommand::SingleScreen(index)
                } else {
                    // Treat as window ID if index is out of bounds
                    ScreenshotCommand::Window(args.to_string())
                }
            } else {
                // Fallback to window ID if can't get screens
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
            // Use our window rect function to capture a specific window
            let result = capture_window(&window_id)?;
            Ok(vec![result])
        }
    }
}

/// Capture all screens as separate images
fn capture_all_screens() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Get all screens
    let screens = Screen::all()?;

    if screens.is_empty() {
        return Err("No screens found".into());
    }

    // Capture each screen separately
    let mut results = Vec::new();

    for (i, screen) in screens.iter().enumerate() {
        crate::bprintln!(dev: "Capturing screen {} ({}x{})", i, screen.display_info.width, screen.display_info.height);

        let image = screen.capture()?;
        let base64_image = process_image(DynamicImage::ImageRgba8(image))?;
        results.push(base64_image);
    }

    Ok(results)
}

/// Capture a single screen by index
fn capture_single_screen(index: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Get all screens
    let screens = Screen::all()?;

    if screens.is_empty() {
        return Err("No screens found".into());
    }

    if index >= screens.len() {
        return Err(format!(
            "Screen index {} out of bounds (0-{})",
            index,
            screens.len() - 1
        )
        .into());
    }

    // Capture the specified screen
    let screen = &screens[index];
    crate::bprintln!(dev: "Capturing screen {} ({}x{})", index, screen.display_info.width, screen.display_info.height);

    let image = screen.capture()?;
    let base64_image = process_image(DynamicImage::ImageRgba8(image))?;

    Ok(vec![base64_image])
}

/// Capture a specific window
fn capture_window(window_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    crate::bprintln!(dev: "Capturing window: {}", window_id);

    // Get the window rectangle using the screendump function
    let window_rect = screendump::get_window_rect(window_id)
        .map_err(|e| format!("Failed to find window: {}", e))?;

    let (app_name, window_title, x, y, width, height) = window_rect;

    crate::bprintln!(dev: "Found window '{}' of app '{}' at {}x{} size {}x{}",
        window_title, app_name, x, y, width, height);

    // Use screenshots crate to capture the region
    let screens = Screen::all()?;

    // Find which screen contains this window
    let mut found_screen = None;
    for screen in &screens {
        let screen_x = screen.display_info.x as i32;
        let screen_y = screen.display_info.y as i32;
        let screen_width = screen.display_info.width as i32;
        let screen_height = screen.display_info.height as i32;

        // Check if the window is at least partially on this screen
        if x < screen_x + screen_width
            && x + width > screen_x
            && y < screen_y + screen_height
            && y + height > screen_y
        {
            found_screen = Some(screen);
            break;
        }
    }

    let screen = found_screen.ok_or_else(|| "Window not on any screen".to_string())?;

    // Calculate coordinates relative to the screen
    let rel_x = (x - screen.display_info.x as i32).max(0) as u32;
    let rel_y = (y - screen.display_info.y as i32).max(0) as u32;

    // Ensure width and height stay within screen bounds
    let cap_width = (width as u32).min(screen.display_info.width - rel_x);
    let cap_height = (height as u32).min(screen.display_info.height - rel_y);

    // Capture the region - with proper type conversions
    let image = screen.capture_area(rel_x as i32, rel_y as i32, cap_width, cap_height)?;

    // Process the image
    process_image(DynamicImage::ImageRgba8(image))
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
    resized_img.write_to(&mut cursor, ImageOutputFormat::Jpeg(75))?;

    // Encode as base64
    let base64_image = general_purpose::STANDARD.encode(&jpeg_data);

    crate::bprintln!(dev: "Generated base64 image data ({} bytes)", base64_image.len());

    Ok(base64_image)
}
