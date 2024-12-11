# Maintainer: kpcyrd <kpcyrd[at]archlinux[dot]org>

pkgbase=rebuilderd
pkgname=(rebuilderd rebuilderd-tools)
pkgver=0.0.0
pkgrel=1
pkgdesc='Independent verification system of binary packages'
url='https://github.com/kpcyrd/rebuilderd'
arch=('x86_64')
license=('GPL3')
depends=('shared-mime-info' 'xz' 'libzstd.so')
makedepends=('cargo' 'sqlite' 'scdoc')
backup=('etc/rebuilderd.conf'
        'etc/rebuilderd-sync.conf'
        'etc/rebuilderd-worker.conf')

build() {
  cd ..
  cargo build --release --locked
  make docs
}

package_rebuilderd() {
  pkgdesc='Independent verification system of binary packages (server package)'
  depends=('rebuilderd-tools' 'sqlite' 'archlinux-repro')
  backup=('etc/rebuilderd.conf'
          'etc/rebuilderd-sync.conf'
          'etc/rebuilderd-worker.conf')

  cd ..
  install -Dm 755 -t "${pkgdir}/usr/bin" \
    target/release/rebuilderd \
    target/release/rebuilderd-worker

  # install rebuilder scripts
  install -Dm 755 -t "${pkgdir}/usr/libexec/rebuilderd" \
    worker/rebuilder-*.sh

  # install config files
  install -Dm 644 -t "${pkgdir}/etc" \
    contrib/confs/rebuilderd-sync.conf
  install -Dm 640 -g 212 -t "${pkgdir}/etc" \
    contrib/confs/rebuilderd-worker.conf \
    contrib/confs/rebuilderd.conf

  # install systemd configs
  install -Dm 644 -t "${pkgdir}/usr/lib/systemd/system" \
    contrib/systemd/rebuilderd-sync@.service \
    contrib/systemd/rebuilderd-sync@.timer \
    contrib/systemd/rebuilderd-worker@.service \
    contrib/systemd/rebuilderd.service

  install -Dm 644 contrib/systemd/rebuilderd.sysusers "${pkgdir}/usr/lib/sysusers.d/rebuilderd.conf"
  install -Dm 644 contrib/systemd/rebuilderd.tmpfiles "${pkgdir}/usr/lib/tmpfiles.d/rebuilderd.conf"

  # install docs
  install -Dm 644 -t "${pkgdir}/usr/share/man/man1" \
    contrib/docs/rebuilderd.1 \
    contrib/docs/rebuilderd-worker.1
  install -Dm 644 -t "${pkgdir}/usr/share/man/man5" \
    contrib/docs/rebuilderd.conf.5 \
    contrib/docs/rebuilderd-sync.conf.5 \
    contrib/docs/rebuilderd-worker.conf.5
}

package_rebuilderd-tools() {
  pkgdesc='Independent verification system of binary packages (tools package)'

  cd ..
  install -Dm 755 -t "${pkgdir}/usr/bin" \
    target/release/rebuildctl

  # install completions
  install -d "${pkgdir}/usr/share/bash-completion/completions" \
             "${pkgdir}/usr/share/zsh/site-functions" \
             "${pkgdir}/usr/share/fish/vendor_completions.d"
  "${pkgdir}/usr/bin/rebuildctl" completions bash > "${pkgdir}/usr/share/bash-completion/completions/rebuildctl"
  "${pkgdir}/usr/bin/rebuildctl" completions zsh > "${pkgdir}/usr/share/zsh/site-functions/_rebuildctl"
  "${pkgdir}/usr/bin/rebuildctl" completions fish > "${pkgdir}/usr/share/fish/vendor_completions.d/rebuildctl.fish"

  # install docs
  install -Dm 644 README.md -t "${pkgdir}/usr/share/doc/${pkgbase}"
  install -Dm 644 -t "${pkgdir}/usr/share/man/man1" \
    contrib/docs/rebuildctl.1
}

# vim: ts=2 sw=2 et:
