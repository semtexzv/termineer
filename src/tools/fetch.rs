use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_RESET, FORMAT_GRAY};

/// Strips HTML tags and JavaScript from the input text
fn strip_html_and_js(html: &str) -> String {
    // A simpler but more robust approach to strip HTML
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut tag_buffer = String::new();
    
    // Use a character-based approach rather than byte indexing
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        // Handle tags
        if !in_tag && i < chars.len() - 1 && chars[i] == '<' {
            in_tag = true;
            tag_buffer.clear();
            tag_buffer.push('<');
            i += 1;
            continue;
        }
        
        if in_tag {
            tag_buffer.push(chars[i]);
            
            // Check for end of tag
            if chars[i] == '>' {
                in_tag = false;
                let tag = tag_buffer.to_lowercase();
                
                // Track script and style sections
                if tag.starts_with("<script") {
                    in_script = true;
                } else if tag == "</script>" {
                    in_script = false;
                } else if tag.starts_with("<style") {
                    in_style = true;
                } else if tag == "</style>" {
                    in_style = false;
                }
                // Special handling for structural tags
                else if tag == "</p>" || tag == "</div>" || tag == "</h1>" || 
                       tag == "</h2>" || tag == "</h3>" || tag == "</h4>" || 
                       tag == "</h5>" || tag == "</h6>" || tag == "<br>" || 
                       tag == "<br/>" || tag == "</li>" || tag == "</tr>" || 
                       tag == "</td>" {
                    result.push('\n');
                } else if tag == "<hr>" || tag == "<hr/>" {
                    result.push_str("\n----------\n");
                }
            }
            
            i += 1;
            continue;
        }
        
        // Skip content inside script and style tags
        if in_script || in_style {
            i += 1;
            continue;
        }
        
        // Handle HTML entities
        if chars[i] == '&' && i < chars.len() - 2 {
            let mut entity = String::new();
            let start_idx = i;
            
            // Collect the entity
            while i < chars.len() && chars[i] != ';' && entity.len() < 10 {
                entity.push(chars[i]);
                i += 1;
            }
            
            // Only process if we found a semicolon
            if i < chars.len() && chars[i] == ';' {
                entity.push(';');
                i += 1;
                
                // Replace common entities
                match entity.as_str() {
                    "&nbsp;" => result.push(' '),
                    "&lt;" => result.push('<'),
                    "&gt;" => result.push('>'),
                    "&amp;" => result.push('&'),
                    "&quot;" => result.push('"'),
                    "&apos;" => result.push('\''),
                    _ => {
                        // For unknown entities, just add the original character
                        result.push('&');
                    }
                }
            } else {
                // Not a valid entity, reset and just add the '&'
                i = start_idx + 1;
                result.push('&');
            }
            
            continue;
        }
        
        // Regular character - add to result
        result.push(chars[i]);
        i += 1;
    }
    
    // Post-processing: clean up multiple newlines and spaces
    let mut cleaned = String::new();
    let mut last_char_was_newline = false;
    let mut last_char_was_space = false;
    
    for c in result.chars() {
        if c == '\n' {
            if !last_char_was_newline {
                cleaned.push('\n');
                last_char_was_newline = true;
                last_char_was_space = false;
            }
        } else if c.is_whitespace() {
            if !last_char_was_space && !last_char_was_newline {
                cleaned.push(' ');
                last_char_was_space = true;
                last_char_was_newline = false;
            }
        } else {
            cleaned.push(c);
            last_char_was_newline = false;
            last_char_was_space = false;
        }
    }
    
    cleaned.trim().to_string()
}

enum SummaryLength {
    Short,  // ~100-200 words
    Medium, // ~300-500 words
    Long,   // ~700-1000 words
    Custom(usize), // Custom word count target
}

