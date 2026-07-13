# Technical notes

This page collects the non-obvious technical problems we ran into while building
`ctddump` and how each was solved. It is written for a general audience — you do
not need to know Rust to follow it. The recurring themes are **memory use**,
**multi-threading**, **how data is read and written in chunks**, and a handful of
**quirks in the Polars data-frame library** we depend on.

Each note is laid out the same way: what the symptom looked like, why it
happened, and what we did about it.

---

## 1. Converting a file used far too much memory

**Symptom.** Converting a single large NetCDF file (a few hundred MB on disk)
pushed memory use to ~8–9 GB, enough to fail on modest machines.

**Why.** A NetCDF CTD file is stored as a dense rectangular grid: every *profile*
(a cast of the instrument) times every *depth level*. Most of that grid is empty
— a shallow cast still reserves a slot for every deep level other casts reach. If
you load the whole grid into memory at once and only then throw away the empty
cells, you have already paid for the full rectangle.

**Fix — stream the file in chunks.** Instead of loading everything, the converter
walks the file in slices along the profile dimension (about one million data rows
at a time). Each slice is turned into a small table and written straight to disk
as one "row group" of the output Parquet file, then discarded before the next
slice is read. The file is only ever partially in memory. On a 230 MB test file
this cut peak memory from ~8.8 GB to ~0.7 GB, and lower still with smaller
chunks.

The chunk size defaults to one million rows and can be tuned with the
`CTDDUMP_CHUNK_ROWS` environment variable — smaller means less memory but more
row groups. Importantly, the chunking only changes *how the output is laid out
on disk*; the actual data and its order are identical no matter what chunk size
you pick.

---

## 2. Empty rows were dropped the slow, memory-leaky way

**Symptom.** Running the batch converter over thousands of small files caused
memory to climb steadily and never come back down — roughly 0.2 MB lost per
file, with no upper limit. Converting 7,905 tiny files eventually consumed 88 GB.

**Why.** This is a bug in the version of the Polars library we use (0.43.1).
Several of Polars' *parallel* operations — filtering rows, gathering rows, and
the default parallel way of writing Parquet columns — leak a small amount of
memory on every call that is never released. On one file it is invisible; across
thousands of files in one long-running process it adds up without bound. The leak
is independent of the memory allocator and cannot be reclaimed manually.

**Fix — avoid the leaky parallel paths.** Two changes, both inside the
converters:

1. When dropping the empty (all-missing) rows, we do it on the plain raw arrays
   *before* handing the data to Polars, so Polars' leaky parallel filter is never
   called and the empty rows never enter Polars at all.
2. Every Parquet write in the converters (and in `concat`) turns off Polars'
   parallel column encoding.

Together these keep batch memory flat regardless of how many files are processed
(3,000 small files went from ~0.9 GB down to ~40 MB), and the output is verified
identical to the old parallel path. If a future Polars release fixes the leak,
these workarounds can be revisited.

---

## 3. Reading a big Parquet file in slices returned the wrong rows

**Symptom.** Commands that re-read a Parquet file in slices (`filter`, `dropqc`,
`dropna`, `markdup`, `dedup`) silently produced wrong results on large files —
for example, output that only contained platforms whose names started with the
first few letters of the alphabet, as if most of the file had vanished.

**Why.** Another bug in Polars 0.43.1. When a Parquet file has more than one row
group (which any file bigger than one chunk does), and you ask its *parallel*
reader for "the rows from position X onwards," it ignores X and keeps returning
rows from the very first row group. So every slice read the same opening chunk
over and over, and the rest of the file was never seen. It was invisible in our
automated tests because test fixtures are small enough to be a single row group.

**Fix — use the sequential reader for sliced reads.** The five affected commands
now read Parquet with Polars' non-parallel reader, which honours the slice
position correctly. Whole-file operations (like `report`) are unaffected and keep
the faster parallel reader. A regression test now writes a deliberately
many-row-group file and slices through it to guard against this coming back.

> This bug was also the root cause of a confusing field report where `dropqc`
> output looked truncated: the user was running an *older build* from before this
> fix. Rebuilding to the current version resolved it.

---

## 4. `markdup` crashed on large files with a "record batch" error

**Symptom.** `markdup` panicked on large merged files with
`RecordBatch requires all its arrays to have an equal number of rows`, and left a
truncated, unreadable output file. It worked fine on small files.

**Why.** `markdup` reads the file in slices, adds a new true/false `is_dup`
column to each slice, and writes it out. When a slice happened to span several
row groups of the input, the columns it read back were internally split into
several pieces ("chunks"), while the freshly-built `is_dup` column was a single
piece. The Parquet writer writes one batch per internal piece and insists every
column be split the same way — the mismatch made it crash. Small files have only
one row group, so their columns were never split and the bug never showed.

**Fix — line up the pieces before writing.** After adding the `is_dup` column,
`markdup` now re-aligns all columns to the same internal chunk layout before
writing each slice. The sibling commands did not need this because their
row-filtering step happens to realign the columns for free. A new test feeds
`markdup` a multi-row-group input to keep this covered.

