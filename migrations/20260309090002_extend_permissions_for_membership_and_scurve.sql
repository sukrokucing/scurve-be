-- Route-permission coverage for project membership and S-curve health endpoints.

INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name)
VALUES
  (lower(hex(randomblob(16))), '/projects/:project_id/members', 'GET', 'project.view'),
  (lower(hex(randomblob(16))), '/projects/:project_id/members', 'POST', 'project.update'),
  (lower(hex(randomblob(16))), '/projects/:project_id/members/:user_id', 'DELETE', 'project.update'),
  (lower(hex(randomblob(16))), '/users/me/projects', 'GET', 'project.view'),
  (lower(hex(randomblob(16))), '/projects/:id/s-curve/health', 'GET', 'project.view'),
  (lower(hex(randomblob(16))), '/portfolio/s-curve/summary', 'GET', 'project.view');
