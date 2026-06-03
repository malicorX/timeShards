# TimeShards — UI/UX Guide (Server & Client)

Leitfaden für den Umbau von **Admin-Server** (`apps/server`) und **Mitarbeiter-Client** (`apps/client`). Ziel: **(a)** schick und modern, **(b)** informativ statt nur funktional.

Stand: bezieht sich auf die bestehende dunkle Desktop-Oberfläche (Tauri 2 + Svelte 5). Neue Screens sollen dieses Dokument erfüllen, bevor sie gemerged werden.

---

## 1. Produktrollen

| App | Nutzer | Aufgabe |
|-----|--------|---------|
| **Server** | Admin, HR, Vorgesetzte | Konfiguration, Freigaben, Übersicht, Zutritt, Perioden |
| **Client** | Mitarbeiter | Stempeln, eigene Woche, Abwesenheit, Badge, ggf. Freigaben |

**Gemeinsam:** gleiche Sprache (Deutsch), gleiche Begriffe, gleiche Farben und Komponenten — keine zwei verschiedenen „Produkte“.

---

## 2. Designprinzipien

### 2.1 Informativ zuerst

Jede Ansicht beantwortet in **3 Sekunden**:

1. **Wo bin ich?** (Titel + Kurztext)
2. **Was ist der Stand?** (Zahlen, Ampel, letzte Aktion)
3. **Was kann ich tun?** (primäre Aktion sichtbar, Sekundäres zurückgestellt)

Regeln:

- **Keine toten Texte:** Wenn etwas wichtig ist (Warnung, KPI, Listenzeile), ist es **klickbar** oder führt zu Erklärung/Detail.
- **Kontextzeile unter jeder Überschrift** (`lead`): 1–2 Sätze, was hier passiert und was *nicht* (z. B. „Schichten = Planung, Soll = Perioden“).
- **Zahlen immer mit Einheit/Label:** nicht nur `39`, sondern `39 Entwürfe` · `3 ohne Kalender`.
- **Leere Zustände** sagen, *warum* leer ist und **nächsten Schritt** (Button + Link).

### 2.2 Modern, aber Büro-tauglich

- **Desktop-first** (Tauri), Maus + Tastatur, keine Mobile-App-Metaphern.
- **Ruhiges Dark Theme** — kein Neon, kein Glassmorphism-Overload.
- **Klare Hierarchie:** wenige Akzentfarben, viel Struktur durch Abstand und Typografie.
- **Kein „AI-Slop“:** keine generischen Stock-Illustrationen, keine leeren Marketing-Floskeln, keine 12 gleich wichtigen Primary-Buttons.

### 2.3 Entdeckbar & sicher

- **Progressive Disclosure:** Expertenfunktionen in `<details>`, „Erweitert“, oder zweiter Tab — nicht alles auf einer Seite.
- **Destruktiv / irreversibel:** Bestätigung + kurzer Grund (z. B. Ablehnung Stundenzettel).
- **Fehler oben, persistent** bis behoben oder geschlossen; Erfolg kurz, dann weg.

---

## 3. Gemeinsames Designsystem

Beide Apps sollen **dieselben CSS-Variablen** nutzen. Kurzfristig: `apps/server/src/app.css` und `apps/client/src/app.css` angleichen; mittelfristig: `apps/shared/styles/tokens.css` (ein Import in beiden).

### 3.1 Farben (Tokens)

