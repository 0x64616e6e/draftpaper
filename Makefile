# make build                          the compositor (cargo; debian/rules runs it)
# make install DESTDIR=... PREFIX=/usr
PREFIX ?= /usr
SHARE = $(DESTDIR)$(PREFIX)/share/draftpaper
LIB = $(DESTDIR)$(PREFIX)/lib/draftpaper
CARGO ?= cargo
COMPOSITOR = compositor/target/release/draftpaper-compositor

all: build

build: $(COMPOSITOR)

$(COMPOSITOR):
	cd compositor && $(CARGO) build --release --locked

install: $(COMPOSITOR)
	# the programs
	install -Dm755 bin/draftpaper $(DESTDIR)$(PREFIX)/bin/draftpaper
	install -Dm755 $(COMPOSITOR) $(DESTDIR)$(PREFIX)/bin/draftpaper-compositor
	install -Dm755 compositor/session/draftpaper-session $(DESTDIR)$(PREFIX)/bin/draftpaper-session
	install -Dm755 compositor/session/draftpaper-as $(DESTDIR)$(PREFIX)/bin/draftpaper-as
	install -d $(LIB)/draftpaper $(SHARE)/qml $(SHARE)/themes $(SHARE)/apps
	install -m644 lib/draftpaper/*.py $(LIB)/draftpaper/
	# the shell, the themes and the applications' templates
	install -m644 qml/*.qml $(SHARE)/qml/
	install -m644 themes/*.toml $(SHARE)/themes/
	install -m644 apps/* $(SHARE)/apps/
	# the session: the display manager's entry, the user units, the portals
	install -Dm644 compositor/session/draftpaper.desktop $(DESTDIR)$(PREFIX)/share/wayland-sessions/draftpaper.desktop
	install -d $(DESTDIR)$(PREFIX)/lib/systemd/user
	install -m644 compositor/session/draftpaper-session.target compositor/session/draftpaper-shell.service \
		compositor/session/draftpaper-idle.service compositor/session/draftpaper-audio.service \
		$(DESTDIR)$(PREFIX)/lib/systemd/user/
	install -Dm644 compositor/session/draftpaper-portals.conf $(DESTDIR)$(PREFIX)/share/xdg-desktop-portal/draftpaper-portals.conf
	install -Dm644 compositor/session/xdg-desktop-portal-wlr.conf $(DESTDIR)/etc/xdg/xdg-desktop-portal-wlr/Draftpaper

check:
	python3 -m py_compile lib/draftpaper/*.py bin/draftpaper
	cd compositor && $(CARGO) test --release --locked

clean:
	cd compositor && $(CARGO) clean

.PHONY: all build install check clean
