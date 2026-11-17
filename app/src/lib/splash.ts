// The logo app.html shows before any script has arrived: gone once the app draws a real screen.
export function hideSplash() {
  const splash = typeof document === "undefined" ? null : document.getElementById("splash");
  if (!splash) return;
  splash.style.opacity = "0";
  setTimeout(() => splash.remove(), 200);
}
