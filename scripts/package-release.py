#!/usr/bin/env python3
"""Create and verify native TermKey release ZIPs using only the Python standard library.

Header/linkage checks work on any host. --run additionally requires the matching
native OS/CPU and executes only --version and --help in an isolated vault directory.
The Linux test/build workflow must still prove that a MUSL toolchain produced the
static ELF; an ELF header alone does not identify its libc implementation.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat
import struct
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = json.loads((ROOT / "scripts/release-targets.json").read_text(encoding="utf-8"))
STAMP = (1980, 1, 1, 0, 0, 0)
MAX_EXECUTABLE = 256 * 1024 * 1024
MAX_SUPPORTING_FILE = 128 * 1024
MAX_NOTICES = 32 * 1024 * 1024
WINDOWS_SYSTEM_DLLS = {
    "advapi32.dll", "bcrypt.dll", "bcryptprimitives.dll", "cfgmgr32.dll",
    "comctl32.dll", "comdlg32.dll", "crypt32.dll", "cryptbase.dll", "cryptsp.dll",
    "dbghelp.dll", "dwmapi.dll", "gdi32.dll", "imm32.dll", "iphlpapi.dll",
    "kernel32.dll", "kernelbase.dll", "mpr.dll", "msimg32.dll", "ncrypt.dll",
    "netapi32.dll", "normaliz.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll",
    "powrprof.dll", "propsys.dll", "psapi.dll", "rpcrt4.dll", "secur32.dll",
    "setupapi.dll", "shell32.dll", "shlwapi.dll", "user32.dll", "userenv.dll",
    "uxtheme.dll", "version.dll", "windowscodecs.dll", "winhttp.dll", "winmm.dll",
    "winspool.drv", "wintrust.dll", "ws2_32.dll", "wtsapi32.dll",
}


def target_info(target):
    for item in TARGETS:
        if item["target"] == target:
            return item
    raise ValueError(f"unsupported release target: {target}")


def version_value(value=None):
    if value is None:
        manifest = (ROOT / "apps/cli/Cargo.toml").read_text(encoding="utf-8")
        value = re.search(r'^version\s*=\s*"([^"]+)"', manifest, re.M).group(1)
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", value):
        raise ValueError("release version must be a semantic version without a v prefix")
    return value


def unpack(fmt, data, offset):
    size = struct.calcsize(fmt)
    if offset < 0 or offset + size > len(data):
        raise ValueError("truncated executable header")
    return struct.unpack_from(fmt, data, offset)


def validate_macho(data, info):
    magic, cpu, _, kind, count, commands_size, _, _ = unpack("<8I", data, 0)
    expected_cpu = 0x0100000C if info["arch"] == "arm64" else 0x01000007
    if magic != 0xFEEDFACF or cpu != expected_cpu or kind != 2:
        raise ValueError("expected a thin native 64-bit Mach-O executable for this target")
    if count > 4096 or 32 + commands_size > len(data):
        raise ValueError("invalid Mach-O load-command table")
    offset = 32
    minimums = []
    for _ in range(count):
        command, size = unpack("<II", data, offset)
        if size < 8 or offset + size > 32 + commands_size:
            raise ValueError("invalid Mach-O load command")
        if command == 0x32:  # LC_BUILD_VERSION
            operating_system, minimum = unpack("<II", data, offset + 8)
            if operating_system != 1:
                raise ValueError("Mach-O is not a macOS executable")
            minimums.append(minimum)
        elif command == 0x24:  # LC_VERSION_MIN_MACOSX
            minimums.append(unpack("<I", data, offset + 8)[0])
        if command in {0xC, 0x80000018, 0x8000001F, 0x80000023}:  # linked dylibs
            name_offset = unpack("<I", data, offset + 8)[0]
            if name_offset < 24 or name_offset >= size:
                raise ValueError("invalid Mach-O dylib reference")
            name = data[offset + name_offset:offset + size].split(b"\0", 1)[0]
            if not name.startswith((b"/usr/lib/", b"/System/Library/")):
                raise ValueError("Mach-O requires a non-system shared library")
        offset += size
    if offset != 32 + commands_size or minimums != [0x000B0000]:
        raise ValueError("macOS deployment target must remain 11.0")


def validate_elf(data, info):
    if data[:7] != b"\x7fELF\x02\x01\x01":
        raise ValueError("expected a little-endian 64-bit ELF executable")
    kind, machine, version, _, phoff, _, _, header_size, entry_size, count = unpack("<HHIQQQIHHH", data, 16)
    expected_machine = 183 if info["arch"] == "arm64" else 62
    if kind not in (2, 3) or machine != expected_machine or version != 1 or header_size != 64:
        raise ValueError("ELF target architecture or executable type does not match")
    if entry_size != 56 or not 0 < count <= 4096:
        raise ValueError("invalid ELF program-header table")
    load_segments = 0
    for index in range(count):
        segment, _, offset, _, _, size, _, _ = unpack("<IIQQQQQQ", data, phoff + index * entry_size)
        if offset + size > len(data):
            raise ValueError("ELF segment exceeds executable size")
        if segment == 1:
            load_segments += 1
        if segment == 3:
            raise ValueError("Linux release must not require a dynamic interpreter")
        if segment == 2:
            if size % 16:
                raise ValueError("invalid ELF dynamic section")
            for dynamic_offset in range(offset, offset + size, 16):
                tag, _ = unpack("<QQ", data, dynamic_offset)
                if tag == 0:
                    break
                if tag == 1:
                    raise ValueError("Linux release must not require shared libraries")
    if load_segments == 0:
        raise ValueError("ELF executable has no loadable segments")


def pe_imports(data):
    if data[:2] != b"MZ":
        raise ValueError("expected a PE executable")
    pe = unpack("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("invalid PE signature")
    machine, sections, _, _, _, optional_size, characteristics = unpack("<HHIIIHH", data, pe + 4)
    optional = pe + 24
    if machine != 0x8664 or not characteristics & 2 or characteristics & 0x2000:
        raise ValueError("expected a Windows x86_64 executable, not a DLL")
    if optional_size < 240 or unpack("<H", data, optional)[0] != 0x20B or not 0 < sections <= 96:
        raise ValueError("invalid PE32+ header")
    unpack(f"<{optional_size}s", data, optional)
    section_table = optional + optional_size
    if section_table + sections * 40 > len(data) or unpack("<I", data, optional + 108)[0] < 14:
        raise ValueError("invalid PE section/data-directory table")
    image_base = unpack("<Q", data, optional + 24)[0]
    size_of_headers = unpack("<I", data, optional + 60)[0]

    def file_offset(rva, size=1):
        if rva < size_of_headers and rva + size <= min(size_of_headers, len(data)):
            return rva
        for index in range(sections):
            _, virtual_size, virtual_address, raw_size, raw_offset = unpack("<8sIIII", data, section_table + index * 40)
            if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
                displacement = rva - virtual_address
                if displacement + size <= raw_size and raw_offset + displacement + size <= len(data):
                    return raw_offset + displacement
                break
        raise ValueError("invalid PE data-directory address")

    def library_name(rva):
        offset = file_offset(rva)
        end = data.find(b"\0", offset, min(offset + 256, len(data)))
        if end < 0:
            raise ValueError("invalid PE import name")
        try:
            return data[offset:end].decode("ascii").lower()
        except UnicodeDecodeError as error:
            raise ValueError("invalid PE import name") from error

    libraries = set()
    for directory_index, descriptor_size in ((1, 20), (13, 32)):
        rva, size = unpack("<II", data, optional + 112 + directory_index * 8)
        if not rva and not size:
            continue
        if not rva or size < descriptor_size or size > len(data):
            raise ValueError("invalid PE import directory")
        terminated = False
        for displacement in range(0, size - descriptor_size + 1, descriptor_size):
            descriptor = unpack(f"<{descriptor_size // 4}I", data, file_offset(rva + displacement, descriptor_size))
            if not any(descriptor):
                terminated = True
                break
            if directory_index == 1:
                name_rva = descriptor[3]
            else:
                name_rva = descriptor[1]
                if descriptor[0] & 1 == 0:
                    name_rva -= image_base
            libraries.add(library_name(name_rva))
        if not terminated:
            raise ValueError("unterminated PE import directory")
    return libraries


def validate_pe(data, _info):
    for library in pe_imports(data):
        api_set = library.startswith(("api-ms-win-", "ext-ms-win-")) and "-crt-" not in library
        if library not in WINDOWS_SYSTEM_DLLS and not api_set:
            raise ValueError(f"Windows release requires an unexpected runtime library: {library}")


def validate_binary(data, info):
    if not data or len(data) > MAX_EXECUTABLE:
        raise ValueError("release executable is empty or exceeds the package size limit")
    {"macos": validate_macho, "linux": validate_elf, "windows": validate_pe}[info["os"]](data, info)


def validate_notices(contents):
    expected = hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest().encode("ascii")
    matches = re.findall(rb"^Cargo.lock SHA-256: ([0-9a-f]{64})$", contents, re.M)
    if len(contents) > MAX_NOTICES or matches != [expected]:
        raise ValueError("third-party notices are missing or stale for Cargo.lock; regenerate them")


def installation_text(info, version):
    common = f"""TermKey {version} — {info['target']}

