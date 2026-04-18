{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
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
      sdl3
    ];

  }
