#![allow(non_snake_case)]

use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;
use lazy_static::lazy_static;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::time::Instant;
use scraper::{Html, Selector};

// Google Search API response structures
// Uses non-snake-case to match Google API response format
#[derive(Debug, Deserialize)]
struct GoogleSearchResponse {
    items: Option<Vec<GoogleSearchItem>>,
    #[allow(dead_code)]
    searchInformation: Option<SearchInformation>,
}

#[derive(Debug, Deserialize)]
struct GoogleSearchItem {
    title: String,
    link: String,
    snippet: Option<String>,
    #[allow(dead_code)]
    displayLink: Option<String>,
    #[allow(dead_code)]
    formattedUrl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchInformation {
    #[allow(dead_code)]
    searchTime: Option<f64>,
    #[allow(dead_code)]
    formattedSearchTime: Option<String>,
    #[allow(dead_code)]
    totalResults: Option<String>,
    #[allow(dead_code)]
    formattedTotalResults: Option<String>,
}

/// Execute DuckDuckGo search by scraping their HTML search results
async fn execute_duckduckgo_search(query: &str, silent_mode: bool) -> ToolResult {
    let start_time = Instant::now();
    
    if !silent_mode {
        bprintln!(
            "{}🔍 Search:{} Searching for \"{}\" via DuckDuckGo...",
            FORMAT_BOLD,
            FORMAT_RESET,
            query
        );
    }
    
    // URL encode the query
    let encoded_query = urlencoding::encode(query);
    let url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);
    
    // Send the request
    let client = Client::new();
    let response = match client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .send()
        .await {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status();
                    let error_msg = format!("Error: DuckDuckGo search returned status code {}", status);
                    
                    if !silent_mode {
                        eprintln!("{}", error_msg);
                    }
                    
                    return ToolResult::error(error_msg);
                }
                response
            },
            Err(err) => {
                let error_msg = format!("Error connecting to DuckDuckGo: {}", err);
                
                if !silent_mode {
                    eprintln!("{}", error_msg);
                }
                
                return ToolResult::error(error_msg);
            }
        };
    
    // Get the HTML response
    let html = match response.text().await {
        Ok(text) => text,
        Err(err) => {
            let error_msg = format!("Error reading DuckDuckGo response: {}", err);
            
            if !silent_mode {
                eprintln!("{}", error_msg);
            }
            
            return ToolResult::error(error_msg);
        }
    };
    
    // Parse the HTML
    let document = Html::parse_document(&html);
    
    // Define selectors for the search results
    let results_selector = match Selector::parse(".result") {
        Ok(selector) => selector,
        Err(err) => {
            let error_msg = format!("Error creating DuckDuckGo results selector: {}", err);
            
            if !silent_mode {
                eprintln!("{}", error_msg);
            }
            
            return ToolResult::error(error_msg);
        }
    };
    
    let title_selector = match Selector::parse(".result__title a") {
        Ok(selector) => selector,
        Err(err) => {
            let error_msg = format!("Error creating DuckDuckGo title selector: {}", err);
            
            if !silent_mode {
                eprintln!("{}", error_msg);
            }
            
            return ToolResult::error(error_msg);
        }
    };
    
    let snippet_selector = match Selector::parse(".result__snippet") {
        Ok(selector) => selector,
        Err(err) => {
            let error_msg = format!("Error creating DuckDuckGo snippet selector: {}", err);
            
            if !silent_mode {
                eprintln!("{}", error_msg);
            }
            
            return ToolResult::error(error_msg);
        }
    };
    
    // Format the results for output
    let mut formatted_results = format!("Search results for \"{}\" (via DuckDuckGo):\n\n", query);
    
    // Extract search results
    let mut result_count = 0;
    
    for (i, result) in document.select(&results_selector).enumerate() {
        if i >= 10 {  // Limit to top 10 results
            break;
        }
        
        result_count += 1;
        
        // Extract title
        let title = match result.select(&title_selector).next() {
            Some(el) => {
                let mut title = String::new();
                for text in el.text() {
                    title.push_str(text);
                }
                title.trim().to_string()
            },
            None => "No title".to_string()
        };
        
        // Extract URL
        let url = match result.select(&title_selector).next() {
            Some(el) => el.value().attr("href").unwrap_or("No URL"),
            None => "No URL"
        };
        
        // Clean up the URL (DuckDuckGo uses redirects)
        let url = if url.contains("/l/?uddg=") {
            // Extract the actual URL from the redirect
            let parts: Vec<&str> = url.split("uddg=").collect();
            if parts.len() > 1 {
                let encoded_url = parts[1].split('&').next().unwrap_or(parts[1]);
                match urlencoding::decode(encoded_url) {
                    Ok(decoded) => decoded.to_string(),
                    Err(_) => encoded_url.to_string()
                }
            } else {
                url.to_string()
            }
        } else if url.starts_with("/") {
            format!("https://duckduckgo.com{}", url)
        } else {
            url.to_string()
        };
        
        // Extract snippet
        let snippet = match result.select(&snippet_selector).next() {
            Some(el) => {
                let mut snippet = String::new();
                for text in el.text() {
                    snippet.push_str(text);
                }
                snippet.trim().to_string()
            },
            None => "No description".to_string()
        };
        
        // Add formatted result to output
        formatted_results.push_str(&format!(
            "{}{}. {}{}\n",
            FORMAT_GRAY,
            i + 1,
            title,
            FORMAT_RESET
        ));
        formatted_results.push_str(&format!(
            "{}   URL: {}{}\n",
            FORMAT_GRAY, url, FORMAT_RESET
        ));
        formatted_results.push_str(&format!(
            "{}   {}{}\n\n",
            FORMAT_GRAY, snippet, FORMAT_RESET
        ));
    }
    
    if result_count == 0 {
        formatted_results.push_str("No results found.\n");
    }
    
    let elapsed = start_time.elapsed();
    
    if !silent_mode {
        bprintln!(tool: "search",
            "Found {} results for \"{}\" via DuckDuckGo (in {:.2}ms)",
            result_count,
            query,
            elapsed.as_millis()
        );
        bprintln!("{}", formatted_results);
    }
    
    ToolResult::success(formatted_results)
}

