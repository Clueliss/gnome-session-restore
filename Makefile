prog:
	cargo build --release

install: prog
	install target/release/gnome-session-restore /usr/local/bin/gnome-session-restore
