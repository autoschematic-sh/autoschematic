use std::{
    fmt::Display,
    io::IsTerminal,
    sync::atomic::{AtomicBool, Ordering},
};

use crossterm::style::StyledContent;

static PLAIN: AtomicBool = AtomicBool::new(false);

pub fn default_plain() -> bool {
    !std::io::stdout().is_terminal()
        || !std::io::stderr().is_terminal()
        || std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
        || std::env::var_os("TERM").is_some_and(|value| value == "dumb")
}

pub fn configure(plain: bool) {
    PLAIN.store(plain, Ordering::Relaxed);
    if plain {
        colored::control::set_override(false);
    }
}

pub fn plain() -> bool {
    PLAIN.load(Ordering::Relaxed)
}

pub fn choose<'text>(fancy: &'text str, plain_text: &'text str) -> &'text str {
    if plain() { plain_text } else { fancy }
}

pub fn styled(content: StyledContent<impl Display>) -> String {
    render_style(content, plain())
}

fn render_style(content: StyledContent<impl Display>, plain: bool) -> String {
    if plain {
        content.content().to_string()
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::Stylize;

    #[test]
    fn plain_removes_attributes_as_well_as_colors() {
        assert_eq!(render_style("Delete café".red().bold().underlined(), true), "Delete café");
    }
}
