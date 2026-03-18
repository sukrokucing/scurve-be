INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name)
VALUES
    ('60317130-0001-4000-8000-000000000001', '/notifications', 'GET', 'project.view'),
    ('60317130-0002-4000-8000-000000000001', '/notifications/unread-count', 'GET', 'project.view'),
    ('60317130-0003-4000-8000-000000000001', '/notifications/read', 'POST', 'project.view'),
    ('60317130-0004-4000-8000-000000000001', '/notifications/read-all', 'POST', 'project.view'),
    ('60317130-0005-4000-8000-000000000001', '/realtime/ws', 'GET', 'project.view');