/// Execute the search tool using Google Custom Search API
/// Falls back to DuckDuckGo search if Google API key is not available
pub async fn execute_search(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Get the Google API key from environment
    let api_key = match env::var("GOOGLE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            if !silent_mode {
                bprintln!(info: "GOOGLE_API_KEY not found, falling back to DuckDuckGo search");
            }
            
            // Fall back to DuckDuckGo search
            return execute_duckduckgo_search(args.trim(), silent_mode).await;
        }
    };

    // Set a default search engine ID - using a programmable search engine for general web search
    // This is the publicly available search engine ID that works with Google API keys
    lazy_static! {
        static ref SEARCH_ENGINE_ID: String = obfstr::obfstring!("77f98042a073d4c0e").to_string();
    }

    // Check if a search query is provided
    let query = args.trim();
    if query.is_empty() {
        let error_msg = "Error: No search query provided. Usage: search <query>".to_string();

        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    if !silent_mode {
        bprintln !(tool: "search",
            "{}🔍 Search:{} Searching for \"{}\"...",
            FORMAT_BOLD,
            FORMAT_RESET,
            query
        );
    }

    // Create the Google Custom Search API URL
    let encoded_query = urlencoding::encode(query);
    let url = format!(
        "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}",
        api_key, &*SEARCH_ENGINE_ID, encoded_query
    );

    // Execute the search request
    let client = Client::new();
    let response = match client.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let error_msg = format!("Error: Google Search API returned status code {}", status);

                if !silent_mode {
                    bprintln !(error:"{}", error_msg);
                }

                return ToolResult::error(error_msg);
            }
            response
        }
        Err(err) => {
            let error_msg = format!("Error connecting to Google Search API: {}", err);

            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    // Parse the response JSON
    let search_results: GoogleSearchResponse = match response.json().await {
        Ok(json) => json,
        Err(err) => {
            let error_msg = format!("Error parsing Google Search API response: {}", err);

            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    // Format the results for output
    let mut formatted_results = format!("Search results for \"{}\":\n\n", query);

    // Process the main search results
    if let Some(items) = &search_results.items {
        if items.is_empty() {
            formatted_results.push_str("No results found.\n");
        } else {
            for (i, item) in items.iter().enumerate() {
                formatted_results.push_str(&format!(
                    "{}{}. {}{}\n",
                    FORMAT_GRAY,
                    i + 1,
                    item.title,
                    FORMAT_RESET
                ));
                formatted_results.push_str(&format!(
                    "{}   URL: {}{}\n",
                    FORMAT_GRAY, item.link, FORMAT_RESET
                ));

                if let Some(snippet) = &item.snippet {
                    formatted_results
                        .push_str(&format!("{}   {}{}\n", FORMAT_GRAY, snippet, FORMAT_RESET));
                }

                formatted_results.push_str("\n");
            }
        }
    } else {
        formatted_results.push_str("No results found.\n");
    }

    if !silent_mode {
        let result_count = search_results.items.as_ref().map_or(0, |items| items.len());
        bprintln !(tool: "search",
            "{}🔍 Search:{} Found {} results for \"{}\"",
            FORMAT_BOLD,
            FORMAT_RESET,
            result_count,
            query
        );
        bprintln!("{}{}{}", FORMAT_GRAY, formatted_results, FORMAT_RESET);
    }

    ToolResult::success(formatted_results)
}
