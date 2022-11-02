ggl --- global git log
======================

This tool shows you a log of git commits from multiple repositories ordered by
time.  The output is nearly identical to the default `git-log`.

install
-------

```
$ go install github.com/honza/ggl
```

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
global git log

Usage:
  ggl [flags]

Flags:
      --config string    (default "config.yaml")
      --fetch
  -h, --help            help for ggl
      --until string    How far back should we go?  e.g. 2022-11-01  Default: 7 days ago
```

license
-------

GPLv3 or later
