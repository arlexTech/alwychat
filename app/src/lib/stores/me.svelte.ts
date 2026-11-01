// Our own profile (name, photo) for the account button and Settings.
import { ownProfile, type UserResult } from "$lib/backend";

class Me {
  profile = $state<UserResult | null>(null);

  async refresh() {
    this.profile = await ownProfile().catch(() => this.profile);
  }
}

export const me = new Me();
