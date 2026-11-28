import { NextResponse, type NextRequest } from "next/server";
import { pickPreserved, safeNext } from "@/lib/params";
import { createClient } from "@/lib/supabase/server";

/** OAuth and magic-link landing: exchange the code for a session, then continue. */
export async function GET(request: NextRequest) {
  const url = request.nextUrl;
  const code = url.searchParams.get("code");
  const next = safeNext(url.searchParams.get("next"));
  if (code) {
    const supabase = await createClient();
    const { error } = await supabase.auth.exchangeCodeForSession(code);
    if (!error) return NextResponse.redirect(new URL(next, url.origin));
  }
  const back = new URL("/login", url.origin);
  pickPreserved(url.searchParams).forEach((v, k) => back.searchParams.set(k, v));
  back.searchParams.set("error", "callback");
  return NextResponse.redirect(back);
}
