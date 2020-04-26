rebuildctl(1)

# NAME

rebuildctl - Manage and query a *rebuilderd*(1)

# SYNOPSIS

*rebuildctl* [-vH https://rebuilder.example.com] [-c /etc/rebuilderd.conf] <command>

# DESCRIPTION

The rebuildctl binary is used to query and manage *rebuilderd* instances.
Functionality is split into multiple subcommands.

# STATUS

Lists the available workers and what they are currently working on.

*--color*
	Force colors even if stdout is not a tty. This is useful with *watch -c*.

*rebuildctl status*

# PKGS

## LS

List the packages known to rebuilderd along with their status.

*--color*
	Force colors even if stdout is not a tty. This is useful with *watch -c*.

*--json*
	Print the response as json instead of pretty-printing it.

*--distro <distro>*
	Select packages from a specific distro, like *archlinux* or *debian*.

*--suite <suite>*
	Select packages from a specific suite/repository, like *main*, *contrib*,
	*non-free* or *core*, *extra* and *community*.

*--architecture <architecture>*
	Select packages with a specific architecture.

*--name <name>*
	Select packages with a specific name.

*--status <status>*
	Select packages with a specific status. Possible values are *GOOD*, *BAD*,
	and *UNKWN*.

*rebuildctl pkgs ls* --status GOOD

## SYNC

Sync a set of packages into rebuilderd and automatically queue them for
verification. For an in-depth description of how the filters work you can look
into *rebuilderd-sync.conf*(5).

*--print-json*
	Do a dry-run and only show what we would sync instead of actually sending
	it to rebuilderd.

*--maintainer <maintainers>*
	Only pick packages by specific maintainers.

*--pkg <pattern>*
	Only pick specific packages or packages matching a pattern.

*--exclude <pattern>*
	Always ignore packages that match this pattern, even if it also matches one
	of the other filters.

*rebuildctl pkgs sync* archlinux community x86_64 \\++
\	https://ftp.halifax.rwth-aachen.de/archlinux/community/os/x86_64/community.db \\++
\	--maintainer kpcyrd --print-json

## SYNC-PROFILES

This command is very similar to *pkgs sync*, but reads all the options from a
configuration file instead. These profiles are also used by the systemd timer.

*--color*
	Force colors even if stdout is not a tty. This is useful with *watch -c*.

*--print-json*
	Do a dry-run and only show what we would sync instead of actually sending
	it to rebuilderd.

*--sync-config /etc/rebuilderd-sync.conf*
	The configuration file to read profiles from.

*rebuildctl pkgs sync-profile* archlinux-core

# QUEUE

## LS

List the current queue.

*--color*
	Force colors even if stdout is not a tty. This is useful with *watch -c*.

*--head*
	Only show the first 25 items in the queue.

*--json*
	Print the response as json instead of pretty-printing it.

*rebuildctl queue ls* --head

## PUSH

Push a specific package to the work queue again. If the version is not provided
we automaticlly select the current one.

*--architecture*
	You can optionally filter by architecture. Otherwise all matches are added
	to the queue.

*rebuildctl queue push* archlinux community rebuilderd

## DROP

Drop a specific package from the work queue. Optionally select a specific version to drop.

*rebuildctl queue drop* archlinux community rebuilderd

# SEE ALSO

*rebuilderd*(1), *rebuilderd.conf*(5), *rebuilderd-sync.conf*(5).

# AUTHORS

rebuilderd was originally written by kpcyrd and is hosted at https://github.com/kpcyrd/rebuilderd
