-- Extend route-permission map for newly added task endpoints.

INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name)
VALUES
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/batch', 'DELETE', 'task.delete'),
  (lower(hex(randomblob(16))), '/projects/:project_id/assignees', 'GET', 'task.view'),
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:id/activity', 'GET', 'task.view'),
  (lower(hex(randomblob(16))), '/tasks/:task_id/progress', 'GET', 'progress.view');
