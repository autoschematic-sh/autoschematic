use crate::connector::DocIdent;
use anyhow::Result;
use pest::{Parser, Span, iterators::Pair};

use crate::ron::parse::{RonParser, Rule};

#[derive(Debug, Clone)]
pub enum Component {
    Name(String),
    Key(String),
    Index(usize),
}

pub fn ron_path_to_string(path: &Vec<Component>) -> String {
    let mut first = true;
    let mut res = String::new();

    for c in path {
        match c {
            Component::Name(s) => {
                // Avoid a leading "." at the start.
                if !first {
                    res.push('.');
                } else {
                    first = false;
                }

                res.push_str(&s);
            }
            Component::Key(s) => {
                res.push('[');
                res.push_str(&s);
                res.push(']');
            }
            Component::Index(n) => {
                res.push('[');
                res.push_str(&n.to_string());
                res.push(']');
            }
        }
    }

    res
}

pub fn path_at(src: &str, line: usize, col: usize) -> Result<Option<Vec<Component>>> {
    // 1) map (line, col) → byte offset
    // let file = SimpleFile::new("input", src);
    //

    // let line_start = file.line_number(id, line_index)
    // let offset = file
    //     .location_to_byte_index(line - 1, col - 1)
    //     .with_context(|| format!("line {line}, column {col} is out of bounds"))?;
    let Some(byte_index) = pos_byte_index(line, col, src) else {
        return Ok(None);
    };

    // 2) parse
    let mut pairs = RonParser::parse(Rule::ron, src)?;
    let root = pairs.next().unwrap(); // SOI … EOI

    // 3) walk the tree to the innermost node that covers `offset`
    let mut trail = Vec::<Component>::new();
    descend(root, byte_index, src, &mut trail)?;

    Ok(Some(trail))
}

/// Assuming src is a RON config file, parse it and return a list of Name1.field1.field2.key: "value"
/// entries for each string in the config file. This is used to intelligently pick out references to other templated output variables.
pub fn find_strings(src: &str) -> Result<(Option<String>, Vec<(Vec<Component>, String)>)> {
    let mut pairs = RonParser::parse(Rule::ron, src)?;
    let root = pairs.next().unwrap();

    let mut results = Vec::new();
    let mut trail = Vec::new();
    let mut root_name = None;
    descend_find_strings(root, &mut root_name, src, &mut trail, &mut results);

    Ok((root_name, results))
}

pub fn ident_at(src: &str, line: usize, col: usize) -> Result<Option<DocIdent>> {
    // 1) map (line, col) → byte offset
    // let file = SimpleFile::new("input", src);
    //

    // let line_start = file.line_number(id, line_index)
    // let offset = file
    //     .location_to_byte_index(line - 1, col - 1)
    //     .with_context(|| format!("line {line}, column {col} is out of bounds"))?;
    let Some(byte_index) = pos_byte_index(line, col, src) else {
        return Ok(None);
    };

    // 2) parse
    let mut pairs = RonParser::parse(Rule::ron, src)?;
    let root = pairs.next().unwrap(); // SOI … EOI

    // 3) walk the tree to the innermost node that covers `offset`
    let mut ident = None;
    let mut parent = None;
    descend_find_docident(root, byte_index, src, &mut parent, &mut ident)?;

    Ok(ident)
}

