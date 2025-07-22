use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;
 // Already present, but good to ensure
use scraper::{Html, Selector}; // Import scraper types
 // For timing if needed later

/// Extracts text content from HTML using the scraper library.
/// It attempts to preserve some structure by adding newlines around block elements.
fn extract_text_with_scraper(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut result = String::new();
    let body_selector = Selector::parse("body").unwrap(); // Start from body to ignore head content

    // Select the body element, or use the root if body is not found
    let root_element = document.select(&body_selector).next().unwrap_or(document.root_element());

    // Recursive function to traverse the DOM and extract text
    fn process_node(element: scraper::ElementRef, result: &mut String) {
        for node in element.children() {
            match node.value() {
                scraper::Node::Text(text_node) => {
                    let trimmed_text = text_node.text.trim(); // Trim the individual text node
                    if !trimmed_text.is_empty() {
                        // Ensure a single space separates from previous non-whitespace content,
                        // unless the result already ends with whitespace/newline.
                        if !result.is_empty() && !result.ends_with(|c: char| c.is_whitespace() || c == '\n') {
                            result.push(' ');
                        }
                        result.push_str(trimmed_text);
                    }
                }
                scraper::Node::Element(el) => {
                    let tag_name = el.name().to_lowercase();
                    // Skip script and style tags entirely
                    if tag_name == "script" || tag_name == "style" {
                        continue;
                    }

                    // Add newlines before/after block-level elements for structure
                    let is_block = matches!(tag_name.as_str(),
                        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" |
                        "ul" | "ol" | "li" | "table" | "tr" | "td" | "th" | "blockquote" | "pre" | "article" | "section" | "header" | "footer" | "nav" | "aside" | "figure" | "figcaption" | "address"
                    );

                    if is_block && !result.is_empty() && !result.ends_with('\n') {
                         // Add a newline before starting a new block element if needed
                         result.push('\n');
                    }

                    // Special handling for <a> tags to preserve links
                    if tag_name == "a" {
                        let href = el.attr("href").unwrap_or("").trim();
                        let mut link_text = String::new();
                        // Recursively process children to get the link text
                        process_node(scraper::ElementRef::wrap(node).unwrap(), &mut link_text);
                        let link_text = link_text.trim(); // Trim whitespace from extracted link text

                        if !link_text.is_empty() && !href.is_empty() {
                            // Add space if needed before the link
                            if !result.is_empty() && !result.ends_with(|c: char| c.is_whitespace() || c == '\n') {
                                result.push(' ');
                            }
                            result.push_str(&format!("{link_text} [{href}]"));
                            // REMOVED: result.push(' '); // Let subsequent processing handle spacing/newlines
                        } else if !link_text.is_empty() {
                            // If no href, just add the text
                             if !result.is_empty() && !result.ends_with(|c: char| c.is_whitespace() || c == '\n') {
                                result.push(' ');
                            }
                            result.push_str(link_text);
                            result.push(' ');
                        }
                        // Skip default processing for children of <a> as we handled it
                        continue; // Skip the generic recursive call below for <a>
                    }

                    // Recursively process children for non-<a> tags
                    process_node(scraper::ElementRef::wrap(node).unwrap(), result);

                    if is_block && !result.is_empty() && !result.ends_with('\n') {
                         // Add a newline after finishing a block element if needed
                         result.push('\n');
                    } else if tag_name == "br" {
                        // Handle <br> tags explicitly
                        result.push('\n');
                    }
                }
                _ => {} // Ignore comments, etc.
            }
        }
    }

    process_node(root_element, &mut result);

    // Final cleanup: Consolidate multiple newlines into single newlines
    let mut cleaned = String::new();
    let _last_char_was_newline = true; // Start as true to prevent leading newline

    for line in result.lines() {
        let processed_line: String = line.split_whitespace().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        if !processed_line.is_empty() {
            if !cleaned.is_empty() {
                cleaned.push('\n');
            }
            cleaned.push_str(&processed_line);
        }
    }
    // Ensure no trailing newline if the original didn't end with significant content
    if cleaned.ends_with('\n') && result.trim_end().ends_with(|c: char| !c.is_whitespace()) {
       cleaned.pop();
    }

    cleaned
}

#[cfg(test)]
mod scraper_tests {
    use super::*;

    #[test]
    fn test_extract_basic_text() {
        let html = "<html><body><p>Just some text.</p><div>More text here.</div></body></html>";
        let expected = "Just some text.\nMore text here.";
        assert_eq!(extract_text_with_scraper(html), expected);
    }

