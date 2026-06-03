# Produktivbetrieb — Go-Live

Kurzer Leitfaden für den Einsatz **ohne Demo-Daten**. Technische Zeitbasis: [FOUNDATION.md](./FOUNDATION.md), Checkliste: [FOUNDATION_CHECKLIST.md](./FOUNDATION_CHECKLIST.md).

## 1. Umgebungsvariablen

| Variable | Empfehlung |
|----------|------------|
| `TIMESHARDS_DISABLE_DEMO=1` | Keine Demo-Logins, Badges und Wochen-Stempelungen |
| `TIMESHARDS_ADMIN_PASSWORD` | Starkes Passwort bei **leerer** Datenbank (ersetzt `admin`/`admin`) |
| `TIMESHARDS_BLOCK_DEFAULT_PASSWORDS=1` | Blockiert bekannte Demo-Passwörter auch wenn Demo-Seed aktiv wäre |
| `TIMESHARDS_HW_ADAPTER=sim` | Standard Simulator; `external` nur mit dokumentiertem TCP-Adapter |

Optional: Datenbankpfad und Bind-Adresse siehe [README.md](../README.md).

## 2. Erster Start

1. Server-App starten (oder `npm run api` headless).
2. Mit Admin anmelden (gesetztes Passwort, nicht `admin`/`admin`).
3. **Übersicht** prüfen (oder **Go-Live-Assistent** in der Produktions-Checkliste):
   - **MA ohne Arbeitskalender** = 0
   - **KW ohne Soll** = 0
   - **Zeit ↔ Zutritt** = 0 (oder bewusst geklärt, siehe unten)
4. `npm run foundation:health` (API muss laufen) oder `GET /api/v1/health` → `time_foundation`.

Bei Abweichungen: **Zeitbasis reparieren** (`POST /api/v1/admin/foundation-fix`).

## 3. Stammdaten

| Aufgabe | Wo |
|---------|-----|
| Mitarbeiter anlegen | Personal — **Arbeitskalender** und ggf. **Zutritt Büro** anhaken |
| Arbeitskalender / Feiertage | Zeit → **Arbeitskalender** |
| Tagesmodelle (Soll, Gleitzeit) | Zeit → Tagesmodell bearbeiten |
| Schichten | Zeit → **Schichtplanung** (Planung only, nicht Soll) |
| Badges & Türen | Zutritt |

## 4. Zeit ↔ Zutritt

Die Übersicht vergleicht **eingestempelt** (letzter Stempel) mit **im Gebäude** (letztes Zutrittsereignis pro Zone).

Typische Abweichungen (nicht zwingend Fehler):

- MA stempelt remote / Homeoffice ohne Zutritt
- Zutritt ohne Stempel (Vergessen, Besucher mit Badge)
- Pause im Gebäude, aber nur Zeiterfassung relevant

Für strikte Kopplung: Prozess + ggf. spätere Automatik (nicht in v1).

## 5. Freigabe & Export

1. Wochen-Stundenzettel: **Neu berechnen** → Ist/Soll/Saldo + Tagesdetails prüfen
2. Freigabe → Buchung auf Gleitzeit/Überstunden
3. **Monatsabschluss** (Zeit → Abschluss & Export) wenn alle KW freigegeben
4. **Lohn-CSV** für die Payroll-Vorstufe (kein DATEV in v1)
5. **HTML / PDF** Stundenzettel mit Tagesdetails: Zeit → Stundenzettel → Export

## 6. Verify vor Go-Live

```powershell
npm run smoke:production   # DISABLE_DEMO, Default-Passwörter blockiert
npm run verify:foundation  # DB-Tests + API-Smoke (Entwicklung)
```

## 7. Backup

SQLite-Datei regelmäßig sichern (Pfad in Server-Logs / App-Datenverzeichnis). Vor Migrationen: Kopie anlegen.

## Bewusst später

- DATEV-Schnittstelle
- Vollständiger Jahres-Kalender-Editor
- Automatische Synchronisation Stempel ↔ Tür
