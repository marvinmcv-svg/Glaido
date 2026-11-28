"use client";

import { Button } from "@vocalis/ui";
import { useRouter } from "next/navigation";
import { createClient } from "@/lib/supabase/client";

export function SignOutButton() {
  const router = useRouter();
  const signOut = async () => {
    await createClient().auth.signOut();
    router.replace("/login");
    router.refresh();
  };
  return (
    <Button variant="ghost" onClick={() => void signOut()}>
      Sign out
    </Button>
  );
}
