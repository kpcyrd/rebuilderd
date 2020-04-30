rebuilderd-sync.conf(5)

# NAME

rebuilderd-sync.conf - *rebuildctl pkgs sync-profile* configuration files

# SYNOPSIS

/etc/rebuilderd-sync.conf

# DESCRIPTION

This file allows configuring profiles that can be started with a systemd timer
or a cronjob. This way you don't need to provide these options on the
commandline all the time.

# OPTIONS

_distro=_
	The name of the distro, currently either *archlinux* or *debian*.

_suite=_
	This is for packages that have multiple suites/repositories, like *main*,
	*contrib*, *non-free* or *core*, *extra* and *community*.

_architecture=_
	The architecture of the package list we want to import.

_source=_
	This is the url we want to import from. For Arch Linux this might look
	like:

	```
	source = "https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch"
	```

_maintainers=_ (optional)
	Select packages from specific maintainers. The strings are supposed to match
	the beginning of the packager field of the packages.

	```
	maintainers = ["kpcyrd", "Levente Polyak"]
	```

_pkgs=_ (optional)
	Select packages by name:

	```
	pkgs = ["rebuilderd", "python-*"]
	```

_excludes=_ (optional)
	If a package matches one of these patterns it's *always* going to be ignored.

	```
	excludes = ["tensorflow*"]
	```

Both *pkgs=* and *excludes=* support glob patterns. If *maintainers=* and
*pkgs=* are both not set then every package is selected.

# EXAMPLE

```
## rebuild all of core
[profile."archlinux-core"]
distro = "archlinux"
suite = "core"
architecture = "x86_64"
source = "https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch"

## rebuild community packages of specific maintainers, or whitelist packages by name.
## If no filter is set, all packages are imported, if both filters are set the package only
## has to match one of them to be included.
#[profile."archlinux-community"]
#distro = "archlinux"
#suite = "community"
#architecture = "x86_64"
#source = "https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch"
#maintainers = ["somebody"]
#pkgs = ["some-pkg", "python-*"]
#excludes = ["tensorflow*"]
```

# AUTOMATIC SYNC

Enable the systemd timer to automatically sync specific profiles:

```
systemctl enable --now rebuilderd-sync@archlinux-core.timer
```

The argument behind the @ is refering to the section in the config file.

# SEE ALSO

*rebuilderd*(1), *rebuildctl*(1).

# AUTHORS

rebuilderd was originally written by kpcyrd and is hosted at https://github.com/kpcyrd/rebuilderd
