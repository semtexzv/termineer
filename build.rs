//! Build script for termineer that encrypts prompt templates
//! for protection in the compiled binary.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Extract the description from a template file
fn extract_template_description(file_path: &Path) -> Option<String> {
    // Open the file
    let file = match File::open(file_path) {
        Ok(file) => file,
        Err(_) => return None,
    };

    let reader = BufReader::new(file);

    // Look for the first line that starts with "{{!"
    for line in reader.lines() {
        if let Ok(line) = line {
            let trimmed = line.trim();
            if trimmed.starts_with("{{!") {
                // Extract the description part (after the dash)
                if let Some(dash_pos) = trimmed.find('-') {
                    // Get text between the dash and the closing comment
                    let mut description = trimmed[(dash_pos + 1)..].trim();

                    // Remove closing Handlebars comment tag if it exists
                    if let Some(end_pos) = description.find("}}") {
                        description = description[..end_pos].trim();
                    }

                    if !description.is_empty() {
                        return Some(description.to_string());
                    }
                }
            }
        }
    }

    None
}

fn main() {
    println!("cargo:rerun-if-changed=prompts");

    // Generate a random 32-byte encryption key
    let encryption_key = Aes256Gcm::generate_key(&mut OsRng);

    // Create destination directory for encrypted files
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let encrypted_dir = Path::new(&out_dir).join("encrypted_prompts");
    fs::create_dir_all(&encrypted_dir).unwrap();

    // Write the raw key bytes to a file in the output directory
    let key_path = Path::new(&out_dir).join("encryption_key.bin");
    let mut key_file = File::create(&key_path).unwrap();
    key_file.write_all(encryption_key.as_slice()).unwrap();
    println!(
        "cargo:info=Encryption key written to: {}",
        key_path.display()
    );

    // Process all files in the prompts directory
    let prompts_dir = Path::new("prompts");

    // Collect information about all encrypted files
    let mut encrypted_files = Vec::new();

    // Collect all agent kinds (templates)
    let mut kinds = HashSet::new();
    // Map to store descriptions for each template
    let mut descriptions = HashMap::new();

    for entry in WalkDir::new(prompts_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            // Get the relative path from the prompts directory
            let rel_path = entry.path().strip_prefix(prompts_dir).unwrap();

            println!(
                "cargo:rerun-if-changed=prompts/{}",
                rel_path.to_string_lossy()
            );

            // If it's a handlebars template (.hbs extension)
            if entry.path().extension().map_or(false, |ext| ext == "hbs") {
                // Get the template path without the extension (for kind identification)
                let template_path = rel_path.with_extension("");
                let kind_id = template_path.to_string_lossy().replace('\\', "/");

                // Only collect templates from the kind directory
                if kind_id.starts_with("kind/") {
                    // Extract the description if available
                    if let Some(description) = extract_template_description(entry.path()) {
                        descriptions.insert(kind_id.clone(), description);
                    }

                    kinds.insert(kind_id.to_string());
                }
            }

            // Read source file
            let mut content = Vec::new();
            File::open(entry.path())
                .unwrap()
                .read_to_end(&mut content)
                .unwrap();

            // Create cipher for encryption using our random key
            let cipher = Aes256Gcm::new(&encryption_key);

            // Generate a random nonce for each file
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

            // Encrypt content
            let encrypted = cipher
                .encrypt(&nonce, content.as_ref())
                .expect(&format!("Failed to encrypt: {}", entry.path().display()));

            // Prepare destination path
            let dest_path = encrypted_dir.join(rel_path);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            // Convert Windows backslashes to forward slashes to ensure consistency
            let normalized_path = rel_path.to_string_lossy().replace('\\', "/");

            // Write nonce + encrypted content
            let mut file = File::create(&dest_path).unwrap();
            file.write_all(nonce.as_slice()).unwrap();
            file.write_all(&encrypted).unwrap();

            // Store the relative path and the destination path for later use
            encrypted_files.push((normalized_path.to_string(), dest_path));

            println!("cargo:info=Encrypted: {}", entry.path().display());
        }
    }

    // Generate a Rust file with the encrypted prompts data and kinds list
    generate_encrypted_prompts_module(&out_dir, &encrypted_files, &kinds, &descriptions);
}

