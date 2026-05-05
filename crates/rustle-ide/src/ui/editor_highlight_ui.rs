use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, TextFormat, Ui};
use rustle_lang::syntax::lexer::Lexer;
use rustle_lang::{Token, TokenKind};

use crate::theme::{
    EDITOR_FONT_SIZE, SYNTAX_FUNCTION, SYNTAX_IDENTIFIER, SYNTAX_KEYWORD, SYNTAX_LITERAL,
    SYNTAX_OPERATOR, SYNTAX_PUNCTUATION, SYNTAX_TYPE,
};

pub fn highlight_layout(ui: &Ui, text: &str) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;

    match Lexer::new(text).tokenize() {
        Ok(tokens) => append_highlighted_text(&mut job, text, &tokens),
        Err(_) => append_plain(&mut job, text, ui.visuals().text_color()),
    }

    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

fn append_highlighted_text(job: &mut egui::text::LayoutJob, text: &str, tokens: &[Token]) {
    let line_starts = line_start_offsets(text);
    let mut cursor = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.kind, TokenKind::Eof) {
            break;
        }

        let start = token_start_offset(text, &line_starts, token);
        if start > cursor && start <= text.len() {
            append_plain(job, &text[cursor..start], SYNTAX_IDENTIFIER);
        }

        let end = start
            .saturating_add(token_source_len(text, start, token))
            .min(text.len());
        if end > start {
            append_plain(job, &text[start..end], token_color(&token.kind, tokens.get(index + 1)));
            cursor = end;
        }
    }

    if cursor < text.len() {
        append_plain(job, &text[cursor..], SYNTAX_IDENTIFIER);
    }
}

fn append_plain(job: &mut egui::text::LayoutJob, text: &str, color: Color32) {
    if text.is_empty() {
        return;
    }

    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::monospace(EDITOR_FONT_SIZE),
            color,
            ..Default::default()
        },
    );
}

fn token_color(kind: &TokenKind, next: Option<&Token>) -> Color32 {
    match kind {
        TokenKind::StringLit(_) | TokenKind::TemplateLit(_) | TokenKind::Float(_) | TokenKind::Bool(_) | TokenKind::HexColor(_) => {
            SYNTAX_LITERAL
        }
        kind if kind.is_type_keyword() => SYNTAX_TYPE,
        kind if kind.is_keyword() => SYNTAX_KEYWORD,
        TokenKind::Ident(_) if next.is_some_and(|token| matches!(token.kind, TokenKind::LParen)) => {
            SYNTAX_FUNCTION
        }
        TokenKind::Ident(_) => SYNTAX_IDENTIFIER,
        TokenKind::Colon
        | TokenKind::Comma
        | TokenKind::Semicolon
        | TokenKind::Dot
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LBracket
        | TokenKind::RBracket => SYNTAX_PUNCTUATION,
        TokenKind::Eof => SYNTAX_IDENTIFIER,
        _ => SYNTAX_OPERATOR,
    }
}

fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn token_start_offset(text: &str, line_starts: &[usize], token: &Token) -> usize {
    let line_start = line_starts
        .get(token.line.saturating_sub(1))
        .copied()
        .unwrap_or(text.len());
    (line_start + token.column.saturating_sub(1)).min(text.len())
}

fn token_source_len(text: &str, start: usize, token: &Token) -> usize {
    let bytes = text.as_bytes();
    match &token.kind {
        TokenKind::Float(_) => scan_number_len(bytes, start),
        TokenKind::Ident(name) => name.len(),
        TokenKind::StringLit(_) => scan_string_len(bytes, start),
        TokenKind::TemplateLit(_) => scan_template_len(bytes, start),
        TokenKind::HexColor(value) => value.len() + 1,
        TokenKind::Bool(true) => 4,
        TokenKind::Bool(false) => 5,
        TokenKind::Fn => 2,
        TokenKind::Let => 3,
        TokenKind::If => 2,
        TokenKind::Else => 4,
        TokenKind::Match => 5,
        TokenKind::While => 5,
        TokenKind::For => 3,
        TokenKind::Foreach => 7,
        TokenKind::In => 2,
        TokenKind::Return => 6,
        TokenKind::Const => 5,
        TokenKind::State => 5,
        TokenKind::Import => 6,
        TokenKind::Out => 3,
        TokenKind::Console => 7,
        TokenKind::Try => 3,
        TokenKind::And => 3,
        TokenKind::Or => 2,
        TokenKind::Not => 3,
        TokenKind::As => 2,
        TokenKind::Break => 5,
        TokenKind::Continue => 8,
        TokenKind::None => 4,
        TokenKind::TFloat => 5,
        TokenKind::TBool => 4,
        TokenKind::TArray => 5,
        TokenKind::TList => 4,
        TokenKind::TRes => 3,
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Eq
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::At
        | TokenKind::Question
        | TokenKind::Colon
        | TokenKind::Comma
        | TokenKind::Semicolon
        | TokenKind::Dot
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LBracket
        | TokenKind::RBracket => 1,
        TokenKind::PlusEq
        | TokenKind::MinusEq
        | TokenKind::PlusPlus
        | TokenKind::MinusMinus
        | TokenKind::StarEq
        | TokenKind::SlashEq
        | TokenKind::EqEq
        | TokenKind::BangEq
        | TokenKind::LtEq
        | TokenKind::GtEq
        | TokenKind::LtLt
        | TokenKind::Arrow
        | TokenKind::FatArrow
        | TokenKind::QuestionQuestion
        | TokenKind::QuestionDot => 2,
        TokenKind::Eof => 0,
    }
}

fn scan_number_len(bytes: &[u8], start: usize) -> usize {
    let mut end = start;

    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }

    if end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }

    end.saturating_sub(start)
}

fn scan_string_len(bytes: &[u8], start: usize) -> usize {
    if start >= bytes.len() || bytes[start] != b'"' {
        return 0;
    }

    let mut index = start + 1;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;

        if escaped {
            escaped = false;
            continue;
        }

        if byte == b'\\' {
            escaped = true;
            continue;
        }

        if byte == b'"' {
            break;
        }
    }

    index.saturating_sub(start)
}

fn scan_template_len(bytes: &[u8], start: usize) -> usize {
    if start >= bytes.len() || bytes[start] != b'`' {
        return 0;
    }
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            index += 1;
            break;
        }
        if bytes[index] == b'\\' {
            index += 1; // skip escaped char
        }
        index += 1;
    }
    index.saturating_sub(start)
}
