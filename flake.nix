{
  description = "Japanese popup dictionary as a native window";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url  = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    fenix,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        lib = pkgs.lib;
        fenix-pkgs = fenix.packages.${system};
        fenix-channel = fenix-pkgs.stable;
        fenix-toolchain = fenix-channel.toolchain;
        craneLib = (crane.mkLib pkgs).overrideScope (final: prev: {
          cargo = fenix-channel.cargo;
          rustc = fenix-channel.rustc;
        });
        runtimeInputs = with pkgs; [
          wayland
          libxkbcommon
          openssl
          vulkan-loader
          libGL
        ];
        dependencyPrograms = with pkgs; [
          tesseract
        ];
        buildtimeInputs = with pkgs; [
            fenix-pkgs.rust-analyzer
            fenix-toolchain
	    pkg-config
        ] ++ runtimeInputs;
      in
      {
        packages =
        let
          unfilteredRoot = ./.;
          src = lib.fileset.toSource {
            root = unfilteredRoot;
            fileset = lib.fileset.unions [
              # Default files from crane (Rust and cargo files)
              (craneLib.fileset.commonCargoSources unfilteredRoot)
              # Folder for images and fonts
              (lib.fileset.maybeMissing ./src/assets)
            ];
          };
        in rec {
          unwrapped = with pkgs; craneLib.buildPackage {
            nativeBuildInputs = buildtimeInputs;
	    buildInputs = buildtimeInputs;

            # Needed for the unit tests.
            LD_LIBRARY_PATH = lib.makeLibraryPath runtimeInputs;
            
            cargoExtraArgs = "--all-features";

            inherit src;
            strictDeps = true;
          };

          default = pkgs.runCommandLocal "popup_dictionary" {
            nativeBuildInputs = [
              pkgs.makeWrapper
            ];
          }
          ''
            mkdir -p $out/bin
            ln -s ${unwrapped}/bin/popup_dictionary $out/bin
            wrapProgram $out/bin/popup_dictionary \
              --set LD_LIBRARY_PATH ${lib.makeLibraryPath runtimeInputs} \
              --set PATH ${lib.makeBinPath dependencyPrograms}
          '';
        };

        devShells.default = with pkgs; pkgs.mkShell {
          buildInputs = [
	    bashInteractive
          ] ++ buildtimeInputs ++ dependencyPrograms;

          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeInputs;

	  shellHook = ''
            export SHELL=${pkgs.bashInteractive}/bin/bash
          '';
        };

      }
    );
}