/// Generate a Rust file containing the encrypted prompts data and kinds list
fn generate_encrypted_prompts_module(
    out_dir: &Path,
    encrypted_files: &[(String, PathBuf)],
    kinds: &HashSet<String>,
    descriptions: &HashMap<String, String>,
) {
    // Create the output file
    let output_path = Path::new(out_dir).join("encrypted_prompts.rs");

    // Generate a sorted vector from the HashSet for consistent output
    let mut sorted_kinds: Vec<String> = kinds.iter().cloned().collect();
    sorted_kinds.sort();

    // Create the content for the available kinds string
    let mut kinds_content = String::new();

    // Separate agent kinds into different categories
    let mut standard_templates = Vec::new();
    let mut plus_templates = Vec::new();
    let mut other_templates = Vec::new();

    for kind in &sorted_kinds {
        if kind.starts_with("kind/plus/") {
            // Keep the 'plus/' prefix for plus templates
            plus_templates.push(kind.replace("kind/plus/", "plus/"));
        } else if kind.starts_with("kind/") {
            standard_templates.push(kind.replace("kind/", ""));
        } else {
            // Agent kinds not in the kind directory go to other_templates
            other_templates.push(kind.clone());
        }
    }

    // Sort agent kinds for consistent output
    standard_templates.sort();
    plus_templates.sort();
    other_templates.sort();

    // Find the longest agent kind name to determine proper alignment
    let longest_standard = standard_templates
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(0);
    let longest_plus = plus_templates.iter().map(|s| s.len()).max().unwrap_or(0);
    let column_width = std::cmp::max(longest_standard, longest_plus) + 4; // Add some padding

    // Add standard agent kinds with aligned descriptions
    kinds_content.push_str("Standard agent kinds:\n");
    for template in &standard_templates {
        let full_path = format!("kind/{}", template);
        let description = descriptions
            .get(&full_path)
            .map(|desc| format!("{}", desc))
            .unwrap_or_else(|| "".to_string());

        // Calculate spaces needed for alignment
        let spaces = " ".repeat(column_width - template.len());
        kinds_content.push_str(&format!("- {}{}  │  {}\n", template, spaces, description));
    }

    // Add plus agent kinds with aligned descriptions
    kinds_content.push_str("\nPlus agent kinds:\n");
    for template in &plus_templates {
        let full_path = format!("kind/plus/{}", template);
        let description = descriptions
            .get(&full_path)
            .map(|desc| format!("{}", desc))
            .unwrap_or_else(|| "".to_string());

        // Calculate spaces needed for alignment
        let spaces = " ".repeat(column_width - template.len());
        kinds_content.push_str(&format!("- {}{}  │  {}\n", template, spaces, description));
    }

    // Create a TokenStream for encrypted files array entries
    let mut encrypted_files_tokens: Vec<TokenStream> = vec![];
    for (path, dest_path) in encrypted_files {
        let path_str = path.clone();
        let include_path = dest_path.to_string_lossy().replace('\\', "/");
        let include_expr = format!("include_bytes!(r#\"{}\"#)", include_path);
        let include_tokens: TokenStream = include_expr.parse().unwrap();

        encrypted_files_tokens.push(quote! {
            (obfstr::obfstring!(#path_str), &#include_tokens[..])
        });
    }

    let mut short_kinds: Vec<TokenStream> = vec![];
    for kind in &sorted_kinds {
        if kind.starts_with("kind/") {
            let short_name = kind.replace("kind/", "");
            short_kinds.push(quote! { obfstr::obfstring!(#short_name) });
        }
    }

    // Create the literal for the kinds content
    let kinds_content_lit = Literal::string(&kinds_content);

    // Generate the final code
    let module = quote! {
        use std::sync::LazyLock;
        // This file is auto-generated by build.rs. Do not edit directly!

        pub static ENCRYPTED_PROMPTS: LazyLock<Vec<(String, &'static [u8])>> = LazyLock::new(|| {
            vec![#(#encrypted_files_tokens),*]
        });

        /// Array of all available agent kinds
        pub static AVAILABLE_KINDS_ARRAY: LazyLock<Vec<String>> = LazyLock::new(|| {
            vec![
                #(#short_kinds),*
            ]
        });

        /// Standard agent kinds (available to all users)
        pub static STANDARD_KINDS: LazyLock<Vec<String>> = LazyLock::new(|| {
            AVAILABLE_KINDS_ARRAY.iter()
                .filter(|k| !k.starts_with("plus/") && !k.starts_with("pro/"))
                .cloned()
                .collect()
        });

        /// Plus agent kinds (available to Plus and Pro users)
        pub static PLUS_KINDS: LazyLock<Vec<String>> = LazyLock::new(|| {
            AVAILABLE_KINDS_ARRAY.iter()
                .filter(|k| k.starts_with("plus/"))
                .cloned()  // Keep the plus/ prefix
                .collect()
        });

        /// Pro agent kinds (available only to Pro users)
        pub static PRO_KINDS: LazyLock<Vec<String>> = LazyLock::new(|| {
            AVAILABLE_KINDS_ARRAY.iter()
                .filter(|k| k.starts_with("pro/"))
                .cloned()  // Keep the pro/ prefix
                .collect()
        });

        /// Get the list of available agent kinds
        pub fn get_available_kinds() -> String {
            obfstr::obfstring!(#kinds_content_lit)
        }

        /// Get the list of available agent kinds for the specified app mode
        pub fn get_kinds_for_mode(mode: crate::config::AppMode) -> String {
            // This is essentially returning the same formatted string as get_available_kinds,
            // but filtering based on the app mode
            
            // The original kinds content is efficiently pre-formatted with descriptions
            let full_listing = get_available_kinds();
            
            // Split into sections by newline
            let sections: Vec<&str> = full_listing.split("\n\n").collect();
            
            let mut result = String::new();
            
            // Always include standard kinds (first section)
            if sections.len() > 0 {
                result.push_str(sections[0]);
                result.push_str("\n");
            }
            
            // Include Plus kinds for Plus and Pro modes (second section)
            if sections.len() > 1 {
                match mode {
                    crate::config::AppMode::Plus | crate::config::AppMode::Pro => {
                        result.push_str("\n");
                        result.push_str(sections[1]);
                        result.push_str("\n");
                    },
                    _ => {}
                }
            }
            
            // Include Pro kinds only for Pro mode (third section if it exists)
            if sections.len() > 2 {
                if let crate::config::AppMode::Pro = mode {
                    result.push_str("\n");
                    result.push_str(sections[2]);
                    result.push_str("\n");
                }
            }
            
            result
        }

    };

    // Write the generated code to the output file
    let mut output_file = File::create(&output_path).unwrap();
    write!(output_file, "{}", module).unwrap();

    println!(
        "cargo:warning=Generated encrypted prompts module with {} kinds at: {}",
        kinds.len(),
        output_path.display()
    );
}
