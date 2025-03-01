use crate::tools::ToolResult;
use crate::constants::FORMAT_BOLD;

pub fn execute_fetch(args: &str, _body: &str) -> ToolResult {
    // Extract URL directly from args
    let url = args.trim();
    
    // Check if URL is provided and valid
    if url.is_empty() {
        return ToolResult {
            success: false,
            user_output: "Error: No URL provided. Usage: fetch https://example.com".to_string(),
            agent_output: "Error: No URL provided. Usage: fetch https://example.com".to_string(),
        }
    }
    
    // Make the request
    let result = match ureq::get(url).call() {
        Ok(response) => {
            if response.status() != 200 {
                return ToolResult {
                    success: false,
                    user_output: format!("Error fetching URL: HTTP status {}", response.status()),
                    agent_output: format!("Error fetching URL: HTTP status {}", response.status()),
                }
            }
            
            // Try to get content type to handle differently
            let content_type = response.header("content-type")
                .unwrap_or("text/html")
                .to_string(); // Clone the string to own it
            
            // Move the response to get the text content
            if let Ok(text) = response.into_string() {
                // Process content based on content type
                let processed_text = if content_type.contains("text/html") {
                    // Automatically convert HTML to more readable format
                    html2md::parse_html(&text)
                } else {
                    // Keep non-HTML content as is
                    text
                };
                
                // Truncate large responses for user output
                let user_text = if processed_text.len() > 2000 {
                    let truncated = &processed_text[0..1997];
                    format!("{}...\n\n[Content truncated, total length: {} characters]", truncated, processed_text.len())
                } else {
                    processed_text.clone()
                };
                
                ToolResult {
                    success: true,
                    user_output: format!("{}{} - Content fetched successfully:{}  \n\n{}", 
                                        FORMAT_BOLD, url, FORMAT_BOLD, user_text),
                    agent_output: format!("Fetched from {}:\n\n{}", url, processed_text),
                }
            } else {
                ToolResult {
                    success: false,
                    user_output: "Error: Could not read response as text".to_string(),
                    agent_output: "Error: Could not read response as text".to_string(),
                }
            }
        },
        Err(err) => {
            ToolResult {
                success: false,
                user_output: format!("Error fetching URL: {}", err),
                agent_output: format!("Error fetching URL: {}", err),
            }
        }
    };
    
    result
}