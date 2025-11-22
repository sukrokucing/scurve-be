pub fn case_uuid(col: &str) -> String {
    let alias = col.split('.').last().unwrap_or(col);
    format!(
        "CASE WHEN typeof({c})='blob' THEN lower(substr(hex({c}),1,8) || '-' || substr(hex({c}),9,4) || '-' || substr(hex({c}),13,4) || '-' || substr(hex({c}),17,4) || '-' || substr(hex({c}),21)) ELSE {c} END as {a}",
        c = col,
        a = alias
    )
}

pub fn match_uuid_clause(col: &str) -> String {
    // returns a predicate that matches either blob hex (without dashes) or text uuid equality
    format!(
        "((typeof({c})='blob' AND hex({c})=upper(replace(?,'-',''))) OR (typeof({c})='text' AND {c} = ?))",
        c = col
    )
}
