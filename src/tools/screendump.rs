//! Tool for capturing the current UI display using accessibility APIs
//! 
//! This tool allows agents to capture the current UI structure as text,
//! providing information about window layout, controls, and hierarchies.

use crate::tools::ToolResult;
use std::env;
use std::process::Command;

/// Execute the UI dump tool to capture accessibility tree
pub async fn execute_screendump(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    let platform = env::consts::OS;
    
    // Parse the command arguments
    let command = parse_command(args);
    
    // Log tool invocation
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
        // Default behavior is now to list windows
        return ScreendumpCommand::ListWindows;
    }
    
    let parts: Vec<&str> = args.split_whitespace().collect();
    
    match parts[0].to_lowercase().as_str() {
        "fullscreen" | "full" => ScreendumpCommand::FullScreen,
        "list" => ScreendumpCommand::ListWindows,
        "window" | "win" => {
            if parts.len() > 1 {
                // Everything after "window" is treated as the identifier
                let window_id = parts[1..].join(" ");
                ScreendumpCommand::WindowDetails(window_id)
            } else {
                // "window" without an ID defaults to listing windows
                ScreendumpCommand::ListWindows
            }
        },
        // If first word is a number, treat it as a window ID
        id if id.parse::<i32>().is_ok() => {
            ScreendumpCommand::WindowDetails(id.to_string())
        },
        // Otherwise, if there's text, assume it's a window identifier
        _ => {
            // Treat the entire args as a window name/identifier
            ScreendumpCommand::WindowDetails(args.to_string())
        }
    }
}

/// Capture UI hierarchy on macOS using Accessibility APIs
fn capture_macos_ui(command: ScreendumpCommand, silent_mode: bool) -> ToolResult {
    if !silent_mode {
        crate::bprintln!("Using macOS Accessibility API via AppleScript...");
    }

    // Prepare the script based on command
    let script = match command {
        ScreendumpCommand::ListWindows => build_macos_list_windows_script(),
        ScreendumpCommand::WindowDetails(ref id) => build_macos_window_details_script(id),
        ScreendumpCommand::FullScreen => build_macos_full_screen_script(),
    };
    
    // Log script being executed (truncated for brevity if needed)
    let script_preview = if script.len() > 100 {
        format!("{}...[{} more chars]", &script[..100], script.len() - 100)
    } else {
        script.clone()
    };
    crate::bprintln!(dev: "Executing script: {}", script_preview);
    
    // Execute the script
    match Command::new("sh")
        .arg("-c")
        .arg(script)
        .output() 
    {
        Ok(result) => {
            crate::bprintln!(dev: "Script execution completed with status: {}", result.status);
            
            if result.status.success() {
                let ui_text = String::from_utf8_lossy(&result.stdout).to_string();
                
                // Log the raw output for development purposes
                crate::bprintln!(dev: "Screendump raw output:\n{}", ui_text);
                
                if ui_text.trim().is_empty() {
                    crate::bprintln!(dev: "ERROR: Empty UI hierarchy output");
                    ToolResult::error("No UI hierarchy information available")
                } else {
                    let formatted_output = match command {
                        ScreendumpCommand::ListWindows => format!("Available Windows:\n{}", ui_text),
                        ScreendumpCommand::WindowDetails(id) => format!("Window Details for '{}':\n{}", id, ui_text),
                        ScreendumpCommand::FullScreen => format!("Full Screen UI Hierarchy:\n{}", ui_text),
                    };
                    
                    // Log the formatted output that will be returned
                    crate::bprintln!(dev: "Screendump formatted output:\n{}", formatted_output);
                    
                    ToolResult::success(formatted_output)
                }
            } else {
                let error = String::from_utf8_lossy(&result.stderr).to_string();
                crate::bprintln!(dev: "ERROR: Script execution failed: {}", error);
                
                // Also log stdout in case it contains useful information
                if !result.stdout.is_empty() {
                    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
                    crate::bprintln!(dev: "Script stdout: {}", stdout);
                }
                
                ToolResult::error(format!("Failed to capture UI hierarchy: {}", error))
            }
        },
        Err(e) => {
            crate::bprintln!(dev: "ERROR: Command execution failed: {}", e);
            ToolResult::error(format!("Failed to execute command: {}", e))
        }
    }
}

