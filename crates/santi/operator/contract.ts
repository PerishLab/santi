import { contract } from "@perish/sealkit/operator";
import ingress from "./ops/edge/ingress.yaml" with { type: "text" };
import nginx from "./ops/edge/resources/nginx.conf" with { type: "text" };
import postinst from "./packaging/deb/postinst" with { type: "text" };
import service from "./packaging/deb/root/lib/systemd/system/santi.service" with {
  type: "text",
};

contract.webhook({ ingress, nginx, collection: "/api/v1/webhooks" });
contract.deb({ service, postinst, user: "santi", unit: "santi.service" });
