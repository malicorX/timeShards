export function reasonLabel(code: string): string {
  const map: Record<string, string> = {
    ok: 'OK',
    no_permission: 'Keine Berechtigung',
    antipassback: 'Anti-Passback',
    schedule_restricted: 'Außerhalb Zeitplan',
    unknown_badge: 'Unbekannte Karte',
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
