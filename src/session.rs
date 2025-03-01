use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::llm::Message;
use crate::agent::Agent;

// Session structure for saving/loading conversations
#[derive(Serialize, Deserialize, Clone)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub timestamp: u64,
    pub model: String,
    pub system_prompt: Option<String>,
    pub conversation: Vec<Message>,
    pub metadata: SessionMetadata,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SessionMetadata {
    pub created_at: u64,
    pub last_updated: u64,
    pub message_count: usize,
    pub token_count: Option<usize>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl Session {
    pub fn new(name: String, client: &Agent) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let id = format!("session_{}", timestamp);
        
        Session {
            id,
            name,
            timestamp,
            model: client.config.model.clone(),
            system_prompt: client.config.system_prompt.clone(),
            conversation: client.conversation.clone(),
            metadata: SessionMetadata {
                created_at: timestamp,
                last_updated: timestamp,
                message_count: client.conversation.len(),
                token_count: None, // Token counts are now per-response rather than global
                description: None,
                tags: Vec::new(),
            },
        }
    }
    
    pub fn to_file(&self, filepath: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create directories if they don't exist
        if let Some(parent) = filepath.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Serialize the session
        let serialized = serde_json::to_string_pretty(self)?;
        
        // Write to file
        fs::write(filepath, serialized)?;
        
        Ok(())
    }
    
    pub fn from_file(filepath: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(filepath)?;
        let session: Session = serde_json::from_str(&content)?;
        Ok(session)
    }
}

// Session directory and file management functions
pub fn get_base_session_dir() -> std::path::PathBuf {
    // Get the config directory for the user's platform
    match dirs::config_dir() {
        Some(config_dir) => config_dir.join("autoswe").join("sessions"),
        None => {
            // Fallback to a local directory if we can't get a config directory
            std::path::PathBuf::from(".autoswe_sessions")
        }
    }
}

pub fn get_session_dir() -> std::io::Result<std::path::PathBuf> {
    // Get the current working directory
    let current_dir = std::env::current_dir()?;
    
    // Convert to a relative path structure (replacing / with _)
    // This ensures we don't create invalid paths
    let dir_string = current_dir
        .to_string_lossy()
        .replace('/', "_")
        .replace('\\', "_");
    
    let path = get_base_session_dir().join(dir_string);
    
    // Ensure directory exists
    fs::create_dir_all(&path)?;
    
    Ok(path)
}

pub fn get_last_session_file() -> std::io::Result<std::path::PathBuf> {
    Ok(get_session_dir()?.join(".last"))
}

// Agent functions for session management
pub fn save_session(client: &Agent, name: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Get session directory for the current working directory
    let session_dir = get_session_dir()?;
    
    // Create a session object
    let session = Session::new(name.to_string(), client);
    
    // Generate filepath
    let filename = format!("{}.json", session.id);
    let filepath = session_dir.join(filename);
    
    // Save session to file
    session.to_file(&filepath)?;
    
    // Save as last session
    save_last_session(client, &session.id)?;
    
    Ok(session.id)
}

pub fn load_session(client: &mut Agent, session_id_or_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Get the session directory
    let session_dir = get_session_dir()?;
    
    // Check if it's a full path 
    if session_id_or_name.ends_with(".json") && Path::new(session_id_or_name).exists() {
        let session = Session::from_file(Path::new(session_id_or_name))?;
        
        // Update client state
        client.config.model = session.model.clone();
        client.config.system_prompt = session.system_prompt.clone();
        client.conversation = session.conversation.clone();
        client.reset_cache_points(); // Reset cache points when loading a session
        
        // Recreate the backend with the loaded model
        client.llm = crate::llm::create_backend(&client.config)
            .expect("Failed to create LLM backend");
        
        // Save as last session
        save_last_session(client, &session.id)?;
        
        return Ok(());
    }
    
    // Check if it's just a session ID
    let session_path = session_dir.join(format!("{}.json", session_id_or_name));
    if session_path.exists() {
        let session = Session::from_file(&session_path)?;
        
        // Update client state
        client.config.model = session.model.clone();
        client.config.system_prompt = session.system_prompt.clone();
        client.conversation = session.conversation.clone();
        client.reset_cache_points(); // Reset cache points when loading a session
        
        // Recreate the backend with the loaded model
        client.llm = crate::llm::create_backend(&client.config)
            .expect("Failed to create LLM backend");
        
        // Save as last session
        save_last_session(client, &session.id)?;
        
        return Ok(());
    }
    
    // Not found by ID, try to find by name
    let sessions = list_sessions(client)?;
    
    // Find sessions with matching names (case-insensitive)
    let matching_sessions: Vec<&Session> = sessions.iter()
        .filter(|s| s.name.to_lowercase() == session_id_or_name.to_lowercase())
        .collect();
    
    if matching_sessions.is_empty() {
        return Err(format!("No session found with ID or name '{}'", session_id_or_name).into());
    }
    
    // If multiple sessions match the name, use the most recent one
    let session = matching_sessions.iter()
        .max_by_key(|s| s.timestamp)
        .unwrap();
    
    // Update client state
    client.config.model = session.model.clone();
    client.config.system_prompt = session.system_prompt.clone();
    client.conversation = session.conversation.clone();
    client.reset_cache_points(); // Reset cache points when loading a session
    
    // Recreate the backend with the loaded model
    client.llm = crate::llm::create_backend(&client.config)
        .expect("Failed to create LLM backend");
    
    // Save as last session
    save_last_session(client, &session.id)?;
    
    Ok(())
}

