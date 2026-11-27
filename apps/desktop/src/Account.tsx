import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@vocalis/ui";
import { useEffect, useState } from "react";

export type Account = { userId: string; email: string };
type AuthChanged = { account: Account | null; error?: string };

/** Signed-in state from the engine, kept current by `auth-changed` events. */
export function useAccount() {
  const [account, setAccount] = useState<Account | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [waiting, setWaiting] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<Account | null>("auth_status").then(setAccount);
    const off = listen<AuthChanged>("auth-changed", (e) => {
      setAccount(e.payload.account);
      setError(e.payload.error ?? null);
      setWaiting(false);
    });
    return () => void off.then((f) => f());
  }, []);

  const signIn = async () => {
    setError(null);
    try {
      await invoke("auth_sign_in");
      setWaiting(true);
    } catch (e) {
      setError(typeof e === "string" ? e : "Couldn't start signing in.");
    }
  };
  const signOut = async () => {
    await invoke("auth_sign_out");
    setAccount(null);
  };
  return { account, error, waiting, signIn, signOut };
}

/** Settings → Account row. */
export function AccountRow() {
  const { account, error, waiting, signIn, signOut } = useAccount();
  return (
    <>
      <div className="row">
        <div>
          <div>Account</div>
          <div className="muted">
            {account
              ? `Signed in as ${account.email}`
              : waiting
                ? "Finish signing in in your browser, then come back here."
                : "Sign in to start dictating."}
          </div>
        </div>
        {account ? (
          <Button variant="ghost" onClick={() => void signOut()}>
            Sign out
          </Button>
        ) : (
          <Button onClick={() => void signIn()}>
            {waiting ? "Open browser again" : "Sign in"}
          </Button>
        )}
      </div>
      {error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : null}
    </>
  );
}
