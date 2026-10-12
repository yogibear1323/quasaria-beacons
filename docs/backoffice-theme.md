# Quasaria Back Office — "Beacons" theme spec

Tagline: **Real light, not promised light.** Calm, confident, transparent; the opposite of hype yield sites. The glow is the proof: nothing glows unless a real on-chain event just happened.

Implementation: `frontend/src/theme/beacons.css` (scoped to `.bo.bk`, Back Office only), `src/lib/beaconPulse.ts`, `src/components/BeaconsPreview.tsx`. Main-site tokens (`tokens.css`) are unchanged; the theme remaps shared tokens only inside `.bo.bk`.

## 1. Design tokens

### Colour
| Token | Hex | Use |
|---|---|---|
| `--bk-bg` | #14121F | base (lifted near-black), page backdrop, wells, chart canvas |
| `--bk-s1` | #1B1829 | card surface |
| `--bk-s2` | #221E33 | nested surface, table header |
| `--bk-s3` | #2A2540 | hover / selected / bar track |
| `--bk-line` | #342D52 | hairline |
| `--bk-line-2` | #40386A | card border |
| `--bk-text` | #ECEBF5 | primary text |
| `--bk-text-2` | #A9A4C4 | secondary |
| `--bk-text-3` | #948EB4 | labels, captions (≥4.7:1 on every surface) |
| `--bk-violet` | #8B7CFF | primary accent, bars, focus |
| `--bk-violet-hi` | #C4B5FD | primary text-on-dark, chips |
| `--bk-cyan` | #22D3EE | secondary accent, live/OK, pulse |
| `--bk-cyan-hi` | #A5F3FC | pulse highlight, tagline |
| `--bk-success` | #34D399 | profit, running |
| `--bk-warn` | #FBBF24 | paused, testnet, not live, example |
| `--bk-danger` | #FB7185 | loss, halted, STALE DATA, kill |

Tier ramp (violet → cyan, rising luminance): Spark #8573F7 · Flare #9A8CFF · Beacon #9AAAFF · Pulsar #66B8F7 · Quasar #3FD3EE · Singularity #A5F3FC.

Contrast (Oct 11 lighter pass): text 12.4–14.7:1, text-2 6.1–7.3:1, text-3 4.7–5.6:1, cyan/success/warn/danger ≥5.4:1, violet #8B7CFF 4.5–5.3:1 across s1–s3; tier labels ≥4.5:1 on s1/s2. STALE DATA fill #2A1520; table row rules #2B2643.

Tints: violet-dim rgba(139,124,255,.14), cyan-dim rgba(34,211,238,.12). No gradients on surfaces; **no gradient text (`background-clip:text` banned).**

### Typography
- UI/headings: Space Grotesk → Inter → system-ui. Numbers/labels/chips: JetBrains Mono.
- Scale: 9.5 (micro label, uppercase, +.12em) · 10.5–11 (chip/caption) · 12.5 (body small) · 14 (body) · 15 (section h2, 600) · 19 (KPI) · 22 (hero number).
- `font-variant-numeric: tabular-nums` on the whole Back Office so columns don't jitter.

### Space, radius, elevation
- 4-pt grid: 4 / 8 / 12 / 16 / 24. Card padding 14×16; grid gaps 8–16.
- Radius: sm 6 (chips, inputs), md 10 (tables, banners, tier cards), lg 14 (cards, panels).
- Elevation: flat; elev-1 = inset 1px highlight; elev-2 = soft drop shadow for sheets on mobile only. Depth comes from surface steps, not glow.

