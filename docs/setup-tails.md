# Setting up a tails worker

Most of this is taken from the [Building a Tails
image](https://tails.boum.org/contribute/build/) instructions.

Install required packages for tails:

    sudo apt install \
        curl \
        psmisc \
        git \
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

If rebuilderd isn't packaged for your operating system, you need to install
rust and compile rebuilderd from source:

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
    apt install liblzma-dev pkg-config libzstd-dev libsqlite3-dev
    git clone https://github.com/kpcyrd/rebuilderd
    cd rebuilderd
    cargo build --release -p rebuilderd-worker
    cargo build --release -p rebuildctl
    cargo build --release -p rebuilderd
    sudo install -Dm 755 target/release/rebuilderd-worker -t /usr/local/bin/
    sudo install -Dm 755 target/release/rebuildctl -t /usr/local/bin/
    sudo install -Dm 755 target/release/rebuilderd -t /usr/local/bin/
    sudo install -Dm 755 worker/rebuilder-tails.sh -t /usr/local/libexec/

Import current tails version into rebuilderd:

    rebuildctl pkgs sync-profile --sync-config contrib/confs/rebuilderd-sync.conf tails

Verify it worked:

    rebuildctl pkgs ls --distro tails

On the worker, either run the build as root or make sure the user can use sudo
to become root. The user then also needs to be in the relevant groups:

    for group in kvm libvirt libvirt-qemu ; do
       sudo adduser "$(whoami)" "$group"
    done

Run the worker:

    rebuilderd-worker connect http://127.0.0.1:8484

You might need to troubleshoot the first few attempts, there's a "Known issues
and workarounds" section in the [Tails build
instructions](https://tails.boum.org/contribute/build/).

If you're stuck there's an irc channel at
<ircs://irc.oftc.net:6697/#reproducible-builds>. You're also welcome to tell us
about your instance if you got it to work!

Systemd units to do this automatically can be found in `contrib/systemd/`.
Instructions on how to configure everything are currently only available in the
[Arch Linux wiki](https://wiki.archlinux.org/title/Rebuilderd).
