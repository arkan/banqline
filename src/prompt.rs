// Interactive prompt support using ratatui.

/// Constants for special category selection choices.
#[allow(dead_code)]
pub const CHOICE_NEW_CATEGORY: &str = "__new__";
#[allow(dead_code)]
pub const CHOICE_SKIP: &str = "__skip__";

/// Prompter abstracts interactive user prompts for testability.
#[allow(dead_code)]
pub trait Prompter {
    /// Presents a list of category options and returns the user's choice.
    fn select_category(&mut self, options: &[String]) -> anyhow::Result<String>;

    /// Prompts for a new category name.
    fn input_category(&mut self) -> anyhow::Result<String>;

    /// Prompts for a pattern, pre-filled with suggested.
    fn input_pattern(&mut self, suggested: &str) -> anyhow::Result<String>;
}

/// No-op prompter for non-interactive contexts.
#[allow(dead_code)]
pub struct NoopPrompter;

impl Prompter for NoopPrompter {
    fn select_category(&mut self, _options: &[String]) -> anyhow::Result<String> {
        anyhow::bail!("interactive prompts are not available in non-TUI mode")
    }

    fn input_category(&mut self) -> anyhow::Result<String> {
        anyhow::bail!("interactive prompts are not available in non-TUI mode")
    }

    fn input_pattern(&mut self, _suggested: &str) -> anyhow::Result<String> {
        anyhow::bail!("interactive prompts are not available in non-TUI mode")
    }
}
