//! Handlebars template implementation
//!
//! This module provides a Handlebars-based template system with grammar integration
//! for consistent tool formatting.

use handlebars::{
    Context, Handlebars, Helper, HelperDef, HelperResult, Output, RenderContext, RenderError,
    Renderable,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::prompts::Grammar;
use std::sync::{Arc, Mutex};

/// Errors that can occur with templates
#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Template rendering error: {0}")]
    Render(#[from] handlebars::RenderError),

    #[error("Template '{0}' not found")]
    TemplateNotFound(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid template format: {0}")]
    InvalidFormat(String),

    #[error("Missing grammar implementation")]
    MissingGrammar,
}

/// Manager for Handlebars templates with grammar integration
pub struct TemplateManager {
    /// Handlebars registry
    handlebars: Handlebars<'static>,

    /// Grammar store for helper access
    grammar: Arc<dyn Grammar>,

    /// Templates directory
    templates_dir: PathBuf,
}

/// Helper for formatting tool invocations
struct ToolHelper {
    grammar: Arc<dyn Grammar>,
}

impl HelperDef for ToolHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let tool_name = h
            .param(0)
            .and_then(|v| v.value().as_str())
            .ok_or_else(|| RenderError::new("Tool name is required"))?;

        // Get the tool content
        let content = h
            .template()
            .map(|t| t.renders(r, ctx, rc))
            .transpose()
            .unwrap()
            .unwrap_or_default();

        out.write(&self.grammar.format_tool_call(tool_name, &content))?;

        Ok(())
    }
}

/// Helper for formatting successful tool results
struct DoneHelper {
    grammar: Arc<dyn Grammar>,
}

impl HelperDef for DoneHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        // Get the optional tool name parameter
        let tool_name = h.param(0).and_then(|v| v.value().as_str()).unwrap();

        // Get the index parameter (required)
        let index = h
            .param(1)
            .and_then(|v| v.value().as_u64())
            .ok_or_else(|| RenderError::new("Tool result index is required"))?;

        // Get the tool result content
        let content = h
            .template()
            .map(|t| t.renders(r, ctx, rc))
            .transpose()
            .unwrap()
            .unwrap_or_default();

        out.write(
            &self
                .grammar
                .format_tool_result(tool_name, index as usize, &content),
        )?;

        Ok(())
    }
}

/// Helper for conditionally including content based on tool enablement
struct IfToolHelper;

impl HelperDef for IfToolHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        // Get the tool name (required parameter) and convert to lowercase
        let tool_name = h
            .param(0)
            .and_then(|v| v.value().as_str())
            .ok_or_else(|| RenderError::new("Tool name is required for iftool helper"))?
            .to_lowercase();

        // Check if this tool is in the enabled_tools array
        let enabled_tools = ctx.data().get("enabled_tools").and_then(|v| v.as_array());

        let is_enabled = match enabled_tools {
            Some(tools) => tools
                .iter()
                .any(|t| t.as_str().map_or(false, |s| s.to_lowercase() == tool_name)),
            None => false, // If no enabled_tools array exists, default to false
        };

        // Render the content only if the tool is enabled
        if is_enabled {
            if let Some(template) = h.template() {
                template.render(r, ctx, rc, out)?;
            }
        }

        Ok(())
    }
}

/// Helper for formatting tool error results
struct ErrorHelper {
    grammar: Arc<dyn Grammar>,
}

impl HelperDef for ErrorHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        // Get the optional tool name parameter
        let tool_name = h.param(0).and_then(|v| v.value().as_str()).unwrap();

        // Get the index parameter (required)
        let index = h
            .param(1)
            .and_then(|v| v.value().as_u64())
            .ok_or_else(|| RenderError::new("Tool error index is required"))?;

        // Get the tool error content
        let content = h
            .template()
            .map(|t| t.renders(r, ctx, rc))
            .transpose()
            .unwrap()
            .unwrap_or_default();

        out.write(
            &self
                .grammar
                .format_tool_error(tool_name, index as usize, &content),
        )?;

        Ok(())
    }
}

