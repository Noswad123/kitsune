"""Build, run, and atomically install the local development binary."""

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
ZIG_VERSION = "0.15.2"


@contextmanager
def child(command, **kwargs):
    # Each build/tool step owns its group, never the caller's terminal group.
    process = None
    pending = []
    handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    # Defer interruption until Popen has returned an owned, reapable process.
    for sig in handlers:
        signal.signal(sig, lambda signum, frame: pending.append(signum))
    try:
        try:
            process = subprocess.Popen(command, start_new_session=os.name == "posix", **kwargs)
        finally:
            for sig, handler in handlers.items():
                signal.signal(sig, handler)
            if pending:
                signal.raise_signal(pending[0])
        yield process
    finally:
        def stop(force=False):
            try:
                if os.name == "posix":
                    os.killpg(process.pid, signal.SIGKILL if force else signal.SIGTERM)
                elif process.poll() is None:
                    process.kill() if force else process.terminate()
            except ProcessLookupError:
                pass

        if process is not None:
            stop()
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                pass
            finally:
                # Also stop descendants if the group leader exited before they did.
                stop(force=True)
            try:
                process.wait(timeout=1)
            finally:
                for stream in (process.stdout, process.stderr):
                    if stream is not None:
                        stream.close()


def capture(command):
    with child(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as process:
        stdout, stderr = process.communicate()
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def compatible_zig(candidate):
    try:
        result = capture([str(candidate), "version"])
        return result.returncode == 0 and result.stdout.strip() == ZIG_VERSION
    except OSError:
        return False


def build():
    env = os.environ.copy()
    if "ZIG" in env:
        zig = shutil.which(env["ZIG"]) if env["ZIG"] else None
        if not zig or not compatible_zig(zig):
            raise RuntimeError(
                f"ZIG={env['ZIG']!r} must select Zig {ZIG_VERSION}; fix or unset ZIG."
            )
    else:
        zig = shutil.which("zig")
        if not zig or not compatible_zig(zig):
            zig = None
            brew = shutil.which("brew")
            if brew:
                prefix = capture([brew, "--prefix", "zig@0.15"])
                if prefix.returncode == 0 and prefix.stdout.strip():
                    candidate = Path(prefix.stdout.strip()) / "bin" / "zig"
                    if compatible_zig(candidate):
                        zig = str(candidate)
        if not zig:
            raise RuntimeError(
                f"Zig {ZIG_VERSION} is required. Set ZIG=/path/to/zig, put it on "
                "PATH, or provide an installed Homebrew zig@0.15. Nothing was installed."
            )
    # build.rs runs Zig from the vendor directory, not this invocation's cwd.
    env["ZIG"] = os.path.abspath(zig)
    executable = None
    with child(
        ["cargo", "build", "--bin", "kitsune", "--message-format=json-render-diagnostics"],
        cwd=ROOT,
        env=env,
        stdout=subprocess.PIPE,
        text=True,
    ) as cargo:
        for line in cargo.stdout:
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                print(line, end="", flush=True)
                continue
            if not isinstance(message, dict):
                print(line, end="", flush=True)
                continue
            if message.get("reason") == "compiler-message":
                rendered = message.get("message", {}).get("rendered")
                if rendered:
                    print(rendered, end="", file=sys.stderr, flush=True)
            elif message.get("reason") == "compiler-artifact":
                target = message.get("target", {})
                if (target.get("name") == "kitsune" and "bin" in target.get("kind", [])
                        and message.get("executable")):
                    executable = Path(message["executable"])
        result = cargo.wait()
    if result:
        raise subprocess.CalledProcessError(result, "cargo build")
    if executable is None:
        raise RuntimeError("Cargo succeeded but did not report the kitsune executable.")
    if not executable.is_absolute():
        executable = ROOT / executable
    return executable


def sign(binary):
    command = [str(ROOT / "scripts/sign_dev_binary.sh"), str(binary)]
    with child(command) as process:
        result = process.wait()
        if result:
            raise subprocess.CalledProcessError(result, command)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "install", "run"))
    args = parser.parse_args()

    def interrupted(signum, frame):
        # Do not let a second catchable signal interrupt child/stage cleanup.
        for sig in previous_handlers:
            signal.signal(sig, signal.SIG_IGN)
        raise SystemExit(128 + signum)

    previous_handlers = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    for sig in previous_handlers:
        signal.signal(sig, interrupted)
    try:
        executable = build()
        if args.action == "install":
            directory = Path(os.environ.get("INSTALL_DIR") or
                             Path.home() / ".local/libexec/arcana-dev/kitsune/bin")
            directory.mkdir(parents=True, exist_ok=True)
            fd, name = tempfile.mkstemp(prefix=".kitsune-", dir=directory)
            stage = Path(name)
            try:
                os.close(fd)
                shutil.copyfile(executable, stage)
                stage.chmod(0o755)
                sign(stage)
                os.replace(stage, directory / "kitsune")
            finally:
                stage.unlink(missing_ok=True)
            print(f'Installed. Run with: "{directory / "kitsune"}"')
        else:
            sign(executable)
            if args.action == "run":
                env = os.environ.copy()
                for key in ("KITSUNE_SOCKET_PATH", "KITSUNE_CLIENT_SOCKET_PATH", "KITSUNE_ENV"):
                    env.pop(key, None)
                os.execve(executable, [str(executable)], env)
    except subprocess.CalledProcessError as error:
        print(f"error: {error}", file=sys.stderr)
        return error.returncode if error.returncode > 0 else 128 - error.returncode
    except (OSError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    finally:
        for sig, handler in previous_handlers.items():
            signal.signal(sig, handler)
    return 0


if __name__ == "__main__":
    sys.exit(main())
