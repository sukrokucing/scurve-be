| Task | Status | Expected | Actual | Variance | Health |

This structure is intentionally minimal so it can work in **engineering, marketing, HR, sales, operations, etc.**

The key is: each column must have a **clear definition and calculation rule**, otherwise teams will enter random values.

I'll explain **what each column means and how to define it properly.**

---

# 1. Task

### Purpose

Human-readable work item.

Represents **a unit of measurable progress**.

### How to Define It

A task should satisfy:

```text
1 task = a piece of work that produces a deliverable
```

Examples across industries:

| Industry    | Task Example                     |
| ----------- | -------------------------------- |
| Engineering | Build authentication API         |
| Marketing   | Launch social media campaign     |
| HR          | Hire backend engineer            |
| Sales       | Close enterprise deal            |
| Operations  | Implement new onboarding process |

---

# 2. Status

### Purpose

Shows **the lifecycle stage of the task**, not performance.

Status answers:

> "What state is the task currently in?"

### Recommended Status Set

| Status      | Meaning              |
| ----------- | -------------------- |
| Not Started | Work has not begun   |
| In Progress | Work has started     |
| Blocked     | Work cannot continue |
| Completed   | Task finished        |

Note: can be finish earlier or overdue

### How to Build It

Derived from task progress and flags:

```text
if progress == 0 → Not Started
if progress > 0 and progress < 100 → In Progress
if blocked_flag == true → Blocked
if progress == 100 → Completed
```
---

# 3. Expected

### Purpose

Represents **where the task should be today according to the plan**.

This is the **planned progress curve**.

### How to Calculate

Formula:

```
expected_progress =
elapsed_days / planned_duration_days
```

Then convert to percentage.

Example:

Task plan:

| Plan Duration | 10 days |
| ------------- | ------- |
| Days passed   | 6       |

Expected progress:

```
6 / 10 = 60%
```
---

# 4. Actual

### Purpose

Represents **real progress of the task**.

This must reflect **completed work**, not time passed.

### How to Define Progress

The best universal approach:

```text
progress = completed milestones / total milestones
```

Example task:

| Milestone      | Weight |
| -------------- | ------ |
| Design         | 30%    |
| Implementation | 40%    |
| Testing        | 20%    |
| Deployment     | 10%    |

If first two are done:

```
Actual = 30 + 40 = 70%
```

This works across industries because most work has **stages**.

Example sales pipeline:

| Stage       | Weight |
| ----------- | ------ |
| Lead        | 10%    |
| Qualified   | 30%    |
| Proposal    | 30%    |
| Negotiation | 20%    |
| Closed      | 10%    |

---

# 5. Variance

### Purpose

Shows **difference between expected progress and actual progress**.

Variance answers:

> "Are we ahead or behind schedule?"

### Formula

```
variance = actual_progress − expected_progress
```

Example:

| Expected | Actual |
| -------- | ------ |
| 60%      | 45%    |

```
Variance = -15%
```

Interpretation:

| Result   | Meaning     |
| -------- | ----------- |
| Positive | ahead       |
| Zero     | on schedule |
| Negative | behind      |

---

# 6. Health

### Purpose

Convert variance into **human-readable status**.

Humans understand **signals better than numbers**.

### Mapping Rule

| Variance    | Health      |
| ----------- | ----------- |
| ≥ +10%      | 🚀 Ahead    |
| -10% → +10% | 🟢 On Track |
| -10% → -25% | 🟡 At Risk  |
| < -25%      | 🔴 Critical |

Example:

| Variance | Health   |
| -------- | -------- |
| +12%     | 🚀 Ahead |
| +3%      | 🟢       |
| -15%     | 🟡       |
| -30%     | 🔴       |

---

# Example Final Table

| Task                    | Status      | Expected | Actual | Variance | Health |
| ----------------------- | ----------- | -------- | ------ | -------- | ------ |
| API Development         | In Progress | 60%      | 45%    | -15%     | 🟡     |
| UI Design               | In Progress | 40%      | 50%    | +10%     | 🚀     |
| Hiring Backend Engineer | In Progress | 30%      | 20%    | -10%     | 🟡     |
| Sales Enterprise Deal   | In Progress | 50%      | 70%    | +20%     | 🚀     |
| Campaign Launch         | Completed   | 100%     | 100%   | 0%       | 🟢     |

---

# How This Feeds the S-Curve

Your S-curve should use:

```
X axis → time
Y axis → cumulative project progress
```

Project progress is:

```
project_progress =
sum(task_weight × actual_progress)
```

Planned curve uses **expected_progress**.

Actual curve uses **actual_progress**.

---

# The Hidden Strength of This Model

This approach works because it separates:

```
time progress (expected)
work progress (actual)
```

And that separation exists in **every industry**.

---

**Key caveats**

* Actual progress must be tied to **milestone completion**, not arbitrary percentages.
* Expected progress assumes **linear scheduling**, which may not perfectly match real work distribution.
* Variance thresholds should remain **configurable per organization**.
