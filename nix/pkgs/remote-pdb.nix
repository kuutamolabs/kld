{ buildPythonPackage, fetchFromGitHub, lib }:

# can be dropped soon after the next nixpkgs update: https://github.com/NixOS/nixpkgs/pull/212976
buildPythonPackage rec {
  pname = "remote-pdb";
  version = "2.1.0";
  src = fetchFromGitHub {
    owner = "ionelmc";
    repo = "python-remote-pdb";
    rev = "v${version}";
    sha256 = "sha256-/7RysJOJigU4coC6d/Ob2lrtw8u8nLZI8wBk4oEEY3g=";
  };
  meta = with lib; {
    description = "Remote vanilla PDB (over TCP sockets).";
    homepage = "https://github.com/ionelmc/python-remote-pdb";
    license = licenses.bsd2;
    maintainers = with maintainers; [ mic92 ];
    platforms = platforms.all;
  };
}
#{
#  #PYTHONBREAKPOINT=remote_pdb.set_trace REMOTE_PDB_HOST=127.0.0.1 REMOTE_PDB_PORT=4444
#}
