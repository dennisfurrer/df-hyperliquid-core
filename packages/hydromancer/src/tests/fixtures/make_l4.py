"""Builds the L4 fixtures with code independent of the crate under test.

The snapshot frame is packed by Python's msgpack and compressed by zstandard (libzstd), in the
layout Hydromancer documents for `l4Book`. Hydromancer's own Python SDK then reads it back
(`unpack_l4_book_snapshot`), and that reading is the expected decode. The SDK's
`L4OrderbookClient` loads the reading and applies the update frames beside this script, and the
levels it holds after each block are the expected books.

Run from a virtualenv holding msgpack 1.2.3, zstandard 0.25.0 and hydromancer-sdk at
4371df886245419c92e45a7f8038c0ec85d0789c:

    python make_l4.py <this directory>
"""

import json
import struct
import sys
from pathlib import Path

import msgpack
import zstandard

from hydromancer_sdk._internal.l4_formats import unpack_l4_book_snapshot
from hydromancer_sdk.orderbook import L4OrderbookClient
from hydromancer_sdk.orderbook.book import format_value

here = Path(sys.argv[1])

SOURCE = (
    "packages/hydromancer/src/tests/fixtures/make_l4.py: msgpack 1.2.3 and zstandard 0.25.0 "
    "packed the frame; hydromancer-sdk 4371df8 (unpack_l4_book_snapshot, L4OrderbookClient) read it "
    "and applied the update frames"
)

ALICE = "0x742d35cc6634c0532925a3b844bc9e7595f7f2e2"
BOB = "0x010461c14e146ac35fe42271bdc1134ee31c703a"
CAROL = "0x8f3c55aa1122c0532925a3b844bc9e7595f7f9a1"

HEIGHT = 782007303
TIMESTAMP_MS = 1704067199930

# [coin, bids, asks]; an order is [user, px, sz, oid], px and sz decimal strings.
ETH = [
    "ETH",
    [
        [ALICE, "3245.00", "3.25", 217148811800],
        [BOB, "3244.50", "0.4", 217148811750],
        [CAROL, "3244.50", "1.1", 217148811751],
    ],
    [
        [BOB, "3246.00", "2.0", 217148811900],
        [ALICE, "3246.00", "0.75", 217148811901],
        [CAROL, "3247.5", "12", 217148811910],
    ],
]
BTC = [
    "BTC",
    [[ALICE, "108708.4", "0.01234", 217148812001]],
    [[BOB, "108712", "0.5", 217148812002]],
]

# One blob says its decompressed size and carries a checksum, the other does neither: both are
# legal zstd frames.
with_size = zstandard.ZstdCompressor(level=3, write_content_size=True, write_checksum=True)
without_size = zstandard.ZstdCompressor(level=19, write_content_size=False, write_checksum=False)
blobs = [
    with_size.compress(msgpack.packb(ETH, use_bin_type=True)),
    without_size.compress(msgpack.packb(BTC, use_bin_type=True)),
]
frame = struct.pack("<IQQ", len(blobs), HEIGHT, TIMESTAMP_MS)
for blob in blobs:
    frame += struct.pack("<I", len(blob)) + blob
(here / "l4_snapshot.bin").write_bytes(frame)

read = unpack_l4_book_snapshot(frame)
(here / "l4_snapshot.json").write_text(json.dumps({"source": SOURCE, **read}, indent=1) + "\n")

client = L4OrderbookClient(coins=["ETH", "BTC"], api_key="offline")
client.load_snapshot(read)


def levels(book):
    """Each side from the touch outward: [px, total size, order count], as the SDK formats them."""
    bids = [[format_value(-key), format_value(level.total_size()), len(level)] for key, level in book.bids.items()]
    asks = [[format_value(key), format_value(level.total_size()), len(level)] for key, level in book.asks.items()]
    return {"bids": bids, "asks": asks}


books = {str(HEIGHT): {coin: levels(book) for coin, book in client.books.items()}}
for name in ["l4_update_782007304.json", "l4_update_782007305.json"]:
    update = json.loads((here / name).read_text())["data"]
    client.apply_update(update)
    books[str(update["height"])] = {coin: levels(book) for coin, book in client.books.items()}

gap = json.loads((here / "l4_update_782007307.json").read_text())["data"]
try:
    client.apply_update(gap)
    refused = None
except Exception as error:  # the SDK raises on a height gap
    refused = str(error)

out = {"source": SOURCE, "gap": refused, "books": books}
(here / "l4_books.json").write_text(json.dumps(out, indent=1) + "\n")
