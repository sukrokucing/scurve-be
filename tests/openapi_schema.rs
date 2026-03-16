#![allow(clippy::uninlined_format_args)]

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
        "progress_method",
        "blocked_flag",
        "blocked_reason",
        "baseline_start_at",
        "baseline_end_at",
        "task_weight",
        "start_date",
        "end_date",
        "duration_days",
        "assignee",
        "progress",
        "completed_at",
        "completed_at_is_backfilled",
        "schedule_status",
        "execution_status",
        "expected_progress_pct",
        "actual_progress_pct",
        "variance_pct",
        "health_status",
        "expected_progress_source",
        "actual_progress_source",
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
fn openapi_has_task_list_schedule_status_filter() -> anyhow::Result<()> {
    let doc = s_curve::docs::build_openapi(8000)?;
    let v = serde_json::to_value(&doc)?;

    let parameters = v
        .get("paths")
        .and_then(Value::as_object)
        .and_then(|paths| paths.get("/projects/{project_id}/tasks"))
        .and_then(Value::as_object)
        .and_then(|item| item.get("get"))
        .and_then(Value::as_object)
        .and_then(|op| op.get("parameters"))
        .and_then(Value::as_array)
        .expect("task list parameters must exist");

    let has_schedule_status = parameters.iter().any(|param| {
        param
            .get("name")
            .and_then(Value::as_str)
            .map(|name| name == "schedule_status")
            .unwrap_or(false)
    });
    assert!(
        has_schedule_status,
        "task list must expose schedule_status query param"
    );

    let has_health_status = parameters.iter().any(|param| {
        param
            .get("name")
            .and_then(Value::as_str)
            .map(|name| name == "health_status")
            .unwrap_or(false)
    });
    assert!(
        has_health_status,
        "task list must expose health_status query param"
    );

    Ok(())
}

#[test]
fn openapi_has_dashboard_summary_fields() -> anyhow::Result<()> {
    let doc = s_curve::docs::build_openapi(8000)?;
    let v = serde_json::to_value(&doc)?;

    let schemas = v
        .get("components")
        .and_then(Value::as_object)
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
        .expect("components.schemas must exist");

    let dashboard_props = schemas
        .get("DashboardResponse")
        .and_then(Value::as_object)
        .and_then(|s| s.get("properties"))
        .and_then(Value::as_object)
        .expect("DashboardResponse.properties must exist");

    for key in [
        "overall_progress_pct",
        "task_status_counts",
        "workload_distribution",
        "assignment_coverage_pct",
        "due_date_coverage_pct",
    ] {
        assert!(
            dashboard_props.contains_key(key),
            "DashboardResponse missing '{}'",
            key
        );
    }

    let schedule_status = schemas
        .get("TaskScheduleStatus")
        .and_then(Value::as_object)
        .and_then(|s| s.get("enum"))
        .and_then(Value::as_array)
        .cloned()
        .expect("TaskScheduleStatus enum must exist");
    assert_eq!(
        schedule_status,
        vec![
            Value::String("finished_early".into()),
            Value::String("overdue".into()),
            Value::String("on_time".into()),
            Value::String("not_specified".into()),
        ]
    );

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
    assert_eq!(sort_by.len(), 11);
    for expected in [
        "expected_progress_pct",
        "actual_progress_pct",
        "variance_pct",
        "health_status",
    ] {
        assert!(
            sort_by.contains(&Value::String(expected.into())),
            "TaskSortBy missing '{}'",
            expected
        );
    }

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

#[test]
fn openapi_has_scurve_status_enums() -> anyhow::Result<()> {
    let doc = s_curve::docs::build_openapi(8000)?;
    let v = serde_json::to_value(&doc)?;

    let schemas = v
        .get("components")
        .and_then(Value::as_object)
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
        .expect("components.schemas must exist");

    let stage = schemas
        .get("SCurveStage")
        .and_then(Value::as_object)
        .and_then(|s| s.get("enum"))
        .and_then(Value::as_array)
        .expect("SCurveStage enum must exist");
    assert_eq!(
        stage,
        &vec![
            Value::String("lag".into()),
            Value::String("log".into()),
            Value::String("maturity".into()),
            Value::String("decline".into()),
        ]
    );

    let rule_status = schemas
        .get("Rule5070Status")
        .and_then(Value::as_object)
        .and_then(|s| s.get("enum"))
        .and_then(Value::as_array)
        .expect("Rule5070Status enum must exist");
    assert!(rule_status.contains(&Value::String("pass".into())));
    assert!(rule_status.contains(&Value::String("fail".into())));
    assert!(rule_status.contains(&Value::String("unsupported_metric".into())));

    let data_status = schemas
        .get("SCurveDataStatus")
        .and_then(Value::as_object)
        .and_then(|s| s.get("enum"))
        .and_then(Value::as_array)
        .expect("SCurveDataStatus enum must exist");
    assert_eq!(
        data_status,
        &vec![
            Value::String("ok".into()),
            Value::String("insufficient_data".into()),
            Value::String("unsupported_metric".into()),
        ]
    );

    Ok(())
}

#[test]
fn openapi_has_task_health_and_progress_component_schemas() -> anyhow::Result<()> {
    let doc = s_curve::docs::build_openapi(8000)?;
    let v = serde_json::to_value(&doc)?;

    let schemas = v
        .get("components")
        .and_then(Value::as_object)
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
        .expect("components.schemas must exist");

    for schema in [
        "TaskProgressMethod",
        "TaskExecutionStatus",
        "TaskHealthStatus",
        "TaskProgressComponent",
        "ReplaceTaskProgressComponentsRequest",
        "TaskHealthRule",
        "TaskHealthRuleSetResponse",
        "UpdateTaskHealthRulesRequest",
    ] {
        assert!(schemas.contains_key(schema), "missing schema '{}'", schema);
    }

    let paths = v
        .get("paths")
        .and_then(Value::as_object)
        .expect("paths must exist");
    for path in [
        "/projects/{project_id}/tasks/{id}/progress-components",
        "/projects/{project_id}/task-health/rules",
    ] {
        assert!(paths.contains_key(path), "missing path '{}'", path);
    }

    Ok(())
}
