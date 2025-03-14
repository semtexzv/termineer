//! Tool for capturing the current UI display using accessibility APIs
//! 
//! This tool allows agents to capture the current UI structure as text,
//! providing information about window layout, controls, and hierarchies.

use crate::tools::ToolResult;
use std::env;
use std::collections::HashMap;

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil, NO};
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSSize};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use objc::runtime::BOOL;
#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(target_os = "macos")]
use std::os::raw::c_char;

/// Execute the UI dump tool to capture accessibility tree
pub async fn execute_screendump(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    let platform = env::consts::OS;
    let command = parse_command(args);
    
    crate::bprintln!(dev: "Screendump tool executing with args: '{}', command: {:?}", args, command);
    
    if !silent_mode {
        match &command {
            ScreendumpCommand::ListWindows => {
                crate::bprintln!("🔍 Listing all windows on {} platform...", platform);
            },
            ScreendumpCommand::WindowDetails(id) => {
                crate::bprintln!("🔍 Capturing details for window '{}' on {} platform...", id, platform);
            },
            ScreendumpCommand::FullScreen => {
                crate::bprintln!("🔍 Capturing full screen UI hierarchy on {} platform...", platform);
            },
        }
    }
    
    match platform {
        "macos" => capture_macos_ui(command, silent_mode),
        "windows" => capture_windows_ui(command),
        "linux" => capture_linux_ui(command),
        _ => ToolResult::error(format!("Screendump not implemented for {} platform", platform))
    }
}

/// Commands supported by the screendump tool
#[derive(Debug, Clone)]
enum ScreendumpCommand {
    /// List all windows with identifiers
    ListWindows,
    /// Capture details for a specific window
    WindowDetails(String),
    /// Capture full screen hierarchy (all windows)
    FullScreen,
}

/// Parse the command arguments
fn parse_command(args: &str) -> ScreendumpCommand {
    let args = args.trim();
    
    if args.is_empty() {
        return ScreendumpCommand::ListWindows;
    }
    
    let parts: Vec<&str> = args.split_whitespace().collect();
    
    match parts[0].to_lowercase().as_str() {
        "fullscreen" | "full" => ScreendumpCommand::FullScreen,
        "list" => ScreendumpCommand::ListWindows,
        "window" | "win" => {
            if parts.len() > 1 {
                ScreendumpCommand::WindowDetails(parts[1..].join(" "))
            } else {
                ScreendumpCommand::ListWindows
            }
        },
        id if id.parse::<i32>().is_ok() => ScreendumpCommand::WindowDetails(id.to_string()),
        _ => ScreendumpCommand::WindowDetails(args.to_string())
    }
}

#[cfg(target_os = "macos")]
struct MacOSWindow {
    app_name: String,
    window_title: String,
    position: (i32, i32),
    size: (i32, i32),
    ax_window: id,
    app_pid: i32,  // Keeping this for future extensions
    index: usize,
}

