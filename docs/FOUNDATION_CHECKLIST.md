# Zeitbasis — Betriebs-Checkliste

Kurze Prüfliste nach Setup oder Migration. Technische Details: [FOUNDATION.md](./FOUNDATION.md), [TIME_MODEL.md](./TIME_MODEL.md).

## Automatisch beim API-Start

- [ ] Tagesmodelle (`wm-std-8h`, `wm-rest`, …) und `wc-default-standard` vorhanden
- [ ] Feiertagskalender `hc-de-standard` befüllt
- [ ] Aktuelle + nächstes Jahr: Mo–Fr im Arbeitskalender (via `generate-year` im Seed)
- [ ] Alle **aktiven** Mitarbeiter ohne Zuordnung erhalten Standard-Kalender (`assign_all_active_employees`)

Prüfen ohne UI:

```powershell
npm run foundation:health   # API muss laufen
# oder:
Invoke-RestMethod http://127.0.0.1:47821/api/v1/health | Select-Object -ExpandProperty time_foundation
# employees_without_work_calendar und current_week_drafts_without_soll sollten 0 sein
```

## Produktion (ohne Demo)

| Env | Wirkung |
|-----|---------|
| `TIMESHARDS_DISABLE_DEMO=1` | Keine Demo-User/Woche; **Arbeitskalender-Seed läuft weiter** |
| `TIMESHARDS_ADMIN_PASSWORD` | Initiales Admin-Passwort (leere DB) |
| `TIMESHARDS_BLOCK_DEFAULT_PASSWORDS=1` | `admin`/`admin` etc. am Login blockiert |

Nach Migration: `npm run foundation:health` und ggf. **Zeitbasis reparieren** in der Server-UI.

## Vor dem produktiven Betrieb

| Schritt | Wo |
|--------|-----|
| Jeder aktive MA hat Arbeitskalender | Personal → Spalte/Filter **Kein Arbeitskalender** oder Übersicht-KPI |
| Teilzeit-% stimmt | Zeit → MA-Zuordnung |
| Soll passt zum Vertrag | Zeit → Tagesmodell bearbeiten / Kalendertage |
| Schichtvorlagen ≠ Soll | Nur Planung; Soll kommt aus Kalender |
| Test-Stempeln | Client: KW zeigt Ist · Soll · Saldo |
| Stundenzettel-Entwurf | Nach **Gehen** auto-rebuild; sonst **Neu berechnen** |

## Wenn etwas fehlt

1. **Übersicht** → **Zeitbasis reparieren** (`POST /api/v1/admin/foundation-fix`)  
   — fehlende Kalender-Zuordnungen + Rebuild aktuelle KW für alle MA.

2. Einzelner MA: Personal → **Arbeitskalender** oder Zeit → neue Zuordnung.

3. Neuer MA: beim Anlegen **Arbeitskalender** angehakt (Standard).

## Freigabe & Konten

- [ ] Wochen-Stundenzettel: Ist/Soll/Saldo plausibel (Tagesdetails nach Rebuild)
- [ ] Freigabe → Buchung auf **Gleitzeit** / **Überstunden** (Zeitkonten)
- [ ] Monatsabschluss erst wenn alle KW des Monats **freigegeben**

## Verify (Entwicklung / CI)

```powershell
npm run verify:foundation   # cargo test timeshards-db + smoke:api
```

## Bewusst nicht in der Basis

- DATEV / Lohnexport
- Ausführliche `unbezahlt`-Kontenlogik
- Vollständiger Jahres-Kalender-Editor (KW kopieren + Jahr befüllen reicht für v1)
