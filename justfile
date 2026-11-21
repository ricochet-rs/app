# Keep equal to RENV_VERSION in ricochet's utils/vendor-versions.sh.
renv_version := "108c2445fa9691d9695f1a139dad91971c9952cb"

# cargo-xwin calls clang-cl, lld-link and llvm-lib by their plain names, which Homebrew and Debian keep off PATH.
llvm_on_path := 'llvm=$(ls -d /opt/homebrew/opt/llvm/bin /usr/lib/llvm-*/bin 2>/dev/null | sort -V | tail -n 1 || true); if [ -n "$llvm" ]; then export PATH="$llvm:$PATH"; fi'

# Build and run the app
run:
    cargo run --bin ricochet-app

# Compile the app and the service launcher
build:
    cargo build --release --bins

# Format the sources in place
fmt:
    cargo fmt

# Fail on anything rustfmt or clippy reports
lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

# Fail on anything clippy reports in the Windows build, cross-compiled with cargo-xwin
lint-windows:
    #!/usr/bin/env bash
    set -euo pipefail
    {{ llvm_on_path }}
    cargo xwin clippy --profile windows --target x86_64-pc-windows-msvc --bins -- -D warnings

# Run the tests
test:
    cargo test

# Sample the CPU, energy impact and memory of the app and its web UI on macOS
energy seconds="60" label="sample":
    scripts/energy.sh {{seconds}} {{label}}

# Download the renv tarball the macOS and Windows builds ship, into dist/
renv:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p dist
    out="dist/renv-{{ renv_version }}.tar.gz"
    [ -f "$out" ] && exit 0
    if [[ "{{ renv_version }}" =~ ^[0-9a-f]{40}$ ]]; then
        url="https://codeload.github.com/rstudio/renv/tar.gz/{{ renv_version }}"
    else
        url="https://github.com/rstudio/renv/archive/refs/tags/v{{ renv_version }}.tar.gz"
    fi
    curl -fsSL -o "$out" "$url"

# Assemble dist/ricochet.app for Apple Silicon, with renv and the service launcher inside
app version="0.0.0": renv
    #!/usr/bin/env bash
    set -euo pipefail
    RICOCHET_APP_VERSION="{{ version }}" cargo build --release --target aarch64-apple-darwin --bins
    bin=target/aarch64-apple-darwin/release
    app=dist/ricochet.app
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Helpers" "$app/Contents/Resources"
    cp "$bin/ricochet-app" "$app/Contents/MacOS/ricochet"
    cp "$bin/ricochet-launcher" "$app/Contents/Helpers/ricochet-launcher"
    sed "s/__VERSION__/{{ version }}/g" Support/Info.plist > "$app/Contents/Info.plist"
    cp Support/AppIcon.icns "$app/Contents/Resources/"
    cp "dist/renv-{{ renv_version }}.tar.gz" "$app/Contents/Resources/renv.tar.gz"
    # Apple Silicon refuses unsigned code, so sign ad hoc until there is a Developer ID.
    codesign --force --sign - "$app/Contents/Helpers/ricochet-launcher"
    codesign --force --sign - "$app"

# Package dist/ricochet-<version>.dmg with an Applications shortcut
dmg version="0.0.0": (app version)
    #!/usr/bin/env bash
    set -euo pipefail
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    cp -R dist/ricochet.app "$stage/"
    ln -s /Applications "$stage/Applications"
    hdiutil create -volname ricochet -srcfolder "$stage" -ov -format UDZO "dist/ricochet-{{ version }}.dmg"

# Package dist/ricochet-<version>-x86_64.AppImage with linuxdeploy, which bundles every library but glibc and the GPU drivers
appimage version="0.0.0":
    #!/usr/bin/env bash
    set -euo pipefail
    RICOCHET_APP_VERSION="{{ version }}" cargo build --release --bin ricochet-app
    appdir=dist/ricochet.AppDir
    rm -rf "$appdir"
    mkdir -p dist
    printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=ricochet' 'Exec=ricochet-app' 'Icon=ricochet' 'Categories=Development;' > dist/ricochet.desktop
    cp Support/AppIcon.png dist/ricochet.png
    tool=dist/linuxdeploy-x86_64.AppImage
    [ -x "$tool" ] || { curl -fsSL -o "$tool" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage; chmod +x "$tool"; }
    # CI containers have no FUSE, so linuxdeploy unpacks itself instead of mounting.
    APPIMAGE_EXTRACT_AND_RUN=1 LINUXDEPLOY_OUTPUT_VERSION="{{ version }}" "$tool" \
        --appdir "$appdir" \
        --executable target/release/ricochet-app \
        --desktop-file dist/ricochet.desktop \
        --icon-file dist/ricochet.png \
        --exclude-library 'libwebkit2gtk-4.1*' --exclude-library 'libjavascriptcoregtk-4.1*' \
        --exclude-library 'libgtk-3*' --exclude-library 'libgdk-3*' --exclude-library 'libgdk_pixbuf-2.0*' \
        --exclude-library 'libsoup-3.0*' --exclude-library 'libglib-2.0*' --exclude-library 'libgobject-2.0*' \
        --exclude-library 'libgio-2.0*' --exclude-library 'libgmodule-2.0*' --exclude-library 'libpango*' \
        --exclude-library 'libcairo*' --exclude-library 'libharfbuzz*' --exclude-library 'libatk*' \
        --output appimage
    mv ricochet-*-x86_64.AppImage "dist/ricochet-{{ version }}-x86_64.AppImage"

# Package dist/ricochet-<version>-windows-x86_64.zip with the service launcher and renv, cross-compiled with cargo-xwin
windows version="0.0.0": renv
    #!/usr/bin/env bash
    set -euo pipefail
    {{ llvm_on_path }}
    RICOCHET_APP_VERSION="{{ version }}" cargo xwin build --profile windows --target x86_64-pc-windows-msvc --bins
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    cp target/x86_64-pc-windows-msvc/windows/ricochet-app.exe "$stage/ricochet.exe"
    cp target/x86_64-pc-windows-msvc/windows/ricochet-launcher.exe "$stage/ricochet-launcher.exe"
    cp "dist/renv-{{ renv_version }}.tar.gz" "$stage/renv.tar.gz"
    out="$PWD/dist/ricochet-{{ version }}-windows-x86_64.zip"
    rm -f "$out"
    (cd "$stage" && zip -q "$out" ricochet.exe ricochet-launcher.exe renv.tar.gz)

# Render the app icons in Support/ from Support/logo.svg (needs rsvg-convert and iconutil)
icons:
    #!/usr/bin/env bash
    set -euo pipefail
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    # Fog rabbit on the dark background tile, sized to the 824-unit tile of Apple's 1024-unit icon grid.
    sed 's|<path fill="currentColor"|<rect x="100" y="100" width="824" height="824" rx="185" fill="#0e080d"/><path transform="translate(232 232) scale(0.546875)" fill="#d7c6f6"|' Support/logo.svg > "$work/app.svg"
    mkdir "$work/AppIcon.iconset"
    for size in 16 32 128 256 512; do
        rsvg-convert -w "$size" -h "$size" -o "$work/AppIcon.iconset/icon_${size}x${size}.png" "$work/app.svg"
        rsvg-convert -w "$((size * 2))" -h "$((size * 2))" -o "$work/AppIcon.iconset/icon_${size}x${size}@2x.png" "$work/app.svg"
    done
    iconutil -c icns -o Support/AppIcon.icns "$work/AppIcon.iconset"
    rsvg-convert -w 512 -h 512 -o Support/AppIcon.png "$work/app.svg"
