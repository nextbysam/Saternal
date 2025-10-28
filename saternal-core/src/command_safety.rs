/// Command safety validation for LLM-generated commands
/// Detects dangerous patterns and determines required confirmation level

// Dangerous command patterns (compiled into binary)
static DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "rm -rf $HOME",
    "dd if=",
    "mkfs",
    "fdisk",
    "> /dev/sd",
    "> /dev/disk",
    "chmod -R 777",
    "chmod 777",
    "kill -9 -1",
    "killall -9",
    ":(){ :|:& };:", // Fork bomb
    "mv / ",
    "mv /* ",
];

static SYSTEM_DIRECTORIES: &[&str] = &[
    "/bin", "/sbin", "/usr", "/etc", "/var", "/boot", "/sys", "/proc",
];

/// Confirmation level required for command execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationLevel {
    /// Standard confirmation: [y/n]
    Standard,
    /// Elevated confirmation: type 'yes' to confirm
    Elevated,
    /// Sudo command: warn about privileged access
    Sudo,
}

/// Check if a command is potentially dangerous
#[inline]
pub fn is_dangerous_command(cmd: &str) -> bool {
    let cmd_lower = cmd.to_lowercase();
    
    // Check for explicit dangerous patterns
    for pattern in DANGEROUS_PATTERNS {
        if cmd_lower.contains(pattern) {
            return true;
        }
    }
    
    // Check for rm with system directories
    if cmd_lower.starts_with("rm ") || cmd_lower.contains(" rm ") {
        for sys_dir in SYSTEM_DIRECTORIES {
            if cmd.contains(sys_dir) {
                return true;
            }
        }
    }
    
    // Check for recursive delete without path (dangerous)
    if (cmd_lower.contains("rm -rf") || cmd_lower.contains("rm -r")) 
        && (cmd_lower.contains(" /") || cmd_lower.contains(" ~")) {
        return true;
    }
    
    false
}

/// Check if command requires sudo/root privileges
#[inline]
pub fn requires_sudo(cmd: &str) -> bool {
    cmd.trim_start().starts_with("sudo ")
}

/// Get the appropriate confirmation level for a command
#[inline]
pub fn get_confirmation_level(cmd: &str) -> ConfirmationLevel {
    if is_dangerous_command(cmd) {
        ConfirmationLevel::Elevated
    } else if requires_sudo(cmd) {
        ConfirmationLevel::Sudo
    } else {
        ConfirmationLevel::Standard
    }
}

/// Get a human-readable warning message for a command
pub fn get_warning_message(cmd: &str) -> Option<String> {
    let level = get_confirmation_level(cmd);
    
    match level {
        ConfirmationLevel::Standard => None,
        ConfirmationLevel::Elevated => Some(
            "⚠️  WARNING: This command may permanently delete or modify system files!".to_string()
        ),
        ConfirmationLevel::Sudo => Some(
            "🔐 This command requires root/administrator privileges.".to_string()
        ),
    }
}

/// Sanitize command for display (remove sensitive data if any)
/// For now, just returns the command as-is, but can be extended
#[inline]
pub fn sanitize_for_display(cmd: &str) -> String {
    cmd.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_patterns() {
        assert!(is_dangerous_command("rm -rf /"));
        assert!(is_dangerous_command("rm -rf /*"));
        assert!(is_dangerous_command("sudo rm -rf /"));
        assert!(is_dangerous_command("dd if=/dev/zero of=/dev/sda"));
        assert!(is_dangerous_command("mkfs.ext4 /dev/sda"));
        assert!(is_dangerous_command("chmod -R 777 /"));
        assert!(is_dangerous_command("kill -9 -1"));
        assert!(is_dangerous_command(":(){ :|:& };:"));
    }

    #[test]
    fn test_safe_commands() {
        assert!(!is_dangerous_command("ls -la"));
        assert!(!is_dangerous_command("git status"));
        assert!(!is_dangerous_command("cat file.txt"));
        assert!(!is_dangerous_command("mkdir test"));
        assert!(!is_dangerous_command("rm file.txt"));
        assert!(!is_dangerous_command("rm -rf node_modules"));
    }

    #[test]
    fn test_system_directory_protection() {
        assert!(is_dangerous_command("rm -rf /usr/bin"));
        assert!(is_dangerous_command("rm -rf /etc"));
        assert!(is_dangerous_command("rm /bin/bash"));
        assert!(!is_dangerous_command("rm -rf /tmp/mydir"));
    }

    #[test]
    fn test_sudo_detection() {
        assert!(requires_sudo("sudo apt install package"));
        assert!(requires_sudo("sudo rm file"));
        assert!(!requires_sudo("ls -la"));
        assert!(!requires_sudo("git status"));
    }

    #[test]
    fn test_confirmation_levels() {
        assert_eq!(
            get_confirmation_level("ls -la"),
            ConfirmationLevel::Standard
        );
        assert_eq!(
            get_confirmation_level("sudo apt install"),
            ConfirmationLevel::Sudo
        );
        assert_eq!(
            get_confirmation_level("rm -rf /"),
            ConfirmationLevel::Elevated
        );
    }

    #[test]
    fn test_warning_messages() {
        assert!(get_warning_message("ls -la").is_none());
        assert!(get_warning_message("sudo apt install").is_some());
        assert!(get_warning_message("rm -rf /").is_some());
    }

    #[test]
    fn test_home_directory_deletion() {
        assert!(is_dangerous_command("rm -rf ~"));
        assert!(is_dangerous_command("rm -rf $HOME"));
    }
}
