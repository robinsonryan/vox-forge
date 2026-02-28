PREFIX     := $(HOME)/.local
BIN_DIR    := $(PREFIX)/bin
SERVICE_DIR := $(HOME)/.config/systemd/user
BINARY     := vox-forge
CARGO_ENV  := LIBCLANG_PATH=/usr/lib/llvm-18/lib

.PHONY: build install uninstall reinstall status

build:
	$(CARGO_ENV) cargo build --release

install: build
	@mkdir -p $(BIN_DIR) $(SERVICE_DIR)
	cp target/release/$(BINARY) $(BIN_DIR)/$(BINARY)
	cp install/vox-forge.service $(SERVICE_DIR)/vox-forge.service
	systemctl --user daemon-reload
	systemctl --user enable --now vox-forge.service
	@echo ""
	@echo "=== VoxForge installed ==="
	@echo "Binary:  $(BIN_DIR)/$(BINARY)"
	@echo "Service: $(SERVICE_DIR)/vox-forge.service"
	@echo ""
	@echo "Daemon is running. Verify with:"
	@echo "  systemctl --user status vox-forge"
	@echo "  vox-forge status"
	@echo ""
	@echo "=== Set up your hotkey ==="
	@echo "COSMIC: Settings > Keyboard > Keyboard Shortcuts > Custom"
	@echo "  Name:     VoxForge Toggle"
	@echo "  Command:  $(BIN_DIR)/$(BINARY) toggle"
	@echo "  Shortcut: Super+Shift+D"
	@echo ""
	@echo "Optional cancel shortcut:"
	@echo "  Command:  $(BIN_DIR)/$(BINARY) cancel"
	@echo ""

uninstall:
	-systemctl --user stop vox-forge.service
	-systemctl --user disable vox-forge.service
	rm -f $(SERVICE_DIR)/vox-forge.service
	systemctl --user daemon-reload
	rm -f $(BIN_DIR)/$(BINARY)
	@echo "VoxForge uninstalled."

reinstall: uninstall install

status:
	@systemctl --user status vox-forge.service || true
	@echo ""
	@$(BIN_DIR)/$(BINARY) status 2>/dev/null || echo "Daemon not responding via IPC."
