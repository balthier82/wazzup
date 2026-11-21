#!/usr/bin/env python3
import argparse
import datetime
import os
import platform
import re
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
APP_ID = "io.github.balthier82.Wazzup"
CARGO_MANIFEST = ROOT / "Cargo.toml"
METAINFO = ROOT / "data" / f"{APP_ID}.metainfo.xml"
DESKTOP_FILE = ROOT / "data" / f"{APP_ID}.desktop"
WORKFLOW = "release.yml"
LINUXDEPLOY_RELEASE = "1-alpha-20251107-1"
GTK_PLUGIN_COMMIT = "7a3fbc31a9e5075073ff8790f26effbac5f84453"
ENGINE_LOCALES = ("en-US", "it", "fr", "de", "es")
ENGINE_LIBRARIES = ("libcef.so", "libvk_swiftshader.so", "libvulkan.so.1")
ENGINE_FILES = (
    "vk_swiftshader_icd.json",
    "icudtl.dat",
    "v8_context_snapshot.bin",
    "resources.pak",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
)
VERSION_PATTERN = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


class Failure(Exception):
    pass


def say(message):
    print(f"\033[1;34m==>\033[0m {message}", flush=True)


def warn(message):
    print(f"\033[1;33mwarning:\033[0m {message}", file=sys.stderr, flush=True)


def tool_environment(extra=None):
    environment = dict(os.environ)
    cargo_bin = Path.home() / ".cargo" / "bin"
    if cargo_bin.is_dir() and str(cargo_bin) not in environment.get("PATH", ""):
        environment["PATH"] = f"{cargo_bin}{os.pathsep}{environment.get('PATH', '')}"
    environment.update(extra or {})
    return environment


def run(command, extra_environment=None, capture=False, check=True):
    shown = " ".join(str(part) for part in command)
    if not capture:
        print(f"\033[2m$ {shown}\033[0m", flush=True)
    completed = subprocess.run(
        [str(part) for part in command],
        cwd=ROOT,
        env=tool_environment(extra_environment),
        text=True,
        capture_output=capture,
    )
    if check and completed.returncode != 0:
        detail = completed.stderr.strip() if capture and completed.stderr else ""
        raise Failure(f"command failed ({completed.returncode}): {shown}\n{detail}".rstrip())
    return completed


def output_of(command):
    return run(command, capture=True).stdout.strip()


def require(program, hint):
    if shutil.which(program, path=tool_environment()["PATH"]) is None:
        raise Failure(f"'{program}' is not installed. {hint}")


def confirm(question, assume_yes):
    if assume_yes:
        return True
    if not sys.stdin.isatty():
        raise Failure(f"{question} Re-run with --yes to confirm without a terminal.")
    answer = input(f"{question} [y/N] ").strip().lower()
    return answer in {"y", "yes", "s", "si", "sì"}


def engine_folder(explicit):
    candidates = [explicit, os.environ.get("CEF_PATH"), str(Path.home() / ".local/share/cef")]
    for candidate in candidates:
        if candidate and (Path(candidate) / "libcef.so").is_file():
            return Path(candidate).resolve()
    raise Failure(
        "Chromium engine not found. Export it with:\n"
        "  cargo install export-cef-dir --version <cef crate version>\n"
        "  export-cef-dir --force ~/.local/share/cef"
    )


def current_version():
    for line in CARGO_MANIFEST.read_text(encoding="utf-8").splitlines():
        match = re.match(r'^version = "([^"]+)"', line)
        if match:
            return match.group(1)
    raise Failure("no version found in Cargo.toml")


def next_version(current, step):
    if VERSION_PATTERN.match(step):
        return step
    match = VERSION_PATTERN.match(current)
    if not match:
        raise Failure(f"current version '{current}' is not MAJOR.MINOR.PATCH")
    major, minor, patch = (int(part) for part in match.groups())
    if step == "major":
        return f"{major + 1}.0.0"
    if step == "minor":
        return f"{major}.{minor + 1}.0"
    if step == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise Failure(f"'{step}' is neither major, minor, patch nor a MAJOR.MINOR.PATCH version")


