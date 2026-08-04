//! Operator-facing recovery capsule commands plus the two deploy integration
//! points. The host script owns validation and all state transitions.

import { runRecoveryRemote } from "@/lib/recovery/remote.ts";

export type RecoveryRequest =
  | { action: "status" }
  | { action: "repair" }
  | { action: "execute"; capsule: string; candidateVersion: string }
  | { action: "accept"; capsule: string };

type RecoveryRemote = (argv: string[]) => Promise<number>;

function requireValue(value: string | undefined, name: string): string {
  if (value === undefined || value.length === 0) {
    throw new Error(`${name} must be nonempty`);
  }
  return value;
}

export function parseRecoveryRequest(argv: string[]): RecoveryRequest {
  if (argv.length === 1 && argv[0] === "status") {
    return { action: "status" };
  }
  if (argv.length === 1 && argv[0] === "repair") {
    return { action: "repair" };
  }
  if (argv.length === 2 && argv[0] === "accept") {
    return { action: "accept", capsule: requireValue(argv[1], "capsule") };
  }
  if (argv.length === 4 && argv[0] === "execute" && argv[2] === "--confirm") {
    const candidateVersion = requireValue(argv[3], "candidate version");
    if (candidateVersion === "--confirm") throw new Error("duplicate --confirm");
    return {
      action: "execute",
      capsule: requireValue(argv[1], "capsule"),
      candidateVersion,
    };
  }
  throw new Error("invalid recovery command");
}

function printHelp(): void {
  console.log("Usage:");
  console.log("  santi operator rollback status");
  console.log("  santi operator rollback repair");
  console.log("  santi operator rollback execute <capsule-id> --confirm <candidate-version>");
  console.log("  santi operator rollback accept <capsule-id>");
  console.log("");
  console.log("Inspect, execute, or accept the single armed post-deploy recovery capsule.");
}

export async function recovery(
  argv: string[],
  remote: RecoveryRemote = runRecoveryRemote,
): Promise<number> {
  if (argv.includes("-h") || argv.includes("--help")) {
    printHelp();
    return 0;
  }

  let request: RecoveryRequest;
  try {
    request = parseRecoveryRequest(argv);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printHelp();
    return 2;
  }

  if (request.action === "status") {
    return await remote(["status"]);
  }
  if (request.action === "repair") {
    return await remote(["arm"]);
  }
  if (request.action === "accept") {
    return await remote(["accept", request.capsule]);
  }
  return await remote([
    "execute",
    request.capsule,
    "--confirm",
    request.candidateVersion,
  ]);
}

/** Refuse a deployment while a previous candidate is still armed. */
export async function guardDeploy(remote: RecoveryRemote = runRecoveryRemote): Promise<number> {
  return await remote(["guard-deploy"]);
}

/** Turn the upgrader's raw pre-deploy snapshot into a durable capsule. */
export async function armRecovery(remote: RecoveryRemote = runRecoveryRemote): Promise<number> {
  return await remote(["arm"]);
}