## 2. Components
- **Cards:** s1 fill, line-2 border, radius lg, no gradient.
- **Tables:** mono, tabular; header s2, uppercase 10px text-3; row hairlines; "fired" rows get a 2px cyan left rule.
- **Chips/badges:** radius sm, mono uppercase. Neutral = violet-dim/violet-hi; live/shadow = cyan-dim/cyan; `Paused` = warn; `paper · failed gate` = cyan-outlined mode chip next to a neutral `testnet / simulated` chip (gate failure stays readable in words); `STALE DATA` / halted = danger. Tier badges: hollow ring dot + tier name in its ramp colour.
- **Buttons:** ghost by default (line-2 border); active/selected = violet-dim fill + violet inset ring; destructive = danger text/border. Focus = existing `--ring`.
- **Inputs:** bg base, line-2 border; focus violet border + 3px violet-dim halo.
- **Nav/sidebar:** inherits main-site nav (unchanged, keeps site consistent); Back Office mode switch is a segmented control on s1.
- **Banners:** flat solid. STALE DATA = #1A0B10 fill, 4px danger left rule; Open items = s1 with 4px violet left rule; DEMO = solid cyan, EXAMPLE = solid amber (labels unchanged).

## 3. Data viz
- Series: violet (primary), cyan (secondary/benchmark), success/danger only for P&L sign. Strategy accents kept for desk identity.
- Sparklines: 1.5px line, no fill or 8% fill; no glow at rest.
- Gridlines: violet at 3.5% alpha, 34px; axes text-3 mono.
- Progress bars: s3 track, flat violet fill.

## 4. Motion
- **Pulse (`.bk-pulse`)**: one 1.8s ring (cyan → violet → none), run once. Triggered only by a real event signature change after first load:
  - Oracle cell: on-chain oracle timestamp advanced (oracle push landed).
  - Trading floor: a new on-chain desk trade tx, or a new Orion live-slice fill.
  - Calibrated signals: Orion logged a new closed-candle decision.
- Never on page load, never on a timer, never in demo/mirror/example modes. No idle glow; no infinite animations in the theme layer.
- `prefers-reduced-motion: reduce` → no animation; a static 1px cyan outline for the same 1.8s.
- Future: each Beacon tier card pulses once when a real payout cycle lands for it.

## 5. Voice and copy rules
- Say what happened, with the number and its source; never what will happen. No "earn up to", no APY promises, no countdowns, no exclamation marks.
- Empty is honest: "—", "No payouts yet", "Not live yet · testnet preview". Never fabricate a balance, rate or holder count.
- Label every non-real thing: testnet, test funds, simulated, example, illustrative.
- Short labels in sentence case; status chips lower-case mono.
- Never use the banned b-word terms (b*nk, b*nking) anywhere in UI or docs.

## 6. Screen-by-screen mapping
| Screen | Theme application |
|---|---|
| Header + badges | mono chips; feed OK = cyan chip, warn = amber, Testnet = amber |
| Fleet ticker | s1 cells on hairline grid; flat violet risk bar; **Oracle cell pulses on real oracle push** |
| STALE DATA breaker | flat danger banner, 4px rule, solid text |
| Mode switch / Demo account | segmented control on s1, violet active; demo card s1 with cyan hairline; inputs per spec |
| Robot floor + wall board | s1 floor, quieter grid, base board, s2 desks; **floor pulses on a real fill** |
| Desk panel | s1 panel, tabular figures |
| Calibrated desk signals (Orion) | s1 card, s2 stats, mono table, cyan rule on fired rows, `paper · failed gate`/`Paused` chips; **pulses on a new real decision** |
| Beacons (preview) — new | six tier cards on ramp colours with QFX minimums (100 / 500 / 2,500 / 10,000 / 50,000 / 250,000 QFX; 1 QFX = 1 XLM), flat share per QFX for all tiers (tiers give perks only: fee discounts, badges), lock boosts 1.10× 30d / 1.25× 90d open to all, payouts in XLM, no per-wallet cap; default Blend pool = Blend's XLM supply pool (asset-matched to XLM payouts; USDC is a non-default alternative, so it is not shown); payout-source table (trading fees by pair, Blend XLM pool interest, tx link) with "—" and "no payouts yet"; "Not live yet · testnet preview" chip |
| Open items | s1 banner, violet rule, cyan link |
| Risk warning / footer | unchanged (shared component) |
