# AI TimeShards — Vision & Feature Catalog

> **Planning:** Use **[ROADMAP.md](./ROADMAP.md)** for milestones and sprint priority.  
> **Reality:** Use **[STATUS.md](./STATUS.md)** and **[docs/FOUNDATION.md](./docs/FOUNDATION.md)** for what is already in the repo.

This document is the **long-range product picture**—features we may build, not a promise that every row is scheduled. Check the **Status** column against the active roadmap before starting work.

---

## Implementation snapshot (2026-06)

| Track | In repo today | Next (see ROADMAP.md) |
|-------|----------------|------------------------|
| Time foundation (calendar → Soll) | ✅ | M3: Feiertag + Umschaltplan UI |
| Stundenzettel + approve → Konten | ✅ | M2: UX polish |
| Access simulation + anti-passback | ✅ | M4: hardware pilot |
| Payroll CSV + Monats-Paket | ✅ | M5: DATEV feedback |
| Go-Live / production mode | ✅ | M1 done |
| Drag-and-drop shift planner | ⬜ partial | Schicht UI exists; not full DnD planner |
| Biometrics / visitor badges | ⬜ | Phase 2+ |
| PostgreSQL / SaaS | ⬜ | M6 deferred |

---

## 1. Vision & Product Identity

**AI TimeShards** is a cross-platform **desktop application** with a role-aware UI that should be **discoverable by exploration, powerful by design**—even for users new to time tracking or access control.

Two domains, one shell:

1. **Time** — attendance, calendars (Soll), shifts (planning), absences, compliance hints, payroll handoff.
2. **Access** — zones, doors, badges, rules, live occupancy, audit.

---

## 2. Target Platforms

| Platform | Tier | Notes |
|----------|------|-------|
| Windows 10/11 | **Primary** | Current CI and pilot focus |
| macOS 12+ | Primary | Tauri-supported; less daily CI |
| Linux (Ubuntu / RHEL) | Secondary | Enterprise installs |

Distribution: Tauri bundles (`.msi`, `.dmg`, `.AppImage` / `.rpm`). Auto-updater: planned, not required for v1 pilot.

---

## 3. UI/UX Philosophy

Aligned with **[docs/UI_UX_GUIDE.md](./docs/UI_UX_GUIDE.md)**:

| Principle | Implementation target |
|-----------|------------------------|
| Informative screens | Lead text + KPIs + empty states with CTA |
| Clickable density | Warnings and stats navigate to fix |
| Shared design system | CSS tokens + `Ts*` components (server + client) |
| Progressive disclosure | Advanced calendar tools in collapsible sections |
| Role-based nav | Server: tabs; Client: pillars (Zeit, Abwesenheit, …) |

### 3.1 Server navigation (current)

| Area | Purpose |
|------|---------|
| Übersicht | Login, KPIs, Go-Live, foundation-fix |
| Personal | Employees, calendar assignment |
| Zeit → Perioden & Soll | Tages-/Jahresperioden, MA-Zuordnung |
| Zeit → Schichtplanung | Templates + KW shifts (planning only) |
| Zeit → Stundenzettel / Abschluss | Approval, export, settlement |
| Abwesenheit | Requests + approve |
| Zutritt | Zones, doors, rules, simulate |
| System | Health, policy, audit |

### 3.2 Client pillars (current)

| Pillar | Purpose |
|--------|---------|
| Zeit | Clock, KW summary, own shifts |
| Abwesenheit | Request + status |
| Freigaben | Manager queue (role-gated) |
| Zutritt | Badge simulate, own events |
| Konto | Settings, logout |

### 3.3 Onboarding (status)

| Feature | Status |
|---------|--------|
| Demo seed + default logins | ✅ dev/demo |
| `TIMESHARDS_ADMIN_PASSWORD` on empty DB | ✅ |
| Production Go-Live wizard | ✅ |
| Full first-run wizard (company, import, badges) | ⬜ M3+ / PHASE2 |

### 3.4 Accessibility

| Feature | Status |
|---------|--------|
| Keyboard: real `<button>` elements | 🔄 improving |
| Focus visible | ⬜ tokens + `:focus-visible` |
| High-contrast theme | ⬜ |
| Full i18n | ⬜ German only today |

---

## 4. Feature Catalog: Time

