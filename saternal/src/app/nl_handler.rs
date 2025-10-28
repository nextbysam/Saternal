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

/// Display "Generating..." message in terminal
pub fn display_nl_processing_message(tab_manager: &Arc<Mutex<crate::tab::TabManager>>) {
    let message = "\r\n🤖 Generating command with Claude...\r\n";
    write_to_terminal(tab_manager, message);
}

/// Display command suggestions with warnings if needed
fn display_suggestions(tab_manager: &Arc<Mutex<crate::tab::TabManager>>, commands: &[String]) {
    let mut message = String::from("\r\n");
    
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

    // Display warning if needed
    if highest_level != ConfirmationLevel::Standard {
        if let Some(warning) = commands
            .iter()
            .find_map(|cmd| get_warning_message(cmd))
        {
            message.push_str(&format!("{}\r\n\r\n", warning));
        }
    }

    message.push_str("💡 Suggested command(s):\r\n");

    // Display commands with numbering
    for (i, cmd) in commands.iter().enumerate() {
        if commands.len() > 1 {
            message.push_str(&format!("  {}. {}\r\n", i + 1, cmd));
        } else {
            message.push_str(&format!("  {}\r\n", cmd));
        }
    }

    message.push_str("\r\n");

    // Prompt based on confirmation level
    match highest_level {
        ConfirmationLevel::Standard => {
            message.push_str("Execute? [y/n]: ");
        }
        ConfirmationLevel::Sudo => {
            message.push_str("Execute? Type 'yes' to confirm: ");
        }
        ConfirmationLevel::Elevated => {
            message.push_str("⚠️  Type 'yes' to execute (or 'n' to cancel): ");
        }
    }

    write_to_terminal(tab_manager, &message);
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
/// Returns true if commands should be executed
pub fn handle_confirmation_input(
    input: &str,
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

    let input_lower = input.trim().to_lowercase();
    
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
        let commands = tab.pending_nl_commands.take().unwrap();
        tab.nl_confirmation_mode = false;
        Some(commands)
    } else if input_lower == "n" || input_lower == "no" {
        // User cancelled
        tab.pending_nl_commands = None;
        tab.nl_confirmation_mode = false;
        let _ = tab.write_input(b"\r\nCancelled.\r\n");
        None
    } else {
        // Invalid input - ask again
        let message = match highest_level {
            ConfirmationLevel::Standard => "Please type 'y' or 'n': ",
            _ => "Please type 'yes' or 'n': ",
        };
        let _ = tab.write_input(message.as_bytes());
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
        for cmd in &commands {
            // Write each command to PTY with newline
            let cmd_with_newline = format!("{}\n", cmd);
            if let Err(e) = tab.write_input(cmd_with_newline.as_bytes()) {
                log::error!("Failed to execute command '{}': {}", cmd, e);
            } else {
                log::info!("✓ Executing: {}", cmd);
            }
        }
    }
}
