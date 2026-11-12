<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { reorderSpace, spaceCategories, type SpaceCategory, type SpaceChild } from "$lib/backend";
  import { peek, remember } from "$lib/cache";
  import { onMount } from "svelte";
  import Page from "./Page.svelte";
  import ReorderList from "./ReorderList.svelte";
  import Hash from "@lucide/svelte/icons/hash";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import Folder from "@lucide/svelte/icons/folder";
  import { sectionOrder, type Section } from "$lib/spaceSections";

  /** Drag a space's chats, voice channels and categories (and each category's chats) into
   *  the order everyone sees. Saved in the space (`order`), so other apps follow it too. */
  let { spaceId, children, onClose }: { spaceId: string; children: SpaceChild[]; onClose: () => void } = $props();

  // Moves show at once; the server catches up behind (and the old order comes back on error).
  // svelte-ignore state_referenced_locally
  let top = $state<SpaceChild[]>(children);
  // svelte-ignore state_referenced_locally
  let categories = $state<SpaceCategory[]>(peek<SpaceCategory[]>(`categories:${spaceId}`, true) ?? []);
  let error = $state<string | null>(null);
  let saving = $state(0);

  onMount(() => {
    spaceCategories(spaceId)
      .then((list) => (categories = remember(`categories:${spaceId}`, list, true)))
      .catch(() => {});
  });

  const chats = $derived(top.filter((c) => !c.is_space && !c.is_voice));
  const voice = $derived(top.filter((c) => c.is_voice));
  const subSpaces = $derived(top.filter((c) => c.is_space));

  function byIds<T extends { id: string }>(list: T[], ids: string[]): T[] {
    const map = new Map(list.map((c) => [c.id, c]));
    return ids.flatMap((id) => map.get(id) ?? []);
  }

  /** Saves run one after another, so a quick second move can't land before the first. */
  let queue: Promise<void> = Promise.resolve();
  function save(space: string, ids: string[], undo: () => void) {
    error = null;
    saving++;
    queue = queue.then(async () => {
      try {
        await reorderSpace(space, ids);
      } catch (e) {
        undo();
        error = tr("Couldn't save the order: {e}", { e });
      } finally {
        saving--;
      }
    });
  }

  const LABELS: Record<Section, { name: string; icon: typeof Hash }> = {
    chats: { name: tr("Chats"), icon: Hash },
    voice: { name: tr("Voice channels"), icon: Volume2 },
    spaces: { name: tr("Categories"), icon: Folder },
  };
  // The space's order is one list: sections one after the other, in the order chosen here.
  const sections = $derived(sectionOrder(top).filter((k) => byKind(k).length));
  const sectionItems = $derived(sections.map((k) => ({ id: k, avatar: null, ...LABELS[k] })));
  function byKind(kind: Section): SpaceChild[] {
    return kind === "chats" ? chats : kind === "voice" ? voice : subSpaces;
  }

  /** A section's rooms moved (`kind`), or the sections themselves (`kind` null). */
  function moveTop(kind: Section | null, ids: string[]) {
    const before = top;
    const order = kind ? sections : (ids as Section[]);
    top = order.flatMap((k) => (k === kind ? byIds(byKind(k), ids) : byKind(k)));
    remember(`space:${spaceId}`, top, true);
    void save(spaceId, top.map((c) => c.id), () => (top = before));
  }

  function moveInCategory(category: SpaceCategory, ids: string[]) {
    const before = categories;
    categories = categories.map((c) => (c.id === category.id ? { ...c, children: byIds(c.children, ids) } : c));
    remember(`categories:${spaceId}`, categories, true);
    void save(category.id, ids, () => (categories = before));
  }

  const categoryOf = (id: string) => categories.find((c) => c.id === id);
</script>

<Page title={tr("Order")} {onClose}>
  <p class="m-hint">{tr("Drag to change the order. Everyone in the space sees it, in other apps too.")}</p>
  {#if sections.length > 1}
    <h3 class="m-section">{tr("Sections")}</h3>
    <ReorderList items={sectionItems} onMove={(ids) => moveTop(null, ids)} />
  {/if}
  {#each sections as kind (kind)}
    <h3 class="m-section">{LABELS[kind].name}</h3>
    <ReorderList items={byKind(kind)} square={kind === "spaces"} disabled={byKind(kind).length < 2} onMove={(ids) => moveTop(kind, ids)} />
  {/each}
  {#each subSpaces as sub (sub.id)}
    {@const cat = categoryOf(sub.id)}
    {#if cat && cat.children.length > 1}
      <h3 class="m-section">{cat.name}</h3>
      <ReorderList items={cat.children} onMove={(ids) => moveInCategory(cat, ids)} />
    {/if}
  {/each}
  {#if !top.length}
    <p class="m-hint">{tr("Nothing to reorder yet.")}</p>
  {/if}
  {#if saving}<p class="m-hint">{tr("Saving…")}</p>{/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>
