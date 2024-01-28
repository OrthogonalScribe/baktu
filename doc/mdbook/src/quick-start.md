# Quick Start

This section provides a quick overview of the normal backup process with `baktu`. We demonstrate creating a new repository, adding several directories to the include list, excluding some paths and files, doing a dry run, adjusting the configuration and creating a snapshot. The below snippets assume a common Linux distribution and `bash` or a similar shell.


## Sample data

First, we'll create a few example directories and files to back up. Make sure you don't already have important files that would be overwritten by the following commands:

```console
~$ mkdir b2demo
~$ cd b2demo/
~/b2demo$ echo Lorem ipsum dolor sit amet > important.txt
~/b2demo$ setfattr -n user.enc-alg -v rot-N important.txt
~/b2demo$ setfattr -n user.enc-arg -v 26 important.txt
~/b2demo$ mkdir project/{,src,target}
~/b2demo$ cd project/
~/b2demo/project$ echo 'print("hello world")' > src/hello.py
~/b2demo/project$ cat << "EOF" > target/CACHEDIR.TAG
Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by example-build-tool.
# For information about cache directory tags, see:
#	http://www.brynosaurus.com/cachedir/
EOF
~/b2demo/project$ head -c 1234567 /dev/urandom > target/data
~/b2demo/project$ head -c 654321 /dev/urandom > src/secret-data
~/b2demo/project$ head -c 54321 /dev/urandom > report.doc
~/b2demo/project$ mkdir doc
~/b2demo/project$ cp report.doc doc/final-report.doc
~/b2demo/project$ cp doc/final-report.doc doc/final-report.v2.doc
~/b2demo/project$ cd ..
~/b2demo$
```

This results in the following file tree:

```console
~/b2demo$ tree -aF .
./
├── important.txt
└── project/
    ├── doc/
    │   ├── final-report.doc
    │   └── final-report.v2.doc
    ├── report.doc
    ├── src/
    │   ├── hello.py
    │   └── secret-data
    └── target/
        ├── CACHEDIR.TAG
        └── data

5 directories, 8 files
~/b2demo$
```


## Creating a repository

To initialize a new `baktu` repository, we first create a directory, enter it, and then use `baktu init`:

```console
~/b2demo$ mkdir bak
~/b2demo$ cd bak/
~/b2demo/bak$ baktu init
~/b2demo/bak$
```

We can visualize what `baktu` has created using the `tree` command:

```console
~/b2demo/bak$ tree -aF .
./
├── BAKTU_REPO.TAG
└── sites/

2 directories, 1 file
~/b2demo/bak$
```

