rebuilderd-worker.conf(5)

# NAME

rebuilderd-worker.conf - *rebuilderd-worker* configuration files

# SYNOPSIS

/etc/rebuilderd-worker.conf

# DESCRIPTION

This file configures *rebuilder-worker connect* and specifies both the endpoint
and the signup_secret. Both are optional but the endpoint needs to be
explicitly provided by commandline if it's not configured in the config.

# EXAMPLE

```
## The rebuilderd to connect to
endpoint = "http://127.0.0.1:8484"
## The server would either whitelist our key or require a signup secret
#signup_secret = "your_signup_key"
```

# SEE ALSO

*rebuilderd-worker*(1).

# AUTHORS

rebuilderd was originally written by kpcyrd and is hosted at https://github.com/kpcyrd/rebuilderd
