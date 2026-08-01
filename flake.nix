{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
      };
    in
    {
      devShells.${system}.default =
        let
          dlopenLibraries = with pkgs; [
            libxkbcommon

            # GPU backend
            vulkan-loader
            # libGL

            # Window system
            wayland
            # xorg.libX11
            # xorg.libXcursor
            # xorg.libXi
          ];
        in
        pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            clippy
            rustfmt
            pkg-config
            alsa-lib # for rodio
            dioxus-cli
            lld
          ];

          RUSTFLAGS = "-C link-arg=-Wl,-rpath,${nixpkgs.lib.makeLibraryPath dlopenLibraries}";
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
    };
}
