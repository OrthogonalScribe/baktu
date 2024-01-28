# Data Access with Standard Unix Tools

This page describes how we can go about accessing data stored in a repository if the `baktu` tool itself is unavailable. This can be useful in scenarios such as (among others) emergency data recovery and future [data archeology](https://en.wikipedia.org/wiki/Data_archaeology).

All examples below assume that `~/bak` is a `baktu` repository  containing a site named `desktop`, itself containing multiple snapshots that are named `0`, `1`, and so on. As a quick reminder of the [repository format](index.md), this means that:
* snapshot data will be contained in `~/bak/sites/<SITE_NAME>/snaps/<SNAP_NAME>/data`, in this case `~/bak/sites/desktop/snaps/0/data`
* each subdirectory in the snapshot data directory will also contain a `baktu` metadata file
    * all `baktu` metadata files in a given snapshot will be named with the contents of the `meta_name.cfg.bin` file in the snapshot directory. This will be `.baktu.meta.brj`, unless it collides with files in the source dataset


## General snapshot exploration

This section describes the steps needed to explore a snapshot's contents, mainly via listing the contents of directories.


### Initial snapshot in a site

This is the easiest case:
* all non-deduplicated files are present in their normal form
* metadata for a given file is available in its record in the `baktu` metadata file in the same directory
* deduplicated files are represented as symlinks to a file with the same data, and have an `is-deduplicated` tag in their metadata record

Thus we can simply browse the snapshot data directory with normal filesystem tools, follow symlinks, and consult the metadata files where needed.


### Latest snapshot

This is slightly more complicated:
* usually most files are unchanged since the previous snapshot, and thus present as *end-markers*. Those are symlinks to the file in the beginning of their current [history interval](index.md#history-intervals), which will be in the first snapshot taken after they started having their current content. End-marker files have a `same-since i` tag in their metadata records, where `i` is the index of the first snapshot in their history interval
* modified and newly created files are present in their normal form

We can browse the snapshot data directory as before, but we will usually encounter many more symlinks, including between directories that haven't changed


### Intermediate snapshots

This is the most difficult use-case, as the snapshot data directory will contain only the subtree of modified files and their ancestors.

First, the easy aspects:
* files that have changed data in this snapshot will be represented normally, so accessing their data and metadata can be done directly using regular filesystem tools
* files whose data is unchanged, but their metadata has changed since the previous snapshot, will be represented as deduplicated files. I.e. a metadata record, an `is-deduplicated` tag and a symlink to the file from the previous snapshot

However, listing a directory's contents involves some indirection due to the history interval representation. To list the children of a directory located at `REL_PATH` in snapshot \\( S_j \\):

1. First, we need to [find the start of the history interval](#finding-the-data-and-metadata-for-a-given-path-and-snapshot) for `REL_PATH` that contains \\( S_j \\). We will denote its snapshot as \\( S_i \\)

2. Now that we have the actual on-disk version of `REL_PATH`, it will contain files for those of its children that have changed in \\( S_i \\). Those that have not changed will, however, only have minimal records in the metadata file inside of `REL_PATH`

    2.1. If neither exist, this means the directory is empty

    2.2. Otherwise, we can use the metadata file inside of `REL_PATH` to find a list of all its children. For those whose entries contain a `same-since x` key-value pair, we can use that to find their backing files, if we want access to their data and metadata

A future version of this documentation may provide sample shell scripts to automate the above.


## Finding the data and metadata for a given path and snapshot

If we want to access a file or directory at path `REL_PATH` in snapshot \\( S_j \\), we will often need to find an earlier snapshot \\( S_i \\) that denotes the start of the history interval for that path that contains \\( S_j \\) - that is where `baktu` stores the normal representation of the path and its metadata (unless deduplicated). There are two possibilities:
* \\( S_j \\) is the initial snapshot in a site. By definition it will be the start of a history interval, so we are done
* in all other cases we fall back to the general algorithm: the start of the history interval will be the latest snapshot \\( S_i \\) older or equal to \\( S_j \\) for which `REL_PATH` exists. We can find that by running a command like `ls ~/bak/sites/<SNAP_NAME>/snaps/*/<REL_PATH> | sort --field-separator=/ --key=8n` (assuming `REL_PATH` doesn't contain newlines) and ignoring all the entries newer than snapshot `j`
    * if the version of `REL_PATH` we found is an end-marker of a history interval (metadata record contains a `same-since x` key-value pair), we then use the `same-since x` key-value pair in its metadata record to jump to the appropriate snapshot that will contain both the file's data and its metadata
    * if the version of `REL_PATH` we found is a deduplicated file (metadata record contains a `is-deduplicated` tag), we can directly access the file's metadata in the current snapshot, but we need to follow the symlink to access the file's data
    * otherwise, we have found both the backing file and metadata of `REL_PATH` for snapshot \\( S_j \\)
