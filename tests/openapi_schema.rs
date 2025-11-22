use serde_json::Value;

#[test]
fn openapi_has_task_timeline_fields() -> anyhow::Result<()> {
    // Build the OpenAPI document the same way the server does
    let doc = s_curve::docs::build_openapi(8000)?;
    let v = serde_json::to_value(&doc)?;

    // Navigate to components.schemas.Task.properties
    let props = v
        .get("components")
        .and_then(Value::as_object)
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
        .and_then(|s| s.get("Task"))
        .and_then(Value::as_object)
        .and_then(|t| t.get("properties"))
        .and_then(Value::as_object)
        .expect("components.schemas.Task.properties must exist");

    // Check for timeline-related keys
    let keys = ["start_date", "end_date", "duration_days", "assignee", "progress"];
    for k in &keys {
        assert!(props.contains_key(*k), "OpenAPI Task schema missing '{}'", k);
    }

    Ok(())
}
