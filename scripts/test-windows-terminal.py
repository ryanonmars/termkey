#!/usr/bin/env python3
"""Exercise the real Windows console through ConPTY with synthetic vault data.

Run only on a disposable Windows validation runner with
TERMKEY_TEST_WINDOWS_CONSOLE=1. Python is a CI tool, not a TermKey runtime dependency.
No captured terminal output or synthetic password is printed, even on failure.
API lifecycle: https://learn.microsoft.com/windows/console/creating-a-pseudoconsole-session
"""
import argparse
import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import time

CSI = re.compile(rb"\x1b\[[0-?]*[ -/]*[@-~]")
OSC = re.compile(rb"\x1b\].*?(?:\x07|\x1b\\)", re.S)
ECHO = 0x0004
LINE_INPUT = 0x0002
TIMEOUT_MS = 30000


def plain_text(data):
    return CSI.sub(b"", OSC.sub(b"", data)).decode("utf-8", "replace")


class COORD(ctypes.Structure):
    _fields_ = [("X", ctypes.c_short), ("Y", ctypes.c_short)]


class STARTUPINFO(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD), ("lpReserved", wintypes.LPWSTR),
        ("lpDesktop", wintypes.LPWSTR), ("lpTitle", wintypes.LPWSTR),
        ("dwX", wintypes.DWORD), ("dwY", wintypes.DWORD),
        ("dwXSize", wintypes.DWORD), ("dwYSize", wintypes.DWORD),
        ("dwXCountChars", wintypes.DWORD), ("dwYCountChars", wintypes.DWORD),
        ("dwFillAttribute", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
        ("wShowWindow", wintypes.WORD), ("cbReserved2", wintypes.WORD),
        ("lpReserved2", ctypes.POINTER(ctypes.c_byte)),
        ("hStdInput", wintypes.HANDLE), ("hStdOutput", wintypes.HANDLE),
        ("hStdError", wintypes.HANDLE),
    ]


class STARTUPINFOEX(ctypes.Structure):
    _fields_ = [("StartupInfo", STARTUPINFO), ("lpAttributeList", ctypes.c_void_p)]


class PROCESS_INFORMATION(ctypes.Structure):
    _fields_ = [("hProcess", wintypes.HANDLE), ("hThread", wintypes.HANDLE),
                ("dwProcessId", wintypes.DWORD), ("dwThreadId", wintypes.DWORD)]