/// recursive descent parser to find the ident and parent, if any, at the cursor position.
fn descend_find_docident(
    pair: Pair<Rule>,
    cursor: usize,
    _src: &str,
    parent: &mut Option<DocIdent>,
    ident: &mut Option<DocIdent>,
) -> Result<bool> {
    let span = pair.as_span();
    if !covers(span, cursor) {
        // eprintln!("cursor not inside this node");
        return Ok(false);
    }

    match pair.as_rule() {
        // -- outermost “ron” rule is ignored
        Rule::ron | Rule::value => {
            for child in pair.into_inner() {
                if descend_find_docident(child, cursor, _src, parent, ident)? {
                    break;
                }
            }
        }

        // structs
        Rule::named_struct | Rule::tuple_struct | Rule::unit_struct => {
            // first inner child may be an ident
            let mut it = pair.clone().into_inner();
            // eprintln!("it: {}", it);
            if let Some(id_pair) = it.next()
                && id_pair.as_rule() == Rule::ident
            {
                if covers(id_pair.as_span(), cursor) {
                    *ident = Some(DocIdent::Struct {
                        name: id_pair.as_str().into(),
                    })
                }
                if covers(pair.as_span(), cursor) {
                    *parent = Some(DocIdent::Struct {
                        name: id_pair.as_str().into(),
                    })
                }
            }

            // if let Some(id_pair) = it.next_if(|p| p.as_rule() == Rule::ident) {
            //     if covers(id_pair.as_span(), cursor) {
            //         trail.push(Component::Name(slice(src, id_pair.as_span())));
            //     }
            // }
            // continue through the rest
            for child in it {
                if descend_find_docident(child, cursor, _src, parent, ident)? {
                    break;
                }
            }
        }

        Rule::named_field => {
            //  ident ":" value
            let mut inner = pair.clone().into_inner();
            let name = inner.next().unwrap(); // ident
            // let val = inner.next().unwrap(); // value
            // trail.push(Component::Name(slice(src, name.as_span())));
            // descend(val, cursor, src, trail)?;
            if name.as_rule() == Rule::ident {
                if covers(name.as_span(), cursor)
                    && let Some(DocIdent::Struct { name: struct_name }) = parent
                {
                    *ident = Some(DocIdent::Field {
                        parent: struct_name.to_string(),
                        name: name.as_str().into(),
                    })
                }
                // trail.push(Component::Name(slice(src, id_pair.as_span())));

                for child in inner {
                    // eprintln!("child: {}", child.as_str());
                    if descend_find_docident(child, cursor, _src, parent, ident)? {
                        // trail.push(Component::Index(idx));
                        break;
                    }
                }
            }
        }

        // lists / tuples
        Rule::list | Rule::tuple => {
            for val in pair.into_inner().filter(|p| p.as_rule() == Rule::value) {
                if descend_find_docident(val, cursor, _src, parent, ident)? {
                    // trail.push(Component::Index(idx));
                    break;
                }
            }
        }

        // maps
        Rule::map => {
            for entry in pair.into_inner() {
                // map_entry
                let mut it = entry.into_inner();
                let key = it.next().unwrap();
                let val = it.next().unwrap();
                if descend_find_docident(key.clone(), cursor, _src, parent, ident)? {
                    // Cursor is inside the key itself: report previous trail
                    break;
                }
                if descend_find_docident(val, cursor, _src, parent, ident)? {
                    // Cursor somewhere in the value: emit key string
                    // let key_str = match key.as_rule() {
                    //     Rule::string_std | Rule::string_raw => unquote_string(src, key.as_span()),
                    //     _ => slice(src, key.as_span()),
                    // };
                    // trail.push(Component::Name(key_str));
                    break;
                }
            }
        }

        // enum variants
        Rule::enum_variant_named | Rule::enum_variant_tuple | Rule::enum_variant_unit => {
            let mut it = pair.clone().into_inner();
            let variant_name = it.next().unwrap(); // ident
            if covers(variant_name.as_span(), cursor) {
                // trail.push(Component::Name(slice(src, variant_name.as_span())));
                return Ok(true);
            }
            // else step into payload
            for child in it {
                if descend_find_docident(child, cursor, _src, parent, ident)? {
                    // trail.push(Component::Name(slice(src, variant_name.as_span())));
                    break;
                }
            }
        }

        // leaf cases we ignore
        _ => {
            // dive blindly; if any child returns true we stop
            for child in pair.clone().into_inner() {
                if descend_find_docident(child, cursor, _src, parent, ident)? {
                    return Ok(true);
                }
            }
        }
    }

    Ok(true)
}

