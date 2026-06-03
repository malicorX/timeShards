export function doorStatusLabel(s: string): string {
  const map: Record<string, string> = {
    closed: 'Geschlossen',
    open: 'Auf',
    forced_open: 'Dauerauf',
    alarm: 'Alarm',
  };
  return map[s] ?? s;
}

export function reasonLabel(code: string): string {
  const map: Record<string, string> = {
    ok: 'OK',
    unknown_badge: 'Unbekannte Karte',
    unknown_door: 'Unbekannte Tür',
    no_permission: 'Keine Berechtigung',
    antipassback: 'Anti-Passback',
    unassigned_badge: 'Badge nicht zugewiesen',
    schedule_restricted: 'Außerhalb Zeitplan',
  };
  return map[code] ?? code;
}

export function accessDecisionLabel(decision: string): string {
  const map: Record<string, string> = {
    grant: 'Zutritt',
    deny: 'Abgelehnt',
    allow: 'Zutritt',
  };
  return map[decision] ?? decision;
}
