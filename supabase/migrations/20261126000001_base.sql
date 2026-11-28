-- Vocalis base schema (master prompt §5.5).
-- Never stored server-side: transcripts, audio, clipboard, command content, history.

create extension if not exists pgcrypto;

-- ---------------------------------------------------------------------------
-- Helpers
-- ---------------------------------------------------------------------------
create or replace function public.set_updated_at() returns trigger
language plpgsql as $$
begin
  new.updated_at := now();
  return new;
end $$;

-- ---------------------------------------------------------------------------
-- Enums
-- ---------------------------------------------------------------------------
create type public.plan_tier as enum ('free', 'pro', 'team', 'enterprise');
create type public.plan_source as enum ('web', 'app_store', 'play');
create type public.team_role as enum ('owner', 'member');
create type public.member_status as enum ('invited', 'active', 'removed');
create type public.billing_cadence as enum ('monthly', 'annual');
create type public.dictionary_kind as enum ('vocab', 'correction');
create type public.rule_kind as enum ('all', 'email', 'custom');
create type public.format_style as enum ('standard', 'casual', 'lowercase');
create type public.settings_platform as enum ('macos', 'windows', 'linux', 'ios', 'android', 'all');
create type public.referral_status as enum ('clicked', 'signed_up', 'qualified');
create type public.deletion_status as enum ('pending', 'processing', 'completed', 'cancelled');

-- ---------------------------------------------------------------------------
-- Profiles
-- ---------------------------------------------------------------------------
create table public.profiles (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null unique references auth.users (id) on delete cascade,
  display_name text check (char_length(display_name) <= 100),
  locale text,
  ui_language text,
  timezone text,
  referral_code text not null unique default encode(gen_random_bytes(6), 'hex'),
  plan public.plan_tier not null default 'free',
  plan_source public.plan_source,
  trial_ends_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

-- Create a profile when a user signs up.
create or replace function public.handle_new_user() returns trigger
language plpgsql security definer set search_path = public as $$
begin
  insert into public.profiles (user_id, display_name)
  values (new.id, coalesce(new.raw_user_meta_data ->> 'full_name', new.raw_user_meta_data ->> 'name'));
  return new;
end $$;

create trigger on_auth_user_created after insert on auth.users
  for each row execute function public.handle_new_user();

-- ---------------------------------------------------------------------------
-- Teams
-- ---------------------------------------------------------------------------
create table public.teams (
  id uuid primary key default gen_random_uuid(),
  name text not null check (char_length(name) between 1 and 100),
  owner_id uuid not null references auth.users (id) on delete restrict,
  seats int not null default 1 check (seats >= 1),
  sso_connection_id text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table public.team_members (
  id uuid primary key default gen_random_uuid(),
  team_id uuid not null references public.teams (id) on delete cascade,
  user_id uuid not null references auth.users (id) on delete cascade,
  role public.team_role not null default 'member',
  status public.member_status not null default 'invited',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (team_id, user_id)
);

-- SECURITY DEFINER so policies can check membership without recursive RLS.
create or replace function public.is_team_member(t uuid) returns boolean
language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from public.team_members
    where team_id = t and user_id = auth.uid() and status = 'active'
  ) or exists (select 1 from public.teams where id = t and owner_id = auth.uid());
$$;

create or replace function public.is_team_owner(t uuid) returns boolean
language sql stable security definer set search_path = public as $$
  select exists (select 1 from public.teams where id = t and owner_id = auth.uid());
$$;

-- ---------------------------------------------------------------------------
-- Billing and usage (written by the service role only)
-- ---------------------------------------------------------------------------
create table public.subscriptions (
  id uuid primary key default gen_random_uuid(),
  user_id uuid references auth.users (id) on delete cascade,
  team_id uuid references public.teams (id) on delete cascade,
  provider text not null,
  provider_customer_id text,
  provider_sub_id text unique,
  status text not null,
  cadence public.billing_cadence,
  current_period_end timestamptz,
  cancel_at_period_end boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check ((user_id is null) <> (team_id is null))
);

create table public.usage_weeks (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users (id) on delete cascade,
  iso_week text not null check (iso_week ~ '^\d{4}-W\d{2}$'),
  words_used int not null default 0 check (words_used >= 0),
  words_limit int,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, iso_week)
);

-- ---------------------------------------------------------------------------
-- Personalization (synced; user-owned)
-- ---------------------------------------------------------------------------
create table public.dictionary_entries (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  word text not null check (char_length(word) between 1 and 50),
  misspelling text check (misspelling is null or char_length(misspelling) between 1 and 50),
  kind public.dictionary_kind not null default 'vocab',
  deleted_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check ((kind = 'correction') = (misspelling is not null))
);
create unique index dictionary_entries_unique
  on public.dictionary_entries (user_id, lower(word), lower(coalesce(misspelling, '')))
  where deleted_at is null;

