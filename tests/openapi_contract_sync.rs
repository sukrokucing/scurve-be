#![allow(clippy::uninlined_format_args)]

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch", "head", "options"];

fn operation_set(spec: &Value) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return out;
    };

    for (path, path_item) in paths {
        let Some(path_obj) = path_item.as_object() else {
            continue;
        };
        for (method, _) in path_obj {
            if HTTP_METHODS.contains(&method.as_str()) {
                out.insert((path.clone(), method.clone()));
            }
        }
    }

    out
}

fn collect_schema_refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get("$ref") {
                if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                    out.push(name.to_string());
                }
            }
            for value in map.values() {
                collect_schema_refs(value, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_schema_refs(item, out);
            }
        }
        _ => {}
    }
}

#[test]
fn committed_openapi_matches_generated_operations() -> anyhow::Result<()> {
    let generated = serde_json::to_value(s_curve::docs::build_openapi(8000)?)?;

    let committed_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
    let committed_str = std::fs::read_to_string(&committed_path)?;
    let committed: Value = serde_json::from_str(&committed_str)?;

    let generated_ops = operation_set(&generated);
    let committed_ops = operation_set(&committed);

    let missing: Vec<_> = generated_ops.difference(&committed_ops).cloned().collect();
    let extra: Vec<_> = committed_ops.difference(&generated_ops).cloned().collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "openapi.json is out of sync. Missing in committed: {:?}. Extra in committed: {:?}",
        missing,
        extra
    );

    Ok(())
}

#[test]
fn generated_openapi_operations_have_summary() -> anyhow::Result<()> {
    let generated = serde_json::to_value(s_curve::docs::build_openapi(8000)?)?;
    let paths = generated
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("paths missing"))?;

    for (path, path_item) in paths.iter() {
        let Some(path_obj) = path_item.as_object() else {
            continue;
        };
        for (method, op) in path_obj {
            if !HTTP_METHODS.contains(&method.as_str()) {
                continue;
            }

            let summary = op
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            assert!(
                !summary.is_empty(),
                "missing summary for operation {} {}",
                method.to_uppercase(),
                path
            );
        }
    }

    Ok(())
}

#[test]
fn generated_openapi_operations_have_description() -> anyhow::Result<()> {
    let generated = serde_json::to_value(s_curve::docs::build_openapi(8000)?)?;
    let paths = generated
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("paths missing"))?;

    for (path, path_item) in paths.iter() {
        let Some(path_obj) = path_item.as_object() else {
            continue;
        };
        for (method, op) in path_obj {
            if !HTTP_METHODS.contains(&method.as_str()) {
                continue;
            }

            let description = op
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            assert!(
                !description.is_empty(),
                "missing description for operation {} {}",
                method.to_uppercase(),
                path
            );
        }
    }

    Ok(())
}

#[test]
fn public_endpoints_are_explicitly_unauthenticated() -> anyhow::Result<()> {
    let generated = serde_json::to_value(s_curve::docs::build_openapi(8000)?)?;

    let public_ops = [
        ("/api/health", "get"),
        ("/auth/login", "post"),
        ("/auth/register", "post"),
        ("/auth/forgot-password", "post"),
        ("/auth/reset-password", "post"),
    ];

    let paths = generated
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("paths missing"))?;

    for (path, method) in public_ops {
        let path_item = paths
            .get(path)
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("missing path {}", path))?;
        let op = path_item
            .get(method)
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("missing operation {} {}", method, path))?;
        let security = op
            .get("security")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("missing security for {} {}", method, path))?;
        assert!(
            security.is_empty(),
            "expected {} {} to be public with empty security, got {:?}",
            method.to_uppercase(),
            path,
            security
        );
    }

    Ok(())
}

#[test]
fn generated_openapi_has_no_dangling_schema_refs() -> anyhow::Result<()> {
    let generated = serde_json::to_value(s_curve::docs::build_openapi(8000)?)?;

    let schema_obj = generated
        .get("components")
        .and_then(Value::as_object)
        .and_then(|c| c.get("schemas"))
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("components.schemas missing"))?;

    let mut refs = Vec::new();
    collect_schema_refs(&generated, &mut refs);

    let missing: Vec<String> = refs
        .into_iter()
        .filter(|r| !schema_obj.contains_key(r))
        .collect();

    assert!(
        missing.is_empty(),
        "dangling schema refs in generated OpenAPI: {:?}",
        missing
    );

    Ok(())
}
