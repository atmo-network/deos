#!/usr/bin/env python3
"""Generate the four-watch fixed hash-selected retirement history for native diagnostics."""

import argparse
import hashlib
import json
import multiprocessing
from pathlib import Path
import struct


PAGES = 9_968
WATCHES = 4
PREFIX_NIBBLES = 5
LOCAL_BASE = 5_000_000
TAIL_BASE = 10_000_000


def prefix(page: int, slot: int) -> int:
    digest = hashlib.blake2b(struct.pack("<QB", page, slot), digest_size=16).digest()
    return int.from_bytes(digest[:3], "big") >> 4


def source_pages(source: int) -> list[int]:
    local = LOCAL_BASE + source
    tail = TAIL_BASE + source
    wanted = {prefix(page, slot) for page in (local, tail) for slot in range(1, 32)}
    selected = [0]
    for start, end, count in (
        (1, local, PAGES // 2 - 1),
        (local + 1, tail, PAGES - PAGES // 2 - 2),
    ):
        chosen = []
        for page in range(start, end):
            if any(prefix(page, slot) in wanted for slot in range(1, 32)):
                chosen.append(page)
                if len(chosen) == count:
                    break
        if len(chosen) != count:
            raise RuntimeError(f"source {source}: fixed interval exhausted")
        selected.extend(chosen)
        print(f"source={source} interval={start}..{end} retained={count} last={chosen[-1]}", flush=True)
        if start == 1:
            selected.append(local)
    selected.append(tail)
    assert len(selected) == PAGES
    assert selected == sorted(set(selected))
    assert selected[PAGES // 2] == local
    return selected


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new JSON output path (existing files are refused)")
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists; validate/reuse it or select a new path")
    # Four deterministic, independent source calculations; result order is source order.
    with multiprocessing.Pool(4) as pool:
        selected = pool.map(source_pages, range(WATCHES))
    artifact = {
        "format": "deos.actor.targeted-retention-pages",
        "version": 1,
        "prefix_nibbles": PREFIX_NIBBLES,
        "source_pages": selected,
    }
    with args.output.open("x", encoding="utf-8") as output:
        json.dump(artifact, output, separators=(",", ":"))
        output.write("\n")
    print(f"output={args.output} pages={PAGES} watches={WATCHES}", flush=True)


if __name__ == "__main__":
    main()
