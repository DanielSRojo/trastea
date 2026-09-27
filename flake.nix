{
  description = "Trastea, a keyboard-driven guitar trainer for the desktop";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # nixpkgs carries one rustc and ignores rust-toolchain.toml, which is the same gap rustup
    # closes everywhere else — this is what makes the pin real on NixOS.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
    in
    {
      devShells = forEachSystem (
        pkgs:
        let
          inherit (pkgs) lib stdenv;
          # winit and wgpu dlopen these at runtime rather than linking them, so the build
          # succeeds without them and the window fails to open instead.
          runtimeLibs = lib.optionals stdenv.hostPlatform.isLinux (
            with pkgs;
            [
              libxkbcommon
              vulkan-loader
              libGL
              wayland
              libx11
              libxcursor
              libxi
              libxrandr
            ]
          );
        in
        {
          default = pkgs.mkShell {
            packages = [
              (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
              pkgs.just
              pkgs.pkg-config
            ]
            # cpal's Linux backend, found by alsa-sys through pkg-config at build time
            ++ lib.optionals stdenv.hostPlatform.isLinux [ pkgs.alsa-lib ];

            LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
          };
        }
      );
    };
}
