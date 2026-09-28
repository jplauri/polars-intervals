"""Read JAIST's endpoint-order catalogs without graph-library dependencies.

The per-order files are plain endpoint lines. The combined LIST downloads use
``size : N`` headers and ``# of size N : COUNT`` footers. On the source page,
``interval_disconnected_N.txt`` means *all* graphs, including connected graphs.
"""

import gzip
import hashlib
import http.client
import io
import re
import shutil
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Iterator
from html.parser import HTMLParser
from pathlib import Path

PAGE_URL = "https://www.jaist.ac.jp/~uehara/graphs/"
SOURCE_URL = PAGE_URL + "#interval"
_HEADER = re.compile(r"size\s*:\s*([1-9][0-9]*)")
_FOOTER = re.compile(r"# of size ([1-9][0-9]*)\s*:\s*([0-9]+)")
_FILENAME = re.compile(r"interval_(disconnected|connected)_([1-9][0-9]*)\.txt")
_COMBINED_FILENAME = re.compile(r"interval-[1-9][0-9]*-[1-9][0-9]*-(disconnected|connected)\.txt")


def parse_endpoint_sequence(
    line: str, expected_n: int | None = None
) -> tuple[list[int], list[int]]:
    """Convert endpoint positions p, q to [p, q + 1), sorted by vertex label.

    Labels are positive integers occurring exactly twice. Endpoint positions are
    distinct, so the conversion preserves overlap while permitting touching
    half-open intervals between consecutive, nonoverlapping endpoint pairs.
    """
    if expected_n is not None and (type(expected_n) is not int or expected_n < 1):
        raise ValueError("expected_n must be a positive integer")
    tokens = line.split()
    if not tokens or len(tokens) % 2:
        raise ValueError("an endpoint sequence must contain a positive even number of tokens")
    positions: dict[int, list[int]] = {}
    for position, token in enumerate(tokens):
        if not token.isascii() or not token.isdecimal() or int(token) < 1:
            raise ValueError(f"invalid positive integer vertex label: {token!r}")
        label = int(token)
        occurrences = positions.setdefault(label, [])
        occurrences.append(position)
        if len(occurrences) > 2:
            raise ValueError(f"vertex label {label} occurs more than twice")
    if any(len(occurrences) != 2 for occurrences in positions.values()):
        raise ValueError("every vertex label must occur exactly twice")
    if expected_n is not None and len(positions) != expected_n:
        raise ValueError(f"expected {expected_n} vertices, found {len(positions)}")
    ordered = [positions[label] for label in sorted(positions)]
    return [pair[0] for pair in ordered], [pair[1] + 1 for pair in ordered]


def iter_catalog(
    path: Path | str,
    *,
    expected_n: int | None = None,
    catalog_kind: str = "all",
    source_url: str | None = None,
    expected_count: int | None = None,
) -> Iterator[tuple[list[int], list[int], dict[str, object]]]:
    """Yield one graph at a time, validating section counts at exhaustion.

    A streaming SHA-256 prepass identifies the original source bytes, including
    compression when supplied. No complete source or graph collection is kept.
    ``catalog_index`` is zero-based and sequential across the entire source,
    including all sections of a combined LIST file. ``expected_n`` validates
    every record; omit it for a combined file containing several orders.
    """
    path = Path(path)
    if catalog_kind not in {"all", "connected"}:
        raise ValueError("catalog_kind must be 'all' or 'connected'")
    if expected_n is not None and (type(expected_n) is not int or expected_n < 1):
        raise ValueError("expected_n must be a positive integer")
    if expected_count is not None and (type(expected_count) is not int or expected_count < 0):
        raise ValueError("expected_count must be a nonnegative integer")
    filename = path.name.removesuffix(".gz")
    known_filename = _FILENAME.fullmatch(filename)
    filename_match = known_filename or _COMBINED_FILENAME.fullmatch(filename)
    if filename_match:
        filename_kind = "all" if filename_match[1] == "disconnected" else "connected"
        if filename_kind != catalog_kind:
            raise ValueError(f"{path.name} belongs to the {filename_kind!r} catalog")
    if known_filename:
        filename_n = int(known_filename[2])
        if expected_n is not None and expected_n != filename_n:
            raise ValueError(f"filename order {filename_n} differs from expected_n={expected_n}")
        expected_n = filename_n
    with path.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    provenance: dict[str, object] = {
        "source": "jaist_interval_catalog",
        "source_url": SOURCE_URL,
        "source_file": path.name,
        "source_sha256": digest,
        "catalog_kind": catalog_kind,
    }
    if source_url is not None:
        provenance["catalog_url"] = source_url
    count = 0
    section_n: int | None = None
    section_count = 0
    previous_n = 0
    plain_n = expected_n
    sectioned: bool | None = None
    with path.open("rb") as raw:
        compressed = raw.read(2) == b"\x1f\x8b"
        raw.seek(0)
        binary = gzip.GzipFile(fileobj=raw) if compressed else raw
        with io.TextIOWrapper(binary, encoding="ascii") as lines:
            for line_number, raw_line in enumerate(lines, 1):
                line = raw_line.strip()
                if not line:
                    continue
                header = _HEADER.fullmatch(line)
                footer = _FOOTER.fullmatch(line)
                try:
                    if sectioned is None:
                        sectioned = header is not None
                    if header:
                        if not sectioned or section_n is not None:
                            raise ValueError("unexpected size header or missing count footer")
                        section_n = int(header[1])
                        if section_n <= previous_n:
                            raise ValueError("catalog sections must have increasing vertex counts")
                        if expected_n is not None and section_n != expected_n:
                            raise ValueError(f"expected order {expected_n}, found {section_n}")
                        section_count = 0
                        continue
                    if footer:
                        if not sectioned or section_n is None or int(footer[1]) != section_n:
                            raise ValueError("count footer has no matching size header")
                        if int(footer[2]) != section_count:
                            raise ValueError(
                                f"declared count {footer[2]} differs from imported {section_count}"
                            )
                        previous_n, section_n = section_n, None
                        continue
                    if sectioned and section_n is None:
                        raise ValueError("endpoint sequence outside a size section")
                    starts, ends = parse_endpoint_sequence(
                        line, expected_n=section_n if sectioned else plain_n
                    )
                    if not sectioned and plain_n is None:
                        plain_n = len(starts)
                except ValueError as error:
                    raise ValueError(f"{path.name}:{line_number}: {error}") from error
                yield (
                    starts,
                    ends,
                    {
                        **provenance,
                        "catalog_n": len(starts),
                        "catalog_index": count,
                    },
                )
                count += 1
                section_count += 1
    if section_n is not None:
        raise ValueError(f"{path.name}: missing count footer for order {section_n}")
    if not count:
        raise ValueError(f"{path.name}: catalog contains no graphs")
    if expected_count is not None and count != expected_count:
        raise ValueError(f"{path.name}: expected {expected_count} graphs, imported {count}")


