//! Repo root resolution, shared by operator commands.

export function repoRoot(): string {
  return Deno.cwd();
}
