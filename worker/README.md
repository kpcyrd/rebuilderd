# Rebuilderd Workers

Workers are tasked with, in short, carrying out the actual rebuild. Here you'll
find:

1. The rust codebase for the worker daemon (that listens in for rebuild
   commands) under src/
2. A series of entrypoints needed by the workers (i.e.,
   rebuilder-{archlinux,debian}) to instantiate new build environments.
3. A series of dockerfiles to build dockerized worker workloads (e.g., in case
   you would want to use an [autoscaling group in a k8s
   cluster](https://kubernetes.io/docs/tasks/run-application/horizontal-pod-autoscale/)
   or in case you just want to keep workers separated).

You can test workers individually by (for example) building a container and
scheduling a build (for a debian worker):

```bash
$ docker build -t rebuilderd-worker-debian worker/Dockerfile.debian
$ docker run --cap-add=SYS_ADMIN --rm \
    rebuilderd-worker-debian build debian \
    https://buildinfos.debian.net/buildinfo-pool/r/rust-sniffglue/rust-sniffglue_0.11.1-6+b1_amd64.buildinfo
```

| :memo: WARNING          |
|:---------------------------|
| note these commands are run in the root of the repo, not here|


| :warning: WARNING          |
|:---------------------------|
| Running Debian workers as containers requires SYS\_ADMIN capabilities which could be dangerous!  |

