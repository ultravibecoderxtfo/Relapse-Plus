#!/usr/bin/env bash
# Relapse + — one-command installer for Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/OWNER/Relapse-Plus/main/install.sh | bash
#
# Environment overrides:
#   RELAPSE_PLUS_REPO=owner/repo     GitHub repository slug
#   RELAPSE_PLUS_DIR=/some/path      where the source is checked out
#   RELAPSE_PLUS_BRANCH=main         branch to install
#   RELAPSE_PLUS_SKIP_DEPS=1         do not touch system packages
set -euo pipefail

REPO_SLUG="${RELAPSE_PLUS_REPO:-OWNER/Relapse-Plus}"
INSTALL_DIR="${RELAPSE_PLUS_DIR:-$HOME/.local/share/relapse-plus}"
BRANCH="${RELAPSE_PLUS_BRANCH:-main}"

log() { printf '\033[1;35m[Relapse +]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[Relapse +]\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Linux" ] || die "Relapse + is a Linux application."

# --- system packages -------------------------------------------------------
install_packages() {
    local pkgs
    if command -v pacman >/dev/null 2>&1; then
        pkgs=(git curl pkgconf base-devel gtk4 gtksourceview5)
        sudo pacman -S --needed --noconfirm "${pkgs[@]}"
    elif command -v apt-get >/dev/null 2>&1; then
        pkgs=(git curl pkg-config build-essential libgtk-4-dev libgtksourceview-5-dev)
        sudo apt-get update -y
        sudo apt-get install -y "${pkgs[@]}"
    elif command -v dnf >/dev/null 2>&1; then
        pkgs=(git curl pkgconf-pkg-config gcc gcc-c++ make gtk4-devel gtksourceview5-devel)
        sudo dnf install -y "${pkgs[@]}"
    elif command -v zypper >/dev/null 2>&1; then
        pkgs=(git curl pkg-config gcc gcc-c++ make gtk4-devel gtksourceview5-devel)
        sudo zypper --non-interactive install "${pkgs[@]}"
    else
        die "Unsupported package manager. Install Git, curl, a C toolchain, GTK 4 and GtkSourceView 5 development packages, then re-run with RELAPSE_PLUS_SKIP_DEPS=1."
    fi
}

ensure_rust() {
    if command -v cargo >/dev/null 2>&1; then
        return
    fi
    log "Installing Rust with rustup…"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
}

if [ "${RELAPSE_PLUS_SKIP_DEPS:-0}" != "1" ]; then
    log "Installing build dependencies…"
    install_packages
fi
ensure_rust

# --- source -----------------------------------------------------------------
if [ -d "$INSTALL_DIR/.git" ]; then
    log "Updating the checkout in $INSTALL_DIR…"
    git -C "$INSTALL_DIR" fetch --depth 1 origin "$BRANCH"
    git -C "$INSTALL_DIR" reset --hard "origin/$BRANCH"
else
    log "Downloading Relapse + into $INSTALL_DIR…"
    mkdir -p "$(dirname "$INSTALL_DIR")"
    git clone --depth 1 --branch "$BRANCH" "https://github.com/$REPO_SLUG.git" "$INSTALL_DIR"
fi

# --- build and install ------------------------------------------------------
log "Building the native UI (this takes a minute the first time)…"
cargo build --release --manifest-path "$INSTALL_DIR/Cargo.toml"

log "Adding Relapse + to the application menu…"
"$INSTALL_DIR/packaging/install-app.sh"

cat <<EOF

Relapse + is installed.

  Start it from your application menu, or run:
      $INSTALL_DIR/run.sh

  Then set the Relapse launcher in Settings → Launcher and press Launch on a
  profile in the Client Manager. The UI does not inject into Roblox by itself.
EOF
