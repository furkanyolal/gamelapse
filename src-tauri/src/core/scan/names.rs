//! Turns release-style folder names into game names.

/// Words that are release tags, never part of a title (compared lowercase).
const TAG_WORDS: [&str; 10] = [
    "repack",
    "fitgirl",
    "dodi",
    "elamigos",
    "ankergames",
    "goldberg",
    "codex",
    "skidrow",
    "plaza",
    "gog",
];

/// Short words kept lowercase in the middle of a title.
const SMALL_WORDS: [&str; 9] = ["of", "the", "a", "an", "and", "in", "on", "to", "for"];

/// Removes `[...]`, `(...)` and `{...}` groups, nesting included.
fn strip_brackets(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0usize;
    for c in raw.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn is_numeric(piece: &str) -> bool {
    !piece.is_empty() && piece.bytes().all(|b| b.is_ascii_digit())
}

/// `v1`, `V2`, `v10b`: a version marker piece.
fn is_v_version(piece: &str) -> bool {
    let mut chars = piece.chars();
    matches!(chars.next(), Some('v' | 'V'))
        && chars
            .as_str()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_alphanumeric())
}

/// Splits a token on dots, dropping version suffixes: everything from the first `vN` piece,
/// or a trailing run of two or more numeric pieces (`1.02`, `1.5.78`).
fn dotted_words(token: &str) -> Vec<String> {
    let pieces: Vec<&str> = token.split('.').filter(|p| !p.is_empty()).collect();
    let mut end = pieces.len();
    if let Some(i) = pieces.iter().position(|p| is_v_version(p)) {
        end = i;
    } else {
        let run = pieces.iter().rev().take_while(|p| is_numeric(p)).count();
        if run >= 2 {
            end -= run;
        }
    }
    pieces[..end].iter().map(|p| p.to_string()).collect()
}

fn is_tag_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    if TAG_WORDS.contains(&lower.as_str()) {
        return true;
    }
    // `multi12`, `multi5`.
    lower
        .strip_prefix("multi")
        .is_some_and(|n| !n.is_empty() && is_numeric(n))
}

/// Inserts spaces at case and digit boundaries: `RedDeadRedemption2` -> `Red Dead Redemption 2`,
/// `XMLParser` -> `XML Parser`.
fn split_camel(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let mut out = String::with_capacity(word.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_alphabetic() && c.is_ascii_digit())
                || (prev.is_ascii_digit()
                    && c.is_uppercase()
                    && next.is_some_and(char::is_lowercase))
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(char::is_lowercase));
            if boundary {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

fn title_case(words: &[String]) -> String {
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let all_lower = w.chars().all(|c| !c.is_uppercase());
            if !all_lower || (i > 0 && SMALL_WORDS.contains(&w.as_str())) {
                return w.clone();
            }
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Cleans a folder name into a game name; falls back to the trimmed input when nothing is
/// left.
pub fn clean_name(raw: &str) -> String {
    let had_separator = raw.contains([' ', '-', '_', '.']);
    let stripped = strip_brackets(raw).replace(['-', '_'], " ");

    let mut words: Vec<String> = Vec::new();
    for token in stripped.split_whitespace() {
        if token.contains('.') {
            words.extend(dotted_words(token));
        } else {
            words.push(token.to_string());
        }
    }

    // Drop tag words, `v1` markers and `Build 123`.
    let mut kept: Vec<String> = Vec::new();
    let mut iter = words.into_iter().peekable();
    while let Some(word) = iter.next() {
        if is_tag_word(&word) || is_v_version(&word) {
            continue;
        }
        if word.eq_ignore_ascii_case("build") && iter.peek().is_some_and(|n| is_numeric(n)) {
            iter.next();
            continue;
        }
        kept.push(word);
    }

    if kept.len() == 1 && !had_separator {
        kept = split_camel(&kept[0])
            .split(' ')
            .map(str::to_string)
            .collect();
    }

    let name = title_case(&kept);
    if name.is_empty() {
        raw.trim().to_string()
    } else {
        name
    }
}
