ggl --- global git log
======================

This tool shows you a log of git commits from multiple repositories ordered by
time.  The output is nearly identical to the default `git-log`.

features
--------

The goal of this tool is to discover a commit that broke things, and as such, we
use `--topo-order` when presenting the results.  This means that a merge commit
is followed by all of its children before other commits are shown.

You can ask `ggl` to run `git fetch` for you.

You can specify which paths you care about in busy repository with filters.

By default, we go 1 week into the past, and of course you can set your own
value.

install
-------

### cargo

``` sh
$ cargo install ggl
```

### from source

``` sh
$ git clone https://github.com/honza/ggl
$ cd ggl
$ cargo build --release
$ ./target/release/ggl --help
```

config
------

A yaml file which specifies which repositories you want to include, and their
respective remotes and branches.

A `block` is a collection of repositories that share a common root directory.
When running `git fetch` we use the `remote` and `branch` information.

By default, we don't run `git fetch`: you have to pass in the `--fetch` flag.
If you never wish to fetch a repo, you can say so in the config.

``` yaml
blocks:
- root: /home/abc/code
  repositories:
    - name: "linux"
      path: "linux"
      remote: "upstream"
      branch: "master"
      fetch: true
      filters:
        - filter_type: Include
          paths:
            - src/important-file.txt
```

`ggl` will look for the config file in the following places:

1.  `--config` flag
2.  `$XDG_CONFIG_HOME/ggl.yaml`
3.  `config.yaml` in the current directory

usage
-----

```
ggl

USAGE:
    ggl [FLAGS] [OPTIONS]

FLAGS:
    -f, --fetch      Run git fetch
    -h, --help       Prints help information
    -j, --json       Print JSON
    -r, --reverse    Reverse the result
    -V, --version    Prints version information

OPTIONS:
    -c, --config <config>    Path to config file
    -u, --until <until>      How far into the past should we go?  e.g. 2022-12-31; defaults to one week ago
```

license
-------

GPLv3 or later
