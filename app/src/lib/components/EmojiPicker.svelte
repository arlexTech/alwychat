<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Clock from "@lucide/svelte/icons/clock";
  import Smile from "@lucide/svelte/icons/smile";
  import Hand from "@lucide/svelte/icons/hand";
  import Heart from "@lucide/svelte/icons/heart";
  import Leaf from "@lucide/svelte/icons/leaf";
  import Pizza from "@lucide/svelte/icons/pizza";
  import Trophy from "@lucide/svelte/icons/trophy";
  import Plane from "@lucide/svelte/icons/plane";
  import Lightbulb from "@lucide/svelte/icons/lightbulb";
  import { untrack, type Component } from "svelte";

  import type { Emote, EmotePack } from "$lib/backend";
  import { mediaUrl } from "$lib/media";
  import Sticker from "@lucide/svelte/icons/sticker";

  /**
   * The composer's emoji panel (Messenger style): recent ones first, then by category, then the
   * custom emoji packs (one tab each) and their stickers. Without `onCustom` / `onSticker`
   * (reactions), only standard emoji.
   */
  let {
    onPick,
    packs = [],
    onCustom,
    onSticker,
  }: {
    onPick: (emoji: string) => void;
    packs?: EmotePack[];
    onCustom?: (emote: Emote) => void;
    onSticker?: (emote: Emote) => void;
  } = $props();

  const emojiPacks = $derived(onCustom ? packs.filter((p) => p.emotes.some((e) => e.emoticon)) : []);
  const stickers = $derived(onSticker ? packs.flatMap((p) => p.emotes.filter((e) => e.sticker)) : []);

  const split = (s: string) => [...new Intl.Segmenter().segment(s)].map((x) => x.segment).filter((x) => x.trim());
  const CATEGORIES: { id: string; icon: Component<{ size?: number }>; emoji: string[] }[] = [
    { id: "Smileys", icon: Smile, emoji: split("😀😃😄😁😆😅🤣😂🙂🙃😉😊😇🥰😍🤩😘😗😚😙🥲😋😛😜🤪😝🤑🤗🤭🤫🤔🤐🤨😐😑😶😏😒🙄😬😮‍💨🤥😌😔😪🤤😴😷🤒🤕🤢🤮🥵🥶🥴😵🤯🤠🥳🥸😎🤓🧐😕😟🙁😮😯😲😳🥺🥹😦😧😨😰😥😢😭😱😖😣😞😓😩😫🥱😤😡😠🤬😈👿💀💩🤡👻👽🤖😺😸😹😻😼😽🙀😿😾") },
    { id: "People", icon: Hand, emoji: split("👋🤚🖐✋🖖👌🤌🤏✌️🤞🫰🤟🤘🤙👈👉👆👇☝️👍👎✊👊🤛🤜👏🙌🫶👐🤲🤝🙏💪🦾👀👁👅👄🫦🧠👶🧒👦👧🧑👱👨🧔👩🧓👴👵🙍🙎🙅🙆💁🙋🧏🙇🤦🤷👮🕵️💂🥷👷🤴👸👳👲🧕🤵👰🤰🤱👼🎅🤶🦸🦹🧙🧚🧛🧜🧝🧞🧟💆💇🚶🧍🧎🏃💃🕺👯🧖🧗🤺🏇⛷🏂🏌️🏄🚣🏊⛹️🏋️🚴🚵🤸🤼🤽🤾🤹🧘🛀🛌👭👫👬💏💑👪") },
    { id: "Hearts", icon: Heart, emoji: split("❤️🧡💛💚💙💜🖤🤍🤎💔❤️‍🔥❤️‍🩹❣️💕💞💓💗💖💘💝💟💌💋💯💢💥💫💦💨🕳💬🗨🗯💭💤✨🌟⭐🔥🎉🎊") },
    { id: "Nature", icon: Leaf, emoji: split("🐶🐱🐭🐹🐰🦊🐻🐼🐻‍❄️🐨🐯🦁🐮🐷🐸🐵🙈🙉🙊🐒🐔🐧🐦🐤🦆🦅🦉🦇🐺🐗🐴🦄🐝🪱🐛🦋🐌🐞🐜🪰🐢🐍🦎🦖🦕🐙🦑🦐🦞🦀🐡🐠🐟🐬🐳🐋🦈🐊🐅🐆🦓🦍🦧🐘🦛🦏🐪🐫🦒🦘🐃🐂🐄🐎🐖🐏🐑🦙🐐🦌🐕🐩🐈🐓🦃🦚🦜🦢🦩🕊🐇🦝🦨🦡🦫🦦🦥🐁🐀🐿🦔🌵🎄🌲🌳🌴🌱🌿☘️🍀🎍🎋🍃🍂🍁🍄🐚🌾💐🌷🌹🥀🌺🌸🌼🌻🌞🌝🌛🌜🌚🌕🌙🌎🪐💫⭐🌟⚡☄️💥🔥🌪🌈☀️🌤⛅🌥☁️🌦🌧⛈🌩🌨❄️☃️⛄🌬💨💧💦☔☂️🌊🌫") },
    { id: "Food", icon: Pizza, emoji: split("🍏🍎🍐🍊🍋🍌🍉🍇🍓🫐🍈🍒🍑🥭🍍🥥🥝🍅🍆🥑🥦🥬🥒🌶🫑🌽🥕🧄🧅🥔🍠🥐🥯🍞🥖🥨🧀🥚🍳🧈🥞🧇🥓🥩🍗🍖🌭🍔🍟🍕🫓🥪🥙🧆🌮🌯🫔🥗🥘🫕🍝🍜🍲🍛🍣🍱🥟🍤🍙🍚🍘🍥🥠🥮🍢🍡🍧🍨🍦🥧🧁🍰🎂🍮🍭🍬🍫🍿🍩🍪🌰🥜🍯🥛🍼☕🫖🍵🧃🥤🧋🍶🍺🍻🥂🍷🥃🍸🍹🧉🍾🧊🥄🍴🍽") },
    { id: "Activities", icon: Trophy, emoji: split("⚽🏀🏈⚾🥎🎾🏐🏉🥏🎱🪀🏓🏸🏒🏑🥍🏏🪃🥅⛳🪁🏹🎣🤿🥊🥋🎽🛹🛼🛷⛸🥌🎿🏆🥇🥈🥉🏅🎖🏵🎗🎫🎟🎪🎭🩰🎨🎬🎤🎧🎼🎹🥁🪘🎷🎺🪗🎸🪕🎻🎲♟🎯🎳🎮🎰🧩") },
    { id: "Travel", icon: Plane, emoji: split("🚗🚕🚙🚌🚎🏎🚓🚑🚒🚐🛻🚚🚛🚜🛵🏍🛺🚲🛴🚨🚔🚍🚘🚖🚡🚠🚟🚃🚋🚞🚝🚄🚅🚈🚂🚆🚇🚊🚉✈️🛫🛬🛩💺🛰🚀🛸🚁🛶⛵🚤🛥🛳⛴🚢⚓⛽🚧🚦🚥🗺🗿🗽🗼🏰🏯🏟🎡🎢🎠⛲⛱🏖🏝🏜🌋⛰🏔🗻🏕⛺🏠🏡🏘🏚🏗🏭🏢🏬🏣🏤🏥🏦🏨🏪🏫🏩💒🏛⛪🕌🕍🛕🕋⛩🌅🌄🌠🎇🎆🌇🌆🏙🌃🌌🌉🌁") },
    { id: "Objects", icon: Lightbulb, emoji: split("⌚📱💻⌨️🖥🖨🖱💽💾💿📀📷📸📹🎥📞☎️📺📻🎙⏰⌛⏳📡🔋🔌💡🔦🕯💸💵💶💷💰💳💎⚖️🧰🔧🔨⚒🛠⛏🔩⚙️🧱⛓🧲🔫💣🧨🪓🔪🗡⚔️🛡🚬⚰️🏺🔮📿🧿💈⚗️🔭🔬🕳💊💉🩸🧬🦠🧫🧪🌡🧹🧺🧻🚽🚰🚿🛁🧼🪥🪒🧽🧴🛎🔑🗝🚪🪑🛋🛏🧸🖼🛍🛒🎁🎈🎏🎀🎊🎉🎎🏮🎐🧧✉️📩📨📧💌📥📤📦🏷📪📫📬📭📮📯📜📃📄📑🧾📊📈📉🗒🗓📆📅🗑📇🗃🗳🗄📋📁📂🗂🗞📰📓📔📒📕📗📘📙📚📖🔖🧷🔗📎🖇📐📏🧮📌📍✂️🖊🖋✒️🖌🖍📝✏️🔍🔎🔏🔐🔒🔓✅☑️✔️❌❎➕➖➗➰➿〽️✳️✴️❇️‼️⁉️❓❔❕❗©️®️™️") },
  ];

  const RECENT_KEY = "alwychat.recentEmoji";
  let recent = $state<string[]>(JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]"));
  // Tab ids aren't translated (the French name used to leave the panel empty at first).
  let tab = $state(untrack(() => (recent.length ? "Recent" : "Smileys")));

  const shown = $derived(tab === "Recent" ? recent : (CATEGORIES.find((c) => c.id === tab)?.emoji ?? []));
  const customShown = $derived(tab.startsWith("pack:") ? (emojiPacks.find((p) => `pack:${p.id}` === tab)?.emotes.filter((e) => e.emoticon) ?? []) : null);
  /** A pack's tab icon: its avatar, else its first emoji. */
  const packIcon = (p: EmotePack) => mediaUrl(p.avatar ?? p.emotes[0]?.url, 22);

  function pick(emoji: string) {
    recent = [emoji, ...recent.filter((e) => e !== emoji)].slice(0, 32);
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
    onPick(emoji);
  }
