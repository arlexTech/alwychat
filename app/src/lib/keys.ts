import { tr } from "$lib/i18n/index.svelte";
/** Readable name of a keyboard key (`KeyboardEvent.code`): "Space", "F", "Left Ctrl"…
 * `label`: what it printed when chosen (`KeyboardEvent.key`), which follows the layout. */
export function keyName(code: string, label?: string | null): string {
  if (label && label.length === 1 && label.trim()) return label.toLocaleUpperCase();
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad")) return tr("Num {value}", { value: code.slice(6) });
  const sides: Record<string, string> = { Left: tr("Left "), Right: tr("Right ") };
  const m = /^(Control|Shift|Alt|Meta)(Left|Right)$/.exec(code);
  if (m) return `${sides[m[2]]}${m[1] === "Control" ? tr("Ctrl") : m[1] === "Meta" ? tr("Super") : m[1]}`;
  return code.replace(/([a-z])([A-Z])/g, "$1 $2");
}
