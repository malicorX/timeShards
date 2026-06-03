# AI TimeShards — Detailed Roadmap

## 1. Vision & Product Identity

**AI TimeShards** is a cross-platform **desktop application** with a **modular, graphical user interface** designed to be **immediately intuitive** — even for first-time users who have never used time-tracking or access-control software before.

Drawing direct inspiration from **Primion PrimeWeb**, the application unifies two traditionally separate domains under one desktop experience:

1. **Time Tracking** — employee attendance, shift planning, overtime rules, and reporting.
2. **Access Control** — physical/logical zone management, permission matrices, and badge/card-based entry control.

The guiding principle for the entire UI/UX is: **discoverable by exploration, powerful by design**.

---

## 2. Target Platforms

| Platform | Tier |
|----------|------|
| Windows 10/11 | Primary |
| macOS 12+ | Primary |
| Linux (Ubuntu & RedHat) | Secondary / Enterprise |

Distribution via standard OS installers (`.msi`, `.dmg`, `.AppImage` / `.rpm`).

---

## 3. UI/UX Philosophy

### 3.1 Modular Dashboard
- A **configurable widget-based home screen** that allows users to:
  - Rearrange modules (e.g. "My Time", "Zone Status", "Pending Approvals").
  - Hide/show entire modules based on role.
  - Choose between **compact** and **expanded** widget views.

### 3.2 Three-Pillar Navigation
The left sidebar groups all functionality into three intuitive pillars, visible to users based on their role:

| Pillar | Description |
|--------|-------------|
| **Time** | Clock-in/out, timesheets, schedules, absence/absence calendars, overtime |
| **Access** | Zones, badges, doors/gates, real-time occupancy, permission rules |
| **Admin** | Users, roles, reporting, audit logs, system configuration |

### 3.3 Guided Onboarding
- **First-run wizard**: configure company structure, import employees, assign badges.
- **Role-aware tooltips** that explain what each panel does on first visit.
- **Contextual help panel** (keyboard shortcut: `F1`) with searchable documentation.

### 3.4 Accessibility
- Full keyboard navigability.
- Screen-reader friendly ARIA labels.
- High-contrast and dyslexia-friendly themes.

---

## 4. Feature Deep-Dive: Time Tracking

| Feature | Details |
|---------|---------|
| **Clock-in / Clock-out** | NFC badge, RFID, mobile companion app, or manual |
| **Shift Scheduling** | Drag-and-drop planner, recurring patterns, coverage alerts |
| **Absence Management** | Vacation, sick leave, special leave with approval workflows |
| **Overtime Calculation** | Configurable rules (daily/weekly thresholds, multipliers) |
| **Break Rules** | Auto-detect, manual override, compliance tracking |
| **Reporting** | PDF/HTML export, scheduled email reports, analytics dashboard |
| **Integrations** | Payroll (Datev, Sage), calendars (Outlook, Google), ERP |

---

## 5. Feature Deep-Dive: Access Control (Primion-Style)

| Feature | Details |
|---------|---------|
| **Zones** | Hierarchical definition (Building → Floor → Room → Zone). Geofence + logical |
| **Permissions** | Time-based, role-based, and exception-based access matrices |
| **Badges / Credentials** | Smart cards, mobile tokens, biometrics (fingerprint, face) |
| **Doors & Controllers** | Hardware integration via REST/MQTT with controllers (e.g., Axis, HID) |
| **Live Monitoring** | Real-time door status, forced-open alarms, occupancy count |
| **Anti-Passback** | Prevent card sharing with strict entry/exit logic |
| **Visitor Management** | Temporary badges with auto-expiry and host notifications |
| **Audit & Compliance** | Immutable access logs, GDPR-compliant data handling |

---

## 6. Architecture & Technical Stack

