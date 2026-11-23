# Publishing AlwyChat on Google Play

What the Play build is, how to make it, and what to fill in on Play Console. The answers below
describe the app as it is; check them again when a feature that touches permissions changes.

## The build

`scripts/play-build.sh [VERSION]` → `~/alwychat-play-<version>.aab` (see the top of the script).
Compared with the builds published elsewhere, the Play build:

- never updates itself and doesn't ask for `REQUEST_INSTALL_PACKAGES` (Play refuses both);
- hides Settings → About → Updates;
- suggests `PLAY_DEFAULT_SERVER` (`.release.env`; matrix.org when unset) on the sign-in screen.

Already right for Play: target API 37, 64-bit only (arm64), native libraries aligned for 16 KB
pages, no cleartext traffic, backups off, photos through the system picker (no media
permission), no background location, release builds not debuggable.

Package name: `be.alwyapps.chat` (it can never change once uploaded). It is a different app
from the `com.alwy.chat` APKs published before 2026-11-23: people who have one of those install
the new app, sign in again and uninstall the old one.

Signing: Play App Signing with a key Google makes (the default). The bundle is signed with the
project's Android key (the buildsign account), which Play registers as the upload key.

## Before the first upload

1. Publish the privacy policy: GitHub → repository Settings → Pages → "Deploy from a branch",
   branch `main`, folder `/docs`. It then lives at
   <https://arlextech.github.io/alwychat/privacy> (the app's About page links there).
2. Play Console → create the app: name "AlwyChat", default language English, type App, Free,
   category **Communication**.
3. A new personal developer account must first run a closed test with 12 testers for 14 days
   before production is unlocked.

## Store listing

Short description (80 characters max):

> Chats, calls, spaces and voice channels on Matrix. Encrypted, free, no ads.

Full description:

> AlwyChat is a messenger for the Matrix network. Sign in to the Matrix server you choose and
> talk with anyone on Matrix, whatever app they use.
>
> • Chats with photos, videos, files, voice messages, polls, replies, threads and reactions
> • End-to-end encryption, device verification by QR code or emoji
> • Voice and video calls, screen sharing, group calls
> • Spaces with voice channels you drop into, like on Discord
> • Themes with animated backgrounds, nicknames and a quick reaction per chat
> • Folders, pinned and archived chats, timed mutes
> • Search through all your chats, encrypted ones included, on your device
> • Location sharing, once or live
> • Disappearing messages and view-once photos
> • Chat bubbles on phones that allow them
> • App lock with a PIN or your fingerprint
>
> No ads, no tracking. AlwyChat is free software (AGPL): the source code is public.
>
> You need an account on a Matrix server to use AlwyChat.

French:

> Discussions, appels, espaces et salons vocaux sur Matrix. Chiffré, libre, sans pub.

> AlwyChat est une messagerie pour le réseau Matrix. Connecte-toi au serveur Matrix de ton
> choix et parle avec n'importe qui sur Matrix, quelle que soit son application.
>
> • Discussions avec photos, vidéos, fichiers, messages vocaux, sondages, réponses, fils et réactions
> • Chiffrement de bout en bout, vérification des appareils par QR code ou emoji
> • Appels audio et vidéo, partage d'écran, appels de groupe
> • Espaces avec salons vocaux où l'on entre d'un geste, comme sur Discord
> • Thèmes aux fonds animés, surnoms et réaction rapide par discussion
> • Dossiers, discussions épinglées et archivées, mises en sourdine temporaires
> • Recherche dans toutes tes discussions, même chiffrées, sur ton appareil
> • Partage de position, ponctuel ou en direct
> • Messages éphémères et photos à vue unique
> • Bulles de discussion sur les téléphones qui les autorisent
> • Verrouillage par code ou empreinte
>
> Sans pub ni pistage. AlwyChat est un logiciel libre (AGPL) : son code source est public.
>
> Il faut un compte sur un serveur Matrix pour utiliser AlwyChat.

Graphics: `docs/play/icon-512.png`, `docs/play/feature-graphic.png`. Still to make: at least
two phone screenshots (chat list, a chat, a call), taken on a phone with nothing private on
screen.

## App content (Policy → App content)

- **Privacy policy**: <https://arlextech.github.io/alwychat/privacy>
- **Ads**: no.
- **App access**: sign-in is required, so Google's reviewers need an account. Give them a test
  account on a server they can reach: server name, username, password, and "sign-in opens the
  server's page in the browser". Without it the review is rejected.
- **Content rating**: questionnaire category "Communication"; users can talk to each other and
  share content and location; content isn't moderated by the app; users can block and report.
- **Target audience**: 13 and over (18+ avoids the families policy entirely).
- **Account deletion** (asked because accounts can be created from the app's invite screen):
  link to the "Deleting your data or your account" part of the privacy policy.
- **User-generated content**: the app has Report (chat settings → Report, sent to the server's
  administrators), Block (someone's profile → Block, "Decline and block" on invites, Settings
  → Blocked people) and leaving chats; the server's own terms apply. Reporting a single
  message isn't there yet: worth adding if a reviewer asks for it.

### Data safety

AlwyChat's developer receives nothing; but Google counts data leaving the phone for the
servers the app talks to. A cautious declaration:

| Data | Collected | Shared | Purpose | Optional |
| --- | --- | --- | --- | --- |
| User IDs, name | yes | no | App functionality, account management | no |
| Messages (other in-app messages) | yes | no | App functionality | no |
| Photos and videos, voice recordings, files | yes | no | App functionality | yes |
| Precise and approximate location | yes | no | App functionality | yes |
| Device or other IDs (push token) | yes | no | App functionality | yes |

Encrypted in transit: yes. Users can request deletion: yes (through their server). No data for
advertising or analytics.

### Permission declarations

- **Foreground services** (each needs a short description and a video): microphone and camera
  = an ongoing voice/video call continues when the app is left; location = a live location
  share the user started, shown with a Stop button.
- **Full-screen intent**: "making and receiving calls" (incoming calls ring over the lock
  screen).
- **Ignore battery optimisations**: offered from Settings → Notifications so calls and messages
  arrive on phones that freeze background apps, and needed with UnifiedPush (no Google push).
  If Google objects, remove the permission from the Play manifest overlay
  (`gen/android/app/src/play/AndroidManifest.xml`).

## Notifications on any server

Notifications while the app is closed need a push gateway holding AlwyChat's Firebase key. A
server that names its own in its well-known uses that one; for every other server the Play
build carries a public gateway (`PLAY_PUSH_GATEWAY` in `.release.env`). The gateway only sees
chat and message IDs and push tokens, never message content.