create table public.snippets (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  trigger text not null check (char_length(trigger) between 1 and 200),
  content text not null check (char_length(content) <= 10000),
  position int not null default 0,
  deleted_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table public.formatting_rules (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  kind public.rule_kind not null default 'custom',
  name text not null check (char_length(name) between 1 and 80),
  enabled boolean not null default true,
  style public.format_style not null default 'standard',
  raw boolean not null default false,
  prompt varchar(500) not null default '',
  apps jsonb not null default '[]'::jsonb check (jsonb_typeof(apps) = 'array'),
  sites text[] not null default '{}',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  check (kind <> 'all' or enabled)
);
-- One All apps rule and one Email rule per user.
create unique index formatting_rules_one_builtin
  on public.formatting_rules (user_id, kind) where kind in ('all', 'email');

-- One app or site belongs to exactly one rule (spec 4.5.3).
create or replace function public.check_rule_uniqueness() returns trigger
language plpgsql as $$
declare
  clash text;
begin
  select coalesce(a ->> 'bundleId', '') into clash
  from jsonb_array_elements(new.apps) a
  where exists (
    select 1 from public.formatting_rules r, jsonb_array_elements(r.apps) b
    where r.user_id = new.user_id and r.id <> new.id
      and lower(b ->> 'bundleId') = lower(a ->> 'bundleId')
      and coalesce(b ->> 'os', '') = coalesce(a ->> 'os', '')
  )
  limit 1;
  if clash is not null then
    raise exception 'app % already belongs to another rule', clash using errcode = 'unique_violation';
  end if;

  select s into clash
  from unnest(new.sites) s
  where exists (
    select 1 from public.formatting_rules r, unnest(r.sites) t
    where r.user_id = new.user_id and r.id <> new.id and lower(t) = lower(s)
  )
  limit 1;
  if clash is not null then
    raise exception 'site % already belongs to another rule', clash using errcode = 'unique_violation';
  end if;
  return new;
end $$;

create trigger formatting_rules_uniqueness before insert or update on public.formatting_rules
  for each row execute function public.check_rule_uniqueness();

-- Hotkeys and other settings are per platform.
create table public.settings (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  platform public.settings_platform not null default 'all',
  key text not null check (key ~ '^[a-z0-9_.]+$'),
  value jsonb not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, platform, key)
);

-- ---------------------------------------------------------------------------
-- Growth, devices, deletion
-- ---------------------------------------------------------------------------
create table public.referrals (
  id uuid primary key default gen_random_uuid(),
  referrer_id uuid not null references auth.users (id) on delete cascade,
  invitee_email text,
  invitee_user_id uuid references auth.users (id) on delete set null,
  status public.referral_status not null default 'clicked',
  reward_applied_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table public.devices (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  platform public.settings_platform not null,
  app_version text not null,
  last_seen_at timestamptz not null default now(),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table public.deletion_requests (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
  reference text not null unique default upper(encode(gen_random_bytes(5), 'hex')),
  status public.deletion_status not null default 'pending',
  completed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table public.feature_flags (
  id uuid primary key default gen_random_uuid(),
  key text not null unique,
  enabled boolean not null default false,
  rules jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table public.announcements (
  id uuid primary key default gen_random_uuid(),
  title text not null,
  body text not null,
  starts_at timestamptz not null default now(),
  ends_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

-- updated_at triggers on every table.
do $$
declare t text;
begin
  foreach t in array array['profiles','teams','team_members','subscriptions','usage_weeks',
    'dictionary_entries','snippets','formatting_rules','settings','referrals','devices',
    'deletion_requests','feature_flags','announcements']
  loop
    execute format('create trigger %I_updated_at before update on public.%I
      for each row execute function public.set_updated_at()', t, t);
    execute format('alter table public.%I enable row level security', t);
  end loop;
end $$;

-- ---------------------------------------------------------------------------
-- Row-level security
-- ---------------------------------------------------------------------------
-- Profiles: read and update your own; plan fields are protected by a column grant below.
create policy profiles_select on public.profiles for select using (user_id = auth.uid());
create policy profiles_update on public.profiles for update using (user_id = auth.uid()) with check (user_id = auth.uid());
revoke update on public.profiles from authenticated;
grant update (display_name, locale, ui_language, timezone) on public.profiles to authenticated;

-- Teams: members read; owners update. Creating teams goes through the billing flow (service role).
create policy teams_select on public.teams for select using (public.is_team_member(id));
create policy teams_update on public.teams for update using (owner_id = auth.uid()) with check (owner_id = auth.uid());
revoke update on public.teams from authenticated;
grant update (name) on public.teams to authenticated;

create policy team_members_select on public.team_members for select
  using (user_id = auth.uid() or public.is_team_member(team_id));
create policy team_members_owner_write on public.team_members for all
  using (public.is_team_owner(team_id)) with check (public.is_team_owner(team_id));

-- Billing and usage: read-only to the owner (or team members); written by the service role.
create policy subscriptions_select on public.subscriptions for select
  using (user_id = auth.uid() or (team_id is not null and public.is_team_member(team_id)));
create policy usage_weeks_select on public.usage_weeks for select using (user_id = auth.uid());

-- User-owned synced data: full CRUD on your own rows.
do $$
declare t text;
begin
  foreach t in array array['dictionary_entries','snippets','formatting_rules','settings','devices']
  loop
    execute format('create policy %I_owner on public.%I for all
      using (user_id = auth.uid()) with check (user_id = auth.uid())', t, t);
  end loop;
end $$;

create policy referrals_select on public.referrals for select using (referrer_id = auth.uid());
create policy referrals_insert on public.referrals for insert
  with check (referrer_id = auth.uid() and status = 'clicked' and reward_applied_at is null and invitee_user_id is null);

create policy deletion_requests_select on public.deletion_requests for select using (user_id = auth.uid());
create policy deletion_requests_insert on public.deletion_requests for insert
  with check (user_id = auth.uid() and status = 'pending');

create policy feature_flags_read on public.feature_flags for select to authenticated, anon using (true);
create policy announcements_read on public.announcements for select to authenticated, anon
  using (starts_at <= now() and (ends_at is null or ends_at > now()));
