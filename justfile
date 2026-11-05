# Keep equal to RENV_VERSION in ricochet's utils/vendor-versions.sh.
renv_version := "108c2445fa9691d9695f1a139dad91971c9952cb"

# The Crow Mac agent's sandbox blocks SwiftPM's sandbox and module cache, and its Swift Build misses SwiftUI's macros.
swiftpm_flags := '--build-system native --disable-sandbox -Xbuild-tools-swiftc -module-cache-path -Xbuild-tools-swiftc "' + justfile_directory() + '/.build/ModuleCache"'

# Build dist/Ricochet.app and open it, quitting the copy already running
run: app
    -pkill -x Ricochet
    open dist/Ricochet.app

# Compile the app
build:
    swift build -c release --arch arm64 {{ swiftpm_flags }}

# Format the Swift sources in place with swift-format
fmt:
    swift format --in-place --recursive --parallel Sources Tests Package.swift

# Fail on any change swift-format would make
lint:
    swift format lint --strict --recursive --parallel Sources Tests Package.swift

# Run the tests, pointing SwiftPM at the Command Line Tools' Swift Testing, which it does not search
test:
    #!/usr/bin/env bash
    set -euo pipefail
    dev="${DEVELOPER_DIR:-$(xcode-select -p)}/Library/Developer"
    swift test {{ swiftpm_flags }} \
        -Xswiftc -F -Xswiftc "$dev/Frameworks" \
        -Xlinker -F -Xlinker "$dev/Frameworks" \
        -Xlinker -rpath -Xlinker "$dev/Frameworks" \
        -Xlinker -rpath -Xlinker "$dev/usr/lib"

# Assemble dist/Ricochet.app with renv bundled
app version="0.0.0": build
    #!/usr/bin/env bash
    set -euo pipefail
    app=dist/Ricochet.app
    rm -rf "$app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp "$(swift build -c release --arch arm64 {{ swiftpm_flags }} --show-bin-path)/Ricochet" "$app/Contents/MacOS/Ricochet"
    sed "s/__VERSION__/{{ version }}/g" Support/Info.plist > "$app/Contents/Info.plist"
    cp Support/AppIcon.icns Support/MenuBarRunning.pdf Support/MenuBarStopped.pdf "$app/Contents/Resources/"
    renv=".build/renv-{{ renv_version }}.tar.gz"
    if [ ! -f "$renv" ]; then
        if [[ "{{ renv_version }}" =~ ^[0-9a-f]{40}$ ]]; then
            url="https://codeload.github.com/rstudio/renv/tar.gz/{{ renv_version }}"
        else
            url="https://github.com/rstudio/renv/archive/refs/tags/v{{ renv_version }}.tar.gz"
        fi
        curl -fsSL -o "$renv" "$url"
    fi
    cp "$renv" "$app/Contents/Resources/renv.tar.gz"
    # Apple Silicon refuses unsigned code, so sign ad hoc until there is a Developer ID.
    codesign --force --sign - "$app"

# Render the menu bar icons and the app icon in Support/ from Support/logo.svg (needs rsvg-convert)
icons:
    #!/usr/bin/env bash
    set -euo pipefail
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    # Pad the canvas so the outline's stroke is not clipped and both menu bar states stay the same size.
    padded='s/viewBox="0 0 1024 1024"/viewBox="-64 -64 1152 1152"/'
    sed "$padded" Support/logo.svg > "$work/running.svg"
    sed -e "$padded" -e 's/fill="currentColor"/fill="none" stroke="currentColor" stroke-width="64" stroke-linejoin="round"/' Support/logo.svg > "$work/stopped.svg"
    rsvg-convert -f pdf -o Support/MenuBarRunning.pdf "$work/running.svg"
    rsvg-convert -f pdf -o Support/MenuBarStopped.pdf "$work/stopped.svg"
    # Fog rabbit on the dark background tile, sized to the 824-unit tile of Apple's 1024-unit icon grid.
    sed 's|<path fill="currentColor"|<rect x="100" y="100" width="824" height="824" rx="185" fill="#0e080d"/><path transform="translate(232 232) scale(0.546875)" fill="#d7c6f6"|' Support/logo.svg > "$work/app.svg"
    mkdir "$work/AppIcon.iconset"
    for size in 16 32 128 256 512; do
        rsvg-convert -w "$size" -h "$size" -o "$work/AppIcon.iconset/icon_${size}x${size}.png" "$work/app.svg"
        rsvg-convert -w "$((size * 2))" -h "$((size * 2))" -o "$work/AppIcon.iconset/icon_${size}x${size}@2x.png" "$work/app.svg"
    done
    iconutil -c icns -o Support/AppIcon.icns "$work/AppIcon.iconset"

# Package dist/ricochet-<version>.dmg with an Applications shortcut
dmg version="0.0.0": (app version)
    #!/usr/bin/env bash
    set -euo pipefail
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    cp -R dist/Ricochet.app "$stage/"
    ln -s /Applications "$stage/Applications"
    hdiutil create -volname ricochet -srcfolder "$stage" -ov -format UDZO "dist/ricochet-{{ version }}.dmg"