[TODO: see if we can one day fix highlight.js's poor PS1 support for the 'console' language]::


## Adding a site

Next, we need to add a *site* (somewhat similar to a git branch). Sites are used to more easily deduplicate data over similar directories, for example a project's clones on a desktop and a laptop. To add a site,  we use `baktu add-site <NAME>`:

```console
~/b2demo/bak$ baktu add-site desktop
~/b2demo/bak$ tree -aF .
./
├── BAKTU_REPO.TAG
└── sites/
    └── desktop/
        ├── config.toml
        ├── exclude-paths.nsv
        ├── include-paths.nsv
        └── snaps/

4 directories, 4 files
~/b2demo/bak$
```


## Including paths to be backed up

Next, we need to choose what paths to include in the backup. To do this, we add entries to the `include-paths.nsv` [Null-Separated Values](repositories/v1/index.md#null-separated-values-format) file. This format is less easy to view and modify, but the simplest one to properly support all valid ext4 file paths. The easiest and least error-prone way to add a path is to use `baktu nsv-add-to <FILE> <PATH>`.

> **Note:** The `nsv-*` subcommands do not check for path existence, multiple occurrences and so on, leaving the validation to later subcommands. This aims to avoid getting in the user's way in scenarios such as including a path that does not exist yet, but can cause surprises before one is used to validating via `baktu snap --dry-run`. A trade-off stemming from that lack of processing is that, for example, we must be precisely in the site's directory when adding relative paths using shell autocompletion, as shown below.
>
> In addition, while the `nsv-*` subcommands mostly avoid processing the paths, there are some gotchas when it comes to shell expansion, as well as symlinks and trailing slashes. This is documented in `baktu nsv-add-to --help` and `baktu nsv-rm-from --help`, which show more extensive versions of the help documentation available via the `-h` flag.

Assuming we **won't** be adding newline-containing paths, we can visualize the contents of `include-paths.nsv` before and after our modifications using the `tr` command:

```console
~/b2demo/bak$ cd sites/desktop/
~/b2demo/bak/sites/desktop$ tr '\0' '\n' < include-paths.nsv
~/b2demo/bak/sites/desktop$
```

To add the plaintext file and directory we created earlier, we execute the following:

```console
~/b2demo/bak/sites/desktop$ baktu nsv-add-to include-paths.nsv ../../../important.txt
~/b2demo/bak/sites/desktop$ baktu nsv-add-to include-paths.nsv ../../../project
~/b2demo/bak/sites/desktop$
```

And we can use `tr` to verify the result:

```console
~/b2demo/bak/sites/desktop$ tr '\0' '\n' < include-paths.nsv
../../../important.txt
../../../project
~/b2demo/bak/sites/desktop$
```


### Removing paths from the include list

To remove a path from the include list, we can use `baktu nsv-rm-from <FILE> <PATH>`. For example, if we accidentally add something we don't want to back up:

```console
~/b2demo/bak/sites/desktop$ baktu nsv-add-to include-paths.nsv .
~/b2demo/bak/sites/desktop$ tr '\0' '\n' < include-paths.nsv
../../../important.txt
../../../project
.
~/b2demo/bak/sites/desktop$
```

Notably, the `nsv-rm-from` command does very little path massaging, as that can easily change the semantics in some edge cases, for example trailing slashes in symlinks to directories. This means we will get an error message if the path we pass does not exactly match one in the NSV file:

```console
~/b2demo/bak/sites/desktop$ baktu nsv-rm-from include-paths.nsv ./
[202X-XX-XXTXX:XX:XX.XXXXXXXXXZ ERROR baktu::cli] [46, 47] not found in "include-paths.nsv"
~/b2demo/bak/sites/desktop$
```

But passing the correct path results in it being removed from the file:

```console
~/b2demo/bak/sites/desktop$ baktu nsv-rm-from include-paths.nsv .
~/b2demo/bak/sites/desktop$ tr '\0' '\n' < include-paths.nsv
../../../important.txt
../../../project
~/b2demo/bak/sites/desktop$
```


## Excluding

### Explicit paths

Adding or removing paths from the exclude list is done the same way as with the include list, but we modify `exclude-paths.nsv` instead. For example, to exclude our `secret-data` file and verify the result, we run:

```console
~/b2demo/bak/sites/desktop$ baktu nsv-add-to exclude-paths.nsv ../../../project/src/secret-data
~/b2demo/bak/sites/desktop$ tr '\0' '\n' < exclude-paths.nsv
../../../project/src/secret-data
~/b2demo/bak/sites/desktop$
```


### Other exclusion criteria

`baktu` can also exclude files marked with the [`d` attribute](https://www.mankier.com/1/chattr#Attributes-d), as well as directories conforming to the [CACHEDIR.TAG specification](https://bford.info/cachedir/). To do that, we can modify the appropriate keys in the `exclude` section of the site's `config.toml` (see example below).


## Snapshot dry-run

Before proceeding with actually attempting to create a snapshot, it is prudent to validate the site configuration, as well as check the source dataset for any configuration-relevant issues. We can do this via running `baktu snap --dry-run` in the site's directory:

```console
~/b2demo/bak/sites/desktop$ baktu snap --dry-run
[202X-XX-XXTXX:XX:XX.XXXXXXXXXZ WARN  baktu::cli] Found valid and unexcluded CACHEDIR.TAG at "/home/user/b2demo/project/target/CACHEDIR.TAG". Rerun with --no-report-cachedir-tag or enable 'exclude.cachedir_tag' in the site 'config.toml' to hide this warning.
~/b2demo/bak/sites/desktop$
```

`baktu` warns us that there is a path that we may want to exclude (our build directory), and suggests how we can do that, as well as how to silence the warning otherwise. These warnings can also be more generally adjusted using the logging instructions in `baktu --help`, or via individual options documented in `baktu snap --help`.

We can exclude all correctly CACHEDIR.TAG-marked directories by adjusting the site configuration with a text editor:

```
~/b2demo/bak/sites/desktop$ $EDITOR config.toml
~/b2demo/bak/sites/desktop$ grep -B3 cachedir_tag config.toml
[exclude]
# Exclude CACHEDIR.TAG-marked directories (see https://bford.info/cachedir/ )
cachedir_tag = true
~/b2demo/bak/sites/desktop$
```


## Creating the initial snapshot

Now that we have a `baktu` repository with a site and some included paths, and we have done a dry-run and adjusted the site configuration, we can finally create a snapshot via running `baktu snap` without the `--dry-run` flag:

```console
~/b2demo/bak/sites/desktop$ baktu snap
~/b2demo/bak/sites/desktop$
```

Our initial snapshot is created, and we can visualize its contents using `tree`:

```console
~/b2demo/bak/sites/desktop$ tree -aF snaps/0
snaps/0/
├── data/
│   ├── .baktu.meta.brj
│   ├── important.txt
│   └── project/
│       ├── .baktu.meta.brj
│       ├── doc/
│       │   ├── .baktu.meta.brj
│       │   ├── final-report.doc
│       │   └── final-report.v2.doc -> final-report.doc
│       ├── report.doc -> doc/final-report.doc
│       └── src/
│           ├── .baktu.meta.brj
│           └── hello.py
└── meta_name.cfg.bin

5 directories, 10 files
~/b2demo/bak/sites/desktop$
```

The initial snapshot lives in `snaps/0`. Its structure is:
* `meta_name.cfg.bin`, specifying the name of the `baktu` metadata file used in the current snapshot. This is necessary to prevent collisions when e.g. backing up another `baktu` repository, or any other directory that might contain that filename.
* `data/`, which contains complete copies of the paths we have included and their children.
    * Each subdirectory contains a `baktu` metadata file, by default named `.baktu.meta.brj`, which stores the metadata of its siblings in a mostly human-readable format, [Binary Record-Jar](repositories/v1/index.md#binary-record-jar-format).
    * Deduplicated files are encoded as a symlink to the first instance of the file during the traversal of the source dataset and a tag in their metadata entry.
    * Excluded paths and their children are not included in the snapshot directory.

[FIXME: possible bug - check how we handle include roots with the same basename]::


## Creating subsequent snapshots

<div class='warning'>

**Work in progress:**

At the moment `baktu` is a work-in-progress proof of concept. It can currently create and manipulate repositories, as well as create initial snapshots with intra-snapshot file deduplication. Full testing and further feature development—including subsequent snapshot creation and FUSE mounting—are pending implementation of more extensive snapshot reading code.
</div>