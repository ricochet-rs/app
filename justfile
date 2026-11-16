# Keep equal to RENV_VERSION in ricochet's utils/vendor-versions.sh.
renv_version := "108c2445fa9691d9695f1a139dad91971c9952cb"

# Build and run the app
run:
    cargo run --bin ricochet-app

# Compile the app and the macOS service launcher
build:
    cargo build --release --bins

# Format the sources in place
fmt:
    cargo fmt

# Fail on anything rustfmt or clippy reports
lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings

# Run the tests
test:
    cargo test

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
    cargo build --release --target aarch64-apple-darwin --bins
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
    dmg="dist/ricochet-{{ version }}.dmg"
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    mkdir "$stage/volume" "$stage/readback"
    cp -R dist/ricochet.app "$stage/volume/"
    ln -s /Applications "$stage/volume/Applications"
    # bsdtar writes the volume as a file, because `hdiutil create -srcfolder` attaches a disk image to fill it, which Crow's Mac sandbox denies.
    # Rock Ridge carries the shortcut and, with `strict`, the file modes as they are; its default drops write permission, and a copied app could not be deleted.
    bsdtar -cf "$stage/volume.iso" --format iso9660 --options 'iso9660:volume-id=ricochet,iso9660:rockridge=strict' -C "$stage/volume" .
    hdiutil convert "$stage/volume.iso" -format UDZO -ov -o "$dmg"
    # Read the app back out of the image, because ISO 9660 keeps no extended attributes and that sandbox cannot mount the image to look.
    hdiutil convert "$dmg" -format UDTO -o "$stage/image"
    bsdtar -xf "$stage/image.cdr" -C "$stage/readback"
    diff -r dist/ricochet.app "$stage/readback/ricochet.app"
    codesign --verify --deep --strict "$stage/readback/ricochet.app"

# Package dist/ricochet-<version>-x86_64.AppImage with linuxdeploy, which bundles every library but glibc and the GPU drivers
appimage version="0.0.0":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --bin ricochet-app
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
        --output appimage
    mv ricochet-*-x86_64.AppImage "dist/ricochet-{{ version }}-x86_64.AppImage"

# Package dist/ricochet-<version>-windows-x86_64.zip with the service launcher and renv, cross-compiled with cargo-xwin
windows version="0.0.0": renv
    #!/usr/bin/env bash
    set -euo pipefail
    cargo xwin build --profile windows --target x86_64-pc-windows-msvc --bins
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