```css
:root {
  /* Flächen */
  --ts-bg-app: #12141a;
  --ts-bg-elevated: #1a1e28;
  --ts-bg-sunken: #12151f;
  --ts-bg-sidebar: #0e1016;

  /* Rahmen */
  --ts-border: #2a3142;
  --ts-border-focus: #3d6cf5;

  /* Text */
  --ts-text: #e8eaed;
  --ts-text-muted: #8b93a7;
  --ts-text-accent: #9fb0ff;

  /* Aktion */
  --ts-primary: #3d6cf5;
  --ts-primary-hover: #4d7aff;
  --ts-secondary: #2a2f3d;

  /* Semantik */
  --ts-success: #7ddea2;
  --ts-warning: #e8c070;
  --ts-danger: #ff8f8f;
  --ts-danger-bg: #3a1518;

  /* Status-Chips */
  --ts-chip-planned: #4a3a10;
  --ts-chip-published: #1e3a2f;
  --ts-chip-draft: #5a6278;

  /* Radius & Schatten */
  --ts-radius-sm: 6px;
  --ts-radius-md: 8px;
  --ts-radius-lg: 12px;
  --ts-shadow-focus: 0 0 0 2px var(--ts-border-focus);
}
```

### 3.2 Typografie

| Stufe | Verwendung | Größe (ca.) |
|-------|------------|-------------|
| **H1** | App-Marke Sidebar | 1rem, `--ts-text-accent` |
| **H2** | Tab-Titel (Zeit, Zutritt) | 1.35rem, normal weight |
| **H3** | Karten-Titel | 1rem, semibold |
| **H4** | Unterabschnitt | 0.95rem, `--ts-text-accent` |
| **Body** | Fließtext | 1rem / 1.5 line-height |
| **Small** | Hilfetext, Metadaten | 0.85–0.9rem, `--ts-text-muted` |
| **Fine** | Fußnoten, Rebuild-Hinweis | 0.8rem |

Schrift: `Segoe UI`, `system-ui`, sans-serif (Windows-native, gut lesbar).

### 3.3 Abstand

- Karten-Padding: `1rem 1.25rem`
- Zwischen Sektionen: `1rem`–`1.5rem`
- Formular-Felder: `0.65rem` Gap in Grids
- Kein `max-width: 400px` auf ganzen Formularen — nur auf schmalen Dialogen (Login)

---

## 4. Informationsarchitektur

### 4.1 Server (Admin)

```
Sidebar (Haupt-Navigation)
├── Übersicht      → KPIs, Login, Go-Live, Deep-Links
├── Personal       → MA, Kalender-Zuweisung, Filter „ohne Kalender“
├── Zeit
│   ├── Perioden & Soll   → Tages-/Jahresperioden, MA-Zuordnung
│   ├── Schichtplanung    → Vorlagen, KW-Raster (nur Planung)
│   ├── Stundenzettel     → Freigabe, Tagesdetails
│   └── Abschluss & Export → Monat, CSV, Konten
├── Abwesenheit
├── Zutritt
└── System
```

**Regel:** Alles, was **Sollzeit** betrifft, lebt unter **Perioden & Soll** — nicht unter Schichten.

### 4.2 Client (Mitarbeiter)

```
Sidebar (Säulen / Pillars)
├── Zeit           → Stempeln, KW-Saldo, Schichten (read-only wo sinnvoll)
├── Abwesenheit
├── Freigaben      → nur mit Rolle, Badge mit Anzahl
├── Zutritt        → Badge simulieren, eigene Events
└── Konto          → Profil, API-URL, Abmelden
```

**Regel:** Client zeigt **nie** globale Admin-Konfiguration — nur „mein“ Kontext + erklärende Hinweise („Soll kommt aus Kalender X“).

---

## 5. Layout-Muster

### 5.1 App-Shell (beide Apps)

```
┌─────────────┬──────────────────────────────────────┐
│  Sidebar    │  Content                             │
│  Logo       │  H2 + Lead                           │
│  User ctx   │  [Sub-Nav / Tabs]                    │
│  Nav        │  [Flash: error | success]            │
│             │  [Hauptinhalt: Karten / Split]       │
└─────────────┴──────────────────────────────────────┘
```

- Sidebar **220px**, fix; Content scrollt.
- **User-Kontext** in Sidebar (Name, PN, Stempelstatus, KW-Saldo) — immer sichtbar im Client, im Server optional kompakt.

