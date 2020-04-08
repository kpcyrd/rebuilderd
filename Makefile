all:
	cargo build --release --all

docs: contrib/rebuilderd.1 contrib/rebuilderctl.1 contrib/rebuilderd-worker.1

contrib/%.1: contrib/%.1.scd
	scdoc < $^ > $@

.PHONY: all docs
