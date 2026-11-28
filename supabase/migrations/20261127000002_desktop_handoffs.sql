-- Desktop sign-in handoff (ADR-0012). The web app, on behalf of a signed-in
-- user, stores a one-time code bound to the desktop app's PKCE challenge; the
-- desktop app redeems it once, within five minutes, with the matching verifier.
-- Only the service role (web app server) touches this table: RLS is on with no
-- policies, and anon/authenticated lose every privilege.
create table public.desktop_handoffs (
  code_hash text primary key check (code_hash ~ '^[0-9a-f]{64}$'),
  challenge text not null check (challenge ~ '^[A-Za-z0-9_-]{43}$'),
  user_id uuid not null references auth.users (id) on delete cascade,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '5 minutes'
);
create index desktop_handoffs_expires on public.desktop_handoffs (expires_at);

alter table public.desktop_handoffs enable row level security;
revoke all on public.desktop_handoffs from public, anon, authenticated;
grant select, insert, delete on public.desktop_handoffs to service_role;
