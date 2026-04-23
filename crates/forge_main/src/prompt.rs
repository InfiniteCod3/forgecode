use std::borrow::Cow;
use std::fmt::Write;
use std::path::PathBuf;

use convert_case::{Case, Casing};
use derive_setters::Setters;
use forge_api::{AgentId, ModelId, Usage};
use forge_domain::Effort;
use nu_ansi_term::{Color, Style};
use reedline::{Prompt, PromptHistorySearchStatus};

use crate::display_constants::markers;
use crate::utils::humanize_number;

// Constants
const MULTILINE_INDICATOR: &str = "::: ";

// Nerd font symbols — left prompt
const DIR_SYMBOL: &str = "\u{ea83}"; // 󪃃  folder icon
const BRANCH_SYMBOL: &str = "\u{f418}"; //   branch icon
const SUCCESS_SYMBOL: &str = "\u{f013e}"; // 󰄾  chevron

// Nerd font symbols — right prompt (ZSH rprompt)
const AGENT_SYMBOL: &str = "\u{f167a}";
const MODEL_SYMBOL: &str = "\u{ec19}";

/// Very Specialized Prompt for the Agent Chat
#[derive(Clone, Setters)]
#[setters(strip_option, borrow_self)]
pub struct ForgePrompt {
    pub cwd: PathBuf,
    pub usage: Option<Usage>,
    pub prev_usage: Option<Usage>,
    pub agent_id: AgentId,
    pub model: Option<ModelId>,
    pub git_branch: Option<String>,
    pub reasoning_effort: Option<Effort>,
    pub context_length: Option<u64>,
}

impl ForgePrompt {
    /// Creates a new `ForgePrompt`, resolving the git branch once at
    /// construction time.
    pub fn new(cwd: PathBuf, agent_id: AgentId) -> Self {
        let git_branch = get_git_branch();
        Self { cwd, usage: None, prev_usage: None, agent_id, model: None, git_branch, reasoning_effort: None, context_length: None }
    }

    pub fn refresh(&mut self) -> &mut Self {
        let git_branch = get_git_branch();
        self.git_branch = git_branch;
        self
    }

    /// Computes a human-readable token delta indicator string.
    ///
    /// Compares current usage to previous usage and returns a formatted
    /// delta string like "▲1.2k" (increase) or "▼800" (decrease).
    /// Returns an empty string if there's no previous usage to compare against.
    fn compute_token_delta(&self) -> String {
        match (self.usage.as_ref(), self.prev_usage.as_ref()) {
            (Some(current), Some(prev)) => {
                let current_total: usize = *current.total_tokens;
                let prev_total: usize = *prev.total_tokens;
                if prev_total == 0 {
                    return String::new();
                }
                let delta = current_total as i64 - prev_total as i64;
                if delta == 0 {
                    return String::new();
                }
                if delta > 0 {
                    format!("▲{}", humanize_number(delta as usize))
                } else {
                    format!("▼{}", humanize_number((-delta) as usize))
                }
            }
            _ => String::new(),
        }
    }

    /// Returns a color based on context window utilization.
    ///
    /// - Green when utilization < 50%
    /// - Yellow when utilization is 50-75%
    /// - LightRed when utilization is 75-90%
    /// - Red when utilization > 90%
    /// Falls back to LightGray when context length is unknown.
    fn context_utilization_color(&self, total_tokens: usize) -> Color {
        match self.context_length {
            Some(limit) if limit > 0 => {
                let utilization = total_tokens as f64 / limit as f64;
                if utilization > 0.90 {
                    Color::Red
                } else if utilization > 0.75 {
                    Color::LightRed
                } else if utilization > 0.50 {
                    Color::Yellow
                } else {
                    Color::Green
                }
            }
            _ => Color::LightGray,
        }
    }
}

