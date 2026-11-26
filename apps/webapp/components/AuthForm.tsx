"use client";

import { Button } from "@vocalis/ui";
import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";
import { createClient } from "@/lib/supabase/client";
import { withParams } from "@/lib/params";

type Mode = "login" | "signup";
type Provider = "google" | "github" | "apple";
type Status = { kind: "error" | "success"; text: string } | null;

const PROVIDERS: { id: Provider; label: string }[] = [
  { id: "google", label: "Continue with Google" },
  { id: "github", label: "Continue with GitHub" },
  { id: "apple", label: "Continue with Apple" },
];

export function AuthForm({ mode, preserved, next }: { mode: Mode; preserved: string; next: string }) {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<Status>(null);

  const callbackUrl = () => {
    const params = new URLSearchParams(preserved);
    params.set("next", next);
    return `${window.location.origin}${withParams("/auth/callback", params)}`;
  };

  async function run(fn: () => Promise<Status | void>) {
    setBusy(true);
    setStatus(null);
    try {
      const result = await fn();
      if (result) setStatus(result);
    } catch {
      setStatus({ kind: "error", text: "We couldn't reach the sign-in service. Check your connection and try again." });
    } finally {
      setBusy(false);
    }
  }

  const onPassword = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      const supabase = createClient();
      if (mode === "signup") {
        const { data, error } = await supabase.auth.signUp({
          email,
          password,
          options: { emailRedirectTo: callbackUrl(), data: Object.fromEntries(new URLSearchParams(preserved)) },
        });
        if (error) return { kind: "error", text: error.message };
        if (!data.session) return { kind: "success", text: "Check your inbox to confirm your email, then come back here." };
      } else {
        const { error } = await supabase.auth.signInWithPassword({ email, password });
        if (error) return { kind: "error", text: "That email and password don't match. Try again or use a sign-in link." };
      }
      router.replace(next);
      router.refresh();
    });
  };

  const onMagicLink = () =>
    run(async () => {
      if (!email) return { kind: "error", text: "Enter your email first, then we'll send the link." };
      const { error } = await createClient().auth.signInWithOtp({
        email,
        options: { emailRedirectTo: callbackUrl(), shouldCreateUser: mode === "signup" },
      });
      if (error) return { kind: "error", text: error.message };
      return { kind: "success", text: `We sent a sign-in link to ${email}. It works once and expires in an hour.` };
    });

  const onOAuth = (provider: Provider) =>
    run(async () => {
      const { error } = await createClient().auth.signInWithOAuth({ provider, options: { redirectTo: callbackUrl() } });
      if (error) return { kind: "error", text: `${provider[0]!.toUpperCase()}${provider.slice(1)} sign-in isn't available right now.` };
    });

  return (
    <div className="stack">
      {PROVIDERS.map((p) => (
        <Button key={p.id} variant="ghost" className="full" disabled={busy} onClick={() => onOAuth(p.id)}>
          {p.label}
        </Button>
      ))}
      <div className="divider">or with email</div>
      <form className="stack" onSubmit={onPassword} noValidate>
        <label className="field">
          Email
          <input type="email" name="email" autoComplete="email" required value={email} onChange={(e) => setEmail(e.target.value)} />
        </label>
        <label className="field">
          Password
          <input
            type="password"
            name="password"
            autoComplete={mode === "signup" ? "new-password" : "current-password"}
            minLength={8}
            required
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        <Button type="submit" className="full" disabled={busy || !email || password.length < 8}>
          {mode === "signup" ? "Create account" : "Sign in"}
        </Button>
        <Button variant="ghost" className="full" disabled={busy} onClick={onMagicLink}>
          Email me a sign-in link instead
        </Button>
      </form>
      <div aria-live="polite">
        {status ? (
          <div className="notice" data-kind={status.kind} role={status.kind === "error" ? "alert" : undefined}>
            {status.text}
          </div>
        ) : null}
      </div>
    </div>
  );
}
