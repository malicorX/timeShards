const TOKEN_KEY = 'timeshards_client_token';

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string | null) {
  if (token) localStorage.setItem(TOKEN_KEY, token);
  else localStorage.removeItem(TOKEN_KEY);
}

export async function api<T>(
  baseUrl: string,
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');
  const token = getToken();
  if (token) headers.set('Authorization', `Bearer ${token}`);

  const res = await fetch(`${baseUrl}${path}`, { ...init, headers });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    throw new Error(body.message ?? `HTTP ${res.status}`);
  }
  return body as T;
}

export async function downloadFile(
  baseUrl: string,
  path: string,
  filename: string,
): Promise<void> {
  const headers = new Headers();
  const token = getToken();
  if (token) headers.set('Authorization', `Bearer ${token}`);
  const res = await fetch(`${baseUrl}${path}`, { headers });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export type LoginResponse = {
  token: string;
  expires_at: string;
  user: {
    id: string;
    username: string;
    display_name: string;
    locale: string;
    roles: string[];
    permissions: string[];
    employee_id?: string;
    employee_no?: string;
  };
};
