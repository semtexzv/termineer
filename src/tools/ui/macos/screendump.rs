#![allow(unexpected_cfgs)]

//! macOS implementation of the screendump tool
//!
//! This module provides macOS-specific implementation for the screendump tool
//! using the macOS Accessibility APIs.

use super::xml_helpers;
use crate::tools::ui::screendump::ScreendumpCommand;
use crate::tools::ToolResult;

use accessibility_ng::{AXUIElement, AXUIElementAttributes};
use accessibility_sys_ng::{pid_t, AXIsProcessTrusted};
use core_graphics_types::geometry::{CGPoint, CGSize};
use objc2::rc::{ Retained};
use objc2::runtime::NSObject;
use objc2::{class, msg_send};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSArray, NSString};

/// MacOS Window representation
struct MacOSWindow {
    app_name: String,
    window_title: String,
    position: (i32, i32),
    size: (i32, i32),
    element: AXUIElement,
    #[allow(dead_code)]
    pid: pid_t,
    index: usize,
}

/// Execute the macOS screendump tool
pub async fn execute_macos_screendump(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    let command = crate::tools::ui::screendump::parse_command(args);

    if !silent_mode {
        match &command {
            ScreendumpCommand::ListWindows => {
                crate::bprintln!("🔍 Listing all windows in XML format...");
            }
            ScreendumpCommand::WindowDetails(id) => {
                crate::bprintln!("🔍 Capturing XML details for window '{}'...", id);
            }
        }
    }

    // Check accessibility permissions
    unsafe {
        if !AXIsProcessTrusted() {
            return ToolResult::error("Accessibility access is not enabled for this application. Please enable it in System Preferences > Security & Privacy > Privacy > Accessibility");
        }
    }

    match command {
        ScreendumpCommand::ListWindows => {
            match list_all_windows() {
                Ok(windows) => {
                    if windows.is_empty() {
                        return ToolResult::success("<UITree><Windows/></UITree>");
                    }

                    // Create a list of all windows in XML format
                    let mut xml_output = String::from(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<UITree>\n  <Windows>\n",
                    );

                    for window in windows {
                        // Print window dimensions for debugging
                        crate::bprintln!(
                            dev: "🖥️ SCREENDUMP: Window '{}' ({}): Position=({},{}) Size={}×{}",
                            window.window_title, window.app_name,
                            window.position.0, window.position.1,
                            window.size.0, window.size.1
                        );

                        xml_output.push_str(&format!(
                            "    <Window id=\"{}:{}\" app=\"{}\" title=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>\n",
                            window.app_name, window.index,
                            window.app_name,
                            window.window_title,
                            window.position.0, window.position.1,
                            window.size.0, window.size.1
                        ));
                    }

                    xml_output.push_str("  </Windows>\n</UITree>");

                    // Log the full XML output for debugging
                    crate::bprintln!(dev: "🖥️ SCREENDUMP: Generated XML output for window list:\n{}", xml_output);

                    ToolResult::success(xml_output)
                }
                Err(e) => ToolResult::error(format!("Failed to list windows: {}", e)),
            }
        }
        ScreendumpCommand::WindowDetails(id) => {
            // Parse window ID or search by title
            let parts: Vec<&str> = id.split(':').collect();

            if parts.len() == 2 && parts[1].parse::<usize>().is_ok() {
                // Handle ID of form "AppName:Index"
                let app_name = parts[0];
                let window_index = parts[1].parse::<usize>().unwrap();

                match get_window_by_app_and_index(app_name, window_index) {
                    Ok(Some(window)) => match get_window_details_xml(&window) {
                        Ok(xml) => ToolResult::success(xml),
                        Err(e) => ToolResult::error(format!("Failed to get window details: {}", e)),
                    },
                    Ok(None) => ToolResult::error(format!("Window with ID '{}' not found", id)),
                    Err(e) => ToolResult::error(format!("Error finding window: {}", e)),
                }
            } else {
                // Search by window title
                match find_window_by_title(&id) {
                    Ok(Some(window)) => match get_window_details_xml(&window) {
                        Ok(xml) => ToolResult::success(xml),
                        Err(e) => ToolResult::error(format!("Failed to get window details: {}", e)),
                    },
                    Ok(None) => ToolResult::error(format!(
                        "Window with title containing '{}' not found",
                        id
                    )),
                    Err(e) => ToolResult::error(format!("Error finding window: {}", e)),
                }
            }
        }
    }
}

