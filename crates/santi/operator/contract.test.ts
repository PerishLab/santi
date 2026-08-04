import { assertSantiContract } from "./contract.ts";
import ingress from "./ops/edge/ingress.yaml" with { type: "text" };
import nginx from "./ops/edge/resources/nginx.conf" with { type: "text" };
import postinst from "./packaging/deb/postinst" with { type: "text" };
import service from "./packaging/deb/root/lib/systemd/system/santi.service" with {
  type: "text",
};

const real = { ingress, nginx, postinst, service };

function mutation(source: string, before: string, after: string): string {
  if (!source.includes(before)) throw new Error(`test mutation source missing: ${before}`);
  return source.replace(before, after);
}

function assertRefuses(
  asset: keyof typeof real,
  before: string,
  after: string,
): void {
  let refused = false;
  try {
    assertSantiContract({ ...real, [asset]: mutation(real[asset], before, after) });
  } catch {
    refused = true;
  }
  if (!refused) throw new Error(`${asset} mutation unexpectedly passed: ${before}`);
}

Deno.test("operator contract accepts the tracked delivery assets", () => {
  assertSantiContract(real);
});

Deno.test("ingress guard preserves the public webhook and protected catch-all boundary", () => {
  const cases = [
    ["PathPrefix(`/api/v1/webhooks/`)", "PathPrefix(`/api/v1/webhooks`)"],
    [
      "      kind: Rule\n      priority: 20",
      "      kind: Rule\n      middlewares:\n        - name: authentik\n      priority: 20",
    ],
    [
      "      middlewares:\n        - name: authentik",
      "      middlewares:\n        - name: unprotected",
    ],
  ];
  for (const [before, after] of cases) assertRefuses("ingress", before, after);
});

Deno.test("nginx guard preserves management gating and signature ingest", () => {
  const cases = [
    ["location = /api/v1/webhooks {", "location /api/v1/webhooks {"],
    ['if ($is_write_method) { set $deny "W"; }', "# removed write predicate"],
    ['if ($is_machine)      { set $deny "${deny}M"; }', "# removed machine predicate"],
    ['if ($deny = "W")      { return 403; }', "# removed rejection"],
    ["location /api/v1/webhooks/ {", "location /api/v1/webhooks {"],
    ["map $request_method $is_write_method {", "map $request_method $other {"],
    ["map $http_x_authentik_uid $is_machine {", "map $http_x_authentik_uid $other {"],
  ];
  for (const [before, after] of cases) assertRefuses("nginx", before, after);
});

Deno.test("systemd guard preserves runtime identity, shutdown, restart, and enablement", () => {
  const cases = [
    ["User=santi", "User=root"],
    ["Group=santi", "Group=root"],
    ["PAMName=login", "PAMName=other"],
    ["EnvironmentFile=-/etc/santi/santi.env", "EnvironmentFile=/etc/santi/santi.env"],
    ["Environment=SANTI_HOME=/home/santi/.santi", "Environment=SANTI_HOME=/tmp/santi"],
    ["ExecStart=/usr/bin/santi-api serve", "ExecStart=/usr/bin/santi-api"],
    ["KillSignal=SIGTERM", "KillSignal=SIGKILL"],
    ["Restart=on-failure", "Restart=no"],
    ["RestartSec=2", "RestartSec=0"],
    ["TimeoutStopSec=50", "TimeoutStopSec=29"],
    ["WantedBy=multi-user.target", "WantedBy=default.target"],
  ];
  for (const [before, after] of cases) assertRefuses("service", before, after);
});

Deno.test("postinst guard preserves configure-only provisioning without activation", () => {
  const cases = [
    ["configure)", "configure|upgrade)"],
    ["        addgroup --system santi", "        # addgroup --system santi"],
    [
      "        adduser --system --ingroup santi --home /home/santi \\",
      "        # user creation removed \\",
    ],
    ["    mkdir -p /etc/santi", "    # mkdir -p /etc/santi"],
    ["    mkdir -p /home/santi/.santi", "    # mkdir -p /home/santi/.santi"],
    [" && [ -d /run/systemd/system ]", " || [ -d /run/systemd/system ]"],
    ["        systemctl daemon-reload || true", "        # daemon reload removed"],
    [
      "        systemctl enable santi.service || true",
      "        systemctl enable other.service || true",
    ],
    ["\nexit 0", "\nrm -rf /home/santi/.santi\nexit 0"],
    ["\nexit 0", "\nprintf bad > /etc/santi/santi.env\nexit 0"],
    ["\nexit 0", "\nsystemctl start santi.service\nexit 0"],
    ["\nexit 0", "\nsystemctl enable --now santi.service\nexit 0"],
  ];
  for (const [before, after] of cases) assertRefuses("postinst", before, after);
});
