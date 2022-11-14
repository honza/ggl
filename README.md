ggl --- global git log
======================

This tool shows you a log of git commits from multiple repositories ordered by
time.  The output is nearly identical to the default `git-log`.

install
-------

TODO


config
------

A yaml file which specifies which repositories you want to include, and their
respective remotes and branches.

``` yaml
root: /home/abc/code
repositories:
  - name: "linux"
    path: "linux"
    remote: "upstream"
    branch: "master"
    fetch: true
```

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
    -V, --version    Prints version information

OPTIONS:
    -u, --until <until>    How far into the past should we go?  e.g. 2022-12-31; defaults to one week ago
```

license
-------

GPLv3 or later