class _CatalogLinks(HTMLParser):
    """Extract only the observed per-order catalog links and their count labels."""

    def __init__(self, base_url: str) -> None:
        super().__init__()
        self.base_url = base_url
        self.links: dict[tuple[str, int], dict[str, object]] = {}
        self.href: str | None = None
        self.label: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "a":
            self.href = dict(attrs).get("href")
            self.label = []

    def handle_data(self, data: str) -> None:
        if self.href is not None:
            self.label.append(data)

    def handle_endtag(self, tag: str) -> None:
        if tag != "a" or self.href is None:
            return
        url = urllib.parse.urljoin(self.base_url, self.href)
        parsed = urllib.parse.urlsplit(url)
        match = _FILENAME.fullmatch(parsed.path.rsplit("/", 1)[-1])
        if match and parsed.netloc == urllib.parse.urlsplit(self.base_url).netloc:
            kind = "all" if match[1] == "disconnected" else "connected"
            n = int(match[2])
            label = "".join(self.label).strip()
            if not label.isascii() or not label.isdecimal():
                raise ValueError(f"catalog page has an unrecognized graph count for order {n}")
            entry: dict[str, object] = {"n": n, "url": url, "count": int(label), "kind": kind}
            key = kind, n
            if key in self.links and self.links[key] != entry:
                raise ValueError(f"catalog page has conflicting links for {kind} order {n}")
            self.links[key] = entry
        self.href = None
        self.label = []


def parse_catalog_links(
    html: str, base_url: str = PAGE_URL
) -> dict[tuple[str, int], dict[str, object]]:
    """Discover the current page's links; fail on changed naming/count formats."""
    parser = _CatalogLinks(base_url)
    parser.feed(html)
    parser.close()
    if not parser.links:
        raise ValueError("JAIST page contains no recognized interval catalog links")
    return parser.links


def download_catalogs(
    orders: list[int], cache: Path | str, catalog_kind: str = "all"
) -> list[dict[str, object]]:
    """Explicitly download selected per-order catalogs, preserving source files.

    Metadata entries contain ``path``, ``n``, ``url``, ``count``, and ``kind``.
    Existing cache files are reused and should be passed to ``iter_catalog``
    with the page's expected count. Orders are discovered, never capped or
    fabricated from a guessed URL. Downloads use a sibling partial file so a
    failed transfer cannot become a reusable cache entry.
    """
    if catalog_kind not in {"all", "connected"}:
        raise ValueError("catalog_kind must be 'all' or 'connected'")
    if not orders or any(type(n) is not int or n < 1 for n in orders):
        raise ValueError("download orders must be positive integers")
    try:
        with urllib.request.urlopen(PAGE_URL, timeout=60) as response:
            links = parse_catalog_links(response.read().decode("utf-8"))
    except (urllib.error.URLError, http.client.HTTPException, OSError, UnicodeError) as error:
        raise ValueError(f"could not read the JAIST catalog page: {error}") from error
    selected = sorted(set(orders))
    missing = [n for n in selected if (catalog_kind, n) not in links]
    if missing:
        available = sorted(n for kind, n in links if kind == catalog_kind)
        raise ValueError(
            f"JAIST has no recognized {catalog_kind} links for orders {missing}; "
            f"published orders: {available}"
        )
    cache = Path(cache)
    cache.mkdir(parents=True, exist_ok=True)
    sources = []
    for n in selected:
        entry = links[catalog_kind, n]
        url = str(entry["url"])
        destination = cache / urllib.parse.urlsplit(url).path.rsplit("/", 1)[-1]
        if not destination.exists():
            partial = destination.with_suffix(destination.suffix + ".part")
            try:
                with urllib.request.urlopen(url, timeout=60) as response, partial.open("wb") as out:
                    shutil.copyfileobj(response, out, length=1024 * 1024)
                partial.replace(destination)
            except (urllib.error.URLError, http.client.HTTPException, OSError) as error:
                partial.unlink(missing_ok=True)
                raise ValueError(f"could not download {url}: {error}") from error
        sources.append({**entry, "path": destination})
    return sources
