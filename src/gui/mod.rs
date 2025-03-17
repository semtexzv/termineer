//! GUI module for Termineer
//!
//! This module provides a graphical frontend for Termineer using egui.

use eframe::egui;
use eframe::egui::{Color32, RichText, Rounding, Vec2};

/// Main application state
struct TermineerApp {
    version: String,
    dragging: bool,
    window_pos: egui::Pos2,
    last_pointer_pos: Option<egui::Pos2>,
}

impl TermineerApp {
    fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            dragging: false,
            window_pos: egui::pos2(0.0, 0.0),
            last_pointer_pos: None,
        }
    }
}

impl eframe::App for TermineerApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        catppuccin_egui::set_theme(ctx, catppuccin_egui::MACCHIATO);

        // Custom titlebar (draggable)
        egui::TopBottomPanel::top("titlebar")
            .frame(egui::Frame::none().fill(catppuccin_egui::MOCHA.base))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("✨ Termineer")
                            .size(18.0)
                            .color(catppuccin_egui::MOCHA.text),
                    );
                });
            });

        // Central panel with main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(catppuccin_egui::MOCHA.base))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Title and version
                    ui.add_space(40.0);
                    ui.heading(
                        RichText::new("Termineer")
                            .color(catppuccin_egui::MOCHA.text)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("Your Terminal Engineer")
                            .italics()
                            .color(catppuccin_egui::MOCHA.subtext0),
                    );
                    ui.label(
                        RichText::new(format!("Version {}", self.version))
                            .small()
                            .color(catppuccin_egui::MOCHA.overlay0),
                    );
                    ui.add_space(40.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            let new_chat_button =
                                egui::Button::new(RichText::new("New Chat").color(Color32::WHITE))
                                    .min_size(Vec2::new(120.0, 40.0))
                                    .fill(catppuccin_egui::MOCHA.blue);

                            let settings_button =
                                egui::Button::new(RichText::new("Settings").color(Color32::WHITE))
                                    .min_size(Vec2::new(120.0, 40.0))
                                    .fill(catppuccin_egui::MOCHA.surface1);

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

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparent background for rounded corners
        egui::Rgba::TRANSPARENT.to_array()
    }
}

/// Start the GUI application
pub fn run_gui() {
    println!("Starting Termineer GUI...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0])
            .with_decorations(false) // Remove standard title bar/border
            .with_transparent(true), // Enable transparency for rounded corners
        ..Default::default()
    };

    // Create app with Catppuccin Mocha theme
    eframe::run_native(
        "Termineer",
        options,
        Box::new(|_cc| {
            // Create app
            Ok(Box::new(TermineerApp::new()))
        }),
    )
    .expect("Failed to start GUI application");
}
