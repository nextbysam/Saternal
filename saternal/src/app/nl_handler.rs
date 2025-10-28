/// Natural language command handler
/// Manages async LLM requests and terminal UI feedback

use parking_lot::Mutex;
use saternal_core::{CommandContext, ConfirmationLevel, LLMClient, get_confirmation_level, get_warning_message};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Message types sent from async task to main thread
#[derive(Debug, Clone)]
pub enum NLMessage {
    /// LLM successfully generated commands
    CommandGenerated {
        original_request: String,
        commands: Vec<String>,
    },
    /// Error occurred during generation
    Error {
        original_request: String,
        error: String,
    },
}

/// Handle natural language command request asynchronously
/// This runs in a tokio task and sends results via channel
pub async fn handle_nl_command_async(
    nl_request: String,
    llm_client: Arc<LLMClient>,
    tx: mpsc::Sender<NLMessage>,
) {
    log::info!("🤖 Processing NL request: '{}'", nl_request);

    // Gather context
    let context = CommandContext::gather();

    // Call LLM API
    match llm_client.generate_command(&nl_request, &context).await {
        Ok(commands) => {
            log::info!("✓ Generated {} command(s)", commands.len());
            let _ = tx
                .send(NLMessage::CommandGenerated {
                    original_request: nl_request,
                    commands,
                })
                .await;
        }
        Err(e) => {
            log::error!("✗ Failed to generate command: {}", e);
            let _ = tx
                .send(NLMessage::Error {
                    original_request: nl_request,
                    error: e.to_string(),
                })
                .await;
        }
    }
}

/// Handle NL message received from async task (runs in main thread)
/// Displays suggestions or errors in terminal
pub fn handle_nl_message(
    msg: NLMessage,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) {
    match msg {
        NLMessage::CommandGenerated { commands, .. } => {
            display_suggestions(tab_manager, &commands);
            
            // Set pending commands state
            let mut tab_mgr = tab_manager.lock();
            if let Some(tab) = tab_mgr.active_tab_mut() {
                tab.pending_nl_commands = Some(commands);
                tab.nl_confirmation_mode = true;
            }
        }
        NLMessage::Error { error, .. } => {
            display_error_message(tab_manager, &error);
        }
    }
}

/// Log "Generating..." message (don't write to terminal to avoid shell execution)
pub fn display_nl_processing_message(_tab_manager: &Arc<Mutex<crate::tab::TabManager>>) {
    log::info!("🤖 Generating command with Claude...");
    // Don't write to PTY stdin - it would cause shell to try executing the emoji as a command
}

/// Log command suggestions (don't write to terminal to avoid shell execution)
fn display_suggestions(_tab_manager: &Arc<Mutex<crate::tab::TabManager>>, commands: &[String]) {
    // Check if any commands are dangerous
    let highest_level = commands
        .iter()
        .map(|cmd| get_confirmation_level(cmd))
        .max_by_key(|level| match level {
            ConfirmationLevel::Standard => 0,
            ConfirmationLevel::Sudo => 1,
            ConfirmationLevel::Elevated => 2,
        })
        .unwrap_or(ConfirmationLevel::Standard);

    // Log warning if needed
    if highest_level != ConfirmationLevel::Standard {
        if let Some(warning) = commands
            .iter()
            .find_map(|cmd| get_warning_message(cmd))
        {
            log::warn!("{}", warning);
        }
    }

    // Log count of commands generated
    if commands.len() == 1 {
        log::info!("💡 Generated 1 command");
    } else {
        log::info!("💡 Generated {} commands", commands.len());
    }

    // Log the actual commands
    for (i, cmd) in commands.iter().enumerate() {
        log::info!("  Command {}: {}", i + 1, cmd);
    }

    // Log the confirmation prompt
    let prompt = match highest_level {
        ConfirmationLevel::Standard => "⏳ Waiting for y/n confirmation at shell prompt",
        ConfirmationLevel::Sudo => "⏳ Waiting for 'yes' confirmation (sudo command)",
        ConfirmationLevel::Elevated => "⚠️  Waiting for 'yes' confirmation (DANGEROUS command)",
    };
    log::info!("{}", prompt);
    log::info!("💡 Tip: Type 'y' or 'yes' and press Enter to execute, 'n' to cancel");
    
    // Don't write to PTY stdin - terminal stays at normal shell prompt
    // User types y/n there, and we intercept it in confirmation mode
}