struct PatchHelper {
    grammar: Arc<dyn Grammar>,
}

impl HelperDef for PatchHelper {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let before = h.template().unwrap();
        let after = h.inverse().unwrap();

        let patch = self.grammar.format_patch(
            &before.renders(r, ctx, rc)?,
            &after.renders(r, ctx, rc)?,
        );

        out.write(&patch)?;
        Ok(())
    }
}

impl TemplateManager {
    /// Create a new template manager
    pub fn new(grammar: Arc<dyn Grammar>) -> Self {
        let handlebars_instance = Self::create_handlebars_with_helpers(Arc::clone(&grammar));

        Self {
            handlebars: handlebars_instance,
            grammar,
            templates_dir: PathBuf::from("prompts"),
        }
    }

    /// Set the template directory
    pub fn with_templates_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.templates_dir = dir.as_ref().to_path_buf();
        self
    }

    /// Load a template from a file
    pub fn load_template(&mut self, template_name: &str) -> Result<(), TemplateError> {
        let path = self.templates_dir.join(format!("{}.hbs", template_name));

        if !path.exists() {
            return Err(TemplateError::TemplateNotFound(template_name.to_string()));
        }

        let source = fs::read_to_string(&path)?;

        self.handlebars
            .register_template_string(template_name, &source)
            .map_err(|e| TemplateError::Render(e.into()))?;

        Ok(())
    }

    /// Load all templates from the templates directory
    /// This loads regular templates and registers partials
    pub fn load_all_templates(&mut self) -> Result<(), TemplateError> {
        if !self.templates_dir.exists() {
            return Err(TemplateError::TemplateNotFound(
                self.templates_dir.to_string_lossy().to_string(),
            ));
        }

        // Read all .hbs files from the directory
        for entry in fs::read_dir(&self.templates_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "hbs") {
                let template_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed");

                let source = fs::read_to_string(&path)?;

                // Register as both a template and a partial
                self.handlebars
                    .register_template_string(template_name, &source)
                    .map_err(|e| TemplateError::Render(e.into()))?;

                self.handlebars
                    .register_partial(template_name, &source)
                    .map_err(|e| TemplateError::Render(e.into()))?;
            }
        }

        Ok(())
    }

    /// Render a template with specific tools enabled
    pub fn render_with_tool_enablement(
        &self,
        template_name: &str,
        enabled_tools: &[&str],
    ) -> Result<String, TemplateError> {
        // Create data object for template variables
        let mut data = serde_json::Map::new();

        // Convert all enabled tools to lowercase
        let lowercase_tools: Vec<String> = enabled_tools.iter().map(|s| s.to_lowercase()).collect();

        // Add the enabled_tools array for template usage
        data.insert("enabled_tools".to_string(), json!(lowercase_tools));

        // Render the template with the variables
        self.handlebars
            .render(template_name, &Value::Object(data))
            .map_err(|e| TemplateError::Render(e))
    }

    /// Create a Handlebars instance with registered helpers
    fn create_handlebars_with_helpers(grammar: Arc<dyn Grammar>) -> Handlebars<'static> {
        let mut handlebars = Handlebars::new();

        // Clone Arc for each helper
        let tool_grammar = Arc::clone(&grammar);
        let done_grammar = Arc::clone(&grammar);
        let error_grammar = Arc::clone(&grammar);

        // Register all helpers
        handlebars.register_helper(
            "tool",
            Box::new(ToolHelper {
                grammar: tool_grammar,
            }),
        );
        handlebars.register_helper(
            "done",
            Box::new(DoneHelper {
                grammar: done_grammar,
            }),
        );
        handlebars.register_helper(
            "error",
            Box::new(ErrorHelper {
                grammar: error_grammar,
            }),
        );

        handlebars.register_helper("patch", Box::new(PatchHelper {
            grammar,
        }));

        // Register the iftool helper for tool-specific conditional content
        handlebars.register_helper("iftool", Box::new(IfToolHelper));

        handlebars.register_escape_fn(|s| s.to_string());

        handlebars
    }
}