impl Prompt for ForgePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        // Left prompt layout:
        //
        //   AGENT_NAME  󪃃 dir   branch
        //   󰄾
        //
        // Colors:
        //   agent  → bold white  (identifies the active agent)
        //   dir    → bold cyan
        //   branch → bold green
        //   chevron → bold green

        let dir_style = Style::new().fg(Color::Cyan).bold();
        let branch_style = Style::new().fg(Color::LightGreen).bold();
        let chevron_style = Style::new().fg(Color::LightGreen).bold();

        let current_dir = self
            .cwd
            .file_name()
            .and_then(|name| name.to_str())
            .map(String::from)
            .unwrap_or_else(|| markers::EMPTY.to_string());

        let mut result = String::with_capacity(80);

        // Directory — folder icon + name, bold cyan
        write!(
            result,
            "{}",
            dir_style.paint(format!("{DIR_SYMBOL} {current_dir}"))
        )
        .unwrap();

        // Git branch — branch icon + name, bold green (only when present and
        // different from the directory name, matching existing behaviour)
        if let Some(branch) = self.git_branch.as_deref()
            && branch != current_dir
        {
            write!(
                result,
                " {}",
                branch_style.paint(format!("{BRANCH_SYMBOL} {branch}"))
            )
            .unwrap();
        }

        // Second line: success chevron
        write!(result, "\n{} ", chevron_style.paint(SUCCESS_SYMBOL)).unwrap();

        Cow::Owned(result)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        // Right prompt layout: agent · tokens [±delta] · cost · model · effort
        // Active (tokens > 0): bright white for agent/tokens, green for cost
        // Inactive (no tokens): all segments dimmed

        let total_tokens = self.usage.as_ref().map(|u| u.total_tokens);
        let active = total_tokens.map(|t| *t > 0).unwrap_or(false);

        let agent_color = if active {
            Color::LightGray
        } else {
            Color::DarkGray
        };
        let mut result = String::with_capacity(128);

        // Agent name with nerd font symbol
        let agent_str = format!(
            "{AGENT_SYMBOL} {}",
            self.agent_id.as_str().to_case(Case::UpperSnake)
        );
        write!(
            result,
            " {}",
            Style::new().bold().fg(agent_color).paint(&agent_str)
        )
        .unwrap();

        // Token count with delta indicator (only shown when active)
            // Token count with delta indicator (only shown when active)
        if let Some(tokens) = total_tokens
            && active
        {
            let prefix = match tokens {
                forge_api::TokenCount::Actual(_) => "",
                forge_api::TokenCount::Approx(_) => "~",
            };
            let count_str = format!("{}{}", prefix, humanize_number(*tokens));

            // Compute token delta from previous usage
            let delta_str = self.compute_token_delta();

            let token_display = if delta_str.is_empty() {
                count_str
            } else {
                format!("{} {}", count_str, delta_str)
            };

            // Color-code token count based on context window utilization
            let token_color = self.context_utilization_color(*tokens);

            write!(
                result,
                " {}",
                Style::new().bold().fg(token_color).paint(&token_display)
            )
            .unwrap();
        }
        // Cost (only shown when active)
        if let Some(cost) = self.usage.as_ref().and_then(|u| u.cost)
            && active
        {
            let cost_str = format!("\u{f155}{cost:.2}");
            write!(
                result,
                " {}",
                Style::new().bold().fg(Color::Green).paint(&cost_str)
            )
            .unwrap();
        }

        // Model with nerd font symbol
        if let Some(model) = self.model.as_ref() {
            let model_str = model.to_string();
            let short_model = model_str.split('/').next_back().unwrap_or(model.as_str());
            let model_label = format!("{MODEL_SYMBOL} {short_model}");
            let color = if active {
                Color::LightMagenta
            } else {
                Color::DarkGray
            };
            write!(result, " {}", Style::new().fg(color).paint(&model_label)).unwrap();
        }

        // Reasoning effort indicator (shown when active)
        if let Some(effort) = self.reasoning_effort.as_ref()
            && active
            && !matches!(effort, Effort::None)
        {
            let effort_label = effort.to_string().to_uppercase();
            let color = match effort {
                Effort::XHigh => Color::Red,
                Effort::High => Color::LightRed,
                Effort::Medium => Color::Yellow,
                Effort::Low => Color::Green,
                Effort::Minimal => Color::Cyan,
                Effort::None => Color::DarkGray,
                Effort::Max => Color::Magenta,
            };
            write!(result, " {}", Style::new().bold().fg(color).paint(&effort_label)).unwrap();
        }

        Cow::Owned(result)
    }

    fn render_prompt_indicator(&self, _prompt_mode: reedline::PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(MULTILINE_INDICATOR)
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: reedline::PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };

        let mut result = String::with_capacity(32);

        // Handle empty search term more elegantly
        if history_search.term.is_empty() {
            write!(result, "({prefix}reverse-search) ").unwrap();
        } else {
            write!(
                result,
                "({}reverse-search: {}) ",
                prefix, history_search.term
            )
            .unwrap();
        }

        Cow::Owned(Style::new().fg(Color::White).paint(&result).to_string())
    }
}

/// Gets the current git branch name if available
fn get_git_branch() -> Option<String> {
    let repo = gix::discover(".").ok()?;
    let head = repo.head().ok()?;
    head.referent_name().map(|r| r.shorten().to_string())
}

#[cfg(test)]
mod tests {
    use nu_ansi_term::Style;
    use pretty_assertions::assert_eq;

    use super::*;

    impl Default for ForgePrompt {
        fn default() -> Self {
            ForgePrompt {
                cwd: PathBuf::from("."),
                usage: None,
                prev_usage: None,
                agent_id: AgentId::default(),
                model: None,
                git_branch: None,
                reasoning_effort: None,
                context_length: None,
            }
        }
    }

