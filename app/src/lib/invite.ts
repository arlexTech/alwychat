// A registration token from an invite link (/register?token=…, shown as the main page by
// hooks.ts), kept for this tab only so a reload of the sign-in screen still has it.
// "" = create an account without a token.
const KEY = "alwychat.invite";

/** Take the token from an invite link's address, if this is one. Returns whether it was. */
export function takeInviteFromAddress(): boolean {
  if (typeof location === "undefined" || location.pathname !== "/register") return false;
  const params = new URLSearchParams(location.search);
  keepInvite(params.get("token") ?? params.get("invite") ?? "");
  return true;
}

export function keepInvite(token: string) {
  try {
    sessionStorage.setItem(KEY, token.trim());
  } catch {}
}

export function pendingInvite(): string | null {
  try {
    return sessionStorage.getItem(KEY);
  } catch {
    return null;
  }
}

export function dropInvite() {
  try {
    sessionStorage.removeItem(KEY);
  } catch {}
}