Verify this ZIP against SHA256SUMS from the same GitHub Release before use.
The archive contains a native executable; no Rust installation is needed.
LICENSE covers TermKey; THIRD-PARTY-LICENSES.txt contains dependency notices.
Do not put your vault inside the executable's installation directory.

"""
    if info["os"] == "windows":
        install = """Extract the ZIP to a user-writable folder, for example
%LOCALAPPDATA%\\Programs\\TermKey. Open Windows Terminal / PowerShell there:
  .\\termkey.exe --help
  .\\termkey.exe
Add that folder (not the executable file) to your user PATH in Windows Settings.
Open a new terminal and run termkey. Administrator rights are not required.
No MSI, WinGet, or separate Visual C++ runtime installation is intended; release
verification checks DLL imports and still requires a clean-machine smoke test.
A downloaded executable may show SmartScreen/reputation warnings. Verify its
origin and checksum; do not disable system security protections.
Your vault and config default to %USERPROFILE%\\.termkey.
"""
    else:
        install = """Extract the ZIP, then install in a user-writable directory:
  mkdir -p "$HOME/.local/bin"
  chmod 755 termkey
  cp termkey "$HOME/.local/bin/termkey"
Add this line to your shell startup file if that directory is not already on PATH:
  export PATH="$HOME/.local/bin:$PATH"
