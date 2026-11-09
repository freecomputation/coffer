# Catalog sources

Where the cargo comes from. Researched 2026-11-09 against the
publishers' own pages. Sizes move at every rebuild: read the live
catalog entry before committing disk to anything large.

Umiak drinks from feeds and files. This document sorts every real
source by how it reaches the Shelf: feeds the catalog reads directly,
books and packs that drop in as files, bulk dumps that become
editions, and the vault tier of very large files that are not books
at all. A last section names the famous resources that are not
routes, so nobody builds on them by mistake.

## Feeds the catalog can read

The Kiwix Library - the stocking catalog, already wired in. OPDS v2
at library.kiwix.org/catalog/v2/entries, files and per-file torrents
at download.kiwix.org/zim/. Roughly 3,500 ZIMs: Wikipedia and the
Wikimedia family in all languages, Gutenberg, Stack Exchange,
DevDocs, LibreTexts and OpenStax, WikiMed and MedlinePlus, iFixit,
WikiHow, Appropedia, TED, Khan Academy, PhET. Most titles rebuild
monthly, and a new edition replaces the file; there are no
incremental ZIM updates. English Wikipedia runs about 100 to 120 GB
in the full maxi form and 48 to 60 GB without pictures, flagged as
figures to confirm on the live entry before a download.

Project Gutenberg - the gold-standard open OPDS feed, at
gutenberg.org/ebooks/search.opds/, no login, 70,000 or more
public-domain books as EPUB. Bulk fetching belongs on the mirrors,
not the main site, which is for human readers; the catalog database
is published so a mirror can serve its own OPDS.

Open Library - OPDS 2.0 (JSON) at openlibrary.org/opds/, the working
successor to the dead Internet Archive BookServer. Public-domain and
openly licensed titles download; in-copyright titles are lending
only. Our catalog reader speaks OPDS 1.x today, so this feed needs
the 2.0 reader before it plugs in.

Unglue.it - Creative Commons books freed by rights-holder campaigns,
OPDS at unglue.it/api/opds/, a few hundred to a few thousand titles,
each under its own CC terms.

SermonIndex - the one independent ZIM publisher with an OPDS feed
found in the sweep, at api.sermonindex.net/kiwix/library.xml,
sermons packaged as ZIM editions with sizes and checksums.

National library feeds - Gallica (France), Wolne Lektury (Poland),
and Aozora Bunko (Japan) all answer on OPDS with public-domain books
in their languages.

## Books and packs that drop in as files

The Kiwix file tree itself - every ZIM at download.kiwix.org/zim/
can also arrive as a plain file or a torrent, from any mirror or a
stick passed hand to hand, and the Shelf takes it the same way.
Wikimedia mirrors the same Wikipedia ZIMs from
dumps.wikimedia.org/other/kiwix/.

openZIM scrapers - the production line behind the catalog:
mwoffliner for any MediaWiki, sotoki for Stack Exchange, zimit for
any freely licensed website. Any open site not yet a book can be
made into one; WP1 builds custom Wikipedia selections of up to
50,000 articles (the file expires in about two weeks, so re-host at
once).

RACHEL and OER2Go modules - over a hundred teaching modules
(Wikipedia for Schools, Khan, CK-12, health, storybooks) published
as zipped websites at oer2go.org. Plain websites convert cleanly
with zimit, which makes these the most convertible packs outside
ZIM. Some modules are device-only at the owner's request; check
each one.

CD3WD - the frozen 2012 appropriate-technology library, some 4,000
to 5,800 practical manuals (agriculture, water, construction,
trades, health), preserved as DVD images on the Internet Archive
and re-hosted at cd3wd.org. Licences are mixed donor terms; audit
before commercial redistribution.

## Bulk dumps that become editions

Wikimedia dumps - every wiki, current text as
pages-articles-multistream XML, at dumps.wikimedia.org, monthly. A
full set is about 9 TB; the last five sets about 35 TB. Text is CC
BY-SA. Wikidata is the jewel here: the full entity graph as JSON,
CC0, weekly, about 130 GB compressed, with daily incrementals.

Gutenberg mirrors - the whole corpus, roughly 90 GB in all formats
and 15 GB text-only, by rsync from a mirror, with the catalog
database for building a local OPDS shelf.

DOAB - 70,000 or more peer-reviewed open-access books. The metadata
is CC0 and truly bulk (the KBART file lists only books with download
URLs); the books themselves are per-title PDF or EPUB under per-book
CC terms. OAPEN's 40,000 hosted titles ride the same metadata.

