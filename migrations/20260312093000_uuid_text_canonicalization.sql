-- Canonicalize UUID-like columns to lower-case hyphenated TEXT.
-- This removes mixed BLOB/TEXT UUID storage that causes non-sargable queries.
-- Runs inside sqlx's SQLite migration transaction; defer FK checks until commit.

PRAGMA defer_foreign_keys = ON;

UPDATE users
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE projects
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE projects
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE tasks
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE tasks
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE tasks
SET assignee = CASE
    WHEN typeof(assignee) = 'blob' AND length(assignee) = 16 THEN
        lower(
            substr(hex(assignee), 1, 8) || '-' ||
            substr(hex(assignee), 9, 4) || '-' ||
            substr(hex(assignee), 13, 4) || '-' ||
            substr(hex(assignee), 17, 4) || '-' ||
            substr(hex(assignee), 21, 12)
        )
    WHEN typeof(assignee) = 'text'
         AND length(replace(assignee, '-', '')) = 32
         AND lower(replace(assignee, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(assignee, '-', ''), 1, 8) || '-' ||
            substr(replace(assignee, '-', ''), 9, 4) || '-' ||
            substr(replace(assignee, '-', ''), 13, 4) || '-' ||
            substr(replace(assignee, '-', ''), 17, 4) || '-' ||
            substr(replace(assignee, '-', ''), 21, 12)
        )
    ELSE assignee
