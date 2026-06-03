export type ProductionHealth = {
  demo_seeding_enabled?: boolean;
  default_password_login_blocked?: boolean;
  time_foundation?: {
    employees_without_work_calendar: number;
    current_week_drafts_without_soll: number;
  };
};

export type ProductionDashboard = {
  employees_without_work_calendar?: number;
  timesheets_current_week_no_soll?: number;
  time_access_mismatch_count?: number;
};

export type ProductionCheckItem = {
  id: 'demo' | 'passwords' | 'calendar' | 'soll' | 'mismatch';
  label: string;
  ok: boolean;
  hint?: string;
};

export function isProductionMode(apiHealth: ProductionHealth | null | undefined): boolean {
  if (!apiHealth) return false;
  return (
    apiHealth.demo_seeding_enabled === false ||
    (apiHealth.default_password_login_blocked === true &&
      apiHealth.demo_seeding_enabled !== true)
  );
}

export function computeProductionChecklist(
  apiHealth: ProductionHealth | null | undefined,
  dashboard: ProductionDashboard | null | undefined,
): ProductionCheckItem[] {
  const tf = apiHealth?.time_foundation;
  const noCal =
    dashboard?.employees_without_work_calendar ?? tf?.employees_without_work_calendar ?? 0;
  const noSoll =
    dashboard?.timesheets_current_week_no_soll ?? tf?.current_week_drafts_without_soll ?? 0;
  const mismatch = dashboard?.time_access_mismatch_count ?? 0;
  const productionMode = isProductionMode(apiHealth);

  return [
    {
      id: 'demo',
      label: 'Demo deaktiviert oder bewusst Staging',
      ok: productionMode,
      hint: productionMode ? undefined : 'TIMESHARDS_DISABLE_DEMO=1 für Produktion',
    },
    {
      id: 'passwords',
      label: 'Keine Standardpasswörter (empfohlen)',
      ok:
        apiHealth?.default_password_login_blocked === true ||
        apiHealth?.demo_seeding_enabled === false,
      hint: 'TIMESHARDS_BLOCK_DEFAULT_PASSWORDS=1',
    },
    {
      id: 'calendar',
      label: 'Alle aktiven MA mit Arbeitskalender',
      ok: noCal === 0,
      hint: noCal > 0 ? `${noCal} ohne Kalender` : undefined,
    },
    {
      id: 'soll',
      label: 'Aktuelle KW: Entwürfe mit Soll',
      ok: noSoll === 0,
      hint: noSoll > 0 ? `${noSoll} ohne Soll` : undefined,
    },
    {
      id: 'mismatch',
      label: 'Stempel ↔ Gebäude ohne offene Abweichung',
      ok: mismatch === 0,
      hint: mismatch > 0 ? `${mismatch} Abweichung(en) prüfen` : undefined,
    },
  ];
}

export function checklistAllOk(items: ProductionCheckItem[]): boolean {
  return items.length > 0 && items.every((i) => i.ok);
}