### 5.2 Seitenkopf (Pflicht)

Jede inhaltliche Sektion beginnt mit:

```html
<header class="page-header">
  <h2>Perioden & Soll</h2>
  <p class="lead muted">Tagesperioden definieren Soll …</p>
</header>
```

### 5.3 Sub-Navigation

- **Horizontal**, Pill-Style (`.sub-nav` / `.period-tabs`).
- Aktiver Tab: Hintergrund + 1px Akzent-Rand.
- **Badge** nur für zählbare Aufgaben (`nav-badge`), nicht dekorativ.

### 5.4 Master–Detail (Listen)

Für Tagesperioden, Türen, MA-Listen:

```
┌──────────────┬─────────────────────────┐
│ Pick-List    │ Editor / Detail         │
│ (klickbar)   │ Formular + Speichern    │
└──────────────┴─────────────────────────┘
```

CSS-Klassen (Server bereits teilweise): `.period-split`, `.pick-list`, `.pick-item`, `.pick-item.selected`.

### 5.5 KPI-Raster (Übersicht)

- `.stat-grid` mit `.stat-card.stat-card-btn` — **jede Karte = Navigation**.
- Hover: Rand `--ts-border-focus`.
- Warn-KPIs: Rand `--ts-warning`, Klick zum passenden Tab.

---

## 6. Komponenten-Katalog

Implementierung: bevorzugt **wiederverwendbare Svelte-Komponenten** in `apps/shared/ui/` (neu) oder pro App spiegeln, bis shared existiert.

| Komponente | Zweck | Hinweise |
|------------|-------|----------|
| `TsButton` | primary / secondary / danger | `disabled`, Loading-Text |
| `TsField` | Label + Input/Select + Hint | immer sichtbares Label, nicht nur placeholder |
| `TsCard` | Abschnitt | optional `title`, `lead`, `actions` slot |
| `TsEmptyState` | leere Liste | Icon optional, 1 CTA |
| `TsStatusChip` | planned / published / draft / alarm | konsistente Farben |
| `TsWeekGrid` | 7-Tage-Raster | Tag = `<button>`, selected state |
| `TsFlash` | error / success | sticky top im Content |
| `TsStatCard` | KPI + optional navigate | `onclick` oder `href` |
| `TsDataTable` | tabellarische Details | sortierbar später |
| `TsDetails` | progressive disclosure | `<details class="ts-advanced">` |

### 6.1 Buttons

| Typ | Wann | Beispiel |
|-----|------|----------|
| **Primary** | eine Hauptaktion pro Karte | Speichern, Freigeben, Einreichen |
| **Secondary** | Hilfsaktion | KW wechseln, Aktualisieren |
| **Danger** | ablehnen, löschen | Ablehnen (mit Dialog) |
| **Link** | Navigation im Text | `.linkish` — keine underline-only ohne Hover |

Max. **1 Primary** pro sichtbarer Kartenfläche.

### 6.2 Formulare

- Labels **über** dem Feld (`.field-label`), nicht nur Placeholder.
- Gruppen: `.editor-grid` (responsive `minmax(180px, 1fr)`).
- Inline-Anlegen: `.inline-form` (Input + „+ Anlegen“).
- Nach Speichern: **Success-Toast** + Daten neu laden; bei Rebuild-Hinweis: `fine-print` („Stundenzettel werden neu berechnet“).

### 6.3 Listen & Zeilen

- Interaktive Zeile: `<button class="…-row-btn">` volle Breite, Hover-Rand.
- Nicht-interaktiv: normales `<li>` nur wenn rein informativ.
- Tabellen für **Tagesdetails** (Stundenzettel): Spalten Tag | Modell | Soll | Ist | Saldo.

### 6.4 Kalender & Zeit

