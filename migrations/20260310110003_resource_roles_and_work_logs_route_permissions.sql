-- Route-permission coverage for resource roles and work logs.

-- Remove stale route mappings if they exist from earlier prototypes.
DELETE FROM route_permissions
WHERE (route_pattern = '/projects/:project_id/resource-roles/:resource_role_id' AND method IN ('PUT', 'DELETE'))
   OR (route_pattern = '/projects/:project_id/tasks/:task_id/work-logs/:work_log_id');

INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name)
VALUES
  (lower(hex(randomblob(16))), '/resource-roles', 'GET', 'role.view'),
  (lower(hex(randomblob(16))), '/resource-roles', 'POST', 'role.manage'),
  (lower(hex(randomblob(16))), '/resource-roles/:id', 'PUT', 'role.manage'),
  (lower(hex(randomblob(16))), '/resource-roles/:id', 'DELETE', 'role.manage'),

  (lower(hex(randomblob(16))), '/projects/:project_id/resource-roles', 'GET', 'project.view'),
  (lower(hex(randomblob(16))), '/projects/:project_id/resource-roles/:resource_role_id/rate', 'PUT', 'project.update'),
  (lower(hex(randomblob(16))), '/projects/:project_id/resource-roles/:resource_role_id/rate', 'DELETE', 'project.update'),

  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/work-logs', 'GET', 'progress.view'),
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/work-logs', 'POST', 'progress.create'),
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/work-logs/:id', 'PUT', 'progress.create'),
  (lower(hex(randomblob(16))), '/projects/:project_id/tasks/:task_id/work-logs/:id', 'DELETE', 'progress.create');
