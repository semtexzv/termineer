#![allow(unexpected_cfgs)]

use std::collections::HashMap;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;
use cocoa::base::id;
use core_graphics_types::geometry::{CGPoint, CGSize};
use accessibility_ng::{
    AXUIElement,
    AXUIElementAttributes
};
use accessibility_sys_ng::{
    AXIsProcessTrusted,
    pid_t,
};
use crate::tools::screendump::ScreendumpCommand;
use crate::tools::ToolResult;

struct MacOSWindow {
    app_name: String,
    window_title: String,
    position: (i32, i32),
    size: (i32, i32),
    element: AXUIElement,
    pid: pid_t,
    index: usize,
}


pub fn capture_macos_ui(command: ScreendumpCommand, silent_mode: bool) -> ToolResult {
    if !silent_mode {
        crate::bprintln!("Using macOS Accessibility API...");
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


fn list_all_windows() -> Result<Vec<MacOSWindow>, String> {
    // Get running application PIDs
    let running_pids = get_running_application_pids()
        .map_err(|e| format!("Failed to get running applications: {}", e))?;

    let mut windows = Vec::new();

    // Process each application
    for (app_idx, pid) in running_pids.iter().enumerate() {
        // Create an accessibility element for this application
        let app = AXUIElement::application(*pid);
        let app_name = get_application_name(*pid)
            .unwrap_or_else(|_| format!("App {}", app_idx + 1));

        // Try to get the application's windows using the windows() method
        match app.windows() {
            Ok(app_windows) => {
                // Process each window
                for (window_idx, window) in app_windows.into_iter().enumerate() {
                    // Get window title using the title() method
                    let window_title = window.title().map(|s| s.to_string())
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
            },
            Err(_) => {
                // Skip applications that don't have windows
                continue;
            }
        }
    }

    Ok(windows)
}


fn get_running_application_pids() -> Result<Vec<pid_t>, String> {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let running_apps: id = msg_send![workspace, runningApplications];
        let count: usize = msg_send![running_apps, count];

        let mut pids = Vec::with_capacity(count);

        for i in 0..count {
            let app: id = msg_send![running_apps, objectAtIndex: i];
            let pid: pid_t = msg_send![app, processIdentifier];

            pids.push(pid);
        }

        Ok(pids)
    }
}


fn get_application_name(pid: pid_t) -> Result<String, String> {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let running_apps: id = msg_send![workspace, runningApplications];
        let count: usize = msg_send![running_apps, count];

        for i in 0..count {
            let app: id = msg_send![running_apps, objectAtIndex: i];
            let app_pid: pid_t = msg_send![app, processIdentifier];

            if app_pid == pid {
                let app_name: id = msg_send![app, localizedName];
                let utf8: *const c_char = msg_send![app_name, UTF8String];
                if !utf8.is_null() {
                    return Ok(CStr::from_ptr(utf8).to_string_lossy().into_owned());
                }
            }
        }

        Err(format!("Could not find application name for PID {}", pid))
    }
}


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
        },
        Err(e) => {
            let error = format!("Failed to get window position: {}", e);
            crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
            Err(error)
        }
    }
}


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
        },
        Err(e) => {
            let error = format!("Failed to get window size: {}", e);
            crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
            Err(error)
        }
    }
}


fn get_window_by_app_and_index(app_name: &str, window_index: usize) -> Result<Option<MacOSWindow>, String> {
    let windows = list_all_windows()?;

    for window in windows {
        if window.app_name == app_name && window.index == window_index {
            return Ok(Some(window));
        }
    }

    Ok(None)
}


fn find_window_by_title(title: &str) -> Result<Option<MacOSWindow>, String> {
    let windows = list_all_windows()?;

    for window in windows {
        if window.window_title.contains(title) {
            return Ok(Some(window));
        }
    }

    Ok(None)
}