END
WHERE assignee IS NOT NULL
  AND (
      (typeof(assignee) = 'blob' AND length(assignee) = 16)
      OR
      (typeof(assignee) = 'text'
       AND length(replace(assignee, '-', '')) = 32
       AND lower(replace(assignee, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE tasks
SET parent_id = CASE
    WHEN typeof(parent_id) = 'blob' AND length(parent_id) = 16 THEN
        lower(
            substr(hex(parent_id), 1, 8) || '-' ||
            substr(hex(parent_id), 9, 4) || '-' ||
            substr(hex(parent_id), 13, 4) || '-' ||
            substr(hex(parent_id), 17, 4) || '-' ||
            substr(hex(parent_id), 21, 12)
        )
    WHEN typeof(parent_id) = 'text'
         AND length(replace(parent_id, '-', '')) = 32
         AND lower(replace(parent_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(parent_id, '-', ''), 1, 8) || '-' ||
            substr(replace(parent_id, '-', ''), 9, 4) || '-' ||
            substr(replace(parent_id, '-', ''), 13, 4) || '-' ||
            substr(replace(parent_id, '-', ''), 17, 4) || '-' ||
            substr(replace(parent_id, '-', ''), 21, 12)
        )
    ELSE parent_id
END
WHERE parent_id IS NOT NULL
  AND (
      (typeof(parent_id) = 'blob' AND length(parent_id) = 16)
      OR
      (typeof(parent_id) = 'text'
       AND length(replace(parent_id, '-', '')) = 32
       AND lower(replace(parent_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE task_progress
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE task_progress
SET task_id = CASE
    WHEN typeof(task_id) = 'blob' AND length(task_id) = 16 THEN
        lower(
            substr(hex(task_id), 1, 8) || '-' ||
            substr(hex(task_id), 9, 4) || '-' ||
            substr(hex(task_id), 13, 4) || '-' ||
            substr(hex(task_id), 17, 4) || '-' ||
            substr(hex(task_id), 21, 12)
        )
    WHEN typeof(task_id) = 'text'
         AND length(replace(task_id, '-', '')) = 32
         AND lower(replace(task_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(task_id, '-', ''), 1, 8) || '-' ||
            substr(replace(task_id, '-', ''), 9, 4) || '-' ||
            substr(replace(task_id, '-', ''), 13, 4) || '-' ||
            substr(replace(task_id, '-', ''), 17, 4) || '-' ||
            substr(replace(task_id, '-', ''), 21, 12)
        )
    ELSE task_id
END
WHERE task_id IS NOT NULL
  AND (
      (typeof(task_id) = 'blob' AND length(task_id) = 16)
      OR
      (typeof(task_id) = 'text'
       AND length(replace(task_id, '-', '')) = 32
       AND lower(replace(task_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE task_progress
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE task_dependencies
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE task_dependencies
SET source_task_id = CASE
    WHEN typeof(source_task_id) = 'blob' AND length(source_task_id) = 16 THEN
        lower(
            substr(hex(source_task_id), 1, 8) || '-' ||
            substr(hex(source_task_id), 9, 4) || '-' ||
            substr(hex(source_task_id), 13, 4) || '-' ||
            substr(hex(source_task_id), 17, 4) || '-' ||
            substr(hex(source_task_id), 21, 12)
        )
    WHEN typeof(source_task_id) = 'text'
         AND length(replace(source_task_id, '-', '')) = 32
         AND lower(replace(source_task_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(source_task_id, '-', ''), 1, 8) || '-' ||
            substr(replace(source_task_id, '-', ''), 9, 4) || '-' ||
            substr(replace(source_task_id, '-', ''), 13, 4) || '-' ||
            substr(replace(source_task_id, '-', ''), 17, 4) || '-' ||
            substr(replace(source_task_id, '-', ''), 21, 12)
        )
    ELSE source_task_id
END
WHERE source_task_id IS NOT NULL
  AND (
      (typeof(source_task_id) = 'blob' AND length(source_task_id) = 16)
      OR
      (typeof(source_task_id) = 'text'
       AND length(replace(source_task_id, '-', '')) = 32
       AND lower(replace(source_task_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE task_dependencies
SET target_task_id = CASE
    WHEN typeof(target_task_id) = 'blob' AND length(target_task_id) = 16 THEN
        lower(
            substr(hex(target_task_id), 1, 8) || '-' ||
            substr(hex(target_task_id), 9, 4) || '-' ||
            substr(hex(target_task_id), 13, 4) || '-' ||
            substr(hex(target_task_id), 17, 4) || '-' ||
            substr(hex(target_task_id), 21, 12)
        )
    WHEN typeof(target_task_id) = 'text'
         AND length(replace(target_task_id, '-', '')) = 32
         AND lower(replace(target_task_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(target_task_id, '-', ''), 1, 8) || '-' ||
            substr(replace(target_task_id, '-', ''), 9, 4) || '-' ||
            substr(replace(target_task_id, '-', ''), 13, 4) || '-' ||
            substr(replace(target_task_id, '-', ''), 17, 4) || '-' ||
            substr(replace(target_task_id, '-', ''), 21, 12)
        )
    ELSE target_task_id
END
WHERE target_task_id IS NOT NULL
  AND (
      (typeof(target_task_id) = 'blob' AND length(target_task_id) = 16)
      OR
      (typeof(target_task_id) = 'text'
       AND length(replace(target_task_id, '-', '')) = 32
       AND lower(replace(target_task_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_plan
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_plan
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE activity_log
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE activity_log
SET actor_id = CASE
    WHEN typeof(actor_id) = 'blob' AND length(actor_id) = 16 THEN
        lower(
            substr(hex(actor_id), 1, 8) || '-' ||
            substr(hex(actor_id), 9, 4) || '-' ||
            substr(hex(actor_id), 13, 4) || '-' ||
            substr(hex(actor_id), 17, 4) || '-' ||
            substr(hex(actor_id), 21, 12)
        )
    WHEN typeof(actor_id) = 'text'
         AND length(replace(actor_id, '-', '')) = 32
         AND lower(replace(actor_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(actor_id, '-', ''), 1, 8) || '-' ||
            substr(replace(actor_id, '-', ''), 9, 4) || '-' ||
            substr(replace(actor_id, '-', ''), 13, 4) || '-' ||
            substr(replace(actor_id, '-', ''), 17, 4) || '-' ||
            substr(replace(actor_id, '-', ''), 21, 12)
        )
    ELSE actor_id
END
WHERE actor_id IS NOT NULL
  AND (
      (typeof(actor_id) = 'blob' AND length(actor_id) = 16)
      OR
      (typeof(actor_id) = 'text'
       AND length(replace(actor_id, '-', '')) = 32
       AND lower(replace(actor_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE activity_log
SET subject_id = CASE
    WHEN typeof(subject_id) = 'blob' AND length(subject_id) = 16 THEN
        lower(
            substr(hex(subject_id), 1, 8) || '-' ||
            substr(hex(subject_id), 9, 4) || '-' ||
            substr(hex(subject_id), 13, 4) || '-' ||
            substr(hex(subject_id), 17, 4) || '-' ||
            substr(hex(subject_id), 21, 12)
        )
    WHEN typeof(subject_id) = 'text'
         AND length(replace(subject_id, '-', '')) = 32
         AND lower(replace(subject_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(subject_id, '-', ''), 1, 8) || '-' ||
            substr(replace(subject_id, '-', ''), 9, 4) || '-' ||
            substr(replace(subject_id, '-', ''), 13, 4) || '-' ||
            substr(replace(subject_id, '-', ''), 17, 4) || '-' ||
            substr(replace(subject_id, '-', ''), 21, 12)
        )
    ELSE subject_id
END
WHERE subject_id IS NOT NULL
  AND (
      (typeof(subject_id) = 'blob' AND length(subject_id) = 16)
      OR
      (typeof(subject_id) = 'text'
       AND length(replace(subject_id, '-', '')) = 32
       AND lower(replace(subject_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE event_store
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE event_store
SET actor_id = CASE
    WHEN typeof(actor_id) = 'blob' AND length(actor_id) = 16 THEN
        lower(
            substr(hex(actor_id), 1, 8) || '-' ||
            substr(hex(actor_id), 9, 4) || '-' ||
            substr(hex(actor_id), 13, 4) || '-' ||
            substr(hex(actor_id), 17, 4) || '-' ||
            substr(hex(actor_id), 21, 12)
        )
    WHEN typeof(actor_id) = 'text'
         AND length(replace(actor_id, '-', '')) = 32
         AND lower(replace(actor_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(actor_id, '-', ''), 1, 8) || '-' ||
            substr(replace(actor_id, '-', ''), 9, 4) || '-' ||
            substr(replace(actor_id, '-', ''), 13, 4) || '-' ||
            substr(replace(actor_id, '-', ''), 17, 4) || '-' ||
            substr(replace(actor_id, '-', ''), 21, 12)
        )
    ELSE actor_id
END
WHERE actor_id IS NOT NULL
  AND (
      (typeof(actor_id) = 'blob' AND length(actor_id) = 16)
      OR
      (typeof(actor_id) = 'text'
       AND length(replace(actor_id, '-', '')) = 32
       AND lower(replace(actor_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE event_store
SET subject_id = CASE
    WHEN typeof(subject_id) = 'blob' AND length(subject_id) = 16 THEN
        lower(
            substr(hex(subject_id), 1, 8) || '-' ||
            substr(hex(subject_id), 9, 4) || '-' ||
            substr(hex(subject_id), 13, 4) || '-' ||
            substr(hex(subject_id), 17, 4) || '-' ||
            substr(hex(subject_id), 21, 12)
        )
    WHEN typeof(subject_id) = 'text'
         AND length(replace(subject_id, '-', '')) = 32
         AND lower(replace(subject_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(subject_id, '-', ''), 1, 8) || '-' ||
            substr(replace(subject_id, '-', ''), 9, 4) || '-' ||
            substr(replace(subject_id, '-', ''), 13, 4) || '-' ||
            substr(replace(subject_id, '-', ''), 17, 4) || '-' ||
            substr(replace(subject_id, '-', ''), 21, 12)
        )
    ELSE subject_id
END
WHERE subject_id IS NOT NULL
  AND (
      (typeof(subject_id) = 'blob' AND length(subject_id) = 16)
      OR
      (typeof(subject_id) = 'text'
       AND length(replace(subject_id, '-', '')) = 32
       AND lower(replace(subject_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE roles
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE permissions
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE role_permissions
SET role_id = CASE
    WHEN typeof(role_id) = 'blob' AND length(role_id) = 16 THEN
        lower(
            substr(hex(role_id), 1, 8) || '-' ||
            substr(hex(role_id), 9, 4) || '-' ||
            substr(hex(role_id), 13, 4) || '-' ||
            substr(hex(role_id), 17, 4) || '-' ||
            substr(hex(role_id), 21, 12)
        )
    WHEN typeof(role_id) = 'text'
         AND length(replace(role_id, '-', '')) = 32
         AND lower(replace(role_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(role_id, '-', ''), 1, 8) || '-' ||
            substr(replace(role_id, '-', ''), 9, 4) || '-' ||
            substr(replace(role_id, '-', ''), 13, 4) || '-' ||
            substr(replace(role_id, '-', ''), 17, 4) || '-' ||
            substr(replace(role_id, '-', ''), 21, 12)
        )
    ELSE role_id
END
WHERE role_id IS NOT NULL
  AND (
      (typeof(role_id) = 'blob' AND length(role_id) = 16)
      OR
      (typeof(role_id) = 'text'
       AND length(replace(role_id, '-', '')) = 32
       AND lower(replace(role_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE role_permissions
SET permission_id = CASE
    WHEN typeof(permission_id) = 'blob' AND length(permission_id) = 16 THEN
        lower(
            substr(hex(permission_id), 1, 8) || '-' ||
            substr(hex(permission_id), 9, 4) || '-' ||
            substr(hex(permission_id), 13, 4) || '-' ||
            substr(hex(permission_id), 17, 4) || '-' ||
            substr(hex(permission_id), 21, 12)
        )
    WHEN typeof(permission_id) = 'text'
         AND length(replace(permission_id, '-', '')) = 32
         AND lower(replace(permission_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(permission_id, '-', ''), 1, 8) || '-' ||
            substr(replace(permission_id, '-', ''), 9, 4) || '-' ||
            substr(replace(permission_id, '-', ''), 13, 4) || '-' ||
            substr(replace(permission_id, '-', ''), 17, 4) || '-' ||
            substr(replace(permission_id, '-', ''), 21, 12)
        )
    ELSE permission_id
END
WHERE permission_id IS NOT NULL
  AND (
      (typeof(permission_id) = 'blob' AND length(permission_id) = 16)
      OR
      (typeof(permission_id) = 'text'
       AND length(replace(permission_id, '-', '')) = 32
       AND lower(replace(permission_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE user_roles
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE user_roles
SET role_id = CASE
    WHEN typeof(role_id) = 'blob' AND length(role_id) = 16 THEN
        lower(
            substr(hex(role_id), 1, 8) || '-' ||
            substr(hex(role_id), 9, 4) || '-' ||
            substr(hex(role_id), 13, 4) || '-' ||
            substr(hex(role_id), 17, 4) || '-' ||
            substr(hex(role_id), 21, 12)
        )
    WHEN typeof(role_id) = 'text'
         AND length(replace(role_id, '-', '')) = 32
         AND lower(replace(role_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(role_id, '-', ''), 1, 8) || '-' ||
            substr(replace(role_id, '-', ''), 9, 4) || '-' ||
            substr(replace(role_id, '-', ''), 13, 4) || '-' ||
            substr(replace(role_id, '-', ''), 17, 4) || '-' ||
            substr(replace(role_id, '-', ''), 21, 12)
        )
    ELSE role_id
END
WHERE role_id IS NOT NULL
  AND (
      (typeof(role_id) = 'blob' AND length(role_id) = 16)
      OR
      (typeof(role_id) = 'text'
       AND length(replace(role_id, '-', '')) = 32
       AND lower(replace(role_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE user_permissions
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE user_permissions
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE user_permissions
SET permission_id = CASE
    WHEN typeof(permission_id) = 'blob' AND length(permission_id) = 16 THEN
        lower(
            substr(hex(permission_id), 1, 8) || '-' ||
            substr(hex(permission_id), 9, 4) || '-' ||
            substr(hex(permission_id), 13, 4) || '-' ||
            substr(hex(permission_id), 17, 4) || '-' ||
            substr(hex(permission_id), 21, 12)
        )
    WHEN typeof(permission_id) = 'text'
         AND length(replace(permission_id, '-', '')) = 32
         AND lower(replace(permission_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(permission_id, '-', ''), 1, 8) || '-' ||
            substr(replace(permission_id, '-', ''), 9, 4) || '-' ||
            substr(replace(permission_id, '-', ''), 13, 4) || '-' ||
            substr(replace(permission_id, '-', ''), 17, 4) || '-' ||
            substr(replace(permission_id, '-', ''), 21, 12)
        )
    ELSE permission_id
END
WHERE permission_id IS NOT NULL
  AND (
      (typeof(permission_id) = 'blob' AND length(permission_id) = 16)
      OR
      (typeof(permission_id) = 'text'
       AND length(replace(permission_id, '-', '')) = 32
       AND lower(replace(permission_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE route_permissions
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE password_reset_tokens
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE password_reset_tokens
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE resource_roles
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET access_role_id = CASE
    WHEN typeof(access_role_id) = 'blob' AND length(access_role_id) = 16 THEN
        lower(
            substr(hex(access_role_id), 1, 8) || '-' ||
            substr(hex(access_role_id), 9, 4) || '-' ||
            substr(hex(access_role_id), 13, 4) || '-' ||
            substr(hex(access_role_id), 17, 4) || '-' ||
            substr(hex(access_role_id), 21, 12)
        )
    WHEN typeof(access_role_id) = 'text'
         AND length(replace(access_role_id, '-', '')) = 32
         AND lower(replace(access_role_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(access_role_id, '-', ''), 1, 8) || '-' ||
            substr(replace(access_role_id, '-', ''), 9, 4) || '-' ||
            substr(replace(access_role_id, '-', ''), 13, 4) || '-' ||
            substr(replace(access_role_id, '-', ''), 17, 4) || '-' ||
            substr(replace(access_role_id, '-', ''), 21, 12)
        )
    ELSE access_role_id
END
WHERE access_role_id IS NOT NULL
  AND (
      (typeof(access_role_id) = 'blob' AND length(access_role_id) = 16)
      OR
      (typeof(access_role_id) = 'text'
       AND length(replace(access_role_id, '-', '')) = 32
       AND lower(replace(access_role_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET created_by = CASE
    WHEN typeof(created_by) = 'blob' AND length(created_by) = 16 THEN
        lower(
            substr(hex(created_by), 1, 8) || '-' ||
            substr(hex(created_by), 9, 4) || '-' ||
            substr(hex(created_by), 13, 4) || '-' ||
            substr(hex(created_by), 17, 4) || '-' ||
            substr(hex(created_by), 21, 12)
        )
    WHEN typeof(created_by) = 'text'
         AND length(replace(created_by, '-', '')) = 32
         AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(created_by, '-', ''), 1, 8) || '-' ||
            substr(replace(created_by, '-', ''), 9, 4) || '-' ||
            substr(replace(created_by, '-', ''), 13, 4) || '-' ||
            substr(replace(created_by, '-', ''), 17, 4) || '-' ||
            substr(replace(created_by, '-', ''), 21, 12)
        )
    ELSE created_by
END
WHERE created_by IS NOT NULL
  AND (
      (typeof(created_by) = 'blob' AND length(created_by) = 16)
      OR
      (typeof(created_by) = 'text'
       AND length(replace(created_by, '-', '')) = 32
       AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET updated_by = CASE
    WHEN typeof(updated_by) = 'blob' AND length(updated_by) = 16 THEN
        lower(
            substr(hex(updated_by), 1, 8) || '-' ||
            substr(hex(updated_by), 9, 4) || '-' ||
            substr(hex(updated_by), 13, 4) || '-' ||
            substr(hex(updated_by), 17, 4) || '-' ||
            substr(hex(updated_by), 21, 12)
        )
    WHEN typeof(updated_by) = 'text'
         AND length(replace(updated_by, '-', '')) = 32
         AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(updated_by, '-', ''), 1, 8) || '-' ||
            substr(replace(updated_by, '-', ''), 9, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 13, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 17, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 21, 12)
        )
    ELSE updated_by
END
WHERE updated_by IS NOT NULL
  AND (
      (typeof(updated_by) = 'blob' AND length(updated_by) = 16)
      OR
      (typeof(updated_by) = 'text'
       AND length(replace(updated_by, '-', '')) = 32
       AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_members
SET deleted_by = CASE
    WHEN typeof(deleted_by) = 'blob' AND length(deleted_by) = 16 THEN
        lower(
            substr(hex(deleted_by), 1, 8) || '-' ||
            substr(hex(deleted_by), 9, 4) || '-' ||
            substr(hex(deleted_by), 13, 4) || '-' ||
            substr(hex(deleted_by), 17, 4) || '-' ||
            substr(hex(deleted_by), 21, 12)
        )
    WHEN typeof(deleted_by) = 'text'
         AND length(replace(deleted_by, '-', '')) = 32
         AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(deleted_by, '-', ''), 1, 8) || '-' ||
            substr(replace(deleted_by, '-', ''), 9, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 13, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 17, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 21, 12)
        )
    ELSE deleted_by
END
WHERE deleted_by IS NOT NULL
  AND (
      (typeof(deleted_by) = 'blob' AND length(deleted_by) = 16)
      OR
      (typeof(deleted_by) = 'text'
       AND length(replace(deleted_by, '-', '')) = 32
       AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_member_resource_roles
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_member_resource_roles
SET membership_id = CASE
    WHEN typeof(membership_id) = 'blob' AND length(membership_id) = 16 THEN
        lower(
            substr(hex(membership_id), 1, 8) || '-' ||
            substr(hex(membership_id), 9, 4) || '-' ||
            substr(hex(membership_id), 13, 4) || '-' ||
            substr(hex(membership_id), 17, 4) || '-' ||
            substr(hex(membership_id), 21, 12)
        )
    WHEN typeof(membership_id) = 'text'
         AND length(replace(membership_id, '-', '')) = 32
         AND lower(replace(membership_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(membership_id, '-', ''), 1, 8) || '-' ||
            substr(replace(membership_id, '-', ''), 9, 4) || '-' ||
            substr(replace(membership_id, '-', ''), 13, 4) || '-' ||
            substr(replace(membership_id, '-', ''), 17, 4) || '-' ||
            substr(replace(membership_id, '-', ''), 21, 12)
        )
    ELSE membership_id
END
WHERE membership_id IS NOT NULL
  AND (
      (typeof(membership_id) = 'blob' AND length(membership_id) = 16)
      OR
      (typeof(membership_id) = 'text'
       AND length(replace(membership_id, '-', '')) = 32
       AND lower(replace(membership_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_member_resource_roles
SET resource_role_id = CASE
    WHEN typeof(resource_role_id) = 'blob' AND length(resource_role_id) = 16 THEN
        lower(
            substr(hex(resource_role_id), 1, 8) || '-' ||
            substr(hex(resource_role_id), 9, 4) || '-' ||
            substr(hex(resource_role_id), 13, 4) || '-' ||
            substr(hex(resource_role_id), 17, 4) || '-' ||
            substr(hex(resource_role_id), 21, 12)
        )
    WHEN typeof(resource_role_id) = 'text'
         AND length(replace(resource_role_id, '-', '')) = 32
         AND lower(replace(resource_role_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(resource_role_id, '-', ''), 1, 8) || '-' ||
            substr(replace(resource_role_id, '-', ''), 9, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 13, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 17, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 21, 12)
        )
    ELSE resource_role_id
END
WHERE resource_role_id IS NOT NULL
  AND (
      (typeof(resource_role_id) = 'blob' AND length(resource_role_id) = 16)
      OR
      (typeof(resource_role_id) = 'text'
       AND length(replace(resource_role_id, '-', '')) = 32
       AND lower(replace(resource_role_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_member_resource_roles
SET created_by = CASE
    WHEN typeof(created_by) = 'blob' AND length(created_by) = 16 THEN
        lower(
            substr(hex(created_by), 1, 8) || '-' ||
            substr(hex(created_by), 9, 4) || '-' ||
            substr(hex(created_by), 13, 4) || '-' ||
            substr(hex(created_by), 17, 4) || '-' ||
            substr(hex(created_by), 21, 12)
        )
    WHEN typeof(created_by) = 'text'
         AND length(replace(created_by, '-', '')) = 32
         AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(created_by, '-', ''), 1, 8) || '-' ||
            substr(replace(created_by, '-', ''), 9, 4) || '-' ||
            substr(replace(created_by, '-', ''), 13, 4) || '-' ||
            substr(replace(created_by, '-', ''), 17, 4) || '-' ||
            substr(replace(created_by, '-', ''), 21, 12)
        )
    ELSE created_by
END
WHERE created_by IS NOT NULL
  AND (
      (typeof(created_by) = 'blob' AND length(created_by) = 16)
      OR
      (typeof(created_by) = 'text'
       AND length(replace(created_by, '-', '')) = 32
       AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_member_resource_roles
SET updated_by = CASE
    WHEN typeof(updated_by) = 'blob' AND length(updated_by) = 16 THEN
        lower(
            substr(hex(updated_by), 1, 8) || '-' ||
            substr(hex(updated_by), 9, 4) || '-' ||
            substr(hex(updated_by), 13, 4) || '-' ||
            substr(hex(updated_by), 17, 4) || '-' ||
            substr(hex(updated_by), 21, 12)
        )
    WHEN typeof(updated_by) = 'text'
         AND length(replace(updated_by, '-', '')) = 32
         AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(updated_by, '-', ''), 1, 8) || '-' ||
            substr(replace(updated_by, '-', ''), 9, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 13, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 17, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 21, 12)
        )
    ELSE updated_by
END
WHERE updated_by IS NOT NULL
  AND (
      (typeof(updated_by) = 'blob' AND length(updated_by) = 16)
      OR
      (typeof(updated_by) = 'text'
       AND length(replace(updated_by, '-', '')) = 32
       AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_member_resource_roles
SET deleted_by = CASE
    WHEN typeof(deleted_by) = 'blob' AND length(deleted_by) = 16 THEN
        lower(
            substr(hex(deleted_by), 1, 8) || '-' ||
            substr(hex(deleted_by), 9, 4) || '-' ||
            substr(hex(deleted_by), 13, 4) || '-' ||
            substr(hex(deleted_by), 17, 4) || '-' ||
            substr(hex(deleted_by), 21, 12)
        )
    WHEN typeof(deleted_by) = 'text'
         AND length(replace(deleted_by, '-', '')) = 32
         AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(deleted_by, '-', ''), 1, 8) || '-' ||
            substr(replace(deleted_by, '-', ''), 9, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 13, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 17, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 21, 12)
        )
    ELSE deleted_by
END
WHERE deleted_by IS NOT NULL
  AND (
      (typeof(deleted_by) = 'blob' AND length(deleted_by) = 16)
      OR
      (typeof(deleted_by) = 'text'
       AND length(replace(deleted_by, '-', '')) = 32
       AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_resource_role_rates
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_resource_role_rates
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_resource_role_rates
SET resource_role_id = CASE
    WHEN typeof(resource_role_id) = 'blob' AND length(resource_role_id) = 16 THEN
        lower(
            substr(hex(resource_role_id), 1, 8) || '-' ||
            substr(hex(resource_role_id), 9, 4) || '-' ||
            substr(hex(resource_role_id), 13, 4) || '-' ||
            substr(hex(resource_role_id), 17, 4) || '-' ||
            substr(hex(resource_role_id), 21, 12)
        )
    WHEN typeof(resource_role_id) = 'text'
         AND length(replace(resource_role_id, '-', '')) = 32
         AND lower(replace(resource_role_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(resource_role_id, '-', ''), 1, 8) || '-' ||
            substr(replace(resource_role_id, '-', ''), 9, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 13, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 17, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 21, 12)
        )
    ELSE resource_role_id
END
WHERE resource_role_id IS NOT NULL
  AND (
      (typeof(resource_role_id) = 'blob' AND length(resource_role_id) = 16)
      OR
      (typeof(resource_role_id) = 'text'
       AND length(replace(resource_role_id, '-', '')) = 32
       AND lower(replace(resource_role_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_resource_role_rates
SET created_by = CASE
    WHEN typeof(created_by) = 'blob' AND length(created_by) = 16 THEN
        lower(
            substr(hex(created_by), 1, 8) || '-' ||
            substr(hex(created_by), 9, 4) || '-' ||
            substr(hex(created_by), 13, 4) || '-' ||
            substr(hex(created_by), 17, 4) || '-' ||
            substr(hex(created_by), 21, 12)
        )
    WHEN typeof(created_by) = 'text'
         AND length(replace(created_by, '-', '')) = 32
         AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(created_by, '-', ''), 1, 8) || '-' ||
            substr(replace(created_by, '-', ''), 9, 4) || '-' ||
            substr(replace(created_by, '-', ''), 13, 4) || '-' ||
            substr(replace(created_by, '-', ''), 17, 4) || '-' ||
            substr(replace(created_by, '-', ''), 21, 12)
        )
    ELSE created_by
END
WHERE created_by IS NOT NULL
  AND (
      (typeof(created_by) = 'blob' AND length(created_by) = 16)
      OR
      (typeof(created_by) = 'text'
       AND length(replace(created_by, '-', '')) = 32
       AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_resource_role_rates
SET updated_by = CASE
    WHEN typeof(updated_by) = 'blob' AND length(updated_by) = 16 THEN
        lower(
            substr(hex(updated_by), 1, 8) || '-' ||
            substr(hex(updated_by), 9, 4) || '-' ||
            substr(hex(updated_by), 13, 4) || '-' ||
            substr(hex(updated_by), 17, 4) || '-' ||
            substr(hex(updated_by), 21, 12)
        )
    WHEN typeof(updated_by) = 'text'
         AND length(replace(updated_by, '-', '')) = 32
         AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(updated_by, '-', ''), 1, 8) || '-' ||
            substr(replace(updated_by, '-', ''), 9, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 13, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 17, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 21, 12)
        )
    ELSE updated_by
END
WHERE updated_by IS NOT NULL
  AND (
      (typeof(updated_by) = 'blob' AND length(updated_by) = 16)
      OR
      (typeof(updated_by) = 'text'
       AND length(replace(updated_by, '-', '')) = 32
       AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE project_resource_role_rates
SET deleted_by = CASE
    WHEN typeof(deleted_by) = 'blob' AND length(deleted_by) = 16 THEN
        lower(
            substr(hex(deleted_by), 1, 8) || '-' ||
            substr(hex(deleted_by), 9, 4) || '-' ||
            substr(hex(deleted_by), 13, 4) || '-' ||
            substr(hex(deleted_by), 17, 4) || '-' ||
            substr(hex(deleted_by), 21, 12)
        )
    WHEN typeof(deleted_by) = 'text'
         AND length(replace(deleted_by, '-', '')) = 32
         AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(deleted_by, '-', ''), 1, 8) || '-' ||
            substr(replace(deleted_by, '-', ''), 9, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 13, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 17, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 21, 12)
        )
    ELSE deleted_by
END
WHERE deleted_by IS NOT NULL
  AND (
      (typeof(deleted_by) = 'blob' AND length(deleted_by) = 16)
      OR
      (typeof(deleted_by) = 'text'
       AND length(replace(deleted_by, '-', '')) = 32
       AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET task_id = CASE
    WHEN typeof(task_id) = 'blob' AND length(task_id) = 16 THEN
        lower(
            substr(hex(task_id), 1, 8) || '-' ||
            substr(hex(task_id), 9, 4) || '-' ||
            substr(hex(task_id), 13, 4) || '-' ||
            substr(hex(task_id), 17, 4) || '-' ||
            substr(hex(task_id), 21, 12)
        )
    WHEN typeof(task_id) = 'text'
         AND length(replace(task_id, '-', '')) = 32
         AND lower(replace(task_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(task_id, '-', ''), 1, 8) || '-' ||
            substr(replace(task_id, '-', ''), 9, 4) || '-' ||
            substr(replace(task_id, '-', ''), 13, 4) || '-' ||
            substr(replace(task_id, '-', ''), 17, 4) || '-' ||
            substr(replace(task_id, '-', ''), 21, 12)
        )
    ELSE task_id
END
WHERE task_id IS NOT NULL
  AND (
      (typeof(task_id) = 'blob' AND length(task_id) = 16)
      OR
      (typeof(task_id) = 'text'
       AND length(replace(task_id, '-', '')) = 32
       AND lower(replace(task_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET resource_role_id = CASE
    WHEN typeof(resource_role_id) = 'blob' AND length(resource_role_id) = 16 THEN
        lower(
            substr(hex(resource_role_id), 1, 8) || '-' ||
            substr(hex(resource_role_id), 9, 4) || '-' ||
            substr(hex(resource_role_id), 13, 4) || '-' ||
            substr(hex(resource_role_id), 17, 4) || '-' ||
            substr(hex(resource_role_id), 21, 12)
        )
    WHEN typeof(resource_role_id) = 'text'
         AND length(replace(resource_role_id, '-', '')) = 32
         AND lower(replace(resource_role_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(resource_role_id, '-', ''), 1, 8) || '-' ||
            substr(replace(resource_role_id, '-', ''), 9, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 13, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 17, 4) || '-' ||
            substr(replace(resource_role_id, '-', ''), 21, 12)
        )
    ELSE resource_role_id
END
WHERE resource_role_id IS NOT NULL
  AND (
      (typeof(resource_role_id) = 'blob' AND length(resource_role_id) = 16)
      OR
      (typeof(resource_role_id) = 'text'
       AND length(replace(resource_role_id, '-', '')) = 32
       AND lower(replace(resource_role_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET created_by = CASE
    WHEN typeof(created_by) = 'blob' AND length(created_by) = 16 THEN
        lower(
            substr(hex(created_by), 1, 8) || '-' ||
            substr(hex(created_by), 9, 4) || '-' ||
            substr(hex(created_by), 13, 4) || '-' ||
            substr(hex(created_by), 17, 4) || '-' ||
            substr(hex(created_by), 21, 12)
        )
    WHEN typeof(created_by) = 'text'
         AND length(replace(created_by, '-', '')) = 32
         AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(created_by, '-', ''), 1, 8) || '-' ||
            substr(replace(created_by, '-', ''), 9, 4) || '-' ||
            substr(replace(created_by, '-', ''), 13, 4) || '-' ||
            substr(replace(created_by, '-', ''), 17, 4) || '-' ||
            substr(replace(created_by, '-', ''), 21, 12)
        )
    ELSE created_by
END
WHERE created_by IS NOT NULL
  AND (
      (typeof(created_by) = 'blob' AND length(created_by) = 16)
      OR
      (typeof(created_by) = 'text'
       AND length(replace(created_by, '-', '')) = 32
       AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET updated_by = CASE
    WHEN typeof(updated_by) = 'blob' AND length(updated_by) = 16 THEN
        lower(
            substr(hex(updated_by), 1, 8) || '-' ||
            substr(hex(updated_by), 9, 4) || '-' ||
            substr(hex(updated_by), 13, 4) || '-' ||
            substr(hex(updated_by), 17, 4) || '-' ||
            substr(hex(updated_by), 21, 12)
        )
    WHEN typeof(updated_by) = 'text'
         AND length(replace(updated_by, '-', '')) = 32
         AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(updated_by, '-', ''), 1, 8) || '-' ||
            substr(replace(updated_by, '-', ''), 9, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 13, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 17, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 21, 12)
        )
    ELSE updated_by
END
WHERE updated_by IS NOT NULL
  AND (
      (typeof(updated_by) = 'blob' AND length(updated_by) = 16)
      OR
      (typeof(updated_by) = 'text'
       AND length(replace(updated_by, '-', '')) = 32
       AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE work_logs
SET deleted_by = CASE
    WHEN typeof(deleted_by) = 'blob' AND length(deleted_by) = 16 THEN
        lower(
            substr(hex(deleted_by), 1, 8) || '-' ||
            substr(hex(deleted_by), 9, 4) || '-' ||
            substr(hex(deleted_by), 13, 4) || '-' ||
            substr(hex(deleted_by), 17, 4) || '-' ||
            substr(hex(deleted_by), 21, 12)
        )
    WHEN typeof(deleted_by) = 'text'
         AND length(replace(deleted_by, '-', '')) = 32
         AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(deleted_by, '-', ''), 1, 8) || '-' ||
            substr(replace(deleted_by, '-', ''), 9, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 13, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 17, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 21, 12)
        )
    ELSE deleted_by
END
WHERE deleted_by IS NOT NULL
  AND (
      (typeof(deleted_by) = 'blob' AND length(deleted_by) = 16)
      OR
      (typeof(deleted_by) = 'text'
       AND length(replace(deleted_by, '-', '')) = 32
       AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rule_sets
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rule_sets
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rule_sets
SET created_by = CASE
    WHEN typeof(created_by) = 'blob' AND length(created_by) = 16 THEN
        lower(
            substr(hex(created_by), 1, 8) || '-' ||
            substr(hex(created_by), 9, 4) || '-' ||
            substr(hex(created_by), 13, 4) || '-' ||
            substr(hex(created_by), 17, 4) || '-' ||
            substr(hex(created_by), 21, 12)
        )
    WHEN typeof(created_by) = 'text'
         AND length(replace(created_by, '-', '')) = 32
         AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(created_by, '-', ''), 1, 8) || '-' ||
            substr(replace(created_by, '-', ''), 9, 4) || '-' ||
            substr(replace(created_by, '-', ''), 13, 4) || '-' ||
            substr(replace(created_by, '-', ''), 17, 4) || '-' ||
            substr(replace(created_by, '-', ''), 21, 12)
        )
    ELSE created_by
END
WHERE created_by IS NOT NULL
  AND (
      (typeof(created_by) = 'blob' AND length(created_by) = 16)
      OR
      (typeof(created_by) = 'text'
       AND length(replace(created_by, '-', '')) = 32
       AND lower(replace(created_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rule_sets
SET updated_by = CASE
    WHEN typeof(updated_by) = 'blob' AND length(updated_by) = 16 THEN
        lower(
            substr(hex(updated_by), 1, 8) || '-' ||
            substr(hex(updated_by), 9, 4) || '-' ||
            substr(hex(updated_by), 13, 4) || '-' ||
            substr(hex(updated_by), 17, 4) || '-' ||
            substr(hex(updated_by), 21, 12)
        )
    WHEN typeof(updated_by) = 'text'
         AND length(replace(updated_by, '-', '')) = 32
         AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(updated_by, '-', ''), 1, 8) || '-' ||
            substr(replace(updated_by, '-', ''), 9, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 13, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 17, 4) || '-' ||
            substr(replace(updated_by, '-', ''), 21, 12)
        )
    ELSE updated_by
END
WHERE updated_by IS NOT NULL
  AND (
      (typeof(updated_by) = 'blob' AND length(updated_by) = 16)
      OR
      (typeof(updated_by) = 'text'
       AND length(replace(updated_by, '-', '')) = 32
       AND lower(replace(updated_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rule_sets
SET deleted_by = CASE
    WHEN typeof(deleted_by) = 'blob' AND length(deleted_by) = 16 THEN
        lower(
            substr(hex(deleted_by), 1, 8) || '-' ||
            substr(hex(deleted_by), 9, 4) || '-' ||
            substr(hex(deleted_by), 13, 4) || '-' ||
            substr(hex(deleted_by), 17, 4) || '-' ||
            substr(hex(deleted_by), 21, 12)
        )
    WHEN typeof(deleted_by) = 'text'
         AND length(replace(deleted_by, '-', '')) = 32
         AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(deleted_by, '-', ''), 1, 8) || '-' ||
            substr(replace(deleted_by, '-', ''), 9, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 13, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 17, 4) || '-' ||
            substr(replace(deleted_by, '-', ''), 21, 12)
        )
    ELSE deleted_by
END
WHERE deleted_by IS NOT NULL
  AND (
      (typeof(deleted_by) = 'blob' AND length(deleted_by) = 16)
      OR
      (typeof(deleted_by) = 'text'
       AND length(replace(deleted_by, '-', '')) = 32
       AND lower(replace(deleted_by, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rules
SET id = CASE
    WHEN typeof(id) = 'blob' AND length(id) = 16 THEN
        lower(
            substr(hex(id), 1, 8) || '-' ||
            substr(hex(id), 9, 4) || '-' ||
            substr(hex(id), 13, 4) || '-' ||
            substr(hex(id), 17, 4) || '-' ||
            substr(hex(id), 21, 12)
        )
    WHEN typeof(id) = 'text'
         AND length(replace(id, '-', '')) = 32
         AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(id, '-', ''), 1, 8) || '-' ||
            substr(replace(id, '-', ''), 9, 4) || '-' ||
            substr(replace(id, '-', ''), 13, 4) || '-' ||
            substr(replace(id, '-', ''), 17, 4) || '-' ||
            substr(replace(id, '-', ''), 21, 12)
        )
    ELSE id
END
WHERE id IS NOT NULL
  AND (
      (typeof(id) = 'blob' AND length(id) = 16)
      OR
      (typeof(id) = 'text'
       AND length(replace(id, '-', '')) = 32
       AND lower(replace(id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE s_curve_stage_rules
SET rule_set_id = CASE
    WHEN typeof(rule_set_id) = 'blob' AND length(rule_set_id) = 16 THEN
        lower(
            substr(hex(rule_set_id), 1, 8) || '-' ||
            substr(hex(rule_set_id), 9, 4) || '-' ||
            substr(hex(rule_set_id), 13, 4) || '-' ||
            substr(hex(rule_set_id), 17, 4) || '-' ||
            substr(hex(rule_set_id), 21, 12)
        )
    WHEN typeof(rule_set_id) = 'text'
         AND length(replace(rule_set_id, '-', '')) = 32
         AND lower(replace(rule_set_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(rule_set_id, '-', ''), 1, 8) || '-' ||
            substr(replace(rule_set_id, '-', ''), 9, 4) || '-' ||
            substr(replace(rule_set_id, '-', ''), 13, 4) || '-' ||
            substr(replace(rule_set_id, '-', ''), 17, 4) || '-' ||
            substr(replace(rule_set_id, '-', ''), 21, 12)
        )
    ELSE rule_set_id
END
WHERE rule_set_id IS NOT NULL
  AND (
      (typeof(rule_set_id) = 'blob' AND length(rule_set_id) = 16)
      OR
      (typeof(rule_set_id) = 'text'
       AND length(replace(rule_set_id, '-', '')) = 32
       AND lower(replace(rule_set_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE telemetry_events
SET user_id = CASE
    WHEN typeof(user_id) = 'blob' AND length(user_id) = 16 THEN
        lower(
            substr(hex(user_id), 1, 8) || '-' ||
            substr(hex(user_id), 9, 4) || '-' ||
            substr(hex(user_id), 13, 4) || '-' ||
            substr(hex(user_id), 17, 4) || '-' ||
            substr(hex(user_id), 21, 12)
        )
    WHEN typeof(user_id) = 'text'
         AND length(replace(user_id, '-', '')) = 32
         AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(user_id, '-', ''), 1, 8) || '-' ||
            substr(replace(user_id, '-', ''), 9, 4) || '-' ||
            substr(replace(user_id, '-', ''), 13, 4) || '-' ||
            substr(replace(user_id, '-', ''), 17, 4) || '-' ||
            substr(replace(user_id, '-', ''), 21, 12)
        )
    ELSE user_id
END
WHERE user_id IS NOT NULL
  AND (
      (typeof(user_id) = 'blob' AND length(user_id) = 16)
      OR
      (typeof(user_id) = 'text'
       AND length(replace(user_id, '-', '')) = 32
       AND lower(replace(user_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE telemetry_events
SET project_id = CASE
    WHEN typeof(project_id) = 'blob' AND length(project_id) = 16 THEN
        lower(
            substr(hex(project_id), 1, 8) || '-' ||
            substr(hex(project_id), 9, 4) || '-' ||
            substr(hex(project_id), 13, 4) || '-' ||
            substr(hex(project_id), 17, 4) || '-' ||
            substr(hex(project_id), 21, 12)
        )
    WHEN typeof(project_id) = 'text'
         AND length(replace(project_id, '-', '')) = 32
         AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(project_id, '-', ''), 1, 8) || '-' ||
            substr(replace(project_id, '-', ''), 9, 4) || '-' ||
            substr(replace(project_id, '-', ''), 13, 4) || '-' ||
            substr(replace(project_id, '-', ''), 17, 4) || '-' ||
            substr(replace(project_id, '-', ''), 21, 12)
        )
    ELSE project_id
END
WHERE project_id IS NOT NULL
  AND (
      (typeof(project_id) = 'blob' AND length(project_id) = 16)
      OR
      (typeof(project_id) = 'text'
       AND length(replace(project_id, '-', '')) = 32
       AND lower(replace(project_id, '-', '')) GLOB '[0-9a-f]*')
  );

UPDATE telemetry_events
SET ingested_by = CASE
    WHEN typeof(ingested_by) = 'blob' AND length(ingested_by) = 16 THEN
        lower(
            substr(hex(ingested_by), 1, 8) || '-' ||
            substr(hex(ingested_by), 9, 4) || '-' ||
            substr(hex(ingested_by), 13, 4) || '-' ||
            substr(hex(ingested_by), 17, 4) || '-' ||
            substr(hex(ingested_by), 21, 12)
        )
    WHEN typeof(ingested_by) = 'text'
         AND length(replace(ingested_by, '-', '')) = 32
         AND lower(replace(ingested_by, '-', '')) GLOB '[0-9a-f]*' THEN
        lower(
            substr(replace(ingested_by, '-', ''), 1, 8) || '-' ||
            substr(replace(ingested_by, '-', ''), 9, 4) || '-' ||
            substr(replace(ingested_by, '-', ''), 13, 4) || '-' ||
            substr(replace(ingested_by, '-', ''), 17, 4) || '-' ||
            substr(replace(ingested_by, '-', ''), 21, 12)
        )
    ELSE ingested_by
END
WHERE ingested_by IS NOT NULL
  AND (
      (typeof(ingested_by) = 'blob' AND length(ingested_by) = 16)
      OR
      (typeof(ingested_by) = 'text'
       AND length(replace(ingested_by, '-', '')) = 32
       AND lower(replace(ingested_by, '-', '')) GLOB '[0-9a-f]*')
  );

PRAGMA defer_foreign_keys = OFF;
