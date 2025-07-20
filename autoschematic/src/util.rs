use crossterm::style::Stylize;
use regex::Regex;

pub fn colour_op_message(message: &str) -> String {
    let re = Regex::new(r"(Deleted|deleted|DELETED|Delete|delete|DELETE|Destroy|destroy|DESTROY|DROPPED|DROP)").unwrap();

    let message = re.replace_all(message, |captures: &regex::Captures| match &captures[0] {
        s => s.red().bold().underline(crossterm::style::Color::DarkGrey).to_string(),
    });

    let re = Regex::new(r"(Created|created|CREATED|Create|create|CREATE)").unwrap();

    re.replace_all(&message, |captures: &regex::Captures| match &captures[0] {
        s => s.green().bold().to_string(),
    })
    .into()
}

/// Look for a fenced ```diff … ``` block in `message`.
/// If found, colourise each added/removed line
pub fn try_colour_op_message_diff(message: &str) -> Option<String> {
    // (?s) → dot matches new-lines
    // (?m) → ^ / $ are line anchors
    let diff_re = Regex::new(r"(?sm)```diff\n(.*?)\n```").unwrap();

    if !diff_re.is_match(message) {
        return None;
    }

    let out = diff_re
        .replace_all(message, |caps: &regex::Captures| {
            let diff_body = &caps[1]; // text between the fences
            diff_body
                .lines()
                .map(|line| {
                    if line.starts_with('+') {
                        line.grey()
                            .on(crossterm::style::Color::Rgb { r: 38, g: 102, b: 33 })
                            .to_string() // green background
                    } else if line.starts_with('-') {
                        line.grey()
                            .on(crossterm::style::Color::Rgb { r: 145, g: 34, b: 17 })
                            .to_string() // red background
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .to_string();

    Some(out)
}
