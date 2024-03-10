# Installation

## Building from source

As `baktu` is still in the work-in-progress proof of concept stage, the only supported method of installation is building from source. First, we need to install [cargo-make](https://github.com/sagiegurari/cargo-make):

```console
$ cargo install cargo-make
```

And then we can clone and build:

```console
~/code$ git clone https://github.com/OrthogonalScribe/baktu.git
~/code$ cd baktu
~/code/baktu$ cargo make
```

This will run a modified `cargo make` flow, including:
- `rustfmt`,
- `clippy`,
- building the main `baktu` executable,
- building the extended attribute helper (see below)
    - installing `libcap-dev`. The script to do this supports only Debian-based distributions at the moment, so in other environments it needs to be modified accordingly
    - permitting the binary to acquire the `CAP_SYS_ADMIN` capability
- running tests


### Setting up extended attribute recording

To properly record extended attributes, including those in the `trusted` namespace, we need the `CAP_SYS_ADMIN` capability. `baktu` provides two ways of achieving that - permitting the entire `baktu` executable to acquire the capability, or giving the permission to a helper executable with a much smaller trusted code base.


#### Option 1: Setting up `get-all-xattrs`

The `cargo make` invocation builds the helper for us, however the produced executable will still need to be in our `PATH` while running `baktu`.

If the helper is necessary (see the other alternative below) and can not be found in the `PATH`, `baktu snap` in a properly set up repository (see [Quick Start](quick-start.md)) will exit with an error:

```console
~/b2demo/bak/sites/desktop$ baktu snap --dry-run
[202X-XX-XXTXX:XX:XX.XXXXXXXXXZ ERROR baktu::cli] Got Os { code: 2, kind: NotFound, message: "No such file or directory" } while trying to spawn `get-all-xattrs`, exiting. Ensure that you're not using '~' instead of the full path in your PATH variable.
~/b2demo/bak/sites/desktop$
```

#### Option 2: Giving `CAP_SYS_ADMIN` directly to `baktu`

Alternatively, we can permit the main `baktu` executable to acquire the capability, which will result in a slight speedup and free us from the `PATH` requirements, at the cost of allowing a much more heterogenous code dependency tree to acquire what are effectively administrator privileges.

```
sudo setcap cap_sys_admin=p target/debug/baktu
```

We can confirm the result by running `getcap target/debug/baktu` (note that the path needs to be adjusted accordingly for release builds).


#### Finding out which method is used

`baktu snap` reports when it's using the helper for extended attribute fetching:

```console
~/b2demo/bak/sites/desktop$ baktu snap --dry-run -v 2>&1 | grep CAP_SYS_ADMIN
[202X-XX-XXTXX:XX:XX.XXXXXXXXXZ INFO  baktu::file::xattrs] CAP_SYS_ADMIN capability not permitted, spawning `get-all-xattrs` to record `trusted.*` extended attributes.
~/b2demo/bak/sites/desktop$
```

If the `baktu` executable itself is permitted to acquire the capability, it will do so when needed and log it at the `TRACE` level, which can be seen via a command like `baktu snap --dry-run -vvv 2>&1 | grep CAP_SYS_ADMIN`


## Setting up the development environment

The `baktu` project uses a few additional tools during development. Setting those up is described below.


### `cargo-todo`

[`cargo-todo`](https://github.com/ProbablyClem/cargo-todo) is used to keep TODO/FIXME/XXX/HACK-style comments close to the source code they reference. In addition to the installation steps in the link, we set up a `~/.cargo/todo_config` file:

```console
~/code/baktu$ cd dev-env/
~/code/baktu/dev-env$ pushd ~/.cargo
~/.cargo ~/code/baktu/dev-env
~/.cargo$ ln -s ~/code/baktu/dev-env/todo_config
~/.cargo$ popd
~/code/baktu/dev-env
~/code/baktu/dev-env$
```


### Pre-commit hook

To ensure more consistent source code formatting, and avoid committing data or code that we don't want to, we set up a pre-commit hook:

```console
~/code/baktu/dev-env$ pushd ../.git/hooks
~/code/baktu/.git/hooks ~/code/baktu/dev-env
~/code/baktu/.git/hooks$ ln -s ../../dev-env/git-hooks/pre-commit
~/code/baktu/.git/hooks$ popd
~/code/baktu/dev-env
~/code/baktu/dev-env$
```