/// Display error message in terminal
fn display_error_message(tab_manager: &Arc<Mutex<crate::tab::TabManager>>, error: &str) {
    let message = format!("\r\n❌ Failed to generate command: {}\r\n", error);
    write_to_terminal(tab_manager, &message);
}

/// Helper to write message to active tab's terminal
fn write_to_terminal(tab_manager: &Arc<Mutex<crate::tab::TabManager>>, message: &str) {
    let mut tab_mgr = tab_manager.lock();
    if let Some(tab) = tab_mgr.active_tab_mut() {
        let _ = tab.write_input(message.as_bytes());
    }
}

/// Handle user confirmation response
/// Reads from tab.confirmation_input buffer
/// Returns Some(commands) if user confirmed, None if cancelled
pub fn handle_confirmation_input(
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) -> Option<Vec<String>> {
    let mut tab_mgr = tab_manager.lock();
    let tab = tab_mgr.active_tab_mut()?;

    // Check if we're in confirmation mode
    if !tab.nl_confirmation_mode {
        return None;
    }

    let commands = tab.pending_nl_commands.as_ref()?;
    
    // Determine required confirmation level
    let highest_level = commands
        .iter()
        .map(|cmd| get_confirmation_level(cmd))
        .max_by_key(|level| match level {
            ConfirmationLevel::Standard => 0,
            ConfirmationLevel::Sudo => 1,
            ConfirmationLevel::Elevated => 2,
        })
        .unwrap_or(ConfirmationLevel::Standard);

    // Read from confirmation buffer (user's actual input, not from grid)
    let input_lower = tab.confirmation_input.trim().to_lowercase();
    
    let should_execute = match highest_level {
        ConfirmationLevel::Standard => {
            input_lower == "y" || input_lower == "yes"
        }
        ConfirmationLevel::Sudo | ConfirmationLevel::Elevated => {
            input_lower == "yes"
        }
    };

    if should_execute {
        // User confirmed - return commands for execution
        log::info!("✓ User confirmed execution");
        let commands = tab.pending_nl_commands.take().unwrap();
        tab.nl_confirmation_mode = false;
        tab.confirmation_input.clear();
        Some(commands)
    } else if input_lower == "n" || input_lower == "no" {
        // User cancelled
        log::info!("✗ User cancelled execution");
        tab.pending_nl_commands = None;
        tab.nl_confirmation_mode = false;
        tab.confirmation_input.clear();
        // Clear the input line the user typed
        let _ = tab.write_input(b"\r\n");
        None
    } else {
        // Not a y/n response - exit confirmation mode and let shell handle it
        log::info!("User entered something else, exiting confirmation mode");
        tab.pending_nl_commands = None;
        tab.nl_confirmation_mode = false;
        // Don't clear input - let it pass through to shell
        None
    }
}

/// Execute commands by writing them to PTY
pub fn execute_nl_commands(
    commands: Vec<String>,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) {
    let mut tab_mgr = tab_manager.lock();
    if let Some(tab) = tab_mgr.active_tab_mut() {
        // Add newline after the "y" confirmation to clear the line
        let _ = tab.write_input(b"\r\n");
        
        // Execute each command (shell will echo them)
        for (i, cmd) in commands.iter().enumerate() {
            log::info!("✓ Executing command {}: {}", i + 1, cmd);
            
            // Write command to PTY stdin with newline to execute
            let cmd_with_newline = format!("{}\n", cmd);
            if let Err(e) = tab.write_input(cmd_with_newline.as_bytes()) {
                log::error!("Failed to execute command '{}': {}", cmd, e);
            }
        }
    }
}
