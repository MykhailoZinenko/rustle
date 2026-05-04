use std::collections::BTreeMap;
use std::ops::Range;

pub struct SuggestionItem {
    pub label: String,
    pub detail: &'static str,
}

pub struct SuggestionContext {
    pub replace_range: Range<usize>,
    pub items: Vec<SuggestionItem>,
}

const KEYWORD_SUGGESTIONS: [(&str, &str); 17] = [
    ("fn", "keyword"),
    ("let", "keyword"),
    ("if", "keyword"),
    ("else", "keyword"),
    ("match", "keyword"),
    ("while", "keyword"),
    ("for", "keyword"),
    ("foreach", "keyword"),
    ("in", "keyword"),
    ("return", "keyword"),
    ("const", "keyword"),
    ("state", "keyword"),
    ("import", "keyword"),
    ("out", "keyword"),
    ("console", "keyword"),
    ("try", "keyword"),
    ("none", "keyword"),
];

const TYPE_SUGGESTIONS: [(&str, &str); 5] = [
    ("float", "type"),
    ("bool", "type"),
    ("array", "type"),
    ("list", "type"),
    ("res", "type"),
];

const BUILTIN_SUGGESTIONS: [(&str, &str); 7] = [
    ("console.warn", "builtin"),
    ("console.error", "builtin"),
    ("resolution", "builtin"),
    ("origin", "builtin"),
    ("and", "operator"),
    ("or", "operator"),
    ("not", "operator"),
];

pub fn suggestion_context(buffer: &str, cursor_char_index: usize) -> SuggestionContext {
    let cursor_byte_index = char_to_byte_index(buffer, cursor_char_index);
    let replace_range = current_ident_range(buffer, cursor_byte_index);
    let prefix = buffer[replace_range.start..cursor_byte_index.min(replace_range.end)].to_string();
    let items = collect_suggestions(buffer, &prefix);

    SuggestionContext {
        replace_range,
        items,
    }
}

pub fn apply_suggestion(
    buffer: &mut String,
    replace_range: Range<usize>,
    suggestion: &str,
) -> usize {
    buffer.replace_range(replace_range.clone(), suggestion);
    byte_to_char_index(buffer, replace_range.start + suggestion.len())
}

fn collect_suggestions(buffer: &str, prefix: &str) -> Vec<SuggestionItem> {
    let mut items = BTreeMap::<String, &'static str>::new();

    for (label, detail) in KEYWORD_SUGGESTIONS
        .into_iter()
        .chain(TYPE_SUGGESTIONS)
        .chain(BUILTIN_SUGGESTIONS)
    {
        items.insert(label.to_string(), detail);
    }

    for identifier in collect_buffer_identifiers(buffer) {
        items.entry(identifier).or_insert("symbol");
    }

    let mut filtered: Vec<_> = items
        .into_iter()
        .filter(|(label, _)| prefix.is_empty() || label.starts_with(prefix))
        .map(|(label, detail)| SuggestionItem { label, detail })
        .collect();

    filtered.sort_by(|left, right| {
        left.label
            .len()
            .cmp(&right.label.len())
            .then_with(|| left.label.cmp(&right.label))
    });
    filtered.truncate(12);
    filtered
}

fn collect_buffer_identifiers(buffer: &str) -> Vec<String> {
    let bytes = buffer.as_bytes();
    let mut identifiers = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if is_ident_start(byte) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_ident_continue(bytes[index]) {
                index += 1;
            }
            identifiers.push(buffer[start..index].to_string());
        } else {
            index += 1;
        }
    }

    identifiers
}

fn current_ident_range(buffer: &str, cursor_byte_index: usize) -> Range<usize> {
    let bytes = buffer.as_bytes();
    let mut start = cursor_byte_index.min(bytes.len());
    let mut end = start;

    while start > 0 && is_ident_continue(bytes[start - 1]) {
        start -= 1;
    }
    while end < bytes.len() && is_ident_continue(bytes[end]) {
        end += 1;
    }

    start..end
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(text.len())
}

fn byte_to_char_index(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
