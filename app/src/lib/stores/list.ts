import type { ListDiff } from "$lib/backend";

/** Apply diffs from the core to a plain array, in place. */
export function applyDiffs<T>(list: T[], diffs: ListDiff<T>[]): void {
  for (const d of diffs) {
    switch (d.op) {
      case "append": list.push(...d.values); break;
      case "clear": list.length = 0; break;
      case "push_front": list.unshift(d.value); break;
      case "push_back": list.push(d.value); break;
      case "pop_front": list.shift(); break;
      case "pop_back": list.pop(); break;
      case "insert": list.splice(d.index, 0, d.value); break;
      case "set": list[d.index] = d.value; break;
      case "remove": list.splice(d.index, 1); break;
      case "truncate": list.length = d.length; break;
      case "reset": list.splice(0, list.length, ...d.values); break;
    }
  }
}