/// Build AppleScript for listing all windows with identifiers
fn build_macos_list_windows_script() -> String {
    crate::bprintln!(dev: "Building macOS list windows script");
    
    r#"osascript -e '
    tell application "System Events"
        set windowList to ""
        set windowCount to 0
        
        -- Iterate through all visible applications
        set visibleApps to application processes where visible is true
        
        repeat with currentApp in visibleApps
            set appName to name of currentApp
            
            -- Get windows for this application
            try
                set appWindows to windows of currentApp
                repeat with i from 1 to count of appWindows
                    set currentWindow to window i of currentApp
                    set windowTitle to name of currentWindow
                    
                    -- Create a unique window identifier (app name + index)
                    set windowId to appName & ":" & i
                    
                    -- Format: [ID] App Name: Window Title
                    set windowCount to windowCount + 1
                    set windowList to windowList & "[" & windowId & "] " & appName & ": " & windowTitle & "
"
                end repeat
            end try
        end repeat
        
        -- Add usage note if windows were found
        if windowCount > 0 then
            set windowList to windowList & "
Use 'screendump window [ID]' or 'screendump window [Window Title]' to view details for a specific window."
        else
            set windowList to "No visible windows found."
        end if
        
        return windowList
    end tell'"#.to_string()
}

/// Build AppleScript for capturing details of a specific window
fn build_macos_window_details_script(window_id: &str) -> String {
    crate::bprintln!(dev: "Building macOS window details script for ID: {}", window_id);
    
    // Check if the ID is in the format "AppName:Index"
    let parts: Vec<&str> = window_id.split(':').collect();
    
    if parts.len() == 2 {
        if let Ok(window_index) = parts[1].parse::<usize>() {
            // We have an app name and window index
            return build_macos_app_window_script(parts[0], window_index);
        }
    }
    
    // Otherwise, try to find the window by title
    r#"osascript -e '
    tell application "System Events"
        set targetId = "#.to_string() + "\"" + window_id + "\"" + r#"
        set foundWindow to false
        set output to ""
        
        -- Try to find the window by the identifier
        set visibleApps to application processes where visible is true
        
        repeat with currentApp in visibleApps
            set appName to name of currentApp
            
            -- Check if this is a direct app:index reference
            if targetId starts with appName & ":" then
                set indexStr to text ((length of appName) + 2) through -1 of targetId
                try
                    set windowIndex to indexStr as number
                    if windowIndex > 0 and windowIndex <= count of windows of currentApp then
                        set currentWindow to window windowIndex of currentApp
                        set foundWindow to true
                        
                        -- Window information
                        set output to "Application: " & appName & "
"
                        set windowName to name of currentWindow
                        set windowPosition to position of currentWindow
                        set windowSize to size of currentWindow
                        
                        set output to output & "Window: " & windowName & "
"
                        set output to output & "Position: " & item 1 of windowPosition & ", " & item 2 of windowPosition & "
"
                        set output to output & "Size: " & item 1 of windowSize & "×" & item 2 of windowSize & "
"
                        
                        -- Get UI elements in the window
                        set output to output & "
UI Elements:
"
                        try
                            set uiElements to UI elements of currentWindow
                            repeat with elem in uiElements
                                set output to output & "  Element:
"
                                try
                                    set output to output & "    Type: " & role of elem & "
"
                                end try
                                try
                                    if description of elem is not "" then
                                        set output to output & "    Description: " & description of elem & "
"
                                    end if
                                end try
                                try
                                    if value of elem is not "" then
                                        set output to output & "    Value: " & value of elem & "
"
                                    end if
                                end try
                            end repeat
                        end try
                        
                        exit repeat
                    end if
                end try
            end if
            
            -- If not found by ID, check window titles
            if not foundWindow then
                try
                    set windowList to windows of currentApp
                    repeat with i from 1 to count of windowList
                        set currentWindow to item i of windowList
                        set windowTitle to name of currentWindow
                        
                        -- Check if window title contains our target
                        if windowTitle contains targetId then
                            set foundWindow to true
                            
                            -- Window information
                            set output to "Application: " & appName & "
"
                            set output to output & "Window: " & windowTitle & "
"
                            set windowPosition to position of currentWindow
                            set windowSize to size of currentWindow
                            
                            set output to output & "Position: " & item 1 of windowPosition & ", " & item 2 of windowPosition & "
"
                            set output to output & "Size: " & item 1 of windowSize & "×" & item 2 of windowSize & "
"
                            
                            -- Get UI elements in the window
                            set output to output & "
UI Elements:
"
                            try
                                set uiElements to UI elements of currentWindow
                                repeat with elem in uiElements
                                    set output to output & "  Element:
"
                                    try
                                        set output to output & "    Type: " & role of elem & "
"
                                    end try
                                    try
                                        if description of elem is not "" then
                                            set output to output & "    Description: " & description of elem & "
"
                                        end if
                                    end try
                                    try
                                        if value of elem is not "" then
                                            set output to output & "    Value: " & value of elem & "
"
                                        end if
                                    end try
                                end repeat
                            end try
                            
                            exit repeat
                        end if
                    end repeat
                end try
            end if
            
            -- If found the window, exit the loop
            if foundWindow then
                exit repeat
            end if
        end repeat
        
        -- Return appropriate message
        if not foundWindow then
            return "Window '" & targetId & "' not found. Use 'screendump' without arguments to list available windows."
        else
            return output
        end if
    end tell'"#
}