PubMed Central Open Access - millions of biomedical full texts in
tarballs sorted by licence tier (commercial-ok, non-commercial,
other), hundreds of GB, moving from NCBI FTP to an S3 layout;
confirm the current address at NCBI before scripting a harvest.

OpenAlex - 250 million works of scholarly metadata, CC0, as a free
quarterly snapshot of about 750 GB of compressed JSONL on S3. Daily
sync is a paid tier; the snapshot is the route.

Tatoeba - 13 million example sentences in 429 languages with
translations, weekly TSV exports at downloads.tatoeba.org/exports,
CC BY 2.0 FR with a CC0-only file for the clean slice. Kaikki.org
adds Wiktionary as structured JSONL (English 24.8 GB, weekly).

GeoNames - 13 million place features and 25 million names, CC BY
4.0, daily dumps; Natural Earth is public-domain cartography in
per-theme files.

SWORD modules - hundreds of Bible translations, commentaries, and
lexicons from CrossWire as raw zips. Every module states its
distribution licence; filter on it and the public-domain majority
drops out cleanly.

Software archives - Debian by rsync mirror or by dated snapshot at
snapshot.debian.org (any day since 2005, exactly reproducible);
FreeBSD the same way from its mirrors; GH Archive publishes the
public GitHub event timeline hourly as small gzipped JSON. PyPI can
be mirrored with bandersnatch but runs to many TB unfiltered, so
mirror an allowlist, never the whole.

## The vault tier

Protomaps basemap - the offline map layer: one PMTiles file of the
whole planet, about 120 GB, rebuilt daily at
maps.protomaps.com/builds, with regional extracts cut by bounding
box over range requests. ODbL; OSM attribution required.

OpenStreetMap planet - the raw data under the map: 89.1 GB of PBF
every Friday from planet.openstreetmap.org, with Geofabrik country
extracts refreshed daily for anything smaller.

Model weights - no bulk product exists; curate per repository from
Hugging Face as GGUF files for the local model service. Genuinely
open licences: Qwen and many Mistral releases (Apache-2.0), Phi and
recent DeepSeek (MIT). Llama and Gemma are community licences, not
open licences, and gated repos need an account acceptance first.

USPTO patents - weekly bulk files, public domain, about 2.1 TB for
2001 to 2023 grants alone. A vault edition, not a shelf book.

## Not routes

Wikimedia Commons media has no bulk download; the dumps carry text
and metadata, never the media files. Do not promise a Commons
mirror.

Open Library dumps are metadata only, about 12 to 30 GB monthly.
The full texts live behind lending control except the public-domain
slice reachable title by title.

arXiv full text is bulk-downloadable (requester-pays S3, Kaggle)
but the default arXiv licence grants distribution rights to arXiv
alone. Metadata is CC0; CC-flagged papers only for redistribution.

Stack Exchange stopped public dumps in July 2024; current dumps sit
behind a login and a click-through. The last Internet Archive dump
is the last public copy, and the sotoki ZIMs are the browsing route.

Standard Ebooks' OPDS answers 401 to non-patrons; the open route is
per-book download or the same texts through Gutenberg. The Internet
Archive BookServer, Feedbooks, ManyBooks, and OAPEN OPDS feeds are
dead; old lists still recommend all four.

Kolibri channels move only inside Kolibri (Studio, peer, or USB
export); there is no publisher file route. Internet-in-a-Box is an
installer that itself pulls Kiwix ZIMs, OER2Go modules, and OSM
maps; treat its pack lists as a shopping list, not a source. KA
Lite is discontinued. eGranary is a sold institutional product.

Hesperian health guides are free to read and download chapter by
chapter, and reposting is prohibited by the publisher. WHO IRIS has
no dump, only a per-item API. AnkiWeb decks are per-deck downloads
with usually unstated rights. MDN offers no offline dump; the
DevDocs ZIM is its practical offline form.

The CIA World Factbook was removed from cia.gov on 2026-02-04 with
no official archive; the frozen Kiwix ZIM and the CC0 factbook.json
project are the surviving bulk copies. data.gov is a catalog, not a
host: its bulk product is a monthly 2.3 GB metadata extract, with
the data itself at the agencies.

Software Heritage's graph dataset is open (CC BY, about 11 TB), but
file contents in bulk require a signed agreement and paid egress.
Common Crawl is free and enormous (about 90 TB per monthly crawl)
and its contents keep their own copyright; it is training material,
not library cargo.