/// Recursive descent that collects a breadcrumb whenever the rule has a
/// semantic meaning (field name, map key, index, …).
fn descend(pair: Pair<Rule>, cursor: usize, src: &str, trail: &mut Vec<Component>) -> Result<bool> {
    let span = pair.as_span();
    if !covers(span, cursor) {
        return Ok(false);
    }

    match pair.as_rule() {
        // outermost “ron” rule is ignored
        Rule::ron | Rule::value => {
            for child in pair.into_inner() {
                if descend(child, cursor, src, trail)? {
                    break;
                }
            }
        }

        // structs
        Rule::named_struct | Rule::tuple_struct | Rule::unit_struct => {
            // first inner child may be an ident
            let mut it = pair.clone().into_inner();
            if let Some(id_pair) = it.next()
                && id_pair.as_rule() == Rule::ident
            {
                // if covers(id_pair.as_span(), cursor) {
                trail.push(Component::Name(slice(src, id_pair.as_span())));
                // }
            }

            // if let Some(id_pair) = it.next_if(|p| p.as_rule() == Rule::ident) {
            //     if covers(id_pair.as_span(), cursor) {
            //         trail.push(Component::Name(slice(src, id_pair.as_span())));
            //     }
            // }
            // continue through the rest
            for child in it {
                if descend(child, cursor, src, trail)? {
                    break;
                }
            }
        }

        Rule::named_field => {
            //  ident ":" value
            let mut inner = pair.clone().into_inner();
            let name = inner.next().unwrap(); // ident
            let val = inner.next().unwrap(); // value
            trail.push(Component::Name(slice(src, name.as_span())));
            descend(val, cursor, src, trail)?;
        }

        // lists / tuples
        Rule::list | Rule::tuple => {
            for (idx, val) in pair.into_inner().filter(|p| p.as_rule() == Rule::value).enumerate() {
                if descend(val, cursor, src, trail)? {
                    trail.push(Component::Index(idx));
                    break;
                }
            }
        }

        // maps
        Rule::map => {
            for entry in pair.into_inner() {
                // map_entry
                let mut it = entry.into_inner();
                let key = it.next().unwrap();
                let val = it.next().unwrap();
                if descend(key.clone(), cursor, src, trail)? {
                    // Cursor is inside the key itself -> report previous trail
                    break;
                }
                if descend(val, cursor, src, trail)? {
                    // Cursor somewhere in the value -> emit key string
                    // let key_str = match key.as_rule() {
                    //     Rule::string_std | Rule::string_raw => unquote_string(src, key.as_span()),
                    //     _ => slice(src, key.as_span()),
                    // };
                    // trail.push(Component::Name(key_str));
                    break;
                }
            }
        }

        // enum variants
        Rule::enum_variant_named | Rule::enum_variant_tuple | Rule::enum_variant_unit => {
            let mut it = pair.clone().into_inner();
            let variant_name = it.next().unwrap(); // ident
            if covers(variant_name.as_span(), cursor) {
                trail.push(Component::Name(slice(src, variant_name.as_span())));
                return Ok(true);
            }
            // else step into payload
            for child in it {
                if descend(child, cursor, src, trail)? {
                    trail.push(Component::Name(slice(src, variant_name.as_span())));
                    break;
                }
            }
        }

        // leaf cases we ignore
        _ => {
            // dive blindly; if any child returns true we stop
            for child in pair.clone().into_inner() {
                if descend(child, cursor, src, trail)? {
                    return Ok(true);
                }
            }
        }
    }

    Ok(true)
}