#[cfg(target_os = "macos")]
fn capture_macos_ui(command: ScreendumpCommand, silent_mode: bool) -> ToolResult {
    if !silent_mode {
        crate::bprintln!("Using macOS native Accessibility API...");
    }

    // Ensure accessibility is enabled
    unsafe {
        let trusted: BOOL = msg_send![class!(AXIsProcessTrustedWithOptions), 
            AXIsProcessTrustedWithOptions: nil];
        if trusted == NO {
            return ToolResult::error("Accessibility access is not enabled for this application. Please enable it in System Preferences > Security & Privacy > Privacy > Accessibility");
        }
    }

    match command {
        ScreendumpCommand::ListWindows => {
            match list_all_windows() {
                Ok(windows) => {
                    if windows.is_empty() {
                        return ToolResult::success("No visible windows found.");
                    }

                    let mut result = String::new();
                    result.push_str("Available Windows:\n");
                    
                    for window in windows {
                        let window_id = format!("{}:{}", window.app_name, window.index);
                        result.push_str(&format!(
                            "[{}] {}: {}\n", 
                            window_id,
                            window.app_name, 
                            window.window_title
                        ));
                    }
                    
                    result.push_str("\nUse 'screendump window [ID]' or 'screendump window [Window Title]' to view details for a specific window.");
                    
                    ToolResult::success(result)
                },
                Err(e) => ToolResult::error(format!("Failed to list windows: {}", e))
            }
        },
        ScreendumpCommand::WindowDetails(id) => {
            // Parse window ID or search by title
            let parts: Vec<&str> = id.split(':').collect();
            
            if parts.len() == 2 && parts[1].parse::<usize>().is_ok() {
                // Handle ID of form "AppName:Index"
                let app_name = parts[0];
                let window_index = parts[1].parse::<usize>().unwrap();
                
                match get_window_by_app_and_index(app_name, window_index) {
                    Ok(Some(window)) => {
                        match get_window_details(&window) {
                            Ok(details) => ToolResult::success(
                                format!("Window Details for '{}':\n{}", id, details)
                            ),
                            Err(e) => ToolResult::error(
                                format!("Failed to get window details: {}", e)
                            )
                        }
                    },
                    Ok(None) => ToolResult::error(
                        format!("Window with ID '{}' not found", id)
                    ),
                    Err(e) => ToolResult::error(
                        format!("Error finding window: {}", e)
                    )
                }
            } else {
                // Search by window title
                match find_window_by_title(&id) {
                    Ok(Some(window)) => {
                        match get_window_details(&window) {
                            Ok(details) => ToolResult::success(
                                format!("Window Details for '{}':\n{}", id, details)
                            ),
                            Err(e) => ToolResult::error(
                                format!("Failed to get window details: {}", e)
                            )
                        }
                    },
                    Ok(None) => ToolResult::error(
                        format!("Window with title containing '{}' not found", id)
                    ),
                    Err(e) => ToolResult::error(
                        format!("Error finding window: {}", e)
                    )
                }
            }
        },
        ScreendumpCommand::FullScreen => {
            match get_full_screen_hierarchy() {
                Ok(hierarchy) => ToolResult::success(
                    format!("Full Screen UI Hierarchy:\n{}", hierarchy)
                ),
                Err(e) => ToolResult::error(
                    format!("Failed to capture full screen hierarchy: {}", e)
                )
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn list_all_windows() -> Result<Vec<MacOSWindow>, String> {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let running_apps: id = msg_send![workspace, runningApplications];
        let count: usize = msg_send![running_apps, count];
        
        let mut windows = Vec::new();
        
        for app_idx in 0..count {
            let app: id = msg_send![running_apps, objectAtIndex: app_idx];
            let app_pid: i32 = msg_send![app, processIdentifier];
            
            // Skip apps that aren't visible
            let visible: BOOL = msg_send![app, isActive] || msg_send![app, isHidden];
            if visible == NO {
                continue;
            }
            
            // Get app name
            let app_name_ns: id = msg_send![app, localizedName];
            let app_name = nsstring_to_string(app_name_ns);
            
            // Get the app's accessibility element
            let ax_app = create_ax_ui_element_from_pid(app_pid)?;
            if ax_app.is_null() {
                continue;
            }
            
            // Get the windows
            let ax_windows = get_ax_windows(ax_app)?;
            
            // Process each window
            for (window_idx, &ax_window) in ax_windows.iter().enumerate() {
                // Get window title
                let title = get_ax_attribute_string(ax_window, "AXTitle")
                    .unwrap_or_else(|_| format!("Window {}", window_idx + 1));
                
                // Get position and size
                let position = get_ax_window_position(ax_window)
                    .unwrap_or((0, 0));
                
                let size = get_ax_window_size(ax_window)
                    .unwrap_or((0, 0));
                
                windows.push(MacOSWindow {
                    app_name: app_name.clone(),
                    window_title: title,
                    position,
                    size,
                    ax_window,
                    app_pid,
                    index: window_idx + 1,
                });
            }
        }
        
        Ok(windows)
    }
}

#[cfg(target_os = "macos")]
fn get_window_by_app_and_index(app_name: &str, window_index: usize) -> Result<Option<MacOSWindow>, String> {
    let windows = list_all_windows()?;
    
    for window in windows {
        if window.app_name == app_name && window.index == window_index {
            return Ok(Some(window));
        }
    }
    
    Ok(None)
}

#[cfg(target_os = "macos")]
fn find_window_by_title(title: &str) -> Result<Option<MacOSWindow>, String> {
    let windows = list_all_windows()?;
    
    for window in windows {
        if window.window_title.contains(title) {
            return Ok(Some(window));
        }
    }
    
    Ok(None)
}

#[cfg(target_os = "macos")]
fn get_window_details(window: &MacOSWindow) -> Result<String, String> {
    let mut result = String::new();
    
    // Basic window info
    result.push_str(&format!("Application: {}\n", window.app_name));
    result.push_str(&format!("Window: {}\n", window.window_title));
    result.push_str(&format!("Position: {}, {}\n", window.position.0, window.position.1));
    result.push_str(&format!("Size: {}×{}\n", window.size.0, window.size.1));
    
    // UI Elements
    result.push_str("\nUI Elements:\n");
    
    match get_ui_elements(window.ax_window) {
        Ok(elements) => {
            if elements.is_empty() {
                result.push_str("  No UI elements found\n");
            } else {
                for element in elements {
                    result.push_str("  Element:\n");
                    
                    // Get role
                    if let Ok(role) = get_ax_attribute_string(element, "AXRole") {
                        result.push_str(&format!("    Type: {}\n", role));
                    }
                    
                    // Get description
                    if let Ok(desc) = get_ax_attribute_string(element, "AXDescription") {
                        if !desc.is_empty() {
                            result.push_str(&format!("    Description: {}\n", desc));
                        }
                    }
                    
                    // Get value
                    if let Ok(value) = get_ax_attribute_string(element, "AXValue") {
                        if !value.is_empty() {
                            result.push_str(&format!("    Value: {}\n", value));
                        }
                    }
                }
            }
        },
        Err(e) => {
            result.push_str(&format!("  Error getting UI elements: {}\n", e));
        }
    }
    
    Ok(result)
}

#[cfg(target_os = "macos")]
fn get_full_screen_hierarchy() -> Result<String, String> {
    let windows = list_all_windows()?;
    
    let mut result = String::new();
    
    let mut app_map: HashMap<String, Vec<&MacOSWindow>> = HashMap::new();
    
    // Group windows by application
    for window in &windows {
        app_map.entry(window.app_name.clone())
            .or_insert_with(Vec::new)
            .push(window);
    }
    
    // Generate output for each application
    for (app_name, app_windows) in app_map {
        result.push_str(&format!("Application: {}\n", app_name));
        
        for window in app_windows {
            result.push_str(&format!("  Window {}: {}\n", window.index, window.window_title));
            result.push_str(&format!("    Position: {}, {}\n", window.position.0, window.position.1));
            result.push_str(&format!("    Size: {}×{}\n", window.size.0, window.size.1));
        }
        
        result.push_str("\n");
    }
    
    Ok(result)
}

// Helper functions for macOS accessibility API

#[cfg(target_os = "macos")]
fn nsstring_to_string(ns_string: id) -> String {
    unsafe {
        let utf8: *const c_char = msg_send![ns_string, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        
        CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}

#[cfg(target_os = "macos")]
fn string_to_cfstring(s: &str) -> CFStringRef {
    CFString::new(s).as_concrete_TypeRef()
}

#[cfg(target_os = "macos")]
fn create_ax_ui_element_from_pid(pid: i32) -> Result<id, String> {
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> id;
    }
    
    unsafe {
        let element = AXUIElementCreateApplication(pid);
        if element.is_null() {
            return Err(format!("Failed to create accessibility element for PID {}", pid));
        }
        
        Ok(element)
    }
}

#[cfg(target_os = "macos")]
fn get_ax_attribute(element: id, attribute: &str) -> Result<id, i32> {
    let cf_attr = string_to_cfstring(attribute);
    
    extern "C" {
        fn AXUIElementCopyAttributeValue(
            element: id,
            attribute: CFStringRef,
            value: *mut id
        ) -> i32;
    }
    
    unsafe {
        let mut result: id = nil;
        let error = AXUIElementCopyAttributeValue(element, cf_attr, &mut result);
        
        if error != 0 {
            return Err(error);
        }
        
        if result.is_null() {
            return Err(-1);
        }
        
        Ok(result)
    }
}

#[cfg(target_os = "macos")]
fn get_ax_attribute_string(element: id, attribute: &str) -> Result<String, String> {
    match get_ax_attribute(element, attribute) {
        Ok(value) => unsafe {
            // Try to convert to NSString first
            let description: id = msg_send![value, description];
            Ok(nsstring_to_string(description))
        },
        Err(error_code) => Err(format!("Error {} getting attribute {}", error_code, attribute))
    }
}

#[cfg(target_os = "macos")]
fn get_ax_windows(app: id) -> Result<Vec<id>, String> {
    match get_ax_attribute(app, "AXWindows") {
        Ok(windows_array) => {
            unsafe {
                let count: usize = msg_send![windows_array, count];
                let mut result = Vec::with_capacity(count);
                
                for i in 0..count {
                    let window: id = msg_send![windows_array, objectAtIndex: i];
                    result.push(window);
                }
                
                Ok(result)
            }
        },
        Err(error_code) => Err(format!("Error {} getting windows", error_code))
    }
}

#[cfg(target_os = "macos")]
fn get_ax_window_position(window: id) -> Result<(i32, i32), String> {
    match get_ax_attribute(window, "AXPosition") {
        Ok(position_value) => unsafe {
            let point: NSPoint = msg_send![position_value, pointValue];
            Ok((point.x as i32, point.y as i32))
        },
        Err(error_code) => Err(format!("Error {} getting window position", error_code))
    }
}

#[cfg(target_os = "macos")]
fn get_ax_window_size(window: id) -> Result<(i32, i32), String> {
    match get_ax_attribute(window, "AXSize") {
        Ok(size_value) => unsafe {
            let size: NSSize = msg_send![size_value, sizeValue];
            Ok((size.width as i32, size.height as i32))
        },
        Err(error_code) => Err(format!("Error {} getting window size", error_code))
    }
}

#[cfg(target_os = "macos")]
fn get_ui_elements(window: id) -> Result<Vec<id>, String> {
    // Try to get UI elements from the window
    match get_ax_attribute(window, "AXChildren") {
        Ok(children_array) => {
            unsafe {
                let count: usize = msg_send![children_array, count];
                let mut result = Vec::with_capacity(count);
                
                for i in 0..count {
                    let element: id = msg_send![children_array, objectAtIndex: i];
                    result.push(element);
                }
                
                Ok(result)
            }
        },
        Err(error_code) => Err(format!("Error {} getting UI elements", error_code))
    }
}

/// Public function to get a window's rectangle by ID
pub fn get_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
    let platform = env::consts::OS;
    
    match platform {
        "macos" => get_macos_window_rect(window_id),
        _ => Err(format!("Window rect retrieval not implemented for {} platform", platform))
    }
}

#[cfg(target_os = "macos")]
fn get_macos_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
    let parts: Vec<&str> = window_id.split(':').collect();
    
    if parts.len() == 2 && parts[1].parse::<usize>().is_ok() {
        // Handle ID of form "AppName:Index"
        let app_name = parts[0];
        let window_index = parts[1].parse::<usize>().unwrap();
        
        match get_window_by_app_and_index(app_name, window_index)? {
            Some(window) => {
                Ok((
                    window.app_name,
                    window.window_title,
                    window.position.0,
                    window.position.1,
                    window.size.0,
                    window.size.1
                ))
            },
            None => Err(format!("Window with ID '{}' not found", window_id))
        }
    } else {
        // Search by window title
        match find_window_by_title(window_id)? {
            Some(window) => {
                Ok((
                    window.app_name,
                    window.window_title,
                    window.position.0,
                    window.position.1,
                    window.size.0,
                    window.size.1
                ))
            },
            None => Err(format!("Window with title containing '{}' not found", window_id))
        }
    }
}

fn capture_windows_ui(command: ScreendumpCommand) -> ToolResult {
    let description = match command {
        ScreendumpCommand::ListWindows => "list windows",
        ScreendumpCommand::WindowDetails(ref id) => &format!("window details for '{}'", id),
        ScreendumpCommand::FullScreen => "full screen",
    };
    
    ToolResult::error(format!("Windows UI capture ({}) not yet implemented", description))
}

fn capture_linux_ui(command: ScreendumpCommand) -> ToolResult {
    let description = match command {
        ScreendumpCommand::ListWindows => "list windows",
        ScreendumpCommand::WindowDetails(ref id) => &format!("window details for '{}'", id),
        ScreendumpCommand::FullScreen => "full screen",
    };
    
    ToolResult::error(format!("Linux UI capture ({}) not yet implemented", description))
}