/** German UI labels for API status / event codes. */
export function statusLabel(s: string): string {
  const map: Record<string, string> = {
    draft: 'Entwurf',
    pending: 'Eingereicht',
    approved: 'Freigegeben',
    rejected: 'Abgelehnt',
    planned: 'Geplant',
    published: 'Veröffentlicht',
    cancelled: 'Storniert',
    clock_in: 'Kommen',
    clock_out: 'Gehen',
    break_start: 'Pause Start',
    break_end: 'Pause Ende',
    urlaub: 'Urlaub',
    krank: 'Krank',
    sonder: 'Sonderurlaub',
    unbezahlt: 'Unbezahlt',
    allow: 'Zutritt',
    grant: 'Zutritt',
    deny: 'Abgelehnt',
  };
  return map[s] ?? s;
}
