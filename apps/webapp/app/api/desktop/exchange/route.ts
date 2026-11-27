import { NextResponse, type NextRequest } from "next/server";
import { exchange } from "@/lib/desktop-handoff";
import { supabaseEnv } from "@/lib/env";
import { createAdminClient, mintSession } from "@/lib/supabase/admin";

export const dynamic = "force-dynamic";

const NO_STORE = { "Cache-Control": "no-store" };

/** Desktop app redeems a one-time sign-in code with its PKCE verifier (ADR-0012). */
export async function POST(request: NextRequest) {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    body = null;
  }
  const result = await exchange(body, {
    consume: async (codeHash) => {
      const { data } = await createAdminClient()
        .from("desktop_handoffs")
        .delete()
        .eq("code_hash", codeHash)
        .gt("expires_at", new Date().toISOString())
        .select("user_id, challenge")
        .maybeSingle();
      return data ? { userId: data.user_id as string, challenge: data.challenge as string } : null;
    },
    mintSession,
  });
  if (!result.ok)
    return NextResponse.json({ error: result.error }, { status: result.status, headers: NO_STORE });
  const { url, anonKey } = supabaseEnv();
  // The URL and anon key are public; the app needs them to refresh its session.
  return NextResponse.json({ ...result.session, supabaseUrl: url, anonKey }, { headers: NO_STORE });
}
