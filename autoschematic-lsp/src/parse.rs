use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "ron.pest"]
struct RonParser;
