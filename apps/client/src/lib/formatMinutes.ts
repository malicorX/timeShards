export function formatMinutes(m: number): string {
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}