Open a new terminal, then run termkey --help and termkey.
Homebrew on supported macOS/Linux hosts can use the same release executable:
  brew install ryanonmars/termkey/termkey
"""
        if info["os"] == "macos":
            install += """macOS deployment target: 11.0. Production binaries must be Developer ID signed
and notarized. Keep Gatekeeper enabled; verify downloaded artifacts on a clean
Mac. The Apple Silicon DMG remains available as an alternative installer.
Your vault and configuration remain in ~/.termkey.
"""
        else:
            install += """Linux ZIPs are built for MUSL; release verification rejects shared-library and
dynamic-interpreter requirements. Native runtime verification is still required.
New vaults use $XDG_DATA_HOME/termkey (default ~/.local/share/termkey), and config
uses $XDG_CONFIG_HOME/termkey (default ~/.config/termkey). Existing ~/.termkey
installations remain in place. X11/Wayland clipboard access depends on the
session and compositor. Headless/SSH sessions can still use other vault features.
"""
    return common + install + """
An absolute TERMKEY_VAULT_DIR overrides the data/config location.

Updates and removal
-------------------
termkey update uses Homebrew for Homebrew installs and otherwise gives the
matching release download instructions. For a portable upgrade, close TermKey
and replace only the executable; preserve your vault and configuration.
termkey uninstall preserves vaults, settings, and exported backups. On Windows,
close the application before removing the portable executable as instructed.
To remove vault data and settings first, run termkey reset and confirm DELETE.
The Settings screen also offers reset. Exported backups are not erased.