class Console:
    def __init__(self, directory):
        self.directory = str(directory)
        # Keep the working directory separate: Windows holds each process CWD
        # open without delete sharing, while the vault enforces its own ACL.
        self.vault_directory = str(Path(directory) / "vault")
        self.api = ctypes.WinDLL("kernel32", use_last_error=True)
        self._declare_api()
        self.handles = []
        self.processes = []
        self.hpc = wintypes.HANDLE()
        self.output = bytearray()
        self.condition = threading.Condition()
        self.closing = False
        self.reader_error = False
        self.probe_count = 0
        self.input_read, self.input_write = self._pipe()
        self.output_read, self.output_write = self._pipe()
        result = self.api.CreatePseudoConsole(
            COORD(120, 35), self.input_read, self.output_write, 0,
            ctypes.byref(self.hpc))
        if result < 0:
            self.close()
            raise RuntimeError("CreatePseudoConsole failed")
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        # Keep this private console alive between applications and mode probes.
        self.spawn([sys.executable, "-c", "import time;time.sleep(3600)"])

    def _declare_api(self):
        declarations = {
            "CreatePipe": ([ctypes.POINTER(wintypes.HANDLE), ctypes.POINTER(wintypes.HANDLE), ctypes.c_void_p, wintypes.DWORD], wintypes.BOOL),
            "CreatePseudoConsole": ([COORD, wintypes.HANDLE, wintypes.HANDLE, wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE)], ctypes.c_long),
            "ClosePseudoConsole": ([wintypes.HANDLE], None),
            "CloseHandle": ([wintypes.HANDLE], wintypes.BOOL),
            "ReadFile": ([wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p], wintypes.BOOL),
            "WriteFile": ([wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p], wintypes.BOOL),
            "InitializeProcThreadAttributeList": ([ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, ctypes.POINTER(ctypes.c_size_t)], wintypes.BOOL),
            "UpdateProcThreadAttribute": ([ctypes.c_void_p, wintypes.DWORD, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p], wintypes.BOOL),
            "DeleteProcThreadAttributeList": ([ctypes.c_void_p], None),
            "CreateProcessW": ([wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, wintypes.BOOL, wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR, ctypes.c_void_p, ctypes.POINTER(PROCESS_INFORMATION)], wintypes.BOOL),
            "WaitForSingleObject": ([wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            "GetExitCodeProcess": ([wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
            "TerminateProcess": ([wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
        }
        for name, (arguments, result) in declarations.items():
            function = getattr(self.api, name)
            function.argtypes = arguments
            function.restype = result

    def _pipe(self):
        read, write = wintypes.HANDLE(), wintypes.HANDLE()
        if not self.api.CreatePipe(ctypes.byref(read), ctypes.byref(write), None, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        self.handles.extend([read, write])
        return read, write

    def _read(self):
        buffer = ctypes.create_string_buffer(65536)
        count = wintypes.DWORD()
        while True:
            if not self.api.ReadFile(self.output_read, buffer, len(buffer), ctypes.byref(count), None):
                self.reader_error = not self.closing and ctypes.get_last_error() not in (109, 232)
                return
            if not count.value:
                return
            chunk = buffer.raw[:count.value]
            with self.condition:
                self.output.extend(chunk)
                self.condition.notify_all()
            # Answer cursor-position requests without touching a user's terminal.
            if not self.closing and b"\x1b[6n" in chunk:
                self.write(b"\x1b[1;1R")

    def write(self, data):
        buffer = ctypes.create_string_buffer(data)
        count = wintypes.DWORD()
        if not self.api.WriteFile(self.input_write, buffer, len(data), ctypes.byref(count), None) or count.value != len(data):
            raise RuntimeError("could not write isolated console input")

    def snapshot(self):
        with self.condition:
            return bytes(self.output)

    def wait_text(self, marker, offset=0):
        deadline = time.monotonic() + 30
        with self.condition:
            while True:
                text = plain_text(bytes(self.output[offset:]))
                if marker in text:
                    return text
                if self.reader_error or time.monotonic() >= deadline:
                    raise RuntimeError("isolated console did not reach expected validation stage")
                self.condition.wait(0.05)

    def spawn(self, arguments):
        size = ctypes.c_size_t()
        self.api.InitializeProcThreadAttributeList(None, 1, 0, ctypes.byref(size))
        attributes = ctypes.create_string_buffer(size.value)
        if not self.api.InitializeProcThreadAttributeList(attributes, 1, 0, ctypes.byref(size)):
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            if not self.api.UpdateProcThreadAttribute(attributes, 0, 0x00020016, self.hpc, ctypes.sizeof(self.hpc), None, None):
                raise ctypes.WinError(ctypes.get_last_error())
            startup = STARTUPINFOEX()
            startup.StartupInfo.cb = ctypes.sizeof(startup)
            startup.lpAttributeList = ctypes.cast(attributes, ctypes.c_void_p)
            command = ctypes.create_unicode_buffer(subprocess.list2cmdline([str(arg) for arg in arguments]))
            environment = dict(os.environ, TERMKEY_VAULT_DIR=self.vault_directory,
                               TERMKEY_GRAPHICS="off", TERMKEY_SPLASH_MS="0")
            block = "\0".join(key + "=" + value for key, value in sorted(environment.items(), key=lambda pair: pair[0].upper())) + "\0\0"
            environment_buffer = ctypes.create_unicode_buffer(block)
            process = PROCESS_INFORMATION()
            if not self.api.CreateProcessW(None, command, None, None, False, 0x00080000 | 0x00000400,
                                           environment_buffer, self.directory, ctypes.byref(startup), ctypes.byref(process)):
                raise ctypes.WinError(ctypes.get_last_error())
            self.api.CloseHandle(process.hThread)
            # The pseudoconsole owns references after attaching the first client.
            for handle in (self.input_read, self.output_write):
                if handle in self.handles:
                    self.api.CloseHandle(handle)
                    self.handles.remove(handle)
            self.processes.append(process.hProcess)
            return process.hProcess
        finally:
            self.api.DeleteProcThreadAttributeList(attributes)

    def wait_exit(self, process):
        if self.api.WaitForSingleObject(process, TIMEOUT_MS) != 0:
            raise RuntimeError("isolated console process did not exit")
        code = wintypes.DWORD()
        if not self.api.GetExitCodeProcess(process, ctypes.byref(code)) or code.value != 0:
            raise RuntimeError("isolated console process failed")

    def mode(self):
        # A second client shares the actual console input buffer. File output
        # avoids racing the TUI's repaint or exposing captured console text.
        self.probe_count += 1
        output = Path(self.directory, "console-mode-" + str(self.probe_count) + ".txt")
        probe = (
            "import ctypes,pathlib,sys;from ctypes import wintypes;"
            "k=ctypes.WinDLL('kernel32',use_last_error=True);"
            "k.GetStdHandle.argtypes=[wintypes.DWORD];k.GetStdHandle.restype=wintypes.HANDLE;"
            "k.GetConsoleMode.argtypes=[wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)];"
            "m=wintypes.DWORD();ok=k.GetConsoleMode(k.GetStdHandle(0xfffffff6),ctypes.byref(m));"
            "assert ok;pathlib.Path(sys.argv[1]).write_text(str(m.value),encoding='ascii')"
        )
        process = self.spawn([sys.executable, "-c", probe, output])
        self.wait_exit(process)
        try:
            return int(output.read_text(encoding="ascii"))
        finally:
            output.unlink(missing_ok=True)

    def wait_hidden(self):
        deadline = time.monotonic() + 5
        while self.mode() & ECHO:
            if time.monotonic() >= deadline:
                raise RuntimeError("password console echo was not disabled")
            time.sleep(0.01)

    def close(self):
        self.closing = True
        for process in self.processes:
            if self.api.WaitForSingleObject(process, 0) != 0:
                self.api.TerminateProcess(process, 1)
                self.api.WaitForSingleObject(process, 5000)
            self.api.CloseHandle(process)
        # Release original pipe ends while the reader continues draining ConPTY.
        for handle in (getattr(self, "input_read", None), getattr(self, "output_write", None)):
            if handle in self.handles:
                self.api.CloseHandle(handle)
                self.handles.remove(handle)
        if self.hpc:
            self.api.ClosePseudoConsole(self.hpc)
            self.hpc = wintypes.HANDLE()
        if hasattr(self, "reader"):
            self.reader.join(timeout=5)
        for handle in self.handles:
            self.api.CloseHandle(handle)
        self.handles.clear()



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    arguments = parser.parse_args()
    if os.name != "nt" or os.environ.get("TERMKEY_TEST_WINDOWS_CONSOLE") != "1":
        raise RuntimeError("requires native Windows and explicit disposable-runner console opt-in")
    if ctypes.sizeof(ctypes.c_void_p) != 8 or ctypes.sizeof(STARTUPINFOEX) != 112:
        raise RuntimeError("console validation requires the native Windows x64 ABI")
    binary = arguments.binary.resolve(strict=True)
    secret = "synthetic console password 日本語 2026".encode("utf-8")
    with tempfile.TemporaryDirectory(prefix="termkey-console-validation-") as directory:
        console = Console(directory)
        try:
            initial_mode = console.mode()
            if initial_mode & (ECHO | LINE_INPUT) != ECHO | LINE_INPUT:
                raise RuntimeError("isolated console did not start in ordinary input mode")
            process = console.spawn([binary, "init"])
            console.wait_text("Choose a master password:")
            console.wait_hidden()
            console.write(secret + b"\r")
            console.wait_text("Confirm master password:")
            console.wait_hidden()
            console.write(secret + b"\r")
            console.wait_text("Set up a 24-word recovery phrase now?")
            # Confirm accepts a single key; do not leave Enter queued for the TUI.
            console.write(b"n")
            console.wait_exit(process)
            if secret.decode("utf-8") in plain_text(console.snapshot()):
                raise RuntimeError("password appeared in console output")
            if not Path(console.vault_directory, "vault.ck").read_bytes().startswith(b"CKPR"):
                raise RuntimeError("console init did not create a production vault")
            if console.mode() & ECHO != initial_mode & ECHO:
                raise RuntimeError("CLI password prompt did not restore console echo")
            print("PASS native Windows CLI init: Unicode hidden input, vault creation, restored echo")

            offset = len(console.snapshot())
            process = console.spawn([binary])
            console.wait_text("Enter your master password", offset)
            if console.mode() & (ECHO | LINE_INPUT):
                raise RuntimeError("TUI did not enter raw console input mode")
            console.write(secret + b"\r")
            console.wait_text("No entries yet.", offset)
            console.write(b"\x11")
            console.wait_exit(process)
            restored_mode = console.mode()
            if restored_mode & (ECHO | LINE_INPUT) != initial_mode & (ECHO | LINE_INPUT):
                raise RuntimeError("TUI exit did not restore console input mode")
            if secret.decode("utf-8") in plain_text(console.snapshot()):
                raise RuntimeError("password appeared in TUI output")
            print("PASS native Windows TUI: real console initialization, Unicode unlock, dashboard, Ctrl-Q, restored input mode")
        finally:
            console.close()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print("Windows terminal validation failed: " + str(error), file=sys.stderr)
        sys.exit(1)
