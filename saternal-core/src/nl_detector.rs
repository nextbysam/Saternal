/// Natural language detection for command input
/// Uses static pattern matching for <100ns performance

// Static patterns compiled into binary (zero runtime allocation)
static QUESTION_WORDS: &[&str] = &[
    "how", "what", "when", "where", "why", "who",
    "show", "list", "find", "search", "get", "display",
    "tell", "explain", "help", "can you", "please",
];

static COMMON_COMMANDS: &[&str] = &[
    "ls", "cd", "pwd", "cat", "echo", "grep", "find", "git",
    "npm", "cargo", "python", "node", "curl", "wget", "ssh",
    "mkdir", "rm", "cp", "mv", "touch", "vim", "nano", "emacs",
    "man", "less", "more", "head", "tail", "wc", "sort", "uniq",
    "tar", "zip", "unzip", "gzip", "gunzip", "sed", "awk",
    "ps", "kill", "top", "htop", "df", "du", "mount", "umount",
];

static ARTICLE_WORDS: &[&str] = &[
    "the", "a", "an", "my", "all", "some", "every", "this", "that",
];

/// Detects if user input is natural language vs shell command
#[derive(Debug, Clone)]
pub struct NLDetector {
    min_words_for_nl: usize,
}

impl Default for NLDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl NLDetector {
    /// Create a new natural language detector
    #[inline]
    pub fn new() -> Self {
        Self {
            min_words_for_nl: 5,
        }
    }

    /// Check if input looks like natural language
    /// 
    /// Performance target: <100ns per call
    /// Uses static pattern matching with early returns
    #[inline(always)]
    pub fn is_natural_language(&self, line: &str) -> bool {
        // Fast path: empty or very short input
        if line.len() < 5 {
            return false;
        }

        let line_lower = line.to_lowercase();
        let words: Vec<&str> = line.split_whitespace().collect();

        // Fast path: too few words
        if words.is_empty() {
            return false;
        }

        // Check 1: Question mark = natural language
        if line.contains('?') {
            return true;
        }

        // Check 2: Starts with question word
        for question_word in QUESTION_WORDS {
            if line_lower.starts_with(question_word) {
                return true;
            }
        }

        // Check 3: Starts with known command = NOT natural language
        let first_word = words[0].to_lowercase();
        for cmd in COMMON_COMMANDS {
            if first_word == *cmd {
                return false;
            }
        }

        // Check 4: Contains articles + multiple words
        let has_articles = ARTICLE_WORDS
            .iter()
            .any(|article| words.iter().any(|w| w.to_lowercase() == **article));

        if has_articles && words.len() > 3 {
            return true;
        }

        // Check 5: More than min_words = likely natural language
        words.len() >= self.min_words_for_nl
    }

    /// Check if input starts with an explicit natural language trigger
    /// Examples: "nl:", "?", "ask:"
    #[inline]
    pub fn has_explicit_trigger(&self, line: &str) -> bool {
        let line = line.trim();
        line.starts_with("nl:") || line.starts_with("? ") || line.starts_with("ask:")
    }

    /// Strip explicit trigger prefix from input
    #[inline]
    pub fn strip_trigger<'a>(&self, line: &'a str) -> &'a str {
        let line = line.trim();
        if line.starts_with("nl:") {
            line[3..].trim()
        } else if line.starts_with("? ") {
            line[2..].trim()
        } else if line.starts_with("ask:") {
            line[4..].trim()
        } else {
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_language_detection() {
        let detector = NLDetector::new();

        // Natural language examples
        assert!(detector.is_natural_language("show me all files"));
        assert!(detector.is_natural_language("how do I list files?"));
        assert!(detector.is_natural_language("find all rust files in this project"));
        assert!(detector.is_natural_language("what is my current directory"));
        assert!(detector.is_natural_language("list all processes using more than 1GB"));
        assert!(detector.is_natural_language("search for todos in the codebase"));
        assert!(detector.is_natural_language("display the last 10 commits"));

        // Shell commands (should NOT be detected as NL)
        assert!(!detector.is_natural_language("ls -la"));
        assert!(!detector.is_natural_language("git status"));
        assert!(!detector.is_natural_language("cd /tmp"));
        assert!(!detector.is_natural_language("grep pattern file.txt"));
        assert!(!detector.is_natural_language("cat file.txt"));
        assert!(!detector.is_natural_language("npm install"));
        assert!(!detector.is_natural_language("cargo build --release"));

        // Edge cases
        assert!(!detector.is_natural_language(""));
        assert!(!detector.is_natural_language("ls"));
        assert!(!detector.is_natural_language("pwd"));
    }

    #[test]
    fn test_question_mark_detection() {
        let detector = NLDetector::new();
        
        assert!(detector.is_natural_language("what is this?"));
        assert!(detector.is_natural_language("how?"));
    }

    #[test]
    fn test_question_words() {
        let detector = NLDetector::new();
        
        assert!(detector.is_natural_language("show me the files"));
        assert!(detector.is_natural_language("list all directories"));
        assert!(detector.is_natural_language("find .rs files"));
        assert!(detector.is_natural_language("search for pattern"));
    }

    #[test]
    fn test_article_detection() {
        let detector = NLDetector::new();
        
        assert!(detector.is_natural_language("display all the logs"));
        assert!(detector.is_natural_language("count every line in files"));
        assert!(detector.is_natural_language("show this directory contents"));
    }

    #[test]
    fn test_explicit_triggers() {
        let detector = NLDetector::new();
        
        assert!(detector.has_explicit_trigger("nl: list files"));
        assert!(detector.has_explicit_trigger("? show status"));
        assert!(detector.has_explicit_trigger("ask: what is this"));
        assert!(!detector.has_explicit_trigger("regular command"));
    }

    #[test]
    fn test_strip_trigger() {
        let detector = NLDetector::new();
        
        assert_eq!(detector.strip_trigger("nl: list files"), "list files");
        assert_eq!(detector.strip_trigger("? show status"), "show status");
        assert_eq!(detector.strip_trigger("ask: help me"), "help me");
        assert_eq!(detector.strip_trigger("no trigger here"), "no trigger here");
    }

    #[test]
    fn test_command_rejection() {
        let detector = NLDetector::new();
        
        // Even with many words, if it starts with a known command, reject it
        assert!(!detector.is_natural_language("git log --since='1 week ago' --oneline"));
        assert!(!detector.is_natural_language("find . -name '*.rs' -type f"));
    }
}
