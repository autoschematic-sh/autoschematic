use anyhow::bail;

pub fn reindent_line(depth: usize, line: &str) -> String {
    let mut line = line.trim().to_string();

    for _ in 0..depth {
        line.insert_str(0, "    ");
    }

    line
}

pub struct IndentState {
    pub indent: isize,
    pub inside_string: bool,
}

pub struct DiffIndentState {
    pub indent: isize,
    pub inside_string: bool,
}

pub fn count_net_indent_shift(line: &str, inside_string: bool) -> DiffIndentState {
    let mut net_indent = 0;
    let mut inside_string = inside_string;

    for c in line.chars() {
        if !inside_string {
            if matches!(c, '{' | '[' | '(') {
                net_indent += 1;
            }

            if matches!(c, '}' | ']' | ')') {
                net_indent -= 1;
            }

            if c == '\"' {
                inside_string = true;
            }
        } else if c == '\"' {
            inside_string = false;
        }
    }

    DiffIndentState {
        indent: net_indent,
        inside_string,
    }
}

pub fn reindent(src: &str) -> anyhow::Result<String> {
    let mut res = String::new();
    let mut indent_state = IndentState {
        indent: 0,
        inside_string: false,
    };

    for line in src.lines() {
        let diff_indent = count_net_indent_shift(line, indent_state.inside_string);
        // eprintln!("line {}", line);
        // eprintln!("net indent {}", diff_indent.indent);

        if diff_indent.indent < 0 {
            res.push_str(&reindent_line(
                usize::try_from(indent_state.indent + diff_indent.indent).unwrap_or(0),
                line,
            ));
        } else {
            res.push_str(&reindent_line(usize::try_from(indent_state.indent).unwrap_or(0), line));
        }

        indent_state.indent += diff_indent.indent;

        res.push('\n');
    }

    Ok(res)
}

pub fn reindent_old(src: &str) -> anyhow::Result<String> {
    let mut out = String::with_capacity(src.len());
    let mut indent = 0usize;
    let mut chars = src.chars().peekable();

    // helper lambdas
    let newline = |out: &mut String, indent: usize| {
        out.push('\n');
        for _ in 0..indent {
            out.push_str("    "); // 4 spaces
        }
    };

    // Whether we’re in some region where braces shouldn't affect indent
    enum Mode {
        Code,
        Str { delim: String }, // delim = closing sequence we’re waiting for
        LineComment,
        BlockComment(usize), // nesting depth
    }

    let mut mode = Mode::Code;

    while let Some(c) = chars.next() {
        match mode {
            //------------------------------------------------------------
            // ─── CODE ──────────────────────────────────────────────────
            Mode::Code => match c {
                // 1) enter comments
                '/' if chars.peek() == Some(&'/') => {
                    out.push_str("//");
                    chars.next();
                    mode = Mode::LineComment;
                }
                '/' if chars.peek() == Some(&'*') => {
                    out.push_str("/*");
                    chars.next();
                    mode = Mode::BlockComment(1);
                }

                // 2) enter strings (we detect both raw and normal)
                'r' if matches!(chars.peek(), Some('#' | '"')) => {
                    // raw string, count hashes
                    let mut delim = String::from("\"");
                    while let Some('#') = chars.peek() {
                        delim.push('#');
                        out.push('#');
                        chars.next();
                    }
                    if chars.next() != Some('"') {
                        bail!("lexer bug: expected opening quote after raw string r###");
                    }
                    out.push('r');
                    out.push('"');
                    mode = Mode::Str { delim };
                }
                'b' if chars.peek() == Some(&'"') => {
                    out.push_str("b\"");
                    chars.next();
                    mode = Mode::Str { delim: "\"".into() };
                }
                '"' => {
                    out.push('"');
                    mode = Mode::Str { delim: "\"".into() };
                }
                '\'' => {
                    out.push('\'');
                    mode = Mode::Str { delim: "'".into() };
                }

                // 3) structural punctuation
                '[' | '{' | '(' => {
                    out.push(c);
                    indent += 1;
                    newline(&mut out, indent);
                }
                ']' | '}' | ')' => {
                    indent = indent.saturating_sub(1);
                    newline(&mut out, indent);
                    out.push(c);
                }
                ',' => {
                    out.push(',');
                    if indent > 0 {
                        newline(&mut out, indent);
                    } else {
                        out.push(' ');
                    }
                }
                // default: just copy
                _ => out.push(c),
            },

            //------------------------------------------------------------
            // ─── STRING ────────────────────────────────────────────────
            Mode::Str { ref delim } => {
                out.push(c);
                // detect escapes in normal strings so \" doesn't terminate
                if (delim == "\"" || delim == "'") && c == '\\' {
                    if let Some(escaped) = chars.next() {
                        out.push(escaped);
                    }
                    continue;
                }
                // check if we hit the closing delimiter
                if delim.starts_with(c)
                    && delim.chars().all(|d| {
                        // look-ahead to match the rest
                        let it = delim.chars().skip(1);
                        let mut ok = true;
                        let mut look = chars.clone();
                        for dc in it {
                            if look.next() != Some(dc) {
                                ok = false;
                                break;
                            }
                        }
                        ok
                    })
                {
                    // consume the rest of the delimiter
                    for _ in 1..delim.len() {
                        out.push(chars.next().unwrap());
                    }
                    mode = Mode::Code;
                }
            }

            //------------------------------------------------------------
            // ─── COMMENTs ──────────────────────────────────────────────
            Mode::LineComment => {
                out.push(c);
                if c == '\n' {
                    // newline(&mut out, indent);
                    mode = Mode::Code;
                }
            }
            Mode::BlockComment(ref mut depth) => {
                out.push(c);
                match c {
                    '/' if chars.peek() == Some(&'*') => {
                        out.push('*');
                        chars.next();
                        *depth += 1;
                    }
                    '*' if chars.peek() == Some(&'/') => {
                        out.push('/');
                        chars.next();
                        *depth -= 1;
                        if *depth == 0 {
                            mode = Mode::Code;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(out)
}