def is_newer(candidate, current):
    as_tuple = lambda version: tuple(int(part) for part in VERSION_PATTERN.match(version).groups())
    return as_tuple(candidate) > as_tuple(current)


def write_version(version):
    manifest = CARGO_MANIFEST.read_text(encoding="utf-8")
    updated, count = re.subn(r'(?m)^version = "[^"]+"', f'version = "{version}"', manifest, count=1)
    if count != 1:
        raise Failure("could not update the version in Cargo.toml")
    CARGO_MANIFEST.write_text(updated, encoding="utf-8")
    run(["cargo", "update", "--workspace", "--offline"])
    metainfo = METAINFO.read_text(encoding="utf-8")
    if f'version="{version}"' not in metainfo:
        today = datetime.date.today().isoformat()
        entry = f'<releases>\n    <release version="{version}" date="{today}"/>'
        metainfo = metainfo.replace("<releases>", entry, 1)
        METAINFO.write_text(metainfo, encoding="utf-8")


def ensure_clean_tree():
    if output_of(["git", "status", "--porcelain"]):
        raise Failure("the working tree has uncommitted changes; commit or stash them first")


def current_branch():
    return output_of(["git", "rev-parse", "--abbrev-ref", "HEAD"])


def tag_exists(tag):
    return run(["git", "rev-parse", "-q", "--verify", f"refs/tags/{tag}"], capture=True, check=False).returncode == 0


def command_check(arguments):
    environment = {"CEF_PATH": str(engine_folder(arguments.cef))}
    library_path = {"LD_LIBRARY_PATH": environment["CEF_PATH"], **environment}
    say("Formatting")
    run(["cargo", "fmt", "--check"])
    say("Lints")
    run(["cargo", "clippy", "--all-targets", "--", "-D", "warnings", "-W", "clippy::pedantic"], environment)
    say("Tests")
    run(["cargo", "test"], library_path)
    say("Translations")
    run([sys.executable, ROOT / "tools" / "extract-strings.py"])
    if shutil.which("desktop-file-validate"):
        say("Desktop entry")
        run(["desktop-file-validate", DESKTOP_FILE])
    if shutil.which("appstreamcli"):
        say("AppStream metadata")
        run(["appstreamcli", "validate", "--no-net", METAINFO])
    say("Everything looks good")


def fetch(destination, address):
    if not destination.is_file() or destination.stat().st_size == 0:
        say(f"Downloading {destination.name}")
        with urllib.request.urlopen(address) as response, open(destination, "wb") as target:
            shutil.copyfileobj(response, target)
    destination.chmod(0o755)


def install_file(source, destination, mode=0o644):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    destination.chmod(mode)


