rebuilderd.conf(5)

# NAME

rebuilderd.conf - *rebuilderd* and *rebuildctl* configuration files

# SYNOPSIS

/etc/rebuilderd.conf

~/.config/rebuilderd.conf

# DESCRIPTION

This file contains configuration for both rebuilderd and rebuildctl.

# OPTIONS

## [http]

_bind_addr=_
	The address to bind to. This is 127.0.0.1:8484 by default.

_endpoint=_
	Set a default endpoint for rebuildctl. This is especially useful for the
	sync timer. Defaults to *http://127.0.0.1:8484*.

## [auth]

_cookie=_
	A random cookie to authenticate administrators. If this value is not set
	rebuilderd is going to generate one on it's own and write it to
	*/var/lib/rebuilderd/auth-cookie*. You only need to configure this if you
	want to administrate remotely.

## [endpoints."https://rebuilder.example.com"]

_cookie=_
	This section is usually only configured in *~/.config/rebuilderd.conf*. It
	allows setting cookies for alternative rebuilderd instances with *-H*. The
	value passed by *-H* need to match the section header exactly, make sure
	there's no trailing slashes.

## [worker]

_authorized_workers=_
	Whitelist public key of *rebuilderd-worker* instances. See the
	authentication section in *rebuilderd*(1) for details.

_signup_secret=_
	Configure a shared secret that a *rebuilderd-worker* must provide to be
	allowed to join. See the authentication section in *rebuilderd*(1) for
	details.

# EXAMPLE

```
## Configuration for http daemon
#[http]
## The address to bind to. This is 127.0.0.1:8484 by default.
#bind_addr = "0.0.0.0:8484"
## Set a default endpoint for rebuildctl. This is especially useful for the sync timer.
#endpoint = "http://127.0.0.1:8484"

## A random cookie for administration is generated at startup and written to /var/lib/rebuilderd/auth-cookie
## You can set this to a fixed value here. Use `pwgen -1s 32` to generate one.
## rebuildctl is searching for this cookie in ~/.config/rebuilderd.conf, /etc/rebuilderd.conf and
## /var/lib/rebuilderd/auth-cookie in that order.
#[auth]
#cookie = "INSECURE"

## The auth cookie above is only used for the default endpoint.
## You can use different endpoints with `rebuildctl -H https://rebuilder.example.com status`,
## In that case you need to configure a section below if you want to attach a cookie.
#[endpoints."https://rebuilder.example.com"]
#cookie = "INSECURE"

## IMPORTANT: in production, make sure either `authorized_workers` or `signup_secret` is configured.
#[worker]
## If we have a fixed set of workers we can whitelist the keys here.
#authorized_workers = ["key1", "key2"]
## If we want to spawn new workers dynamically we can configure a sign up secret below.
## Use `pwgen -1s 32` to generate one.
#signup_secret = "INSECURE"
```

# SEE ALSO

*rebuilderd*(1), *rebuildctl*(1).

# AUTHORS

rebuilderd was originally written by kpcyrd and is hosted at https://github.com/kpcyrd/rebuilderd
