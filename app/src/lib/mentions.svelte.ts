import { roomMembers, type Member, type MentionInput } from "$lib/backend";

export type Suggestion = Member | { user_id: "@room"; name: "room"; avatar: null };

const MAX_SUGGESTIONS = 6;
const normalize = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();

/** `@` autocomplete for the composer, plus the mentions picked for the current draft. */
export class MentionAutocomplete {
  /** The `@query` being typed: [start, end) in the draft. */
  query = $state<{ start: number; end: number; text: string } | null>(null);
  index = $state(0);
  /** Mentions inserted in the current draft; sent along so they become pills. */
  picked: MentionInput[] = [];

  #members = $state<Member[]>([]);
  #roomId = "";
  #isGroup = false;

  /** Call when the open chat changes. Members load lazily on the first `@`. */
  reset(roomId: string, isGroup: boolean) {
    if (roomId !== this.#roomId) this.#members = [];
    this.#roomId = roomId;
    this.#isGroup = isGroup;
    this.clear();
  }

  clear() {
    this.query = null;
    this.picked = [];
  }

  get open() {
    return this.query !== null && this.suggestions.length > 0;
  }

  get suggestions(): Suggestion[] {
    if (!this.query) return [];
    const q = normalize(this.query.text);
    const matches = this.#members.filter((m) => {
      const name = normalize(m.name);
      return name.startsWith(q) || name.includes(` ${q}`) || normalize(m.user_id).startsWith(`@${q}`);
    });
    const list: Suggestion[] = matches.slice(0, MAX_SUGGESTIONS);
    if (this.#isGroup && "room".startsWith(q)) list.push({ user_id: "@room", name: "room", avatar: null });
    return list;
  }

  /** Look for an `@word` right before the caret. */
  update(draft: string, caret: number) {
    const before = draft.slice(0, caret);
    const match = /(?:^|\s)@([^\s@]{0,32})$/u.exec(before);
    if (!match) {
      this.query = null;
      return;
    }
    const text = match[1];
    this.query = { start: caret - text.length - 1, end: caret, text };
    this.index = 0;
    if (!this.#members.length) {
      const id = this.#roomId;
      roomMembers(id)
        .then((m) => id === this.#roomId && (this.#members = m))
        .catch(() => {});
    }
  }

  move(delta: number) {
    const n = this.suggestions.length;
    if (n) this.index = (this.index + delta + n) % n;
  }

  /** Insert the suggestion; returns the new draft and caret position. */
  pick(draft: string, s: Suggestion = this.suggestions[this.index]): { draft: string; caret: number } {
    const q = this.query!;
    const text = `@${s.name}`;
    const next = `${draft.slice(0, q.start)}${text} ${draft.slice(q.end)}`;
    if (s.user_id !== "@room") this.picked.push({ user_id: s.user_id, text });
    this.query = null;
    return { draft: next, caret: q.start + text.length + 1 };
  }
}
