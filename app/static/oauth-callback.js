// Separate file so a strict Content-Security-Policy (no inline scripts) still allows it.
if (window.opener) {
  window.opener.postMessage({ type: "alwychat-login", url: location.href }, location.origin);
  setTimeout(() => window.close(), 300);
} else {
  document.querySelector("p").textContent = "You can close this window and go back to AlwyChat.";
}
