use std::sync::OnceLock;

fn use_text_fast_path() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("UUID_TEXT_FAST_PATH")
            .ok()
            .map(|raw| {
                matches!(
                    raw.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    })
}

pub fn expr_uuid(col: &str) -> String {
    if use_text_fast_path() {
        return format!("CAST({c} AS TEXT)", c = col);
    }
    format!(
        "CAST(CASE WHEN typeof({c})='blob' THEN lower(substr(hex({c}),1,8) || '-' || substr(hex({c}),9,4) || '-' || substr(hex({c}),13,4) || '-' || substr(hex({c}),17,4) || '-' || substr(hex({c}),21)) ELSE {c} END AS TEXT)",
        c = col
    )
}

pub fn case_uuid(col: &str) -> String {
    let alias = col.split('.').next_back().unwrap_or(col);
    format!("{} as {}", expr_uuid(col), alias)
}

pub fn match_uuid_clause(col: &str) -> String {
    if use_text_fast_path() {
        // Keep two placeholders for compatibility with existing call-sites.
        // The second placeholder is a bind-only constant and does not affect results.
        return format!("({c} = ? AND ? IS NOT NULL)", c = col);
    }
    // returns a predicate that matches either blob hex (without dashes) or text uuid equality
    format!(
        "((typeof({c})='blob' AND hex({c})=upper(replace(?,'-',''))) OR (typeof({c})='text' AND {c} = ?))",
        c = col
    )
}