    #[test]
    fn test_render_prompt_left() {
        let prompt = ForgePrompt::default();
        let actual = prompt.render_prompt_left();

        // Starship directory icon present
        assert!(actual.contains(DIR_SYMBOL));
        // Starship success chevron present
        assert!(actual.contains(SUCCESS_SYMBOL));
    }

    #[test]
    fn test_render_prompt_left_with_branch() {
        let prompt = ForgePrompt { git_branch: Some("main".to_string()), ..Default::default() };
        let actual = prompt.render_prompt_left();

        // Agent name is on the right prompt, not the left
        // Branch icon and name present
        assert!(actual.contains(BRANCH_SYMBOL));
        assert!(actual.contains("main"));
    }

    #[test]
    fn test_render_prompt_right_inactive() {
        // No tokens → dimmed agent + model, no token/cost segments
        let mut prompt = ForgePrompt::default();
        let _ = prompt.model(ModelId::new("gpt-4"));

        let actual = prompt.render_prompt_right();
        // Agent symbol and name present
        assert!(actual.contains(AGENT_SYMBOL));
        assert!(actual.contains("FORGE"));
        // Model symbol and name present
        assert!(actual.contains(MODEL_SYMBOL));
        assert!(actual.contains("gpt-4"));
        // No token count text in inactive state (no humanized number segment)
        assert!(!actual.contains("1k") && !actual.contains("~"));
    }

    #[test]
    fn test_render_prompt_right_active_with_tokens() {
        // Tokens > 0 → active colours; approx tokens show "~" prefix
        let usage = Usage {
            prompt_tokens: forge_api::TokenCount::Actual(10),
            completion_tokens: forge_api::TokenCount::Actual(20),
            total_tokens: forge_api::TokenCount::Approx(30),
            ..Default::default()
        };
        let mut prompt = ForgePrompt::default();
        let _ = prompt.usage(usage);

        let actual = prompt.render_prompt_right();
        assert!(actual.contains("~30"));
        assert!(actual.contains(AGENT_SYMBOL));
    }

    #[test]
    fn test_render_prompt_multiline_indicator() {
        let prompt = ForgePrompt::default();
        let actual = prompt.render_prompt_multiline_indicator();
        let expected = MULTILINE_INDICATOR;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_render_prompt_history_search_indicator_passing() {
        let prompt = ForgePrompt::default();
        let history_search = reedline::PromptHistorySearch {
            status: PromptHistorySearchStatus::Passing,
            term: "test".to_string(),
        };
        let actual = prompt.render_prompt_history_search_indicator(history_search);
        let expected = Style::new()
            .fg(Color::White)
            .paint("(reverse-search: test) ")
            .to_string();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_render_prompt_history_search_indicator_failing() {
        let prompt = ForgePrompt::default();
        let history_search = reedline::PromptHistorySearch {
            status: PromptHistorySearchStatus::Failing,
            term: "test".to_string(),
        };
        let actual = prompt.render_prompt_history_search_indicator(history_search);
        let expected = Style::new()
            .fg(Color::White)
            .paint("(failing reverse-search: test) ")
            .to_string();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_render_prompt_history_search_indicator_empty_term() {
        let prompt = ForgePrompt::default();
        let history_search = reedline::PromptHistorySearch {
            status: PromptHistorySearchStatus::Passing,
            term: "".to_string(),
        };
        let actual = prompt.render_prompt_history_search_indicator(history_search);
        let expected = Style::new()
            .fg(Color::White)
            .paint("(reverse-search) ")
            .to_string();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_render_prompt_right_strips_provider_prefix() {
        // Model ID like "anthropic/claude-3" should show only "claude-3"
        let usage = Usage {
            prompt_tokens: forge_api::TokenCount::Actual(10),
            completion_tokens: forge_api::TokenCount::Actual(20),
            total_tokens: forge_api::TokenCount::Actual(30),
            ..Default::default()
        };
        let mut prompt = ForgePrompt::default();
        let _ = prompt.usage(usage);
        let _ = prompt.model(ModelId::new("anthropic/claude-3"));

        let actual = prompt.render_prompt_right();
        assert!(actual.contains("claude-3"));
        assert!(!actual.contains("anthropic/claude-3"));
        assert!(actual.contains("30"));
    }

    #[test]
    fn test_render_prompt_right_with_cost() {
        // Cost shown when active
        let usage = Usage {
            total_tokens: forge_api::TokenCount::Actual(1500),
            cost: Some(0.01),
            ..Default::default()
        };
        let mut prompt = ForgePrompt::default();
        let _ = prompt.usage(usage);

        let actual = prompt.render_prompt_right();
        assert!(actual.contains("0.01"));
        assert!(actual.contains("1.5k"));
    }
}
