all:
	cargo build --release --all

docs: contrib/docs/rebuilderd.1 contrib/docs/rebuildctl.1 contrib/docs/rebuilderd-worker.1 contrib/docs/rebuilderd.conf.5 contrib/docs/rebuilderd-sync.conf.5 contrib/docs/rebuilderd-worker.conf.5

contrib/docs/%: contrib/docs/%.scd
	scdoc < $^ > $@

.PHONY: all docs
