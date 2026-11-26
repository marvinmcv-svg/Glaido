import { expect, test } from "@playwright/test";

test("login page renders sign-in options", async ({ page }) => {
  await page.goto("/login");
  await expect(page.getByRole("heading", { name: "Welcome back" })).toBeVisible();
  for (const p of ["Google", "GitHub", "Apple"]) {
    await expect(page.getByRole("button", { name: `Continue with ${p}` })).toBeVisible();
  }
  await expect(page.getByLabel("Email")).toBeVisible();
  await expect(page.getByRole("button", { name: "Sign in" })).toBeDisabled();
});

test("signup keeps plan params and drops unknown ones", async ({ page }) => {
  await page.goto("/signup?plan=pro&cadence=annual&source=pricing&token=leak");
  await expect(page.getByText("Start with Pro.")).toBeVisible();
  const href = await page.getByRole("link", { name: "Sign in" }).getAttribute("href");
  expect(href).toBe("/login?plan=pro&cadence=annual&source=pricing");
});

test("account is protected", async ({ page }) => {
  await page.goto("/account");
  await expect(page).toHaveURL(/\/login\?next=%2Faccount/);
});

test("bad callback shows a helpful error", async ({ page }) => {
  await page.goto("/auth/callback?plan=pro");
  await expect(page).toHaveURL(/\/login\?plan=pro&error=callback/);
  await expect(page.getByRole("alert").filter({ hasText: "sign-in link" })).toContainText("didn't work");
});
