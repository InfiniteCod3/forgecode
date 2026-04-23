use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use colored::Colorize;
use forge_domain::ConsoleWriter;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};

mod progress_bar;

pub use progress_bar::*;

const TICK_DURATION_MS: u64 = 60;
const TICKS: &[&str; 10] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Formats elapsed time into a compact string representation.
///
/// # Arguments
///
/// * `duration` - The elapsed time duration
///
/// # Returns
///
/// A formatted string:
/// - Less than 1 minute: "01s", "02s", etc.
/// - Less than 1 hour: "1:01m", "1:59m", etc.
/// - 1 hour or more: "1:01h", "2:30h", etc.
fn format_elapsed_time(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    if total_seconds < 60 {
        format!("{:02}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}:{:02}m", minutes, seconds)
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}:{:02}h", hours, minutes)
    }
}

/// Manages spinner functionality for the UI.
///
/// Uses indicatif's built-in `{elapsed}` template for time display,
/// eliminating the need for a background task to update the message.
/// Accumulated time is preserved across start/stop cycles using
/// `with_elapsed()`. Spinner tick position is also preserved to maintain
/// smooth animation continuity.
pub struct SpinnerManager<P: ConsoleWriter> {
    spinner: Option<ProgressBar>,
    accumulated_elapsed: Duration,
    phase: SpinnerPhase,
    tool_count: usize,
    message: Option<String>,
    printer: Arc<P>,
}

/// Phase-aware spinner message selection.
///
/// Instead of random words, the spinner shows contextually appropriate
/// messages based on the current phase of the conversation cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerPhase {
    /// Initial startup / connecting to provider
    Connecting,
    /// Agent is thinking / reasoning
    Thinking,
    /// Agent is executing a tool
    ToolExecution,
}

impl SpinnerPhase {
    /// Returns a phase-appropriate word for the spinner.
    fn word(&self) -> &'static str {
        match self {
            SpinnerPhase::Connecting => "Connecting",
            SpinnerPhase::Thinking => "Thinking",
            SpinnerPhase::ToolExecution => "Executing",
        }
    }
}

impl<P: ConsoleWriter> SpinnerManager<P> {
    /// Creates a new SpinnerManager with the given output printer.
    pub fn new(printer: Arc<P>) -> Self {
        Self {
            spinner: None,
            accumulated_elapsed: Duration::ZERO,
            phase: SpinnerPhase::Thinking,
            tool_count: 0,
            message: None,
            printer,
        }
    }

    /// Start the spinner with a message
    pub fn start(&mut self, message: Option<&str>) -> Result<()> {
        self.stop(None)?;

        let word = match message {
            Some(msg) => msg.to_string(),
            None => {
                let phase_word = self.phase.word().to_string();
                if self.tool_count > 0 && self.phase == SpinnerPhase::ToolExecution {
                    format!("{} ({}/{})", phase_word, self.tool_count, self.tool_count)
                } else {
                    phase_word
                }
            }
        };

        self.message = Some(word.clone());

        // Create the spinner with accumulated elapsed time
        // Use custom elapsed formatter for "01s", "1:01m", "1:01h" format
        let pb = ProgressBar::new_spinner()
            .with_elapsed(self.accumulated_elapsed)
            .with_style(
                ProgressStyle::default_spinner()
                    .tick_strings(TICKS)
                    .template("{spinner:.green} {msg} {elapsed_custom:.white} {prefix:.white.dim}")
                    .unwrap()
                    .with_key(
                        "elapsed_custom",
                        |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                            let _ = write!(w, "{}", format_elapsed_time(state.elapsed()));
                        },
                    ),
            )
            .with_message(word.green().bold().to_string())
            .with_prefix("· Ctrl+C to interrupt");

        // Preserve spinner tick position for visual continuity
        // The spinner has 10 tick positions cycling every 600ms (60ms per tick)
        let tick_count: usize = TICKS.len();
        let elapsed_ms = self.accumulated_elapsed.as_millis() as u64;
        let cycle_ms = TICK_DURATION_MS * tick_count as u64;
        let ticks_to_advance = (elapsed_ms % cycle_ms) / TICK_DURATION_MS;

        // Advance to the correct tick position
        (0..ticks_to_advance).for_each(|_| pb.tick());

        pb.enable_steady_tick(Duration::from_millis(TICK_DURATION_MS));

        self.spinner = Some(pb);

