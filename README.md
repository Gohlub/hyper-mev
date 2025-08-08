# Hyper‑MEV: Aave Liquidation Syndicate MVP

A Hyperware app plus a native Artemis bridge that collaboratively finds, shares, and executes Aave v3 liquidation opportunities using a P2P environment.

This README documents current capabilities, how it works, how to run it, and the main TODOs.

**Note:** You need to get the most recent Hyperware binary (which actually enables the trusted P2P environment) from here: https://github.com/hyperware-ai/hyperdrive

## Current capabilities

- Hyperware process (`hyper-mev/`)
  - WebSocket endpoint at `/artemis` for a native Artemis MEV bot
  - In‑memory stores keyed by `opp_id` for opportunities, intents, and receipts
  - P2P messages for opportunity broadcast, intent submission, and receipt sharing
  - Simple deterministic allocation planner (per‑opp coverage using received‑order)
  - Forwards selected intents to Artemis over WS for execution and stores receipts returned by Artemis

- Artemis bridge (`artemis-bridge/`)
  - WS client that connects to a Hyperware node
  - Runs an Artemis engine on a single thread via `LocalSet` (non‑Send compatible)
  - Aave v3 liquidation strategy stub that:
    - Monitors recent blocks and liquidation events
    - Heuristically identifies potential opps (simplified HF and profit checks)
    - Broadcasts opportunities to Hyperware over WS
    - Accepts `IntentCollection` and returns a simulated `ExecutionReceipt`

Notes
- Execution is simulated. On‑chain liquidation call + routing is not wired yet.
- Opportunity detection is simplified. It demonstrates end‑to‑end messaging flows.

## How it works (high level)

1) Detect & Broadcast (Finder)
- Artemis strategy scans chain and, on a candidate, sends `ArtemisMessage::OpportunityBroadcast` to Hyperware.
- Hyperware stores the opp and broadcasts a P2P `MevMessage::OpportunityBroadcast` to peers.

2) Commit Liquidity (Capital Providers)
- Peers respond with `MevMessage::IntentSubmission` which Hyperware stores in‑memory by `opp_id`.

3) Allocate Capital (Deterministic, per‑opp coverage)
- Hyperware runs a simple allocator: sort intents by `received_at`, pick until `max_repay_amount` is covered.
- Selected intents are sent to Artemis via `ArtemisMessage::IntentCollection`.

4) Execute (Executor)
- Artemis simulates execution and responds with `ArtemisMessage::ExecutionReceipt`.
- Hyperware stores the receipt and broadcasts `MevMessage::ExecutionReceipt` to peers.

5) Split/Accounting (stub)
- Hyperware includes a placeholder for proceeds calculation; full deterministic splitting across multi‑opp is a TODO.

## Message types (bridged)

- WebSocket Artemis <-> Hyperware
  - `ArtemisMessage::NodeConfig` (Hyperware -> Artemis on connect)
  - `ArtemisMessage::OpportunityBroadcast` (Artemis -> Hyperware)
  - `ArtemisMessage::IntentCollection` (Hyperware -> Artemis)
  - `ArtemisMessage::ExecutionReceipt` (Artemis -> Hyperware)

- P2P (Hyperware <-> Peers)
  - `MevMessage::OpportunityBroadcast`
  - `MevMessage::IntentSubmission`
  - `MevMessage::ExecutionReceipt`

## Quick Demo (Single Node)

This demonstrates the P2P MEV coordination flow with simulated liquidation opportunities.

1. **Build the Hyperware app** (from repo root):
```bash
cd /path/to/hyper-mev
kit b --hyperapp
```

2. **Build the Artemis bridge**:
```bash
cd artemis-bridge
cargo build --release
```

3. **Start Hyperware** (in terminal 1):
```bash
kit s
```

4. **Run Artemis bridge** (in terminal 2):
```bash
cd artemis-bridge
cargo run --release
```

5. **Watch the coordination flow**:
- Every 15 seconds, Artemis finds a liquidation opportunity
- Hyperware receives it and broadcasts to P2P peers
- If configured as a Capital Provider, it auto-submits intents
- If configured as an Executor, it runs allocation and sends to Artemis
- Artemis simulates execution and returns receipts
- Receipts are stored and broadcast to peers

You'll see output like:
```
🎯 Found Liquidation Opportunity:
   Victim: 0x742d35Cc6634C0532925a3b844D0C4E7F2a21eBc
   Health Factor: 950000000000000000
   Max Repay: $1500 USDC
   Est. Profit: $75
   
📡 Received opportunity from Artemis:
   Opp ID: 123e4567-e89b-12d3-a456-426614174000
   ✅ Broadcasting to 0 P2P peers...

🎮 Executing opportunity 123e4567-e89b-12d3-a456-426614174000:
   Sending 1 intents to Artemis for execution

✅ Execution Receipt from Artemis:
   Status: Success
   Total proceeds: $2000000000000000000
   Gas cost: $50000000 USDC
```

