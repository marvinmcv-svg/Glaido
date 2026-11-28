import type { Metadata } from "next";
import { redirect } from "next/navigation";
import { Brand } from "@/components/Brand";
import { isChallenge, isState, loginRedirect } from "@/lib/desktop-handoff";
import { createClient } from "@/lib/supabase/server";
import { OpenApp } from "./OpenApp";

export const metadata: Metadata = { title: "Sign in to the app" };
export const dynamic = "force-dynamic";

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

/** Desktop sign-in (ADR-0012): the app opens this page; the user confirms. */
export default async function DesktopSignIn({ searchParams }: Props) {
  const q = await searchParams;
  const challenge = typeof q.challenge === "string" ? q.challenge : "";
  const state = typeof q.state === "string" ? q.state : "";
  const valid = isChallenge(challenge) && isState(state);

  const supabase = await createClient();
  const { data } = await supabase.auth.getUser();
  if (valid && !data.user) redirect(loginRedirect(challenge, state));

  return (
    <main className="auth">
      <div className="auth__panel">
        <Brand />
        <h1>Sign in to the Vocalis app</h1>
        {valid && data.user ? (
          <>
            <p>
              Signed in as <strong>{data.user.email}</strong>. Open the app to finish. It gets its
              own session, which you can sign out of separately.
            </p>
            <OpenApp challenge={challenge} state={state} />
            <p style={{ marginTop: 24 }}>
              Not you? Sign out here first, then start again from the app.
            </p>
          </>
        ) : (
          <div className="notice" data-kind="error" role="alert">
            This link is missing details. Open Vocalis on your computer and choose Sign in again.
          </div>
        )}
      </div>
    </main>
  );
}
