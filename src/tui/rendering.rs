//! Rendering functions for the Terminal UI components

use crate::tui::state::TuiState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Rendering functions for the TUI
pub fn render_ui(state: &TuiState, f: &mut Frame) {
    let size = f.size();
    f.render_widget(Clear, size);

    // Calculate the height needed for input box based on content
    let input_height = if state.temp_output.visible {
        3 // Default height when showing temporary output
    } else {
        // Dynamic height based on input content (min 3, includes borders)
        state.calculate_input_height() + 2 // +2 for borders
    };

    // Create the layout with header, content, and variable-height input areas
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),            // Header
            Constraint::Min(1),               // Content (flexible)
            Constraint::Length(input_height), // Dynamic-height input
        ])
        .split(size);

    // Render the header with agent list
    f.render_widget(Clear, chunks[0]);
    render_header(state, f, chunks[0]);

    // Render the content area with conversation history
    f.render_widget(Clear, chunks[1]);
    render_content(state, f, chunks[1]);

    // Render the input prompt
    f.render_widget(Clear, chunks[2]);
    render_input(state, f, chunks[2]);

    // Render the command suggestions popup if in command mode and temp output is not visible
    if state.command_mode && !state.temp_output.visible {
        render_command_suggestions(state, f);
    }

    // Render the temporary output window if visible
    if state.temp_output.visible {
        render_temp_output(state, f, chunks[2], chunks[1]);
    }
}

/// Render the temporary output window that overlays input and grows upward
pub fn render_temp_output(
    state: &TuiState,
    f: &mut Frame,
    input_area: Rect,
    content_area: Rect,
) {
    // Start with the input area as the base
    let mut output_area = input_area;

    // Calculate the total number of lines needed for content
    let available_width = output_area.width.saturating_sub(4); // Allow for borders and padding
    let needed_lines = state.temp_output.count_lines(available_width);

    // Determine how many lines we can extend upward into the content area
    let max_extension = content_area.height.saturating_sub(5) as usize; // Leave 5 lines of content visible
    let extension_lines = needed_lines.saturating_sub(1).min(max_extension);

    // Extend upward if needed
    if extension_lines > 0 {
        output_area.y = output_area.y.saturating_sub(extension_lines as u16);
        output_area.height += extension_lines as u16;
    }

    // Clear the area
    f.render_widget(Clear, output_area);

    // Create the temporary output widget with dark orange styling
    let content_text = state.temp_output.content.join("\n");
    let output_widget = Paragraph::new(content_text)
        .style(
            Style::default()
                .fg(Color::LightCyan) // More visible cyan text instead of white
                .bg(Color::Rgb(180, 80, 0)),
        ) // Dark orange background
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(255, 140, 0))) // Brighter orange border
                .title(format!(
                    "{} (Press ESC or Enter to dismiss)",
                    state.temp_output.title
                ))
                .title_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: true });

    // Render the output
    f.render_widget(output_widget, output_area);
}

/// Render command suggestions popup
pub fn render_command_suggestions(state: &TuiState, f: &mut Frame) {
    // Only render if suggestions are visible and we have any
    if !state.command_suggestions.visible
        || state.command_suggestions.filtered_commands.is_empty()
    {
        return;
    }

    let area = f.size();

    // Calculate total rows needed (one per command)
    let num_commands = state.command_suggestions.filtered_commands.len();

    // Set a maximum height for the popup
    let popup_height = num_commands.min(8) as u16 + 2; // +2 for borders

    // Calculate width based on longest command and description
    let max_cmd_width = state
        .command_suggestions
        .filtered_commands
        .iter()
        .map(|cmd| cmd.name.len())
        .max()
        .unwrap_or(10) as u16;

    let max_desc_width = state
        .command_suggestions
        .filtered_commands
        .iter()
        .map(|cmd| cmd.description.len())
        .max()
        .unwrap_or(30) as u16;

    // Set popup width with some padding
    let popup_width = (max_cmd_width + max_desc_width + 10)
        .min(area.width.saturating_sub(4))
        .max(30);

    // Position popup at the left bottom edge of screen, above input area
    let input_area_y = area.height.saturating_sub(3); // Input is 3 lines from bottom

    // Fixed position at left edge
    let popup_x = 0;
    let popup_y = input_area_y.saturating_sub(popup_height);

    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    // Clear the area under the popup
    f.render_widget(Clear, popup_area);

    // Create lines for each suggestion with proper highlighting
    let mut content_lines: Vec<Line> = Vec::with_capacity(num_commands);

    for (index, suggestion) in state
        .command_suggestions
        .filtered_commands
        .iter()
        .enumerate()
    {
        // Determine if this is the selected suggestion
        let is_selected = index == state.command_suggestions.selected_index;

        // Create style for command name based on selection
        let cmd_style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };

        // Create style for description
        let desc_style = if is_selected {
            Style::default().bg(Color::White).fg(Color::Black)
        } else {
            Style::default().fg(Color::Gray)
        };

        // Format the line with proper spacing
        let line = Line::from(vec![
            Span::styled(suggestion.name.clone(), cmd_style),
            Span::styled(" - ", desc_style),
            Span::styled(suggestion.description.clone(), desc_style),
        ]);

        content_lines.push(line);
    }

    // Create the suggestions widget
    let suggestions_widget = Paragraph::new(content_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Commands (TAB to complete)"),
    );

    // Render the suggestions
    f.render_widget(suggestions_widget, popup_area);
}

