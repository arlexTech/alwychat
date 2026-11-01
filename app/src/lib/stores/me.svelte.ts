// Our own profile (name, photo) for the account button and Settings. The last copy is kept
// locally so they show at once on the next start; the server's copy follows.
import { ownProfile, type UserResult } from "$lib/backend";

const KEY = "alwychat.me";

class Me {
  profile = $state<UserResult | null>(JSON.parse(localStorage.getItem(KEY) ?? "null"));

  async refresh() {
    const fresh = await ownProfile().catch(() => null);
    if (!fresh) return;
    this.profile = fresh;
    localStorage.setItem(KEY, JSON.stringify(fresh));
  }
}

export const me = new Me();
