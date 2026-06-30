.PHONY: install install-waybar install-ubuntu deps test doctor

install:
	@./install

install-waybar:
	@./install --waybar

install-gnome:
	@./install --gnome

install-desktop:
	@./install --gnome

deps:
	@./scripts/install-ubuntu-deps.sh

doctor:
	port-killer doctor

build:
	cargo build --release

test:
	cargo test

status:
	port-killer status

setup-waybar:
	port-killer setup waybar --install
