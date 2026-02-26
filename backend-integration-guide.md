# Backend Integration Guide: Audit Log API

This document outlines the required backend endpoints for the **Audit Log** feature in the RBAC Admin UI.

## Current State

The frontend has an "Audit Log" button in the Policy page (`/admin/policy`) that is currently non-functional. The button is intended to display a history of RBAC changes (role assignments, permission grants, etc.).

## Required Backend Endpoints

### 1. GET `/rbac/audit-logs`

**Description**: Returns a paginated list of audit log entries for RBAC-related actions.

**Authentication**: Bearer token required. Requires `audit.view` permission or admin role.

#### Query Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `page` | integer | No | Page number (1-based, default: 1) |
| `per_page` | integer | No | Items per page (default: 25, max: 100) |
| `action` | string | No | Filter by action type (e.g., `role.assign`, `permission.grant`) |
| `user_id` | uuid | No | Filter by target user ID |
| `actor_id` | uuid | No | Filter by actor (who performed the action) |
| `from` | datetime | No | Filter entries from this timestamp |
| `to` | datetime | No | Filter entries until this timestamp |

#### Response (200 OK)

```json
{
  "items": [
    {
      "id": "uuid",
      "action": "role.assign",
      "actor_id": "uuid",
      "actor_name": "Admin User",
      "target_user_id": "uuid",
      "target_user_name": "John Doe",
      "details": {
        "role_id": "uuid",
        "role_name": "Project Manager"
      },
      "created_at": "2025-12-20T15:00:00Z"
    }
  ],
  "total": 150,
  "page": 1,
  "per_page": 25
}
```

#### Response Headers

| Header | Description |
|--------|-------------|
| `X-Total-Count` | Total number of audit log entries matching the query |

### 2. Audit Log Entry Schema

```json
{
  "AuditLogEntry": {
    "properties": {
      "id": {
        "type": "string",
        "format": "uuid"
      },
      "action": {
        "type": "string",
        "enum": [
          "role.create",
          "role.update",
          "role.delete",
          "role.assign",
          "role.revoke",
          "permission.grant",
          "permission.revoke",
          "user.create",
          "user.update",
          "user.delete"
        ]
      },
      "actor_id": {
        "type": "string",
        "format": "uuid",
        "description": "The user who performed the action"
      },
      "actor_name": {
        "type": "string",
        "description": "Display name of the actor"
      },
      "target_user_id": {
        "type": "string",
        "format": "uuid",
        "nullable": true,
        "description": "The user affected by the action (if applicable)"
      },
      "target_user_name": {
        "type": "string",
        "nullable": true
      },
      "details": {
        "type": "object",
        "description": "Action-specific metadata (role_id, permission_id, etc.)"
      },
      "created_at": {
        "type": "string",
        "format": "date-time"
      }
    },
    "required": ["id", "action", "actor_id", "actor_name", "created_at"]
  }
}
```

## Frontend Integration Plan

Once the backend implements this endpoint, the frontend will:

1. **Add API client function** in `src/api/rbac.ts`:
   ```typescript
   listAuditLogs: async (params?: { page?: number; per_page?: number; action?: string }) => {
     const response = await axiosWithAuth.get('/rbac/audit-logs', { params });
     return response.data;
   }
   ```

2. **Create AuditLogDialog component** that displays:
   - Table with columns: Action, Actor, Target User, Details, Timestamp
   - Filter controls for action type and date range
   - Pagination controls

3. **Update PolicyPage.tsx** to open the dialog when "Audit Log" button is clicked.

## OpenAPI Schema Addition

Add the following to `openapi.json`:

```json
"/rbac/audit-logs": {
  "get": {
    "operationId": "list_audit_logs",
    "summary": "List RBAC audit logs",
    "description": "Returns paginated audit log entries for RBAC actions.",
    "tags": ["RBAC"],
    "security": [{ "bearerAuth": [] }],
    "parameters": [
      {
        "name": "page",
        "in": "query",
        "schema": { "type": "integer", "minimum": 1 }
      },
      {
        "name": "per_page",
        "in": "query",
        "schema": { "type": "integer", "minimum": 1, "maximum": 100 }
      },
      {
        "name": "action",
        "in": "query",
        "schema": { "type": "string" }
      }
    ],
    "responses": {
      "200": {
        "description": "Audit log entries",
        "headers": {
          "X-Total-Count": {
            "schema": { "type": "integer" }
          }
        },
        "content": {
          "application/json": {
            "schema": {
              "type": "object",
              "properties": {
                "items": {
                  "type": "array",
                  "items": { "$ref": "#/components/schemas/AuditLogEntry" }
                },
                "total": { "type": "integer" },
                "page": { "type": "integer" },
                "per_page": { "type": "integer" }
              }
            }
          }
        }
      }
    }
  }
}
```

## Implementation Priority

| Priority | Feature | Status |
|----------|---------|--------|
| P1 | Basic audit log endpoint with pagination | **Required** |
| P2 | Filter by action type | Nice to have |
| P3 | Filter by date range | Nice to have |