</script>

<div class="picker">
  <div class="tabs">
    <button type="button" class:on={tab === "Recent"} onclick={() => (tab = "Recent")} aria-label={tr("Recent")} disabled={!recent.length}><Clock size={18} /></button>
    {#each CATEGORIES as c (c.id)}
      <button type="button" class:on={tab === c.id} onclick={() => (tab = c.id)} aria-label={c.id}><c.icon size={18} /></button>
    {/each}
    {#each emojiPacks as p (p.id)}
      <button type="button" class:on={tab === `pack:${p.id}`} onclick={() => (tab = `pack:${p.id}`)} aria-label={p.name} title={p.name}>
        <img class="pack-icon" src={packIcon(p)} alt="" />
      </button>
    {/each}
    {#if stickers.length}
      <button type="button" class:on={tab === "stickers"} onclick={() => (tab = "stickers")} aria-label={tr("Stickers")} title={tr("Stickers")}><Sticker size={18} /></button>
    {/if}
  </div>
  {#if tab === "stickers"}
    <div class="grid stickers">
      {#each stickers as e, i (e.url + i)}
        <button type="button" onclick={() => onSticker?.(e)} title={e.body ?? e.shortcode}><img src={mediaUrl(e.url, 96)} alt={e.body ?? e.shortcode} loading="lazy" /></button>
      {/each}
    </div>
  {:else if customShown}
    <div class="grid">
      {#each customShown as e (e.shortcode)}
        <button type="button" onclick={() => onCustom?.(e)} title={`:${e.shortcode}:`}><img src={mediaUrl(e.url, 40)} alt={`:${e.shortcode}:`} loading="lazy" /></button>
      {/each}
    </div>
  {:else}
    <div class="grid">
      {#each shown as e, i (e + i)}
        <button type="button" onclick={() => pick(e)}>{e}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Solid over a chat theme (which makes the surfaces see-through for its panels). */
  :global(.chat-pane.scenic) .picker {
    --surface: #15181f;
    --surface-2: #1c2029;
    --surface-hover: #2a2f3b;
    --border: #2c3240;
  }
  .picker {
    display: flex;
    flex-direction: column;
    height: 280px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  .tabs {
    display: flex;
    justify-content: space-around;
    padding: 4px;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    display: grid;
    width: 36px;
    height: 32px;
    place-items: center;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--surface-2);
    color: var(--text);
  }
  .tabs button:disabled {
    opacity: 0.3;
  }
  .grid {
    display: grid;
    flex: 1;
    grid-template-columns: repeat(auto-fill, minmax(42px, 1fr));
    align-content: start;
    padding: 6px;
    overflow-y: auto;
  }
  .grid button {
    height: 42px;
    border: none;
    border-radius: 10px;
    background: none;
    font-size: 1.6rem;
    cursor: pointer;
  }
  .grid button:hover,
  .grid button:active {
    background: var(--surface-2);
  }
  .grid img {
    width: 30px;
    height: 30px;
    object-fit: contain;
  }
  .grid.stickers {
    grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
  }
  .grid.stickers button {
    height: 84px;
  }
  .grid.stickers img {
    width: 72px;
    height: 72px;
  }
  .pack-icon {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    object-fit: cover;
  }
  .tabs {
    overflow-x: auto;
  }
</style>