/// Get window details in XML format
fn get_window_details_xml(window: &MacOSWindow) -> Result<String, String> {
    crate::bprintln!(dev: "🖥️ SCREENDUMP: Getting XML window details for '{}' ({})", window.window_title, window.app_name);

    // Print window dimensions for debugging
    crate::bprintln!(
        dev: "🖥️ SCREENDUMP: Window dimensions - Position: ({}, {}) Size: {}×{}",
        window.position.0, window.position.1,
        window.size.0, window.size.1
    );

    // Create a structured UIWindow using our helper
    let ui_window = xml_helpers::create_ui_window_from_macos_window(
        window.app_name.clone(),
        window.window_title.clone(),
        window.position,
        window.size,
        &window.element,
    );

    // Convert to XML
    let xml_result = ui_window.to_xml();

    // Log the generated XML
    if let Ok(ref xml_string) = xml_result {
        // Log the full XML output for debugging
        crate::bprintln!(dev: "🖥️ SCREENDUMP: Generated XML output for window '{}':\n{}", 
                         window.window_title, xml_string);
    }

    xml_result
}

/// List all windows on the system
fn list_all_windows() -> Result<Vec<MacOSWindow>, String> {
    // Get running application PIDs
    let running_pids = get_running_application_pids()
        .map_err(|e| format!("Failed to get running applications: {}", e))?;

    let mut windows = Vec::new();

    // Process each application
    for (app_idx, pid) in running_pids.iter().enumerate() {
        // Create an accessibility element for this application
        let app = AXUIElement::application(*pid);
        let app_name =
            get_application_name(*pid).unwrap_or_else(|_| format!("App {}", app_idx + 1));

        // Try to get the application's windows using the windows() method
        match app.windows() {
            Ok(app_windows) => {
                // Process each window
                for (window_idx, window) in app_windows.into_iter().enumerate() {
                    // Get window title using the title() method
                    let window_title = window
                        .title()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| format!("Window {}", window_idx + 1));

                    // Get position
                    let position = get_window_position(&window).unwrap_or((0, 0));

                    // Get size
                    let size = get_window_size(&window).unwrap_or((0, 0));

                    windows.push(MacOSWindow {
                        app_name: app_name.clone(),
                        window_title,
                        position,
                        size,
                        element: window.clone(),
                        pid: *pid,
                        index: window_idx + 1,
                    });
                }
            }
            Err(_) => {
                // Skip applications that don't have windows
                continue;
            }
        }
    }

    Ok(windows)
}

/// Get running application process IDs
fn get_running_application_pids() -> Result<Vec<pid_t>, String> {
    unsafe {
        // Get the shared workspace using the static method
        let workspace: Retained<NSWorkspace> = msg_send![class!(NSWorkspace), sharedWorkspace];

        // Get running applications
        let running_apps: Retained<NSArray<NSObject>> = msg_send![&workspace, runningApplications];
        let count = running_apps.count();

        let mut pids = Vec::with_capacity(count);

        for i in 0..count {
            let app: Retained<NSObject> = msg_send![&running_apps, objectAtIndex: i];
            let pid: pid_t = msg_send![&app, processIdentifier];

            pids.push(pid);
        }

        Ok(pids)
    }
}

/// Get application name from process ID
fn get_application_name(pid: pid_t) -> Result<String, String> {
    unsafe {
        // Get the shared workspace using the static method
        let workspace: Retained<NSWorkspace> = msg_send![class!(NSWorkspace), sharedWorkspace];
        
        // Get running applications
        let running_apps: Retained<NSArray<NSObject>> = msg_send![&workspace, runningApplications];
        let count = running_apps.count();

        for i in 0..count {
            let app: Retained<NSObject> = msg_send![&running_apps, objectAtIndex: i];
            let app_pid: pid_t = msg_send![&app, processIdentifier];

            if app_pid == pid {
                let app_name: Option<Retained<NSString>> = msg_send![&app, localizedName];
                if let Some(name) = app_name {
                    return Ok(name.to_string());
                }
            }
        }

        Err(format!("Could not find application name for PID {}", pid))
    }
}