/// Build a script targeting a specific app and window index
fn build_macos_app_window_script(app_name: &str, window_index: usize) -> String {
    crate::bprintln!(dev: "Building macOS app window script for app: {}, index: {}", app_name, window_index);
    
    r#"osascript -e '
    tell application "System Events"
        set targetApp = "#.to_string() + "\"" + app_name + "\"" + r#"
        set targetIndex = "# + &window_index.to_string() + r#"
        set foundWindow to false
        set output to ""
        
        -- Try to find the application
        try
            set appProcess to first application process whose name is targetApp
            
            -- Check if the window index is valid
            if targetIndex > 0 and targetIndex <= count of windows of appProcess then
                set currentWindow to window targetIndex of appProcess
                set foundWindow to true
                
                -- Window information
                set output to "Application: " & targetApp & "
"
                set windowName to name of currentWindow
                set windowPosition to position of currentWindow
                set windowSize to size of currentWindow
                
                set output to output & "Window: " & windowName & "
"
                set output to output & "Position: " & item 1 of windowPosition & ", " & item 2 of windowPosition & "
"
                set output to output & "Size: " & item 1 of windowSize & "×" & item 2 of windowSize & "
"
                
                -- Get UI elements in the window
                set output to output & "
UI Elements:
"
                try
                    set uiElements to UI elements of currentWindow
                    repeat with elem in uiElements
                        set output to output & "  Element:
"
                        try
                            set output to output & "    Type: " & role of elem & "
"
                        end try
                        try
                            if description of elem is not "" then
                                set output to output & "    Description: " & description of elem & "
"
                            end if
                        end try
                        try
                            if value of elem is not "" then
                                set output to output & "    Value: " & value of elem & "
"
                            end if
                        end try
                    end repeat
                end try
            else
                set output to "Window index " & targetIndex & " not found for application '" & targetApp & "'."
            end if
        on error
            set output to "Application '" & targetApp & "' not found or not accessible."
        end try
        
        return output
    end tell'"#
}