---

## 5. Multi-threaded batch runs crashed with a stack overflow

**Symptom.** Batch conversion processed each file on a separate worker thread and
crashed on large files, even though converting the same file on its own worked.

**Why.** Worker threads get a much smaller memory "stack" by default (about 2 MB)
than the program's main thread (about 8 MB). Polars' Parquet writer uses deep
call chains on large files that need more stack than the 2 MB default, so it
overflowed only when run on a worker thread.

**Fix — give the workers a bigger stack.** Batch mode builds its thread pool with
a 16 MB stack per worker, and the program also raises the stack size for Polars'
own internal threads. Single-file conversion never hit this because it runs on
the roomier main thread.

---

## 6. Asking for N threads actually spawned far more

**Symptom.** Requesting, say, 10 threads created many more than 10 busy threads,
oversubscribing the machine.

**Why.** Polars keeps its *own* thread pool, sized to the number of CPU cores. So
running our 10 file-workers, each of which calls into Polars, produced roughly
10 + (number of cores) threads all competing for the CPU.

**Fix — pin Polars to one internal thread.** Batch and concat set Polars to a
single internal thread before the first Polars call, so our own `--threads`
setting is the real, honest knob. We already parallelise across files (or across
data ranges in `concat`), so we do not need Polars to add a second layer of
threading on top.

---

## 7. A single large file stalled all the other threads

**Symptom.** In a multi-threaded batch run, watching a system monitor showed all
cores busy at first, then most of them going idle near the end while one core
finished a large file. It looked as though files were processed in fixed groups
that waited for the slowest member.

**Why.** The work scheduler treats every file as equally expensive — it has no
idea one file is much bigger than the others — and a single file cannot be split
across threads. If a big file happened to be picked up late, the other workers
would finish all the small files, find nothing left to do but that one big file
(which they cannot help with), and sit idle until it finished.

**Fix — start the biggest files first.** The batch converter now sorts files
largest-first before handing them out, and dispatches them one at a time so a
free worker always grabs the next individual file. Starting heavy files at the
very beginning lets the many small files fill in around them, so the run finishes
close to "total work ÷ threads" instead of being dragged out by a late big file.
The one unavoidable limit: if a *single* file is larger than everything else
combined, its own runtime still sets the floor — no scheduler can split one file
across threads. Starting it first is simply the best possible placement.

---

## 8. Merging many files without loading them all at once

**Challenge.** `concat` merges many Parquet files into one and re-numbers the
profiles consistently across the whole result. Doing that naively means holding
every input in memory at the same time.

**Fix — merge one platform at a time.** Re-numbering is grouped by platform
(each platform's profiles are numbered together), so `concat` processes one
contiguous block of platforms at a time and writes each block out as it goes. A
quick first pass reads only the tiny "platform" column of every file to plan the
blocks; the second pass re-reads only the files that contribute to each block.
Because a given platform's data always lands in the same block, profiles that
were split across input files are still merged correctly. Producing the blocks in
order gives a result identical to merging everything at once — only the on-disk
layout differs.

When more than one thread is allowed, whole platform blocks are independent units
of work, so `concat` renumbers several blocks at once, each writing to a
temporary file, and then stitches the temporaries together in order. The final
file is byte-for-byte identical to the single-threaded result.

---

## 9. Missing datasets should not break the pipeline

**Symptom / need.** One planned dataset (the global product for the Baltic Sea)
is not yet published by the data provider, but the workflow already refers to it.
We did not want the whole run to fail just because those files do not exist yet.

**Fix — treat "no matching files" as a no-op, not an error.** When a batch or
concat command is pointed at a pattern that matches nothing (or an empty/absent
folder), it now prints a short informational message and writes no output,
instead of stopping with an error. The helper scripts likewise skip any step
whose input file is missing, with a note. The result is that the pipeline runs
cleanly today and will automatically pick up the Baltic global data the moment it
becomes available, with no further changes.

---

## 10. Harmless HDF5 diagnostic messages

**Symptom.** On some systems, reading certain files prints scary-looking
`HDF5-DIAG` messages to the screen.

**Why.** These come from an older version of the underlying HDF5 library (1.10.x,
used on our continuous-integration runner) encountering a newer file attribute it
does not recognise. It is only a diagnostic — the data is read correctly.

**What to do.** Nothing. The messages are harmless and the output is unaffected.
Using a newer HDF5 library avoids them.

---

## A recurring lesson

Most of the hard bugs above share a shape: **they only appear at scale.** A file
big enough to have multiple row groups, or a run long enough for a small leak to
matter, or a workload uneven enough for scheduling to bite. Small test fixtures
pass happily while the real data fails. Wherever we fixed one of these, we also
added a test that deliberately recreates the "large" condition (many row groups,
tiny chunks, mixed sizes) so the fix stays fixed.
