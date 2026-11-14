{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "piano-roll-prototype";

  nativeBuildInputs = [
    pkgs.pkg-config
  ];

  buildInputs = with pkgs; [
    alsa-lib
    fontconfig
    freetype
    expat
    libxkbcommon
    wayland
    wayland-protocols
    vulkan-loader
    vulkan-headers
    xorg.libX11
    xorg.libxcb
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    dbus
    zlib
    openssl
  ];

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
    pkgs.alsa-lib
    pkgs.fontconfig
    pkgs.freetype
    pkgs.libxkbcommon
    pkgs.wayland
    pkgs.vulkan-loader
    pkgs.xorg.libX11
    pkgs.xorg.libxcb
    pkgs.xorg.libXcursor
    pkgs.xorg.libXrandr
    pkgs.xorg.libXi
    pkgs.dbus
  ];
}