### 6.1 Desktop Frontend

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| **Framework** | Tauri v2 + SvelteKit 5 | Native performance, tiny bundle, modern web-based UI |
| **UI Components** | Tailwind CSS + shadcn-svelte | Rapid, accessible, consistent styling |
| **State** | Svelte 5 Runes + TanStack Query | Reactive, minimal boilerplate |
| **Charts** | Chart.js or Victory | Responsive, accessible data viz |
| **i18n** | `svelte-i18n` | Multi-language from day one |
| **IPC** | Tauri Commands + Events | Secure bridge between Rust backend and JS frontend |

### 6.2 Backend & Runtime

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| **Core Runtime** | Rust (Tauri) | Memory-safe, fast, small footprint, easy distribution |
| **Local DB** | SQLite (via `sqlx` or `diesel`) | Zero-config, single-file, easy backups |
| **Enterprise DB** | PostgreSQL (optional) | Scale for multi-user enterprise server mode |
| **ORM / Query** | `sqlx` (async, compile-time checked) | Type safety, performance |
| **API Layer** | Axum (embedded in Tauri) | For local HTTP API, third-party integrations |
| **Background Jobs** | `tokio` tasks + local queue | Notifications, report generation, sync |

### 6.3 Hardware Integration

| Component | Protocol | Notes |
|-----------|----------|-------|
| Badge readers | Wiegand / OSDP² | Via USB/Serial to HID device handler |
| Door controllers | REST / MQTT / Modbus | Rust async clients (`rumqttc`, `reqwest`) |
| HID / USB | `rusb` / `hidapi` | Cross-platform Rust crates |
| Biometrics | Vendor SDK (C/C++) via FFI | Wrapped in safe Rust modules |

### 6.4 Build & Distribution

| Step | Tool |
|------|------|
| Bundling | Tauri CLI (`tauri build`) |
| CI/CD | GitHub Actions (matrix: Win/macOS/Linux) |
| Code signing | `signtool` (Win), `codesign` (macOS), `gpg` (Linux) |
| Auto-updater | Tauri Updater (GitHub Releases backend) |

---

## 7. Module Breakdown (Implementation Order)

### Phase 1: Foundation (Q1)
- [ ] Tauri + SvelteKit desktop shell with sidebar navigation
- [ ] SQLite schema for Users, Roles, Zones, Badges, TimeEntries
- [ ] Basic clock-in/out (manual UI, no hardware yet)
- [ ] Settings & first-run wizard

### Phase 2: Time Tracking Core (Q2)
- [ ] Shift scheduling UI (drag-and-drop calendar)
- [ ] Absence request & approval workflow
- [ ] Overtime rule engine
- [ ] Timesheet PDF export

### Phase 3: Access Control (Q3)
- [ ] Zone management (hierarchical editor)
- [ ] Permission matrix (role × zone × time)
- [ ] Badge management (CRUD, activation/deactivation)
- [ ] Simulated door events (UI test harness)

### Phase 4: Hardware Integration (Q4)
- [ ] Wiegand reader integration
- [ ] Real controller communication (MQTT/REST)
- [ ] Biometric enrollment & matching
- [ ] Anti-passback logic

### Phase 5: Enterprise Hardening (Q5)
- [ ] PostgreSQL backend option
- [ ] Multi-terminal synchronization
- [ ] Advanced audit & GDPR compliance tools
- [ ] Third-party ERP/payroll connectors

---

## 8. Related Documents

- `ROADMAP.md` — High-level project milestones and strategic goals (to be kept synchronized with this file).

---

## 9. Open Questions & Decisions

| # | Question | Status |
|---|----------|--------|
| 1 | Should we support cloud-hosted mode (SaaS) or strictly on-prem desktop? | TBD |
| 2 | Which biometric vendors to prioritize? (e.g., Suprema, Idemia) | TBD |
| 3 | Mobile companion app — native (Swift/Kotlin) or shared (Capacitor)? | TBD |
| 4 | Licensing model: perpetual, subscription, per-user? | TBD |

*Decision log should be updated as answers become available.*
