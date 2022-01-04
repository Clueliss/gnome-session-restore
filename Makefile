prog:
	cargo build --release

install: prog
	sudo install target/release/gnome-session-restore /usr/local/bin/gnome-session-restore

uninstall:
	sudo rm /usr/local/bin/gnome-session-restore
