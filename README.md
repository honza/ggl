ggl --- global git log
======================

This tool shows you a log of git commits from multiple repositories ordered by
time.  The output is nearly identical to the default `git-log`.

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

``` yaml
root: /home/abc/code
repositories:
  - name: "linux"
    path: "linux"
    remote: "upstream"
    branch: "master"
    fetch: true
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
    -V, --version    Prints version information

OPTIONS:
    -c, --config <config>    Path to config file
    -u, --until <until>      How far into the past should we go?  e.g. 2022-12-31; defaults to one week ago
```

license
-------

GPLv3 or later
