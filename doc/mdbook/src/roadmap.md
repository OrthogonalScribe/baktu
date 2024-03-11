# Roadmap

The letters in front of some of the items are the [MoSCoW](https://en.wikipedia.org/wiki/MoSCoW_method) prioritization categories.


## MVP functionality

Issues that need to be handled for this to be a [Minimum Viable Product](https://en.wikipedia.org/wiki/Minimum_viable_product) *for the author's own use*:

* [ ] `M` appropriate handling of multiple include paths with the same basename
* [ ] `M` appropriate handling of file access time changes due to `baktu` activity. Related issues in [restic](https://github.com/restic/restic/issues/53), [borg](https://github.com/borgbackup/borg/issues/4673). Strongly consider `O_NOATIME`, but also changing how (and if) we record and use the `atime` metadata, and interactions with `noatime`/`relatime` or even `atime` mounts
* [ ] `M` code necessary to validate initial snapshot:
    * either FUSE mount + internal baktu metadata getters for where our Rust FUSE stack doesn't help, or fully internal baktu FS functions. Former option preferable if not too much overhead
    * directory tree diff program, existing one if it can handle the excludes, includes and full set of metadata we record *and* somehow interop with repo reading approach, or otherwise our own
* [ ] `M` subsequent snapshot generation
    * correct pruning
    * bring up rest of code to be able to validate the snapshot against the source dataset
* [ ] `M` acceptable method of accessing snapshots
* [ ] `S` integrity checking subcommand


## Happy path functionality

Issues that need to be handled for acceptable usability in the [happy path](https://en.wikipedia.org/wiki/Happy_path):

* [ ] `M` proper computation of `meta_name.cfg.bin` contents - currently we hardcode and error out on collision during `baktu snap`
* [ ] `M` FUSE mount
    * good high- and low-level API overview in [FSL FUSE appendices]
* [ ] `M` acceptable method of accessing those parts of the metadata that are not available via the used FUSE stack
    * possibly `LD_PRELOAD` shared object


## Build issues

* `S` (FUSE-only) old nom and cexpr versions cause deprecation warnings
    * caused by the `carlosgaldino/fuse-rs` dependency. Shortened from the git log: the `libfuse-sys`<-`bindgen`<-`cexpr`<-`nom^4` dep is too old, hitting the [trailing semicolon in macro used in expression position](https://github.com/rust-lang/rust/issues/79813) issue. Right now this emits warnings, but will become a hard error in the future. This is fixed in nom 5.1.3+ and 6.2.2+ by [rust-bakery/nom#1657](https://github.com/rust-bakery/nom/pull/1657).


## Usability issues

* `C` consider implementing `baktu unmount` as a QoL wrapper over `fusermount -u`
* `S` consider memory usage and speed of current in-memory hashmap deduplication approach: estimate scalability in current use-case and plan migration before RAM usage becomes an issue
    * bloom or cuckoo filters may be useful to speed up other approaches
* `S` improve payload representation on `nsv-rm-from` not found error
* `C` caching where needed to provide sufficiently good responsiveness


## Documentation issues

* `S` create integration test for [Quick Start](quick-start.md) scenario
    * `C` attempt to use the documentation itself as a test case
        * possibly with `trycmd` if [issue #105](https://github.com/assert-rs/trycmd/issues/105)
            ([reddit](https://reddit.com/r/rust/comments/xvlo5w/trycmd_just_ignores_my_tests/))
            isn't a problem
        * [term-transcript](https://docs.rs/term-transcript/0.3.0/term_transcript/test/index.html)
        is another candidate
* `S` host code and documentation on baktu.net and mirror on one or more of the mainstream platforms
    * add appropriate GH topics to project
* `S` consider using [cargo-msrv](https://github.com/foresterre/cargo-msrv)
* `C` ensure that internal links to headings are checked


## Stretch goals

Concerns and ideas that can be handled at a later time:

* `S` tools to allow simple file and directory removal from the backup repository
* `S` acceptable [sparse file](https://en.wikipedia.org/wiki/Sparse_file) handling
* `S` reduce unnecessary disk writes during [intermediate snapshot pruning](repositories/v1/index.md#history-intervals). When a prune is needed, we should be able to move the intermediate snapshot's end-marker to the currently created one. This reduces disk writes (useful on e.g. [NAND flash](https://en.wikipedia.org/wiki/Flash_memory#Memory_wear)) at the cost of mixing snapshot creation and intermediate snapshot pruning
    * `C` further optimize disk write patterns for the common situation where entire directories are unchanged (e.g. a post-order traversal algorithm that creates a directory only when any of its children have changed, which can also skip sorting children otherwise)
* `S` acceptably handle situations where snapshot creation is interrupted - ideally have ability to resume, but otherwise a way for the user to undo any incomplete changes
* `S` extensive refactoring or from-scratch rewrite (if this was not already done at the MVP stage) to handle the technical debt that was introduced due to the PoC nature of the codebase
* `S` extend integrity data from file-level hashes of regular files only, to something similar to a [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree) covering both data and metadata
* `C` save an `excluded.nsv` log file in each snapshot's metadata, can be used to warn the user when the effective excluded path set changes
    * consider logging more (ideally all) of the site configuration during snapshot creation
* `S` provide sample shell scripts to automate the more difficult tasks in [Data Access with Standard Unix Tools](repositories/v1/access-with-unix-tools.md)
* `S` [Redundancy data](https://en.wikipedia.org/wiki/Error_correction_code), likely a `baktu`-esque [systematic erasure code](https://en.wikipedia.org/wiki/Systematic_code) overlay in the repository (without otherwise modifying it or `baktu`), borrowing some ideas from the [PAR3 specification draft](https://parchive.github.io/doc/Parity_Volume_Set_Specification_v3.0.html)
* `S` equality and diff tools at the dir, snapshot, site and repo levels (including against a source dataset) to enable more fine-grained manual verification of backup integrity, and other workflows (pre-snapshot preview, staging, summaries)
* `C` repository cloning and pulling subcommands
* `S` mount entire site or even repository via FUSE
    * needs investigation into possible issues due to inter-snapshot inode number collisions - we can reuse the underlying `ino` in the first and last snapshot of a history interval, but need to come up with something robust for the intermediate ones
        * might be a non-issue, considering each instance of a file in a history interval is equivalent to a hard-link (same meta/data). Might even work for directories with most tools, since it shouldn't introduce loops
* `C` explore possible FUSE performance optimizations based on the [FAST'17 paper](https://www.usenix.org/conference/fast17/technical-sessions/presentation/vangoor) from [FSL], read splicing via `read_buf()` for starters
* `S` automated testing
    * [dir-diff](https://github.com/assert-rs/dir-diff), [snapbox](https://github.com/assert-rs/trycmd/tree/main/crates/snapbox) and [others](https://docs.rs/snapbox/latest/snapbox/#which-tool-is-right) might help with filesystem-related tests
    * ideally documentation code testing too, especially for all versions of [Data Access with Standard Unix Tools](repositories/v1/access-with-unix-tools.md)
* `C` investigate possible usability issues due to spurious `statx()` metadata changes (e.g. `mnt_id` or other filesystem-related changes that do not affect the source dataset)
* `C` consider interface to snapshotting utilities, c.f. [`rsnapshot` and LVM](https://www.mankier.com/1/rsnapshot#Configuration-linux_lvm_cmd_lvcreate)
* `C` make the backup as read-only as possible ([eponymous section in the Rubel article](http://www.mikerubel.org/computers/rsync_snapshots/#ReadOnly))
    * Merkle tree integrity checking should reduce the urgency of this, as we'll be able to at least detect corruption in non-adversarial situations; however it will still be useful for reducing the chance of data loss, via [defense in depth](https://en.wikipedia.org/wiki/Defense_in_depth_(computing)) - e.g. in addition to a 3-2-1 workflow
* `C` reduce required privileges ([PoLP](https://en.wikipedia.org/wiki/Principle_of_least_privilege))
    * possibly have helper tool with smaller TCB[^TCB] to check if any path has properties requiring elevated privileges. Con: possible [TOCTOU](https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use) issues. Might be able to reduce window by using streaming and error-on-admin-required, but that might result in snapshot inconsistency issues during non-dry `baktu snap` execution
* `C` better modularization of code
    * backing file and directory resolution
    * splitting up into crates and modules
        * will improve logging tags too
        * repo reading code in its own crate
    * better code locality for each subcommand, e.g. help, args, validation and execution code in a separate file for each one
    * generation of effective included path list, for better third party tool interop
        * possibly an option to operate on such a list as input too (c.f. [cpio](https://www.mankier.com/1/cpio)). Questionable usability if include/exclude logic is already sufficiently configurable
* `C` expose all metadata via FUSE
    * resolve limitations of the Rust FUSE stack
        * note that `listxattr`, `getxattr` and some kind of `ioctl` call are already available in the C API (see [FSL FUSE appendices]), so adding them to <https://github.com/carlosgaldino/fuse-rs> might not be too big of an effort
    * resolve limitations of `libfuse` and the kernel side code
        * check existing work, such as [Miklos Szeredi's Aug 2023 statx patch to fuse](https://lwn.net/Articles/941067/)
* `S` prune dependencies that are no longer needed
* `C` consider publishing to crates.io
* `C` consider further hardening:
    * of both the main executable, and the helper
    * only allow writes to the baktu repository, ideally even a subset of it
    * disallow networking
    * potentially restrict reads too
    * could also whitelist system calls
    * might be able to use some of SELinux, AppArmor, seccomp, namespaces (unshare, etc) for that
* `C` consider return to the original (more abstract) architecture
    * repository seen as a set of `(tag, location, interval, value)` tuples plus some metadata (sites, snapshots)
        * enables easier comparison (and thus also cloning, pulling) and migration to different on-disk representations
    * `pathSetGen` -> `deltaGen` -> `actionGen` -> `execution` snapshotting pipeline
        * with branches at `deltaGen` and `actionGen` for diff and snap-preview (a more flexible dry-run) functionality
        * deltas and actions can be further used to enable creation of flexible representations of differences between snapshots and source datasets, or any two supported filesystem forests. This can be used by downstream tools (integrated, or via FUSE exports and existing tools) to streamline FS change exploration and manipulation (c.f. [Unison](https://www.cis.upenn.edu/~bcpierce/unison/))
            * manipulation can be made more interactive by speeding up diff/action regeneration, both offline (complete re-scanning) and later online (server keeping state and tracking changes via [inotify](https://en.wikipedia.org/wiki/Inotify) and similar, or via client-guided partial re-scanning)


[^TCB]: [Trusted Code Base](https://www.google.com/search?q=%22trusted+code+base%22), not to be confused with [Trusted Computing Base](https://en.wikipedia.org/wiki/Trusted_computing_base)

[FSL]: https://www.fsl.cs.stonybrook.edu/all-pubs.html
[FSL FUSE appendices]: https://www.fsl.cs.stonybrook.edu/docs/fuse/fuse-article-appendices.html