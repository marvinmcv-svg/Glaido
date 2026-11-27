# Deployments

| Part | Host | Where | Source |
|---|---|---|---|
| Webapp (Next.js) | Vercel, team "marvinmcv-svg's projects", project `vocalis-webapp` | https://vocalis-webapp-marvinmcv-svgs-projects.vercel.app | `apps/webapp`, root directory set in the project |
| Gateway (Rust) | Railway, project `vocalis`, service `gateway` | https://gateway-production-80039d71.up.railway.app | `apps/gateway/Dockerfile`, health check `/healthz` |
| Auth + database | Supabase project `vocalis` (`zptahygqxhhzshewusma`, us-east-1) | https://zptahygqxhhzshewusma.supabase.co | `supabase/migrations/*` |

All three currently follow the branch `claude/focused-carson-pf34w8`. After PR #1 merges, point Railway's source branch and Vercel's production branch at `main`.

## Configuration

- **Vercel env:** `NEXT_PUBLIC_SUPABASE_URL`, `NEXT_PUBLIC_SUPABASE_ANON_KEY` (Supabase publishable key; public by design). Vercel Authentication protects previews only, so production is public.
- **Railway variables:** `SUPABASE_URL` (the gateway verifies ES256 tokens via the project's JWKS), `RUST_LOG`, `STT_PROVIDERS`, `ANTHROPIC_CLEANUP_MODEL`, `MIN_CLIENT_VERSION`. Add `GROQ_API_KEY`, `DEEPGRAM_API_KEY` and `ANTHROPIC_API_KEY` in the Railway dashboard to enable dictation; until then `/v1/meta` reports providers as "unconfigured" and dictation returns `PROVIDER_UNAVAILABLE`.
- **Supabase:** migrations are applied through the Supabase MCP / CLI (`supabase db push`). After schema changes, run the security advisor. Two findings are intentional: signed-in users may call `is_team_member` / `is_team_owner`, which RLS policies need and which only answer for the caller.

## Still to do by hand

1. Supabase dashboard → Authentication → URL Configuration: set **Site URL** to the Vercel URL and add `https://vocalis-webapp-marvinmcv-svgs-projects.vercel.app/auth/callback` and `vocalis://auth/callback` to the redirect URLs. Until then, confirmation emails and magic links point at the Supabase default.
2. Authentication → Providers: enable Google, GitHub and Apple once you have client IDs.
3. Email: hosted Supabase's built-in mailer is rate limited; configure custom SMTP before inviting users.
