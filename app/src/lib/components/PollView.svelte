<script lang="ts">
  import { tr, trn } from "$lib/i18n/index.svelte";
  import Check from "@lucide/svelte/icons/check";
  import { endPoll, votePoll, type MessageContent } from "$lib/backend";

  type Poll = Extract<MessageContent, { kind: "poll" }>;
  let { roomId, pollId, poll }: { roomId: string; pollId: string; poll: Poll } = $props();

  /** Our choice before the server echoes it back (votes are local echoes in the timeline too). */
  let picking = $state<string[] | null>(null);
  let error = $state<string | null>(null);

  const mine = $derived(picking ?? poll.answers.filter((a) => a.mine).map((a) => a.id));
  const voted = $derived(mine.length > 0);
  const showResults = $derived(poll.ended || poll.disclosed);
  const total = $derived(poll.answers.reduce((n, a) => n + a.votes, 0));
  const winning = $derived(Math.max(0, ...poll.answers.map((a) => a.votes)));
  const pending = $derived(!pollId.startsWith("$"));

  async function choose(id: string) {
    if (poll.ended || pending) return;
    let next: string[];
    if (poll.max_selections > 1) next = mine.includes(id) ? mine.filter((x) => x !== id) : [...mine, id];
    else next = mine.includes(id) ? [] : [id];
    picking = next;
    error = null;
    try {
      await votePoll(roomId, pollId, next);
    } catch (e) {
      error = String(e);
    } finally {
      picking = null;
    }
  }

  async function end() {
    error = null;
    try {
      await endPoll(roomId, pollId);
    } catch (e) {
      error = String(e);
    }
  }

  function footer() {
    const people = trn(poll.voters, "{n} vote", "{n} votes");
    if (poll.ended) return tr("Final results · {people}", { people: people });
    if (!poll.disclosed) return voted ? tr("Results will show when the poll ends") : tr("Results show when the poll ends");
    return poll.max_selections > 1 ? tr("{people} · choose several", { people: people }) : people;
  }
</script>

<div class="poll">
  <strong class="question">{poll.question}</strong>
  <div class="answers">
    {#each poll.answers as a (a.id)}
      {@const share = total ? a.votes / total : 0}
      <button
        class="answer"
        class:chosen={mine.includes(a.id)}
        class:winner={poll.ended && a.votes === winning && winning > 0}
        disabled={poll.ended || pending}
        onclick={() => choose(a.id)}
      >
        {#if showResults}<span class="fill" style:width="{share * 100}%"></span>{/if}
        <span class="mark">{#if mine.includes(a.id)}<Check size={14} strokeWidth={3} />{/if}</span>
        <span class="label">{a.text}</span>
        {#if showResults}<span class="count">{a.votes}</span>{/if}
      </button>
    {/each}
  </div>
  <div class="footer">
    <small>{footer()}</small>
    {#if poll.is_mine && !poll.ended && !pending}
      <button class="end" onclick={end}>{tr("End poll")}</button>
    {/if}
  </div>
  {#if error}<small class="error">{error}</small>{/if}
</div>

<style>
  .poll {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: min(260px, 70vw);
    padding: 2px 0;
  }
  .question {
    font-size: 1rem;
  }
  .answers {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .answer {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, currentColor 18%, transparent);
    border-radius: 12px;
    background: rgb(0 0 0 / 0.12);
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: border-color 0.15s var(--ease);
  }
  .answer:disabled {
    cursor: default;
  }
  .answer.chosen {
    border-color: color-mix(in srgb, currentColor 70%, transparent);
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: color-mix(in srgb, currentColor 14%, transparent);
    transition: width 0.3s var(--ease);
  }
  .winner .fill {
    background: color-mix(in srgb, currentColor 24%, transparent);
  }
  .mark {
    position: relative;
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 2px solid color-mix(in srgb, currentColor 60%, transparent);
    border-radius: 50%;
  }
  .chosen .mark {
    border-color: currentColor;
    background: var(--accent);
    color: white;
  }
  .label {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .count {
    position: relative;
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .footer small {
    opacity: 0.75;
  }
  .end {
    padding: 4px 10px;
    border: 1px solid color-mix(in srgb, currentColor 40%, transparent);
    border-radius: 999px;
    background: none;
    color: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .error {
    color: var(--danger);
  }
</style>
