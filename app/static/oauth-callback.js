// Separate file so a strict Content-Security-Policy (no inline scripts) still allows it.
// Hand the address (it carries the sign-in result) to the AlwyChat tab that started the sign-in.
// window.opener is often gone by now: the server's sign-in page sends a Cross-Origin-Opener-Policy
// that cuts the link to the opener (mobile browsers, which open the "popup" as a tab). So also
// say it on a same-origin BroadcastChannel, which reaches the AlwyChat tab either way.
const message = { type: "alwychat-login", url: location.href };
try {
  new BroadcastChannel("alwychat-login").postMessage(message);
} catch {}
if (window.opener) {
  try {
    window.opener.postMessage(message, location.origin);
  } catch {}
}
document.querySelector("p").textContent = "Signed in. You can close this tab and go back to AlwyChat.";
setTimeout(() => window.close(), 300);
