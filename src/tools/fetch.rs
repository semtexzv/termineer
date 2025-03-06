use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;

/// Strips HTML tags and JavaScript from the input text, but preserves links
/// in a format like "link text [URL]" for better readability
fn strip_html_and_js(html: &str) -> String {
    // A simpler but more robust approach to strip HTML
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut in_anchor = false;
    let mut tag_buffer = String::new();
    let mut link_url = String::new();
    let mut link_text = String::new();

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
                // Handle anchor tags specially to preserve links
                else if tag.starts_with("<a ") {
                    in_anchor = true;
                    link_text.clear();
                    link_url.clear();

                    // Extract href attribute
                    if let Some(href_start) = tag.find("href=\"") {
                        let href_content_start = href_start + 6; // 6 = length of href="
                        if let Some(href_end) = tag[href_content_start..].find('"') {
                            link_url = tag[href_content_start..(href_content_start + href_end)]
                                .to_string();
                        }
                    } else if let Some(href_start) = tag.find("href='") {
                        let href_content_start = href_start + 6; // 6 = length of href='
                        if let Some(href_end) = tag[href_content_start..].find('\'') {
                            link_url = tag[href_content_start..(href_content_start + href_end)]
                                .to_string();
                        }
                    }
                } else if tag == "</a>" {
                    in_anchor = false;
                    // Only add the link if we have both text and URL
                    if !link_text.is_empty() && !link_url.is_empty() {
                        result.push_str(&format!("{} [{}]", link_text.trim(), link_url));
                    } else if !link_text.is_empty() {
                        result.push_str(&link_text);
                    }
                }
                // Special handling for structural tags
                else if tag == "</p>"
                    || tag == "</div>"
                    || tag == "</h1>"
                    || tag == "</h2>"
                    || tag == "</h3>"
                    || tag == "</h4>"
                    || tag == "</h5>"
                    || tag == "</h6>"
                    || tag == "<br>"
                    || tag == "<br/>"
                    || tag == "</li>"
                    || tag == "</tr>"
                    || tag == "</td>"
                {
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

                // Get the processed entity character
                let entity_char = match entity.as_str() {
                    "&nbsp;" => ' ',
                    "&lt;" => '<',
                    "&gt;" => '>',
                    "&amp;" => '&',
                    "&quot;" => '"',
                    "&apos;" => '\'',
                    _ => '&', // For unknown entities
                };

                // Add to link text if in anchor, otherwise to main result
                if in_anchor {
                    link_text.push(entity_char);
                } else {
                    result.push(entity_char);
                }
            } else {
                // Not a valid entity, reset and just add the '&'
                i = start_idx + 1;

                // Add to link text if in anchor, otherwise to main result
                if in_anchor {
                    link_text.push('&');
                } else {
                    result.push('&');
                }
            }

            continue;
        }

        // Regular character - add to result or link text
        if in_anchor {
            link_text.push(chars[i]);
        } else {
            result.push(chars[i]);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_basic_tags() {
        let html = "<div><p>This is a <b>test</b> paragraph.</p></div>";
        let expected = "This is a test paragraph.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_links() {
        let html =
            "<p>Visit our <a href=\"https://example.com\">website</a> for more information.</p>";
        let expected = "Visit our website [https://example.com] for more information.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_links_with_single_quotes() {
        let html =
            "<p>Check out <a href='https://rust-lang.org'>Rust</a> programming language.</p>";
        let expected = "Check out Rust [https://rust-lang.org] programming language.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_nested_tags() {
        let html = "<div><h1>Title</h1><div><p>Nested <span>content</span> here.</p></div></div>";
        let expected = "Title\nNested content here.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_script_and_style() {
        let html = "<html><body><p>Text</p><script>alert('hidden');</script><p>More text</p><style>.hidden{}</style></body></html>";
        let expected = "Text\nMore text";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_entities() {
        let html = "This has &lt;brackets&gt; and &quot;quotes&quot; and &amp; character.";
        let expected = "This has <brackets> and \"quotes\" and & character.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_line_breaks() {
        let html = "<p>Line 1</p><p>Line 2</p><br>Line 3<hr>Line 4";
        let expected = "Line 1\nLine 2\nLine 3\n----------\nLine 4";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_whitespace_normalization() {
        let html = "Too    many    spaces and \n\n\n newlines.";
        let expected = "Too many spaces and \nnewlines.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_tables() {
        let html = "<table><tr><td>Cell 1</td><td>Cell 2</td></tr><tr><td>Cell 3</td><td>Cell 4</td></tr></table>";
        let expected = "Cell 1\nCell 2\nCell 3\nCell 4";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_lists() {
        let html = "<ul><li>Item 1</li><li>Item 2</li><li>Item 3</li></ul>";
        let expected = "Item 1\nItem 2\nItem 3";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_empty_content() {
        let html = "";
        let expected = "";
        assert_eq!(strip_html_and_js(html), expected);

        let html = "<div></div>";
        let expected = "";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_malformed_tags() {
        let html = "This has <unclosed tag and <b>this is bold</b>.";
        let expected = "This has this is bold.";
        assert_eq!(strip_html_and_js(html), expected);
    }

    #[test]
    fn test_strip_html_complex_document() {
        // Using a string that's assembled piece by piece to avoid any parsing issues
        let html = String::from("<!DOCTYPE html>")
            + "<html>"
            + "<head>"
            + "<title>Test Document</title>"
            + "<style>body { font-family: Arial; }</style>"
            + "<script>console.log('This should be removed');</script>"
            + "</head>"
            + "<body>"
            + "<header>"
            + "<h1>Main Title</h1>"
            + "<nav>Navigation: <a href='/link1'>Link 1</a> | <a href='/link2'>Link 2</a></nav>"
            + "</header>"
            + "<main>"
            + "<section>"
            + "<h2>Section Title</h2>"
            + "<p>This is <em>important</em> content with some <strong>bold text</strong>.</p>"
            + "<ul>"
            + "<li>List item 1</li>"
            + "<li>List item 2</li>"
            + "</ul>"
            + "</section>"
            + "<hr>"
            + "<section>"
            + "<table>"
            + "<tr><th>Header 1</th><th>Header 2</th></tr>"
            + "<tr><td>Data 1</td><td>Data 2</td></tr>"
            + "</table>"
            + "</section>"
            + "</main>"
            + "<footer>&copy; 2023 Test Company</footer>"
            + "</body>"
            + "</html>";

        let expected = "Test DocumentMain Title\nNavigation: Link 1 [/link1] | Link 2 [/link2]Section Title\nThis is important content with some bold text.\nList item 1\nList item 2\n----------\nHeader 1Header 2\nData 1\nData 2\n& 2023 Test Company";
        assert_eq!(strip_html_and_js(&html), expected);
    }
}

/// Extract URL from arguments
fn parse_fetch_args(args: &str) -> String {
    // Just take the first non-empty argument as the URL
    args.split_whitespace().next().unwrap_or("").to_string()
}

pub async fn execute_fetch(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Parse arguments - just get the URL
    let url = parse_fetch_args(args);

    // Check if URL is provided and valid
    if url.is_empty() {
        let error_msg = "Error: No URL provided. Usage: fetch https://example.com".to_string();

        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    // Make the request using reqwest
    let client = reqwest::Client::new();
    let response = match client.get(&url).send().await {
        Ok(response) => response,
        Err(err) => {
            if !silent_mode {
                bprintln !(error:"Error fetching URL: {}", err);
            }

            return ToolResult::error(format!("Error fetching URL: {}", err));
        }
    };

    // Check status code
    if !response.status().is_success() {
        if !silent_mode {
            bprintln !(error:"Error fetching URL: HTTP status {}", response.status());
        }

        return ToolResult::error(format!(
            "Error fetching URL: HTTP status {}",
            response.status()
        ));
    }

    // Try to get content type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/html")
        .to_string();

    // Get text content
    let text = match response.text().await {
        Ok(text) => text,
        Err(err) => {
            if !silent_mode {
                bprintln !(error:"Error reading response: {}", err);
            }

            return ToolResult::error(format!("Error reading response: {}", err));
        }
    };

    // Process text based on content type
    let processed_text = if content_type.contains("text/html") || content_type.contains("html") {
        strip_html_and_js(&text)
    } else {
        // For plain text, JSON, or other formats, use as-is
        text
    };

    // Truncate large responses for user output - show first 1000 and last 1000 characters
    let user_text = processed_text.clone();

    // Return the fetched content
    if !silent_mode {
        bprintln !(tool: "fetch",
            "{}🌐 Fetch:{} {} - Content fetched successfully",
            FORMAT_BOLD,
            FORMAT_RESET,
            url
        );
        bprintln !(debug: "{}{}{}", FORMAT_GRAY, user_text, FORMAT_RESET);
    }

    ToolResult::success(format!("Fetched from {}:\n\n{}", url, processed_text))
}
