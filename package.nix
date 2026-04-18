{
  lib,
  pkgs,
  rustPlatform,
  fetchFromGitHub,
}:

rustPlatform.buildRustPackage rec {
  pname = "lite-anvil";
  version = "2.11.3";

  src = fetchFromGitHub {
    owner = "danpozmanter";
    repo = "lite-anvil";
    rev = "v${version}";
    hash = "sha256-U992bztpXenJugAlz7J0hYS5VYzp9cFfRT76DKwcF4s=";
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    rustc
    cargo
    cmake
  ];

  buildInputs = with pkgs; [
    freetype
    pcre2
    xorg.libX11
    xorg.libXcursor
    xorg.libXext
    xorg.libXi
    xorg.libXrandr
    xorg.libxcb
    xorg.libXScrnSaver
    xorg.libXfixes
    libxtst
    brotli
    zlib
    # libxkbcommon
    # wayland
    # wayland-scanner
    # pkg-config
    sdl3
  ];

  # cargoPatches = [ ./use-distro-sdl3.patch ];

  installPhase = ''
    install -D -T -m 755 "./target/x86_64-unknown-linux-gnu/release/lite-anvil" "$out/bin/lite-anvil"
    install -D -T -m 755 "./target/x86_64-unknown-linux-gnu/release/nano-anvil" "$out/bin/nano-anvil"
    install -D -T -m 644 "./resources/linux/com.lite_anvil.LiteAnvil.desktop" "$out/share/applications/lite-anvil.desktop"
    install -D -T -m 644 "./resources/linux/com.nano_anvil.NanoAnvil.desktop" "$out/share/applications/nano-anvil.desktop"
    install -D -T -m 644 "./resources/icons/lite-anvil.png" "$out/share/icons/hicolor/256x256/apps/lite-anvil.png"
    install -D -T -m 644 "./resources/icons/nano-anvil.png" "$out/share/icons/hicolor/256x256/apps/nano-anvil.png"
    mkdir -p "$out/share/lite-anvil/data/"
    mkdir -p "$out/share/nano-anvil/data/"
    cp -R "./data/assets" "$out/share/lite-anvil/data/"
    cp -R "./data/fonts" "$out/share/lite-anvil/data/"
    cp -R "./data-nano/assets" "$out/share/nano-anvil/data/"
    cp -R "./data-nano/fonts" "$out/share/nano-anvil/data/"
  '';

  cargoHash = "sha256-FiDOAvtV1QLpooU7XTYP3JvFKyFgTDqaxhlSkpQg/r4=";

  meta = {
    description = "A code editor in Rust";
    homepage = "https://github.com/danpozmanter/lite-anvil";
    changelog = "https://github.com/danpozmanter/lite-anvil/blob/${src.rev}/changelog.md";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ ];
    mainProgram = "lite-anvil";
  };
}