- **Wochennavigation:** `← KW` · Label · `KW →` · `Diese Woche` — immer gleich.
- **Schicht-Chips:** `.shift-chip` + Modifier `planned` | `published` | `cancelled`.
- **Perioden-Woche:** Tag klickbar → Detail-Editor unter dem Raster.

---

## 7. Inhalt & Begriffe (Deutsch)

Einheitliches Glossar — in UI und Doku gleich:

| UI-Text | Bedeutung | Nicht verwenden |
|---------|-----------|-----------------|
| **Tagesperiode** | `workday_model`, Soll/Gleit/Pause | Tagesmodell (ok als Synonym in Klammern) |
| **Jahresperiode** | `work_calendar`, Tag→Tagesperiode | Arbeitskalender (ok in Hilfetext) |
| **MA-Zuordnung** | `employee_work_assignment` | — |
| **Stundenzettel** | timesheet KW | Sheet, Report |
| **Soll / Ist / Saldo** | Minuten, `formatMinutes` | Sollzeit nur wenn nötig |
| **Schicht** | geplante Instanz | nicht mit Soll verwechseln |

**Ton:** sachlich, Sie-Form optional (aktuell neutral „Speichern“, „Wählen“ — durchgängig halten).

**Zahlen:** immer `formatMinutes` / `weekLabelForAnchor` — keine Roh-ISO-Strings in der UI.

---

## 8. Interaktion: „Alles Wichtige klickbar“

Checkliste pro Screen:

- [ ] KPI → springt zum passenden Tab mit Filter
- [ ] Warnung in Health/Übersicht → Link/Button zur Behebung
- [ ] Listenzeile mit ID/Name → Detail, Formular füllen oder expandieren
- [ ] Kalendertag → Zuweisung / Editor
- [ ] Stundenzettel-Kopf → Tagesdetails toggle
- [ ] Tür-Alert → Zutritt-Tab + Aktion „Zurücksetzen“
- [ ] „Kein Arbeitskalender“ → Personal oder Perioden-Zuordnung

**Nicht klickbar:** reine Labels, Tabellen-Header, deaktivierte Felder (mit `title`/Hint warum).

---

## 9. Zustände

| Zustand | Darstellung |
|---------|-------------|
| **Loading** | Button-Text „…“ / „Lädt…“, keine leere Fläche ohne Hinweis |
| **Leer** | `TsEmptyState` + 1 CTA |
| **Fehler API** | `.error` oben + konkrete Meldung (kein „Error 500“) |
| **Erfolg** | `.success`, 3–5 s oder bis nächste Aktion |
| **Degraded** | Health ≠ ok → Banner in Übersicht |
| **Teilweise Daten** | „Keine Tage in dieser Woche — Jahr befüllen“ |

---

## 10. Barrierefreiheit (Minimum)

- Interaktive Elemente: `<button type="button">`, nicht `<div onclick>`.
- Fokus sichtbar: `:focus-visible { box-shadow: var(--ts-shadow-focus); }`.
- Kontrast: Text auf `--ts-bg-elevated` ≥ WCAG AA für Normaltext.
- `aria-label` auf Icon-only-Buttons.
- Tabellen mit `<th scope="col">`.

---

## 11. Server vs. Client — spezifische Ziele

### 11.1 Server — Umbau-Priorität

| Prio | Bereich | Ziel |
|------|---------|------|
| P0 | Design-Tokens shared | eine `tokens.css`, Duplikat in client entfernen |
| P0 | Perioden & Soll | Master–Detail, Tabs (Referenz-Implementierung) |
| P1 | Übersicht | alle KPIs verlinkt, Setup-Guide visuell ruhiger |
| P1 | Stundenzettel | Zeile klickbar, Status-Chips, Filter sticky |
| P2 | Zutritt | Zonen/Türen als Pick-List + Detail |
| P2 | Personal | Tabellenzeile → Drawer/Panel MA |
| P3 | System | gruppierte Env-Hinweise, weniger Roh-JSON |

