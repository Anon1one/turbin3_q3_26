# Vault & Escrow (Q3 2026)

Two Solana programs written with Anchor, each in its own folder with its own tests.

| | What it does |
|---|---|
| [`vault_q326`](vault_q326) | Per-user SOL vault — deposit, withdraw, close |
| [`escrow_q326`](escrow_q326) | Trustless SPL token swap — make, take, refund, update |

Both have their own README with the details. Short version below.

## vault_q326

Every user gets a vault derived from their own pubkey, so nobody can touch anyone else's.
Two PDAs: `vault_state` holds the bumps, `vault` holds the actual SOL as a plain system
account. `withdraw` and `close` sign the CPI with the vault's own seeds, since a PDA cannot
sign for itself otherwise.

## escrow_q326

Maker locks token A in a PDA-owned vault and says how much token B they want. Any taker
holding token B completes the swap in one instruction, so it can never half-finish. If no
taker turns up, the maker refunds themselves.

`update` is the one worth reading about. Of the seven fields on the escrow account only two
can change after `make` — `receive` and `expiration` — and each only in the direction that
is safe for a taker who is already mid-transaction. The reasoning, including the front-running
attack it closes and why touching `mint_a` would strand the deposit forever, is written up in
[escrow_q326/README.md](escrow_q326/README.md#about-update).

## Running either one

```bash
cd vault_q326     # or escrow_q326
anchor build
cargo test
```

Tests use LiteSVM, so there is no validator and no devnet involved. `anchor build` has to run
first because the tests load the compiled `.so` out of `target/deploy/`.