/// Recursive descent that visits every node and collects all `string_std`
/// leaves along with their (concise) path.
fn descend_find_strings(
    pair: Pair<Rule>,
    root_name: &mut Option<String>,
    src: &str,
    trail: &mut Vec<Component>,
    results: &mut Vec<(Vec<Component>, String)>,
) {
    eprintln!("{pair:?}, {src}, {trail:?}");
    match pair.as_rule() {
        Rule::string_std => {
            let raw = pair.as_str();
            let content = raw[1..raw.len() - 1].to_owned();
            results.push((trail.clone(), content));
        }

        Rule::ron | Rule::value => {
            for child in pair.into_inner() {
                descend_find_strings(child, root_name, src, trail, results);
            }
        }

        Rule::named_struct | Rule::tuple_struct | Rule::unit_struct => {
            let mut it = pair.into_inner().peekable();
            let pushed = if it.peek().map(|p| p.as_rule()) == Some(Rule::ident) {
                let id_pair = it.next().unwrap();
                // trail.push(Component::Name(slice(src, id_pair.as_span())));
                if root_name.is_none() {
                    *root_name = Some(slice(src, id_pair.as_span()));
                }
                true
            } else {
                false
            };
            for child in it {
                descend_find_strings(child, root_name, src, trail, results);
            }
            if pushed {
                trail.pop();
            }
        }

        Rule::named_field => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap();
            trail.push(Component::Name(slice(src, name.as_span())));
            for child in inner {
                descend_find_strings(child, root_name, src, trail, results);
            }
            trail.pop();
        }

        Rule::list | Rule::tuple => {
            for (idx, child) in pair.into_inner().filter(|p| p.as_rule() == Rule::value).enumerate() {
                trail.push(Component::Index(idx));
                descend_find_strings(child, root_name, src, trail, results);
                trail.pop();
            }
        }

        Rule::map => {
            for entry in pair.into_inner() {
                let mut it = entry.into_inner();
                let key = it.next().unwrap();
                let val = it.next().unwrap();
                // Collect strings from the key itself
                descend_find_strings(key.clone(), root_name, src, trail, results);
                // Use key text as path component for the value
                trail.push(Component::Key(slice(src, key.as_span())));
                descend_find_strings(val, root_name, src, trail, results);
                trail.pop();
            }
        }

        Rule::enum_variant_named | Rule::enum_variant_tuple | Rule::enum_variant_unit => {
            let mut it = pair.into_inner();
            let variant_name = it.next().unwrap();
            // trail.push(Component::Name(slice(src, variant_name.as_span())));
            if root_name.is_none() {
                *root_name = Some(slice(src, variant_name.as_span()));
            }
            for child in it {
                descend_find_strings(child, root_name, src, trail, results);
            }
            trail.pop();
        }

        _ => {
            for child in pair.into_inner() {
                descend_find_strings(child, root_name, src, trail, results);
            }
        }
    }
}

fn covers(span: Span, pos: usize) -> bool {
    let (lo, hi) = (span.start(), span.end());
    lo <= pos && pos < hi
}

fn slice<'s>(src: &'s str, span: Span<'s>) -> String {
    src[span.start()..span.end()].to_owned()
}

// Rudimentary `"` or `r#"..."#` stripper so map keys render nicely
// fn unquote_string(src: &str, span: Span) -> String {
//     let txt = &src[span.start()..span.end()];
//     let first_quote = txt.find('"').unwrap();
//     let last_quote = txt.rfind('"').unwrap();
//     txt[first_quote + 1..last_quote].to_owned()
// }

pub fn pos_byte_index(line: usize, col: usize, s: &str) -> Option<usize> {
    let mut line_no = 1;
    let mut col_no = 1;

    let mut i = 0;

    // Slightly non-intuitive arithmetic: a zero-length string at line 1, col 1 -> 0

    if line_no == line && col_no == col {
        return Some(i);
    }

    for (byte_idx, ch) in s.char_indices() {
        if line_no == line && col_no == col {
            return Some(i);
        }

        // "\n" and "\r\n" each come through the iterator as a single grapheme
        if ch == '\n' {
            line_no += 1;
            col_no = 1;
        } else {
            col_no += 1;
        }

        i = byte_idx;
    }

    // ...and a string of length 7 at line 1, col 8 -> 7
    if line_no == line && col_no == col {
        return Some(i);
    }

    None
}