## Multi-Node Demo

For true P2P coordination between multiple nodes:

1. **Start multiple Hyperware nodes**:
```bash
# Terminal 1 - Alice
kit s --fake-node alice.os

# Terminal 2 - Bob  
kit s --fake-node bob.os
```

2. **Connect nodes via UI** (http://localhost:8080):
- On Alice: Click "Connect to Peer", enter `bob.os`
- On Bob: Click "Connect to Peer", enter `alice.os`

3. **Configure roles**:
- Alice: Enable "Finder" and "Capital Provider"
- Bob: Enable "Capital Provider" and "Executor"

4. **Run Artemis on Alice's port** (Terminal 3):
```bash
cd artemis-bridge
cargo run --release
```

5. **Observe P2P coordination**:
- Artemis → Alice (opportunity found)
- Alice → Bob (P2P broadcast)
- Bob → Alice (intent submission)
- Alice → Artemis (collected intents)
- Artemis → Alice (execution receipt)
- Alice → Bob (receipt broadcast)

## Build & run

Prereqs
- Hyperware dev kit installed (`kit`)
- Rust toolchain

### Environment Variables

Set the following environment variables before running:

```bash
# For Artemis Bridge
export ETH_WS_URL="wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"  # Your Ethereum RPC endpoint
export HYPERWARE_WS_URL="ws://localhost:8080/hyper-mev:hyper-mev:template.os"  # Hyperware WebSocket URL
```

Build Hyper‑MEV app (Hyperware process)
```bash
cd /Users/you/path/to/hyper-mev
kit b --hyperapp
```

Build Artemis bridge
```bash
cd /Users/you/path/to/hyper-mev/artemis-bridge
cargo build
```

Run
1) Start Hyperware app (one or more nodes) using your normal Hyperware workflow (e.g., `kit s`, multi‑node fake setup if desired).
2) Launch Artemis bridge:
```bash
cd /Users/you/path/to/hyper-mev/artemis-bridge
RUST_LOG=info cargo run
```
3) Artemis connects to `ws://localhost:8080/artemis` and begins broadcasting simulated opportunities.

Config
- Replace the placeholder Ethereum WS URL in `artemis-bridge/src/main.rs` with a valid provider key:
  - `wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY`

## Repo structure (key paths)

```
hyper-mev/
├── hyper-mev/src/lib.rs            # Hyperware process (WS, P2P, state, allocation)
├── artemis-bridge/src/main.rs      # Artemis bridge entrypoint (WS client, engine lifecycle)
├── artemis-bridge/src/aave_strategy.rs
│                                    # Aave strategy stub + opportunity broadcast
└── artemis-bridge/src/types.rs      # Shared Artemis-side message types
```

## Limitations

- Liquidation execution and routing are simulated; the on‑chain path is not yet implemented.
- HF tracking and EV calculation are simplified; no real price/routing engine.
- Signatures/attestations on P2P messages are not enforced.
- No persistence; all state is in‑memory per the MVP.

## TODO 
- Strategy (Finder)
  - Accurate HF detection and candidate set maintenance
  - EV simulation: liquidationCall → swap seized collateral → normalize to USDC → gas and slippage
  - Include `finder_fee_bps`, `close_factor_bps`, route hints, and sim head in opportunities

- Capital & Intents (CP)
  - Signature verification and expiry enforcement on intents
  - Richer CP constraints: `max_gas_gwei`, partial coverage preferences

- Allocation (Multi‑opp aware)
  - Full multi‑opp capital planner maximizing mesh EV across opps
  - Deterministic tie‑breakers per spec (key lex order, rounding rules)

- Execution (Executor)
  - Election policy (first‑ready lock, micro‑auction)
  - Real execution via private bundles; RBF fallback with gas escalator
  - Batched execution across multiple opps

- Receipts & Splits
  - Normalize proceeds to USDC and compute deterministic splits across opps
  - Aggregate CP earnings when capital is split across multiple opps

- Networking & P2P
  - Harden P2P message handlers and backpressure policies
  - Gossip and peer discovery improvements

- Observability & Ops
  - Structured logging, metrics, tracing
  - Health checks and UI status panels

- Code quality
  - Remove warnings (`GasBidInfo` import, unused variables/variants)
  - Unify shared types to avoid drift between Hyperware and Artemis bridge

## Developer notes

- WebSocket server setup is in `hyper-mev/src/lib.rs` initializer; it binds `/artemis` and pushes initial `NodeConfig` to Artemis on connect.
- Artemis engine is started with `tokio::task::LocalSet` and `spawn_local` to avoid `Send` constraints from the 3rd‑party engine error type.
- The allocation function currently uses received‑order per opp to demonstrate deterministic behavior. Replace with the full planner when ready.

This MVP is intended to validate the end‑to‑end flow (Finder → CP → Executor) and act as a foundation for wiring real on‑chain logic and the full planner.