        Ok(())
    }

    /// Stop the active spinner if any
    pub fn stop(&mut self, message: Option<String>) -> Result<()> {
        if let Some(spinner) = self.spinner.take() {
            // Capture elapsed time before finishing
            self.accumulated_elapsed = spinner.elapsed();
            spinner.finish_and_clear();
            if let Some(msg) = message {
                self.println(&msg);
            }
        } else if let Some(message) = message {
            self.println(&message);
        }

        self.message = None;

        Ok(())
    }

    /// Updates the spinner's displayed message.
    pub fn set_message(&mut self, message: &str) -> Result<()> {
        self.message = Some(message.to_owned());
        if let Some(spinner) = &self.spinner {
            spinner.set_message(message.green().bold().to_string());
        }
        Ok(())
    }

    /// Sets the current spinner phase for context-aware messages.
    pub fn set_phase(&mut self, phase: SpinnerPhase) {
        self.phase = phase;
    }

    /// Increments the tool counter and updates the spinner if active.
    pub fn increment_tool_count(&mut self) -> Result<()> {
        self.tool_count += 1;
        // If spinner is active with no custom message, update to show tool count
        if self.spinner.is_some() && self.message.is_none() {
            let msg = format!(
                "{} ({}/{})",
                self.phase.word(),
                self.tool_count,
                self.tool_count
            );
            self.set_message(&msg)?;
        }
        Ok(())
    }

    /// Resets the elapsed time and phase to zero/defaults.
    /// Call this when starting a completely new task/conversation.
    pub fn reset(&mut self) {
        self.accumulated_elapsed = Duration::ZERO;
        self.phase = SpinnerPhase::Thinking;
        self.tool_count = 0;
        self.message = None;
    }

    /// Writes a line to stdout, suspending the spinner if active.
    pub fn write_ln(&mut self, message: impl ToString) -> Result<()> {
        let msg = message.to_string();
        if let Some(spinner) = &self.spinner {
            spinner.suspend(|| self.println(&msg));
        } else {
            self.println(&msg);
        }
        Ok(())
    }

    /// Writes a line to stderr, suspending the spinner if active.
    pub fn ewrite_ln(&mut self, message: impl ToString) -> Result<()> {
        let msg = message.to_string();
        if let Some(spinner) = &self.spinner {
            spinner.suspend(|| self.eprintln(&msg));
        } else {
            self.eprintln(&msg);
        }
        Ok(())
    }

    /// Prints a line to stdout through the printer.
    fn println(&self, msg: &str) {
        let line = format!("{msg}\n");
        let _ = self.printer.write(line.as_bytes());
        let _ = self.printer.flush();
    }

    /// Prints a line to stderr through the printer.
    fn eprintln(&self, msg: &str) {
        let line = format!("{msg}\n");
        let _ = self.printer.write_err(line.as_bytes());
        let _ = self.printer.flush_err();
    }
}

impl<P: ConsoleWriter> Drop for SpinnerManager<P> {
    fn drop(&mut self) {
        // Stop spinner before flushing to ensure finish_and_clear() is called
        // This prevents the spinner from leaving the cursor at column 0 without a
        // newline
        let _ = self.stop(None);
        // Flush both stdout and stderr to ensure all output is visible
        // This prevents race conditions with shell prompt resets
        let _ = self.printer.flush();
        let _ = self.printer.flush_err();
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;
    use std::time::Duration;

    use forge_domain::ConsoleWriter;
    use pretty_assertions::assert_eq;

    use super::{SpinnerManager, format_elapsed_time};

    /// A simple printer that writes directly to stdout/stderr.
    /// Used for testing when synchronized output is not needed.
    #[derive(Clone, Copy)]
    struct DirectPrinter;

    impl ConsoleWriter for DirectPrinter {
        fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
            std::io::stdout().write(buf)
        }

        fn write_err(&self, buf: &[u8]) -> std::io::Result<usize> {
            std::io::stderr().write(buf)
        }

        fn flush(&self) -> std::io::Result<()> {
            std::io::stdout().flush()
        }

        fn flush_err(&self) -> std::io::Result<()> {
            std::io::stderr().flush()
        }
    }

    fn fixture_spinner() -> SpinnerManager<DirectPrinter> {
        SpinnerManager::new(Arc::new(DirectPrinter))
    }

    #[test]
    fn test_spinner_reset_clears_accumulated_time() {
        let mut fixture_spinner = fixture_spinner();

        // Simulate some accumulated time
        fixture_spinner.accumulated_elapsed = std::time::Duration::from_secs(100);

        // Reset should clear accumulated time
        fixture_spinner.reset();

        let actual = fixture_spinner.accumulated_elapsed;
        let expected = std::time::Duration::ZERO;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_spinner_reset_clears_phase() {
        let mut fixture_spinner = fixture_spinner();

        // Set a phase
        fixture_spinner.phase = super::SpinnerPhase::ToolExecution;

        // Reset should clear it
        fixture_spinner.reset();

        let actual = fixture_spinner.phase;
        let expected = super::SpinnerPhase::Thinking;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_spinner_reset_clears_tool_count() {
        let mut fixture_spinner = fixture_spinner();

        // Set a tool count
        fixture_spinner.tool_count = 5;

        // Reset should clear it
        fixture_spinner.reset();

        let actual = fixture_spinner.tool_count;
        let expected = 0;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_spinner_reset_clears_message() {
        let mut fixture_spinner = fixture_spinner();

        // Set a message
        fixture_spinner.message = Some("Test".to_string());

        // Reset should clear it
        fixture_spinner.reset();

        let actual = fixture_spinner.message.clone();
        let expected = None;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_phase_is_preserved_across_starts() {
        let mut fixture_spinner = fixture_spinner();

        fixture_spinner.phase = super::SpinnerPhase::ToolExecution;
        fixture_spinner.start(None).unwrap();
        let first_phase = fixture_spinner.phase;
        fixture_spinner.stop(None).unwrap();

        fixture_spinner.start(None).unwrap();
        let second_phase = fixture_spinner.phase;
        fixture_spinner.stop(None).unwrap();

        // Phase should be identical because it's preserved
        assert_eq!(first_phase, second_phase);
    }

    #[test]
    fn test_format_elapsed_time_seconds_only() {
        let actual = format_elapsed_time(Duration::from_secs(5));
        let expected = "05s";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(59));
        let expected = "59s";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_elapsed_time_minutes_and_seconds() {
        let actual = format_elapsed_time(Duration::from_secs(60));
        let expected = "1:00m";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(125));
        let expected = "2:05m";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(3599));
        let expected = "59:59m";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_elapsed_time_hours_and_minutes() {
        let actual = format_elapsed_time(Duration::from_secs(3600));
        let expected = "1:00h";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(3661));
        let expected = "1:01h";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(7200));
        let expected = "2:00h";
        assert_eq!(actual, expected);

        let actual = format_elapsed_time(Duration::from_secs(9000));
        let expected = "2:30h";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_elapsed_time_zero() {
        let actual = format_elapsed_time(Duration::ZERO);
        let expected = "00s";
        assert_eq!(actual, expected);
    }
}
