const BERLIN = 'Europe/Berlin';

export function toLocalDatetimeInputValue(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function fromLocalDatetimeInputValue(value: string): string {
  return new Date(value).toISOString();
}

type Ymd = { y: number; m: number; d: number };

function berlinYmd(instant: Date): Ymd {
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone: BERLIN,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).formatToParts(instant);
  const get = (t: string) => Number(parts.find((p) => p.type === t)?.value ?? 0);
  return { y: get('year'), m: get('month'), d: get('day') };
}

/** ISO weekday in Berlin: 1 = Monday … 7 = Sunday. */
function berlinIsoWeekday(instant: Date): number {
  const w = new Intl.DateTimeFormat('en-US', { timeZone: BERLIN, weekday: 'short' }).format(
    instant,
  );
  const map: Record<string, number> = { Mon: 1, Tue: 2, Wed: 3, Thu: 4, Fri: 5, Sat: 6, Sun: 7 };
  return map[w] ?? 1;
}

function addYmdDays(ymd: Ymd, days: number): Ymd {
  const t = Date.UTC(ymd.y, ymd.m - 1, ymd.d + days, 12, 0, 0);
  return berlinYmd(new Date(t));
}

/** UTC instant of 00:00 on a Berlin calendar date (matches server `week_bounds_utc`). */
export function berlinMidnightIso(ymd: Ymd): string {
  const base = Date.UTC(ymd.y, ymd.m - 1, ymd.d, 12, 0, 0);
  for (let h = -14; h <= 14; h++) {
    const t = new Date(base + h * 3_600_000);
    const p = berlinYmd(t);
    if (p.y !== ymd.y || p.m !== ymd.m || p.d !== ymd.d) continue;
    const hour = Number(
      new Intl.DateTimeFormat('en-GB', {
        timeZone: BERLIN,
        hour: 'numeric',
        hour12: false,
      }).format(t),
    );
    const minute = Number(
      new Intl.DateTimeFormat('en-GB', { timeZone: BERLIN, minute: 'numeric' }).format(t),
    );
    if (hour === 0 && minute === 0) return t.toISOString();
  }
  throw new Error(`Berlin midnight not found for ${ymd.y}-${ymd.m}-${ymd.d}`);
}

/** Noon Berlin on a calendar date — stable anchor for week day labels. */
function berlinNoonDate(ymd: Ymd): Date {
  const base = Date.UTC(ymd.y, ymd.m - 1, ymd.d, 12, 0, 0);
  for (let h = -14; h <= 14; h++) {
    const t = new Date(base + h * 3_600_000);
    const p = berlinYmd(t);
    if (p.y === ymd.y && p.m === ymd.m && p.d === ymd.d) {
      const hour = Number(
        new Intl.DateTimeFormat('en-GB', {
          timeZone: BERLIN,
          hour: 'numeric',
          hour12: false,
        }).format(t),
      );
      if (hour === 12) return t;
    }
  }
  return new Date(base);
}

/** Anchor date for week UI controls from API `period_start`. */
export function anchorFromPeriodStart(periodStartIso: string): Date | null {
  const d = new Date(periodStartIso);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Monday 00:00 Berlin – next Monday 00:00 Berlin as ISO strings for API filters. */
export function weekRangeContaining(anchor: Date): { from: string; to: string; days: Date[] } {
  const anchorYmd = berlinYmd(anchor);
  const wd = berlinIsoWeekday(anchor);
  const monday = addYmdDays(anchorYmd, -(wd - 1));
  const from = berlinMidnightIso(monday);
  const nextMonday = addYmdDays(monday, 7);
  const to = berlinMidnightIso(nextMonday);
  const days: Date[] = [];
  for (let i = 0; i < 7; i++) {
    days.push(berlinNoonDate(addYmdDays(monday, i)));
  }
  return { from, to, days };
}

export function formatDayLabel(d: Date): string {
  return d.toLocaleDateString('de-DE', {
    timeZone: BERLIN,
    weekday: 'short',
    day: '2-digit',
    month: '2-digit',
  });
}

export function formatIsoShort(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString('de-DE', {
    timeZone: BERLIN,
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatDateDe(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString('de-DE', {
    timeZone: BERLIN,
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  });
}

export function formatPeriodRange(
  startIso: string | null | undefined,
  endIso: string | null | undefined,
): string {
  if (!startIso) return '';
  const start = formatDateDe(startIso);
  if (!endIso) return start;
  return `${start} – ${formatDateDe(endIso)}`;
}

export function weekLabelForAnchor(anchor: Date): string {
  const { from } = weekRangeContaining(anchor);
  return calendarWeekLabel(from);
}

export function calendarWeekLabel(iso: string | null | undefined): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const ymd = berlinYmd(d);
  const monday = addYmdDays(ymd, -(berlinIsoWeekday(d) - 1));
  const jan4 = { y: ymd.y, m: 1, d: 4 };
  const week1Monday = addYmdDays(jan4, -(berlinIsoWeekday(new Date(berlinMidnightIso(jan4))) - 1));
  const diffDays =
    (Date.UTC(monday.y, monday.m - 1, monday.d) -
      Date.UTC(week1Monday.y, week1Monday.m - 1, week1Monday.d)) /
    86_400_000;
  const kw = Math.floor(diffDays / 7) + 1;
  return `KW ${kw}`;
}

export function shiftOnDay(shiftStartIso: string, day: Date): boolean {
  const start = new Date(shiftStartIso);
  const s = berlinYmd(start);
  const t = berlinYmd(day);
  return s.y === t.y && s.m === t.m && s.d === t.d;
}
