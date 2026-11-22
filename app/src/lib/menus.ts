// Right-click menus shared by several places (stores/menu.svelte.ts draws them).

import { tr } from "$lib/i18n/index.svelte";
import { fileAction, openDm, openExternal, type SearchHit, type SharedItem } from "$lib/backend";
import { contextMenu, type MenuEntry } from "$lib/stores/menu.svelte";
import { call } from "$lib/stores/call.svelte";
import { nav } from "$lib/stores/nav.svelte";
import { eventMediaUrl } from "$lib/media";
import { saveToGallery } from "$lib/save";
import UserRound from "@lucide/svelte/icons/user-round";
import MessageCircle from "@lucide/svelte/icons/message-circle";
import AtSign from "@lucide/svelte/icons/at-sign";
import Copy from "@lucide/svelte/icons/copy";
import Download from "@lucide/svelte/icons/download";
import Forward from "@lucide/svelte/icons/forward";
import MessageSquare from "@lucide/svelte/icons/message-square";
import ExternalLink from "@lucide/svelte/icons/external-link";
import Link from "@lucide/svelte/icons/link";
import ImageIcon from "@lucide/svelte/icons/image";
import FolderOpen from "@lucide/svelte/icons/folder-open";

/** Someone (a message's sender, a member, a 1:1 chat's person). */
export function personMenu(
  event: MouseEvent,
  person: { userId: string; name: string },
  opts: { onMention?: () => void; isMe?: boolean; extra?: MenuEntry[] } = {},
) {
  contextMenu.show(
    event,
    [
      { label: tr("Profile"), icon: UserRound, run: () => (call.profile = person.userId) },
      !opts.isMe && {
        label: tr("Message"),
        icon: MessageCircle,
        run: async () => nav.openChat(await openDm(person.userId)),
      },
      opts.onMention && { label: tr("Mention"), icon: AtSign, run: opts.onMention },
      ...(opts.extra ?? []),
      "-",
      { label: tr("Copy user ID"), icon: Copy, run: () => contextMenu.copy(person.userId) },
    ],
    person.name,
  );
}

/** A photo as PNG on the clipboard (what other apps paste). */
export async function copyImage(media: string) {
  const blob = await (await fetch(eventMediaUrl(media))).blob();
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
  bitmap.close();
  const png = await new Promise<Blob>((ok, fail) => canvas.toBlob((b) => (b ? ok(b) : fail(new Error("no image"))), "image/png"));
  await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
  return tr("Photo copied");
}

/** A photo/video, file or link of a chat (shared media, search results). */
export function sharedItemMenu(event: MouseEvent, roomId: string, item: SharedItem, onShow?: (eventId: string) => void) {
  const visual = item.media && (item.kind === "image" || item.kind === "video");
  const file = item.media && !visual;
  const entries: MenuEntry[] = [
    item.url && { label: tr("Open link"), icon: ExternalLink, run: () => openExternal(item.url!) },
    item.url && { label: tr("Copy link"), icon: Link, run: () => contextMenu.copy(item.url!) },
    file && { label: tr("Open"), icon: FolderOpen, run: () => fileAction(item.media!, "open") },
    visual && { label: tr("Save"), icon: Download, run: () => saveToGallery([item.media!]) },
    file && { label: tr("Save"), icon: Download, run: () => fileAction(item.media!, "downloads") },
    item.media && item.kind === "image" && { label: tr("Copy photo"), icon: ImageIcon, run: () => copyImage(item.media!) },
    "-",
    { label: tr("Forward"), icon: Forward, run: () => contextMenu.forward(roomId, [item.event_id]) },
    onShow && { label: tr("Show in chat"), icon: MessageSquare, run: () => onShow(item.event_id) },
  ];
  contextMenu.show(event, entries);
}

/** A search result. */
export function searchHitMenu(event: MouseEvent, hit: SearchHit, onShow: () => void) {
  const visual = hit.media && (hit.kind === "image" || hit.kind === "video");
  const file = hit.media && !visual;
  contextMenu.show(event, [
    { label: tr("Show in chat"), icon: MessageSquare, run: onShow },
    visual && { label: tr("Save"), icon: Download, run: () => saveToGallery([hit.media!]) },
    file && { label: tr("Save"), icon: Download, run: () => fileAction(hit.media!, "downloads") },
    hit.media && hit.kind === "image" && { label: tr("Copy photo"), icon: ImageIcon, run: () => copyImage(hit.media!) },
    { label: tr("Forward"), icon: Forward, run: () => contextMenu.forward(hit.room_id, [hit.event_id]) },
    "-",
    !hit.is_own && { label: tr("{name}'s profile", { name: hit.sender_name }), icon: UserRound, run: () => (call.profile = hit.sender) },
  ]);
}