def command_appimage(arguments):
    require("cargo", "Install Rust with rustup.")
    require("strip", "Install binutils.")
    engine = engine_folder(arguments.cef)
    architecture = platform.machine()
    version = os.environ.get("WAZZUP_VERSION") or current_version()
    work = ROOT / "target" / "appimage"
    app_dir = work / "AppDir"
    tools = work / "tools"
    output = ROOT / "dist"
    tools.mkdir(parents=True, exist_ok=True)
    output.mkdir(parents=True, exist_ok=True)

    linuxdeploy = tools / f"linuxdeploy-{architecture}.AppImage"
    fetch(
        linuxdeploy,
        f"https://github.com/linuxdeploy/linuxdeploy/releases/download/{LINUXDEPLOY_RELEASE}/linuxdeploy-{architecture}.AppImage",
    )
    fetch(
        tools / "linuxdeploy-plugin-gtk.sh",
        f"https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/{GTK_PLUGIN_COMMIT}/linuxdeploy-plugin-gtk.sh",
    )

    say(f"Building Wazzup {version} for {architecture}")
    run(["cargo", "build", "--release"], {"CEF_PATH": str(engine)})

    say("Assembling the AppDir")
    shutil.rmtree(app_dir, ignore_errors=True)
    install_file(ROOT / "target" / "release" / "wazzup", app_dir / "usr/bin/wazzup", 0o755)
    libraries = app_dir / "usr/lib"
    for name in ENGINE_LIBRARIES:
        install_file(engine / name, libraries / name, 0o755)
    for name in ENGINE_FILES:
        install_file(engine / name, libraries / name)
    for locale in ENGINE_LOCALES:
        install_file(engine / "locales" / f"{locale}.pak", libraries / "locales" / f"{locale}.pak")
    run(["strip", "--strip-unneeded", libraries / "libcef.so", libraries / "libvk_swiftshader.so"])
    install_file(METAINFO, app_dir / "usr/share/metainfo" / METAINFO.name)
    shutil.copytree(ROOT / "data" / "icons", app_dir / "usr/share/icons", dirs_exist_ok=True)

    packaging = {
        "PATH": f"{tools}{os.pathsep}{tool_environment()['PATH']}",
        "APPIMAGE_EXTRACT_AND_RUN": "1",
        "DEPLOY_GTK_VERSION": "4",
        "NO_STRIP": "1",
    }
    say("Bundling GTK and libraries")
    run(
        [
            linuxdeploy,
            "--appdir", app_dir,
            "--executable", app_dir / "usr/bin/wazzup",
            "--desktop-file", DESKTOP_FILE,
            "--icon-file", ROOT / "data/icons/hicolor/scalable/apps" / f"{APP_ID}.svg",
            "--plugin", "gtk",
        ],
        packaging,
    )
    hook = app_dir / "apprun-hooks" / "linuxdeploy-plugin-gtk.sh"
    if hook.is_file():
        kept = [
            line
            for line in hook.read_text(encoding="utf-8").splitlines()
            if not line.startswith(("export GDK_BACKEND=", "export GTK_THEME="))
        ]
        hook.write_text("\n".join(kept) + "\n", encoding="utf-8")

    artifact = output / f"Wazzup-{version}-{architecture}.AppImage"
    say("Writing the AppImage")
    run(
        [linuxdeploy, "--appdir", app_dir, "--output", "appimage"],
        {
            **packaging,
            "LDAI_OUTPUT": str(artifact),
            "LDAI_UPDATE_INFORMATION": f"gh-releases-zsync|balthier82|wazzup|latest|Wazzup-*-{architecture}.AppImage.zsync",
        },
    )
    size = artifact.stat().st_size / (1024 * 1024)
    say(f"Built {artifact.relative_to(ROOT)} ({size:.0f} MB)")


def prepare_version(arguments):
    ensure_clean_tree()
    branch = current_branch()
    if branch != "main":
        warn(f"you are on '{branch}', not on main")
    current = current_version()
    target = next_version(current, arguments.step)
    tag = f"v{target}"
    if tag_exists(tag):
        raise Failure(f"tag {tag} already exists")
    first_tag = target == current
    if not first_tag and not is_newer(target, current):
        raise Failure(f"{target} is not newer than {current}")
    say(f"Tagging the current version {target}" if first_tag else f"Version {current} → {target}")
    if not arguments.skip_checks:
        command_check(arguments)
    if not first_tag:
        write_version(target)
        run(["git", "add", CARGO_MANIFEST, ROOT / "Cargo.lock", METAINFO])
        run(["git", "commit", "--message", f"Release {tag}"])
    run(["git", "tag", "--annotate", tag, "--message", f"Wazzup {target}"])
    say(f"Created commit and tag {tag}")
    return branch, tag


def push(branch, tag, assume_yes):
    remote = output_of(["git", "remote", "get-url", "origin"])
    question = f"Push branch '{branch}' and tag {tag} to {remote}? This starts the GitHub release build."
    if not confirm(question, assume_yes):
        say(f"Not pushed. Later run: git push origin {branch} {tag}")
        return
    run(["git", "push", "origin", branch])
    run(["git", "push", "origin", tag])
    say(f"Pushed {tag}. Follow the build with: ./waz-cmd.py status")


def command_bump(arguments):
    branch, tag = prepare_version(arguments)
    if arguments.push:
        push(branch, tag, arguments.yes)
    else:
        say(f"Not pushed. Publish with: git push origin {branch} {tag}")


