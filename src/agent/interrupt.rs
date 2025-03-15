//! Interrupt coordination for agents
//!
//! This module provides structures and functions for coordinating interrupt
//! signals between shell tools and the main agent processing loop.

use crate::agent::types::{InterruptReceiver, InterruptSender, InterruptSignal};
use std::future::Future;
// Removed unused imports
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Coordinator for routing interrupts between shell commands and agent processing
pub struct InterruptCoordinator {
    /// Optional container for shell interrupt data when a shell is running
    interrupt_data: Mutex<Option<InterruptSender>>,

    /// Channel for interrupting the agent itself
    agent_interrupt_tx: mpsc::Sender<()>,
}

impl InterruptCoordinator {
    /// Create a new interrupt coordinator
    pub fn new(agent_interrupt_tx: mpsc::Sender<()>) -> Self {
        Self {
            interrupt_data: Mutex::new(None),
            agent_interrupt_tx,
        }
    }

    /// Set whether a shell is running and update interrupt data
    pub fn set_shell_running(&self, _running: bool, data: Option<InterruptSender>) {
        // Then update the interrupt channel
        *self.interrupt_data.lock().unwrap() = data;
    }

    /// Handle an interrupt based on current state
    pub fn handle_interrupt(&self) -> impl Future<Output = ()> + Send + 'static {
        let data = { self.interrupt_data.lock().unwrap().clone() };
        let agent_tx = self.agent_interrupt_tx.clone();

        async move {
            // Shell has priority - interrupt shell if running
            if let Some(interrupt) = data {
                // Send interrupt with reason
                match interrupt
                    .send(InterruptSignal::new(Some(
                        "User requested interruption (Ctrl+C)".to_string(),
                    )))
                    .await
                {
                    Ok(_) => {
                        bprintln!(
                            "{}{}{} by user signal{}",
                            crate::constants::FORMAT_BOLD,
                            crate::constants::FORMAT_BLUE,
                            "Shell interrupted",
                            crate::constants::FORMAT_RESET
                        );
                    }
                    Err(e) => {
                        // Channel error - shell might have completed just before interrupt
                        bprintln !(error:"Failed to interrupt shell: {}", e);

                        // Fall back to agent interrupt if shell interrupt fails
                        if let Err(e) = agent_tx.try_send(()) {
                            bprintln !(error:"Failed to interrupt agent after shell interrupt failure: {}", e);
                        } else {
                            // Only show internal fallback mechanism details in debug builds
                            bprintln !(dev: "{}{}{} to agent interrupt{}",
                                crate::constants::FORMAT_BOLD,
                                crate::constants::FORMAT_BLUE,
                                "Falling back",
                                crate::constants::FORMAT_RESET);
                        }
                    }
                }
            } else {
                // No shell running - interrupt agent processing
                if let Err(e) = agent_tx.try_send(()) {
                    bprintln !(error:"Failed to interrupt agent: {}", e);
                }
            }
        }
    }
}

/// Spawn a task to monitor for Ctrl+C signals and route them appropriately
pub fn spawn_interrupt_monitor(
    coordinator: Arc<InterruptCoordinator>,
    mut interrupt_receiver: InterruptReceiver,
) -> JoinHandle<()> {
    crate::output::spawn(async move {
        loop {
            if (interrupt_receiver.recv().await).is_some() {
                // Handle the interrupt using the coordinator
                coordinator.handle_interrupt().await;
            }
        }
    })
}
