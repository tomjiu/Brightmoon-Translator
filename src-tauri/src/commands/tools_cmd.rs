/// Variable name transformation tool
/// Converts between different naming conventions:
/// snake_case, SNAKE_CASE, kebab-case, camelCase, PascalCase, dot.notation, Title Case
#[tauri::command]
pub fn transform_variable_name(text: String, target_format: String) -> String {
    if text.trim().is_empty() {
        return text;
    }

    // First, split the input into words regardless of current format
    let words = split_into_words(&text);

    match target_format.as_str() {
        "snake_case" => words.join("_").to_lowercase(),
        "SNAKE_CASE" => words.join("_").to_uppercase(),
        "kebab-case" => words.join("-").to_lowercase(),
        "camelCase" => {
            let mut result = String::new();
            for (i, word) in words.iter().enumerate() {
                if i == 0 {
                    result.push_str(&word.to_lowercase());
                } else {
                    result.push_str(&capitalize_first(word));
                }
            }
            result
        }
        "PascalCase" => words.iter().map(|w| capitalize_first(w)).collect(),
        "dot.notation" => words.join(".").to_lowercase(),
        "Title Case" => words
            .iter()
            .map(|w| capitalize_first(w))
            .collect::<Vec<_>>()
            .join(" "),
        _ => text,
    }
}

fn split_into_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current_word = String::new();

    for ch in text.chars() {
        if ch == '_' || ch == '-' || ch == '.' || ch == ' ' {
            if !current_word.is_empty() {
                words.push(current_word.clone());
                current_word.clear();
            }
        } else if ch.is_uppercase() && !current_word.is_empty() && !current_word.ends_with(|c: char| c.is_uppercase()) {
            // camelCase or PascalCase boundary
            words.push(current_word.clone());
            current_word.clear();
            current_word.push(ch);
        } else {
            current_word.push(ch);
        }
    }

    if !current_word.is_empty() {
        words.push(current_word);
    }

    words
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
    }
}

/// Cycle through naming conventions
#[tauri::command]
pub fn cycle_variable_name(text: String) -> (String, String) {
    let formats = [
        "snake_case",
        "SNAKE_CASE",
        "kebab-case",
        "camelCase",
        "PascalCase",
        "dot.notation",
        "Title Case",
    ];

    // Detect current format
    let current_format = detect_format(&text);

    // Find next format
    let current_index = formats
        .iter()
        .position(|&f| f == current_format)
        .unwrap_or(0);
    let next_index = (current_index + 1) % formats.len();
    let next_format = formats[next_index];

    let transformed = transform_variable_name(text, next_format.to_string());
    (transformed, next_format.to_string())
}

fn detect_format(text: &str) -> &str {
    if text.contains('_') {
        if text == text.to_uppercase() {
            "SNAKE_CASE"
        } else {
            "snake_case"
        }
    } else if text.contains('-') {
        "kebab-case"
    } else if text.contains('.') {
        "dot.notation"
    } else if text.contains(' ') {
        "Title Case"
    } else if text.chars().next().map_or(false, |c| c.is_uppercase()) {
        "PascalCase"
    } else {
        "camelCase"
    }
}
