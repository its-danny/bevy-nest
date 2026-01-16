{pkgs, ...}: {
  dotenv.enable = true;

  packages = [pkgs.blightmud pkgs.cocogitto pkgs.git pkgs.just];

  languages = {
    rust = {
      enable = true;
      channel = "stable";
    };
  };

  git-hooks.hooks = {
    # Nix

    alejandra.enable = true;

    # Rust

    cargo-check.enable = true;
    rustfmt.enable = true;

    clippy = {
      enable = true;
      entry = "cargo clippy -- -D warnings";
      pass_filenames = false;
      stages = ["pre-commit"];
    };

    cargo-test = {
      enable = true;
      entry = "cargo test";
      pass_filenames = false;
      stages = ["pre-push"];
    };
  };
}
