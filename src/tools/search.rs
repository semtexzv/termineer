use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::time::Instant;

// Google Search API response structures
#[derive(Debug, Deserialize)]
struct GoogleSearchResponse {
    items: Option<Vec<GoogleSearchItem>>,
    searchInformation: Option<SearchInformation>,
}

#[derive(Debug, Deserialize)]
struct GoogleSearchItem {
    title: String,
    link: String,
    snippet: Option<String>,
    displayLink: Option<String>,
    formattedUrl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchInformation {
    searchTime: Option<f64>,
    formattedSearchTime: Option<String>,
    totalResults: Option<String>,
    formattedTotalResults: Option<String>,
}

/// Execute the search tool using Google Custom Search API
pub async fn execute_search(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Get the Google API key from environment
    let api_key = match env::var("GOOGLE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            let error_msg = "Error: GOOGLE_API_KEY environment variable not set. Please add your Google API key to the .env file.".to_string();
            
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
            }
            
            return ToolResult::error(error_msg);
        }
    };
    
    // Set a default search engine ID - using a programmable search engine for general web search
    // This is the publicly available search engine ID that works with Google API keys
    const SEARCH_ENGINE_ID: &str = "77f98042a073d4c0e";
    
    // Check if a search query is provided
    let query = args.trim();
    if query.is_empty() {
        let error_msg = "Error: No search query provided. Usage: search <query>".to_string();
        
        if !silent_mode {
            crate::berror_println!("{}", error_msg);
        }
        
        return ToolResult::error(error_msg);
    }
    
    // Start timing the search
    let start_time = Instant::now();
    
    if !silent_mode {
        crate::btool_println!(
            "search",
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
        api_key, SEARCH_ENGINE_ID, encoded_query
    );
    
    // Execute the search request
    let client = Client::new();
    let response = match client.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let error_msg = format!("Error: Google Search API returned status code {}", status);
                
                if !silent_mode {
                    crate::berror_println!("{}", error_msg);
                }
                
                return ToolResult::error(error_msg);
            }
            response
        },
        Err(err) => {
            let error_msg = format!("Error connecting to Google Search API: {}", err);
            
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
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
                crate::berror_println!("{}", error_msg);
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
                formatted_results.push_str(&format!("{}. {}\n", i + 1, item.title));
                formatted_results.push_str(&format!("   URL: {}\n", item.link));
                
                if let Some(snippet) = &item.snippet {
                    formatted_results.push_str(&format!("   {}\n", snippet));
                }
                
                formatted_results.push_str("\n");
            }
        }
    } else {
        formatted_results.push_str("No results found.\n");
    }
    
    // Add search information
    if let Some(search_info) = &search_results.searchInformation {
        if let Some(total_results) = &search_info.formattedTotalResults {
            formatted_results.push_str(&format!("Total results: {}\n", total_results));
        }
        
        if let Some(search_time) = &search_info.searchTime {
            formatted_results.push_str(&format!("Search engine time: {:.2} seconds\n", search_time));
        }
    }
    
    // Calculate our search time
    let search_time = start_time.elapsed();
    formatted_results.push_str(&format!("Total search time: {:.2?}", search_time));
    
    if !silent_mode {
        let result_count = search_results.items.as_ref().map_or(0, |items| items.len());
        crate::btool_println!(
            "search",
            "{}🔍 Search:{} Found {} results for \"{}\"",
            FORMAT_BOLD,
            FORMAT_RESET,
            result_count,
            query
        );
        crate::bprintln!("{}{}{}", FORMAT_GRAY, formatted_results, FORMAT_RESET);
    }
    
    ToolResult::success(formatted_results)
}