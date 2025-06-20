use colored::Colorize;
use regex::Regex;

pub fn colour_op_message(message: &str) -> String {
    let re = Regex::new(r"(Delete|delete|DELETE|Destroy|destroy|DESTROY)").unwrap();

    let message = re.replace_all(message, |captures: &regex::Captures| match &captures[0] {
        s => s.red().bold().underline().to_string(),
    });

    let re = Regex::new(r"(Create|create|CREATE)").unwrap();

    let message = re
        .replace_all(&message, |captures: &regex::Captures| match &captures[0] {
            s => s.green().bold().to_string(),
        })
        .into();

    message
}
