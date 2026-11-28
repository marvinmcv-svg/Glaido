import { createClient } from "@supabase/supabase-js";
import type { Session } from "../desktop-handoff";
import { supabaseEnv } from "../env";

/** Service-role client. Server only: the key bypasses RLS. */
export function createAdminClient() {
  const { url } = supabaseEnv();
  const key = process.env.SUPABASE_SERVICE_ROLE_KEY;
  if (!key) throw new Error("Set SUPABASE_SERVICE_ROLE_KEY (server only) for desktop sign-in.");
  return createClient(url, key, { auth: { persistSession: false, autoRefreshToken: false } });
}

/**
 * A brand-new session for `userId`, independent of any browser session: an
 * admin-generated magic-link token (never emailed) verified by a throwaway
 * anon client.
 */
export async function mintSession(userId: string): Promise<Session | null> {
  const admin = createAdminClient();
  const { data: u } = await admin.auth.admin.getUserById(userId);
  const email = u.user?.email;
  if (!email) return null;
  const { data: link, error } = await admin.auth.admin.generateLink({ type: "magiclink", email });
  if (error || !link.properties?.hashed_token) return null;
  const { url, anonKey } = supabaseEnv();
  const anon = createClient(url, anonKey, {
    auth: { persistSession: false, autoRefreshToken: false },
  });
  const { data } = await anon.auth.verifyOtp({
    token_hash: link.properties.hashed_token,
    type: "email",
  });
  const s = data.session;
  if (!s) return null;
  return {
    accessToken: s.access_token,
    refreshToken: s.refresh_token,
    expiresAt: s.expires_at ?? Math.floor(Date.now() / 1000) + s.expires_in,
    user: { id: s.user.id, email },
  };
}
