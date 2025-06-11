use std::path::PathBuf;

use autoschematic_core::diag::{DiagnosticOutput, DiagnosticSeverity};
use lsp_types::ExecuteCommandParams;
use tower_lsp_server::jsonrpc;

pub fn severity_to_lsp(severity: u8) -> Option<lsp_types::DiagnosticSeverity> {
    match severity {
        val if val == DiagnosticSeverity::ERROR as u8 => Some(lsp_types::DiagnosticSeverity::ERROR),
        val if val == DiagnosticSeverity::WARNING as u8 => Some(lsp_types::DiagnosticSeverity::WARNING),
        val if val == DiagnosticSeverity::INFORMATION as u8 => Some(lsp_types::DiagnosticSeverity::INFORMATION),
        val if val == DiagnosticSeverity::HINT as u8 => Some(lsp_types::DiagnosticSeverity::HINT),
        _ => None,
    }
}

pub fn diag_to_lsp(diag_output: DiagnosticOutput) -> Vec<lsp_types::Diagnostic> {
    eprintln!("diag_to_lsp: {:?}", diag_output);
    let mut res = Vec::new();
    for diag in diag_output.diagnostics {
        res.push(lsp_types::Diagnostic {
            range: lsp_types::Range::new(
                lsp_types::Position {
                    line: diag.span.start.line - 1,
                    character: diag.span.start.col - 1,
                },
                lsp_types::Position {
                    line: diag.span.end.line - 1,
                    character: diag.span.end.col - 1,
                },
            ),
            severity: severity_to_lsp(diag.severity),
            code: None,
            code_description: None,
            source: None,
            message: diag.message,
            related_information: None,
            tags: None,
            data: None,
        });
    }
    res
}

pub fn lsp_bail(msg: &str) -> tower_lsp_server::jsonrpc::Error {
    let mut err = jsonrpc::Error::internal_error();
    err.data = Some(serde_json::Value::String(msg.into()));
    err
}

pub fn lsp_error(e: anyhow::Error) -> tower_lsp_server::jsonrpc::Error {
    let mut err = jsonrpc::Error::internal_error();
    let msg = format!("{}", e);
    err.data = Some(serde_json::Value::String(msg));
    err
}

pub fn lsp_param_to_path(params: ExecuteCommandParams) -> Option<PathBuf> {
    if params.arguments.len() != 1 {
        return None;
    }

    let Some(path_arg) = params.arguments.first() else {
        return None;
    };

    let Ok(file_path) = serde_json::from_value::<String>(path_arg.clone()) else {
        return None;
    };

    let file_path = PathBuf::from(file_path);

    let Ok(file_path) = file_path.strip_prefix(std::env::current_dir().unwrap()) else {
        return None;
    };

    Some(file_path.into())
}

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