fn get_window_details(window: &MacOSWindow) -> Result<String, String> {
    crate::bprintln!(dev: "🖥️ SCREENDUMP: Getting window details for '{}' ({})", window.window_title, window.app_name);

    let mut result = String::new();

    // Basic window info
    result.push_str(&format!("Application: {}\n", window.app_name));
    result.push_str(&format!("Window: {}\n", window.window_title));
    result.push_str(&format!("Position: {}, {}\n", window.position.0, window.position.1));
    result.push_str(&format!("Size: {}×{}\n", window.size.0, window.size.1));

    // UI Elements
    result.push_str("\nUI Elements:\n");

    // Get children elements using the children() method
    crate::bprintln!(dev: "🖥️ SCREENDUMP: Fetching UI elements for window");
    match window.element.children() {
        Ok(children) => {
            if children.is_empty() {
                crate::bprintln!(dev: "🖥️ SCREENDUMP: No UI elements found in window");
                result.push_str("  No UI elements found\n");
            } else {
                crate::bprintln!(dev: "🖥️ SCREENDUMP: Found {} UI elements", children.len());
                // Process elements with recursive exploration
                let mut element_index = 0;
                for child in &children {
                    element_index += 1;
                    crate::bprintln!(dev: "🖥️ SCREENDUMP: Processing element {}", element_index);
                    result.push_str("  Element:\n");

                    // Get role using the role() method
                    if let Ok(role) = child.role() {
                        let role_str = role.to_string();
                        crate::bprintln!(dev: "🖥️ SCREENDUMP:   - Type: {}", role_str);
                        result.push_str(&format!("    Type: {}\n", role_str));
                    }

                    // Get description using the description() method
                    if let Ok(desc) = child.description() {
                        let desc_str = desc.to_string();
                        if !desc_str.is_empty() {
                            crate::bprintln!(dev: "🖥️ SCREENDUMP:   - Description: {}", desc_str);
                            result.push_str(&format!("    Description: {}\n", desc_str));
                        }
                    }

                    // Get value using the value() method
                    if let Ok(value) = child.value() {
                        crate::bprintln!(dev: "🖥️ SCREENDUMP:   - Value: {:?}", value);
                        result.push_str(&format!("    Value: {:?}\n", value));
                    }

                    // Try to get position of element if available
                    if let Ok(position) = child.position() {
                        if let Some(point) = position.get_value::<CGPoint>().ok() {
                            let x = point.x as i32;
                            let y = point.y as i32;
                            crate::bprintln!(dev: "🖥️ SCREENDUMP:   - Position: ({}, {})", x, y);
                            result.push_str(&format!("    Position: ({}, {})\n", x, y));
                        }
                    }

                    // Try to get size of element if available
                    if let Ok(size) = child.size() {
                        if let Some(sz) = size.get_value::<CGSize>().ok() {
                            let width = sz.width as i32;
                            let height = sz.height as i32;
                            crate::bprintln!(dev: "🖥️ SCREENDUMP:   - Size: {}×{}", width, height);
                            result.push_str(&format!("    Size: {}×{}\n", width, height));
                        }
                    }
                    
                    // Try to get child elements (one level deep)
                    if let Ok(sub_children) = child.children() {
                        if !sub_children.is_empty() {
                            crate::bprintln!(dev: "🖥️ SCREENDUMP:   - Found {} child elements", sub_children.len());
                            result.push_str(&format!("    Child Elements: {}\n", sub_children.len()));
                            
                            for (subindex, subchild) in sub_children.iter().enumerate().take(5) {
                                let mut subinfo = String::new();
                                
                                if let Ok(role) = subchild.role() {
                                    subinfo.push_str(&format!("{}", role.to_string()));
                                }
                                
                                if let Ok(desc) = subchild.description() {
                                    let desc_str = desc.to_string();
                                    if !desc_str.is_empty() {
                                        if !subinfo.is_empty() {
                                            subinfo.push_str(" - ");
                                        }
                                        subinfo.push_str(&desc_str);
                                    }
                                }
                                
                                if subinfo.is_empty() {
                                    subinfo = "Unknown".to_string();
                                }
                                
                                crate::bprintln!(dev: "🖥️ SCREENDUMP:     + Child {}: {}", subindex + 1, subinfo);
                                result.push_str(&format!("      Child {}: {}\n", subindex + 1, subinfo));
                            }
                            
                            if sub_children.len() > 5 {
                                result.push_str(&format!("      ... and {} more children\n", sub_children.len() - 5));
                            }
                        }
                    }
                }
            }
        },
        Err(e) => {
            crate::bprintln!(error: "🖥️ SCREENDUMP: Failed to get UI elements: {}", e);
            result.push_str(&format!("  Failed to get UI elements: {}\n", e));
        }
    }

    Ok(result)
}


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

pub fn get_macos_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
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
                    window.size.1
                ))
            },
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
                    window.size.1
                ))
            },
            None => {
                let error = format!("Window with title containing '{}' not found", window_id);
                crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
                Err(error)
            }
        }
    }
}
