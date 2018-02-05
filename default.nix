with import <nixpkgs> {}; {
  env = stdenv.mkDerivation {
    name = "env";
    buildInputs = [
      stdenv
      llvmPackages.clang-unwrapped.lib
      libcap
    ];

    LIBCLANG_PATH = "${llvmPackages.clang-unwrapped.lib}/lib";
    CPATH = "${stdenv.cc.libc.dev}/include";
  };
}