/// Get window position
fn get_window_position(window: &AXUIElement) -> Result<(i32, i32), String> {
    // Use the position accessor directly from AXUIElementAttributes trait
    match window.position() {
        Ok(point) => {
            let point = point.get_value::<CGPoint>().unwrap();
            // Log detailed coordinate information
            crate::bprintln!(
                dev: "🖥️ SCREENDUMP: Window position: ({}, {}) - macOS uses bottom-left origin coordinate system",
                point.x as i32,
                point.y as i32
            );

            Ok((point.x as i32, point.y as i32))
        }
        Err(e) => {
            let error = format!("Failed to get window position: {}", e);
            crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
            Err(error)
        }
    }
}

/// Get window size
fn get_window_size(window: &AXUIElement) -> Result<(i32, i32), String> {
    // Use the size accessor directly from AXUIElementAttributes trait
    match window.size() {
        Ok(size) => {
            let size = size.get_value::<CGSize>().unwrap();

            // Log window size information
            crate::bprintln!(
                dev: "🖥️ SCREENDUMP: Window size: width={}, height={}",
                size.width as i32,
                size.height as i32
            );

            Ok((size.width as i32, size.height as i32))
        }
        Err(e) => {
            let error = format!("Failed to get window size: {}", e);
            crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
            Err(error)
        }
    }
}

/// Find a window by app name and index
fn get_window_by_app_and_index(
    app_name: &str,
    window_index: usize,
) -> Result<Option<MacOSWindow>, String> {
    let windows = list_all_windows()?;

    for window in windows {
        if window.app_name == app_name && window.index == window_index {
            return Ok(Some(window));
        }
    }

    Ok(None)
}

/// Find a window by title
fn find_window_by_title(title: &str) -> Result<Option<MacOSWindow>, String> {
    let windows = list_all_windows()?;

    for window in windows {
        if window.window_title.contains(title) {
            return Ok(Some(window));
        }
    }

    Ok(None)
}

/// Get a window's rectangle by ID
pub fn get_macos_window_rect(
    window_id: &str,
) -> Result<(String, String, i32, i32, i32, i32), String> {
    crate::bprintln!(dev: "🖥️ SCREENDUMP: Getting window rect for '{}'", window_id);

    let parts: Vec<&str> = window_id.split(':').collect();

    if parts.len() == 2 && parts[1].parse::<usize>().is_ok() {
        // Handle ID of form "AppName:Index"
        let app_name = parts[0];
        let window_index = parts[1].parse::<usize>().unwrap();

        crate::bprintln!(dev: "🖥️ SCREENDUMP: Looking up window by app='{}', index={}", app_name, window_index);

        match get_window_by_app_and_index(app_name, window_index)? {
            Some(window) => {
                // Log detailed coordinate info
                crate::bprintln!(
                    dev: "🖥️ SCREENDUMP: Found window '{}' ({}): Position=({},{}) Size={}×{} - macOS coordinate system with (0,0) at bottom-left",
                    window.window_title,
                    window.app_name,
                    window.position.0,
                    window.position.1,
                    window.size.0,
                    window.size.1
                );

                Ok((
                    window.app_name,
                    window.window_title,
                    window.position.0,
                    window.position.1,
                    window.size.0,
                    window.size.1,
                ))
            }
            None => {
                let error = format!("Window with ID '{}' not found", window_id);
                crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
                Err(error)
            }
        }
    } else {
        // Search by window title
        crate::bprintln!(dev: "🖥️ SCREENDUMP: Looking up window by title match '{}'", window_id);

        match find_window_by_title(window_id)? {
            Some(window) => {
                // Log detailed coordinate info
                crate::bprintln!(
                    dev: "🖥️ SCREENDUMP: Found window '{}' ({}): Position=({},{}) Size={}×{} - macOS coordinate system with (0,0) at bottom-left",
                    window.window_title,
                    window.app_name,
                    window.position.0,
                    window.position.1,
                    window.size.0,
                    window.size.1
                );

                Ok((
                    window.app_name,
                    window.window_title,
                    window.position.0,
                    window.position.1,
                    window.size.0,
                    window.size.1,
                ))
            }
            None => {
                let error = format!("Window with title containing '{}' not found", window_id);
                crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
                Err(error)
            }
        }
    }
}
