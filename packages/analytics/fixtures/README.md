# Fixtures

Where each file came from. Every one is a recorded answer or a provider's documented example,
copied unchanged from the crate or repository named; none holds a key or a token.

| File | Source |
|---|---|
| `dashboard_volume_daily.json` | Hydromancer's documented example of `/dashboard/outcome-builder-volume-daily` (packed), with a real builder and the `null` bucket, copied from hl-hydromancer's `src/tests/fixtures`. |
| `referral_builder.json` | Live mainnet `referral` for hip4-tui's builder `0x6967..a843`, 2026-10-08, copied from hl-account's `fixtures`. |
| `perp_dexs_mainnet.json`, `all_perp_metas_mainnet.json` | Live mainnet `perpDexs` and `allPerpMetas`, 2026-09-27, recorded by hip4-tui (`net/src/tests/fixtures`) and copied from hl-hip3's `src/tests/fixtures`. |
| `meta_and_asset_ctxs_para_mainnet.json` | Live mainnet `metaAndAssetCtxs` for `para`, 2026-09-26, recorded by hip4-tui (`tui/src/app/tests/fixtures`) and copied from hl-hip3's `src/tests/fixtures`. |
| `builder_fills_*_20260926.csv.lz4` | Two builders' files for 2026-09-26 from Hyperliquid's `stats-data` bucket, stored as sent, recorded by hip4-tui and copied from hl-hydromancer's `src/tests/fixtures`, whose sums the tests take. |
