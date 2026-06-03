<script lang="ts">
  import { api } from '../lib/api';
  import type { LoginResponse } from '../lib/api';
  import TsPageHeader from '@timeshards/shared/ui/TsPageHeader.svelte';
  import TsEmptyState from '@timeshards/shared/ui/TsEmptyState.svelte';

  type Employee = {
    id: string;
    employee_no: string;
    display_name: string;
    user_id: string | null;
    org_unit?: string | null;
    username?: string | null;
    active?: boolean;
    work_calendar_assigned?: boolean;
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    apiUrl,
    user,
    active = false,
    onMessage,
    onDataChange,
  }: {
    apiUrl: string;
    user: LoginResponse['user'] | null;
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onDataChange?: (employees: Employee[]) => void;
  } = $props();

  let personnelSearch = $state('');
  let includeInactiveUsers = $state(false);
  let includeInactiveEmployees = $state(false);
  let personnelShowSetupOpen = $state(false);
  let personnelShowNoCalendar = $state(false);
  let users = $state<
    { id: string; username: string; display_name: string; roles: string[]; status: string }[]
  >([]);
  let employees = $state<Employee[]>([]);
  let roles = $state<{ id: string; name: string }[]>([]);
  let badges = $state<{ employee_id: string | null; status: string }[]>([]);
  let accessRules = $state<{ principal_id: string; mode: string }[]>([]);
  let resetPasswordUserId = $state('');
  let resetPasswordValue = $state('');
  let newUser = $state({
    username: '',
    password: '',
    display_name: '',
    role_name: 'employee',
  });
  let newEmployee = $state({
    display_name: '',
    employee_no: '',
    org_unit: '',
    issue_badge: true,
    grant_zone_access: true,
    grant_work_calendar: true,
  });
  let linkUserByEmployee = $state<Record<string, string>>({});
  let editingEmployeeId = $state('');
  let editDisplayName = $state('');
  let editOrgUnit = $state('');
  let selectedEmployeeId = $state('');

  const displayedEmployees = $derived(
    employees.filter((e) => {
      if (personnelShowSetupOpen && e.active !== false) {
        if (employeeHasActiveBadge(e.id) && employeeHasZoneAllow(e.id)) return false;
      }
      if (personnelShowNoCalendar && e.active !== false && e.work_calendar_assigned !== false) {
        return false;
      }
      return true;
    }),
  );

  const usersWithoutEmployee = $derived(
    users.filter((u) => !employees.some((e) => e.user_id === u.id)),
  );

  const selectedEmployee = $derived(
    displayedEmployees.find((e) => e.id === selectedEmployeeId) ?? null,
  );

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  function employeeHasActiveBadge(employeeId: string) {
    return badges.some((b) => b.employee_id === employeeId && b.status === 'active');
  }

  function employeeHasZoneAllow(employeeId: string) {
    return accessRules.some((r) => r.principal_id === employeeId && r.mode === 'allow');
  }

  export function focusNoCalendar() {
    personnelShowNoCalendar = true;
    personnelShowSetupOpen = false;
  }

  export async function refresh() {
    const q = personnelSearch.trim();
    const qParam = q ? `&q=${encodeURIComponent(q)}` : '';
    const userPath = includeInactiveUsers
      ? `/api/v1/admin/users?include_inactive=true${qParam}`
      : `/api/v1/admin/users${q ? `?q=${encodeURIComponent(q)}` : ''}`;
    users = await api(apiUrl, userPath);
    const empPath = includeInactiveEmployees
      ? `/api/v1/admin/employees?include_inactive=true${qParam}`
      : `/api/v1/admin/employees${q ? `?q=${encodeURIComponent(q)}` : ''}`;
    employees = await api(apiUrl, empPath);
    roles = await api(apiUrl, '/api/v1/admin/roles');
    badges = await api<{ employee_id: string | null; status: string }[]>(
      apiUrl,
      '/api/v1/access/badges',
    ).catch(() => []);
    accessRules = await api<{ principal_id: string; mode: string }[]>(
      apiUrl,
      '/api/v1/access/rules',
    ).catch(() => []);
    onDataChange?.(employees);
    if (
      displayedEmployees.length &&
      !displayedEmployees.some((e) => e.id === selectedEmployeeId)
    ) {
      selectedEmployeeId = displayedEmployees[0].id;
    }
  }

  $effect(() => {
    if (!active) return;
    if (
      displayedEmployees.length &&
      !displayedEmployees.some((e) => e.id === selectedEmployeeId)
    ) {
      selectedEmployeeId = displayedEmployees[0].id;
    }
  });

  $effect(() => {
    if (active && user) void refresh();
  });

  async function createUser() {
    try {
      await api(apiUrl, '/api/v1/admin/users', {
        method: 'POST',
        body: JSON.stringify(newUser),
      });
      newUser = { username: '', password: '', display_name: '', role_name: 'employee' };
      await refresh();
      notify('success', 'Benutzer angelegt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createEmployee() {
    try {
      const res = await api<{ employee_no: string }>(apiUrl, '/api/v1/admin/employees', {
        method: 'POST',
        body: JSON.stringify({
          display_name: newEmployee.display_name,
          employee_no: newEmployee.employee_no.trim() || null,
          org_unit: newEmployee.org_unit.trim() || null,
          issue_badge: newEmployee.issue_badge,
          grant_zone_access: newEmployee.grant_zone_access,
          grant_work_calendar: newEmployee.grant_work_calendar,
        }),
      });
      newEmployee = {
        display_name: '',
        employee_no: '',
        org_unit: '',
        issue_badge: true,
        grant_zone_access: true,
        grant_work_calendar: true,
      };
      await refresh();
      notify('success', `Mitarbeiter ${res.employee_no} angelegt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function disableUser(id: string) {
    try {
      await api(apiUrl, `/api/v1/admin/users/${id}/disable`, { method: 'POST' });
      await refresh();
      notify('success', 'Benutzer deaktiviert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function enableUser(id: string) {
    try {
      await api(apiUrl, `/api/v1/admin/users/${id}/enable`, { method: 'POST' });
      await refresh();
      notify('success', 'Benutzer reaktiviert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function resetUserPassword() {
    if (!resetPasswordUserId || resetPasswordValue.length < 6) {
      notify('error', 'Benutzer und Passwort (min. 6) wählen');
      return;
    }
    try {
      await api(apiUrl, `/api/v1/admin/users/${resetPasswordUserId}/reset-password`, {
        method: 'POST',
        body: JSON.stringify({ new_password: resetPasswordValue }),
      });
      resetPasswordValue = '';
      notify('success', 'Passwort zurückgesetzt (Benutzer muss sich neu anmelden)');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  function startEditEmployee(e: Employee) {
    editingEmployeeId = e.id;
    editDisplayName = e.display_name;
    editOrgUnit = e.org_unit ?? '';
  }

  function cancelEditEmployee() {
    editingEmployeeId = '';
  }

  async function saveEmployeeEdit(id: string) {
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({
          display_name: editDisplayName,
          org_unit: editOrgUnit.trim() || null,
        }),
      });
      editingEmployeeId = '';
      await refresh();
      notify('success', 'Mitarbeiter aktualisiert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function grantWorkCalendarForEmployee(employeeId: string) {
    try {
      await api(apiUrl, `/api/v1/admin/employees/${employeeId}/grant-work-calendar`, {
        method: 'POST',
      });
      await refresh();
      notify('success', 'Arbeitskalender zugewiesen');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function grantZoneAccessForEmployee(id: string) {
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}/grant-zone-access`, { method: 'POST' });
      await refresh();
      notify('success', 'Zutritt Büro (Allow-Regel) angelegt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function issueBadgeForEmployee(e: { id: string; employee_no: string }) {
    try {
      await api(apiUrl, '/api/v1/access/badges', {
        method: 'POST',
        body: JSON.stringify({
          employee_id: e.id,
          credential_uid: `DEMO-${e.employee_no}`,
        }),
      });
      await refresh();
      notify('success', `Badge DEMO-${e.employee_no} ausgestellt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function setupEmployeeAccess(e: { id: string; employee_no: string }) {
    try {
      if (!employeeHasActiveBadge(e.id)) {
        await api(apiUrl, '/api/v1/access/badges', {
          method: 'POST',
          body: JSON.stringify({
            employee_id: e.id,
            credential_uid: `DEMO-${e.employee_no}`,
          }),
        });
      }
      if (!employeeHasZoneAllow(e.id)) {
        await api(apiUrl, `/api/v1/admin/employees/${e.id}/grant-zone-access`, {
          method: 'POST',
        });
      }
      await refresh();
      notify('success', `Badge + Zutritt für ${e.employee_no} eingerichtet`);
    } catch (err) {
      notify('error', err instanceof Error ? err.message : String(err));
    }
  }

  async function linkEmployeeUser(employeeId: string) {
    const userId = linkUserByEmployee[employeeId];
    if (!userId) {
      notify('error', 'Bitte Benutzer wählen');
      return;
    }
    try {
      await api(apiUrl, `/api/v1/admin/employees/${employeeId}`, {
        method: 'PATCH',
        body: JSON.stringify({ user_id: userId }),
      });
      await refresh();
      notify('success', 'Benutzer verknüpft');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function unlinkEmployeeUser(employeeId: string) {
    try {
      await api(apiUrl, `/api/v1/admin/employees/${employeeId}`, {
        method: 'PATCH',
        body: JSON.stringify({ user_id: '' }),
      });
      await refresh();
      notify('success', 'Verknüpfung entfernt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function deactivateEmployee(id: string) {
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}/deactivate`, { method: 'POST' });
      await refresh();
      notify('success', 'Mitarbeiter deaktiviert (Badges gesperrt)');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function reactivateEmployee(id: string) {
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}/reactivate`, { method: 'POST' });
      await refresh();
      notify('success', 'Mitarbeiter reaktiviert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

<TsPageHeader
  title="Personal"
  lead="Benutzer und Mitarbeiter. In der Liste wählen, dann Badge, Zutritt und Kalender zuweisen."
/>
<div class="grid-form" style="margin-top: 0.75rem; max-width: 480px;">
  <input
    bind:value={personnelSearch}
    placeholder="Suche Name, PN, Benutzername…"
    onkeydown={(e) => e.key === 'Enter' && refresh()}
  />
  <button class="secondary" type="button" onclick={() => refresh()}>Suchen</button>
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Neuer Benutzer (+ Mitarbeiter)</h3>
  <div class="grid-form">
    <input bind:value={newUser.username} placeholder="Benutzername" />
    <input bind:value={newUser.display_name} placeholder="Anzeigename" />
    <input type="password" bind:value={newUser.password} placeholder="Passwort (min. 6)" />
    <select bind:value={newUser.role_name}>
      {#each roles as r}<option value={r.name}>{r.name}</option>{/each}
    </select>
    <button type="button" onclick={createUser}>Anlegen</button>
  </div>
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Benutzer</h3>
  <label class="muted">
    <input type="checkbox" bind:checked={includeInactiveUsers} onchange={() => refresh()} />
    Inaktive Benutzer anzeigen
  </label>
  <div class="grid-form" style="margin: 0.75rem 0;">
    <select bind:value={resetPasswordUserId}>
      <option value="">Passwort zurücksetzen für…</option>
      {#each users as u}
        <option value={u.id}>{u.username} — {u.display_name}</option>
      {/each}
    </select>
    <input type="password" bind:value={resetPasswordValue} placeholder="Neues Passwort (min. 6)" />
    <button class="secondary" type="button" onclick={resetUserPassword}>Passwort setzen</button>
  </div>
  <ul class="compact-list">
    {#each users as u}
      <li class="row-card">
        {u.display_name} — <code>{u.username}</code> ({u.roles.join(', ')})
        {#if u.status !== 'active'}<span class="muted"> [inaktiv]</span>{/if}
        {#if u.id !== user?.id}
          {#if u.status === 'active'}
            <button class="secondary" type="button" onclick={() => disableUser(u.id)}>Deaktivieren</button>
          {:else}
            <button class="secondary" type="button" onclick={() => enableUser(u.id)}>Reaktivieren</button>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
  <h3>Mitarbeiter (ohne Login)</h3>
  <label class="muted">
    <input type="checkbox" bind:checked={includeInactiveEmployees} onchange={() => refresh()} />
    Inaktive anzeigen
  </label>
  <label class="muted">
    <input type="checkbox" bind:checked={personnelShowSetupOpen} />
    Nur ohne Badge oder Zutritt
  </label>
  <label class="muted">
    <input type="checkbox" bind:checked={personnelShowNoCalendar} />
    Nur ohne Arbeitskalender
  </label>
  {#if personnelShowSetupOpen || personnelShowNoCalendar}
    <span class="muted">({displayedEmployees.length} Treffer)</span>
  {/if}
  <div class="grid-form">
    <input bind:value={newEmployee.display_name} placeholder="Anzeigename" />
    <input bind:value={newEmployee.employee_no} placeholder="Personalnr. (leer = auto)" />
    <input bind:value={newEmployee.org_unit} placeholder="Organisation (optional)" />
    <label class="muted">
      <input type="checkbox" bind:checked={newEmployee.issue_badge} />
      Demo-Badge ausstellen (DEMO-PN)
    </label>
    <label class="muted">
      <input type="checkbox" bind:checked={newEmployee.grant_zone_access} />
      Zutritt Büro (Allow-Regel)
    </label>
    <label class="muted">
      <input type="checkbox" bind:checked={newEmployee.grant_work_calendar} />
      Arbeitskalender (Sollzeit, Standard)
    </label>
    <button type="button" onclick={createEmployee}>Mitarbeiter anlegen</button>
  </div>
  {#if displayedEmployees.length === 0}
    <TsEmptyState
      message={personnelShowSetupOpen
        ? 'Keine Mitarbeiter ohne Badge oder Zutritt — alles eingerichtet.'
        : 'Keine Mitarbeiter in der Liste — Suche oder Filter anpassen.'}
    />
  {:else}
    <div class="period-split" style="margin-top: 0.75rem;">
      <ul class="pick-list" role="listbox" aria-label="Mitarbeiter">
        {#each displayedEmployees as e}
          <li>
            <button
              type="button"
              class="pick-item"
              class:selected={selectedEmployeeId === e.id}
              role="option"
              aria-selected={selectedEmployeeId === e.id}
              onclick={() => {
                selectedEmployeeId = e.id;
                if (editingEmployeeId && editingEmployeeId !== e.id) cancelEditEmployee();
              }}
            >
              <span class="pick-title">{e.employee_no} — {e.display_name}</span>
              <span class="pick-meta">
                {#if e.active === false}
                  inaktiv
                {:else if e.work_calendar_assigned === false}
                  ohne Arbeitskalender
                {:else if !employeeHasActiveBadge(e.id) || !employeeHasZoneAllow(e.id)}
                  Setup offen
                {:else}
                  aktiv
                {/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
      <div class="period-editor-pane">
        {#if selectedEmployee}
          {@const e = selectedEmployee}
          <h4 style="margin: 0 0 0.35rem;">{e.display_name}</h4>
          <p class="muted" style="margin: 0 0 0.75rem;">
            <code>{e.employee_no}</code>
            {#if e.org_unit} · {e.org_unit}{/if}
            {#if e.active !== false && e.work_calendar_assigned === false}
              <span class="error" style="font-size: 0.85rem;"> · Kein Arbeitskalender</span>
            {/if}
          </p>
          {#if editingEmployeeId === e.id}
            <div class="grid-form">
              <input bind:value={editDisplayName} placeholder="Anzeigename" />
              <input bind:value={editOrgUnit} placeholder="Organisation" />
              <button type="button" onclick={() => saveEmployeeEdit(e.id)}>Speichern</button>
              <button class="secondary" type="button" onclick={cancelEditEmployee}>Abbrechen</button>
            </div>
          {:else}
            <div class="btn-row" style="flex-wrap: wrap;">
              <button class="secondary" type="button" onclick={() => startEditEmployee(e)}>Bearbeiten</button>
              {#if e.active !== false && e.work_calendar_assigned === false}
                <button
                  class="secondary"
                  type="button"
                  onclick={() => grantWorkCalendarForEmployee(e.id)}
                >
                  Arbeitskalender
                </button>
              {/if}
              {#if e.active !== false && !employeeHasActiveBadge(e.id)}
                <button class="secondary" type="button" onclick={() => issueBadgeForEmployee(e)}>
                  Badge ausstellen
                </button>
              {/if}
              {#if e.active !== false && !employeeHasZoneAllow(e.id)}
                <button
                  class="secondary"
                  type="button"
                  onclick={() => grantZoneAccessForEmployee(e.id)}
                >
                  Zutritt Büro
                </button>
              {/if}
              {#if e.active !== false && (!employeeHasActiveBadge(e.id) || !employeeHasZoneAllow(e.id))}
                <button class="secondary" type="button" onclick={() => setupEmployeeAccess(e)}>
                  Badge + Zutritt
                </button>
              {/if}
              {#if e.active === false}
                <button class="secondary" type="button" onclick={() => reactivateEmployee(e.id)}>
                  Reaktivieren
                </button>
              {:else if e.username}
                <span class="muted">Login: <code>{e.username}</code></span>
                <button class="secondary" type="button" onclick={() => unlinkEmployeeUser(e.id)}>
                  Login trennen
                </button>
                <button class="secondary" type="button" onclick={() => deactivateEmployee(e.id)}>
                  Deaktivieren
                </button>
              {:else}
                <select
                  bind:value={linkUserByEmployee[e.id]}
                  onchange={(ev) => {
                    linkUserByEmployee = { ...linkUserByEmployee, [e.id]: ev.currentTarget.value };
                  }}
                >
                  <option value="">Benutzer wählen…</option>
                  {#each usersWithoutEmployee as u}
                    <option value={u.id}>{u.username} — {u.display_name}</option>
                  {/each}
                </select>
                <button class="secondary" type="button" onclick={() => linkEmployeeUser(e.id)}>
                  Verknüpfen
                </button>
                <button class="secondary" type="button" onclick={() => deactivateEmployee(e.id)}>
                  Deaktivieren
                </button>
              {/if}
            </div>
          {/if}
        {:else}
          <TsEmptyState message="Mitarbeiter in der Liste wählen." />
        {/if}
      </div>
    </div>
  {/if}
</div>
