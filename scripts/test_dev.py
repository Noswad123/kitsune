import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parent.parent


class DevTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="kitsune-dev-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        shutil.copyfile(ROOT / "scripts/dev.py", self.root / "scripts/dev.py")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.artifact = self.root / "custom target" / "configured-triple" / "debug/kitsune"
        self.artifact.parent.mkdir(parents=True)
        self.script(self.artifact, "print('fresh executable')")
        stale = self.root / "target/debug/kitsune"
        stale.parent.mkdir(parents=True)
        stale.write_text("stale executable")
        self.env = os.environ.copy()
        for key in ("ZIG", "INSTALL_DIR"):
            self.env.pop(key, None)
        self.env.update(
            PATH=str(self.bin), HOME=str(self.root / "home"),
            CARGO_TARGET_DIR=str(self.root / "custom target"),
            ARTIFACT=str(self.artifact), RECORD=str(self.root / "cargo.json"),
        )
        self.script(self.bin / "zig", "print('0.15.2')")
        self.script(self.bin / "cargo", """
import json, os, sys
from pathlib import Path
Path(os.environ['RECORD']).write_text(json.dumps({'args': sys.argv[1:], 'zig': os.environ['ZIG'], 'target': os.environ['CARGO_TARGET_DIR'], 'cwd': os.getcwd()}))
print('normal build output', flush=True)
print(json.dumps({'reason': 'compiler-message', 'message': {'rendered': 'rendered diagnostic\\n'}}))
if not os.environ.get('NO_ARTIFACT'):
    print(json.dumps({'reason': 'compiler-artifact', 'target': {'name': 'kitsune', 'kind': ['bin']}, 'executable': os.environ['ARTIFACT']}))
sys.exit(int(os.environ.get('CARGO_FAIL', '0')))
""")
        self.script(self.root / "scripts/sign_dev_binary.sh", """
import os, sys
from pathlib import Path
Path(os.environ['RECORD'] + '.sign').write_text(sys.argv[1])
sys.exit(int(os.environ.get('SIGN_FAIL', '0')))
""")

    def script(self, path, body):
        path.write_text(f"#!{sys.executable}\n{body}\n")
        path.chmod(0o755)

    def invoke(self, action="install", **env):
        return subprocess.run(
            [sys.executable, str(self.root / "scripts/dev.py"), action],
            env={**self.env, **env}, cwd=self.root, capture_output=True, text=True,
        )

    def installed(self):
        return self.root / "home/.local/libexec/arcana-dev/kitsune/bin/kitsune"

    def record(self):
        return json.loads((self.root / "cargo.json").read_text())

    def fallback(self, version="0.15.2"):
        self.script(self.bin / "zig", "print('0.17.1')")
        prefix = self.root / "brew prefix"
        (prefix / "bin").mkdir(parents=True)
        self.script(prefix / "bin/zig", f"print({version!r})")
        self.script(self.bin / "brew", f"import sys\nassert sys.argv[1:] == ['--prefix', 'zig@0.15']\nprint({str(prefix)!r})")
        return prefix / "bin/zig"

    def test_path_compiler_and_default_install(self):
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["zig"], str(self.bin / "zig"))
        self.assertEqual(self.installed().read_bytes(), self.artifact.read_bytes())
        self.assertEqual(self.installed().stat().st_mode & 0o777, 0o755)
        self.assertIn("normal build output", result.stdout)
        self.assertIn("rendered diagnostic", result.stderr)
        self.assertIn("--message-format=json-render-diagnostics", self.record()["args"])
        self.assertEqual(self.record()["target"], self.env["CARGO_TARGET_DIR"])
        self.assertEqual(self.record()["cwd"], str(self.root))

    def test_relative_explicit_zig_is_absolute_for_cargo(self):
        result = self.invoke(ZIG="./bin/zig")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["zig"], str(self.bin / "zig"))

    def test_relative_path_zig_is_absolute_for_cargo(self):
        result = self.invoke(PATH="bin")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["zig"], str(self.bin / "zig"))

    def test_explicit_zig_command_resolves_on_path(self):
        result = self.invoke(ZIG="zig")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["zig"], str(self.bin / "zig"))

    def test_relative_cargo_artifact_is_relative_to_root_not_caller(self):
        caller = self.root / "caller"
        caller.mkdir()
        result = subprocess.run(
            [sys.executable, str(self.root / "scripts/dev.py"), "install"],
            cwd=caller, env={**self.env, "ARTIFACT": str(self.artifact.relative_to(self.root))},
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["cwd"], str(self.root))
        self.assertEqual(self.installed().read_bytes(), self.artifact.read_bytes())

    def test_brew_fallback(self):
        zig = self.fallback()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["zig"], str(zig))

    def test_explicit_zig_precedes_path_and_brew(self):
        self.fallback()
        zig = self.root / "explicit zig"
        self.script(zig, "print('0.15.2')")
        result = self.invoke(ZIG=str(zig))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.record()["zig"], str(zig))

    def test_invalid_override_refused_before_cargo(self):
        for zig in (str(self.root / "missing"), str(self.bin / "zig"), ""):
            with self.subTest(zig=zig):
                self.script(self.bin / "zig", "print('0.17.1')")
                result = self.invoke(ZIG=zig)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("fix or unset ZIG", result.stderr)
                self.assertFalse((self.root / "cargo.json").exists())

    def test_incompatible_brew_refused_before_cargo(self):
        self.fallback("0.17.1")
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Zig 0.15.2 is required", result.stderr)
        self.assertFalse((self.root / "cargo.json").exists())

    def test_missing_brew_and_incompatible_path(self):
        self.script(self.bin / "zig", "print('0.17.1')")
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "cargo.json").exists())

    def test_compatible_path_does_not_probe_brew(self):
        self.script(self.bin / "brew", "raise AssertionError('must not probe brew')")
        self.assertEqual(self.invoke().returncode, 0)

    def test_cargo_failure_leaves_old_install(self):
        self.installed().parent.mkdir(parents=True)
        self.installed().write_text("old")
        result = self.invoke(CARGO_FAIL="42")
        self.assertEqual(result.returncode, 42)
        self.assertEqual(self.installed().read_text(), "old")
        self.assertNotIn("Installed.", result.stdout)
        self.assertFalse((self.root / "cargo.json.sign").exists())

    def test_missing_artifact_never_uses_stale_default(self):
        result = self.invoke(NO_ARTIFACT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not report", result.stderr)
        self.assertFalse(self.installed().exists())

    def test_install_dir_with_spaces(self):
        directory = self.root / "install with spaces"
        result = self.invoke(INSTALL_DIR=str(directory))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((directory / "kitsune").read_bytes(), self.artifact.read_bytes())
        signed = Path((self.root / "cargo.json.sign").read_text())
        self.assertEqual(signed.parent, directory)
        self.assertFalse(signed.exists())

    def test_sign_failure_keeps_old_install_and_cleans_stage(self):
        self.installed().parent.mkdir(parents=True)
        self.installed().write_text("old")
        result = self.invoke(SIGN_FAIL="23")
        self.assertEqual(result.returncode, 23)
        self.assertEqual(self.installed().read_text(), "old")
        self.assertEqual(list(self.installed().parent.iterdir()), [self.installed()])
        self.assertNotIn("Installed.", result.stdout)

    @unittest.skipUnless(sys.platform == "linux", "checks Linux process disappearance including zombies")
    def test_signals_stop_and_reap_signer_before_stage_cleanup(self):
        self.installed().parent.mkdir(parents=True)
        self.installed().write_text("old")
        descendant = self.root / "codesign"
        self.script(descendant, """
import os, signal, sys, time
from pathlib import Path
def stop(signum, frame):
    Path('stage-present').write_text(str(Path(sys.argv[1]).exists()))
    sys.exit(0)
signal.signal(signal.SIGTERM, stop)
Path('descendant.pid').write_text(str(os.getpid()))
while True:
    time.sleep(0.05)
""")
        self.script(self.root / "scripts/sign_dev_binary.sh", f"""
import os, signal, subprocess, sys, time
from pathlib import Path
signal.signal(signal.SIGTERM, signal.SIG_IGN)
process = subprocess.Popen([{sys.executable!r}, {str(descendant)!r}, sys.argv[1]])
while not Path('descendant.pid').exists():
    time.sleep(0.01)
Path('signer.pid').write_text(str(os.getpid()))
process.wait()
Path('descendant-reaped').write_text('yes')
while True:
    time.sleep(0.05)
""")
        for sig in (signal.SIGTERM, signal.SIGINT):
            with self.subTest(signal=sig):
                for name in ("signer.pid", "descendant.pid", "stage-present", "descendant-reaped"):
                    (self.root / name).unlink(missing_ok=True)
                helper = subprocess.Popen(
                    [sys.executable, str(self.root / "scripts/dev.py"), "install"],
                    cwd=self.root, env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    text=True,
                )
                signer = None
                try:
                    deadline = time.monotonic() + 5
                    while not (self.root / "signer.pid").exists():
                        if helper.poll() is not None or time.monotonic() >= deadline:
                            self.fail("blocked signer did not become ready")
                        time.sleep(0.01)
                    signer = int((self.root / "signer.pid").read_text())
                    child_pid = int((self.root / "descendant.pid").read_text())
                    helper.send_signal(sig)
                    stdout, stderr = helper.communicate(timeout=5)
                    self.assertEqual(helper.returncode, 128 + sig, stderr)
                    self.assertEqual(self.installed().read_text(), "old")
                    self.assertEqual(list(self.installed().parent.iterdir()), [self.installed()])
                    self.assertNotIn("Installed.", stdout)
                    self.assertEqual((self.root / "stage-present").read_text(), "True")
                    self.assertEqual((self.root / "descendant-reaped").read_text(), "yes")
                    # kill(pid, 0) also succeeds for zombies, so require ESRCH.
                    for pid in (signer, child_pid):
                        with self.assertRaises(ProcessLookupError):
                            os.kill(pid, 0)
                finally:
                    if signer is not None:
                        try:
                            os.killpg(signer, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    if helper.poll() is None:
                        helper.kill()
                    helper.communicate(timeout=5)

    def test_build_signs_reported_artifact(self):
        result = self.invoke("build")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "cargo.json.sign").read_text(), str(self.artifact))
        self.assertFalse(self.installed().exists())

    def test_run_clears_runtime_env_and_uses_reported_artifact(self):
        self.script(self.artifact, "import os, json\nprint(json.dumps({key: os.environ.get(key) for key in ('KITSUNE_SOCKET_PATH', 'KITSUNE_CLIENT_SOCKET_PATH', 'KITSUNE_ENV')}))")
        result = self.invoke("run", KITSUNE_SOCKET_PATH="socket", KITSUNE_CLIENT_SOCKET_PATH="client", KITSUNE_ENV="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout.splitlines()[-1]), {
            "KITSUNE_SOCKET_PATH": None, "KITSUNE_CLIENT_SOCKET_PATH": None, "KITSUNE_ENV": None,
        })

    @unittest.skipUnless(sys.platform == "linux" and shutil.which("cc"), "needs Linux and cc")
    def test_atomic_replacement_of_executing_destination(self):
        self.installed().parent.mkdir(parents=True)
        # A tiny native executable holds the destination mapped, unlike a script.
        subprocess.run([shutil.which("cc"), "-x", "c", "-o", str(self.installed()), "-"],
                       input='#include <unistd.h>\nint main(void) { write(1, "ready\\n", 6); sleep(30); return 0; }\n',
                       text=True, check=True, capture_output=True)
        before = self.installed().stat().st_ino
        with subprocess.Popen([str(self.installed())], stdout=subprocess.PIPE, text=True) as running:
            try:
                self.assertEqual(running.stdout.readline(), "ready\n")
                result = self.invoke()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIsNone(running.poll())
                self.assertNotEqual(self.installed().stat().st_ino, before)
                self.assertEqual(self.installed().read_bytes(), self.artifact.read_bytes())
            finally:
                running.terminate()
                running.wait()

    def test_just_recipe_wiring(self):
        just = shutil.which("just")
        if not just:
            self.skipTest("just unavailable")
        for recipe, action in (("dev-build", "build"), ("install", "install"), ("run", "run")):
            with self.subTest(recipe=recipe):
                result = subprocess.run([just, "--dry-run", recipe], cwd=ROOT,
                                        text=True, capture_output=True, check=True)
                output = result.stdout + result.stderr
                self.assertIn(f"python3 scripts/dev.py {action}", output)
                self.assertNotIn("target/debug", output)
                self.assertNotIn("cargo build", output)


if __name__ == "__main__":
    unittest.main()