### 11.2 Client — Umbau-Priorität

| Prio | Bereich | Ziel |
|------|---------|------|
| P0 | Shell | gleiche Tokens; Sidebar mit KW-Saldo prominenter |
| P0 | Zeit-Säule | große Stempel-CTAs, darunter „Heute / KW“ informativ |
| P1 | Abwesenheit | Status + Timeline, nicht nur Formular |
| P1 | Freigaben | Queue wie Server-Stundenzettel (klickbar) |
| P2 | Zutritt | letzte Scans, Leser wählen mit Erklärung |
| P2 | Konto | Verbindung, Version, Hilfe-Link |

---

## 12. Umsetzung in Phasen

### Phase A — Fundament (1–2 Tage)

1. `tokens.css` anlegen, Server + Client importieren.
2. `TsCard`, `TsField`, `TsFlash` extrahieren (minimal).
3. `page-header` + `lead` auf jedem Haupt-Tab.

### Phase B — Informationsdichte (3–5 Tage)

1. Übersicht: KPI-Navigation vervollständigen.
2. Perioden: Referenz-Screen fertig (Tabs, klickbare Tage, Anlegen).
3. Stundenzettel: einheitliche Zeilen + Tagesdetails.
4. Client Zeit-Säule: Statusblock + Erklärung Soll/Ist.

### Phase C — Polish (laufend)

1. Fokus-Ringe, Animationen dezent (150ms border/background).
2. Leere Zustände überall.
3. Screenshots in `docs/screenshots/` für Regression (optional).

**Definition of Done** pro Screen:

- [ ] Lead-Text vorhanden
- [ ] Leerzustand mit CTA
- [ ] Primäraktion klar
- [ ] Wichtige Daten klickbar oder expandierbar
- [ ] Gleiche Tokens wie Guide
- [ ] `svelte-check` ohne a11y-Warnungen zu interaktiven Divs

---

## 13. Anti-Patterns (vermeiden)

- Mehrere volle Breiten-Formulare untereinander ohne Struktur.
- Technische IDs als einzige Beschriftung (`wm-std-8h` nur im `title`, Anzeige = Name).
- Listen ohne Hover-Feedback, die bearbeitbar sein sollten.
- Schichtplan und Perioden auf einer scrollbaren Endlos-Seite.
- Farben hardcoded (`#3d6cf5`) in Komponenten — nur CSS-Variablen.
- Englische Platzhalter in deutscher UI.
- Success-Meldung ohne sichtbare Datenänderung (immer `refresh` nach Mutation).

---

## 14. Referenz im Code (Ist-Stand)

| Muster | Referenz |
|--------|----------|
| Perioden-Tabs + Split | `apps/server/src/components/WorkCalendarCard.svelte` |
| KPI klickbar | `apps/server/src/components/OverviewTab.svelte` |
| Sub-Nav Zeit | `apps/server/src/App.svelte` (`timeSection`) |
| Client-Shell | `apps/client/src/components/ClientAppShell.svelte` |
| Globale Styles | `apps/server/src/app.css`, `apps/client/src/app.css` |

Nach Phase A sollten neue Screens **nur** noch gemeinsame Komponenten/Tokens nutzen — nicht erneut ad-hoc CSS kopieren.

---

## 15. Kurz-Checkliste für Reviews

Vor Merge fragen:

1. Sieht man auf einen Blick **Stand + nächste Aktion**?
2. Ist die **Hauptaktion** eindeutig?
3. Sind **Warnungen** klickbar und führen sie zur Lösung?
4. Nutzt der Screen **Tokens** und bestehende Klassen?
5. Ist der Text **deutsch** und glossar-konform?
6. Server und Client fühlen sich wie **ein Produkt** an?

---

*Bei Änderungen am Guide: `docs/README.md` verweist hierher; optional einen Satz in `AGENTS.md` ergänzen.*
