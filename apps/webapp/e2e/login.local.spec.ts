import { expect, test } from "@playwright/test";

// Full auth round trip against a running local Supabase (`supabase start`).
// Opt in with E2E_SUPABASE=1; CI runs only the static specs.
test.skip(!process.env.E2E_SUPABASE, "set E2E_SUPABASE=1 with local Supabase running");

test("sign up, see the account, sign out, sign back in", async ({ page }) => {
  const email = `e2e-${Date.now()}@example.test`;
  const password = "correct-horse-battery";

  await page.goto("/signup?plan=pro&cadence=annual");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();

  await expect(page).toHaveURL(/\/account$/);
  await expect(page.getByText(`Signed in as ${email}.`)).toBeVisible();
  await expect(page.getByText("Plan: free")).toBeVisible(); // profile row created by trigger, read via RLS

  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page).toHaveURL(/\/login$/);
  await page.goto("/account");
  await expect(page).toHaveURL(/\/login\?next=%2Faccount/);

  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill("wrong-password-123");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "don't match" })).toBeVisible();

  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/account$/);
});
