<script lang="ts">
  /** A search snippet: the core marks matches with \u0002…\u0003 (plain text, never HTML). */
  let { text }: { text: string } = $props();

  const parts = $derived(
    text.split("\u0002").flatMap((chunk, i) => {
      if (i === 0) return [{ match: false, text: chunk }];
      const [hit, rest = ""] = chunk.split("\u0003");
      return [
        { match: true, text: hit },
        { match: false, text: rest },
      ];
    }),
  );
</script>

{#each parts as part, i (i)}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}

<style>
  mark {
    padding: 0 1px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--accent-a) 30%, transparent);
    color: inherit;
  }
</style>
