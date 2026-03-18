UPDATE tasks
SET
    baseline_start_at = COALESCE(baseline_start_at, start_date),
    baseline_end_at = COALESCE(baseline_end_at, end_date, due_date),
    updated_at = datetime('now')
WHERE deleted_at IS NULL
  AND (baseline_start_at IS NULL OR baseline_end_at IS NULL)
  AND COALESCE(baseline_start_at, start_date) IS NOT NULL
  AND COALESCE(baseline_end_at, end_date, due_date) IS NOT NULL
  AND julianday(COALESCE(baseline_end_at, end_date, due_date)) > julianday(COALESCE(baseline_start_at, start_date));
