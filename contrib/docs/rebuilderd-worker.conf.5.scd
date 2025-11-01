rebuilderd-worker.conf(5)

# NAME

rebuilderd-worker.conf - *rebuilderd-worker* configuration files

# SYNOPSIS

/etc/rebuilderd-worker.conf

# DESCRIPTION

This file configures *rebuilder-worker connect* and specifies both the endpoint
and the signup_secret. Both are optional but the endpoint needs to be
explicitly provided by commandline if it's not configured in the config.

# OPTIONS

_endpoint=_
	The rebuilderd to connect to.

_signup_secret=_
	The server would either allowlist our key or require a signup secret.

## [build]

_timeout=_
	Set a timeout in seconds after which diffoscope is terminated (defaults to 24 hours).

_max_bytes=_
	Set a maximum diffoscope output limit in bytes (default: none).
	When reaching this limit the log is truncated but the rebuilder backend is
	*not* terminated.

_silent=_
	By default build output is forwarded to stdout/stderr.
	This can be disabled by settings this to true.

## [diffoscope]

_enabled=_
	Generate and attach diffs with diffoscope when rebuilding.

_args=_
	Pass additional arguments to diffoscope. Use wisely, some options might not work well.

_timeout=_
	Set a timeout in seconds after which diffoscope is terminated (defaults to 1 hour).

_max_bytes=_
	Set a maximum diffoscope output limit in bytes (default: none).
	When reaching this limit diffoscope is terminated and the output is truncated.

# EXAMPLE

```
## The rebuilderd to connect to
endpoint = "http://127.0.0.1:8484"
## The server would either allowlist our key or require a signup secret
#signup_secret = "INSECURE"

[build]
#timeout = 86400 # 24 hours
## Set a maximum build log limit in bytes (default: none).
## When reaching this limit the log is truncated but the rebuilder backend is *not* terminated.
max_bytes = 10485760 # 10 MiB

[diffoscope]
## Generate and attach diffs with diffoscope when rebuilding
enabled = false
## Pass additional arguments to diffoscope. Use wisely, some options might not work well.
#args = ["--max-container-depth", "2", "--fuzzy-threshold", "0"]
## Set a timeout in seconds after which diffoscope is terminated (default: 3600)
#timeout = 600 # 10 minutes
## Set a maximum diffoscope output limit in bytes (default: none).
## When reaching this limit diffoscope is terminated and the output is truncated.
max_bytes = 41943040 # 40 MiB
```

# SEE ALSO

*rebuilderd-worker*(1).

# AUTHORS

rebuilderd was originally written by kpcyrd and is hosted at https://github.com/kpcyrd/rebuilderd
