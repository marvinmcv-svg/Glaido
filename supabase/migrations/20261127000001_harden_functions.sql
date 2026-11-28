-- Hardening from the Supabase security advisor (2026-11-27).

-- Pin search_path on trigger functions (lint 0011).
alter function public.set_updated_at() set search_path = public;
alter function public.check_rule_uniqueness() set search_path = public;

-- handle_new_user is only a trigger on auth.users; nobody should call it over the API (lints 0028/0029).
revoke execute on function public.handle_new_user() from public, anon, authenticated;

-- The team helpers back RLS policies evaluated as signed-in users, so `authenticated`
-- keeps EXECUTE (they only answer for auth.uid()). Anonymous callers get nothing.
revoke execute on function public.is_team_member(uuid) from public, anon;
revoke execute on function public.is_team_owner(uuid) from public, anon;
grant execute on function public.is_team_member(uuid) to authenticated;
grant execute on function public.is_team_owner(uuid) to authenticated;