| Feature | Status | Details |
|---------|--------|---------|
| Clock-in / out, breaks | ✅ | API + client; flex advisory/enforce |
| **Soll from calendar** | ✅ | Tagesperiode + Jahresperiode |
| Shift templates & KW shifts | ✅ | Planning; does not define Soll |
| Absence request + approve | ✅ | Credit on approved types |
| Overtime / flex accounts | ✅ | On timesheet approve |
| Month close + settlement preview | ✅ | |
| Timesheet HTML + Tagesdetails | ✅ | |
| Payroll CSV (Lohn + Abwesenheiten) | ✅ | [PAYROLL_EXPORT.md](./docs/PAYROLL_EXPORT.md) |
| Drag-and-drop shift planner | ⬜ | Recurring patterns, coverage alerts |
| Scheduled email reports | ⬜ | |
| Outlook/Google calendar sync | ⬜ | |
| ERP connectors | ⬜ | DATEV track in [DATEV.md](./docs/DATEV.md) |

---

## 5. Feature Catalog: Access

| Feature | Status | Details |
|---------|--------|---------|
| Zones, doors, rules | ✅ | Admin UI |
| Badge CRUD + simulate | ✅ | DEMO-* UIDs in dev |
| Anti-passback | ✅ | In/out readers |
| Live occupancy + door alerts | ✅ | Dashboard + Zutritt |
| Hardware-present channel | ✅ | Worker + poll events |
| External TCP ingest | ✅ partial | Bridge-friendly JSON/lines |
| Hierarchical zones (building→room) | ⬜ partial | Flat zones today |
| Mobile credentials | ⬜ | |
| Visitor temp badges | ⬜ | |
| Wiegand / OSDP in-process | ⬜ | M4+ |
| GDPR export/erase tooling | ⬜ partial | Audit log exists |

---

## 6. Architecture (as built)

| Layer | Technology | Notes |
|-------|------------|-------|
| Desktop shell | **Tauri 2** | `apps/server`, `apps/client` |
| UI | **Svelte 5** + global CSS | Not Tailwind/shadcn today—see UI guide |
| API | **Axum** `timeshards-api` | Port 47821; headless `npm run api` |
| DB | **SQLite** `timeshards-db` | Migrations + seed |
| Hardware | `timeshards-hardware` | `sim` \| `external` |
| Kernel sketch | `timeshards-kernel` | **Not** production path |

Optional later: PostgreSQL, embedded vs sidecar API—see [PHASE2.md](./docs/PHASE2.md).

---

## 7. Historical phase plan (superseded)

The Q1–Q5 checklist below was an **early estimate**. Use **[ROADMAP.md](./ROADMAP.md)** milestones M1–M6 instead.

<details>
<summary>Original phase checklist (archive)</summary>

### Phase 1: Foundation
- [x] Tauri + Svelte shell, sidebar nav
- [x] SQLite schema (users, zones, time, calendars)
- [x] Clock-in/out
- [ ] Full first-run wizard (partial: Go-Live only)

### Phase 2: Time core
- [x] Absence workflow
- [x] Timesheet export
- [ ] Full DnD shift planner
- [x] Overtime/flex via accounts (partial vs original “rule engine” vision)

### Phase 3: Access
- [x] Zones, rules, badges
- [x] Simulated door events
- [ ] Permission matrix UI at scale

### Phase 4: Hardware
- [ ] Wiegand/OSDP native
- [x] External TCP bridge (partial)
- [ ] Biometrics

### Phase 5: Enterprise
- [ ] PostgreSQL
- [ ] Multi-terminal sync
- [ ] ERP connectors

</details>

---

## 8. Open questions

| # | Question | Status | Notes |
|---|----------|--------|-------|
| 1 | Cloud SaaS vs on-prem only? | **TBD** | Default: on-prem desktop + LAN API |
| 2 | Biometric vendors? | TBD | After hardware pilot scope |
| 3 | Mobile companion? | TBD | Capacitor vs native vs none |
| 4 | Licensing (perpetual / sub / per-seat)? | TBD | Business decision |

Record answers in [ROADMAP.md § Decision log](./ROADMAP.md#6-decision-log).

---

## 9. Document map

| Doc | Use when |
|-----|----------|
| [ROADMAP.md](./ROADMAP.md) | Prioritizing next sprint |
| [STATUS.md](./STATUS.md) | Release / pilot status |
| [docs/UI_UX_GUIDE.md](./docs/UI_UX_GUIDE.md) | Designing or refactoring UI |
| [docs/PHASE2.md](./docs/PHASE2.md) | Post-v1 integration tracks |
| [ROADMAP.md Appendix A](./ROADMAP.md#appendix-a--platform-vision-deferred) | Platform/kernel curiosity only |
