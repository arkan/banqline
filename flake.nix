{
  description = "Banqline — terminal-first personal banking CLI and TUI powered by Enable Banking";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = system: import nixpkgs { inherit system; };
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          banqline = pkgs.rustPlatform.buildRustPackage {
            pname = "banqline";
            version = "0.1.0";
            src = self;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = [ pkgs.pkg-config ];

            buildInputs = nixpkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];

            cargoBuildFlags = [ "--bin" "banqline" ];
            cargoTestFlags = [ "--bin" "banqline" "--test" "cli_ux" ];

            doInstallCheck = true;
            installCheckPhase = ''
              runHook preInstallCheck
              $out/bin/banqline --help >/dev/null
              $out/bin/banqline version >/dev/null
              runHook postInstallCheck
            '';

            meta = {
              description = "Terminal-first personal banking CLI and TUI powered by Enable Banking";
              homepage = "https://github.com/arkan/banqline";
              license = pkgs.lib.licenses.mit;
              mainProgram = "banqline";
              platforms = supportedSystems;
            };
          };

          default = self.packages.${system}.banqline;
        });

      apps = forAllSystems (system: {
        banqline = {
          type = "app";
          program = "${self.packages.${system}.banqline}/bin/banqline";
        };
        default = self.apps.${system}.banqline;
      });

      checks = forAllSystems (system: {
        default = self.packages.${system}.banqline;
      });

      formatter = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        pkgs.nixpkgs-fmt);

      devShells = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.rustfmt
              pkgs.clippy
              pkgs.pkg-config
            ] ++ nixpkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];
          };
        });
    };
}
