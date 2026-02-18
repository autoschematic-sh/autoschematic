use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "ron/ron.pest"]
pub struct RonParser;
