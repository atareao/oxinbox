export function getToken(): string | null {
  try {
    return sessionStorage.getItem("oxinbox_token") || localStorage.getItem("oxinbox_token");
  } catch {
    return null;
  }
}

export function setToken(token: string): void {
  try {
    sessionStorage.setItem("oxinbox_token", token);
    localStorage.setItem("oxinbox_token", token);
  } catch { /* noop */ }
}

export function clearToken(): void {
  try {
    sessionStorage.removeItem("oxinbox_token");
    localStorage.removeItem("oxinbox_token");
  } catch { /* noop */ }
}