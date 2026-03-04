-- Permission and route mapping for telemetry ingestion endpoint.

INSERT OR IGNORE INTO permissions (id, name, description) VALUES
    ('10000000-0000-0000-0000-000000000051', 'telemetry.ingest', 'Ingest frontend telemetry events');

INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000051'),
    ('00000000-0000-0000-0000-000000000003', '10000000-0000-0000-0000-000000000051'),
    ('00000000-0000-0000-0000-000000000004', '10000000-0000-0000-0000-000000000051'),
    ('00000000-0000-0000-0000-000000000005', '10000000-0000-0000-0000-000000000051');

INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name)
VALUES
    (lower(hex(randomblob(16))), '/telemetry/events', 'POST', 'telemetry.ingest');
