/** German UI labels for API status / event codes (client). */
export function statusLabel(s: string): string {
  const map: Record<string, string> = {
    draft: 'Entwurf',
    pending: 'Eingereicht',
    approved: 'Freigegeben',
    rejected: 'Abgelehnt',
    clock_in: 'Kommen',
    clock_out: 'Gehen',
    break_start: 'Pause Start',
    break_end: 'Pause Ende',
    urlaub: 'Urlaub',
    krank: 'Krank',
    sonder: 'Sonderurlaub',
    unbezahlt: 'Unbezahlt',
    planned: 'Geplant',
    published: 'Veröffentlicht',
    cancelled: 'Storniert',
  };
  return map[s] ?? s;
}
