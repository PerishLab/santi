import {
  armRecovery,
  guardDeploy,
  parseRecoveryRequest,
  recovery,
} from "@/lib/recovery/recovery.ts";
import { recoveryRemoteCommand } from "@/lib/recovery/remote.ts";

function assertEquals(actual: unknown, expected: unknown): void {
  const left = JSON.stringify(actual);
  const right = JSON.stringify(expected);
  if (left !== right) throw new Error(`expected ${right}, received ${left}`);
}

function assertThrows(action: () => unknown): void {
  try {
    action();
  } catch {
    return;
  }
  throw new Error("expected action to throw");
}

async function assertRejects(action: () => Promise<unknown>, expected: Error): Promise<void> {
  try {
    await action();
  } catch (error) {
    if (error !== expected) throw new Error("unexpected rejection");
    return;
  }
  throw new Error("expected promise to reject");
}

function decodeArguments(bytes: Uint8Array): string[] {
  const decoded: string[] = [];
  let start = 0;
  for (let index = 0; index < bytes.length; index += 1) {
    if (bytes[index] !== 0) continue;
    decoded.push(new TextDecoder().decode(bytes.slice(start, index)));
    start = index + 1;
  }
  if (start !== bytes.length) throw new Error("shell output was not NUL terminated");
  return decoded;
}

async function shellRoundTrip(argv: string[]): Promise<string[]> {
  const child = new Deno.Command("bash", {
    args: ["-c", recoveryRemoteCommand(argv)],
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
  }).spawn();
  const writer = child.stdin.getWriter();
  await writer.write(new TextEncoder().encode("if (( $# > 0 )); then printf '%s\\0' \"$@\"; fi\n"));
  await writer.close();
  const output = await child.output();
  if (!output.success) {
    throw new Error(`local shell failed: ${new TextDecoder().decode(output.stderr)}`);
  }
  return decodeArguments(output.stdout);
}

Deno.test("recovery parser accepts only the four exact command forms", () => {
  const cases = [
    [["status"], { action: "status" }],
    [["repair"], { action: "repair" }],
    [["accept", "capsule"], { action: "accept", capsule: "capsule" }],
    [
      ["execute", "capsule", "--confirm", "0.1.0-beta.54"],
      { action: "execute", capsule: "capsule", candidateVersion: "0.1.0-beta.54" },
    ],
  ] as const;
  for (const [argv, expected] of cases) assertEquals(parseRecoveryRequest([...argv]), expected);
});

Deno.test("recovery parser rejects empty, unknown, missing, extra, reordered, and duplicate forms", () => {
  const rejected = [
    [],
    ["unknown"],
    ["status", "extra"],
    ["repair", "extra"],
    ["accept"],
    ["accept", ""],
    ["accept", "capsule", "extra"],
    ["execute"],
    ["execute", "capsule"],
    ["execute", "capsule", "--confirm"],
    ["execute", "", "--confirm", "version"],
    ["execute", "capsule", "--confirm", ""],
    ["execute", "capsule", "version", "--confirm"],
    ["execute", "--confirm", "version", "capsule"],
    ["--confirm", "version", "execute", "capsule"],
    ["execute", "capsule", "--confirmation", "version"],
    ["execute", "capsule", "--confirm", "version", "extra"],
    ["execute", "capsule", "--confirm", "--confirm"],
    ["status", "status"],
  ];
  for (const argv of rejected) assertThrows(() => parseRecoveryRequest(argv));
});

Deno.test("recovery help wins in any position without transport", async () => {
  let calls = 0;
  const remote = (_argv: string[]) => {
    calls += 1;
    return Promise.resolve(99);
  };
  assertEquals(await recovery(["--help"], remote), 0);
  assertEquals(await recovery(["unknown", "-h", "extra"], remote), 0);
  assertEquals(calls, 0);
});

Deno.test("recovery parse errors return two and do not invoke transport", async () => {
  let calls = 0;
  const code = await recovery(["execute", "capsule", "--confirm"], () => {
    calls += 1;
    return Promise.resolve(0);
  });
  assertEquals(code, 2);
  assertEquals(calls, 0);
});

Deno.test("recovery maps requests and propagates numeric remote exits", async () => {
  const cases = [
    [["status"], ["status"]],
    [["repair"], ["arm"]],
    [["accept", "capsule"], ["accept", "capsule"]],
    [
      ["execute", "capsule", "--confirm", "version"],
      ["execute", "capsule", "--confirm", "version"],
    ],
  ];
  for (const [input, expected] of cases) {
    let observed: string[] = [];
    const code = await recovery(input, (argv) => {
      observed = argv;
      return Promise.resolve(37);
    });
    assertEquals(code, 37);
    assertEquals(observed, expected);
  }
});

Deno.test("deploy guard and recovery arm keep their fixed host mappings", async () => {
  const observed: string[][] = [];
  const remote = (argv: string[]) => {
    observed.push(argv);
    return Promise.resolve(0);
  };
  await guardDeploy(remote);
  await armRecovery(remote);
  assertEquals(observed, [["guard-deploy"], ["arm"]]);
});

Deno.test("recovery preserves rejected transport exceptions", async () => {
  const transport = new Error("transport rejected");
  await assertRejects(
    () => recovery(["status"], () => Promise.reject(transport)),
    transport,
  );
});

Deno.test("parser mapping reaches the real serializer with hostile values", async () => {
  const capsule = "capsule ' \" \\ ; $()\n第二行";
  const version = "--candidate\tversion";
  let serialized = "";
  const code = await recovery(
    ["execute", capsule, "--confirm", version],
    async (argv) => {
      serialized = recoveryRemoteCommand(argv);
      assertEquals(await shellRoundTrip(argv), argv);
      return 71;
    },
  );
  assertEquals(code, 71);
  if (serialized.length === 0) throw new Error("serializer was not reached");
});

Deno.test("POSIX serializer round-trips hostile argv without collapsing boundaries", async () => {
  const vectors = [
    [],
    [""],
    ["a b"],
    ["a", "b"],
    ["space value", "tab\tvalue", "line one\nline two"],
    ["'single'", '"double"', "back\\slash"],
    [";$()|&<>*?[]{}!#~`", "--leading-dash", "雪/火"],
    ["", "middle", ""],
  ];
  for (const argv of vectors) assertEquals(await shellRoundTrip(argv), argv);
  if (recoveryRemoteCommand(["a b"]) === recoveryRemoteCommand(["a", "b"])) {
    throw new Error("space-containing and split arguments collided");
  }
  if (recoveryRemoteCommand([""]) === recoveryRemoteCommand([])) {
    throw new Error("empty argument and empty argv collided");
  }
});
