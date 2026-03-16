INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name)
VALUES
  (lower(hex(randomblob(16))), '/projects/:project_id/task-health/rules', 'GET', 'project.view'),
  (lower(hex(randomblob(16))), '/projects/:project_id/task-health/rules', 'PUT', 'project.update'),
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:id/progress-components', 'GET', 'progress.view'),
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:id/progress-components', 'PUT', 'progress.create');