pub fn list_sessions(_client: &Agent) -> Result<Vec<Session>, Box<dyn std::error::Error>> {
    // Get the session directory
    let session_dir = get_session_dir()?;
    
    let mut sessions = Vec::new();
    
    // Iterate through all files in the session directory
    for entry in fs::read_dir(session_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        // Only process JSON files, skip the .last file
        if path.is_file() && 
           path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            if let Ok(session) = Session::from_file(&path) {
                sessions.push(session);
            }
        }
    }
    
    // Sort by timestamp (newest first)
    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    Ok(sessions)
}

pub fn list_all_sessions(_client: &Agent) -> Result<Vec<(String, Vec<Session>)>, Box<dyn std::error::Error>> {
    // Get the base session directory
    let base_dir = get_base_session_dir();
    
    // Create if it doesn't exist
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir)?;
        return Ok(Vec::new()); // No sessions yet
    }
    
    let mut all_sessions = Vec::new();
    
    // Iterate through all subdirectories (each representing a working directory)
    for entry in fs::read_dir(&base_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let dir_name = path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            let mut sessions = Vec::new();
            
            // Read all session files in this directory
            for file_entry in fs::read_dir(&path)? {
                let file_entry = file_entry?;
                let file_path = file_entry.path();
                
                // Only process JSON files, skip the .last file
                if file_path.is_file() && 
                   file_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    if let Ok(session) = Session::from_file(&file_path) {
                        sessions.push(session);
                    }
                }
            }
            
            // Sort by timestamp (newest first)
            sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            
            // Add this directory's sessions to the result
            if !sessions.is_empty() {
                all_sessions.push((dir_name, sessions));
            }
        }
    }
    
    // Sort directories by name
    all_sessions.sort_by(|(a, _), (b, _)| a.cmp(b));
    
    Ok(all_sessions)
}

pub fn save_last_session(_client: &Agent, session_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let last_file = get_last_session_file()?;
    fs::write(last_file, session_id)?;
    Ok(())
}

// New function to delete a session
pub fn delete_session(_client: &Agent, session_id_or_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Get the session directory
    let session_dir = get_session_dir()?;
    
    // Check if it's a full path 
    if session_id_or_name.ends_with(".json") && Path::new(session_id_or_name).exists() {
        fs::remove_file(session_id_or_name)?;
        return Ok(());
    }
    
    // Check if it's just a session ID
    let session_path = session_dir.join(format!("{}.json", session_id_or_name));
    if session_path.exists() {
        fs::remove_file(&session_path)?;
        return Ok(());
    }
    
    // Not found by ID, try to find by name
    let sessions = list_sessions(_client)?;
    
    // Find sessions with matching names (case-insensitive)
    let matching_sessions: Vec<&Session> = sessions.iter()
        .filter(|s| s.name.to_lowercase() == session_id_or_name.to_lowercase())
        .collect();
    
    if matching_sessions.is_empty() {
        return Err(format!("No session found with ID or name '{}'", session_id_or_name).into());
    }
    
    // If multiple sessions match the name, use the most recent one
    let session = matching_sessions.iter()
        .max_by_key(|s| s.timestamp)
        .unwrap();
    
    // Delete the file
    let session_path = session_dir.join(format!("{}.json", session.id));
    fs::remove_file(&session_path)?;
    
    Ok(())
}

// For loading the last used session when starting
pub fn load_last_session(client: &mut Agent) -> Result<(), Box<dyn std::error::Error>> {
    let last_file = get_last_session_file()?;
    
    if !last_file.exists() {
        return Err("No previous session found".into());
    }
    
    let session_id = fs::read_to_string(last_file)?.trim().to_string();
    load_session(client, &session_id)
}