{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [ rustc cargo gcc ];
  buildInputs = with pkgs; [ rustfmt clippy bridge_utils killall cargo-watch cargo-license ];

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}

#
# Note for nixos-release:
#
# - Package path: pkgs/tools/networking/wg-netmanager
# - Service path:
#
# 1. Update default.nix, update version and replace hashes with 00000.. and 1111.
# 2. Run `nix-build ~/src/nixpkgs -A wg-netmanager --show-trace`
# 3. Replace fetchFromGitHub-hash
# 4. Run `nix-build ~/src/nixpkgs -A wg-netmanager --show-trace`
# 5. Replace cargoSha256
# 6. Run `nix-build ~/src/nixpkgs -A wg-netmanager --show-trace`
#    => should work now
# 7. cd to ~/.dotfiles and run `./apply-local-system.sh switch'