impl SummaryLength {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "short" => Some(SummaryLength::Short),
            "medium" => Some(SummaryLength::Medium),
            "long" => Some(SummaryLength::Long),
            custom if custom.parse::<usize>().is_ok() => {
                Some(SummaryLength::Custom(custom.parse().unwrap()))
            },
            _ => None,
        }
    }
    
    fn to_word_count(&self) -> usize {
        match self {
            SummaryLength::Short => 150,
            SummaryLength::Medium => 400,
            SummaryLength::Long => 800,
            SummaryLength::Custom(count) => *count,
        }
    }
}

/// Process arguments for the fetch command
fn parse_fetch_args(args: &str) -> (String, bool, Option<SummaryLength>) {
    let mut url = String::new();
    let mut summarize = false;
    let mut length = None;
    
    // Split args by whitespace
    let parts: Vec<&str> = args.split_whitespace().collect();
    let mut i = 0;
    
    while i < parts.len() {
        match parts[i] {
            "--summarize" | "-s" => {
                summarize = true;
                i += 1;
            },
            "--length" | "-l" => {
                if i + 1 < parts.len() {
                    length = SummaryLength::from_str(parts[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            },
            part => {
                if url.is_empty() {
                    url = part.to_string();
                }
                i += 1;
            }
        }
    }
    
    // If summarize is true but no length specified, default to medium
    if summarize && length.is_none() {
        length = Some(SummaryLength::Medium);
    }
    
    (url, summarize, length)
}

/// Summarize text using Gemini
async fn summarize_text(text: &str, length: &SummaryLength) -> Result<String, String> {
    use crate::llm::factory::create_backend_for_task;
    use crate::llm::{Message, MessageInfo, Content};
    
    // Create a Gemini backend for summarization
    // Use correct Gemini model identifier - the flash model is designed for fast responses
    let backend = match create_backend_for_task(Some("google/gemini-1.5-flash-001")) {
        Ok(backend) => backend,
        Err(e) => return Err(format!("Failed to create summarization backend: {}", e)),
    };
    
    // Prepare the prompt based on desired length
    let word_count = length.to_word_count();
    let prompt = format!(
        "Summarize the following content in approximately {} words. Focus on the main points and key information:\n\n{}",
        word_count,
        text
    );
    
    // Create message for the model
    let message = Message::text(
        "user",
        prompt,
        MessageInfo::User,
    );
    
    // Send to the model - use 1000 tokens max for summaries to keep them concise
    let max_tokens = Some(1000);
    let response = match backend.send_message(&[message], None, None, None, None, max_tokens).await {
        Ok(response) => response,
        Err(e) => return Err(format!("Summarization failed: {}", e)),
    };
    
    // Extract the text from the response
    if let Some(Content::Text { text }) = response.content.first() {
        Ok(text.clone())
    } else {
        Err("Failed to get summary text from model response".to_string())
    }
}

pub async fn execute_fetch(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Parse arguments
    let (url, do_summarize, summary_length) = parse_fetch_args(args);
    
    // Check if URL is provided and valid
    if url.is_empty() {
        let error_msg = "Error: No URL provided. Usage: fetch [--summarize] [--length short|medium|long|<word_count>] https://example.com".to_string();
        
        if !silent_mode {
            println!("{}❌ Error:{} {}", 
                FORMAT_BOLD, FORMAT_RESET, error_msg);
        }
        
        return ToolResult {
            success: false,
            agent_output: error_msg,
        }
    }
    
    // Make the request using reqwest
    let client = reqwest::Client::new();
    let response = match client.get(&url).send().await {
        Ok(response) => response,
        Err(err) => {
            let error_msg = format!("Error fetching URL: {}", err);
            
            if !silent_mode {
                println!("{}❌ Error:{} {}", 
                    FORMAT_BOLD, FORMAT_RESET, error_msg);
            }
            
            return ToolResult {
                success: false,
                agent_output: error_msg,
            }
        }
    };
    
    // Check status code
    if !response.status().is_success() {
        let error_msg = format!("Error fetching URL: HTTP status {}", response.status());
        
        if !silent_mode {
            println!("{}❌ Error:{} {}", 
                FORMAT_BOLD, FORMAT_RESET, error_msg);
        }
        
        return ToolResult {
            success: false,
            agent_output: error_msg,
        }
    }
    
    // Try to get content type
    let content_type = response.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();
    
    // Get text content
    let text = match response.text().await {
        Ok(text) => text,
        Err(err) => {
            let error_msg = format!("Error reading response: {}", err);
            
            if !silent_mode {
                println!("{}❌ Error:{} {}", 
                    FORMAT_BOLD, FORMAT_RESET, error_msg);
            }
            
            return ToolResult {
                success: false,
                agent_output: error_msg,
            }
        }
    };
    
    // Process text based on content type
    let processed_text = if content_type.contains("text/html") || 
                            content_type.contains("application/xhtml") {
        strip_html_and_js(&text)
    } else {
        // For plain text, JSON, or other formats, use as-is
        text
    };
    
    // Truncate large responses for user output - show first 1000 and last 1000 characters
    let user_text = if processed_text.len() > 2000 {
        let first_part = &processed_text[0..1000];
        let last_part = &processed_text[processed_text.len() - 1000..];
        format!(
            "{}...\n\n[... {} characters truncated ...]\n\n{}\n\n[Total content length: {} characters]",
            first_part,
            processed_text.len() - 2000,
            last_part,
            processed_text.len()
        )
    } else {
        processed_text.clone()
    };
    
    // Process summarization if requested
    if do_summarize {
        match summary_length {
            Some(length) => {
                match summarize_text(&processed_text, &length).await {
                    Ok(summary) => {
                        // Print output with summary if not in silent mode
                        if !silent_mode {
                            println!("{}🌐 Fetch:{} {} - Content summarized successfully", 
                                FORMAT_BOLD, FORMAT_RESET, url);
                            println!("{}{}{}", FORMAT_GRAY, summary, FORMAT_RESET);
                        }
                        
                        // Return summarized content with info about original
                        let word_count = processed_text.split_whitespace().count();
                        ToolResult {
                            success: true,
                            agent_output: format!(
                                "Fetched and summarized from {}:\n\nOriginal content length: ~{} words\n\n{}", 
                                url, 
                                word_count,
                                summary
                            ),
                        }
                    },
                    Err(err) => {
                        // Summarization failed, log error and fall back to original content
                        if !silent_mode {
                            println!("{}⚠️ Warning:{} Failed to summarize content: {}", 
                                FORMAT_BOLD, FORMAT_RESET, err);
                            println!("{}🌐 Fetch:{} {} - Returning full content instead", 
                                FORMAT_BOLD, FORMAT_RESET, url);
                            println!("{}{}{}", FORMAT_GRAY, user_text, FORMAT_RESET);
                        }
                        
                        ToolResult {
                            success: true,
                            agent_output: format!("Fetched from {} (summarization failed: {}):\n\n{}", 
                                                    url, err, processed_text),
                        }
                    }
                }
            },
            None => {
                // No length specified (shouldn't happen with our parsing logic)
                if !silent_mode {
                    println!("{}🌐 Fetch:{} {} - Content fetched successfully", 
                        FORMAT_BOLD, FORMAT_RESET, url);
                    println!("{}{}{}", FORMAT_GRAY, user_text, FORMAT_RESET);
                }
                
                ToolResult {
                    success: true,
                    agent_output: format!("Fetched from {}:\n\n{}", url, processed_text),
                }
            }
        }
    } else {
        // Standard fetch without summarization
        if !silent_mode {
            println!("{}🌐 Fetch:{} {} - Content fetched successfully", 
                FORMAT_BOLD, FORMAT_RESET, url);
            println!("{}{}{}", FORMAT_GRAY, user_text, FORMAT_RESET);
        }
        
        ToolResult {
            success: true,
            agent_output: format!("Fetched from {}:\n\n{}", url, processed_text),
        }
    }
}