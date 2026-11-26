import Link from "next/link";
import { isSupabaseConfigured } from "@/lib/env";
import { pickPreserved, safeNext, withParams } from "@/lib/params";
import { AuthForm } from "./AuthForm";
import { Brand } from "./Brand";

type SearchParams = Record<string, string | string[] | undefined>;

export function AuthPage({ mode, searchParams }: { mode: "login" | "signup"; searchParams: SearchParams }) {
  const preserved = pickPreserved(searchParams);
  const next = safeNext(typeof searchParams.next === "string" ? searchParams.next : null);
  const error = typeof searchParams.error === "string" ? searchParams.error : null;
  const other = mode === "login" ? "/signup" : "/login";

  return (
    <main className="auth">
      <div className="auth__panel">
        <Brand />
        <h1>{mode === "login" ? "Welcome back" : "Create your account"}</h1>
        <p>
          {mode === "login"
            ? "Sign in to sync your dictionary, snippets and settings."
            : preserved.get("plan") === "pro"
              ? "Start with Pro. You can change plans at any time."
              : "Free to start. Your words stay on your device."}
        </p>
        {error === "callback" ? (
          <div className="notice" data-kind="error" role="alert" style={{ marginBottom: 16 }}>
            That sign-in link didn't work. It may have expired or already been used. Request a new one below.
          </div>
        ) : null}
        {isSupabaseConfigured() ? (
          <AuthForm mode={mode} preserved={preserved.toString()} next={next} />
        ) : (
          <div className="notice" data-kind="error" role="alert">
            Sign-in isn't configured on this server yet. Set the Supabase environment variables (see .env.example).
          </div>
        )}
        <div className="foot">
          {mode === "login" ? "New here? " : "Already have an account? "}
          <Link href={withParams(other, preserved)}>{mode === "login" ? "Create an account" : "Sign in"}</Link>
        </div>
      </div>
    </main>
  );
}
