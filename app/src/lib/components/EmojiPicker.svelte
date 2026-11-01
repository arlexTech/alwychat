<script lang="ts">
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

  /** The composer's emoji panel (Messenger style): recent ones first, then by category. */
  let { onPick }: { onPick: (emoji: string) => void } = $props();

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
  let tab = $state(untrack(() => (recent.length ? "Recent" : "Smileys")));

  const shown = $derived(tab === "Recent" ? recent : (CATEGORIES.find((c) => c.id === tab)?.emoji ?? []));

  function pick(emoji: string) {
    recent = [emoji, ...recent.filter((e) => e !== emoji)].slice(0, 32);
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
    onPick(emoji);
  }
</script>

<div class="picker">
  <div class="tabs">
    <button type="button" class:on={tab === "Recent"} onclick={() => (tab = "Recent")} aria-label="Recent" disabled={!recent.length}><Clock size={18} /></button>
    {#each CATEGORIES as c (c.id)}
      <button type="button" class:on={tab === c.id} onclick={() => (tab = c.id)} aria-label={c.id}><c.icon size={18} /></button>
    {/each}
  </div>
  <div class="grid">
    {#each shown as e, i (e + i)}
      <button type="button" onclick={() => pick(e)}>{e}</button>
    {/each}
  </div>
</div>

<style>
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
</style>
