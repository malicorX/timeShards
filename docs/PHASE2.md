# Phase 2 — after v1 time foundation

v1 is **shipped** — see [STATUS.md](../STATUS.md), [FOUNDATION.md](./FOUNDATION.md), and milestone **M1** in [ROADMAP.md](../ROADMAP.md).

Phase 2 items are **tracks**, not a fixed quarter plan. Priority is set in [ROADMAP.md](../ROADMAP.md) (M2–M6).

---

## Tracks ↔ roadmap milestones

| Track | Roadmap | Goal | Today |
|-------|---------|------|--------|
| **UI/UX** | M2 | Shared tokens, informative + clickable UI | [UI_UX_GUIDE.md](./UI_UX_GUIDE.md); Perioden UI partial |
| **Perioden** | M3 | Feiertag, Umschaltplan editor, full-year UX | KW + Jahr befüllen; POST calendars/models |
| **Hardware** | M4 | Production reader via external bridge | `sim` + `external`; [HARDWARE.md](./HARDWARE.md) |
| **DATEV / Lohn** | M5 | Bureau-validated export | Lohn- + Abwesenheiten-CSV; [DATEV.md](./DATEV.md) draft |
| **First-run setup** | M3+ | Empty-DB onboarding beyond seed | Admin password + Go-Live wizard |
| **Stamp ↔ door** | — | Optional auto-sync or stricter alerts | Dashboard KPI only |
| **Enterprise DB** | M6 | PostgreSQL, multi-site | SQLite only |

---

## Suggested order

1. **M2** — UX foundation (tokens, client Zeit pillar, empty states).
2. **Pilot** on v1 — demo off, [PILOT.md](./PILOT.md), one payroll month.
3. **M3** — HR self-service for calendars (Feiertag, rotation editor).
4. **M5** — Payroll bureau feedback → update DATEV.md or CSV columns.
5. **M4** — Only if pilot needs physical doors.
6. **M6** — Only when a second site needs central DB.

---

## Verify before each release

```powershell
npm run verify:all
npm run smoke:production
```

---

## Out of scope for Phase 2 (unless replanned)

- Micro-kernel / shard marketplace ([ROADMAP.md Appendix A](../ROADMAP.md#appendix-a--platform-vision-deferred))
- Full SaaS multi-tenant
- Native mobile apps
