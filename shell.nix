{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    alsaLib
    cmake
    freetype
    expat
  ];
}
