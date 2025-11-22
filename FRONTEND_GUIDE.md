# Frontend Integration Guide for S-Curve App

This document is designed to assist frontend developers (and AI agents) in building the client-side application for the S-Curve Project Management backend.

## 1. System Overview
*   **Base URL**: `https://localhost:8800` (Local Development)
*   **Authentication**: JWT (Bearer Token).
    *   Header: `Authorization: Bearer <token>`
    *   Token is obtained via `/auth/login` or `/auth/register`.
*   **Date Format**: ISO 8601 (`YYYY-MM-DDTHH:mm:ssZ`). All times are UTC.

## 2. Core Features & Business Logic

### A. Gantt Chart (Tasks & Dependencies)
The Gantt chart is built from **Tasks** and **Dependencies**.
*   **Hierarchy**: Tasks can have subtasks via `parent_id`.
*   **Dependencies**: Tasks can depend on others. Currently supports `finish_to_start` (Task B cannot start until Task A finishes).
*   **Batch Updates**: When dragging/dropping tasks in the Gantt view, use the **Batch Update** endpoint (`PUT /projects/:id/tasks/batch`) to save multiple changes (dates, progress) in a single transaction.

### B. S-Curve (Project Plan vs. Actual)
The S-Curve visualizes the project's progress over time.
*   **Planned Progress**: Defined by the Project Manager. It's a series of data points (Date, % Complete). Managed via `/projects/:id/plan`.
*   **Actual Progress**: Automatically calculated from the daily average progress of all tasks in the project.
*   **Dashboard**: The `/projects/:id/dashboard` endpoint returns everything needed for the main view:
    *   The Project details.
    *   The Plan (S-Curve baseline).
    *   The Actual progress (aggregated daily).

## 3. TypeScript Interfaces

Use these interfaces to ensure type safety with the backend.

```typescript
// --- Common ---
export type Uuid = string;
export type IsoDate = string; // e.g., "2025-10-01T09:00:00Z"

// --- Auth ---
export interface User {
  id: Uuid;
  name: string;
  email: string;
  provider: string;
  created_at: IsoDate;
}

export interface AuthResponse {
  token: string;
  user: User;
}

// --- Projects ---
export interface Project {
  id: Uuid;
  user_id: Uuid;
  name: string;
  description?: string;
  theme_color: string;
  created_at: IsoDate;
  updated_at: IsoDate;
}

export interface ProjectCreateRequest {
  name: string;
  description?: string;
  theme_color?: string;
}

// --- Tasks ---
export interface Task {
  id: Uuid;
  project_id: Uuid;
  title: string;
  status: 'pending' | 'in_progress' | 'completed' | string;
  due_date?: IsoDate;
  start_date?: IsoDate;
  end_date?: IsoDate;
  duration_days?: number;
  assignee?: Uuid;
  parent_id?: Uuid;
  progress: number; // 0-100
  created_at: IsoDate;
  updated_at: IsoDate;
}

export interface TaskCreateRequest {
  title: string;
  status?: string;
  due_date?: IsoDate;
  start_date?: IsoDate;
  end_date?: IsoDate;
  assignee?: Uuid;
  parent_id?: Uuid;
  progress?: number;
}

export interface TaskBatchUpdatePayload {
  tasks: TaskBatchUpdateRequest[];
}

export interface TaskBatchUpdateRequest {
  id: Uuid;
  title?: string;
  status?: string;
  due_date?: IsoDate;
  start_date?: IsoDate;
  end_date?: IsoDate;
  assignee?: Uuid;
  parent_id?: Uuid;
  progress?: number;
}

// --- Dependencies ---
export interface TaskDependency {
  id: Uuid;
  source_task_id: Uuid; // The predecessor
  target_task_id: Uuid; // The successor
  type: 'finish_to_start';
  created_at: IsoDate;
}

export interface DependencyCreateRequest {
  source_task_id: Uuid;
  target_task_id: Uuid;
  type?: 'finish_to_start';
}

// --- Project Plan (S-Curve) ---
export interface ProjectPlanPoint {
  id: Uuid;
  project_id: Uuid;
  date: IsoDate;
  planned_progress: number; // 0-100
}

export interface ProjectPlanCreateRequest {
  date: IsoDate;
  planned_progress: number;
}

// --- Dashboard ---
export interface ActualPoint {
  date: string; // YYYY-MM-DD (Simple date string from SQL)
  actual: number;
}

export interface DashboardResponse {
  project: Project;
  plan: ProjectPlanPoint[];
  actual: ActualPoint[];
}
```

## 4. API Interaction Patterns

### Fetching Project Data
To load the main project view, you typically need two calls:
1.  **Dashboard**: `GET /projects/:id/dashboard` -> Gets Project info + S-Curve data.
2.  **Tasks**: `GET /projects/:id/tasks` -> Gets the list of tasks for the Gantt chart.
3.  **Dependencies**: `GET /projects/:id/tasks/dependencies` -> Gets the links between tasks.

### Updating the Gantt Chart
When a user moves a task or group of tasks:
1.  Calculate the new dates on the frontend.
2.  Prepare a `TaskBatchUpdatePayload`.
3.  Call `PUT /projects/:id/tasks/batch`.
4.  Refetch or update local state.

### Managing Dependencies
To link Task A (Predecessor) to Task B (Successor):
1.  Call `POST /projects/:id/tasks/dependencies` with `{ source_task_id: A.id, target_task_id: B.id }`.
2.  The backend prevents cycles and self-links. Handle 400 errors gracefully.

### Editing the S-Curve Plan
The plan is a collection of points. To update it:
1.  The user defines a new set of milestones (Date, %).
2.  Send the **entire** new list to `POST /projects/:id/plan`.
3.  The backend replaces the old plan with the new one (transactional delete-insert).
