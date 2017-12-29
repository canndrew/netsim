with import <nixpkgs> {}; {
  env = stdenv.mkDerivation {
    name = "env";
    buildInputs = [
      stdenv
      llvmPackages.clang-unwrapped
    ];

    LIBCLANG_PATH = "${llvmPackages.clang-unwrapped}/lib";
    CPATH = "${stdenv.cc.libc.dev}/include";
  };
}
