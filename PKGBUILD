# Maintainer: kpcyrd <kpcyrd[at]archlinux[dot]org>

pkgname=rebuilderd
pkgver=0.0.0
pkgrel=1
pkgdesc='Independent verification system of binary packages'
url='https://github.com/kpcyrd/rebuilderd'
arch=('x86_64')
license=('GPL3')
depends=('libsodium' 'sqlite' 'archlinux-repro')
makedepends=('cargo')
backup=('etc/rebuilderd.conf'
        'etc/rebuilderd-sync.conf'
        'etc/rebuilderd-worker.conf')

build() {
  cd ..
  cargo build --release --locked
}

package() {
  cd ..
  install -Dm 755 -t "${pkgdir}/usr/bin" \
    target/release/rebuilderd \
    target/release/rebuilderd-worker \
    target/release/rebuildctl

  # install rebuilder scripts
  install -Dm 755 -t "${pkgdir}/usr/libexec/rebuilderd" \
    worker/rebuilder-archlinux.sh \
    worker/rebuilder-debian.sh

  # install config files
  install -Dm 644 -t "${pkgdir}/etc" \
    contrib/confs/rebuilderd-sync.conf
  install -Dm 640 -g 212 -t "${pkgdir}/etc" \
    contrib/confs/rebuilderd-worker.conf \
    contrib/confs/rebuilderd.conf

  # install completions
  install -d "${pkgdir}/usr/share/bash-completion/completions" \
             "${pkgdir}/usr/share/zsh/site-functions" \
             "${pkgdir}/usr/share/fish/vendor_completions.d"
  "${pkgdir}/usr/bin/rebuildctl" completions bash > "${pkgdir}/usr/share/bash-completion/completions/rebuildctl"
  "${pkgdir}/usr/bin/rebuildctl" completions zsh > "${pkgdir}/usr/share/zsh/site-functions/_rebuildctl"
  "${pkgdir}/usr/bin/rebuildctl" completions fish > "${pkgdir}/usr/share/fish/vendor_completions.d/rebuildctl.fish"

  # install systemd configs
  install -Dm 644 -t "${pkgdir}/usr/lib/systemd/system" \
    contrib/systemd/rebuilderd-sync@.service \
    contrib/systemd/rebuilderd-sync@.timer \
    contrib/systemd/rebuilderd-worker@.service \
    contrib/systemd/rebuilderd.service

  install -Dm 644 contrib/systemd/rebuilderd.sysusers "${pkgdir}/usr/lib/sysusers.d/rebuilderd.conf"
  install -Dm 644 contrib/systemd/rebuilderd.tmpfiles "${pkgdir}/usr/lib/tmpfiles.d/rebuilderd.conf"

  # install docs
  install -Dm 644 README.md -t "${pkgdir}/usr/share/doc/${pkgname}"
}

# vim: ts=2 sw=2 et:
