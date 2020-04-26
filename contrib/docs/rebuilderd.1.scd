rebuilderd(1)

# NAME

rebuilderd - Independent verification of binary packages (daemon)

# SYNOPSIS

*rebuilderd* [-v] [-c /etc/rebuilderd.conf]

# DESCRIPTION

The rebuilderd binary keeps track of the state for all packages, the work queue
and assigns rebuilds to *rebuilderd-worker* instances.

# AUTHENTICATION

There are two public routes so anybody can run *rebuildctl pkgs ls* and
*rebuildctl queue ls*. Everything else requires authentication. rebuilderd is
going to check multiple locations in this order:

- ~/.config/rebuilderd.conf
- /etc/rebuilderd.conf
- /var/lib/rebuilderd/auth-cookie
- ~/.local/share/rebuilderd-auth-cookie



Paths ending with *.conf* are parsed as config files and we're looking for a
section like this:

```
[auth]
cookie = "Put your secret here"
```

With paths ending in *auth-cookie* we're taking the content as-is. If we can't
find a pre-configured value we're going to generate one and write it to
*/var/lib/rebuilderd/auth-cookie*.

# WORKER AUTHENTICATION

There are two ways to authenticate workers. If you work with a fixed number of
workers you can whitelist their keys:

```
[worker]
authorized_workers = ["key1", "key2"]
```

NOTE: the keys are currently not challenged and the worker doesn't have to
prove ownership of the private key. This is going to change in future versions.

You can also use a shared secret that allows you to dynamically join new
workers. This is especially useful in cloud setups. A secret can be generated
with `pwgen -1s 32`:

```
[worker]
signup_secret = "Put your secret here"
```

If both are not configured the workers need to provide admin credentials
described in the previous section.

# SEE ALSO

*rebuilderd.conf*(5), *rebuildctl*(1), *rebuilderd-worker*(1).

# AUTHORS

rebuilderd was originally written by kpcyrd and is hosted at https://github.com/kpcyrd/rebuilderd