/// Public function to get a window's rectangle by ID
/// Returns (app_name, window_id, x, y, width, height) if window is found
pub fn get_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
    crate::bprintln!(dev: "Getting window rect for ID: {}", window_id);
    
    let platform = env::consts::OS;
    
    match platform {
        "macos" => get_macos_window_rect(window_id),
        _ => Err(format!("Window rect retrieval not implemented for {} platform", platform))
    }
}

/// Get a window's rectangle on macOS
fn get_macos_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
    // Create the script to get window rect
    let script = build_macos_window_rect_script(window_id);
    
    // Execute the script
    match Command::new("sh")
        .arg("-c")
        .arg(script)
        .output() 
    {
        Ok(result) => {
            if result.status.success() {
                let output = String::from_utf8_lossy(&result.stdout).to_string();
                
                if output.starts_with("ERROR:") {
                    return Err(output.trim().to_string());
                }
                
                // Parse the output (format: "AppName|WindowTitle|X|Y|Width|Height")
                let parts: Vec<&str> = output.trim().split('|').collect();
                if parts.len() == 6 {
                    let app_name = parts[0].to_string();
                    let window_title = parts[1].to_string();
                    
                    let x = parts[2].parse::<i32>().map_err(|_| "Invalid X coordinate")?;
                    let y = parts[3].parse::<i32>().map_err(|_| "Invalid Y coordinate")?;
                    let width = parts[4].parse::<i32>().map_err(|_| "Invalid width")?;
                    let height = parts[5].parse::<i32>().map_err(|_| "Invalid height")?;
                    
                    return Ok((app_name, window_title, x, y, width, height));
                } else {
                    return Err(format!("Invalid window rect format: {}", output));
                }
            } else {
                let error = String::from_utf8_lossy(&result.stderr).to_string();
                return Err(format!("Failed to get window rect: {}", error));
            }
        },
        Err(e) => {
            return Err(format!("Failed to execute command: {}", e));
        }
    }
}

/// Build AppleScript to get a window's rectangle
fn build_macos_window_rect_script(window_id: &str) -> String {
    // Check if the ID is in the format "AppName:Index"
    let parts: Vec<&str> = window_id.split(':').collect();
    
    if parts.len() == 2 {
        if let Ok(window_index) = parts[1].parse::<usize>() {
            // We have an app name and window index
            let script = format!("osascript -e 'tell application \"System Events\"\n\
                set targetApp to \"{}\"\n\
                set targetIndex to {}\n\
                \n\
                try\n\
                    set appProcess to first application process whose name is targetApp\n\
                    \n\
                    if targetIndex > 0 and targetIndex <= count of windows of appProcess then\n\
                        set currentWindow to window targetIndex of appProcess\n\
                        set windowTitle to name of currentWindow\n\
                        set windowPosition to position of currentWindow\n\
                        set windowSize to size of currentWindow\n\
                        \n\
                        set x to item 1 of windowPosition\n\
                        set y to item 2 of windowPosition\n\
                        set width to item 1 of windowSize\n\
                        set height to item 2 of windowSize\n\
                        \n\
                        return targetApp & \"|\" & windowTitle & \"|\" & x & \"|\" & y & \"|\" & width & \"|\" & height\n\
                    else\n\
                        return \"ERROR: Window index \" & targetIndex & \" not found for application '\" & targetApp & \"'.\"\n\
                    end if\n\
                on error errMsg\n\
                    return \"ERROR: \" & errMsg\n\
                end try\n\
            end tell'", parts[0], window_index);
            
            return script;
        }
    }
    
    // Otherwise, try to find the window by title
    let script = format!("osascript -e 'tell application \"System Events\"\n\
        set targetId to \"{}\"\n\
        set foundWindow to false\n\
        \n\
        set visibleApps to application processes where visible is true\n\
        \n\
        repeat with currentApp in visibleApps\n\
            set appName to name of currentApp\n\
            \n\
            -- Check if this is a direct app:index reference\n\
            if targetId starts with appName & \":\" then\n\
                set indexStr to text ((length of appName) + 2) through -1 of targetId\n\
                try\n\
                    set windowIndex to indexStr as number\n\
                    if windowIndex > 0 and windowIndex <= count of windows of currentApp then\n\
                        set currentWindow to window windowIndex of currentApp\n\
                        set windowTitle to name of currentWindow\n\
                        set windowPosition to position of currentWindow\n\
                        set windowSize to size of currentWindow\n\
                        \n\
                        set x to item 1 of windowPosition\n\
                        set y to item 2 of windowPosition\n\
                        set width to item 1 of windowSize\n\
                        set height to item 2 of windowSize\n\
                        \n\
                        return appName & \"|\" & windowTitle & \"|\" & x & \"|\" & y & \"|\" & width & \"|\" & height\n\
                    end if\n\
                end try\n\
            end if\n\
            \n\
            -- If not found by ID, check window titles\n\
            if not foundWindow then\n\
                try\n\
                    set windowList to windows of currentApp\n\
                    repeat with i from 1 to count of windowList\n\
                        set currentWindow to item i of windowList\n\
                        set windowTitle to name of currentWindow\n\
                        \n\
                        if windowTitle contains targetId then\n\
                            set windowPosition to position of currentWindow\n\
                            set windowSize to size of currentWindow\n\
                            \n\
                            set x to item 1 of windowPosition\n\
                            set y to item 2 of windowPosition\n\
                            set width to item 1 of windowSize\n\
                            set height to item 2 of windowSize\n\
                            \n\
                            return appName & \"|\" & windowTitle & \"|\" & x & \"|\" & y & \"|\" & width & \"|\" & height\n\
                        end if\n\
                    end repeat\n\
                end try\n\
            end if\n\
        end repeat\n\
        \n\
        return \"ERROR: Window '\" & targetId & \"' not found\"\n\
    end tell'", window_id);
    
    script
}

