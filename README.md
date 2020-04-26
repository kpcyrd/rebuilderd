# rebuilderd(1) [![crates.io][crates-img]][crates] [![irc.freenode.net.org:6697/#rebuilderd][irc-img]][irc]

[crates-img]:   https://img.shields.io/crates/v/rebuilderd.svg
[crates]:       https://crates.io/crates/rebuilderd
[irc-img]:      https://img.shields.io/badge/freenode-%23rebuilderd-blue.svg
[irc]:          https://webchat.freenode.net/#rebuilderd

Independent verification system of binary packages.

![rebuildctl pkgs ls example output](.github/assets/Vx35qrG.png)

# Setup

## Arch Linux

Please see the setup instructions in the [Arch Linux Wiki](https://wiki.archlinux.org/index.php/Rebuilderd).

## Development

A rebuilder consists of the `rebuilderd` daemon and >= 1 workers:

Run rebuilderd:
```
cd daemon; cargo run
```

Run a rebuild worker:
```
cd worker; cargo run connect http://127.0.0.1:8484
```

Afterwards you should import some packages:
```
cd tools; cargo run pkgs sync archlinux community x86_64 https://ftp.halifax.rwth-aachen.de/archlinux/community/os/x86_64/community.db --maintainer kpcyrd
```

The `--maintainer` option is optional and allows you to rebuild packages by a specific maintainer only.

To show the current status of our imported packages run:
```
cd tools; cargo run pkgs ls
```

To inspect the queue run:
```
cd tools; cargo run queue ls
```

# Dependencies

Debian: pkg-config liblzma-dev libssl-dev libsodium-dev libsqlite3-dev

# Support

| Distro     | Status       |
| ---------- | ------------ |
| Arch Linux | Experimental |
| Debian     | Planned      |

# Funding

Development is currently funded by:

- kpcyrd's savings account

Please consider [supporting the project](https://github.com/sponsors/kpcyrd).

# License

GPLv3+
