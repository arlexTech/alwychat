import type { SpaceChild } from "$lib/backend";

export type Section = "chats" | "voice" | "spaces";

export const kindOf = (c: SpaceChild): Section => (c.is_space ? "spaces" : c.is_voice ? "voice" : "chats");

/** A space's sections in its order: a kind's place is where its first room is. Kinds with
 *  nothing in them keep the default place (chats, voice channels, categories). */
export function sectionOrder(children: SpaceChild[]): Section[] {
  const first = new Map<Section, number>();
  children.forEach((c, i) => {
    const kind = kindOf(c);
    if (!first.has(kind)) first.set(kind, i);
  });
  const defaults: Section[] = ["chats", "voice", "spaces"];
  return [...defaults].sort((a, b) => (first.get(a) ?? 1e9 + defaults.indexOf(a)) - (first.get(b) ?? 1e9 + defaults.indexOf(b)));
}
