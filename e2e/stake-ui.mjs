// Scripted-signer E2E of /beacons staking on the LIVE site (testnet). Secret comes from env, never written to disk.
import { chromium } from "/workspace/quasaria-strat/frontend/node_modules/playwright/index.mjs";
const URL = "https://yogibear1323.github.io/quasaria/beacons?v=" + Date.now();
const SHOTS = "/workspace/beacons-shots/stake/";
const [secret, steps, tag] = [process.env.E2E_SECRET, (process.env.STEPS || "").split(","), process.env.TAG || "w"];
const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1280, height: 1000 } });
p.on("dialog", (d) => d.accept());
await p.goto(URL, { waitUntil: "networkidle" });
const sec = p.locator("#stake-testnet");
await sec.scrollIntoViewIfNeeded(); await p.waitForTimeout(1500);
if (steps.includes("disconnected")) await sec.screenshot({ path: SHOTS + "01-disconnected.png" });
await sec.getByRole("button", { name: "Connect wallet" }).click();
await p.getByRole("button", { name: /Import a secret key/ }).click();
await p.locator("input.secret-value").fill(secret);
await p.getByRole("button", { name: "Import", exact: true }).click();
await p.waitForTimeout(2500);
const close = p.getByRole("button", { name: /Start trading|Close/ }).first(); if (await close.isVisible().catch(() => false)) await close.click();
await p.waitForTimeout(4000); await sec.scrollIntoViewIfNeeded();
const status = async (label) => { await p.waitForFunction(() => /confirmed|failed/.test(document.querySelector("#stake-testnet [role=status]")?.textContent || ""), null, { timeout: 120000 }); const t = await sec.locator("[role=status]").textContent(); console.log(label, "=>", t); await p.waitForTimeout(5000); return t; };
let i = 2; const shot = async (name) => { await sec.scrollIntoViewIfNeeded(); await sec.screenshot({ path: SHOTS + String(i++).padStart(2, "0") + "-" + tag + "-" + name + ".png" }); };
await shot("connected");
for (const s of steps) {
  if (s === "stake") { await sec.locator("input[aria-label='Amount in QFX']").fill("1000"); await sec.getByRole("button", { name: /^No lock/ }).click(); await sec.getByRole("button", { name: "Stake QFX" }).click(); await status("stake"); await shot("staked"); }
  if (s === "add") { await sec.locator("input[aria-label='Amount in QFX']").fill("1500"); await sec.getByRole("button", { name: "Add to stake" }).click(); await status("add_stake"); await shot("added"); }
  if (s === "claim") { await sec.getByRole("button", { name: "Claim XLM" }).click(); await status("claim"); await shot("claimed"); }
  if (s === "unbond") { await sec.getByRole("button", { name: "Start unbonding" }).click(); await status("unbond"); await shot("unbonding"); }
  if (s === "paused") { await shot("peg-paused"); await sec.locator("input[aria-label='Amount in QFX']").fill("100"); console.log("stake button:", await sec.locator("button.btn").first().textContent()); }
  if (s === "error") { await sec.locator("input[aria-label='Amount in QFX']").fill("50"); console.log("stake disabled below min:", await sec.getByRole("button", { name: /Stake QFX|Add to stake/ }).isDisabled()); }
}
await b.close();