Backups and recovery
--------------------
Export with termkey export <directory>; import a .termkey backup with
termkey import <file> using its backup password. Do not rename it to vault.ck.
Secondary passwords remain required for their protected entries.
Raw vault.ck files can be copied while all TermKey instances are closed; preserve
matching recovery config and fix local absolute paths when changing operating
systems. Reapply private destination permissions. Backup import is simpler for
a new installation; set up its local recovery phrase afterward.
Changing the master password or recovering a vault rotates the encryption key
and invalidates the old recovery phrase. Set up a new phrase in Settings.

Security limits
---------------
Keep vault/config/export permissions private. Windows ACL enforcement requires
a filesystem supporting ACLs; permission failures must not be ignored. OS paging,
crash dumps, clipboard history/managers, screenshots and other processes can
retain information. Clipboard clearing is best effort and cannot erase copies
retained elsewhere. No cloud account or telemetry is used by TermKey. GitHub
update metadata requests contain the app version, not vault secrets.

Full instructions: https://github.com/ryanonmars/termkey#readme
"""


def package(info, binary, output_directory, version):
    binary = Path(binary)
    if not binary.is_file() or binary.stat().st_size > MAX_EXECUTABLE:
        raise ValueError("release executable is missing or exceeds the package size limit")
    with binary.open("rb") as source:
        data = source.read(MAX_EXECUTABLE + 1)
    validate_binary(data, info)
    license_text = (ROOT / "LICENSE").read_bytes()
    if not license_text.strip():
        raise ValueError("release license is missing")
    notices = (ROOT / "apps/cli/packaging/THIRD-PARTY-LICENSES.txt").read_bytes()
    validate_notices(notices)
    members = {
        info["executable"]: data,
        "THIRD-PARTY-LICENSES.txt": notices,
        "LICENSE": license_text,
        "README.txt": installation_text(info, version).encode("utf-8"),
    }
    output_directory = Path(output_directory)
    output_directory.mkdir(parents=True, exist_ok=True)
    destination = output_directory / info["archive"]
    with tempfile.NamedTemporaryFile(dir=output_directory, prefix=".termkey-package-", delete=False) as temp:
        temporary_path = Path(temp.name)
    try:
        with zipfile.ZipFile(temporary_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name in sorted(members):
                entry = zipfile.ZipInfo(name, STAMP)
                entry.create_system = 3
                mode = 0o755 if name == info["executable"] else 0o644
                entry.external_attr = (stat.S_IFREG | mode) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(entry, members[name], compresslevel=9)
        verify(info, temporary_path)
        os.replace(temporary_path, destination)
    finally:
        temporary_path.unlink(missing_ok=True)
    return destination


def verify_native(info, executable, version, require_signature=False, team_id=None):
    host_os = {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}.get(platform.system())
    host_arch = {"aarch64": "arm64", "arm64": "arm64", "x86_64": "x64", "AMD64": "x64"}.get(platform.machine())
    if (host_os, host_arch) != (info["os"], info["arch"]):
        raise ValueError("native smoke verification requires the matching OS and CPU")
    if require_signature:
        if host_os != "macos" or not team_id or not re.fullmatch(r"[A-Z0-9]{10}", team_id):
            raise ValueError("macOS signature verification requires a valid --team-id")
        subprocess.run(["codesign", "--verify", "--strict", "--verbose=2", str(executable)], check=True, capture_output=True)
        details = subprocess.run(["codesign", "-dv", "--verbose=4", str(executable)], check=True, capture_output=True, text=True).stderr
        if not all(re.search(pattern, details, re.M) for pattern in (
            rf"^TeamIdentifier={re.escape(team_id)}$", rf"^Authority=Developer ID Application: .* \({re.escape(team_id)}\)$",
            r"^Timestamp=.+$", r"^Runtime Version=.+$",
        )):
            raise ValueError("macOS executable does not meet Developer ID/runtime/timestamp requirements")
    with tempfile.TemporaryDirectory(prefix="termkey-release-smoke-") as directory:
        environment = dict(os.environ, TERMKEY_VAULT_DIR=directory, TERMKEY_SPLASH_MS="0", TERMKEY_GRAPHICS="off")
        for argument, expected in (("--version", f"termkey {version}"), ("--help", "Encrypted storage for")):
            result = subprocess.run([str(executable), argument], env=environment, capture_output=True, text=True, timeout=30, check=True)
            if (argument == "--version" and result.stdout.strip() != expected) or expected not in result.stdout:
                raise ValueError(f"native {argument} smoke test returned unexpected output")
        if any(Path(directory).iterdir()):
            raise ValueError("help/version smoke tests unexpectedly created vault data")


def verify(info, archive_path, run=False, require_signature=False, team_id=None):
    expected = {info["executable"], "LICENSE", "README.txt", "THIRD-PARTY-LICENSES.txt"}
    with zipfile.ZipFile(archive_path) as archive:
        members = archive.infolist()
        if len(members) != len(expected) or {entry.filename for entry in members} != expected:
            raise ValueError("archive must contain only the native executable, LICENSE, THIRD-PARTY-LICENSES.txt, and README.txt at root")
        for entry in members:
            maximum = MAX_EXECUTABLE if entry.filename == info["executable"] else MAX_NOTICES if entry.filename == "THIRD-PARTY-LICENSES.txt" else MAX_SUPPORTING_FILE
            mode = entry.external_attr >> 16
            expected_mode = 0o755 if entry.filename == info["executable"] else 0o644
            if entry.flag_bits & 1 or entry.file_size <= 0 or entry.file_size > maximum:
                raise ValueError("encrypted, empty, or oversized archive member")
            if entry.create_system != 3 or not stat.S_ISREG(mode) or stat.S_IMODE(mode) != expected_mode:
                raise ValueError("archive member has incorrect type or permissions")
        data = archive.read(info["executable"])
        validate_binary(data, info)
        if b"MIT License" not in archive.read("LICENSE"):
            raise ValueError("archive license is missing or incorrect")
        validate_notices(archive.read("THIRD-PARTY-LICENSES.txt"))
        instructions = archive.read("README.txt").decode("utf-8")
        header = re.match(r"TermKey (\d+\.\d+\.\d+) — " + re.escape(info["target"]) + r"\n", instructions)
        if header is None or "termkey reset" not in instructions:
            raise ValueError("archive installation instructions do not match target")
        if run or require_signature:
            with tempfile.TemporaryDirectory(prefix="termkey-release-extract-") as directory:
                executable = Path(directory) / info["executable"]
                executable.write_bytes(data)
                executable.chmod(0o755)
                verify_native(info, executable, header.group(1), require_signature, team_id)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    package_parser = subparsers.add_parser("package")
    package_parser.add_argument("--target", required=True)
    package_parser.add_argument("--binary", required=True, type=Path)
    package_parser.add_argument("--output-dir", required=True, type=Path)
    package_parser.add_argument("--version")
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--target", required=True)
    verify_parser.add_argument("--archive", required=True, type=Path)
    verify_parser.add_argument("--run", action="store_true")
    verify_parser.add_argument("--require-signature", action="store_true")
    verify_parser.add_argument("--team-id")
    arguments = parser.parse_args()
    info = target_info(arguments.target)
    if arguments.command == "package":
        destination = package(info, arguments.binary, arguments.output_dir, version_value(arguments.version))
        print(destination)
        print(f"SHA256 {hashlib.sha256(destination.read_bytes()).hexdigest()}")
    else:
        verify(info, arguments.archive, arguments.run, arguments.require_signature, arguments.team_id)
        print(f"Verified {info['target']}: {arguments.archive}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        print(f"package-release: {error}", file=sys.stderr)
        sys.exit(1)