/// Render the header with agent list
pub fn render_header(state: &TuiState, f: &mut Frame, area: Rect) {
    // Get agents directly using the static function
    let agents = crate::agent::get_agents();
    
    // Create spans for each agent
    let agent_spans = agents
        .iter()
        .map(|(id, name)| {
            // Get state indicator based on agent state
            let state_opt = crate::agent::get_agent_state(*id).ok();
            let state_char = if let Some(state) = state_opt {
                TuiState::get_state_indicator(&state)
            } else {
                "?" // Unknown state
            };

            if *id == state.selected_agent_id {
                Span::styled(
                    format!(" {} {} [{}] ", state_char, name, id),
                    Style::default()
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    format!(" {} {} [{}] ", state_char, name, id),
                    Style::default().fg(Color::LightBlue),
                )
            }
        })
        .collect::<Vec<Span>>();

    // Add a final span with empty content to fill remaining space
    // This ensures old content is fully cleared
    let mut all_spans = agent_spans;
    all_spans.push(Span::styled(
        " ".repeat((area.width as usize).saturating_sub(2)), // -2 for borders
        Style::default().fg(Color::DarkGray), // Ensure text is visible but subdued
    ));

    let header = Paragraph::new(Line::from(all_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Agents"),
    );

    f.render_widget(header, area);
}

/// Render the content area with conversation history
pub fn render_content(state: &TuiState, f: &mut Frame, area: Rect) {
    let lines = state.agent_buffer.lines();
    let total_lines = lines.len();

    // Calculate visible area height (accounting for borders)
    // -2 for the top and bottom borders of the block
    let visible_height = area.height.saturating_sub(2) as usize;

    // Create empty list items for filling the visible area
    let mut items: Vec<Line> = Vec::with_capacity(visible_height);

    if total_lines > 0 {
        // Calculate the start index for the visible region
        let start_idx = if state.scroll_offset < total_lines {
            state.scroll_offset
        } else {
            0
        };

        // When at maximum scroll offset (bottom), we want to ensure the last line is visible
        // This requires special handling
        let adjusted_start =
            if state.scroll_offset == state.max_scroll_offset && total_lines > visible_height {
                // Ensure we show the last line by adjusting start index
                // This forces display of the range ending with the last line
                total_lines - visible_height
            } else {
                // Normal scroll position
                start_idx
            };

        // Get the visible range of lines
        let end_idx = (adjusted_start + visible_height).min(total_lines);

        // Extract the lines for the visible range
        if adjusted_start < total_lines {
            // Use an iterator to be more explicit about the range
            items = (adjusted_start..end_idx)
                .filter_map(|i| lines.get(i))
                .map(|line| line.converted_line.clone())
                .collect();
        }
    }

    // Fill remaining space with empty lines to ensure old content is cleared
    while items.len() < visible_height {
        items.push(Line::from(""));
    }

    // Create title with scroll info and most recent messages indicator
    let scroll_info = if total_lines > visible_height {
        let latest_indicator = if state.scroll_offset == state.max_scroll_offset {
            " (Most Recent ↓)"
        } else {
            ""
        };

        format!(
            " | Scroll: {}/{}{}",
            state.scroll_offset, state.max_scroll_offset, latest_indicator
        )
    } else {
        String::new()
    };

    let title = format!("Conversation ({} lines{})", total_lines, scroll_info);

    let conversation = Paragraph::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title),
    );
    f.render_widget(conversation, area);
}

/// Render the input area with support for multi-line text
pub fn render_input(state: &TuiState, f: &mut Frame, area: Rect) {
    // Normal input rendering
    let input_style = if state.command_mode {
        Style::default().fg(Color::Yellow)
    } else if state.pound_command_mode {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Black)
    };

    // Get the agent state from the agent manager
    let agent_state_str = state.get_agent_state_string();

    // Get the current agent name using static function
    let agents = crate::agent::get_agents();
    let agent_name = agents
        .iter()
        .find_map(|(id, name)| if *id == state.selected_agent_id { Some(name.clone()) } else { None })
        .unwrap_or_else(|| "Unknown".to_string());

    // Create title with agent state
    let title = format!(
        "Input [{} [{}] | {}]",
        agent_name, state.selected_agent_id, agent_state_str
    );

    // Create the input widget with text wrapping enabled
    let input_text = Paragraph::new(state.input.clone())
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(title),
        )
        .wrap(Wrap { trim: true }); // Enable wrapping, don't trim to preserve formatting

    f.render_widget(input_text, area);

    // Only show cursor if temporary output is not visible
    if !state.temp_output.visible {
        // Calculate cursor position for wrapped text
        // This is a simplified calculation that works for basic wrapping
        let available_width = area.width.saturating_sub(2) as usize; // -2 for borders

        // Calculate cursor row and column
        let cursor_pos_in_chars = state.cursor_position;
        let cursor_column = (cursor_pos_in_chars % available_width) as u16 + 1; // +1 for border
        let cursor_row = (cursor_pos_in_chars / available_width) as u16 + 1; // +1 for border

        // Show cursor at calculated position
        f.set_cursor(area.x + cursor_column, area.y + cursor_row);
    }
}