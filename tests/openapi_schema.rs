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
    let keys = [
        "start_date",
        "end_date",
        "duration_days",
        "assignee",
        "progress",
    ];
    for k in &keys {
        assert!(
            props.contains_key(*k),
            "OpenAPI Task schema missing '{}'",
            k
        );
    }

    Ok(())
}

#[test]
fn openapi_has_task_list_sort_enums() -> anyhow::Result<()> {
    let doc = s_curve::docs::build_openapi(8000)?;
    let v = serde_json::to_value(&doc)?;

    let schemas = v
        .get("components")
        .and_then(Value::as_object)
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
        .expect("components.schemas must exist");

    let sort_by = schemas
        .get("TaskSortBy")
        .and_then(Value::as_object)
        .and_then(|s| s.get("enum"))
        .and_then(Value::as_array)
        .expect("TaskSortBy enum must exist");
    assert_eq!(sort_by.len(), 7);

    let sort_dir = schemas
        .get("TaskSortDir")
        .and_then(Value::as_object)
        .and_then(|s| s.get("enum"))
        .and_then(Value::as_array)
        .expect("TaskSortDir enum must exist");
    assert_eq!(
        sort_dir,
        &vec![Value::String("asc".into()), Value::String("desc".into())]
    );

    Ok(())
}
