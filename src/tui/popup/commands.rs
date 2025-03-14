//! Command suggestions popup for command autocompletion

/// Command suggestion entry
#[derive(Clone, Debug)]
pub struct CommandSuggestion {
    /// Command name (including the slash)
    pub name: String,
    /// Command description
    pub description: String,
}

/// Command suggestions popup for auto-completion
pub struct CommandSuggestionsPopup {
    /// List of all available commands
    pub all_commands: Vec<CommandSuggestion>,
    /// Filtered commands matching the current input
    pub filtered_commands: Vec<CommandSuggestion>,
    /// Currently selected command index
    pub selected_index: usize,
    /// Whether the popup is visible
    pub visible: bool,
}

impl CommandSuggestionsPopup {
    /// Create a new command suggestions popup
    pub fn new() -> Self {
        // Initialize with all available commands
        let all_commands = vec![
            CommandSuggestion {
                name: "/help".to_string(),
                description: "Show available commands".to_string(),
            },
            CommandSuggestion {
                name: "/exit".to_string(),
                description: "Exit the application".to_string(),
            },
            CommandSuggestion {
                name: "/quit".to_string(),
                description: "Exit the application".to_string(),
            },
            CommandSuggestion {
                name: "/interrupt".to_string(),
                description: "Interrupt the current agent".to_string(),
            },
            CommandSuggestion {
                name: "/model".to_string(),
                description: "Set the model for the current agent".to_string(),
            },
            CommandSuggestion {
                name: "/tools".to_string(),
                description: "Enable or disable tools".to_string(),
            },
            CommandSuggestion {
                name: "/system".to_string(),
                description: "Set the system prompt".to_string(),
            },
            CommandSuggestion {
                name: "/reset".to_string(),
                description: "Reset the conversation".to_string(),
            },
            CommandSuggestion {
                name: "/thinking".to_string(),
                description: "Set the thinking budget in tokens".to_string(),
            },
        ];

        Self {
            all_commands: all_commands.clone(),
            filtered_commands: all_commands,
            selected_index: 0,
            visible: false,
        }
    }

    /// Show the suggestions popup and filter based on current input
    pub fn show(&mut self, current_input: &str) {
        self.visible = true;
        self.update_suggestions(current_input);
    }

    /// Hide the suggestions popup
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Update filtered suggestions based on current input
    pub fn update_suggestions(&mut self, current_input: &str) {
        // Skip the leading slash for matching
        let search_text = current_input.trim_start_matches('/');

        // If empty, show all commands
        if search_text.is_empty() {
            self.filtered_commands = self.all_commands.clone();
            self.selected_index = 0;
            return;
        }

        // Filter commands that match the input prefix
        self.filtered_commands = self
            .all_commands
            .iter()
            .filter(|cmd| cmd.name.trim_start_matches('/').starts_with(search_text))
            .cloned()
            .collect();

        // Reset selection index
        self.selected_index = 0;
    }

    /// Select the next suggestion
    pub fn next(&mut self) {
        if !self.filtered_commands.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_commands.len();
        }
    }

    /// Get the currently selected command if any
    pub fn selected_command(&self) -> Option<&CommandSuggestion> {
        if self.filtered_commands.is_empty() {
            None
        } else {
            self.filtered_commands.get(self.selected_index)
        }
    }
}