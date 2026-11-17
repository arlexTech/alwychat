// Invite links (/register?token=…) show the main page directly instead of redirecting to it:
// one page load less on slow networks. The page reads the token (lib/invite.ts).
import type { Reroute } from "@sveltejs/kit";

export const reroute: Reroute = ({ url }) => (url.pathname === "/register" ? "/" : undefined);
