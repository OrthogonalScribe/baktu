# Repositories

This page provides an overview of version 1 `baktu` repositories, both on the conceptual level, as well as their on-disk representation. It also provides details on some of the design choices and file formats used.

The following sections describe the current format. For a more historical overview and influences, see [Design Evolution](#design-evolution).


## Structure

### Repository

A `baktu` repository is a set of sites, each containing a sequence of snapshots. This is represented as a self-contained directory that contains:
* a `BAKTU_REPO.TAG` file, identifying the format and version of the directory
* a `sites` directory


### Sites

Conceptually, a site is a place in a `baktu` repository where snapshots of a particular *source dataset* (a set of files and directories to be snapshotted) are contained. For example, `desktop` and `laptop` sites to back up a user's partially synchronized home directories on two devices in a single repository in order to share storage for the files that are duplicated. A site is usually not a self-contained entity, as it may refer to data in other sites in the repository for the purposes of file deduplication.

In practice, this is represented as a directory within the `sites` directory of a repository, containing the following:
* `include-paths.nsv` - a [Null-Separated Values](#null-separated-values-format) file listing all the paths to be included in the next snapshot made for this site
* `exclude-paths.nsv` - same as the above, but for paths to be excluded
* `config.toml` - site-specific configuration, currently several flags for predicate-based path exclusion
* `snaps` directory, containing the sequence of snapshots. Those are named `0`, `1` and so on


### Snapshots

A snapshot contains a representation of the source dataset that aims to be as close as possible to a lossless direct copy of the paths to be included. As snapshots are incremental, they are even less self-contained than a site - all snapshots in a site but the initial one will extensively refer to their predecessors.

On disk, the snapshot is a directory that contains:
* `meta_name.cfg.bin` - a file that contains the name to be used for the [Binary Record-Jar](#binary-record-jar-format) metadata files within this snapshot. This is the approach chosen to handle cases where the default `.baktu.meta.brj` name is already used by some other path in the source dataset
* `data`, the directory that contains a representation of the source dataset


### Files

Each unexcluded path in the source dataset is recreated as closely as possible within the snapshot data directory, with some caveats:
* metadata is only recorded in the `baktu` metadata files, and not set on the files in the snapshot data directory. This may change in the future, but is not a priority at the moment due to:
    * the highest priority being having the data and metadata recorded in a format that would be sufficiently easy to access, everything else becoming effectively ease-of-use enhancements for [access without `baktu`](access-with-unix-tools.md)
    * recreating some properties requires elevated privileges - something that `baktu` strives to minimize
    * some properties are not pragmatically recreatable or might cause filesystem issues, for example a file's inode number
* files are:
    * copied as is, if they are unique within the repository or too small for there to be storage reduction benefits from their deduplication
    * deduplicated otherwise, by being represented as
        * a relative symlink to the backing data file in the repository. This backing file is the version from the previous snapshot (if the data is identical), or the first instance of the data encountered by `baktu snap` during the creation of any of the snapshots within the repository.
        * an `is-deduplicated` tag within the file's metadata record
            * the existence of this tag [SHOULD] be verified by clients before assuming a relative symlink is a deduplicated file, as it is the simplest differentiator between a deduplicated file and an appropriately crafted symlink in the source dataset
* unchanged files, directories and their metadata are pruned in intermediate snapshots. See [History intervals](#history-intervals)


## Concepts

### History intervals

To reduce inode costs per snapshot when most files in a source dataset are unchanged for a long time (assumed to be the common case), `baktu` represents a file's history within a snapshot as a series of *history intervals*, making the costs proportional to the number of changes, instead of number of snapshots. If a file is completely unchanged (including metadata) in an interval of snapshots \\( \[S_i, S_j\] \\), at the data layer it will only be present on-disk in the \\(S_i\\) and \\(S_j\\) snapshots. On the metadata layer, its full metadata will be present in \\(S_i\\), and a minimal reference will be present in every snapshot in \\( \[S_{i+1}, S_j\] \\).

[we need to escape the opening and closing brackets we use to denote intervals, as the link checker doesn't understand the MathJax context and thinks these are links without targets]::

The full in-repository representation of a history interval \\( \[S_i, S_j\] \\) in a particular file's history is thus:
* \\( S_i \\) contains the normal file representation, including a full metadata dump
    * this may also be a deduplicated file representation, in the case of either metadata-only changes to the file from the previous snapshot, or regular data duplication within the repository
* each snapshot in \\( \[S_{i+1}, S_j\] \\) contains a minimal metadata record for the path, composed of only two lines: the path's basename, and a `same-since i` key-value pair
* \\(S_j\\) also contains an *end-marker*, a symlink to the file in  \\( S_i \\). This is not strictly necessary, but was chosen to drastically lower the barrier to [manual exploration](access-with-unix-tools.md) of the latest snapshots in a repository in situations where the tool may not be available, such as emergency data recovery or future [data archeology](https://en.wikipedia.org/wiki/Data_archaeology)
    * the presence of a `same-since i` line in the metadata record for the file [SHOULD] be verified by repository clients, as it is the simplest differentiator between an end-marker and an appropriately crafted symlink in the source dataset

This representation and pruning also applies to directories and special files, although unlike regular files, they are not deduplicated.


### Binary record-jar format

`baktu` metadata files use the Binary Record-Jar format (BRJ), which is based on the *record-jar* format, itself an extension of the *cookie-jar* one. The record-jar format is described in [The Art of Unix Programming](http://www.catb.org/~esr/writings/taoup/html/ch05s02.html#id2906931) and referenced in [RFC5646](https://datatracker.ietf.org/doc/html/rfc5646#section-3.1.1).


BRJ does not guarantee being ASCII-only, nor Unicode, instead prioritizing minimal conversion of the stored payloads for the purposes of easier manual searching in the repository and easier from-scratch parser writing. A BRJ file is a sequence of records separated by the bytes `\n--\n`. Records are a sequence of lines separated by the newline byte `\n`. Each record line itself is one of:
* a tag - a line that contains no spaces
* a key-value pair, separated by the first space byte in the line

At the next layer of parsing, `baktu` uses [Tagged Raw/Hex Encoding](#tagged-rawhex-encoding) for payloads whose allowed values might introduce ambiguity:
* the path's basename, under key `name`. In ext4 filesystems, that can be a sequence of any bytes except `\0` and `/`[^ext4-allowed], hence any basename containing a newline gets hex-encoded
* extended attribute keys and values, under key `x`. Each key-value pair is on a single line in the `x␣k.<rhenc(key, hex_on_space=true)>␣v.<rhenc(val)>` pattern, with `␣` representing space, 0x20. Due to the choice of putting the pair on a single line, extended attribute keys containing spaces are also hex-encoded.


To visualize this better, the following is the somewhat redacted content of the root `.baktu.meta.brj` file from the [Quick Start example](../../quick-start.md#creating-the-initial-snapshot):

```
name r-13 important.txt
b3sum a822a88d59fd350ad50ccc38ecbc2ae5d2ec239e1c114e7de6c659f7c8555dd6
blksize 4096
attributes
nlink 1
uid 1000
gid 1000
mode 644
type reg
ino XXXXXX
size 27
blocks 8
atime 17XXXXXXXX.XXXXXXXXX
btime 17XXXXXXXX.XXXXXXXXX
ctime 17XXXXXXXX.XXXXXXXXX
mtime 17XXXXXXXX.XXXXXXXXX
dev_major 254
dev_minor 1
mnt_id 28
dio_mem_align 512
dio_offset_align 512
lsattr e
x k.r-12 user.enc-alg v.r-5 rot-N
x k.r-14 user.enc-arg v.r-2 26
--
name r-7 project
blksize 4096
attributes
nlink 5
uid 1000
gid 1000
mode 755
type dir
ino YYYYYY
size 4096
blocks 8
atime 17YYYYYYYY.YYYYYYYYY
btime 17YYYYYYYY.YYYYYYYYY
ctime 17YYYYYYYY.YYYYYYYYY
mtime 17YYYYYYYY.YYYYYYYYY
dev_major 254
dev_minor 1
mnt_id 28
lsattr e
--
```

Future BRJ parser implementations might benefit from previous work on the record-jar format:

* [Python parsing code for *record-jar*](https://filebox.ece.vt.edu/~ece2524/reading/record_jar/index.html)
* [Open-RJ: C & C++ library with a C API and mappings to D, .NET, Python, Ruby](https://openrj.sourceforge.net/)


#### Tagged Raw/Hex encoding

This encoding is a simple format that represents binary data in one of two tagged variants:
* raw: the data as-is, prefixed by `r-<SIZE_IN_BYTES>␣` (with `␣` representing space, 0x20)
* hex: lowercase-hex encoded, prefixed by `h␣`
3
Design notes:
* It aims to be simple to parse and is used instead of a more conventional escaping scheme to reduce the risk of false negative searches due to interactions between escaping methods in the shell, search tool and searched format.
* It includes the payload size in the raw variant to prevent confusion for users manually accessing data such as the raw encoding of the [*security.capability* extended attribute](https://www.mankier.com/7/capabilities#Description-File_capabilities) that can contain non-printable characters.


### Null-Separated Values format

A format containing entries separated by the NULL byte `\0`. Useful as an unambiguous format for payloads that must not contain `\0`, such as ext4 paths. Also used by `find -print0` and `xargs`'s `-0`/`--null`.


## Design evolution

`baktu` repositories can be seen as an evolution (with a peculiar set of evolutionary pressures) of [`rsnapshot`](https://rsnapshot.org/) ones (based on the article [*Easy* Automated Snapshot-Style Backups with Linux and Rsync](http://www.mikerubel.org/computers/rsync_snapshots/)), themselves an evolution of the simple `rsync` backup copy.

The journey to reaching the current design can be summarized as follows:
* a manual re-implementation of the core of the `rsnapshot` scheme, stripping off the rotation and deletion functionality to provide a more "infinite retention" mode of operation
* introduction of [sidecar metadata recording](#binary-record-jar-format) to allow for retention of metadata that was previously not collected (`btime`), can not be recreated, or is lost during `rsync`'s hard-link deduplication. As a bonus, this protects against metadata loss in case of backup transmission through a lossy medium (non-ext4 filesystems, various archive formats, file synchronization programs)
* addition of cryptographic hash recording in the metadata file to enable efficient different-path deduplication. This also gives us data to enable integrity checking and [CAS](https://en.wikipedia.org/wiki/Content-addressable_storage) for free
    * [BLAKE2](https://en.wikipedia.org/wiki/BLAKE_(hash_function)#BLAKE2) initially chosen as a faster and more secure alternative to SHA-2, and due to availability in coreutils as `b2sum`
    * switch to [BLAKE3](https://en.wikipedia.org/wiki/BLAKE_(hash_function)#BLAKE3) due to further speed increases and reduction in the configuration space and overall algorithm complexity, which ought to simplify future re-implementation and reduce the risk of bugs during use and implementation
* switch to different-path deduplication to properly support large move/rename operations between snapshots in an infinite retention context
* switch to soft-link representation of duplicate [files](#files) to encode an explicit reference to the backing file's path (and thus snapshot) and speed up repository reading both by `baktu` and via [standard unix tools](access-with-unix-tools.md). As a bonus, this makes the deduplicated representation of files and directories more uniform
* introduction of [history intervals](#history-intervals) to reduce inode costs per snapshot
* introduction of [sites](#sites) as intermediate layer between snapshots and repository to enable deduplication over different source datasets
* switch to `same-since i` minimal metadata records for files' history intervals to simplify snapshot creation, as well as manual directory listing, at the cost of a slight disk space increase per snapshot


[^ext4-allowed]: see "Allowed filename characters" in <https://en.wikipedia.org/wiki/Ext4>

[SHOULD]: https://datatracker.ietf.org/doc/html/rfc2119#section-3