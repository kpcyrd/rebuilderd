# Setting up a tails worker

Most of this is taken from the [Building a Tails
image](https://tails.boum.org/contribute/build/) instructions. This has been
tested on debian bullseye. The instructions are assumed to be executed by a
regular user which is allowed to sudo to root without a password (otherwise the
tails build wouldn't be non-interactive).

If you're running this in a VM you need to make sure you have [nested
virtualization](https://pve.proxmox.com/wiki/Nested_Virtualization) setup
because the tails build itself is also creating VMs.

Install required packages for tails:

```sh
sudo apt install \
    curl \
    sudo \
    dpkg-dev \
    psmisc \
    git \
    gpg \
    gpg-agent \
    rake \
    libvirt-daemon-system \
    dnsmasq-base \
    ebtables \
    faketime \
    pigz \
    qemu-system-x86 \
    qemu-utils \
    vagrant \
    vagrant-libvirt \
    vmdb2
```

## Installing

If rebuilderd isn't packaged for your operating system, you need to install
rust and compile rebuilderd from source:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
sudo apt install liblzma-dev pkg-config libzstd-dev libsqlite3-dev gcc libssl-dev
git clone https://github.com/kpcyrd/rebuilderd
cd rebuilderd
cargo build --release
sudo install -Dm 755 -t /usr/bin/ \
    target/release/rebuilderd \
    target/release/rebuildctl \
    target/release/rebuilderd-worker
sudo install -Dm 755 worker/rebuilder-tails.sh -t /usr/libexec/rebuilderd/
sudo install -Dm 644 -t /etc \
    contrib/confs/rebuilderd-sync.conf \
    contrib/confs/rebuilderd-worker.conf \
    contrib/confs/rebuilderd.conf
```

Note: the permissions on `contrib/confs/rebuilderd.conf` need to be set more
strictly if you're planning to add secrets to this file, by default the file
doesn't contain any sensitive information.

## Starting the daemon and worker

### With systemd

Install the systemd config files:

```sh
sudo install -Dm 644 -t "/usr/lib/systemd/system" \
    contrib/systemd/rebuilderd-sync@.service \
    contrib/systemd/rebuilderd-sync@.timer \
    contrib/systemd/rebuilderd-worker@.service \
    contrib/systemd/rebuilderd.service
sudo install -Dm 644 contrib/systemd/rebuilderd.sysusers "/usr/lib/sysusers.d/rebuilderd.conf"
sudo install -Dm 644 contrib/systemd/rebuilderd.tmpfiles "/usr/lib/tmpfiles.d/rebuilderd.conf"
```

Run setup:

```sh
sudo systemd-sysusers
sudo systemd-tmpfiles --create
```

Start the daemon and a worker:

```sh
sudo systemctl enable --now rebuilderd rebuilderd-worker@0
```

To manage rebuilderd you need access to `/var/lib/rebuilderd/`, for now
check everything is working correctly by runnig:

```sh
sudo rebuildctl status
```

This should show one worker that's currently idle.

You can add yourself to the rebuilderd group so you don't need to run
rebuildctl with sudo:

```sh
sudo adduser "$(whoami)" rebuilderd
```

You need to re-login for this to work. Check it worked correctly like this:

```sh
id
rebuildctl status
```

### Manually

You can skip this section if you've setup rebuilderd to to run with systemd.

Open a new terminal to run the rebuilderd daemon in the background. Be aware
that rebuilderd creates data in the working directory:

```sh
mkdir ~/rebuilderd-data
cd ~/rebuilderd-data
rebuilderd -c /etc/rebuilderd.conf -v
```

Open another terminal and start a worker, you have to add yourself to the right
groups first:

```sh
for group in kvm libvirt libvirt-qemu ; do
   sudo adduser "$(whoami)" "$group"
done
```

You need to re-login afterwards, verify you're in the right groups:

```sh
id
```

Then run the worker. Be aware that the worker creates data in the working
directory:

```sh
mkdir ~/rebuilderd-worker
cd ~/rebuilderd-worker
rebuilderd-worker connect http://127.0.0.1:8484
```

## Starting the rebuild

Import the current tails version into rebuilderd:

```sh
rebuildctl pkgs sync-profile --sync-config /etc/rebuilderd-sync.conf tails
```

Verify it worked, this should show two images in "unknown" state:

```sh
rebuildctl pkgs ls --distro tails
```

You can monitor the build queue like this, it's going to indicate when the job has started:

```sh
CLICOLOR_FORCE=1 watch -c rebuildctl queue ls --head
```

It's going to take a few moments for the worker to pickup the job. You can
speed this up by restarting the worker.

If you're using systemd you can monitor the build log with journalctl

```sh
sudo journalctl -fu rebuilderd-worker@0
```

You might need to troubleshoot the first few attempts, there's a "Known issues
and workarounds" section in the [Tails build
instructions](https://tails.boum.org/contribute/build/).

If something went wrong it's going to occasionally retry after a while, you can
cause an immediate requeue like this:

```sh
rebuildctl pkgs requeue --reset --distro tails
```

If you're stuck there's an irc channel at
<ircs://irc.oftc.net:6697/#reproducible-builds>. You're also welcome to tell us
about your instance if you got it to work!

Systemd units to do this automatically can be found in `contrib/systemd/`.
Instructions on how to configure everything are currently only available in the
[Arch Linux wiki](https://wiki.archlinux.org/title/Rebuilderd).
