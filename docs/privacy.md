---
title: AlwyChat privacy policy
---

# AlwyChat privacy policy

*Last updated: 23 November 2026*

AlwyChat is a free, open-source app ([source code](https://github.com/arlexTech/alwychat),
AGPL-3.0-or-later) for chatting and calling on the [Matrix](https://matrix.org) network. This
page says what happens to your data when you use it, on Android, iPhone, computers and the web.

## The short version

- AlwyChat has no ads, no analytics and no trackers.
- AlwyChat's developer runs no service that the app reports to, and receives none of your
  messages, contacts, files or usage data.
- Your account and your chats live on the Matrix server you sign in to (your "homeserver"),
  which you choose. That server's own privacy policy applies to what it stores.

## What the app sends, and to whom

**Your homeserver.** Everything you do in a chat goes to the homeserver you signed in to, and
from there to the servers of the people you talk with: messages, photos, videos, files, voice
messages, reactions, read receipts, typing notices, your profile (name, photo, status, time
zone if you share it) and whether you're online (if you leave that on). In encrypted chats the
content is end-to-end encrypted: servers can't read it. Some things are not encrypted and can
be read by your homeserver: who is in which chat, when messages are sent, your profile, and the
settings AlwyChat keeps in your account so your devices agree (folders, chat list choices,
muted chats, unsent drafts, chat themes and nicknames).

**Notifications.** To notify you while the app is closed, the Android app gets a push token
from Google's Firebase Cloud Messaging and gives it to your homeserver, which uses its push
gateway to wake the app. The pushed message only holds the IDs of a chat and of a message,
never its content: the app then fetches and decrypts the message itself. You can use a
UnifiedPush app instead of Google's service (Settings → Notifications → Push service), or turn
notifications off.

**Calls.** Voice and video calls go through the call server (LiveKit) that your homeserver
names. In encrypted chats, call audio and video are end-to-end encrypted.

**Link previews.** When a message has a link, the preview card is made by your homeserver, not
by the app visiting the site.

**Location.** Only when you choose "Share location" (once, or live for the time you pick) is
your position read and sent to that chat. AlwyChat never reads your location otherwise, and
not while the app is closed unless a live share you started is running (a notification shows
it, with a Stop button).

**Camera, microphone, photos and files.** Used only when you take a photo or video, record a
voice message, are in a call, or pick something to send. The app reads only what you pick.

**Updates.** Versions installed from Google Play are updated by Google Play. Versions
downloaded elsewhere check for a newer version a few times a day by fetching a small file from
where they were published (GitHub, or the site that offered the download); nothing about you
is sent beyond what any web request shows (your IP address).

**Problem reports.** "Report a problem" makes a text file with the app's recent technical log
and sends it only to the chat you pick. Nothing is sent automatically.

## What stays on your device

Your session, encryption keys, a copy of recent messages and the search index of your chats
are kept in the app's private storage (on computers, the sign-in secrets are in the system
keychain). Photos and videos you save, or that you asked a chat to save automatically, go to
your gallery. Signing out removes the app's data from the device.

## Deleting your data or your account

- Delete a message: long-press it → Delete. It is removed for everyone, as far as their
  servers and apps honour it.
- Sign out (Settings → Sign out) removes everything AlwyChat keeps on that device.
- Your account belongs to your homeserver. To delete it, open Settings → Manage account in the
  app, which opens your homeserver's account page, or ask your homeserver's administrator.
  AlwyChat's developer has no access to accounts on servers they don't run.

## Children

AlwyChat is not directed at children under 13, and your homeserver may set a higher minimum
age.

## Changes and contact

Changes to this policy are published on this page with a new date. Questions or requests:
open an issue at <https://github.com/arlexTech/alwychat/issues>.
