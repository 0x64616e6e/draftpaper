# make install DESTDIR=... PREFIX=/usr     (debian/rules uses this)
PREFIX ?= /usr
SHARE = $(DESTDIR)$(PREFIX)/share/draftpaper
LIB = $(DESTDIR)$(PREFIX)/lib/draftpaper

all: check

install:
	install -Dm755 bin/draftpaper $(DESTDIR)$(PREFIX)/bin/draftpaper
	install -d $(LIB)/draftpaper $(SHARE)/qml $(SHARE)/themes $(SHARE)/i3
	install -m644 lib/draftpaper/*.py $(LIB)/draftpaper/
	install -m644 qml/*.qml $(SHARE)/qml/
	install -m644 themes/*.toml $(SHARE)/themes/
	install -m644 i3/draftpaper.conf $(SHARE)/i3/

check:
	python3 -m py_compile lib/draftpaper/*.py bin/draftpaper

.PHONY: all install check
