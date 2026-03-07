{
description = "CLI tool for WLmouse gaming mice. Configure DPI, polling rate, LOD, and other settings without the browser-based web app.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          wl-mouse = pkgs.rustPlatform.buildRustPackage {
            pname = "wl-mouse";
            version = "0.1.1";

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.udev ];

            meta = with nixpkgs.lib; {
              description = "CLI tool for WLmouse gaming mice. Configure DPI, polling rate, LOD, and other settings without the browser-based web app.";
              homepage = "https://heliopolis.live/creations/wl-mouse";
              license = licenses.agpl3Plus;
              platforms = platforms.linux;
              maintainers = [ ];
            };
          };
          default = self.packages.${system}.wl-mouse;
        });

      devShells = forAllSystems (system:
        let
          pkgs = pkgsFor system;
          
          rustToolchain = pkgs.rust-bin.stable."1.92.0".default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
          };
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.wl-mouse ];
            nativeBuildInputs = [ rustToolchain ];

            shellHook = ''
              export PKG_CONFIG_PATH="${pkgs.udev.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            '';
          };
        });
    };
}
