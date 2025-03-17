//! GUI module for Termineer
//!
//! This module provides a graphical frontend for Termineer using egui.

use eframe::egui;
use eframe::egui::{Color32, RichText, Rounding, Vec2};

/// Main application state
struct TermineerApp {
    version: String,
}

impl TermineerApp {
    fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl eframe::App for TermineerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set default fonts and style
        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::proportional(32.0)),
            (egui::TextStyle::Body, egui::FontId::proportional(18.0)),
            (egui::TextStyle::Monospace, egui::FontId::monospace(14.0)),
            (egui::TextStyle::Button, egui::FontId::proportional(16.0)),
            (egui::TextStyle::Small, egui::FontId::proportional(12.0)),
        ]
        .into();
        ctx.set_style(style);

        // Central panel with main content
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Title and version
                ui.add_space(80.0);
                ui.heading(RichText::new("Termineer").color(Color32::WHITE).strong());
                ui.label(RichText::new("Your Terminal Engineer").italics().color(Color32::LIGHT_GRAY));
                ui.label(RichText::new(format!("Version {}", self.version)).small().color(Color32::GRAY));
                ui.add_space(40.0);

                // Buttons
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        let new_chat_button = egui::Button::new(RichText::new("New Chat").color(Color32::WHITE))
                            .min_size(Vec2::new(120.0, 40.0))
                            .rounding(Rounding::same(8.0))
                            .fill(Color32::from_rgb(74, 125, 255));
                        
                        let settings_button = egui::Button::new(RichText::new("Settings").color(Color32::WHITE))
                            .min_size(Vec2::new(120.0, 40.0))
                            .rounding(Rounding::same(8.0))
                            .fill(Color32::from_rgb(68, 68, 68));

                        if ui.add(new_chat_button).clicked() {
                            // TODO: Handle New Chat button click
                            println!("New Chat clicked");
                        }

                        ui.add_space(10.0);

                        if ui.add(settings_button).clicked() {
                            // TODO: Handle Settings button click
                            println!("Settings clicked");
                        }
                    });
                });
            });
        });
    }
}

/// Start the GUI application
pub fn run_gui() {
    println!("Starting Termineer GUI...");
    
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    // Create app with dark mode
    eframe::run_native(
        "Termineer",
        options,
        Box::new(|cc| {
            // Set dark mode
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            
            // Create app
            Box::new(TermineerApp::new())
        }),
    )
    .expect("Failed to start GUI application");
}