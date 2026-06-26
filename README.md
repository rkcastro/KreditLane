# KreditLane 💸

> Milestone-based escrow for SEA freelancers and student creators — powered by Soroban on Stellar.

---

## The Problem

A 20-year-old graphic design student in Cebu, Philippines takes a remote gig from a Singapore-based startup for ₱15,000. She delivers the first batch of assets, but the client ghosts her — no payment, no recourse, no legal leverage across borders. She loses two weeks of work.

## The Solution

KreditLane lets the client lock USDC into a Soroban escrow contract before work begins. The creator submits each milestone on-chain; the client approves and the contract releases exactly 1/N of the funds instantly. If there's a dispute, a trusted admin arbitrates — all logic enforced by code, not promises. Stellar's sub-cent fees and 5-second finality make even ₱500 micro-gigs economically viable.

---

## Stellar Features Used

| Feature | Why |
|---|---|
| **Soroban Smart Contracts** | Trustless escrow, milestone logic, dispute resolution |
| **USDC (Custom Asset / SEP-24 anchor)** | Stable-value payments across borders |
| **XLM transfers** | Gas & optional low-value micropayment path |
| **Trustlines** | Client opts-in to USDC before depositing |

---

## Target Users

- **Creators / Freelancers** — Filipino and SEA design students, video editors, copywriters aged 18–26 earning $50–$500/gig via remote platforms (Upwork, Twitter/X DMs, Discord servers).
- **Clients** — SEA-based SMEs and solo founders who want accountability without legal overhead.

---

## Core MVP Feature (demo in < 2 min)

```
Client locks 100 USDC  →  create_job() called on-chain  →  funds held in contract
Creator delivers work  →  submit_work() called            →  status = Submitted
Client approves        →  approve_milestone() called      →  100 USDC sent to creator
```

All three steps visible in Stellar Testnet Explorer in under 90 seconds.

---

## Why This Wins

KreditLane directly addresses Stellar's remittance + gig economy target market with a real, demo-able user story. The Soroban contract is lean (< 200 lines), composable with any USDC anchor, and the dispute flow proves admin-gated governance — hitting all hackathon scoring axes: real users, local economy, on-chain logic, and composability.

---

## Optional Edge (Bonus)

**AI work verification**: integrate a Claude API call (off-chain Oracle pattern) that scores whether a submitted design brief PDF matches the original job description before the `submit_work` transaction is sent — reducing frivolous disputes.

---

## Project Structure

```
kredit_lane/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs      # Soroban contract (escrow + milestone + dispute)
    └── test.rs     # 5 unit tests
```

---

## Prerequisites

| Tool | Version |
|---|---|
| Rust | `>= 1.74` (install via [rustup](https://rustup.rs)) |
| Soroban CLI | `>= 21.0.0` (`cargo install --locked soroban-cli`) |
| Stellar Testnet account | Free via [Stellar Laboratory](https://laboratory.stellar.org) |

Add the Wasm target:
```bash
rustup target add wasm32-unknown-unknown
```

---

## Build

```bash
soroban contract build
# Output: target/wasm32-unknown-unknown/release/kredit_lane.wasm
```

---

## Test

```bash
cargo test
```

Expected output:
```
test tests::test_happy_path_single_milestone        ... ok
test tests::test_wrong_caller_cannot_approve        ... ok
test tests::test_state_after_first_of_three_milestones ... ok
test tests::test_dispute_resolved_for_client        ... ok
test tests::test_cannot_resubmit_completed_job      ... ok

test result: ok. 5 passed; 0 failed
```

---

## Deploy to Testnet

```bash
# 1. Fund a testnet account (get XLM from Friendbot)
soroban keys generate --global my-key --network testnet
soroban keys fund my-key --network testnet

# 2. Deploy the contract
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/kredit_lane.wasm \
  --source my-key \
  --network testnet

# Returns: CONTRACT_ID (copy this for the next steps)

# 3. Initialise (set admin)
soroban contract invoke \
  --id CONTRACT_ID \
  --source my-key \
  --network testnet \
  -- initialize \
  --admin GADMIN_ADDRESS
```

---

## Sample CLI Invocations (MVP Flow)

```bash
# Step 1 — Client creates job (locks 100 USDC, 2 milestones)
soroban contract invoke \
  --id CONTRACT_ID \
  --source client-key \
  --network testnet \
  -- create_job \
  --client GCLIENT_ADDRESS \
  --creator GCREATOR_ADDRESS \
  --token USDC_CONTRACT_ID \
  --amount 1000000000 \
  --milestone_count 2

# Step 2 — Creator submits work for milestone 1
soroban contract invoke \
  --id CONTRACT_ID \
  --source creator-key \
  --network testnet \
  -- submit_work \
  --creator GCREATOR_ADDRESS \
  --job_id 1

# Step 3 — Client approves → 50 USDC released to creator
soroban contract invoke \
  --id CONTRACT_ID \
  --source client-key \
  --network testnet \
  -- approve_milestone \
  --client GCLIENT_ADDRESS \
  --job_id 1

# View job state at any time
soroban contract invoke \
  --id CONTRACT_ID \
  --network testnet \
  --source my-key \
  -- get_job \
  --job_id 1
```

---

## Vision & Purpose

Millions of young creators across the Philippines, Indonesia, and Vietnam are building freelance careers on social platforms — but they are one bad client away from losing their work with no recourse. KreditLane is the trust layer that makes cross-border gig work safe at the speed and cost of Stellar. Long term, completed job history stored on-chain becomes a verifiable credit record — the first step toward on-chain lending for the unbanked creative economy.

---

## License

MIT © 2025 KreditLane Contributors