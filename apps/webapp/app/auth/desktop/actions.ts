"use server";

import { desktopCallbackUrl, hashCode, isChallenge, isState, newCode } from "@/lib/desktop-handoff";
import { createAdminClient } from "@/lib/supabase/admin";
import { createClient } from "@/lib/supabase/server";

export type ApproveResult = { url: string } | { error: string };

/** The signed-in user confirmed: store a one-time code and hand back the deep link. */
export async function approveDesktop(challenge: string, state: string): Promise<ApproveResult> {
  if (!isChallenge(challenge) || !isState(state))
    return { error: "This sign-in link is incomplete. Start again from the Vocalis app." };
  const supabase = await createClient();
  const { data } = await supabase.auth.getUser();
  if (!data.user) return { error: "Your session ended. Sign in again, then retry." };

  const code = newCode();
  const admin = createAdminClient();
  // Opportunistic cleanup keeps the table tiny.
  await admin.from("desktop_handoffs").delete().lt("expires_at", new Date().toISOString());
  const { error } = await admin
    .from("desktop_handoffs")
    .insert({ code_hash: hashCode(code), challenge, user_id: data.user.id });
  if (error) return { error: "We couldn't finish signing in. Please try again." };
  return { url: desktopCallbackUrl(code, state) };
}
