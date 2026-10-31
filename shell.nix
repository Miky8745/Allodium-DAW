{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
  ];

  buildInputs = with pkgs; [
    glib
    gtk3
    webkitgtk_4_1
    libsoup_3
    librsvg
    openssl
    libayatana-appindicator
    libpulseaudio
    alsa-lib
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
    gst_all_1.gst-plugins-bad
    gst_all_1.gst-plugins-ugly
    gst_all_1.gst-libav
  ];

  shellHook = ''
    export GST_PLUGIN_SYSTEM_PATH_1_0="${with pkgs; lib.makeSearchPath "lib/gstreamer-1.0" [
      gst_all_1.gstreamer.out
      gst_all_1.gst-plugins-base.out
      gst_all_1.gst-plugins-good.out
      gst_all_1.gst-plugins-bad.out
      gst_all_1.gst-plugins-ugly.out
      gst_all_1.gst-libav.out
    ]}"
  '';
}
