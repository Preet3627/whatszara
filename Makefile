.PHONY: setup check bridge desktop build build-macos build-linux build-windows clean help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

setup: ## Full one-click setup (recommended)
	chmod +x setup.sh && ./setup.sh

check: ## Check prerequisites without installing
	chmod +x setup.sh && ./setup.sh check

bridge: ## Start WhatsApp bridge (scan QR code)
	cd whatsapp-bridge && go run main.go

desktop: ## Launch Tauri desktop app (dev mode)
	cd desktop-app && npm run tauri dev

build: ## Build desktop app for current platform
	cd desktop-app && npm run tauri build

build-macos: ## Build macOS universal binary
	cd desktop-app && npm run tauri build -- --target universal-apple-darwin

build-linux: ## Build Linux app
	cd desktop-app && npm run tauri build

build-windows: ## Build Windows app
	cd desktop-app && npm run tauri build

run: bridge desktop ## Start bridge + desktop
	@echo "Run in two terminals: make bridge | make desktop"

clean: ## Clean build artifacts
	rm -rf desktop-app/dist desktop-app/src-tauri/target
	find . -name "*.db" -delete
	@echo "Cleaned."
