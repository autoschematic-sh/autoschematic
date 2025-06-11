use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "ron.pest"]
pub struct RonParser;