def command_release(arguments):
    branch, tag = prepare_version(arguments)
    push(branch, tag, arguments.yes)


def command_rerun(arguments):
    require("gh", "Install the GitHub CLI and run 'gh auth login'.")
    latest = output_of(
        ["gh", "run", "list", "--workflow", WORKFLOW, "--limit", "1", "--json", "databaseId,headBranch,conclusion",
         "--jq", r'.[0] | "\(.databaseId) \(.headBranch) \(.conclusion)"']
    )
    if not latest:
        raise Failure("no release build found on GitHub yet")
    run_id, reference, conclusion = (latest.split(" ") + ["", "", ""])[:3]
    if not confirm(f"Run the release build for {reference} again (last result: {conclusion or 'running'})?", arguments.yes):
        return
    run(["gh", "run", "rerun", run_id])
    say("Build restarted. Follow it with: ./waz-cmd.py status")


def command_status(arguments):
    require("gh", "Install the GitHub CLI and run 'gh auth login'.")
    say("Recent release builds")
    if run(["gh", "run", "list", "--workflow", WORKFLOW, "--limit", str(arguments.limit)], check=False).returncode != 0:
        warn("the release workflow is not on GitHub yet; push the repository first")
    say("Releases")
    run(["gh", "release", "list", "--limit", str(arguments.limit)], check=False)


def command_dev(arguments):
    engine = engine_folder(arguments.cef)
    environment = {
        "CEF_PATH": str(engine),
        "WAZZUP_ENGINE_DIR": str(engine),
        "LD_LIBRARY_PATH": f"{engine}{os.pathsep}{os.environ.get('LD_LIBRARY_PATH', '')}",
    }
    if arguments.profile:
        environment["WAZZUP_PROFILE"] = arguments.profile
    run(["cargo", "build"], environment)
    os.execvpe(
        str(ROOT / "target" / "debug" / "wazzup"),
        ["wazzup", *arguments.rest],
        tool_environment(environment),
    )


def parser():
    root = argparse.ArgumentParser(
        prog="waz-cmd.py",
        description="Build, check and release Wazzup.",
    )
    root.add_argument("--cef", help="folder with the exported Chromium engine (default: $CEF_PATH or ~/.local/share/cef)")
    commands = root.add_subparsers(dest="command", required=True)

    commands.add_parser("check", help="formatting, lints, tests, translations and metadata").set_defaults(handler=command_check)
    commands.add_parser("appimage", help="build an AppImage for this machine into dist/").set_defaults(handler=command_appimage)

    for name, handler, help_text in (
        ("bump", command_bump, "raise the version, commit and tag it locally"),
        ("release", command_release, "raise the version, tag it and push to start the draft release on GitHub"),
    ):
        command = commands.add_parser(name, help=help_text)
        command.add_argument("step", help="major, minor, patch or an explicit version such as 1.2.0")
        command.add_argument("--skip-checks", action="store_true", help="do not run 'check' before tagging")
        command.add_argument("--yes", action="store_true", help="push without asking")
        if name == "bump":
            command.add_argument("--push", action="store_true", help="also push the commit and the tag")
        command.set_defaults(handler=handler)

    rerun = commands.add_parser("rerun", help="run the latest GitHub release build again")
    rerun.add_argument("--yes", action="store_true", help="restart without asking")
    rerun.set_defaults(handler=command_rerun)

    status = commands.add_parser("status", help="show recent GitHub builds and releases")
    status.add_argument("--limit", type=int, default=5)
    status.set_defaults(handler=command_status)

    dev = commands.add_parser("dev", help="build and start the development version")
    dev.add_argument("--profile", help="run with a separate profile")
    dev.add_argument("rest", nargs=argparse.REMAINDER, help="arguments passed to Wazzup")
    dev.set_defaults(handler=command_dev)
    return root


def main():
    arguments = parser().parse_args()
    try:
        arguments.handler(arguments)
    except Failure as failure:
        print(f"\033[1;31merror:\033[0m {failure}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 130
    return 0


if __name__ == "__main__":
    sys.exit(main())