/// Build AppleScript for capturing full screen UI hierarchy
fn build_macos_full_screen_script() -> String {
    crate::bprintln!(dev: "Building macOS full screen UI script");
    
    r#"osascript -e '
    tell application "System Events"
        set appList to ""
        set visibleApps to application processes where visible is true
        
        repeat with currentApp in visibleApps
            set appName to name of currentApp
            set appList to appList & "Application: " & appName & "
"
            
            try
                repeat with i from 1 to count of windows of currentApp
                    try
                        set currentWindow to window i of currentApp
                        set windowName to name of currentWindow
                        set windowPosition to position of currentWindow
                        set windowSize to size of currentWindow
                        
                        set appList to appList & "  Window " & i & ": " & windowName & "
"
                        set appList to appList & "    Position: " & item 1 of windowPosition & ", " & item 2 of windowPosition & "
"
                        set appList to appList & "    Size: " & item 1 of windowSize & "×" & item 2 of windowSize & "
"
                    end try
                end repeat
            end try
            
            set appList to appList & "
"
        end repeat
        
        return appList
    end tell'"#.to_string()
}

/// Capture UI hierarchy on Windows
fn capture_windows_ui(command: ScreendumpCommand) -> ToolResult {
    // Windows implementation would use UI Automation API
    let description = match command {
        ScreendumpCommand::ListWindows => "list windows",
        ScreendumpCommand::WindowDetails(ref id) => &format!("window details for '{}'", id),
        ScreendumpCommand::FullScreen => "full screen",
    };
    
    ToolResult::error(format!("Windows UI capture ({}) not yet implemented", description))
}

/// Capture UI hierarchy on Linux
fn capture_linux_ui(command: ScreendumpCommand) -> ToolResult {
    // Linux implementation would use AT-SPI or similar
    let description = match command {
        ScreendumpCommand::ListWindows => "list windows",
        ScreendumpCommand::WindowDetails(ref id) => &format!("window details for '{}'", id),
        ScreendumpCommand::FullScreen => "full screen",
    };
    
    ToolResult::error(format!("Linux UI capture ({}) not yet implemented", description))
}