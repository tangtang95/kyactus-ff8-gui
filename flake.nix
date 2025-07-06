{
  description = "Flake to develop with winit apps";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    pkgs = import nixpkgs {system = "x86_64-linux";};
  in {
    devShells.x86_64-linux.default = pkgs.mkShell rec {
      buildInputs = with pkgs; [
        libxkbcommon
        libGL

        # WINIT_UNIX_BACKEND=wayland
        wayland

        # WINIT_UNIX_BACKEND=x11
        xorg.libXcursor
        xorg.libXrandr
        xorg.libXi
        xorg.libX11
      ];
      LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";
    };
  };
}
