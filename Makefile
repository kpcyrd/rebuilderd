all:
	cargo build --release --all

docs: contrib/docs/rebuilderd.1 contrib/docs/rebuildctl.1 contrib/docs/rebuilderd-worker.1 contrib/docs/rebuilderd.conf.5 contrib/docs/rebuilderd-sync.conf.5 contrib/docs/rebuilderd-worker.conf.5

contrib/docs/%: contrib/docs/%.scd
	scdoc < $^ > $@

install:
	@if [ ! -e target/release/rebuilderd ]; then \
		echo >&2 "No binary found, run make first"; \
		false; \
	fi
	install -Dm 755 -t "$(DESTDIR)/usr/bin/" \
		target/release/rebuilderd \
		target/release/rebuildctl \
		target/release/rebuilderd-worker
	install -Dm 755	worker/rebuilder-*.sh -t "$(DESTDIR)/usr/libexec/rebuilderd/" \
	for x in rebuilderd.conf rebuilderd-sync.conf rebuilderd-worker.conf; do \
		test -e "$(DESTDIR)/etc/$$x" || install -Dm 644 "contrib/confs/$$x" -t "$(DESTDIR)/etc" ; \
	done
	install -Dm 644 -t "$(DESTDIR)/usr/lib/systemd/system" \
		contrib/systemd/rebuilderd-sync@.service \
		contrib/systemd/rebuilderd-sync@.timer \
		contrib/systemd/rebuilderd-worker@.service \
		contrib/systemd/rebuilderd.service
	install -Dm 644 contrib/systemd/rebuilderd.sysusers "$(DESTDIR)/usr/lib/sysusers.d/rebuilderd.conf"
	install -Dm 644 contrib/systemd/rebuilderd.tmpfiles "$(DESTDIR)/usr/lib/tmpfiles.d/rebuilderd.conf"
	systemd-sysusers
	systemd-tmpfiles --create

.PHONY: all docs install
