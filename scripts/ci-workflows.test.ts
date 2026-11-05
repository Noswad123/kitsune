import { expect, test } from "bun:test";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const workflows = join(root, ".github", "workflows");

function workflow(name: string) {
  return Bun.YAML.parse(readFileSync(join(workflows, name), "utf8"));
}

function dryRun(recipe: string) {
  const result = Bun.spawnSync(["just", "--dry-run", recipe], {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  return new TextDecoder().decode(result.stdout) + new TextDecoder().decode(result.stderr);
}

test("workflow files parse with event and job definitions", () => {
  for (const name of readdirSync(workflows).filter((name) => /\.ya?ml$/.test(name))) {
    const parsed = workflow(name);
    expect(parsed).toBeObject();
    expect(parsed.on).toBeObject();
    expect(parsed.jobs).toBeObject();
  }
});

test("CI and Nix target only main pushes and pull requests", () => {
  for (const name of ["ci.yml", "nix.yml"]) {
    const parsed = workflow(name);
    expect(parsed.on.push.branches).toEqual(["main"]);
    expect(parsed.on.pull_request.branches).toEqual(["main"]);
    expect(JSON.stringify(parsed)).not.toMatch(/master|windows/i);
  }
});

test("CI does not enforce commit message or PR title formatting", () => {
  const ci = workflow("ci.yml");
  expect(ci.jobs["conventional-commits"]).toBeUndefined();
  expect(JSON.stringify(ci)).not.toMatch(/conventional_commits\.py|Validate commit subjects|Validate PR title/);
});

test("hook installation retains lint but does not enforce commit message formatting", () => {
  expect(existsSync(join(root, ".githooks", "commit-msg"))).toBe(false);
  expect(existsSync(join(root, "scripts", "conventional_commits.py"))).toBe(false);
  expect(readFileSync(join(root, ".githooks", "pre-commit"), "utf8")).toContain("just lint");
  const commands = dryRun("install-hooks");
  expect(commands).toContain("chmod +x .githooks/pre-commit");
  expect(commands).not.toMatch(/commit-msg|conventional_commits/);
});

test("CI retains Linux and macOS coverage and the macOS handoff exclusion", () => {
  const ci = workflow("ci.yml");
  expect(Object.keys(ci.jobs)).toEqual(["check"]);
  expect(ci.jobs.check.strategy.matrix.include).toEqual([
    { os: "ubuntu-latest", nextest_filter: "all()" },
    { os: "macos-latest", nextest_filter: "not binary(live_handoff)" },
  ]);
  expect(ci.jobs.check.steps.find((step) => step.name === "Run checks").if).toBeUndefined();
});

test("manual artifacts offer only Linux, macOS and all", () => {
  const manual = workflow("build-artifacts-manual.yml");
  expect(manual.on.workflow_dispatch.inputs.build_group.options).toEqual(["linux", "macos", "all"]);
  expect(Object.keys(manual.jobs).sort()).toEqual(["build-linux", "build-macos"]);
  expect(JSON.stringify(manual)).not.toMatch(/windows|msvc|conpty/i);
});

test("CI retains read-only repository permissions", () => {
  for (const name of ["ci.yml", "nix.yml"]) {
    expect(workflow(name).permissions).toEqual({ contents: "read" });
  }
});

test("full local checks retain native coverage without installing targets", () => {
  const commands = dryRun("check");
  expect(commands).toContain("cargo fmt --check");
  expect(commands).toContain("cargo clippy --all-targets --locked -- -D warnings");
  expect(commands).toContain("cargo nextest run --locked");
  expect(commands).toContain("just maintenance-test");
  expect(commands).toContain("just integration-assets-test");
  expect(commands).toContain("just ci-workflows-test");
  expect(commands).not.toMatch(/rustup target add|windows|msvc/i);
});
