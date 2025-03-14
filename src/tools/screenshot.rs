//! Screen capture tool for AI agents
//!
//! This tool allows agents to capture screenshots and analyze what's on the screen
//! using Claude's computer vision capability.

use crate::tools::ToolResult;
use screenshots::Screen;
use base64::{Engine as _, engine::general_purpose};
use std::io::Cursor;
// Command import removed as it's not needed
use image::{ImageOutputFormat, GenericImageView, DynamicImage};
use crate::llm::ImageSource;
use crate::tools::screendump;

/// Execute the screenshot tool - captures the screen and returns it as a base64-encoded image
pub async fn execute_screenshot(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Parse screenshot command
    let command = parse_command(args);
    
    // Log tool invocation
    crate::bprintln!(dev: "Screenshot tool executing with args: '{}', command: {:?}", args, command);
    
    if !silent_mode {
        match &command {
            ScreenshotCommand::FullScreen => {
                crate::bprintln!("📷 Capturing full screen screenshot...");
            },
            ScreenshotCommand::Window(id) => {
                crate::bprintln!("📷 Capturing screenshot of window '{}'...", id);
            },
        }
    }

    // Attempt to capture a screenshot
    match capture_screenshot(command) {
        Ok(base64_image) => {
            // Create an image content object with the base64 data
            let content = vec![crate::llm::Content::Image { 
                source: ImageSource::Base64 {
                    media_type: "image/jpeg".to_string(),
                    data: base64_image,
                }
            }];

            if !silent_mode {
                crate::bprintln!("✅ Screenshot captured successfully");
            }

            ToolResult::success_with_content(content)
        },
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
    /// Capture the full screen (all displays)
    FullScreen,
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
        },
        // If first word is a number, treat it as a window ID
        id if id.parse::<i32>().is_ok() => {
            ScreenshotCommand::Window(id.to_string())
        },
        // Otherwise, if there's text, assume it's a window identifier
        _ => {
            // Treat the entire args as a window name/identifier
            ScreenshotCommand::Window(args.to_string())
        }
    }
}

/// Capture a screenshot and convert it to base64-encoded JPEG
fn capture_screenshot(command: ScreenshotCommand) -> Result<String, Box<dyn std::error::Error>> {
    crate::bprintln!(dev: "Capturing screenshot with command: {:?}", command);
    
    match command {
        ScreenshotCommand::FullScreen => {
            // Capture all displays
            capture_full_screen()
        },
        ScreenshotCommand::Window(window_id) => {
            // Use our window rect function to capture a specific window
            capture_window(&window_id)
        }
    }
}

/// Capture the full screen (all displays)
fn capture_full_screen() -> Result<String, Box<dyn std::error::Error>> {
    // Get all screens
    let screens = Screen::all()?;
    
    if screens.is_empty() {
        return Err("No screens found".into());
    }
    
    // If only one screen, capture it directly
    if screens.len() == 1 {
        let screen = &screens[0];
        let image = screen.capture()?;
        return process_image(DynamicImage::ImageRgba8(image));
    }
    
    // For multiple screens, capture each one and combine them
    crate::bprintln!(dev: "Capturing {} screens", screens.len());
    
    // First, determine the combined size and position of all screens
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    
    for screen in &screens {
        let x = screen.display_info.x as i32;
        let y = screen.display_info.y as i32;
        let width = screen.display_info.width as i32;
        let height = screen.display_info.height as i32;
        
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + width);
        max_y = max_y.max(y + height);
    }
    
    // Calculate dimensions of the composite image
    let total_width = (max_x - min_x) as u32;
    let total_height = (max_y - min_y) as u32;
    
    crate::bprintln!(dev: "Creating composite image of size {}x{}", total_width, total_height);
    
    // Create a new image with the combined size
    let mut composite = image::RgbaImage::new(total_width, total_height);
    
    // Capture each screen and place it in the correct position
    for screen in &screens {
        // Capture the screen
        let image = screen.capture()?;
        let x = (screen.display_info.x as i32 - min_x) as u32;
        let y = (screen.display_info.y as i32 - min_y) as u32;
        
        // Copy pixels from this screen to the composite image
        for (src_x, src_y, pixel) in image.enumerate_pixels() {
            let dest_x = x + src_x;
            let dest_y = y + src_y;
            
            // Check bounds to avoid panics
            if dest_x < total_width && dest_y < total_height {
                composite.put_pixel(dest_x, dest_y, *pixel);
            }
        }
    }
    
    // Process the composite image
    process_image(DynamicImage::ImageRgba8(composite))
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
        if x < screen_x + screen_width && x + width > screen_x &&
           y < screen_y + screen_height && y + height > screen_y {
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