    #[test]
    fn test_extract_with_link() {
        let html = "<p>Visit <a href=\"https://example.com\">Example Site</a>.</p>";
        let expected = "Visit Example Site [https://example.com] ."; // Note the space added after the link
        assert_eq!(extract_text_with_scraper(html), expected.trim()); // Trim final space for comparison
    }

     #[test]
    fn test_extract_link_with_nested_tags() {
        let html = "<p>Check <a href=\"/page\"><b>bold</b> link</a>.</p>";
        let expected = "Check bold link [/page] .";
        assert_eq!(extract_text_with_scraper(html), expected.trim());
    }

    #[test]
    fn test_extract_multiple_links() {
        let html = "<p><a href=\"/1\">Link 1</a> and <a href=\"/2\">Link 2</a></p>";
        let expected = "Link 1 [/1] and Link 2 [/2]";
        assert_eq!(extract_text_with_scraper(html), expected);
    }

     #[test]
    fn test_extract_link_no_href() {
        let html = "<p>This is <a>just text</a>.</p>";
        let expected = "This is just text ."; // Link text is preserved, space added
        assert_eq!(extract_text_with_scraper(html), expected.trim());
    }

     #[test]
    fn test_extract_link_no_text() {
        let html = "<p>Link: <a href=\"/empty\"></a></p>";
        let expected = "Link:"; // No link text, so nothing added for the anchor
        assert_eq!(extract_text_with_scraper(html), expected);
    }


    #[test]
    fn test_ignore_script_and_style() {
        let html = "<style>body { color: red; }</style><p>Visible text</p><script>alert('invisible');</script>";
        let expected = "Visible text";
        assert_eq!(extract_text_with_scraper(html), expected);
    }

    #[test]
    fn test_newline_handling_for_blocks() {
        let html = "<h1>Title</h1><p>Paragraph 1.</p><div>Div content.</div><p>Paragraph 2.</p>";
        let expected = "Title\nParagraph 1.\nDiv content.\nParagraph 2.";
        assert_eq!(extract_text_with_scraper(html), expected);
    }

     #[test]
    fn test_br_tag_handling() {
        let html = "<p>Line one<br>Line two</p>";
        let expected = "Line one\nLine two";
         assert_eq!(extract_text_with_scraper(html), expected);
    }

    #[test]
    fn test_whitespace_trimming() {
        let html = "<p>  Lots  of   spaces  </p>";
        let expected = "Lots of spaces"; // Inner spaces are collapsed by trim() on text nodes
        assert_eq!(extract_text_with_scraper(html), expected);
    }

     #[test]
    fn test_complex_structure() {
        let html = r#"
            <body>
                <h1>Main Heading</h1>
                <p>
                    This is a paragraph with a <a href="http://test.com">link</a>.
                    It also has <strong>strong</strong> text.
                </p>
                <div><script>var x=1;</script>Another block.</div>
                <ul><li>Item 1</li><li>Item 2 with <a href="/item2">nested link</a></li></ul>
            </body>
        "#;
        let expected = "Main Heading\nThis is a paragraph with a link [http://test.com] .\nIt also has strong text.\nAnother block.\nItem 1\nItem 2 with nested link [/item2]";
        assert_eq!(extract_text_with_scraper(html), expected);
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
            bprintln !(error:"{error_msg}");
        }

        return ToolResult::error(error_msg);
    }

    // Make the request using reqwest
    let client = reqwest::Client::new();
    let response = match client.get(&url).send().await {
        Ok(response) => response,
        Err(err) => {
            if !silent_mode {
                bprintln !(error:"Error fetching URL: {err}");
            }

            return ToolResult::error(format!("Error fetching URL: {err}"));
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
                bprintln !(error:"Error reading response: {err}");
            }

            return ToolResult::error(format!("Error reading response: {err}"));
        }
    };

    // Process text based on content type
    let processed_text = if content_type.contains("text/html") || content_type.contains("html") {
        // Use the new scraper-based function
        extract_text_with_scraper(&text)
    } else {
        // For plain text, JSON, or other formats, use as-is
        text
    };

    // Truncate large responses for user output - show first 1000 and last 1000 characters
    let user_text = processed_text.clone();

    // Return the fetched content
    if !silent_mode {
        bprintln !(tool: "fetch",
            "{FORMAT_BOLD}🌐 Fetch:{FORMAT_RESET} {url} - Content fetched successfully"
        );
        bprintln !(dev: "{FORMAT_GRAY}{user_text}{FORMAT_RESET}");
    }

    ToolResult::success(format!("Fetched from {url}:\n\n{processed_text}"))
}
