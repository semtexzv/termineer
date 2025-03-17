//! Temporary output window component

/// Temporary output window that overlays the input area and can grow upward
pub struct TemporaryOutput {
    /// Title of the output window
    pub title: String,
    /// Content lines of the output
    pub content: Vec<String>,
    /// Whether the output is visible
    pub visible: bool,
}

impl TemporaryOutput {
    /// Create a new temporary output
    pub fn new() -> Self {
        Self {
            title: String::new(),
            content: Vec::new(),
            visible: false,
        }
    }

    /// Count the number of lines needed to display content
    pub fn count_lines(&self, width: u16) -> usize {
        self.content
            .iter()
            .map(|line| {
                // Calculate how many display lines this content line will take
                // with wrapping at the specified width
                let chars = line.chars().count();
                if chars == 0 {
                    1 // Empty line still takes one line
                } else {
                    // Number of full lines plus one for any partial line
                    (chars / width as usize) + if chars % width as usize > 0 { 1 } else { 0 }
                }
            })
            .sum()
    }

    /// Hide the output
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Show output with new content
    pub fn show(&mut self, title: String, content: String) {
        self.title = title;
        self.content = content.lines().map(String::from).collect();
        self.visible = true;
    }
}
