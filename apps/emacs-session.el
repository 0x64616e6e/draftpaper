;;; draftpaper-session.el --- Emacs as you left it  -*- lexical-binding: t -*-
;; Written by `draftpaper apply` (config.toml [session] apps); loaded by a line in init.el.
;; The files open, each at its place, the windows and frames: saved to
;; ~/.local/state/draftpaper/emacs/ as they change (and as Emacs ends), read again at the start.

(require 'desktop)
(let ((dir (expand-file-name "draftpaper/emacs/" (or (getenv "XDG_STATE_HOME") "~/.local/state"))))
  (make-directory dir t)
  (setq desktop-path (list dir)
        desktop-dirname dir
        desktop-save t                          ; saved without asking
        desktop-load-locked-desktop 'check-pid  ; a lock left by an Emacs that died is no matter
        desktop-auto-save-timeout 15)
  (setq save-place-file (expand-file-name "places" dir)))
;; a desktop not there yet is taken all the same, else it is saved only when Emacs ends (and a
;; logout ends it without asking)
(add-hook 'desktop-no-desktop-file-hook (lambda () (desktop-claim-lock desktop-dirname)))
(desktop-save-mode 1)
(save-place-mode 1)

(provide 'draftpaper-session)
