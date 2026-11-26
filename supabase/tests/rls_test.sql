-- RLS and constraint checks. Run after auth_shim.sql and the migrations.
-- Every check raises on failure; the script ends with "ALL RLS TESTS PASSED".
\set ON_ERROR_STOP on
set client_min_messages = warning;

insert into auth.users (id, email, raw_user_meta_data) values
  ('00000000-0000-0000-0000-00000000000a', 'a@example.test', '{"full_name":"Ada"}'),
  ('00000000-0000-0000-0000-00000000000b', 'b@example.test', '{}');

create or replace function pg_temp.act_as(u text) returns void language plpgsql as $$
begin
  perform set_config('request.jwt.claim.sub', u, false);
  execute 'set role authenticated';
end $$;

create or replace function pg_temp.expect(cond boolean, msg text) returns void language plpgsql as $$
begin
  if not cond then raise exception 'FAILED: %', msg; end if;
end $$;

-- Profile created by trigger.
select pg_temp.expect((select display_name from public.profiles where user_id = '00000000-0000-0000-0000-00000000000a') = 'Ada', 'profile trigger');

-- User A writes personalization data.
select pg_temp.act_as('00000000-0000-0000-0000-00000000000a');
insert into public.dictionary_entries (word) values ('Vocalis');
insert into public.dictionary_entries (word, misspelling, kind) values ('PostgreSQL', 'postgres', 'correction');
insert into public.snippets (trigger, content) values ('my address', '1 Main St');
insert into public.formatting_rules (kind, name) values ('all', 'All apps');
insert into public.formatting_rules (name, sites, apps) values ('Code', '{github.com}', '[{"bundleId":"com.microsoft.VSCode","os":"macos"}]');
insert into public.settings (platform, key, value) values ('macos', 'hotkeys.macos.dictation', '"fn"');

-- Duplicate dictionary entry (case-insensitive) is rejected.
do $$ begin
  insert into public.dictionary_entries (word) values ('vocalis');
  raise exception 'FAILED: duplicate dictionary entry accepted';
exception when unique_violation then null; end $$;

-- Over-long word rejected.
do $$ begin
  insert into public.dictionary_entries (word) values (repeat('x', 51));
  raise exception 'FAILED: 51-char word accepted';
exception when check_violation then null; end $$;

-- Site owned by another rule is rejected.
do $$ begin
  insert into public.formatting_rules (name, sites) values ('Dup', '{GitHub.com}');
  raise exception 'FAILED: duplicate site accepted';
exception when unique_violation then null; end $$;

-- Same app reused is rejected.
do $$ begin
  insert into public.formatting_rules (name, apps) values ('Dup2', '[{"bundleId":"COM.MICROSOFT.VSCODE","os":"macos"}]');
  raise exception 'FAILED: duplicate app accepted';
exception when unique_violation then null; end $$;

-- Prompt over 500 chars rejected.
do $$ begin
  insert into public.formatting_rules (name, prompt) values ('Long', repeat('p', 501));
  raise exception 'FAILED: 501-char prompt accepted';
exception when string_data_right_truncation then null; end $$;

-- A user cannot raise their own plan.
do $$ begin
  update public.profiles set plan = 'pro';
  raise exception 'FAILED: plan update allowed';
exception when insufficient_privilege then null; end $$;
update public.profiles set display_name = 'Ada L.';

-- A user cannot write usage.
do $$ begin
  insert into public.usage_weeks (user_id, iso_week, words_used) values (auth.uid(), '2026-W48', 0);
  raise exception 'FAILED: usage insert allowed';
exception when insufficient_privilege then null; end $$;

reset role;

-- User B sees none of A's data and cannot write rows for A.
select pg_temp.act_as('00000000-0000-0000-0000-00000000000b');
select pg_temp.expect((select count(*) from public.dictionary_entries) = 0, 'B sees A dictionary');
select pg_temp.expect((select count(*) from public.snippets) = 0, 'B sees A snippets');
select pg_temp.expect((select count(*) from public.formatting_rules) = 0, 'B sees A rules');
select pg_temp.expect((select count(*) from public.settings) = 0, 'B sees A settings');
select pg_temp.expect((select count(*) from public.profiles) = 1, 'B sees only own profile');
-- B can reuse a site A uses (uniqueness is per user).
insert into public.formatting_rules (name, sites) values ('Mine', '{github.com}');
do $$ begin
  insert into public.snippets (user_id, trigger, content) values ('00000000-0000-0000-0000-00000000000a', 'x', 'y');
  raise exception 'FAILED: B wrote a row for A';
exception when insufficient_privilege then null; end $$;
update public.snippets set content = 'hacked';
delete from public.dictionary_entries;
reset role;

select pg_temp.expect((select count(*) from public.dictionary_entries where user_id = '00000000-0000-0000-0000-00000000000a') = 2, 'A dictionary intact');
select pg_temp.expect((select content from public.snippets limit 1) = '1 Main St', 'A snippet intact');

-- Teams: members see the team, outsiders do not.
insert into public.teams (id, name, owner_id) values ('00000000-0000-0000-0000-0000000000f1', 'Acme', '00000000-0000-0000-0000-00000000000a');
select pg_temp.act_as('00000000-0000-0000-0000-00000000000b');
select pg_temp.expect((select count(*) from public.teams) = 0, 'outsider sees team');
reset role;
insert into public.team_members (team_id, user_id, status) values ('00000000-0000-0000-0000-0000000000f1', '00000000-0000-0000-0000-00000000000b', 'active');
select pg_temp.act_as('00000000-0000-0000-0000-00000000000b');
select pg_temp.expect((select count(*) from public.teams) = 1, 'member cannot see team');
do $$ begin
  insert into public.team_members (team_id, user_id) values ('00000000-0000-0000-0000-0000000000f1', '00000000-0000-0000-0000-00000000000b');
  raise exception 'FAILED: member added a member';
exception when insufficient_privilege or unique_violation then null; end $$;
reset role;

-- Deleting a user cascades their data.
delete from public.teams;
delete from auth.users where id = '00000000-0000-0000-0000-00000000000a';
select pg_temp.expect((select count(*) from public.dictionary_entries) = 0, 'cascade delete');

select 'ALL RLS TESTS PASSED' as result;
