# Fixtures

Where each file came from. None holds an API key or token: every file was searched for one
before it was copied.

| File | Source |
|---|---|
| `dashboard_stats.json` | Live mainnet answer of `/dashboard/outcome-builder-stats`, as hip4-tui's `tui/src/app/tests/board.rs` holds it. That copy blanks the builder to `0x0`; it is restored here from the category answer, whose `fillsShare` and `uniqueUsersShare` are this answer's `fills` and `uniqueUsers` to the tenth place. |
| `dashboard_categories.json` | Live mainnet answer of `/dashboard/outcome-builder-category-stats` for builder `0xab5d..b704`, recorded by hip4-tui on 2026-09-18 (`net/src/tests/hydromancer.rs`). |
| `dashboard_hourly.json` | Live mainnet answer of `/dashboard/outcome-builder-hourly`, three hours of 2026-09-17, recorded by hip4-tui on 2026-09-18 (same file). |
| `dashboard_volume_daily.json` | Hydromancer's documented example of `/dashboard/outcome-builder-volume-daily` (packed), with the page's placeholder builders replaced by a real builder and the unattributed `null` bucket, as hip4-tui's test reads it. |
| `dashboard_profitability.json` | Hydromancer's documented example of `/dashboard/outcome-builder-profitability`, verbatim (docs.hydromancer.xyz, read 2026-10-08). |
| `prices_live.json` | Live mainnet answer of `perpPriceHistoryByTime` in its batch form, a native coin and a HIP-3 coin, recorded by hip4-tui on 2026-09-19 (`net/src/tests/prices.rs`). |
| `builder_fills.json` | Live mainnet `builderFills` frame from `wss://api.hydromancer.xyz/ws`, recorded by hip4-tui on 2026-09-18 (`net/src/tests/builderfills.rs`). |
| `builder_fills_*_20260926.csv.lz4` | Two builders' files for 2026-09-26 from Hyperliquid's `stats-data` bucket, stored as sent, copied from hip4-tui (`net/src/tests/fixtures`). The sums the tests expect were taken from the same files opened by the `lz4` tool and summed in Python's `decimal`. |
| `l4_update_782007304.json` | Hydromancer's documented example `l4BookUpdates` frame, verbatim. |
| `l4_update_782007305.json`, `l4_update_782007307.json` | Written for these tests in the documented shape: an ask that crosses the bids, followed by the venue's own removes and update for the orders it took; then a frame two blocks on, a gap. |
| `l4_snapshot.bin` | An `l4Book` frame in the documented layout, built by `make_l4.py` with Python's msgpack and zstandard (libzstd), not by this crate. One blob carries its size and a checksum, the other neither. No live L4 frame was on record in hip4-tui or atlas. |
| `l4_snapshot.json` | `l4_snapshot.bin` as Hydromancer's own Python SDK (`hydromancer-sdk` 4371df8, `unpack_l4_book_snapshot`) reads it. |
| `l4_books.json` | The levels the SDK's `L4OrderbookClient` holds after loading that reading and applying the update frames, and what it says on the gap. The SDK keeps no ALO queue position, so queue order within a level is checked against the documented `insertBefore` rule instead. |

`make_l4.py` rebuilds the last three from the update frames beside it.
