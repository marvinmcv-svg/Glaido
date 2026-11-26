import type { Metadata } from "next";
import { redirect } from "next/navigation";
import { Brand } from "@/components/Brand";
import { createClient } from "@/lib/supabase/server";
import { SignOutButton } from "./SignOutButton";

export const metadata: Metadata = { title: "Account" };
export const dynamic = "force-dynamic";

export default async function AccountPage() {
  const supabase = await createClient();
  const { data } = await supabase.auth.getUser();
  if (!data.user) redirect("/login?next=/account");

  const { data: profile } = await supabase.from("profiles").select("display_name, plan, referral_code").maybeSingle();

  return (
    <main className="auth">
      <div className="auth__panel">
        <Brand />
        <h1>Your account</h1>
        <p>Signed in as {data.user.email}.</p>
        <div className="notice stack">
          <div>Plan: {profile?.plan ?? "free"}</div>
          {profile?.display_name ? <div>Name: {profile.display_name}</div> : null}
        </div>
        <div className="foot">
          <SignOutButton />
        </div>
      </div>
    </main>
  );
}
