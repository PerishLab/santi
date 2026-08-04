import ingress from "./ops/edge/ingress.yaml" with { type: "text" };
import nginx from "./ops/edge/resources/nginx.conf" with { type: "text" };
import postinst from "./packaging/deb/postinst" with { type: "text" };
import service from "./packaging/deb/root/lib/systemd/system/santi.service" with {
  type: "text",
};

interface SantiContractAssets {
  ingress: string;
  nginx: string;
  postinst: string;
  service: string;
}

function refuse(message: string): never {
  throw new Error(`Santi operator contract: ${message}`);
}

function requirePattern(source: string, pattern: RegExp, message: string): void {
  if (!pattern.test(source)) refuse(message);
}

function activeLines(source: string): string {
  return source.split("\n").map((line) => line.replace(/^\s*#.*$/, "").replace(/\s+#.*$/, "")).join(
    "\n",
  );
}

function yamlDocument(source: string, name: string): string {
  const documents = activeLines(source).split(/^\s*---\s*$/m).filter((document) =>
    /^kind:\s*IngressRoute\s*$/m.test(document) &&
    new RegExp(`^  name: ${name}$`, "m").test(document)
  );
  if (documents.length !== 1) refuse(`expected one ${name} IngressRoute`);
  return documents[0];
}

function assertIngress(source: string): void {
  const active = activeLines(source);
  const paths = [...active.matchAll(/\b(Path(?:Prefix)?)\(`(\/api\/v1\/webhooks[^`]*)`\)/g)]
    .map((match) => `${match[1]}:${match[2]}`);
  if (paths.length !== 1 || paths[0] !== "PathPrefix:/api/v1/webhooks/") {
    refuse("the only public webhook route must be the trailing-slash PathPrefix");
  }

  const webhooks = yamlDocument(active, "santi-webhooks");
  if (/^\s*middlewares:\s*$/m.test(webhooks)) {
    refuse("signature-authenticated webhook ingest must not use forward auth");
  }

  const catchAll = yamlDocument(active, "santi");
  requirePattern(catchAll, /^\s*- match: Host\(`[^`]+`\)\s*$/m, "catch-all host route missing");
  requirePattern(
    catchAll,
    /^\s*middlewares:\s*\n\s*- name: authentik\s*$/m,
    "catch-all route must use Authentik",
  );
}

function bracedBlock(source: string, header: RegExp, name: string): string {
  const match = header.exec(source);
  if (match?.index === undefined) refuse(`${name} block missing`);
  const start = source.indexOf("{", match.index);
  let depth = 0;
  for (let index = start; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(start + 1, index);
  }
  return refuse(`${name} block is unclosed`);
}

function normalized(source: string): string {
  return source.replace(/\s+/g, " ").trim();
}

function requireSnippet(source: string, snippet: string, message: string): void {
  if (!normalized(source).includes(normalized(snippet))) refuse(message);
}

function assertNginx(source: string): void {
  const active = activeLines(source);
  const exact = bracedBlock(
    active,
    /^\s*location\s*=\s*\/api\/v1\/webhooks\s*\{/m,
    "exact webhook management location",
  );
  const ingest = bracedBlock(
    active,
    /^\s*location\s+\/api\/v1\/webhooks\/\s*\{/m,
    "signature webhook ingest location",
  );
  const webhookLocations = active.match(
    /^\s*location\s+(?:=\s*)?\/api\/v1\/webhooks\/?\s*\{/gm,
  ) ?? [];
  if (webhookLocations.length !== 2) refuse("webhook locations must remain exact and distinct");

  requireSnippet(exact, 'set $deny "";', "management write gate state missing");
  requireSnippet(
    exact,
    'if ($is_write_method) { set $deny "W"; }',
    "management write-method gate missing",
  );
  requireSnippet(
    exact,
    'if ($is_machine) { set $deny "${deny}M"; }',
    "management machine gate missing",
  );
  requireSnippet(exact, 'if ($deny = "W") { return 403; }', "management denial missing");
  requirePattern(
    ingest,
    /proxy_pass\s+http:\/\/127\.0\.0\.1:43307\s*;/,
    "signature webhook ingest proxy missing",
  );
  if (/\$(?:is_write_method|is_machine|deny)\b/.test(ingest)) {
    refuse("signature webhook ingest must remain distinct from the management gate");
  }

  const writeMap = bracedBlock(
    active,
    /^\s*map\s+\$request_method\s+\$is_write_method\s*\{/m,
    "write-method map",
  );
  requirePattern(writeMap, /\bdefault\s+1\s*;/, "write methods must default to denied");
  for (const method of ["GET", "HEAD", "OPTIONS"]) {
    requirePattern(
      writeMap,
      new RegExp(`\\b${method}\\s+0\\s*;`),
      `${method} must remain read-only`,
    );
  }

  const machineMap = bracedBlock(
    active,
    /^\s*map\s+\$http_x_authentik_uid\s+\$is_machine\s*\{/m,
    "machine identity map",
  );
  requirePattern(machineMap, /\bdefault\s+0\s*;/, "identities must default to non-machine");
  requirePattern(machineMap, /"[a-f0-9]{64}"\s+1\s*;/, "machine identity admission missing");
}

function serviceSection(source: string, name: string): string[] {
  const lines = activeLines(source).split("\n");
  const start = lines.indexOf(`[${name}]`);
  if (start < 0) refuse(`[${name}] section missing`);
  const following = lines.slice(start + 1);
  const end = following.findIndex((line) => /^\[[^\]]+\]$/.test(line));
  return (end < 0 ? following : following.slice(0, end)).map((line) => line.trim()).filter(Boolean);
}

function directive(lines: string[], key: string): string {
  const values = lines.filter((line) => line.startsWith(`${key}=`));
  if (values.length !== 1) refuse(`${key} must occur exactly once`);
  return values[0].slice(key.length + 1);
}

function assertService(source: string): void {
  const serviceLines = serviceSection(source, "Service");
  const installLines = serviceSection(source, "Install");
  const exact = new Map([
    ["User", "santi"],
    ["Group", "santi"],
    ["PAMName", "login"],
    ["EnvironmentFile", "-/etc/santi/santi.env"],
    ["Environment", "SANTI_HOME=/home/santi/.santi"],
    ["ExecStart", "/usr/bin/santi-api serve"],
    ["KillSignal", "SIGTERM"],
    ["Restart", "on-failure"],
  ]);
  for (const [key, expected] of exact) {
    if (directive(serviceLines, key) !== expected) refuse(`${key} contract changed`);
  }

  const restartSeconds = Number(directive(serviceLines, "RestartSec").replace(/s$/, ""));
  if (!Number.isFinite(restartSeconds) || restartSeconds <= 0) {
    refuse("RestartSec must be positive");
  }
  const stopSeconds = Number(directive(serviceLines, "TimeoutStopSec").replace(/s$/, ""));
  if (!Number.isFinite(stopSeconds) || stopSeconds < 30) {
    refuse("TimeoutStopSec must be at least 30s");
  }
  if (directive(installLines, "WantedBy") !== "multi-user.target") {
    refuse("santi.service must be enabled for multi-user");
  }
}

function assertPostinst(source: string): void {
  const active = activeLines(source);
  const branches = [...active.matchAll(/^\s*([A-Za-z0-9_|-]+)\)\s*$/gm)].map((match) => match[1]);
  if (branches.length !== 1 || branches[0] !== "configure") {
    refuse("postinst must be configure-only");
  }

  const required = [
    [/getent\s+group\s+santi/, "system group lookup missing"],
    [/addgroup\s+--system\s+santi/, "system group provisioning missing"],
    [/getent\s+passwd\s+santi/, "system user lookup missing"],
    [
      /adduser\s+--system\s+--ingroup\s+santi\s+--home\s+\/home\/santi[\s\\]+--shell\s+\/usr\/sbin\/nologin\s+--disabled-password\s+santi/,
      "system user provisioning missing",
    ],
    [/mkdir\s+-p\s+\/etc\/santi/, "config directory provisioning missing"],
    [/chmod\s+755\s+\/etc\/santi/, "config directory mode missing"],
    [/mkdir\s+-p\s+\/home\/santi\/\.santi/, "runtime home provisioning missing"],
    [/chown\s+-R\s+santi:santi\s+\/home\/santi\/\.santi/, "runtime ownership missing"],
    [
      /if\s+command\s+-v\s+loginctl[^\n]*&&\s*\[\s+-d\s+\/run\/systemd\/system\s*\]\s*;\s*then[\s\S]*?loginctl\s+enable-linger\s+santi[\s\S]*?fi/,
      "conditional lingering enablement missing",
    ],
    [/systemctl\s+daemon-reload\s*\|\|\s*true/, "daemon reload missing"],
    [/systemctl\s+enable\s+santi\.service\s*\|\|\s*true/, "service enablement missing"],
  ] as const;
  for (const [pattern, message] of required) requirePattern(active, pattern, message);

  if (/^\s*(?:rm|rmdir)\b/m.test(active) || /\bfind\b[^\n]*\s-delete\b/.test(active)) {
    refuse("postinst must not delete preserved state");
  }
  if (active.includes("/etc/santi/santi.env")) refuse("postinst must not touch operator secrets");
  if (
    /\bsystemctl\s+(?:(?:--[^\s]+)\s+)*(?:start|restart|try-restart|reload-or-restart)\b/.test(
      active,
    ) ||
    /\bsystemctl\s+(?:enable[^\n]*--now|--now\s+enable)\b/.test(active) ||
    /\bservice\s+santi(?:\.service)?\s+(?:start|restart)\b/.test(active)
  ) {
    refuse("postinst must never start or restart santi.service");
  }
}

export function assertSantiContract(assets: SantiContractAssets): void {
  assertIngress(assets.ingress);
  assertNginx(assets.nginx);
  assertService(assets.service);
  assertPostinst(assets.postinst);
}

assertSantiContract({ ingress, nginx, postinst, service });
